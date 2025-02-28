use std::net::SocketAddr;
use std::sync::{mpsc, Arc, Mutex};

use musicdb_lib::data::album::Album;
use musicdb_lib::data::artist::Artist;
use musicdb_lib::data::database::{Database, UpdateEndpoint};
use musicdb_lib::data::queue::{Queue, QueueContent, QueueFolder};
use musicdb_lib::data::song::Song;
use musicdb_lib::data::SongId;
use musicdb_lib::server::{Action, Command, Req};
use rocket::futures::{SinkExt, StreamExt};
use rocket::http::ContentType;
use rocket::response::content::RawHtml;
use rocket::response::Responder;
use rocket::{get, routes, Config, Response, State};
use rocket_seek_stream::SeekStream;
use rocket_ws::{Message, WebSocket};
use tokio::select;
use tokio::sync::mpsc::Sender;

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
    command_sender: mpsc::Sender<(Command, Option<u64>)>,
    websocket_connections: Arc<tokio::sync::Mutex<Vec<Sender<Message>>>>,
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
var dbPlaying = null;
function showDbPlaying() {
    if (dbPlaying === true) {
        document.getElementById("playLiveCurrent").play();
    } else if (dbPlaying === false) {
        document.getElementById("playLiveCurrent").pause();
    } else {
        document.getElementById("playLiveCurrent").pause();
    }
    const postheader = document.getElementById("postheader");
    if (postheader) {
        if (dbPlaying === true) {
            postheader.innerText = "";
        } else if (dbPlaying === false) {
            postheader.innerText = "(paused)";
        } else {
            postheader.innerText = "";
        }
    }
}
async function updateNowPlaying() {
    nowPlayingDiv.innerHTML = await (await fetch("/now-playing-html")).text();
    showDbPlaying();
}
async function updateQueue() {
    queueDiv.innerHTML = await (await fetch("/queue-html")).text();
}
var livePlaybackCurrentId = null;
var livePlaybackNextId = null;
async function updateLivePlaybackIds() {
    if (document.getElementById("playLiveEnabled").checked) {
        let resp = (await (await fetch("/now-playing-ids")).text()).trim();
        let current = null;
        let next = null;
        if (resp != "") {
            if (resp.includes("/")) {
                let [c, n] = resp.split("/");
                current = c.trim();
                next = n.trim();
            } else {
                current = resp;
            }
        }
        if (current !== livePlaybackCurrentId) {
            livePlaybackCurrentId = current;
            document.getElementById("playLiveCurrentSrc").src = livePlaybackCurrentId == null ? "" : "/song/" + livePlaybackCurrentId + "/current";
            let audioElem = document.getElementById("playLiveCurrent");
            audioElem.pause();
            audioElem.currentTime = 0;
            if (dbPlaying) {
                audioElem.setAttribute("autoplay", "");
            } else {
                audioElem.removeAttribute("autoplay");
            }
            audioElem.load();
            if (dbPlaying) {
                audioElem.play();
            }
        }
        if (next !== livePlaybackNextId) {
            livePlaybackNextId = next;
            document.getElementById("playLiveNextSrc").src = livePlaybackNextId == null ? "" : "/song/" + livePlaybackNextId + "/next";
            document.getElementById("playLiveNext").load();
        }
    } else {
        if (livePlaybackCurrentId !== null) {
            livePlaybackCurrentId = null;
            let audioElem = document.getElementById("playLiveCurrent");
            audioElem.pause();
            audioElem.currentTime = 0;
            document.getElementById("playLiveCurrentSrc").src = "";
            audioElem.load();
        }
        if (livePlaybackNextId !== null) {
            livePlaybackNextId = null;
            document.getElementById("playLiveNextSrc").src = "";
            document.getElementById("playLiveNext").load();
        }
    }
}
async function runLoop() {
    let websocketConnection = null;
    try {
        websocketConnection = new WebSocket("/ws");
        var websocketCounter = 0;
        websocketConnection.addEventListener("message", async function(e) {
            ++websocketCounter;
            if (websocketCounter > 2) {
                websocketCounter = 0;
            }
            document.getElementById("warnLag").innerText = "using websocket" + (websocketCounter == 0 ? "." : (websocketCounter == 1 ? ".." : "..."));
            switch (e.data.trim()) {
                case "init/playing=true":
                    if (dbPlaying === null) {
                        dbPlaying = true;
                        showDbPlaying();
                    }
                    break;
                case "init/playing=false":
                    if (dbPlaying === null) {
                        dbPlaying = false;
                        showDbPlaying();
                    }
                    break;
                case "pause":
                    dbPlaying = false;
                    showDbPlaying();
                    break;
                case "stop":
                    dbPlaying = false;
                    document.getElementById("playLiveCurrent").pause();
                    document.getElementById("playLiveCurrent").currentTime = 0;
                    showDbPlaying();
                    break;
                case "resume":
                    dbPlaying = true;
                    showDbPlaying();
                    break;
                case "next":
                    await updateLivePlaybackIds();
                    await updateNowPlaying();
                    await updateQueue();
                    break;
                case "update/data":
                    await updateLivePlaybackIds();
                    await updateNowPlaying();
                    await updateQueue();
                    break;
                case "update/queue":
                    await updateLivePlaybackIds();
                    await updateNowPlaying();
                    await updateQueue();
                    break;
                default:
                    console.log("Unknown websocket message: ", e.data.trim());
                    break;
            }
        });
        return;
    } catch (e) {
        console.log("Error in websocket connection:");
        console.log(e);
        console.log("Falling back to polling.");
        websocketConnection = null;
    }
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
        await updateNowPlaying();
        await updateQueue();
        var elapsedTime = new Date() - startTime;
        didFinish = true;
        averageLoopTimeMs = ((averageLoopTimeMs * 4) + elapsedTime) / 5;
        document.getElementById("warnLag").innerText = "Average update time: " + Math.round(averageLoopTimeMs) + "ms";
    }
}
runLoop();</script>"#;
    let buttons = "<button onclick=\"fetch('/play')\">play</button><button onclick=\"fetch('/pause')\">pause</button><button onclick=\"fetch('/stop')\">stop</button><button onclick=\"fetch('/skip')\">skip</button><button onclick=\"fetch('/clear-queue')\">clear queue</button>";
    let search = "<input id=\"searchFieldArtist\" placeholder=\"artist\"><input id=\"searchFieldAlbum\" placeholder=\"album\"><input id=\"searchFieldTitle\" placeholder=\"title\">
