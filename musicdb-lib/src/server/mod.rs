pub mod get;

use std::{
    eprintln,
    io::{BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpListener},
    path::PathBuf,
    sync::{mpsc, Arc, Mutex},
    thread,
    time::Duration,
};

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
    player::Player,
    server::get::handle_one_connection_as_get,
};

#[derive(Clone, Debug)]
pub enum Command {
    Resume,
    Pause,
    Stop,
    Save,
    NextSong,
    SyncDatabase(Vec<Artist>, Vec<Album>, Vec<Song>),
    QueueUpdate(Vec<usize>, Queue),
    QueueAdd(Vec<usize>, Queue),
    QueueInsert(Vec<usize>, usize, Queue),
    QueueRemove(Vec<usize>),
    QueueGoto(Vec<usize>),
    QueueSetShuffle(Vec<usize>, Vec<usize>, usize),
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
    SetLibraryDirectory(PathBuf),
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
pub fn run_server(
    database: Arc<Mutex<Database>>,
    addr_tcp: Option<SocketAddr>,
    sender_sender: Option<tokio::sync::mpsc::Sender<mpsc::Sender<Command>>>,
) {
    let mut player = Player::new().unwrap();
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
                    if let Ok((connection, con_addr)) = v.accept() {
                        let command_sender = command_sender.clone();
                        let db = Arc::clone(&db);
                        thread::spawn(move || {
                            eprintln!("[info] TCP connection accepted from {con_addr}.");
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
                eprintln!("[WARN] Couldn't start TCP listener: {e}");
            }
        }
    }
    // for now, update the player 10 times a second so it can detect when a song has finished and start a new one.
    // TODO: player should send a NextSong update to the mpsc::Sender to wake up this thread
    let dur = Duration::from_secs_f32(0.1);
    loop {
        player.update(&mut database.lock().unwrap());
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
impl ToFromBytes for Command {
    fn to_bytes<T>(&self, s: &mut T) -> Result<(), std::io::Error>
    where
        T: Write,
    {
        match self {
            Self::Resume => s.write_all(&[0b11000000])?,
            Self::Pause => s.write_all(&[0b00110000])?,
            Self::Stop => s.write_all(&[0b11110000])?,
            Self::Save => s.write_all(&[0b11110011])?,
            Self::NextSong => s.write_all(&[0b11110010])?,
            Self::SyncDatabase(a, b, c) => {
                s.write_all(&[0b01011000])?;
                a.to_bytes(s)?;
                b.to_bytes(s)?;
                c.to_bytes(s)?;
            }
            Self::QueueUpdate(index, new_data) => {
                s.write_all(&[0b00011100])?;
                index.to_bytes(s)?;
                new_data.to_bytes(s)?;
            }
            Self::QueueAdd(index, new_data) => {
                s.write_all(&[0b00011010])?;
                index.to_bytes(s)?;
                new_data.to_bytes(s)?;
            }
            Self::QueueInsert(index, pos, new_data) => {
                s.write_all(&[0b00011110])?;
                index.to_bytes(s)?;
                pos.to_bytes(s)?;
                new_data.to_bytes(s)?;
            }
            Self::QueueRemove(index) => {
                s.write_all(&[0b00011001])?;
                index.to_bytes(s)?;
            }
            Self::QueueGoto(index) => {
                s.write_all(&[0b00011011])?;
                index.to_bytes(s)?;
            }
            Self::QueueSetShuffle(path, map, next) => {
                s.write_all(&[0b10011011])?;
                path.to_bytes(s)?;
                map.to_bytes(s)?;
                next.to_bytes(s)?;
            }
            Self::AddSong(song) => {
                s.write_all(&[0b01010000])?;
                song.to_bytes(s)?;
            }
            Self::AddAlbum(album) => {
                s.write_all(&[0b01010011])?;
                album.to_bytes(s)?;
            }
            Self::AddArtist(artist) => {
                s.write_all(&[0b01011100])?;
                artist.to_bytes(s)?;
            }
            Self::AddCover(cover) => {
                s.write_all(&[0b01011101])?;
                cover.to_bytes(s)?;
            }
            Self::ModifySong(song) => {
                s.write_all(&[0b10010000])?;
                song.to_bytes(s)?;
            }
            Self::ModifyAlbum(album) => {
                s.write_all(&[0b10010011])?;
                album.to_bytes(s)?;
            }
            Self::ModifyArtist(artist) => {
                s.write_all(&[0b10011100])?;
                artist.to_bytes(s)?;
            }
            Self::RemoveSong(song) => {
                s.write_all(&[0b11010000])?;
                song.to_bytes(s)?;
            }
            Self::RemoveAlbum(album) => {
                s.write_all(&[0b11010011])?;
                album.to_bytes(s)?;
            }
            Self::RemoveArtist(artist) => {
                s.write_all(&[0b11011100])?;
                artist.to_bytes(s)?;
            }
            Self::SetLibraryDirectory(path) => {
                s.write_all(&[0b00110001])?;
                path.to_bytes(s)?;
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
        Ok(match kind[0] {
            0b11000000 => Self::Resume,
            0b00110000 => Self::Pause,
            0b11110000 => Self::Stop,
            0b11110011 => Self::Save,
            0b11110010 => Self::NextSong,
            0b01011000 => Self::SyncDatabase(
                ToFromBytes::from_bytes(s)?,
                ToFromBytes::from_bytes(s)?,
                ToFromBytes::from_bytes(s)?,
            ),
            0b00011100 => {
                Self::QueueUpdate(ToFromBytes::from_bytes(s)?, ToFromBytes::from_bytes(s)?)
            }
            0b00011010 => Self::QueueAdd(ToFromBytes::from_bytes(s)?, ToFromBytes::from_bytes(s)?),
            0b00011110 => Self::QueueInsert(
                ToFromBytes::from_bytes(s)?,
                ToFromBytes::from_bytes(s)?,
                ToFromBytes::from_bytes(s)?,
            ),
            0b00011001 => Self::QueueRemove(ToFromBytes::from_bytes(s)?),
            0b00011011 => Self::QueueGoto(ToFromBytes::from_bytes(s)?),
            0b10011011 => Self::QueueSetShuffle(
                ToFromBytes::from_bytes(s)?,
                ToFromBytes::from_bytes(s)?,
                ToFromBytes::from_bytes(s)?,
            ),
            0b01010000 => Self::AddSong(ToFromBytes::from_bytes(s)?),
            0b01010011 => Self::AddAlbum(ToFromBytes::from_bytes(s)?),
            0b01011100 => Self::AddArtist(ToFromBytes::from_bytes(s)?),
            0b10010000 => Self::ModifySong(ToFromBytes::from_bytes(s)?),
            0b10010011 => Self::ModifyAlbum(ToFromBytes::from_bytes(s)?),
            0b10011100 => Self::ModifyArtist(ToFromBytes::from_bytes(s)?),
            0b11010000 => Self::RemoveSong(ToFromBytes::from_bytes(s)?),
            0b11010011 => Self::RemoveAlbum(ToFromBytes::from_bytes(s)?),
            0b11011100 => Self::RemoveArtist(ToFromBytes::from_bytes(s)?),
            0b01011101 => Self::AddCover(ToFromBytes::from_bytes(s)?),
            0b00110001 => Self::SetLibraryDirectory(ToFromBytes::from_bytes(s)?),
            _ => {
                eprintln!("unexpected byte when reading command; stopping playback.");
                Self::Stop
            }
        })
    }
}
