use std::borrow::Cow;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, mpsc};

use musicdb_lib::data::database::{Database, UpdateEndpoint};
use musicdb_lib::data::queue::{Queue, QueueContent, QueueFolder};
use musicdb_lib::data::{AlbumId, CoverId, SongId};
use musicdb_lib::server::{Action, Command, Req};
use rocket::futures::{SinkExt, StreamExt};
use rocket::http::{ContentType, Status};
use rocket::response::content::{RawHtml, RawJson};
use rocket::{Config, State, get, routes};
use rocket_seek_stream::SeekStream;
use rocket_ws::{Message, WebSocket};
use serde::Serialize;
use tokio::select;
use tokio::sync::mpsc::Sender;

struct Data {
    db: Arc<Mutex<Database>>,
    command_sender: mpsc::Sender<(Command, Option<u64>)>,
    websocket_connections: Arc<tokio::sync::Mutex<Vec<Sender<Message>>>>,
}

#[get("/")]
fn index() -> RawHtml<&'static str> {
    RawHtml(include_str!("index.html"))
}

fn is_false(b: &bool) -> bool {
    !b
}
#[derive(Serialize)]
struct SongInfo<'a> {
    #[serde(skip_serializing_if = "is_false")]
    on: bool,
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    next: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    album: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    artist: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cover: Option<String>,
    ms: u64,
}
#[get("/current")]
fn current(data: &State<Data>) -> Option<RawJson<String>> {
    let db_lock = data.db.lock().unwrap();
    let db: &Database = &db_lock;
    if let Some(id) = db.queue.get_current_song()
        && let Some(song) = db.get_song(id)
    {
        Some(RawJson(
            serde_json::to_string(&SongInfo {
                on: db.playing,
                id: id.to_string(),
                next: db.queue.get_next_song().map(|id| id.to_string()),
                title: Some(&song.title),
                album: song
                    .album
                    .and_then(|i| db.albums().get(&i))
                    .map(|v| v.name.as_str()),
                artist: db.artists().get(&song.artist).map(|v| v.name.as_str()),
                cover: db
                    .get_song(id)
                    .and_then(|song| {
                        song.cover
                            .or_else(|| {
                                song.album.and_then(|id| {
                                    db.albums().get(&id).and_then(|album| album.cover)
                                })
                            })
                            .or_else(|| {
                                db.artists()
                                    .get(&song.artist)
                                    .and_then(|artist| artist.cover)
                            })
                    })
                    .map(|v| v.to_string()),
                ms: song.duration_millis,
            })
            .unwrap(),
        ))
    } else {
        None
    }
}
#[get("/cover/<id>")]
fn cover(data: &State<Data>, id: CoverId) -> Option<(ContentType, Vec<u8>)> {
    let mut db = data.db.lock().unwrap();
    let db: &mut Database = &mut db;
    db.covers()
        .get(&id)
        .and_then(|cover| cover.get_bytes_from_file(|p| db.get_path(p), |b| b.clone()))
        .map(|bytes| (ContentType::new("image", "jpeg"), bytes))
}
#[get("/favicon")]
fn favicon() -> (ContentType, Vec<u8>) {
    (ContentType::new("image", "jpeg"), vec![])
}