<button onclick=\"performSearch()\">search</button><div id=\"searchResultDiv\"></div>";
    let playback_live = r#"<div><input id="playLiveEnabled" onchange="updateLivePlaybackIds();" type="checkbox"><audio controls autoplay id="playLiveCurrent"><source id="playLiveCurrentSrc" src=""></audio><audio style="visibility:hidden;" id="playLiveNext"><source id="playLiveNextSrc" src=""></audio></span></div>"#;
    let db = data.db.lock().unwrap();
    let now_playing = gen_now_playing(&db);
    let mut queue = String::new();
    gen_queue_html(&db.queue, &mut queue, &db);
    dbg!(&queue);
    drop(db);
    RawHtml(format!(
        "{HTML_START}<title>MusicDb</title>{script}{HTML_SEP}<small><small><div id=\"warnLag\">no javascript? reload to see updated information.</div></small></small><div id=\"nowPlayingDiv\">{now_playing}</div><div>{buttons}</div>{playback_live}<div id=\"searchDiv\" style=\"display:none;\">{search}</div><div id=\"queueDiv\">{queue}</div>{script2}{HTML_END}",
    ))
}
#[get("/now-playing-html")]
fn now_playing_html(data: &State<Data>) -> RawHtml<String> {
    RawHtml(gen_now_playing(&*data.db.lock().unwrap()))
}
#[get("/now-playing-ids")]
fn now_playing_ids(data: &State<Data>) -> String {
    let db = data.db.lock().unwrap();
    let (c, n) = (
        db.queue.get_current_song().copied(),
        db.queue.get_next_song().copied(),
    );
    drop(db);
    if let Some(c) = c {
        if let Some(n) = n {
            format!("{c}/{n}")
        } else {
            format!("{c}")
        }
    } else {
        "".to_owned()
    }
}

