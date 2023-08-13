use std::{
    eprintln,
    io::Write,
    net::{SocketAddr, TcpListener},
    path::PathBuf,
    sync::{mpsc, Arc, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};

use crate::{
    data::{
        album::Album,
        artist::Artist,
        database::{Database, UpdateEndpoint},
        queue::Queue,
        song::Song,
    },
    load::ToFromBytes,
    player::Player,
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
    /// .id field is ignored!
    AddSong(Song),
    /// .id field is ignored!
    AddAlbum(Album),
    /// .id field is ignored!
    AddArtist(Artist),
    ModifySong(Song),
    ModifyAlbum(Album),
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
                    if let Ok((mut connection, con_addr)) = v.accept() {
                        eprintln!("[info] TCP connection accepted from {con_addr}.");
                        let command_sender = command_sender.clone();
                        let db = Arc::clone(&db);
                        thread::spawn(move || {
                            // sync database
                            let mut db = db.lock().unwrap();
                            db.init_connection(&mut connection)?;
                            db.update_endpoints.push(UpdateEndpoint::Bytes(Box::new(
                                connection.try_clone().unwrap(),
                            )));
                            drop(db);
                            loop {
                                if let Ok(command) = Command::from_bytes(&mut connection) {
                                    command_sender.send(command).unwrap();
                                } else {
                                    break;
                                }
                            }
                            Ok::<(), std::io::Error>(())
                        });
                    }
                });
            }
            Err(e) => {
                eprintln!("[WARN] Couldn't start TCP listener: {e}");
            }
        }
    }
    let dur = Duration::from_secs_f32(0.1);
    loop {
        player.update(&mut database.lock().unwrap());
        if let Ok(command) = command_receiver.recv_timeout(dur) {
            player.handle_command(&command);
            database.lock().unwrap().apply_command(command);
        }
    }
}

pub trait Connection: Sized + Send + 'static {
    type SendError: Send;
    fn send_command(&mut self, command: Command) -> Result<(), Self::SendError>;
    fn receive_updates(&mut self) -> Result<Vec<Command>, Self::SendError>;
    fn receive_update_blocking(&mut self) -> Result<Command, Self::SendError>;
    fn move_to_thread<F: FnMut(&mut Self, Command) -> bool + Send + 'static>(
        mut self,
        mut handler: F,
    ) -> JoinHandle<Result<Self, Self::SendError>> {
        std::thread::spawn(move || loop {
            let update = self.receive_update_blocking()?;
            if handler(&mut self, update) {
                return Ok(self);
            }
        })
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
            0b01010000 => Self::AddSong(ToFromBytes::from_bytes(s)?),
            0b01010011 => Self::AddAlbum(ToFromBytes::from_bytes(s)?),
            0b01011100 => Self::AddArtist(ToFromBytes::from_bytes(s)?),
            0b10010000 => Self::AddSong(ToFromBytes::from_bytes(s)?),
            0b10010011 => Self::AddAlbum(ToFromBytes::from_bytes(s)?),
            0b10011100 => Self::AddArtist(ToFromBytes::from_bytes(s)?),
            0b00110001 => Self::SetLibraryDirectory(ToFromBytes::from_bytes(s)?),
            _ => {
                eprintln!("unexpected byte when reading command; stopping playback.");
                Self::Stop
            }
        })
    }
}
