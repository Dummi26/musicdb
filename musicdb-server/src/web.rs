use std::net::SocketAddr;
use std::sync::{mpsc, Arc, Mutex};

use musicdb_lib::data::album::Album;
use musicdb_lib::data::artist::Artist;
use musicdb_lib::data::database::Database;
use musicdb_lib::data::queue::{Queue, QueueContent, QueueFolder};
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
    "<!DOCTYPE html><html><head><meta charset=\"UTF-8\"><meta name=\"color-scheme\" content=\"light dark\">";
const HTML_SEP: &'static str = "</head><body>";
const HTML_END: &'static str = "</body></html>";

struct Data {
    db: Arc<Mutex<Database>>,
    command_sender: mpsc::Sender<Command>,
}

#[get("/")]
fn index(data: &State<Data>) -> RawHtml<String> {
    dbg!(());
    let script = r#"<script>
const sleep = ms => new Promise(r => setTimeout(r, ms));
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
    if (query || confirm("You didn't search for anything specific. If you continue, the whole library will be loaded, which can take a while, use a lot of bandwidth, and may crash your browser!")) {
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
    let script2 = r#"<script>
const searchDiv = document.getElementById("searchDiv");
searchDiv.style.display = "";
document.getElementById("warnLag").innerText = "connecting...";
const nowPlayingDiv = document.getElementById("nowPlayingDiv");
const queueDiv = document.getElementById("queueDiv");
var didFinish = false;
var averageLoopTimeMs = 250;
async function runLoop() {
    while (true) {
        await sleep(1000);
        didFinish = false;
        var startTime = new Date();
        sleep(averageLoopTimeMs*2).then(async function() {
            while (!didFinish) {
                var elapsed = new Date() - startTime;
                document.getElementById("warnLag").innerText = "Warning: slow connection, server may be busy. be patient. (" + Math.round(averageLoopTimeMs) + "ms exceeded by " + Math.round((elapsed-averageLoopTimeMs)/averageLoopTimeMs) + "x)";
                await sleep(100);
            }
        });
        nowPlayingDiv.innerHTML = await (await fetch("/now-playing-html")).text();
        queueDiv.innerHTML = await (await fetch("/queue-html")).text();
        var elapsedTime = new Date() - startTime;
        didFinish = true;
        averageLoopTimeMs = ((averageLoopTimeMs * 4) + elapsedTime) / 5;
        document.getElementById("warnLag").innerText = "Average update time: " + Math.round(averageLoopTimeMs) + "ms";
    }
}
runLoop();</script>"#;
    let buttons = "<button onclick=\"fetch('/play')\">play</button><button onclick=\"fetch('/pause')\">pause</button><button onclick=\"fetch('/skip')\">skip</button><button onclick=\"fetch('/clear-queue')\">clear queue</button>";
    let search = "<input id=\"searchFieldArtist\" placeholder=\"artist\"><input id=\"searchFieldAlbum\" placeholder=\"album\"><input id=\"searchFieldTitle\" placeholder=\"title\">
<button onclick=\"performSearch()\">search</button><div id=\"searchResultDiv\"></div>";
    let db = data.db.lock().unwrap();
    let now_playing = gen_now_playing(&db);
    let mut queue = String::new();
    gen_queue_html(&db.queue, &mut queue, &db);
    dbg!(&queue);
    drop(db);
    RawHtml(format!(
        "{HTML_START}<title>MusicDb</title>{script}{HTML_SEP}<div id=\"warnLag\">no javascript? reload to see updated information.</div><div id=\"nowPlayingDiv\">{now_playing}</div><div>{buttons}</div><div id=\"searchDiv\" style=\"display:none;\">{search}</div><div id=\"queueDiv\">{queue}</div>{script2}{HTML_END}",
    ))
}
#[get("/now-playing-html")]
fn now_playing_html(data: &State<Data>) -> RawHtml<String> {
    RawHtml(gen_now_playing(&*data.db.lock().unwrap()))
}
#[get("/queue-html")]
fn queue_html(data: &State<Data>) -> RawHtml<String> {
    let mut str = String::new();
    let db = data.db.lock().unwrap();
    gen_queue_html(&db.queue, &mut str, &db);
    RawHtml(str)
}
fn gen_now_playing(db: &Database) -> String {
    if let Some(current_song) = db.queue.get_current_song().and_then(|id| db.get_song(id)) {
        format!(
            "<h1>Now Playing</h1><h4>{}</h4>",
            html_escape::encode_safe(&current_song.title),
        )
    } else {
        format!("<h1>Now Playing</h1><p>nothing</p>",)
    }
}
fn gen_queue_html(queue: &Queue, str: &mut String, db: &Database) {
    gen_queue_html_impl(queue, str, db, true, &mut "".to_owned());
}
fn gen_queue_html_impl(
    queue: &Queue,
    str: &mut String,
    db: &Database,
    active_highlight: bool,
    path: &mut String,
) {
    match queue.content() {
        QueueContent::Song(id) => {
            if let Some(song) = db.songs().get(id) {
                str.push_str("<div>");
                if active_highlight {
                    str.push_str("<b>");
                }
                str.push_str(&format!("<button onclick=\"fetch('/queue-goto/{path}')\">"));
                str.push_str(&html_escape::encode_text(&song.title));
                str.push_str("</button>");
                if active_highlight {
                    str.push_str("</b>");
                }
                str.push_str("<small>");
                if let Some(artist) = db.artists().get(&song.artist) {
                    str.push_str(" by ");
                    str.push_str(&html_escape::encode_text(&artist.name));
                }
                if let Some(album) = song.album.as_ref().and_then(|id| db.albums().get(id)) {
                    str.push_str(" on ");
                    str.push_str(&html_escape::encode_text(&album.name));
                }
                str.push_str(&format!(
                    "<button onclick=\"fetch('/queue-remove/{path}')\">rm</button>"
                ));
                str.push_str("</small></div>");
            } else {
                str.push_str("<div><small>unknown song</small></div>");
            }
        }
        QueueContent::Folder(f) => {
            let html_shuf: &'static str = " <small><small>shuffled</small></small>";
            if f.content.is_empty() {
                str.push_str("[0/0] ");
                if active_highlight {
                    str.push_str("<b>");
                }
                str.push_str(&html_escape::encode_text(&f.name));
                if active_highlight {
                    str.push_str("</b>");
                }
                if f.order.is_some() {
                    str.push_str(html_shuf);
                }
            } else {
                str.push_str(&format!("[{}/{}] ", f.index + 1, f.content.len(),));
                if active_highlight {
                    str.push_str("<b>");
                }
                str.push_str(&html_escape::encode_text(&f.name));
                if active_highlight {
                    str.push_str("</b>");
                }
                if f.order.is_some() {
                    str.push_str(html_shuf);
                }
                str.push_str("<ol>");
                for (i, v) in f.iter().enumerate() {
                    str.push_str("<li>");
                    if !path.is_empty() {
                        path.push('_');
                    }
                    path.push_str(&format!("{i}"));
                    gen_queue_html_impl(v, str, db, active_highlight && i == f.index, path);
                    while !(path.is_empty() || path.ends_with('_')) {
                        path.pop();
                    }
                    path.pop();
                    str.push_str("</li>");
                }
                str.push_str("</ol>");
            }
        }
        QueueContent::Loop(d, t, i) => {
            if active_highlight {
                str.push_str("<b>");
            }
            if *t == 0 {
                str.push_str(&format!("<small>[{}/&infin;]</small>", d + 1));
            } else {
                str.push_str(&format!("<small>[{}/{}]</small>", d + 1, t));
            }
            if active_highlight {
                str.push_str("</b>");
            }
            if !path.is_empty() {
                path.push('_');
            }
            path.push('0');
            gen_queue_html_impl(i, str, db, active_highlight, path);
            while !(path.is_empty() || path.ends_with('_')) {
                path.pop();
            }
            path.pop();
        }
    }
}

#[get("/queue-remove/<path>")]
fn queue_remove(data: &State<Data>, path: &str) {
    if let Some(path) = path.split('_').map(|v| v.parse().ok()).collect() {
        data.command_sender
            .send(Command::QueueRemove(path))
            .unwrap();
    }
}
#[get("/queue-goto/<path>")]
fn queue_goto(data: &State<Data>, path: &str) {
    if let Some(path) = path.split('_').map(|v| v.parse().ok()).collect() {
        data.command_sender.send(Command::QueueGoto(path)).unwrap();
    }
}

#[get("/play")]
fn play(data: &State<Data>) {
    data.command_sender.send(Command::Resume).unwrap();
}
#[get("/pause")]
fn pause(data: &State<Data>) {
    data.command_sender.send(Command::Pause).unwrap();
}
#[get("/skip")]
fn skip(data: &State<Data>) {
    data.command_sender.send(Command::NextSong).unwrap();
}
#[get("/clear-queue")]
fn clear_queue(data: &State<Data>) {
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
fn add_song(data: &State<Data>, id: SongId) {
    data.command_sender
        .send(Command::QueueAdd(
            vec![],
            vec![QueueContent::Song(id).into()],
        ))
        .unwrap();
}

#[get("/search?<artist>&<album>&<title>&<artist_tags>&<album_tags>&<song_tags>")]
fn search(
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
            routes![
                index,
                play,
                pause,
                skip,
                clear_queue,
                queue_goto,
                queue_remove,
                add_song,
                search,
                now_playing_html,
                queue_html
            ],
        )
        .launch()
        .await
        .unwrap();
}