#[get("/song/<id>/<name>")]
fn song(data: &State<Data>, id: SongId, name: String) -> Option<SeekStream> {
    let db = data.db.lock().unwrap();
    if let Some(song) = db.get_song(&id) {
        song.cached_data().cache_data_start_thread(&*db, song);
        if let Some(bytes) = song.cached_data().cached_data_await() {
            drop(db);
            Some(SeekStream::new(std::io::Cursor::new(ArcBytes(bytes))))
        } else {
            None
        }
    } else {
        None
    }
}
struct ArcBytes(pub Arc<Vec<u8>>);
impl AsRef<[u8]> for ArcBytes {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
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
            "<h1>Now Playing <small id=\"postheader\"></small></h1><h4>{}</h4>",
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
                str.push_str(&format!("<button onclick=\"fetch('/queue-goto/{path}')\">"));
                if active_highlight {
                    str.push_str("<b>");
                }
                str.push_str(&html_escape::encode_text(&song.title));
                if active_highlight {
                    str.push_str("</b>");
                }
                str.push_str("</button>");
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
            .send((Action::QueueRemove(path).cmd(0xFFu8), None))
            .unwrap();
    }
}
#[get("/queue-goto/<path>")]
fn queue_goto(data: &State<Data>, path: &str) {
    if let Some(path) = path.split('_').map(|v| v.parse().ok()).collect() {
        data.command_sender
            .send((Action::QueueGoto(path).cmd(0xFFu8), None))
            .unwrap();
    }
}

#[get("/play")]
fn play(data: &State<Data>) {
    data.command_sender
        .send((Action::Resume.cmd(0xFFu8), None))
        .unwrap();
}
#[get("/pause")]
fn pause(data: &State<Data>) {
    data.command_sender
        .send((Action::Pause.cmd(0xFFu8), None))
        .unwrap();
}
#[get("/stop")]
fn stop(data: &State<Data>) {
    data.command_sender
        .send((Action::Stop.cmd(0xFFu8), None))
        .unwrap();
}
#[get("/skip")]
fn skip(data: &State<Data>) {
    data.command_sender
        .send((Action::NextSong.cmd(0xFFu8), None))
        .unwrap();
}
#[get("/clear-queue")]
fn clear_queue(data: &State<Data>) {
    data.command_sender
        .send((
            Action::QueueUpdate(
                vec![],
                QueueContent::Folder(QueueFolder {
                    index: 0,
                    content: vec![],
                    name: String::new(),
                    order: None,
                })
                .into(),
                Req::none(),
            )
            .cmd(0xFFu8),
            None,
        ))
        .unwrap();
}

