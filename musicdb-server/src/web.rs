use std::convert::Infallible;
use std::mem;
use std::net::SocketAddr;
use std::sync::{mpsc, Arc, Mutex};
use std::task::Poll;
use std::time::Duration;

use axum::extract::{Path, State};
use axum::response::sse::Event;
use axum::response::{Html, Sse};
use axum::routing::{get, post};
use axum::{Router, TypedHeader};
use futures::{stream, Stream};
use musicdb_lib::data::database::{Database, UpdateEndpoint};
use musicdb_lib::data::queue::{Queue, QueueContent};
use musicdb_lib::server::Command;
use tokio_stream::StreamExt as _;

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

#[derive(Clone)]
pub struct AppState {
    db: Arc<Mutex<Database>>,
    html: Arc<AppHtml>,
}
#[derive(Debug)]
pub struct AppHtml {
    /// /
    /// can use:
    root: Vec<HtmlPart>,

    /// sse:artists
    /// can use: artists (0+ repeats of artists_one)
    artists: Vec<HtmlPart>,
    /// can use: id, name
    artists_one: Vec<HtmlPart>,

    /// /artist-view/:artist-id
    /// can use: albums (0+ repeats of albums_one)
    artist_view: Vec<HtmlPart>,
    /// can use: name
    albums_one: Vec<HtmlPart>,

    /// /album-view/:album-id
    /// can use: id, name, songs (0+ repeats of songs_one)
    album_view: Vec<HtmlPart>,
    /// can use: title
    songs_one: Vec<HtmlPart>,

    /// /queue
    /// can use: currentTitle, nextTitle, content
    queue: Vec<HtmlPart>,
    /// can use: path, title
    queue_song: Vec<HtmlPart>,
    /// can use: path, title
    queue_song_current: Vec<HtmlPart>,
    /// can use: path, content, name
    queue_folder: Vec<HtmlPart>,
    /// can use: path, content, name
    queue_folder_current: Vec<HtmlPart>,
    /// can use: path, total, current, inner
    queue_loop: Vec<HtmlPart>,
    /// can use: path, total, current, inner
    queue_loop_current: Vec<HtmlPart>,
    /// can use: path, current, inner
    queue_loopinf: Vec<HtmlPart>,
    /// can use: path, current, inner
    queue_loopinf_current: Vec<HtmlPart>,
    /// can use: path, content
    queue_random: Vec<HtmlPart>,
    /// can use: path, content
    queue_random_current: Vec<HtmlPart>,
    /// can use: path, content
    queue_shuffle: Vec<HtmlPart>,
    /// can use: path, content
    queue_shuffle_current: Vec<HtmlPart>,
}
impl AppHtml {
    pub fn from_dir<P: AsRef<std::path::Path>>(dir: P) -> std::io::Result<Self> {
        let dir = dir.as_ref();
        Ok(Self {
            root: Self::parse(&std::fs::read_to_string(dir.join("root.html"))?),
            artists: Self::parse(&std::fs::read_to_string(dir.join("artists.html"))?),
            artists_one: Self::parse(&std::fs::read_to_string(dir.join("artists_one.html"))?),
            artist_view: Self::parse(&std::fs::read_to_string(dir.join("artist-view.html"))?),
            albums_one: Self::parse(&std::fs::read_to_string(dir.join("albums_one.html"))?),
            album_view: Self::parse(&std::fs::read_to_string(dir.join("album-view.html"))?),
            songs_one: Self::parse(&std::fs::read_to_string(dir.join("songs_one.html"))?),
            queue: Self::parse(&std::fs::read_to_string(dir.join("queue.html"))?),
            queue_song: Self::parse(&std::fs::read_to_string(dir.join("queue_song.html"))?),
            queue_song_current: Self::parse(&std::fs::read_to_string(
                dir.join("queue_song_current.html"),
            )?),
            queue_folder: Self::parse(&std::fs::read_to_string(dir.join("queue_folder.html"))?),
            queue_folder_current: Self::parse(&std::fs::read_to_string(
                dir.join("queue_folder_current.html"),
            )?),
            queue_loop: Self::parse(&std::fs::read_to_string(dir.join("queue_loop.html"))?),
            queue_loop_current: Self::parse(&std::fs::read_to_string(
                dir.join("queue_loop_current.html"),
            )?),
            queue_loopinf: Self::parse(&std::fs::read_to_string(dir.join("queue_loopinf.html"))?),
            queue_loopinf_current: Self::parse(&std::fs::read_to_string(
                dir.join("queue_loopinf_current.html"),
            )?),
            queue_random: Self::parse(&std::fs::read_to_string(dir.join("queue_random.html"))?),
            queue_random_current: Self::parse(&std::fs::read_to_string(
                dir.join("queue_random_current.html"),
            )?),
            queue_shuffle: Self::parse(&std::fs::read_to_string(dir.join("queue_shuffle.html"))?),
            queue_shuffle_current: Self::parse(&std::fs::read_to_string(
                dir.join("queue_shuffle_current.html"),
            )?),
        })
    }
    pub fn parse(s: &str) -> Vec<HtmlPart> {
        let mut o = Vec::new();
        let mut c = String::new();
        let mut chars = s.chars().peekable();
        loop {
            if let Some(ch) = chars.next() {
                if ch == '\\' && chars.peek().is_some_and(|ch| *ch == ':') {
                    chars.next();
                    o.push(HtmlPart::Plain(mem::replace(&mut c, String::new())));
                    loop {
                        if let Some(ch) = chars.peek() {
                            if !ch.is_ascii_alphabetic() {
                                o.push(HtmlPart::Insert(mem::replace(&mut c, String::new())));
                                break;
                            } else {
                                c.push(*ch);
                                chars.next();
                            }
                        } else {
                            if c.len() > 0 {
                                o.push(HtmlPart::Insert(c));
                            }
                            return o;
                        }
                    }
                } else {
                    c.push(ch);
                }
            } else {
                if c.len() > 0 {
                    o.push(HtmlPart::Plain(c));
                }
                return o;
            }
        }
    }
}
#[derive(Debug)]
pub enum HtmlPart {
    /// text as plain html
    Plain(String),
    /// insert some value depending on context and key
    Insert(String),
}

