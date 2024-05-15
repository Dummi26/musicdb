use std::net::SocketAddr;
use std::sync::{mpsc, Arc, Mutex};

use musicdb_lib::data::album::Album;
use musicdb_lib::data::artist::Artist;
use musicdb_lib::data::database::Database;
use musicdb_lib::data::queue::{QueueContent, QueueFolder};
use musicdb_lib::data::song::Song;
use musicdb_lib::data::SongId;
use musicdb_lib::server::Command;
use rocket::response::content::RawHtml;
use rocket::{get, routes, Config, State};

/*

23E9 ⏩︎ fast forward
23EA ⏪︎ rewind, fast backwards
23EB ⏫︎ fast increase
23EC ⏬︎ fast decrease
23ED ⏭︎ skip to end, next
23EE ⏮︎ skip to start, previous
23EF ⏯︎ play/pause toggle
23F1 ⏱︎ stopwatch
23F2 ⏲︎ timer clock
23F3 ⏳︎ hourglass
23F4 ⏴︎ reverse, back
23F5 ⏵︎ forward, next, play
23F6 ⏶︎ increase
23F7 ⏷︎ decrease
23F8 ⏸︎ pause
23F9 ⏹︎ stop
23FA ⏺︎ record

*/

const HTML_START: &'static str =
    "<!DOCTYPE html><html><head><meta name=\"color-scheme\" content=\"light dark\">";
const HTML_SEP: &'static str = "</head><body>";
const HTML_END: &'static str = "</body></html>";

struct Data {
    db: Arc<Mutex<Database>>,
    command_sender: mpsc::Sender<Command>,
}

#[get("/")]
async fn index(data: &State<Data>) -> RawHtml<String> {
    let script = r#"<script>
async function performSearch() {
    var searchResultDiv = document.getElementById("searchResultDiv");
    searchResultDiv.innerHTML = "Loading...";
    var sfArtist = document.getElementById("searchFieldArtist").value;
    var sfAlbum = document.getElementById("searchFieldAlbum").value;
    var sfTitle = document.getElementById("searchFieldTitle").value;
    var query = "";
    if (sfArtist) {
        query += "artist=" + encodeURIComponent(sfArtist);
    }
    if (sfAlbum) {
        if (query) {
            query += "&";
        }
        query += "album=" + encodeURIComponent(sfAlbum);
    }
    if (sfTitle) {
        if (query) {
            query += "&";
        }
        query += "title=" + encodeURIComponent(sfTitle);
    }
    if (query || confirm("You didn't search for anything specific. If you continue, the whole library will be loaded, which can take a while and use a lot of bandwidth!")) {
        console.log("Performing search with query '" + query + "'.");
        var r1 = await fetch("/search?" + query);
        var r2 = await r1.text();
        searchResultDiv.innerHTML = r2;
    } else {
        searchResultDiv.innerHTML = "";
    }
}
async function addSong(id) {
    await fetch("/add-song/" + id);
}
</script>"#;
    let buttons = "<button onclick=\"fetch('/play').then(() => location.reload())\">play</button><button onclick=\"fetch('/pause').then(() => location.reload())\">pause</button><button onclick=\"fetch('/skip').then(() => location.reload())\">skip</button><button onclick=\"fetch('/clear-queue').then(() => location.reload())\">clear queue</button><button onclick=\"location.reload()\">reload</button>";
    let search = "<input id=\"searchFieldArtist\" placeholder=\"artist\"><input id=\"searchFieldAlbum\" placeholder=\"album\"><input id=\"searchFieldTitle\" placeholder=\"title\">
<button onclick=\"performSearch()\">search</button><div id=\"searchResultDiv\"></div>";
    let db = data.db.lock().unwrap();
    let now_playing =
        if let Some(current_song) = db.queue.get_current_song().and_then(|id| db.get_song(id)) {
            format!(
                "<h1>Now Playing</h1><h4>{}</h4>",
                html_escape::encode_safe(&current_song.title),
            )
        } else {
            format!("<h1>Now Playing</h1><p>nothing</p>",)
        };
    drop(db);
    RawHtml(format!(
        "{HTML_START}<title>MusicDb</title>{script}{HTML_SEP}{now_playing}<div>{buttons}</div><div>{search}</div>{HTML_END}",
    ))
}

#[get("/play")]
async fn play(data: &State<Data>) {
    data.command_sender.send(Command::Resume).unwrap();
}
#[get("/pause")]
async fn pause(data: &State<Data>) {
    data.command_sender.send(Command::Pause).unwrap();
}
#[get("/skip")]
async fn skip(data: &State<Data>) {
    data.command_sender.send(Command::NextSong).unwrap();
}
#[get("/clear-queue")]
async fn clear_queue(data: &State<Data>) {
    data.command_sender
        .send(Command::QueueUpdate(
            vec![],
            QueueContent::Folder(QueueFolder {
                index: 0,
                content: vec![],
                name: String::new(),
                order: None,
            })
            .into(),
        ))
        .unwrap();
}

#[get("/add-song/<id>")]
async fn add_song(data: &State<Data>, id: SongId) {
    data.command_sender
        .send(Command::QueueAdd(
            vec![],
            vec![QueueContent::Song(id).into()],
        ))
        .unwrap();
}

