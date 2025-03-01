pub mod get;

use std::{
    io::{BufRead as _, BufReader, Read, Write},
    net::{SocketAddr, TcpListener},
    sync::{mpsc, Arc, Mutex},
    thread,
    time::Duration,
};

use colorize::AnsiColor;

#[cfg(feature = "playback")]
use crate::player::Player;
use crate::server::get::handle_one_connection_as_get;
use crate::{
    data::{
        album::Album,
        artist::Artist,
        database::{Cover, Database, UpdateEndpoint},
        queue::Queue,
        song::Song,
        AlbumId, ArtistId, SongId,
    },
    load::ToFromBytes,
};

#[derive(Clone, Debug)]
pub struct Command {
    /// when sending to the server, this should be the most recent sequence number,
    /// or `0xFF` to indicate that the action should be performed regardless of if the sequence number would be up to date or not.
    /// when receiving from the server, this contains the most recent sequence number. It is never `0xFF`.
    /// used to avoid issues due to desynchronization
    pub seq: u8,
    pub action: Action,
}
impl Command {
    pub fn new(seq: u8, action: Action) -> Self {
        Self { seq, action }
    }
}
impl Action {
    pub fn cmd(self, seq: u8) -> Command {
        Command::new(seq, self)
    }
    pub fn take_req_all(&mut self) -> Vec<Req> {
        self.req_mut()
            .into_iter()
            .map(|r| std::mem::replace(r, Req::none()))
            .collect()
    }
    pub fn get_req_all(&mut self) -> Vec<Req> {
        self.req_mut().into_iter().map(|r| *r).collect()
    }
    pub fn get_req_if_some(&mut self) -> Vec<Req> {
        self.req_mut()
            .into_iter()
            .map(|r| *r)
            .filter(|r| r.is_some())
            .collect()
    }
    pub fn put_req_all(&mut self, reqs: Vec<Req>) {
        for (o, n) in self.req_mut().into_iter().zip(reqs) {
            *o = n;
        }
    }
    fn req_mut(&mut self) -> Vec<&mut Req> {
        match self {
            Self::QueueUpdate(_, _, req)
            | Self::QueueAdd(_, _, req)
            | Self::QueueInsert(_, _, _, req)
            | Self::AddSong(_, req)
            | Self::AddAlbum(_, req)
            | Self::AddArtist(_, req)
            | Self::AddCover(_, req)
            | Self::ModifySong(_, req)
            | Self::ModifyAlbum(_, req)
            | Self::ModifyArtist(_, req)
            | Self::Denied(req) => vec![req],
            Self::Resume
            | Self::Pause
            | Self::Stop
            | Self::NextSong
            | Self::SyncDatabase(_, _, _)
            | Self::QueueRemove(_)
            | Self::QueueMove(_, _)
            | Self::QueueMoveInto(_, _)
            | Self::QueueGoto(_)
            | Self::QueueShuffle(_, _)
            | Self::QueueSetShuffle(_, _, _)
            | Self::QueueUnshuffle(_)
            | Self::RemoveSong(_)
            | Self::RemoveAlbum(_)
            | Self::RemoveArtist(_)
            | Self::SetSongDuration(_, _)
            | Self::TagSongFlagSet(_, _)
            | Self::TagSongFlagUnset(_, _)
            | Self::TagAlbumFlagSet(_, _)
            | Self::TagAlbumFlagUnset(_, _)
            | Self::TagArtistFlagSet(_, _)
            | Self::TagArtistFlagUnset(_, _)
            | Self::TagSongPropertySet(_, _, _)
            | Self::TagSongPropertyUnset(_, _)
            | Self::TagAlbumPropertySet(_, _, _)
            | Self::TagAlbumPropertyUnset(_, _)
            | Self::TagArtistPropertySet(_, _, _)
            | Self::TagArtistPropertyUnset(_, _)
            | Self::InitComplete
            | Self::Save
            | Self::ErrorInfo(_, _) => vec![],
            Self::Multiple(actions) => actions.iter_mut().flat_map(|v| v.req_mut()).collect(),
        }
    }
}
/// Should be stored in the same lock as the database
pub struct Commander {
    seq: u8,
}
pub struct Requester {
    req: u8,
}
impl Commander {
    pub fn new(ff: bool) -> Self {
        Self {
            seq: if ff { 0xFFu8 } else { 0u8 },
        }
    }
    pub fn inc(&mut self) {
        if self.seq < 0xFEu8 {
            self.seq += 1;
        } else {
            self.seq = 0;
        }
    }
    pub fn pack(&self, action: Action) -> Command {
        Command::new(self.seq, action)
    }
    pub fn recv(&mut self, command: Command) -> Action {
        self.seq = command.seq;
        command.action
    }
    pub fn seq(&self) -> u8 {
        self.seq
    }
}
impl Requester {
    pub fn new() -> Self {
        Self { req: 0 }
    }
    pub fn inc(&mut self) -> Req {
        if self.req < 0xFFu8 {
            self.req += 1;
        } else {
            self.req = 1;
        }
        Req(self.req)
    }
}
/// A request ID, or None
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Req(u8);
impl Req {
    pub fn none() -> Self {
        Self(0)
    }
    pub fn is_none(self) -> bool {
        self.0 == 0
    }
    pub fn is_some(self) -> bool {
        self.0 != 0
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Resume,
    Pause,
    Stop,
    NextSong,
    SyncDatabase(Vec<Artist>, Vec<Album>, Vec<Song>),
    QueueUpdate(Vec<usize>, Queue, Req),
    QueueAdd(Vec<usize>, Vec<Queue>, Req),
    QueueInsert(Vec<usize>, usize, Vec<Queue>, Req),
    QueueRemove(Vec<usize>),
    /// Move an element from A to B
    QueueMove(Vec<usize>, Vec<usize>),
    /// Take an element from A and add it to the end of the folder B
    QueueMoveInto(Vec<usize>, Vec<usize>),
    QueueGoto(Vec<usize>),
    /// sent by clients when they want to shuffle a folder
    /// last parameter: 0 = don't change index, 1 = set folder index to new index of previously active element
    QueueShuffle(Vec<usize>, u8),
    /// sent by the server when the folder was shuffled
    /// last parameter, see QueueShuffle
    QueueSetShuffle(Vec<usize>, Vec<usize>, u8),
    QueueUnshuffle(Vec<usize>),

    /// .id field is ignored!
    AddSong(Song, Req),
    /// .id field is ignored!
    AddAlbum(Album, Req),
    /// .id field is ignored!
    AddArtist(Artist, Req),
    AddCover(Cover, Req),
    ModifySong(Song, Req),
    ModifyAlbum(Album, Req),
    ModifyArtist(Artist, Req),
    RemoveSong(SongId),
    RemoveAlbum(AlbumId),
    RemoveArtist(ArtistId),
    SetSongDuration(SongId, u64),
    /// Add the given Tag to the song's tags, if it isn't set already.
    TagSongFlagSet(SongId, String),
    /// Remove the given Tag fron the song's tags, if it exists.
    TagSongFlagUnset(SongId, String),
    TagAlbumFlagSet(AlbumId, String),
    TagAlbumFlagUnset(AlbumId, String),
    TagArtistFlagSet(ArtistId, String),
    TagArtistFlagUnset(ArtistId, String),
    /// For the arguments `Key`, `Val`: If the song has a Tag `Key<anything>`, it will be removed. Then, `KeyVal` will be added.
    /// For example, to set "Year=2010", Key would be "Year=", and Val would be "2010". Then, "Year=1990", ..., would be removed and "Year=2010" would be added.
    TagSongPropertySet(SongId, String, String),
    /// For the arguments `Key`, `Val`: If the song has a Tag `Key<anything>`, it will be removed.
    TagSongPropertyUnset(SongId, String),
    TagAlbumPropertySet(AlbumId, String, String),
    TagAlbumPropertyUnset(AlbumId, String),
    TagArtistPropertySet(ArtistId, String, String),
    TagArtistPropertyUnset(ArtistId, String),

    Multiple(Vec<Self>),

    InitComplete,
    Save,
    ErrorInfo(String, String),

    /// The server denied a request or an action.
    /// Contains the Request ID that was rejected, if there was a request ID.
    Denied(Req),
}
impl Command {
    pub fn send_to_server(self, db: &Database, client: Option<u64>) -> Result<(), Self> {
        if let Some(sender) = &db.command_sender {
            sender.send((self, client)).unwrap();
            Ok(())
        } else {
            Err(self)
        }
    }
    pub fn send_to_server_or_apply(self, db: &mut Database, client: Option<u64>) {
        if let Some(sender) = &db.command_sender {
            sender.send((self, client)).unwrap();
        } else {
            db.apply_command(self, client);
        }
    }
}

/// starts handling database.command_sender events and optionally spawns a tcp server.
/// this function creates a new command_sender.
/// if you wish to implement your own server, set db.command_sender to None,
/// start a new thread running this function,
/// wait for db.command_sender to be Some,
/// then start your server.
/// for tcp-like protocols, you only need to
/// a) sync and register new connections using db.init_connection and db.update_endpoints.push
/// b) handle the decoding of messages using Command::from_bytes(), then send them to the db using db.command_sender.
/// for other protocols (like http + sse)
/// a) initialize new connections using db.init_connection() to synchronize the new client
/// b) handle the decoding of messages using Command::from_bytes()
/// c) re-encode all received messages using Command::to_bytes_vec(), send them to the db, and send them to all your clients.
pub fn run_server(
    database: Arc<Mutex<Database>>,
    addr_tcp: Option<SocketAddr>,
    sender_sender: Option<Box<dyn FnOnce(mpsc::Sender<(Command, Option<u64>)>)>>,
    play_audio: bool,
) {
    run_server_caching_thread_opt(database, addr_tcp, sender_sender, None, play_audio)
}
pub fn run_server_caching_thread_opt(
    database: Arc<Mutex<Database>>,
    addr_tcp: Option<SocketAddr>,
    sender_sender: Option<Box<dyn FnOnce(mpsc::Sender<(Command, Option<u64>)>)>>,
    caching_thread: Option<Box<dyn FnOnce(&mut crate::data::cache_manager::CacheManager)>>,
    play_audio: bool,
) {
    #[cfg(not(feature = "playback"))]
    if play_audio {
        panic!("Can't run the server: cannot play audio because the `playback` feature was disabled when compiling, but `play_audio` was set to `true`!");
    }

    use std::time::Instant;

    use crate::data::cache_manager::CacheManager;
    #[cfg(feature = "playback-via-playback-rs")]
    use crate::player::playback_rs::PlayerBackendPlaybackRs;
    #[cfg(feature = "playback-via-rodio")]
    use crate::player::rodio::PlayerBackendRodio;
    #[cfg(feature = "playback-via-sleep")]
    use crate::player::sleep::PlayerBackendSleep;
    #[cfg(any(
        feature = "playback",
        feature = "playback-via-playback-rs",
        feature = "playback-via-rodio"
    ))]
    use crate::player::PlayerBackend;

    // commands sent to this will be handeled later in this function in an infinite loop.
    // these commands are sent to the database asap.
    let (command_sender, command_receiver) = mpsc::channel();

    #[cfg(feature = "playback")]
    let mut player = if play_audio {
        #[cfg(feature = "playback-via-sleep")]
        let backend = PlayerBackendSleep::new(Some(command_sender.clone())).unwrap();
        #[cfg(feature = "playback-via-playback-rs")]
        let backend = PlayerBackendPlaybackRs::new(command_sender.clone()).unwrap();
        #[cfg(feature = "playback-via-rodio")]
        let backend = PlayerBackendRodio::new(command_sender.clone()).unwrap();
        Some(Player::new(backend))
    } else {
        None
    };
    #[allow(unused)]
    let cache_manager = if let Some(func) = caching_thread {
        let mut cm = CacheManager::new(Arc::clone(&database));
        func(&mut cm);
        Some(cm)
    } else {
        None
    };
    if let Some(s) = sender_sender {
        s(command_sender.clone())
    }
    database.lock().unwrap().command_sender = Some(command_sender.clone());
    if let Some(addr) = addr_tcp {
        match TcpListener::bind(addr) {
            Ok(v) => {
                let command_sender = command_sender.clone();
                let db = Arc::clone(&database);
                thread::spawn(move || loop {
                    if let Ok((connection, _con_addr)) = v.accept() {
                        let command_sender = command_sender.clone();
                        let db = Arc::clone(&db);
                        thread::spawn(move || {
                            // each connection first has to send one line to tell us what it wants
                            let mut connection = BufReader::new(connection);
                            let mut line = String::new();
                            if connection.read_line(&mut line).is_ok() {
                                // based on that line, we adjust behavior
                                match line.as_str().trim() {
                                    // sends all updates to this connection and reads commands from it
                                    "main" => {
                                        let connection = connection.into_inner();
                                        _ = handle_one_connection_as_main(
                                            db,
                                            &mut connection.try_clone().unwrap(),
                                            connection,
                                            &command_sender,
                                        )
                                    }
                                    // reads commands from the connection, but (unlike main) doesn't send any updates
                                    "control" => handle_one_connection_as_control(
                                        &mut connection,
                                        &command_sender,
                                        None,
                                    ),
                                    "get" => _ = handle_one_connection_as_get(db, &mut connection),
                                    _ => {
                                        _ = connection
                                            .into_inner()
                                            .shutdown(std::net::Shutdown::Both)
                                    }
                                }
                            }
                        });
                    }
                });
            }
            Err(e) => {
                eprintln!("[{}] Couldn't start TCP listener: {e}", "ERR!".red());
            }
        }
    }
    #[cfg(feature = "playback")]
    let song_done_polling = player
        .as_ref()
        .is_some_and(|p| p.backend.song_finished_polling());
    #[cfg(not(feature = "playback"))]
    let song_done_polling = false;
    let (dur, check_every) = if song_done_polling {
        (Duration::from_millis(50), 200)
    } else {
        (Duration::from_secs(10), 0)
    };
    let mut check = 0;
    let mut checkf = true;
    loop {
        check += 1;
        #[cfg(feature = "playback")]
        let song_finished = player.as_ref().is_some_and(|p| p.backend.song_finished());
        #[cfg(not(feature = "playback"))]
        let song_finished = false;
        if check >= check_every || checkf || song_finished {
            check = 0;
            checkf = false;
            // at the start and once after every command sent to the server,
            let mut db = database.lock().unwrap();
            // update the player
            #[cfg(feature = "playback")]
            if let Some(player) = &mut player {
                if cache_manager.is_some() {
                    player.update_dont_uncache(&mut db);
                } else {
                    player.update(&mut db);
                }
            }
            // autosave if necessary
            if let Some((first, last)) = db.times_data_modified {
                let now = Instant::now();
                if (now - first).as_secs_f32() > 60.0 && (now - last).as_secs_f32() > 5.0 {
                    if let Err(e) = db.save_database(None) {
                        eprintln!("[{}] Autosave failed: {e}", "ERR!".red());
                    }
                }
            }
        }
        if let Ok((command, client)) = command_receiver.recv_timeout(dur) {
            checkf = true;
            #[cfg(feature = "playback")]
            if let Some(player) = &mut player {
                player.handle_action(&command.action);
            }
            database.lock().unwrap().apply_command(command, client);
        }
    }
}