pub async fn main(db: Arc<Mutex<Database>>, sender: mpsc::Sender<Command>, addr: SocketAddr) {
    let db1 = Arc::clone(&db);
    let state = AppState {
        db,
        html: Arc::new(AppHtml::from_dir("assets").unwrap()),
    };
    let (s1, s2, s3, s4, s5, s6, s7, s8, s9) = (
        sender.clone(),
        sender.clone(),
        sender.clone(),
        sender.clone(),
        sender.clone(),
        sender.clone(),
        sender.clone(),
        sender.clone(),
        sender,
    );
    let state1 = state.clone();

    let app = Router::new()
        // root
        .nest_service(
            "/",
            get(move || async move {
                Html(
                    state1
                        .html
                        .root
                        .iter()
                        .map(|v| match v {
                            HtmlPart::Plain(v) => v,
                            HtmlPart::Insert(_) => "",
                        })
                        .collect::<String>(),
                )
            }),
        )
        // server-sent events
        .route("/sse", get(sse_handler))
        // inner views (embedded in root)
        .route("/artist-view/:artist-id", get(artist_view_handler))
        .route("/album-view/:album-id", get(album_view_handler))
        // handle POST requests via the mpsc::Sender instead of locking the db.
        .route(
            "/pause",
            post(move || async move {
                _ = s1.send(Command::Pause);
            }),
        )
        .route(
            "/resume",
            post(move || async move {
                _ = s2.send(Command::Resume);
            }),
        )
        .route(
            "/stop",
            post(move || async move {
                _ = s3.send(Command::Stop);
            }),
        )
        .route(
            "/next",
            post(move || async move {
                _ = s4.send(Command::NextSong);
            }),
        )
        .route(
            "/queue/clear",
            post(move || async move {
                _ = s5.send(Command::QueueUpdate(
                    vec![],
                    QueueContent::Folder(0, vec![], String::new()).into(),
                ));
            }),
        )
        .route(
            "/queue/remove/:i",
            post(move |Path(i): Path<String>| async move {
                let mut ids = vec![];
                for id in i.split('-') {
                    if let Ok(n) = id.parse() {
                        ids.push(n);
                    } else {
                        return;
                    }
                }
                _ = s8.send(Command::QueueRemove(ids));
            }),
        )
        .route(
            "/queue/goto/:i",
            post(move |Path(i): Path<String>| async move {
                let mut ids = vec![];
                for id in i.split('-') {
                    if let Ok(n) = id.parse() {
                        ids.push(n);
                    } else {
                        return;
                    }
                }
                _ = s9.send(Command::QueueGoto(ids));
            }),
        )
        .route(
            "/queue/add-song/:song-id",
            post(move |Path(song_id)| async move {
                _ = s6.send(Command::QueueAdd(
                    vec![],
                    QueueContent::Song(song_id).into(),
                ));
            }),
        )
        .route(
            "/queue/add-album/:album-id",
            post(move |Path(album_id)| async move {
                if let Some(album) = db1.lock().unwrap().albums().get(&album_id) {
                    _ = s7.send(Command::QueueAdd(
                        vec![],
                        QueueContent::Folder(
                            0,
                            album
                                .songs
                                .iter()
                                .map(|id| QueueContent::Song(*id).into())
                                .collect(),
                            album.name.clone(),
                        )
                        .into(),
                    ));
                }
            }),
        )
        .with_state(state);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn sse_handler(
    TypedHeader(user_agent): TypedHeader<headers::UserAgent>,
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    println!("`{}` connected", user_agent.as_str());

    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    let mut db = state.db.lock().unwrap();
    _ = sender.send(Arc::new(Command::SyncDatabase(vec![], vec![], vec![])));
    _ = sender.send(Arc::new(Command::NextSong));
    _ = sender.send(Arc::new(if db.playing {
        Command::Resume
    } else {
        Command::Pause
    }));
    db.update_endpoints
        .push(UpdateEndpoint::CmdChannelTokio(sender));
    drop(db);

    let stream = stream::poll_fn(move |_ctx| {
        if let Ok(cmd) = receiver.try_recv() {
            Poll::Ready(Some(match cmd.as_ref() {
                Command::Resume => Event::default().event("playing").data("playing"),
                Command::Pause => Event::default().event("playing").data("paused"),
                Command::Stop => Event::default().event("playing").data("stopped"),
                Command::SyncDatabase(..)
                | Command::ModifySong(..)
                | Command::ModifyAlbum(..)
                | Command::ModifyArtist(..)
                | Command::AddSong(..)
                | Command::AddAlbum(..)
                | Command::AddArtist(..)
                | Command::AddCover(..)
                | Command::RemoveSong(_)
                | Command::RemoveAlbum(_)
                | Command::RemoveArtist(_) => Event::default().event("artists").data({
                    let db = state.db.lock().unwrap();
                    let mut a = db.artists().iter().collect::<Vec<_>>();
                    a.sort_unstable_by_key(|(_id, artist)| &artist.name);
                    let mut artists = String::new();
                    for (id, artist) in a {
                        for v in &state.html.artists_one {
                            match v {
                                HtmlPart::Plain(v) => artists.push_str(v),
                                HtmlPart::Insert(key) => match key.as_str() {
                                    "id" => artists.push_str(&id.to_string()),
                                    "name" => artists.push_str(&artist.name),
                                    _ => {}
                                },
                            }
                        }
                    }
                    state
                        .html
                        .artists
                        .iter()
                        .map(|v| match v {
                            HtmlPart::Plain(v) => v,
                            HtmlPart::Insert(key) => match key.as_str() {
                                "artists" => &artists,
                                _ => "",
                            },
                        })
                        .collect::<String>()
                }),
                Command::NextSong
                | Command::QueueUpdate(..)
                | Command::QueueAdd(..)
                | Command::QueueInsert(..)
                | Command::QueueRemove(..)
                | Command::QueueGoto(..)
                | Command::QueueSetShuffle(..) => {
                    let db = state.db.lock().unwrap();
                    let current = db
                        .queue
                        .get_current_song()
                        .map_or(None, |id| db.songs().get(id));
                    let next = db
                        .queue
                        .get_next_song()
                        .map_or(None, |id| db.songs().get(id));
                    let mut content = String::new();
                    build_queue_content_build(
                        &db,
                        &state,
                        &mut content,
                        &db.queue,
                        String::new(),
                        true,
                        false,
                    );
                    Event::default().event("queue").data(
                        state
                            .html
                            .queue
                            .iter()
                            .map(|v| match v {
                                HtmlPart::Plain(v) => v,
                                HtmlPart::Insert(key) => match key.as_str() {
                                    "currentTitle" => {
                                        if let Some(s) = current {
                                            &s.title
                                        } else {
                                            ""
                                        }
                                    }
                                    "nextTitle" => {
                                        if let Some(s) = next {
                                            &s.title
                                        } else {
                                            ""
                                        }
                                    }
                                    "content" => &content,
                                    _ => "",
                                },
                            })
                            .collect::<String>(),
                    )
                }
                Command::Save | Command::SetLibraryDirectory(_) => return Poll::Pending,
            }))
        } else {
            return Poll::Pending;
        }
    })
    .map(Ok);
    // .throttle(Duration::from_millis(100));

    Sse::new(stream)
        .keep_alive(axum::response::sse::KeepAlive::new().interval(Duration::from_millis(250)))
}

async fn artist_view_handler(
    State(state): State<AppState>,
    Path(artist_id): Path<u64>,
) -> Html<String> {
    let db = state.db.lock().unwrap();
    if let Some(artist) = db.artists().get(&artist_id) {
        let mut albums = String::new();
        for id in artist.albums.iter() {
            if let Some(album) = db.albums().get(id) {
                for v in &state.html.albums_one {
                    match v {
                        HtmlPart::Plain(v) => albums.push_str(v),
                        HtmlPart::Insert(key) => match key.as_str() {
                            "id" => albums.push_str(&id.to_string()),
                            "name" => albums.push_str(&album.name),
                            _ => {}
                        },
                    }
                }
            }
        }
        let id = artist_id.to_string();
        Html(
            state
                .html
                .artist_view
                .iter()
                .map(|v| match v {
                    HtmlPart::Plain(v) => v,
                    HtmlPart::Insert(key) => match key.as_str() {
                        "id" => &id,
                        "name" => &artist.name,
                        "albums" => &albums,
                        _ => "",
                    },
                })
                .collect(),
        )
    } else {
        Html(format!(
            "<h1>Bad ID</h1><p>There is no artist with the id {artist_id} in the database</p>"
        ))
    }
}

async fn album_view_handler(
    State(state): State<AppState>,
    Path(album_id): Path<u64>,
) -> Html<String> {
    let db = state.db.lock().unwrap();
    if let Some(album) = db.albums().get(&album_id) {
        let mut songs = String::new();
        for id in album.songs.iter() {
            if let Some(song) = db.songs().get(id) {
                for v in &state.html.songs_one {
                    match v {
                        HtmlPart::Plain(v) => songs.push_str(v),
                        HtmlPart::Insert(key) => match key.as_str() {
                            "id" => songs.push_str(&id.to_string()),
                            "title" => songs.push_str(&song.title),
                            _ => {}
                        },
                    }
                }
            }
        }
        let id = album_id.to_string();
        Html(
            state
                .html
                .album_view
                .iter()
                .map(|v| match v {
                    HtmlPart::Plain(v) => v,
                    HtmlPart::Insert(key) => match key.as_str() {
                        "id" => &id,
                        "name" => &album.name,
                        "songs" => &songs,
                        _ => "",
                    },
                })
                .collect(),
        )
    } else {
        Html(format!(
            "<h1>Bad ID</h1><p>There is no album with the id {album_id} in the database</p>"
        ))
    }
}

fn build_queue_content_build(
    db: &Database,
    state: &AppState,
    html: &mut String,
    queue: &Queue,
    path: String,
    current: bool,
    skip_folder: bool,
) {
    // TODO: Do something for disabled ones too (they shouldn't just be hidden)
    if queue.enabled() {
        match queue.content() {
            QueueContent::Song(id) => {
                if let Some(song) = db.songs().get(id) {
                    for v in if current {
                        &state.html.queue_song_current
                    } else {
                        &state.html.queue_song
                    } {
                        match v {
                            HtmlPart::Plain(v) => html.push_str(v),
                            HtmlPart::Insert(key) => match key.as_str() {
                                "path" => html.push_str(&path),
                                "title" => html.push_str(&song.title),
                                _ => {}
                            },
                        }
                    }
                }
            }
            QueueContent::Folder(ci, c, name) => {
                if skip_folder || path.is_empty() {
                    for (i, c) in c.iter().enumerate() {
                        let current = current && *ci == i;
                        build_queue_content_build(db, state, html, c, i.to_string(), current, false)
                    }
                } else {
                    for v in if current {
                        &state.html.queue_folder_current
                    } else {
                        &state.html.queue_folder
                    } {
                        match v {
                            HtmlPart::Plain(v) => html.push_str(v),
                            HtmlPart::Insert(key) => match key.as_str() {
                                "path" => html.push_str(&path),
                                "name" => html.push_str(name),
                                "content" => {
                                    for (i, c) in c.iter().enumerate() {
                                        let current = current && *ci == i;
                                        build_queue_content_build(
                                            db,
                                            state,
                                            html,
                                            c,
                                            format!("{path}-{i}"),
                                            current,
                                            false,
                                        )
                                    }
                                }
                                _ => {}
                            },
                        }
                    }
                }
            }
            QueueContent::Loop(total, cur, inner) => {
                for v in match (*total, current) {
                    (0, false) => &state.html.queue_loopinf,
                    (0, true) => &state.html.queue_loopinf_current,
                    (_, false) => &state.html.queue_loop,
                    (_, true) => &state.html.queue_loop_current,
                } {
                    match v {
                        HtmlPart::Plain(v) => html.push_str(v),
                        HtmlPart::Insert(key) => match key.as_str() {
                            "path" => html.push_str(&path),
                            "total" => html.push_str(&format!("{total}")),
                            "current" => html.push_str(&format!("{cur}")),
                            "inner" => build_queue_content_build(
                                db,
                                state,
                                html,
                                &inner,
                                format!("{path}-0"),
                                current,
                                true,
                            ),
                            _ => {}
                        },
                    }
                }
            }
            QueueContent::Random(q) => {
                for v in if current {
                    &state.html.queue_random_current
                } else {
                    &state.html.queue_random
                } {
                    match v {
                        HtmlPart::Plain(v) => html.push_str(v),
                        HtmlPart::Insert(key) => match key.as_str() {
                            "path" => html.push_str(&path),
                            "content" => {
                                for (i, v) in q.iter().enumerate() {
                                    build_queue_content_build(
                                        db,
                                        state,
                                        html,
                                        &v,
                                        format!("{path}-0"),
                                        current && i == q.len().saturating_sub(2),
                                        true,
                                    )
                                }
                            }
                            _ => {}
                        },
                    }
                }
            }
            QueueContent::Shuffle(cur, map, content, _) => {
                for v in if current {
                    &state.html.queue_shuffle_current
                } else {
                    &state.html.queue_shuffle
                } {
                    match v {
                        HtmlPart::Plain(v) => html.push_str(v),
                        HtmlPart::Insert(key) => match key.as_str() {
                            "path" => html.push_str(&path),
                            "content" => {
                                for (i, v) in map.iter().filter_map(|i| content.get(*i)).enumerate()
                                {
                                    build_queue_content_build(
                                        db,
                                        state,
                                        html,
                                        &v,
                                        format!("{path}-0"),
                                        current && i == *cur,
                                        true,
                                    )
                                }
                            }
                            _ => {}
                        },
                    }
                }
            }
        }
    }
}