#[get("/add-song/<id>")]
fn add_song(data: &State<Data>, id: SongId) {
    data.command_sender
        .send((
            Action::QueueAdd(vec![], vec![QueueContent::Song(id).into()], Req::none()).cmd(0xFFu8),
            None,
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

#[get("/ws")]
async fn websocket(websocket: WebSocket, state: &State<Data>) -> rocket_ws::Channel<'static> {
    // a channel so other threads/tasks can send messages to this websocket client
    let (sender, mut receiver) = tokio::sync::mpsc::channel(5);
    state.websocket_connections.lock().await.push(sender);
    let (db_playing, ()) = tokio::task::block_in_place(|| {
        let db = state.db.lock().unwrap();
        (db.playing, ())
    });

    // handle messages from the websocket and from the channel
    websocket.channel(move |mut websocket| {
        Box::pin(async move {
            if db_playing {
                let _ = websocket.send(Message::text("init/playing=true")).await;
            } else {
                let _ = websocket.send(Message::text("init/playing=false")).await;
            }
            loop {
                // async magic:
                // handle a message from the websocket client or from other
                // threads/tasks in the server, whichever happens first
                select! {
                    message = websocket.next() => {
                        if let Some(message) = message {
                            // server received `message` from the websocket client
                            match message? {
                                Message::Text(text) => {
                                    // it was a text message, prefix it with "You sent: " and echo
                                    websocket
                                        .send(Message::text(format!("You sent: {text}")))
                                        .await?
                                }
                                Message::Binary(_bytes) => {
                                    // it was a binary message, ignore it
                                }
                                Message::Ping(payload) => {
                                    websocket.send(Message::Pong(payload)).await?
                                }
                                Message::Close(close) => {
                                    websocket.close(close).await?;
                                    break;
                                }
                                // these messages get ignored
                                Message::Pong(_) | Message::Frame(_) => (),
                            }
                        } else {
                            // websocket connection was closed
                            break;
                        }
                    },
                    message_to_be_sent = receiver.recv() => {
                        if let Some(message) = message_to_be_sent {
                            // server received `message` from another thread/task
                            websocket.send(message).await?;
                        } else {
                            // channel has been closed, close websocket connection too
                            websocket.close(None).await?;
                            break;
                        }
                    },
                }
            }
            Ok(())
        })
    })
}

pub fn main(
    db: Arc<Mutex<Database>>,
    command_sender: mpsc::Sender<(Command, Option<u64>)>,
    addr: SocketAddr,
) {
    let websocket_connections = Arc::new(tokio::sync::Mutex::new(vec![]));
    let data = Data {
        db: Arc::clone(&db),
        command_sender,
        websocket_connections: Arc::clone(&websocket_connections),
    };
    let mut db = db.lock().unwrap();
    let udepid = db.update_endpoints_id;
    db.update_endpoints_id += 1;
    db.update_endpoints.push((
        udepid,
        UpdateEndpoint::Custom(Box::new(move |cmd| {
            let mut msgs = vec![];
            fn action(a: &Action, msgs: &mut Vec<Message>) {
                match a {
                    Action::Resume => msgs.push(Message::text("resume")),
                    Action::Pause => msgs.push(Message::text("pause")),
                    Action::Stop => msgs.push(Message::text("stop")),
                    Action::NextSong => msgs.push(Message::text("next")),
                    Action::SyncDatabase(..)
                    | Action::AddSong(..)
                    | Action::AddAlbum(..)
                    | Action::AddArtist(..)
                    | Action::AddCover(..)
                    | Action::ModifySong(..)
                    | Action::ModifyAlbum(..)
                    | Action::ModifyArtist(..)
                    | Action::RemoveSong(..)
                    | Action::RemoveAlbum(..)
                    | Action::RemoveArtist(..)
                    | Action::SetSongDuration(..)
                    | Action::TagSongFlagSet(..)
                    | Action::TagSongFlagUnset(..)
                    | Action::TagAlbumFlagSet(..)
                    | Action::TagAlbumFlagUnset(..)
                    | Action::TagArtistFlagSet(..)
                    | Action::TagArtistFlagUnset(..)
                    | Action::TagSongPropertySet(..)
                    | Action::TagSongPropertyUnset(..)
                    | Action::TagAlbumPropertySet(..)
                    | Action::TagAlbumPropertyUnset(..)
                    | Action::TagArtistPropertySet(..)
                    | Action::TagArtistPropertyUnset(..) => msgs.push(Message::text("update/data")),
                    Action::QueueUpdate(..)
                    | Action::QueueAdd(..)
                    | Action::QueueInsert(..)
                    | Action::QueueRemove(..)
                    | Action::QueueMove(..)
                    | Action::QueueMoveInto(..)
                    | Action::QueueGoto(..)
                    | Action::QueueShuffle(..)
                    | Action::QueueSetShuffle(..)
                    | Action::QueueUnshuffle(..) => msgs.push(Message::text("update/queue")),
                    Action::Multiple(actions) => {
                        for inner in actions {
                            action(inner, msgs);
                        }
                    }
                    Action::InitComplete
                    | Action::Save
                    | Action::ErrorInfo(..)
                    | Action::Denied(..) => {}
                }
            }
            action(&cmd.action, &mut msgs);
            if !msgs.is_empty() {
                let mut ws_cons = websocket_connections.blocking_lock();
                let mut rm = vec![];
                for msg in msgs {
                    rm.clear();
                    for (i, con) in ws_cons.iter_mut().enumerate() {
                        if con.blocking_send(msg.clone()).is_err() {
                            rm.push(i);
                        }
                    }
                    for i in rm.iter().rev() {
                        ws_cons.remove(*i);
                    }
                }
            }
        })),
    ));
    drop(db);
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async_main(data, addr));
}
pub async fn async_main(data: Data, addr: SocketAddr) {
    rocket::build()
        .configure(Config {
            address: addr.ip(),
            port: addr.port(),
            ..Default::default()
        })
        .manage(data)
        .mount(
            "/",
            routes![
                index,
                websocket,
                play,
                pause,
                stop,
                skip,
                clear_queue,
                queue_goto,
                queue_remove,
                add_song,
                search,
                now_playing_html,
                now_playing_ids,
                song,
                queue_html,
            ],
        )
        .launch()
        .await
        .unwrap();
}