#[get("/search?<artist>&<album>&<title>&<artist_tags>&<album_tags>&<song_tags>")]
async fn search(
    data: &State<Data>,
    artist: Option<&str>,
    album: Option<&str>,
    title: Option<&str>,
    artist_tags: Vec<&str>,
    album_tags: Vec<&str>,
    song_tags: Vec<&str>,
) -> RawHtml<String> {
    let db = data.db.lock().unwrap();
    let mut out = String::new();
    let artist = artist.map(|v| v.to_lowercase());
    let artist = artist.as_ref().map(|v| v.as_str());
    let album = album.map(|v| v.to_lowercase());
    let album = album.as_ref().map(|v| v.as_str());
    let title = title.map(|v| v.to_lowercase());
    let title = title.as_ref().map(|v| v.as_str());
    find1(
        &*db,
        artist,
        album,
        title,
        &artist_tags,
        &album_tags,
        &song_tags,
        &mut out,
    );
    fn find1(
        db: &Database,
        artist: Option<&str>,
        album: Option<&str>,
        title: Option<&str>,
        artist_tags: &[&str],
        album_tags: &[&str],
        song_tags: &[&str],
        out: &mut String,
    ) {
        if let Some(f) = artist {
            find2(
                db,
                db.artists()
                    .values()
                    .filter(|v| v.name.to_lowercase().contains(f)),
                album,
                title,
                artist_tags,
                album_tags,
                song_tags,
                out,
            )
        } else {
            find2(
                db,
                db.artists().values(),
                album,
                title,
                artist_tags,
                album_tags,
                song_tags,
                out,
            )
        }
    }
    fn find2<'a>(
        db: &'a Database,
        artists: impl IntoIterator<Item = &'a Artist>,
        album: Option<&str>,
        title: Option<&str>,
        artist_tags: &[&str],
        album_tags: &[&str],
        song_tags: &[&str],
        out: &mut String,
    ) {
        for artist in artists {
            if artist_tags
                .iter()
                .all(|t| artist.general.tags.iter().any(|v| v == t))
            {
                let mut func_artist = Some(|out: &mut String| {
                    out.push_str("<h3>");
                    out.push_str(&artist.name);
                    out.push_str("</h3>");
                });
                let mut func_album = None;
                if false {
                    // so they have the same type
                    std::mem::swap(&mut func_artist, &mut func_album);
                }
                if album.is_none() && album_tags.is_empty() {
                    find4(
                        db,
                        artist.singles.iter().filter_map(|v| db.get_song(v)),
                        title,
                        song_tags,
                        out,
                        &mut func_artist,
                        &mut func_album,
                    );
                }
                let iter = artist.albums.iter().filter_map(|v| db.albums().get(v));
                if let Some(f) = album {
                    find3(
                        db,
                        iter.filter(|v| v.name.to_lowercase().contains(f)),
                        title,
                        album_tags,
                        song_tags,
                        out,
                        &mut func_artist,
                    )
                } else {
                    find3(
                        db,
                        iter,
                        title,
                        album_tags,
                        song_tags,
                        out,
                        &mut func_artist,
                    )
                }
            }
        }
    }
    fn find3<'a>(
        db: &'a Database,
        albums: impl IntoIterator<Item = &'a Album>,
        title: Option<&str>,
        album_tags: &[&str],
        song_tags: &[&str],
        out: &mut String,
        func_artist: &mut Option<impl FnOnce(&'_ mut String)>,
    ) {
        for album in albums {
            if album_tags
                .iter()
                .all(|t| album.general.tags.iter().any(|v| v == t))
            {
                let mut func_album = Some(|out: &mut String| {
                    out.push_str("<h4>");
                    out.push_str(&album.name);
                    out.push_str("</h4>");
                });
                find4(
                    db,
                    album.songs.iter().filter_map(|v| db.get_song(v)),
                    title,
                    song_tags,
                    out,
                    func_artist,
                    &mut func_album,
                )
            }
        }
    }
    fn find4<'a>(
        db: &'a Database,
        songs: impl IntoIterator<Item = &'a Song>,
        title: Option<&str>,
        song_tags: &[&str],
        out: &mut String,
        func_artist: &mut Option<impl FnOnce(&'_ mut String)>,
        func_album: &mut Option<impl FnOnce(&'_ mut String)>,
    ) {
        if let Some(f) = title {
            find5(
                db,
                songs
                    .into_iter()
                    .filter(|v| v.title.to_lowercase().contains(f)),
                song_tags,
                out,
                func_artist,
                func_album,
            )
        } else {
            find5(db, songs, song_tags, out, func_artist, func_album)
        }
    }
    fn find5<'a>(
        db: &'a Database,
        songs: impl IntoIterator<Item = &'a Song>,
        song_tags: &[&str],
        out: &mut String,
        func_artist: &mut Option<impl FnOnce(&'_ mut String)>,
        func_album: &mut Option<impl FnOnce(&'_ mut String)>,
    ) {
        for song in songs {
            if song_tags
                .iter()
                .all(|t| song.general.tags.iter().any(|v| v == t))
            {
                find6(db, song, out, func_artist, func_album)
            }
        }
    }
    fn find6<'a>(
        _db: &Database,
        song: &Song,
        out: &mut String,
        func_artist: &mut Option<impl FnOnce(&'_ mut String)>,
        func_album: &mut Option<impl FnOnce(&'_ mut String)>,
    ) {
        if let Some(f) = func_artist.take() {
            f(out)
        }
        if let Some(f) = func_album.take() {
            f(out)
        }
        out.push_str("<button onclick=\"addSong('");
        out.push_str(&format!("{}", song.id));
        out.push_str("')\">");
        out.push_str(&song.title);
        out.push_str("</button><br>");
    }
    RawHtml(out)
}

pub async fn main(
    db: Arc<Mutex<Database>>,
    command_sender: mpsc::Sender<Command>,
    addr: SocketAddr,
) {
    rocket::build()
        .configure(Config {
            address: addr.ip(),
            port: addr.port(),
            ..Default::default()
        })
        .manage(Data { db, command_sender })
        .mount(
            "/",
            routes![index, play, pause, skip, clear_queue, add_song, search],
        )
        .launch()
        .await
        .unwrap();
}