pub fn handle_one_connection_as_main(
    db: Arc<Mutex<Database>>,
    connection: &mut impl Read,
    mut send_to: (impl Write + Sync + Send + 'static),
    command_sender: &mpsc::Sender<(Command, Option<u64>)>,
) -> Result<(), std::io::Error> {
    // sync database
    let mut db = db.lock().unwrap();
    db.init_connection(&mut send_to)?;
    // keep the client in sync:
    // the db will send all updates to the client once it is added to update_endpoints
    let udepid = db.update_endpoints_id;
    db.update_endpoints_id += 1;
    db.update_endpoints.push((
        udepid,
        UpdateEndpoint::Bytes(Box::new(
            // try_clone is used here to split a TcpStream into Writer and Reader
            send_to,
        )),
    ));
    // drop the mutex lock
    drop(db);
    handle_one_connection_as_control(connection, command_sender, Some(udepid));
    Ok(())
}
pub fn handle_one_connection_as_control(
    connection: &mut impl Read,
    command_sender: &mpsc::Sender<(Command, Option<u64>)>,
    client: Option<u64>,
) {
    // read updates from the tcp stream and send them to the database, exit on EOF or Err
    loop {
        if let Ok(command) = Command::from_bytes(connection) {
            command_sender.send((command, client)).unwrap();
        } else {
            break;
        }
    }
}

// 01_***_*** => Simple commands
// 01_00*_*** => Playback
// 01_010_*** => Other
// 01_100_*** => Errors
// 10_***_*** => Complicated commands
// 10_00*_*** => Queue
// 10_010_*** => Misc
// 10_100_*** => Library

const BYTE_RESUME: u8 = 0b01_000_000;
const BYTE_PAUSE: u8 = 0b01_000_001;
const BYTE_STOP: u8 = 0b01_000_010;
const BYTE_NEXT_SONG: u8 = 0b01_000_100;

const BYTE_MULTIPLE: u8 = 0b01_010_100;
const BYTE_INIT_COMPLETE: u8 = 0b01_010_000;
const BYTE_SET_SONG_DURATION: u8 = 0b01_010_001;
const BYTE_SAVE: u8 = 0b01_010_010;
const BYTE_ERRORINFO: u8 = 0b01_100_010;
const BYTE_DENIED: u8 = 0b01_100_011;

const BYTE_QUEUE_UPDATE: u8 = 0b10_000_000;
const BYTE_QUEUE_ADD: u8 = 0b10_000_001;
const BYTE_QUEUE_INSERT: u8 = 0b10_000_010;
const BYTE_QUEUE_REMOVE: u8 = 0b10_000_100;
const BYTE_QUEUE_MOVE: u8 = 0b10_001_000;
const BYTE_QUEUE_MOVE_INTO: u8 = 0b10_001_001;
const BYTE_QUEUE_GOTO: u8 = 0b10_001_010;
const BYTE_QUEUE_ACTION: u8 = 0b10_001_100;
const SUBBYTE_ACTION_SHUFFLE: u8 = 0b01_000_001;
const SUBBYTE_ACTION_SET_SHUFFLE: u8 = 0b01_000_010;
const SUBBYTE_ACTION_UNSHUFFLE: u8 = 0b01_000_100;

const BYTE_SYNC_DATABASE: u8 = 0b10_010_100;

const BYTE_LIB_ADD: u8 = 0b10_100_000;
const BYTE_LIB_MODIFY: u8 = 0b10_100_001;
const BYTE_LIB_REMOVE: u8 = 0b10_100_010;
const BYTE_LIB_TAG: u8 = 0b10_100_100;
const SUBBYTE_SONG: u8 = 0b10_001_000;
const SUBBYTE_ALBUM: u8 = 0b10_001_001;
const SUBBYTE_ARTIST: u8 = 0b10_001_010;
const SUBBYTE_COVER: u8 = 0b10_001_100;
const SUBBYTE_TAG_SONG_FLAG_SET: u8 = 0b10_001_000;
const SUBBYTE_TAG_SONG_FLAG_UNSET: u8 = 0b10_001_001;
const SUBBYTE_TAG_ALBUM_FLAG_SET: u8 = 0b10_001_010;
const SUBBYTE_TAG_ALBUM_FLAG_UNSET: u8 = 0b10_001_100;
const SUBBYTE_TAG_ARTIST_FLAG_SET: u8 = 0b10_010_000;
const SUBBYTE_TAG_ARTIST_FLAG_UNSET: u8 = 0b10_010_001;
const SUBBYTE_TAG_SONG_PROPERTY_SET: u8 = 0b10_010_010;
const SUBBYTE_TAG_SONG_PROPERTY_UNSET: u8 = 0b10_010_100;
const SUBBYTE_TAG_ALBUM_PROPERTY_SET: u8 = 0b10_100_000;
const SUBBYTE_TAG_ALBUM_PROPERTY_UNSET: u8 = 0b10_100_001;
const SUBBYTE_TAG_ARTIST_PROPERTY_SET: u8 = 0b10_100_010;
const SUBBYTE_TAG_ARTIST_PROPERTY_UNSET: u8 = 0b10_100_100;

impl ToFromBytes for Command {
    fn to_bytes<T>(&self, s: &mut T) -> Result<(), std::io::Error>
    where
        T: Write,
    {
        s.write_all(&[self.seq])?;
        self.action.to_bytes(s)
    }
    fn from_bytes<T>(s: &mut T) -> Result<Self, std::io::Error>
    where
        T: Read,
    {
        Ok(Self {
            seq: ToFromBytes::from_bytes(s)?,
            action: Action::from_bytes(s)?,
        })
    }
}

impl ToFromBytes for Req {
    fn to_bytes<T>(&self, s: &mut T) -> Result<(), std::io::Error>
    where
        T: Write,
    {
        self.0.to_bytes(s)?;
        Ok(())
    }
    fn from_bytes<T>(s: &mut T) -> Result<Self, std::io::Error>
    where
        T: Read,
    {
        Ok(Self(ToFromBytes::from_bytes(s)?))
    }
}
impl ToFromBytes for Action {
    fn to_bytes<T>(&self, s: &mut T) -> Result<(), std::io::Error>
    where
        T: Write,
    {
        match self {
            Self::Resume => s.write_all(&[BYTE_RESUME])?,
            Self::Pause => s.write_all(&[BYTE_PAUSE])?,
            Self::Stop => s.write_all(&[BYTE_STOP])?,
            Self::NextSong => s.write_all(&[BYTE_NEXT_SONG])?,
            Self::SyncDatabase(a, b, c) => {
                s.write_all(&[BYTE_SYNC_DATABASE])?;
                a.to_bytes(s)?;
                b.to_bytes(s)?;
                c.to_bytes(s)?;
            }
            Self::QueueUpdate(index, new_data, req) => {
                s.write_all(&[BYTE_QUEUE_UPDATE])?;
                index.to_bytes(s)?;
                new_data.to_bytes(s)?;
                req.to_bytes(s)?;
            }
            Self::QueueAdd(index, new_data, req) => {
                s.write_all(&[BYTE_QUEUE_ADD])?;
                index.to_bytes(s)?;
                new_data.to_bytes(s)?;
                req.to_bytes(s)?;
            }
            Self::QueueInsert(index, pos, new_data, req) => {
                s.write_all(&[BYTE_QUEUE_INSERT])?;
                index.to_bytes(s)?;
                pos.to_bytes(s)?;
                new_data.to_bytes(s)?;
                req.to_bytes(s)?;
            }
            Self::QueueRemove(index) => {
                s.write_all(&[BYTE_QUEUE_REMOVE])?;
                index.to_bytes(s)?;
            }
            Self::QueueMove(a, b) => {
                s.write_all(&[BYTE_QUEUE_MOVE])?;
                a.to_bytes(s)?;
                b.to_bytes(s)?;
            }
            Self::QueueMoveInto(a, b) => {
                s.write_all(&[BYTE_QUEUE_MOVE_INTO])?;
                a.to_bytes(s)?;
                b.to_bytes(s)?;
            }
            Self::QueueGoto(index) => {
                s.write_all(&[BYTE_QUEUE_GOTO])?;
                index.to_bytes(s)?;
            }
            Self::QueueShuffle(path, set_index) => {
                s.write_all(&[BYTE_QUEUE_ACTION])?;
                s.write_all(&[SUBBYTE_ACTION_SHUFFLE])?;
                path.to_bytes(s)?;
                set_index.to_bytes(s)?;
            }
            Self::QueueSetShuffle(path, map, set_index) => {
                s.write_all(&[BYTE_QUEUE_ACTION])?;
                s.write_all(&[SUBBYTE_ACTION_SET_SHUFFLE])?;
                path.to_bytes(s)?;
                map.to_bytes(s)?;
                set_index.to_bytes(s)?;
            }
            Self::QueueUnshuffle(path) => {
                s.write_all(&[BYTE_QUEUE_ACTION])?;
                s.write_all(&[SUBBYTE_ACTION_UNSHUFFLE])?;
                path.to_bytes(s)?;
            }
            Self::AddSong(song, req) => {
                s.write_all(&[BYTE_LIB_ADD])?;
                s.write_all(&[SUBBYTE_SONG])?;
                song.to_bytes(s)?;
                req.to_bytes(s)?;
            }
            Self::AddAlbum(album, req) => {
                s.write_all(&[BYTE_LIB_ADD])?;
                s.write_all(&[SUBBYTE_ALBUM])?;
                album.to_bytes(s)?;
                req.to_bytes(s)?;
            }
            Self::AddArtist(artist, req) => {
                s.write_all(&[BYTE_LIB_ADD])?;
                s.write_all(&[SUBBYTE_ARTIST])?;
                artist.to_bytes(s)?;
                req.to_bytes(s)?;
            }
            Self::AddCover(cover, req) => {
                s.write_all(&[BYTE_LIB_ADD])?;
                s.write_all(&[SUBBYTE_COVER])?;
                cover.to_bytes(s)?;
                req.to_bytes(s)?;
            }
            Self::ModifySong(song, req) => {
                s.write_all(&[BYTE_LIB_MODIFY])?;
                s.write_all(&[SUBBYTE_SONG])?;
                song.to_bytes(s)?;
                req.to_bytes(s)?;
            }
            Self::ModifyAlbum(album, req) => {
                s.write_all(&[BYTE_LIB_MODIFY])?;
                s.write_all(&[SUBBYTE_ALBUM])?;
                album.to_bytes(s)?;
                req.to_bytes(s)?;
            }
            Self::ModifyArtist(artist, req) => {
                s.write_all(&[BYTE_LIB_MODIFY])?;
                s.write_all(&[SUBBYTE_ARTIST])?;
                artist.to_bytes(s)?;
                req.to_bytes(s)?;
            }
            Self::RemoveSong(song) => {
                s.write_all(&[BYTE_LIB_REMOVE])?;
                s.write_all(&[SUBBYTE_SONG])?;
                song.to_bytes(s)?;
            }
            Self::RemoveAlbum(album) => {
                s.write_all(&[BYTE_LIB_REMOVE])?;
                s.write_all(&[SUBBYTE_ALBUM])?;
                album.to_bytes(s)?;
            }
            Self::RemoveArtist(artist) => {
                s.write_all(&[BYTE_LIB_REMOVE])?;
                s.write_all(&[SUBBYTE_ARTIST])?;
                artist.to_bytes(s)?;
            }
            Self::TagSongFlagSet(id, tag) => {
                s.write_all(&[BYTE_LIB_TAG])?;
                s.write_all(&[SUBBYTE_TAG_SONG_FLAG_SET])?;
                id.to_bytes(s)?;
                tag.to_bytes(s)?;
            }
            Self::TagSongFlagUnset(id, tag) => {
                s.write_all(&[BYTE_LIB_TAG])?;
                s.write_all(&[SUBBYTE_TAG_SONG_FLAG_UNSET])?;
                id.to_bytes(s)?;
                tag.to_bytes(s)?;
            }
            Self::TagAlbumFlagSet(id, tag) => {
                s.write_all(&[BYTE_LIB_TAG])?;
                s.write_all(&[SUBBYTE_TAG_ALBUM_FLAG_SET])?;
                id.to_bytes(s)?;
                tag.to_bytes(s)?;
            }
            Self::TagAlbumFlagUnset(id, tag) => {
                s.write_all(&[BYTE_LIB_TAG])?;
                s.write_all(&[SUBBYTE_TAG_ALBUM_FLAG_UNSET])?;
                id.to_bytes(s)?;
                tag.to_bytes(s)?;
            }
            Self::TagArtistFlagSet(id, tag) => {
                s.write_all(&[BYTE_LIB_TAG])?;
                s.write_all(&[SUBBYTE_TAG_ARTIST_FLAG_SET])?;
                id.to_bytes(s)?;
                tag.to_bytes(s)?;
            }
            Self::TagArtistFlagUnset(id, tag) => {
                s.write_all(&[BYTE_LIB_TAG])?;
                s.write_all(&[SUBBYTE_TAG_ARTIST_FLAG_UNSET])?;
                id.to_bytes(s)?;
                tag.to_bytes(s)?;
            }
            Self::TagSongPropertySet(id, key, val) => {
                s.write_all(&[BYTE_LIB_TAG])?;
                s.write_all(&[SUBBYTE_TAG_SONG_PROPERTY_SET])?;
                id.to_bytes(s)?;
                key.to_bytes(s)?;
                val.to_bytes(s)?;
            }
            Self::TagSongPropertyUnset(id, key) => {
                s.write_all(&[BYTE_LIB_TAG])?;
                s.write_all(&[SUBBYTE_TAG_SONG_PROPERTY_UNSET])?;
                id.to_bytes(s)?;
                key.to_bytes(s)?;
            }
            Self::TagAlbumPropertySet(id, key, val) => {
                s.write_all(&[BYTE_LIB_TAG])?;
                s.write_all(&[SUBBYTE_TAG_ALBUM_PROPERTY_SET])?;
                id.to_bytes(s)?;
                key.to_bytes(s)?;
                val.to_bytes(s)?;
            }
            Self::TagAlbumPropertyUnset(id, key) => {
                s.write_all(&[BYTE_LIB_TAG])?;
                s.write_all(&[SUBBYTE_TAG_ALBUM_PROPERTY_UNSET])?;
                id.to_bytes(s)?;
                key.to_bytes(s)?;
            }
            Self::TagArtistPropertySet(id, key, val) => {
                s.write_all(&[BYTE_LIB_TAG])?;
                s.write_all(&[SUBBYTE_TAG_ARTIST_PROPERTY_SET])?;
                id.to_bytes(s)?;
                key.to_bytes(s)?;
                val.to_bytes(s)?;
            }
            Self::TagArtistPropertyUnset(id, key) => {
                s.write_all(&[BYTE_LIB_TAG])?;
                s.write_all(&[SUBBYTE_TAG_ARTIST_PROPERTY_UNSET])?;
                id.to_bytes(s)?;
                key.to_bytes(s)?;
            }
            Self::SetSongDuration(i, d) => {
                s.write_all(&[BYTE_SET_SONG_DURATION])?;
                i.to_bytes(s)?;
                d.to_bytes(s)?;
            }
            Self::Multiple(actions) => {
                s.write_all(&[BYTE_MULTIPLE])?;
                actions.to_bytes(s)?;
            }
            Self::InitComplete => {
                s.write_all(&[BYTE_INIT_COMPLETE])?;
            }
            Self::Save => s.write_all(&[BYTE_SAVE])?,
            Self::ErrorInfo(t, d) => {
                s.write_all(&[BYTE_ERRORINFO])?;
                t.to_bytes(s)?;
                d.to_bytes(s)?;
            }
            Self::Denied(req) => {
                s.write_all(&[BYTE_DENIED])?;
                req.to_bytes(s)?;
            }
        }
        Ok(())
    }
    fn from_bytes<T>(s: &mut T) -> Result<Self, std::io::Error>
    where
        T: std::io::Read,
    {
        macro_rules! from_bytes {
            () => {
                ToFromBytes::from_bytes(s)?
            };
        }
        Ok(match s.read_byte()? {
            BYTE_RESUME => Self::Resume,
            BYTE_PAUSE => Self::Pause,
            BYTE_STOP => Self::Stop,
            BYTE_NEXT_SONG => Self::NextSong,
            BYTE_SYNC_DATABASE => Self::SyncDatabase(from_bytes!(), from_bytes!(), from_bytes!()),
            BYTE_QUEUE_UPDATE => Self::QueueUpdate(from_bytes!(), from_bytes!(), from_bytes!()),
            BYTE_QUEUE_ADD => Self::QueueAdd(from_bytes!(), from_bytes!(), from_bytes!()),
            BYTE_QUEUE_INSERT => {
                Self::QueueInsert(from_bytes!(), from_bytes!(), from_bytes!(), from_bytes!())
            }
            BYTE_QUEUE_REMOVE => Self::QueueRemove(from_bytes!()),
            BYTE_QUEUE_MOVE => Self::QueueMove(from_bytes!(), from_bytes!()),
            BYTE_QUEUE_MOVE_INTO => Self::QueueMoveInto(from_bytes!(), from_bytes!()),
            BYTE_QUEUE_GOTO => Self::QueueGoto(from_bytes!()),
            BYTE_QUEUE_ACTION => match s.read_byte()? {
                SUBBYTE_ACTION_SHUFFLE => Self::QueueShuffle(from_bytes!(), from_bytes!()),
                SUBBYTE_ACTION_SET_SHUFFLE => {
                    Self::QueueSetShuffle(from_bytes!(), from_bytes!(), from_bytes!())
                }
                SUBBYTE_ACTION_UNSHUFFLE => Self::QueueUnshuffle(from_bytes!()),
                _ => {
                    eprintln!(
                        "[{}] unexpected byte when reading command:queueAction; stopping playback.",
                        "WARN".yellow()
                    );
                    Self::Stop
                }
            },
            BYTE_LIB_ADD => match s.read_byte()? {
                SUBBYTE_SONG => Self::AddSong(from_bytes!(), from_bytes!()),
                SUBBYTE_ALBUM => Self::AddAlbum(from_bytes!(), from_bytes!()),
                SUBBYTE_ARTIST => Self::AddArtist(from_bytes!(), from_bytes!()),
                SUBBYTE_COVER => Self::AddCover(from_bytes!(), from_bytes!()),
                _ => {
                    eprintln!(
                        "[{}] unexpected byte when reading command:libAdd; stopping playback.",
                        "WARN".yellow()
                    );
                    Self::Stop
                }
            },
            BYTE_LIB_MODIFY => match s.read_byte()? {
                SUBBYTE_SONG => Self::ModifySong(from_bytes!(), from_bytes!()),
                SUBBYTE_ALBUM => Self::ModifyAlbum(from_bytes!(), from_bytes!()),
                SUBBYTE_ARTIST => Self::ModifyArtist(from_bytes!(), from_bytes!()),
                _ => {
                    eprintln!(
                        "[{}] unexpected byte when reading command:libModify; stopping playback.",
                        "WARN".yellow()
                    );
                    Self::Stop
                }
            },
            BYTE_LIB_REMOVE => match s.read_byte()? {
                SUBBYTE_SONG => Self::RemoveSong(from_bytes!()),
                SUBBYTE_ALBUM => Self::RemoveAlbum(from_bytes!()),
                SUBBYTE_ARTIST => Self::RemoveArtist(from_bytes!()),
                _ => {
                    eprintln!(
                        "[{}] unexpected byte when reading command:libRemove; stopping playback.",
                        "WARN".yellow()
                    );
                    Self::Stop
                }
            },
            BYTE_LIB_TAG => match s.read_byte()? {
                SUBBYTE_TAG_SONG_FLAG_SET => Self::TagSongFlagSet(from_bytes!(), from_bytes!()),
                SUBBYTE_TAG_SONG_FLAG_UNSET => Self::TagSongFlagUnset(from_bytes!(), from_bytes!()),
                SUBBYTE_TAG_ALBUM_FLAG_SET => Self::TagAlbumFlagSet(from_bytes!(), from_bytes!()),
                SUBBYTE_TAG_ALBUM_FLAG_UNSET => {
                    Self::TagAlbumFlagUnset(from_bytes!(), from_bytes!())
                }
                SUBBYTE_TAG_ARTIST_FLAG_SET => Self::TagArtistFlagSet(from_bytes!(), from_bytes!()),
                SUBBYTE_TAG_ARTIST_FLAG_UNSET => {
                    Self::TagArtistFlagUnset(from_bytes!(), from_bytes!())
                }
                SUBBYTE_TAG_SONG_PROPERTY_SET => {
                    Self::TagSongPropertySet(from_bytes!(), from_bytes!(), from_bytes!())
                }
                SUBBYTE_TAG_SONG_PROPERTY_UNSET => {
                    Self::TagSongPropertyUnset(from_bytes!(), from_bytes!())
                }
                SUBBYTE_TAG_ALBUM_PROPERTY_SET => {
                    Self::TagAlbumPropertySet(from_bytes!(), from_bytes!(), from_bytes!())
                }
                SUBBYTE_TAG_ALBUM_PROPERTY_UNSET => {
                    Self::TagAlbumPropertyUnset(from_bytes!(), from_bytes!())
                }
                SUBBYTE_TAG_ARTIST_PROPERTY_SET => {
                    Self::TagArtistPropertySet(from_bytes!(), from_bytes!(), from_bytes!())
                }
                SUBBYTE_TAG_ARTIST_PROPERTY_UNSET => {
                    Self::TagArtistPropertyUnset(from_bytes!(), from_bytes!())
                }
                _ => {
                    eprintln!(
                        "[{}] unexpected byte when reading command:libTag; stopping playback.",
                        "WARN".yellow()
                    );
                    Self::Stop
                }
            },
            BYTE_SET_SONG_DURATION => Self::SetSongDuration(from_bytes!(), from_bytes!()),
            BYTE_MULTIPLE => Self::Multiple(from_bytes!()),
            BYTE_INIT_COMPLETE => Self::InitComplete,
            BYTE_SAVE => Self::Save,
            BYTE_ERRORINFO => Self::ErrorInfo(from_bytes!(), from_bytes!()),
            BYTE_DENIED => Self::Denied(from_bytes!()),
            _ => {
                eprintln!(
                    "[{}] unexpected byte when reading command; stopping playback.",
                    "WARN".yellow()
                );
                Self::Stop
            }
        })
    }
}

trait ReadByte {
    fn read_byte(&mut self) -> std::io::Result<u8>;
}
impl<T: Read> ReadByte for T {
    fn read_byte(&mut self) -> std::io::Result<u8> {
        let mut b = [0];
        self.read_exact(&mut b)?;
        Ok(b[0])
    }
}

#[test]
fn test_to_from_bytes() {
    use crate::data::queue::QueueContent;
    use std::io::Cursor;
    for v in [
        Action::Resume,
        Action::Pause,
        Action::Stop,
        Action::NextSong,
        Action::SyncDatabase(vec![], vec![], vec![]),
        Action::QueueUpdate(vec![], QueueContent::Song(12).into(), Req::none()),
        Action::QueueAdd(vec![], vec![], Req::none()),
        Action::QueueInsert(vec![], 5, vec![], Req::none()),
        Action::QueueRemove(vec![]),
        Action::QueueMove(vec![], vec![]),
        Action::QueueMoveInto(vec![], vec![]),
        Action::QueueGoto(vec![]),
        Action::QueueShuffle(vec![], 1),
        Action::QueueSetShuffle(vec![], vec![], 0),
        Action::QueueUnshuffle(vec![]),
        // Action::AddSong(Song, Req),
        // Action::AddAlbum(Album, Req),
        // Action::AddArtist(Artist, Req),
        // Action::AddCover(Cover, Req),
        // Action::ModifySong(Song, Req),
        // Action::ModifyAlbum(Album, Req),
        // Action::ModifyArtist(Artist, Req),
        // Action::RemoveSong(SongId),
        // Action::RemoveAlbum(AlbumId),
        // Action::RemoveArtist(ArtistId),
        // Action::SetSongDuration(SongId, u64),
        // Action::TagSongFlagSet(SongId, String),
        // Action::TagSongFlagUnset(SongId, String),
        // Action::TagAlbumFlagSet(AlbumId, String),
        // Action::TagAlbumFlagUnset(AlbumId, String),
        // Action::TagArtistFlagSet(ArtistId, String),
        // Action::TagArtistFlagUnset(ArtistId, String),
        // Action::TagSongPropertySet(SongId, String, String),
        // Action::TagSongPropertyUnset(SongId, String),
        // Action::TagAlbumPropertySet(AlbumId, String, String),
        // Action::TagAlbumPropertyUnset(AlbumId, String),
        // Action::TagArtistPropertySet(ArtistId, String, String),
        // Action::TagArtistPropertyUnset(ArtistId, String),
        Action::InitComplete,
        Action::Save,
        Action::ErrorInfo(format!("some error"), format!("with a message")),
        Action::Denied(Req::none()),
    ] {
        let v = v.cmd(0xFF);
        assert_eq!(
            v.action,
            Command::from_bytes(&mut Cursor::new(v.to_bytes_vec()))
                .unwrap()
                .action
        );
    }
}
