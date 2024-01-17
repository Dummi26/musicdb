pub mod get;

use std::{
    io::{Read, Write},
    sync::{mpsc, Arc, Mutex},
};

use colorize::AnsiColor;

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
#[cfg(feature = "playback")]
use crate::{player::Player, server::get::handle_one_connection_as_get};
#[cfg(feature = "playback")]
use std::{
    io::{BufRead, BufReader},
    net::{SocketAddr, TcpListener},
    thread,
    time::Duration,
};

#[derive(Clone, Debug)]
pub enum Command {
    Resume,
    Pause,
    Stop,
    NextSong,
    SyncDatabase(Vec<Artist>, Vec<Album>, Vec<Song>),
    QueueUpdate(Vec<usize>, Queue),
    QueueAdd(Vec<usize>, Vec<Queue>),
    QueueInsert(Vec<usize>, usize, Vec<Queue>),
    QueueRemove(Vec<usize>),
    QueueGoto(Vec<usize>),
    QueueSetShuffle(Vec<usize>, Vec<usize>),

    /// .id field is ignored!
    AddSong(Song),
    /// .id field is ignored!
    AddAlbum(Album),
    /// .id field is ignored!
    AddArtist(Artist),
    AddCover(Cover),
    ModifySong(Song),
    ModifyAlbum(Album),
    RemoveSong(SongId),
    RemoveAlbum(AlbumId),
    RemoveArtist(ArtistId),
    ModifyArtist(Artist),
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