#[get("/song/<id>")]
fn song1(data: &State<Data>, id: SongId) -> Option<SeekStream<'_>> {
    song(data, id)
}
#[get("/song/<id>/<_>")]
fn song2(data: &State<Data>, id: SongId) -> Option<SeekStream<'_>> {
    song(data, id)
}
fn song(data: &State<Data>, id: SongId) -> Option<SeekStream<'_>> {
    let db = data.db.lock().unwrap();
    if let Some(song) = db.get_song(&id) {
        song.cached_data().cache_data_start_thread(&db, song);
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

#[get("/add-song/<id>/<path>")]
fn add_song(id: SongId, path: &str, data: &State<Data>) -> Status {
    let mut db = data.db.lock().unwrap();
    let db: &mut Database = &mut db;
    if db.get_song(&id).is_some() {
        add_any(QueueContent::Song(id).into(), path, data)
    } else {
        Status::BadRequest
    }
}
#[get("/add-album/<id>/<path>")]
fn add_album(id: AlbumId, path: &str, data: &State<Data>) -> Status {
    let mut db = data.db.lock().unwrap();
    let db: &mut Database = &mut db;
    if let Some(album) = db.albums().get(&id) {
        add_any(
            QueueContent::Folder(QueueFolder {
                index: 0,
                content: album
                    .songs
                    .iter()
                    .filter_map(|id| db.get_song(id).map(|_| QueueContent::Song(*id).into()))
                    .collect(),
                name: album.name.to_owned(),
                order: None,
            })
            .into(),
            path,
            data,
        )
    } else {
        Status::BadRequest
    }
}
#[get("/add-artist/<id>/<path>")]
fn add_artist(id: AlbumId, path: &str, data: &State<Data>) -> Status {
    let mut db = data.db.lock().unwrap();
    let db: &mut Database = &mut db;
    if let Some(artist) = db.artists().get(&id) {
        add_any(
            QueueContent::Folder(QueueFolder {
                index: 0,
                content: artist
                    .singles
                    .iter()
                    .filter_map(|id| db.get_song(id).map(|_| QueueContent::Song(*id).into()))
                    .chain(
                        artist
                            .albums
                            .iter()
                            .filter_map(|id| db.albums().get(id))
                            .map(|album| {
                                QueueContent::Folder(QueueFolder {
                                    index: 0,
                                    content: album
                                        .songs
                                        .iter()
                                        .filter_map(|id| {
                                            db.get_song(id).map(|_| QueueContent::Song(*id).into())
                                        })
                                        .collect(),
                                    name: album.name.to_owned(),
                                    order: None,
                                })
                                .into()
                            }),
                    )
                    .collect(),
                name: artist.name.to_owned(),
                order: None,
            })
            .into(),
            path,
            data,
        )
    } else {
        Status::BadRequest
    }
}
fn add_any(queue: Queue, path: &str, data: &Data) -> Status {
    let (into, path) = path
        .strip_prefix('!')
        .map(|p| (true, p))
        .unwrap_or((false, path));
    if let Some(mut path) = path
        .split('_')
        .skip_while(|v| v.is_empty())
        .map(|v| v.parse().ok())
        .collect()
    {
        if into {
            data.command_sender
                .send((
                    Action::QueueAdd(path, vec![queue], Req::none()).cmd(0xFF),
                    None,
                ))
                .unwrap();
            Status::Ok
        } else if let Some(last) = path.pop() {
            data.command_sender
                .send((
                    Action::QueueInsert(path, last, vec![queue], Req::none()).cmd(0xFF),
                    None,
                ))
                .unwrap();
            Status::Ok
        } else {
            Status::BadRequest
        }
    } else {
        Status::BadRequest
    }
}

#[derive(Serialize)]
struct QueueElement<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<Cow<'a, str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    extra: Option<Cow<'a, str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    children: Option<Vec<Self>>,
}
#[get("/queue")]
fn queue(data: &State<Data>) -> RawJson<String> {
    fn build_queue_element<'a>(queue: &'a Queue, db: &'a Database) -> QueueElement<'a> {
        match queue.content() {
            QueueContent::Song(id) => {
                if let Some(song) = db.get_song(id)
                    && let Some(artist) = db.artists().get(&song.artist)
                {
                    if let Some(album) = song.album
                        && let Some(album) = db.albums().get(&album)
                    {
                        QueueElement {
                            title: Some(Cow::Borrowed(song.title.as_str())),
                            extra: Some(Cow::Owned(format!(
                                "by {} on {}",
                                artist.name, album.name
                            ))),
                            children: None,
                        }
                    } else {
                        QueueElement {
                            title: Some(Cow::Borrowed(song.title.as_str())),
                            extra: Some(Cow::Owned(format!("by {}", artist.name))),
                            children: None,
                        }
                    }
                } else {
                    QueueElement {
                        title: None,
                        extra: None,
                        children: None,
                    }
                }
            }
            QueueContent::Folder(folder) => QueueElement {
                title: Some(Cow::Borrowed(folder.name.as_str())),
                extra: None,
                children: Some(if let Some(order) = &folder.order {
                    order
                        .iter()
                        .map(|q| folder.content.get(*q))
                        .map(|q| {
                            q.map(|q| build_queue_element(q, db))
                                .unwrap_or_else(|| QueueElement {
                                    title: None,
                                    extra: None,
                                    children: None,
                                })
                        })
                        .collect()
                } else {
                    folder
                        .content
                        .iter()
                        .map(|q| build_queue_element(q, db))
                        .collect()
                }),
            },
            QueueContent::Loop(total, done, queue) => {
                let mut queue = build_queue_element(queue, db);
                let extra = if *total == 0 {
                    format!("{}/∞", *done)
                } else {
                    format!("{}/{}", *done, *total)
                };
                if queue.extra.is_none() {
                    queue.extra = Some(Cow::Owned(extra));
                    queue
                } else {
                    QueueElement {
                        title: None,
                        extra: Some(Cow::Owned(extra)),
                        children: Some(vec![queue]),
                    }
                }
            }
        }
    }
    let db = data.db.lock().unwrap();
    RawJson(serde_json::to_string(&build_queue_element(&db.queue, &db)).unwrap())
}

#[get("/queue-move/<p1>/<p2>")]
fn queue_move(p1: &str, p2: &str, data: &State<Data>) -> Status {
    if let Some(p1) = p1
        .split('_')
        .skip_while(|v| v.is_empty())
        .map(|v| v.parse().ok())
        .collect()
        && let Some(p2) = p2
            .split('_')
            .skip_while(|v| v.is_empty())
            .map(|v| v.parse().ok())
            .collect()
    {
        data.command_sender
            .send((Action::QueueMove(p1, p2).cmd(0xFF), None))
            .unwrap();
        Status::Ok
    } else {
        Status::BadRequest
    }
}
#[get("/queue-moveinto/<p1>/<p2>")]
fn queue_move_into(p1: &str, p2: &str, data: &State<Data>) -> Status {
    if let Some(p1) = p1
        .split('_')
        .skip_while(|v| v.is_empty())
        .map(|v| v.parse().ok())
        .collect()
        && let Some(p2) = p2
            .split('_')
            .skip_while(|v| v.is_empty())
            .map(|v| v.parse().ok())
            .collect()
    {
        data.command_sender
            .send((Action::QueueMoveInto(p1, p2).cmd(0xFF), None))
            .unwrap();
        Status::Ok
    } else {
        Status::BadRequest
    }
}
#[get("/queue-remove/<path>")]
fn queue_remove(data: &State<Data>, path: &str) {
    if let Some(path) = path
        .split('_')
        .skip_while(|v| v.is_empty())
        .map(|v| v.parse().ok())
        .collect()
    {
        data.command_sender
            .send((Action::QueueRemove(path).cmd(0xFFu8), None))
            .unwrap();
    }
}
#[get("/queue-goto/<path>")]
fn queue_goto(data: &State<Data>, path: &str) {
    if let Some(path) = path
        .split('_')
        .skip_while(|v| v.is_empty())
        .map(|v| v.parse().ok())
        .collect()
    {
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

#[derive(Serialize)]
#[serde(tag = "t")]
enum SearchResult {
    #[serde(rename = "a")]
    Artist {
        name: String,
        id: String,
        has: Vec<Self>,
    },
    #[serde(rename = "b")]
    Album {
        name: String,
        id: String,
        has: Vec<Self>,
    },
    #[serde(rename = "s")]
    Song { title: String, id: String },
}
#[get("/search?<artist>&<album>&<title>")]
fn search(
    data: &State<Data>,
    artist: Option<&str>,
    album: Option<&str>,
    title: Option<&str>,
) -> RawJson<String> {
    let db = data.db.lock().unwrap();
    let db: &Database = &db;
    let artist = artist.map(|v| v.to_lowercase());
    let artist = artist.as_deref().unwrap_or("");
    let album = album.map(|v| v.to_lowercase());
    let album = album.as_deref().unwrap_or("");
    let title = title.map(|v| v.to_lowercase());
    let title = title.as_deref().unwrap_or("");
    let mut out = vec![];
    for artist in db
        .artists()
        .values()
        .filter(|a| a.name.to_lowercase().contains(artist))
    {
        let mut a1 = vec![];
        for song in artist
            .singles
            .iter()
            .filter_map(|id| db.get_song(id))
            .filter(|a| a.title.to_lowercase().contains(title))
        {
            a1.push(SearchResult::Song {
                title: song.title.clone(),
                id: song.id.to_string(),
            });
        }
        for album in artist
            .albums
            .iter()
            .filter_map(|id| db.albums().get(id))
            .filter(|a| a.name.to_lowercase().contains(album))
        {
            let mut a2 = vec![];
            for song in album
                .songs
                .iter()
                .filter_map(|id| db.get_song(id))
                .filter(|a| a.title.to_lowercase().contains(title))
            {
                a2.push(SearchResult::Song {
                    title: song.title.clone(),
                    id: song.id.to_string(),
                });
            }
            if !a2.is_empty() {
                a1.push(SearchResult::Album {
                    name: album.name.clone(),
                    id: album.id.to_string(),
                    has: a2,
                });
            }
        }
        if !a1.is_empty() {
            out.push(SearchResult::Artist {
                name: artist.name.clone(),
                id: artist.id.to_string(),
                has: a1,
            });
        }
    }
    RawJson(serde_json::to_string(&out).unwrap())
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
                    | Action::SavedQueue(..)
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

async fn async_main(data: Data, addr: SocketAddr) {
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
                queue_move,
                queue_move_into,
                queue,
                add_song,
                add_album,
                add_artist,
                search,
                current,
                cover,
                favicon,
                song1,
                song2,
            ],
        )
        .launch()
        .await
        .unwrap();
}