    InitComplete,
    Save,
    ErrorInfo(String, String),
}
impl Command {
    pub fn send_to_server(self, db: &Database) -> Result<(), Self> {
        if let Some(sender) = &db.command_sender {
            sender.send(self).unwrap();
            Ok(())
        } else {
            Err(self)
        }
    }
    pub fn send_to_server_or_apply(self, db: &mut Database) {
        if let Some(sender) = &db.command_sender {
            sender.send(self).unwrap();
        } else {
            db.apply_command(self);
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
#[cfg(feature = "playback")]
pub fn run_server(
    database: Arc<Mutex<Database>>,
    addr_tcp: Option<SocketAddr>,
    sender_sender: Option<tokio::sync::mpsc::Sender<mpsc::Sender<Command>>>,
) {
    run_server_caching_thread_opt(database, addr_tcp, sender_sender, None)
}
#[cfg(feature = "playback")]
pub fn run_server_caching_thread_opt(
    database: Arc<Mutex<Database>>,
    addr_tcp: Option<SocketAddr>,
    sender_sender: Option<tokio::sync::mpsc::Sender<mpsc::Sender<Command>>>,
    caching_thread: Option<Box<dyn FnOnce(&mut crate::data::cache_manager::CacheManager)>>,
) {
    use std::time::Instant;

    use crate::data::cache_manager::CacheManager;

    let mut player = Player::new().unwrap();
    let cache_manager = if let Some(func) = caching_thread {
        let mut cm = CacheManager::new(Arc::clone(&database));
        func(&mut cm);
        Some(cm)
    } else {
        None
    };
    // commands sent to this will be handeled later in this function in an infinite loop.
    // these commands are sent to the database asap.
    let (command_sender, command_receiver) = mpsc::channel();
    if let Some(s) = sender_sender {
        s.blocking_send(command_sender.clone()).unwrap();
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
    let dur = Duration::from_secs(10);
    let command_sender = Arc::new(move |cmd| {
        _ = command_sender.send(cmd);
    });
    loop {
        {
            // at the start and once after every command sent to the server,
            let mut db = database.lock().unwrap();
            // update the player
            if cache_manager.is_some() {
                player.update_dont_uncache(&mut db, &command_sender);
            } else {
                player.update(&mut db, &command_sender);
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
        if let Ok(command) = command_receiver.recv_timeout(dur) {
            player.handle_command(&command);
            database.lock().unwrap().apply_command(command);
        }
    }
}

pub fn handle_one_connection_as_main(
    db: Arc<Mutex<Database>>,
    connection: &mut impl Read,
    mut send_to: (impl Write + Sync + Send + 'static),
    command_sender: &mpsc::Sender<Command>,
) -> Result<(), std::io::Error> {
    // sync database
    let mut db = db.lock().unwrap();
    db.init_connection(&mut send_to)?;
    // keep the client in sync:
    // the db will send all updates to the client once it is added to update_endpoints
    db.update_endpoints.push(UpdateEndpoint::Bytes(Box::new(
        // try_clone is used here to split a TcpStream into Writer and Reader
        send_to,
    )));
    // drop the mutex lock
    drop(db);
    handle_one_connection_as_control(connection, command_sender);
    Ok(())
}
pub fn handle_one_connection_as_control(
    connection: &mut impl Read,
    command_sender: &mpsc::Sender<Command>,
) {
    // read updates from the tcp stream and send them to the database, exit on EOF or Err
    loop {
        if let Ok(command) = Command::from_bytes(connection) {
            command_sender.send(command).unwrap();
        } else {
            break;
        }
    }
}

const BYTE_RESUME: u8 = 0b11000000;
const BYTE_PAUSE: u8 = 0b00110000;
const BYTE_STOP: u8 = 0b11110000;
const BYTE_NEXT_SONG: u8 = 0b11110010;

const BYTE_SYNC_DATABASE: u8 = 0b01011000;
const BYTE_QUEUE_UPDATE: u8 = 0b00011100;
const BYTE_QUEUE_ADD: u8 = 0b00011010;
const BYTE_QUEUE_INSERT: u8 = 0b00011110;
const BYTE_QUEUE_REMOVE: u8 = 0b00011001;
const BYTE_QUEUE_GOTO: u8 = 0b00011011;
const BYTE_QUEUE_SET_SHUFFLE: u8 = 0b10011011;

const BYTE_ADD_SONG: u8 = 0b01010000;
const BYTE_ADD_ALBUM: u8 = 0b01010011;
const BYTE_ADD_ARTIST: u8 = 0b01011100;
const BYTE_ADD_COVER: u8 = 0b01011101;
const BYTE_MODIFY_SONG: u8 = 0b10010000;
const BYTE_MODIFY_ALBUM: u8 = 0b10010011;
const BYTE_MODIFY_ARTIST: u8 = 0b10011100;
const BYTE_REMOVE_SONG: u8 = 0b11010000;
const BYTE_REMOVE_ALBUM: u8 = 0b11010011;
const BYTE_REMOVE_ARTIST: u8 = 0b11011100;
const BYTE_TAG_SONG_FLAG_SET: u8 = 0b11100000;
const BYTE_TAG_SONG_FLAG_UNSET: u8 = 0b11100001;
const BYTE_TAG_ALBUM_FLAG_SET: u8 = 0b11100010;
const BYTE_TAG_ALBUM_FLAG_UNSET: u8 = 0b11100011;
const BYTE_TAG_ARTIST_FLAG_SET: u8 = 0b11100110;
const BYTE_TAG_ARTIST_FLAG_UNSET: u8 = 0b11100111;
const BYTE_TAG_SONG_PROPERTY_SET: u8 = 0b11101001;
const BYTE_TAG_SONG_PROPERTY_UNSET: u8 = 0b11101010;
const BYTE_TAG_ALBUM_PROPERTY_SET: u8 = 0b11101011;
const BYTE_TAG_ALBUM_PROPERTY_UNSET: u8 = 0b11101100;
const BYTE_TAG_ARTIST_PROPERTY_SET: u8 = 0b11101110;
const BYTE_TAG_ARTIST_PROPERTY_UNSET: u8 = 0b11101111;

const BYTE_SET_SONG_DURATION: u8 = 0b11111000;

const BYTE_INIT_COMPLETE: u8 = 0b00110001;
const BYTE_SAVE: u8 = 0b11110011;
const BYTE_ERRORINFO: u8 = 0b11011011;

impl ToFromBytes for Command {
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
            Self::QueueUpdate(index, new_data) => {
                s.write_all(&[BYTE_QUEUE_UPDATE])?;
                index.to_bytes(s)?;
                new_data.to_bytes(s)?;
            }
            Self::QueueAdd(index, new_data) => {
                s.write_all(&[BYTE_QUEUE_ADD])?;
                index.to_bytes(s)?;
                new_data.to_bytes(s)?;
            }
            Self::QueueInsert(index, pos, new_data) => {
                s.write_all(&[BYTE_QUEUE_INSERT])?;
                index.to_bytes(s)?;
                pos.to_bytes(s)?;
                new_data.to_bytes(s)?;
            }
            Self::QueueRemove(index) => {
                s.write_all(&[BYTE_QUEUE_REMOVE])?;
                index.to_bytes(s)?;
            }
            Self::QueueGoto(index) => {
                s.write_all(&[BYTE_QUEUE_GOTO])?;
                index.to_bytes(s)?;
            }
            Self::QueueSetShuffle(path, map) => {
                s.write_all(&[BYTE_QUEUE_SET_SHUFFLE])?;
                path.to_bytes(s)?;
                map.to_bytes(s)?;
            }
            Self::AddSong(song) => {
                s.write_all(&[BYTE_ADD_SONG])?;
                song.to_bytes(s)?;
            }
            Self::AddAlbum(album) => {
                s.write_all(&[BYTE_ADD_ALBUM])?;
                album.to_bytes(s)?;
            }
            Self::AddArtist(artist) => {
                s.write_all(&[BYTE_ADD_ARTIST])?;
                artist.to_bytes(s)?;
            }
            Self::AddCover(cover) => {
                s.write_all(&[BYTE_ADD_COVER])?;
                cover.to_bytes(s)?;
            }
            Self::ModifySong(song) => {
                s.write_all(&[BYTE_MODIFY_SONG])?;
                song.to_bytes(s)?;
            }
            Self::ModifyAlbum(album) => {
                s.write_all(&[BYTE_MODIFY_ALBUM])?;
                album.to_bytes(s)?;
            }
            Self::ModifyArtist(artist) => {
                s.write_all(&[BYTE_MODIFY_ARTIST])?;
                artist.to_bytes(s)?;
            }
            Self::RemoveSong(song) => {
                s.write_all(&[BYTE_REMOVE_SONG])?;
                song.to_bytes(s)?;
            }
            Self::RemoveAlbum(album) => {
                s.write_all(&[BYTE_REMOVE_ALBUM])?;
                album.to_bytes(s)?;
            }
            Self::RemoveArtist(artist) => {
                s.write_all(&[BYTE_REMOVE_ARTIST])?;
                artist.to_bytes(s)?;
            }
            Self::TagSongFlagSet(id, tag) => {
                s.write_all(&[BYTE_TAG_SONG_FLAG_SET])?;
                id.to_bytes(s)?;
                tag.to_bytes(s)?;
            }
            Self::TagSongFlagUnset(id, tag) => {
                s.write_all(&[BYTE_TAG_SONG_FLAG_UNSET])?;
                id.to_bytes(s)?;
                tag.to_bytes(s)?;
            }
            Self::TagAlbumFlagSet(id, tag) => {
                s.write_all(&[BYTE_TAG_ALBUM_FLAG_SET])?;
                id.to_bytes(s)?;
                tag.to_bytes(s)?;
            }
            Self::TagAlbumFlagUnset(id, tag) => {
                s.write_all(&[BYTE_TAG_ALBUM_FLAG_UNSET])?;
                id.to_bytes(s)?;
                tag.to_bytes(s)?;
            }
            Self::TagArtistFlagSet(id, tag) => {
                s.write_all(&[BYTE_TAG_ARTIST_FLAG_SET])?;
                id.to_bytes(s)?;
                tag.to_bytes(s)?;
            }
            Self::TagArtistFlagUnset(id, tag) => {
                s.write_all(&[BYTE_TAG_ARTIST_FLAG_UNSET])?;
                id.to_bytes(s)?;
                tag.to_bytes(s)?;
            }
            Self::TagSongPropertySet(id, key, val) => {
                s.write_all(&[BYTE_TAG_SONG_PROPERTY_SET])?;
                id.to_bytes(s)?;
                key.to_bytes(s)?;
                val.to_bytes(s)?;
            }
            Self::TagSongPropertyUnset(id, key) => {
                s.write_all(&[BYTE_TAG_SONG_PROPERTY_UNSET])?;
                id.to_bytes(s)?;
                key.to_bytes(s)?;
            }
            Self::TagAlbumPropertySet(id, key, val) => {
                s.write_all(&[BYTE_TAG_ALBUM_PROPERTY_SET])?;
                id.to_bytes(s)?;
                key.to_bytes(s)?;
                val.to_bytes(s)?;
            }
            Self::TagAlbumPropertyUnset(id, key) => {
                s.write_all(&[BYTE_TAG_ALBUM_PROPERTY_UNSET])?;
                id.to_bytes(s)?;
                key.to_bytes(s)?;
            }
            Self::TagArtistPropertySet(id, key, val) => {
                s.write_all(&[BYTE_TAG_ARTIST_PROPERTY_SET])?;
                id.to_bytes(s)?;
                key.to_bytes(s)?;
                val.to_bytes(s)?;
            }
            Self::TagArtistPropertyUnset(id, key) => {
                s.write_all(&[BYTE_TAG_ARTIST_PROPERTY_UNSET])?;
                id.to_bytes(s)?;
                key.to_bytes(s)?;
            }
            Self::SetSongDuration(i, d) => {
                s.write_all(&[BYTE_SET_SONG_DURATION])?;
                i.to_bytes(s)?;
                d.to_bytes(s)?;
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
        }
        Ok(())
    }
    fn from_bytes<T>(s: &mut T) -> Result<Self, std::io::Error>
    where
        T: std::io::Read,
    {
        let mut kind = [0];
        s.read_exact(&mut kind)?;
        macro_rules! from_bytes {
            () => {
                ToFromBytes::from_bytes(s)?
            };
        }
        Ok(match kind[0] {
            BYTE_RESUME => Self::Resume,
            BYTE_PAUSE => Self::Pause,
            BYTE_STOP => Self::Stop,
            BYTE_NEXT_SONG => Self::NextSong,
            BYTE_SYNC_DATABASE => Self::SyncDatabase(from_bytes!(), from_bytes!(), from_bytes!()),
            BYTE_QUEUE_UPDATE => Self::QueueUpdate(from_bytes!(), from_bytes!()),
            BYTE_QUEUE_ADD => Self::QueueAdd(from_bytes!(), from_bytes!()),
            BYTE_QUEUE_INSERT => Self::QueueInsert(from_bytes!(), from_bytes!(), from_bytes!()),
            BYTE_QUEUE_REMOVE => Self::QueueRemove(from_bytes!()),
            BYTE_QUEUE_GOTO => Self::QueueGoto(from_bytes!()),
            BYTE_QUEUE_SET_SHUFFLE => Self::QueueSetShuffle(from_bytes!(), from_bytes!()),
            BYTE_ADD_SONG => Self::AddSong(from_bytes!()),
            BYTE_ADD_ALBUM => Self::AddAlbum(from_bytes!()),
            BYTE_ADD_ARTIST => Self::AddArtist(from_bytes!()),
            BYTE_MODIFY_SONG => Self::ModifySong(from_bytes!()),
            BYTE_MODIFY_ALBUM => Self::ModifyAlbum(from_bytes!()),
            BYTE_MODIFY_ARTIST => Self::ModifyArtist(from_bytes!()),
            BYTE_REMOVE_SONG => Self::RemoveSong(from_bytes!()),
            BYTE_REMOVE_ALBUM => Self::RemoveAlbum(from_bytes!()),
            BYTE_REMOVE_ARTIST => Self::RemoveArtist(from_bytes!()),
            BYTE_TAG_SONG_FLAG_SET => Self::TagSongFlagSet(from_bytes!(), from_bytes!()),
            BYTE_TAG_SONG_FLAG_UNSET => Self::TagSongFlagUnset(from_bytes!(), from_bytes!()),
            BYTE_TAG_ALBUM_FLAG_SET => Self::TagAlbumFlagSet(from_bytes!(), from_bytes!()),
            BYTE_TAG_ALBUM_FLAG_UNSET => Self::TagAlbumFlagUnset(from_bytes!(), from_bytes!()),
            BYTE_TAG_ARTIST_FLAG_SET => Self::TagArtistFlagSet(from_bytes!(), from_bytes!()),
            BYTE_TAG_ARTIST_FLAG_UNSET => Self::TagArtistFlagUnset(from_bytes!(), from_bytes!()),
            BYTE_TAG_SONG_PROPERTY_SET => {
                Self::TagSongPropertySet(from_bytes!(), from_bytes!(), from_bytes!())
            }
            BYTE_TAG_SONG_PROPERTY_UNSET => {
                Self::TagSongPropertyUnset(from_bytes!(), from_bytes!())
            }
            BYTE_TAG_ALBUM_PROPERTY_SET => {
                Self::TagAlbumPropertySet(from_bytes!(), from_bytes!(), from_bytes!())
            }
            BYTE_TAG_ALBUM_PROPERTY_UNSET => {
                Self::TagAlbumPropertyUnset(from_bytes!(), from_bytes!())
            }
            BYTE_TAG_ARTIST_PROPERTY_SET => {
                Self::TagArtistPropertySet(from_bytes!(), from_bytes!(), from_bytes!())
            }
            BYTE_TAG_ARTIST_PROPERTY_UNSET => {
                Self::TagArtistPropertyUnset(from_bytes!(), from_bytes!())
            }
            BYTE_SET_SONG_DURATION => Self::SetSongDuration(from_bytes!(), from_bytes!()),
            BYTE_ADD_COVER => Self::AddCover(from_bytes!()),
            BYTE_INIT_COMPLETE => Self::InitComplete,
            BYTE_SAVE => Self::Save,
            BYTE_ERRORINFO => Self::ErrorInfo(from_bytes!(), from_bytes!()),
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
