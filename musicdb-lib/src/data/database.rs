use std::{
    collections::HashMap,
    fs::{self, File},
    io::{BufReader, Read, Write},
    path::PathBuf,
    sync::{mpsc, Arc, Mutex},
    time::{Duration, Instant},
};

use crate::{load::ToFromBytes, server::Command};

use super::{
    album::Album,
    artist::Artist,
    queue::{Queue, QueueContent},
    song::Song,
    AlbumId, ArtistId, CoverId, DatabaseLocation, SongId,
};

pub struct Database {
    /// the path to the file used to save/load the data. empty if database is in client mode.
    pub db_file: PathBuf,
    /// the path to the directory containing the actual music and cover image files
    pub lib_directory: PathBuf,
    artists: HashMap<ArtistId, Artist>,
    albums: HashMap<AlbumId, Album>,
    songs: HashMap<SongId, Song>,
    covers: HashMap<CoverId, Cover>,
    // These will be used for autosave once that gets implemented
    db_data_file_change_first: Option<Instant>,
    db_data_file_change_last: Option<Instant>,
    pub queue: Queue,
    /// if the database receives an update, it will inform all of its clients so they can stay in sync.
    /// this is a list containing all the clients.
    pub update_endpoints: Vec<UpdateEndpoint>,
    /// true if a song is/should be playing
    pub playing: bool,
    pub command_sender: Option<mpsc::Sender<Command>>,
    pub remote_server_as_song_file_source:
        Option<Arc<Mutex<crate::server::get::Client<Box<dyn ClientIo>>>>>,
}
pub trait ClientIo: Read + Write + Send {}
impl<T: Read + Write + Send> ClientIo for T {}
// for custom server implementations, this enum should allow you to deal with updates from any context (writers such as tcp streams, sync/async mpsc senders, or via closure as a fallback)
pub enum UpdateEndpoint {
    Bytes(Box<dyn Write + Sync + Send>),
    CmdChannel(mpsc::Sender<Arc<Command>>),
    CmdChannelTokio(tokio::sync::mpsc::UnboundedSender<Arc<Command>>),
    Custom(Box<dyn FnMut(&Command) + Send>),
}

impl Database {
    /// TODO!
    fn panic(&self, msg: &str) -> ! {
        // custom panic handler
        // make a backup
        // exit
        panic!("DatabasePanic: {msg}");
    }
    pub fn is_client(&self) -> bool {
        self.db_file.as_os_str().is_empty()
    }
    pub fn get_path(&self, location: &DatabaseLocation) -> PathBuf {
        self.lib_directory.join(&location.rel_path)
    }
    // NOTE: just use `songs` directly? not sure yet...
    pub fn get_song(&self, song: &SongId) -> Option<&Song> {
        self.songs.get(song)
    }
    pub fn get_song_mut(&mut self, song: &SongId) -> Option<&mut Song> {
        self.songs.get_mut(song)
    }
    /// adds a song to the database.
    /// ignores song.id and just assigns a new id, which it then returns.
    /// this function also adds a reference to the new song to the album (or artist.singles, if no album)
    pub fn add_song_new(&mut self, song: Song) -> SongId {
        let album = song.album.clone();
        let artist = song.artist.clone();
        let id = self.add_song_new_nomagic(song);
        if let Some(Some(album)) = album.map(|v| self.albums.get_mut(&v)) {
            album.songs.push(id);
        } else {
            if let Some(Some(artist)) = artist.map(|v| self.artists.get_mut(&v)) {
                artist.singles.push(id);
            }
        }
        id
    }
    /// used internally
    pub fn add_song_new_nomagic(&mut self, mut song: Song) -> SongId {
        for key in 0.. {
            if !self.songs.contains_key(&key) {
                song.id = key;
                self.songs.insert(key, song);
                return key;
            }
        }
        self.panic("database.songs all keys used - no more capacity for new songs!");
    }
    /// adds an artist to the database.
    /// ignores artist.id and just assigns a new id, which it then returns.
    /// this function does nothing special.
    pub fn add_artist_new(&mut self, artist: Artist) -> ArtistId {
        let id = self.add_artist_new_nomagic(artist);
        id
    }
    /// used internally
    fn add_artist_new_nomagic(&mut self, mut artist: Artist) -> ArtistId {
        for key in 0.. {
            if !self.artists.contains_key(&key) {
                artist.id = key;
                self.artists.insert(key, artist);
                return key;
            }
        }
        self.panic("database.artists all keys used - no more capacity for new artists!");
    }
    /// adds an album to the database.
    /// ignores album.id and just assigns a new id, which it then returns.
    /// this function also adds a reference to the new album to the artist
    pub fn add_album_new(&mut self, album: Album) -> AlbumId {
        let artist = album.artist.clone();
        let id = self.add_album_new_nomagic(album);
        if let Some(Some(artist)) = artist.map(|v| self.artists.get_mut(&v)) {
            artist.albums.push(id);
        }
        id
    }
    /// used internally
    fn add_album_new_nomagic(&mut self, mut album: Album) -> AlbumId {
        for key in 0.. {
            if !self.albums.contains_key(&key) {
                album.id = key;
                self.albums.insert(key, album);
                return key;
            }
        }
        self.panic("database.artists all keys used - no more capacity for new artists!");
    }
    /// adds a cover to the database.
    /// assigns a new id, which it then returns.
    pub fn add_cover_new(&mut self, cover: Cover) -> AlbumId {
        self.add_cover_new_nomagic(cover)
    }
    /// used internally
    fn add_cover_new_nomagic(&mut self, cover: Cover) -> AlbumId {
        for key in 0.. {
            if !self.covers.contains_key(&key) {
                self.covers.insert(key, cover);
                return key;
            }
        }
        self.panic("database.artists all keys used - no more capacity for new artists!");
    }
    /// updates an existing song in the database with the new value.
    /// uses song.id to find the correct song.
    /// if the id doesn't exist in the db, Err(()) is returned.
    /// Otherwise Some(old_data) is returned.
    pub fn update_song(&mut self, song: Song) -> Result<Song, ()> {
        if let Some(prev_song) = self.songs.get_mut(&song.id) {
            Ok(std::mem::replace(prev_song, song))
        } else {
            Err(())
        }
    }
    pub fn update_album(&mut self, album: Album) -> Result<Album, ()> {
        if let Some(prev_album) = self.albums.get_mut(&album.id) {
            Ok(std::mem::replace(prev_album, album))
        } else {
            Err(())
        }
    }
    pub fn update_artist(&mut self, artist: Artist) -> Result<Artist, ()> {
        if let Some(prev_artist) = self.artists.get_mut(&artist.id) {
            Ok(std::mem::replace(prev_artist, artist))
        } else {
            Err(())
        }
    }
    /// [NOT RECOMMENDED - use add_song_new or update_song instead!] inserts the song into the database.
    /// uses song.id. If another song with that ID exists, it is replaced and Some(other_song) is returned.
    /// If no other song exists, the song will be added to the database with the given ID and None is returned.
    pub fn update_or_add_song(&mut self, song: Song) -> Option<Song> {
        self.songs.insert(song.id, song)
    }

    pub fn remove_song(&mut self, song: SongId) -> Option<Song> {
        if let Some(removed) = self.songs.remove(&song) {
            Some(removed)
        } else {
            None
        }
    }
    pub fn remove_album(&mut self, song: SongId) -> Option<Song> {
        if let Some(removed) = self.songs.remove(&song) {
            Some(removed)
        } else {
            None
        }
    }
    pub fn remove_artist(&mut self, song: SongId) -> Option<Song> {
        if let Some(removed) = self.songs.remove(&song) {
            Some(removed)
        } else {
            None
        }
    }

    pub fn init_connection<T: Write>(&self, con: &mut T) -> Result<(), std::io::Error> {
        // TODO! this is slow because it clones everything - there has to be a better way...
        Command::SyncDatabase(
            self.artists().iter().map(|v| v.1.clone()).collect(),
            self.albums().iter().map(|v| v.1.clone()).collect(),
            self.songs().iter().map(|v| v.1.clone()).collect(),
        )
        .to_bytes(con)?;
        Command::QueueUpdate(vec![], self.queue.clone()).to_bytes(con)?;
        if self.playing {
            Command::Resume.to_bytes(con)?;
        }
        // since this is so easy to check for, it comes last.
        // this allows clients to find out when init_connection is done.
        Command::SetLibraryDirectory(self.lib_directory.clone()).to_bytes(con)?;
        // is initialized now - client can receive updates after this point.
        // NOTE: Don't write to connection anymore - the db will dispatch updates on its own.
        // we just need to handle commands (receive from the connection).
        Ok(())
    }

    pub fn apply_command(&mut self, command: Command) {
        // since db.update_endpoints is empty for clients, this won't cause unwanted back and forth
        self.broadcast_update(&command);
        match command {
            Command::Resume => self.playing = true,
            Command::Pause => self.playing = false,
            Command::Stop => self.playing = false,
            Command::NextSong => {
                if !Queue::advance_index_db(self) {
                    // end of queue
                    self.apply_command(Command::Pause);
                    let mut actions = Vec::new();
                    self.queue.init(vec![], &mut actions);
                    Queue::handle_actions(self, actions);
                }
            }
            Command::Save => {
                if let Err(e) = self.save_database(None) {
                    eprintln!("Couldn't save: {e}");
                }
            }
            Command::SyncDatabase(a, b, c) => self.sync(a, b, c),
            Command::QueueUpdate(index, new_data) => {
                if let Some(v) = self.queue.get_item_at_index_mut(&index, 0) {
                    *v = new_data;
                }
            }
            Command::QueueAdd(mut index, new_data) => {
                if let Some(v) = self.queue.get_item_at_index_mut(&index, 0) {
                    if let Some(i) = v.add_to_end(new_data) {
                        index.push(i);
                        if let Some(q) = self.queue.get_item_at_index_mut(&index, 0) {
                            let mut actions = Vec::new();
                            q.init(index, &mut actions);
                            Queue::handle_actions(self, actions);
                        }
                    }
                }
            }
            Command::QueueInsert(mut index, pos, mut new_data) => {
                if let Some(v) = self.queue.get_item_at_index_mut(&index, 0) {
                    index.push(pos);
                    let mut actions = Vec::new();
                    new_data.init(index, &mut actions);
                    v.insert(new_data, pos);
                    Queue::handle_actions(self, actions);
                }
            }
            Command::QueueRemove(index) => {
                self.queue.remove_by_index(&index, 0);
            }
            Command::QueueGoto(index) => Queue::set_index_db(self, &index),
            Command::QueueSetShuffle(path, map, next) => {
                if let Some(elem) = self.queue.get_item_at_index_mut(&path, 0) {
                    if let QueueContent::Shuffle(_, m, _, n) = elem.content_mut() {
                        *m = map;
                        *n = next;
                    }
                }
            }
            Command::AddSong(song) => {
                self.add_song_new(song);
            }
            Command::AddAlbum(album) => {
                self.add_album_new(album);
            }
            Command::AddArtist(artist) => {
                self.add_artist_new(artist);
            }
            Command::AddCover(cover) => _ = self.add_cover_new(cover),
            Command::ModifySong(song) => {
                _ = self.update_song(song);
            }
            Command::ModifyAlbum(album) => {
                _ = self.update_album(album);
            }
            Command::ModifyArtist(artist) => {
                _ = self.update_artist(artist);
            }
            Command::RemoveSong(song) => {
                _ = self.remove_song(song);
            }
            Command::RemoveAlbum(album) => {
                _ = self.remove_album(album);
            }
            Command::RemoveArtist(artist) => {
                _ = self.remove_artist(artist);
            }
            Command::SetLibraryDirectory(new_dir) => {
                self.lib_directory = new_dir;
            }
        }
    }
}

// file saving/loading

impl Database {
    /// Database is also used for clients, to keep things consistent.
    /// A client database doesn't need any storage paths and won't perform autosaves.
    pub fn new_clientside() -> Self {
        Self {
            db_file: PathBuf::new(),
            lib_directory: PathBuf::new(),
            artists: HashMap::new(),
            albums: HashMap::new(),
            songs: HashMap::new(),
            covers: HashMap::new(),
            db_data_file_change_first: None,
            db_data_file_change_last: None,
            queue: QueueContent::Folder(0, vec![], String::new()).into(),
            update_endpoints: vec![],
            playing: false,
            command_sender: None,
            remote_server_as_song_file_source: None,
        }
    }
    pub fn new_empty(path: PathBuf, lib_dir: PathBuf) -> Self {
        Self {
            db_file: path,
            lib_directory: lib_dir,
            artists: HashMap::new(),
            albums: HashMap::new(),
            songs: HashMap::new(),
            covers: HashMap::new(),
            db_data_file_change_first: None,
            db_data_file_change_last: None,
            queue: QueueContent::Folder(0, vec![], String::new()).into(),
            update_endpoints: vec![],
            playing: false,
            command_sender: None,
            remote_server_as_song_file_source: None,
        }
    }
    pub fn load_database(path: PathBuf) -> Result<Self, std::io::Error> {
        let mut file = BufReader::new(File::open(&path)?);
        eprintln!("[info] loading library from {file:?}");
        let lib_directory = ToFromBytes::from_bytes(&mut file)?;
        eprintln!("[info] library directory is {lib_directory:?}");
        Ok(Self {
            db_file: path,
            lib_directory,
            artists: ToFromBytes::from_bytes(&mut file)?,
            albums: ToFromBytes::from_bytes(&mut file)?,
            songs: ToFromBytes::from_bytes(&mut file)?,
            covers: ToFromBytes::from_bytes(&mut file)?,
            db_data_file_change_first: None,
            db_data_file_change_last: None,
            queue: QueueContent::Folder(0, vec![], String::new()).into(),
            update_endpoints: vec![],
            playing: false,
            command_sender: None,
            remote_server_as_song_file_source: None,
        })
    }
    /// saves the database's contents. save path can be overridden
    pub fn save_database(&self, path: Option<PathBuf>) -> Result<PathBuf, std::io::Error> {
        let path = if let Some(p) = path {
            p
        } else {
            self.db_file.clone()
        };
        // if no path is set (client mode), do nothing
        if path.as_os_str().is_empty() {
            return Ok(path);
        }
        eprintln!("[info] saving db to {path:?}.");
        let mut file = fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&path)?;
        self.lib_directory.to_bytes(&mut file)?;
        self.artists.to_bytes(&mut file)?;
        self.albums.to_bytes(&mut file)?;
        self.songs.to_bytes(&mut file)?;
        self.covers.to_bytes(&mut file)?;
        Ok(path)
    }
    pub fn broadcast_update(&mut self, update: &Command) {
        let mut remove = vec![];
        let mut bytes = None;
        let mut arc = None;
        for (i, udep) in self.update_endpoints.iter_mut().enumerate() {
            match udep {
                UpdateEndpoint::Bytes(writer) => {
                    if bytes.is_none() {
                        bytes = Some(update.to_bytes_vec());
                    }
                    if writer.write_all(bytes.as_ref().unwrap()).is_err() {
                        remove.push(i);
                    }
                }
                UpdateEndpoint::CmdChannel(sender) => {
                    if arc.is_none() {
                        arc = Some(Arc::new(update.clone()));
                    }
                    if sender.send(arc.clone().unwrap()).is_err() {
                        remove.push(i);
                    }
                }
                UpdateEndpoint::CmdChannelTokio(sender) => {
                    if arc.is_none() {
                        arc = Some(Arc::new(update.clone()));
                    }
                    if sender.send(arc.clone().unwrap()).is_err() {
                        remove.push(i);
                    }
                }
                UpdateEndpoint::Custom(func) => func(update),
            }
        }
        if !remove.is_empty() {
            eprintln!(
                "[info] closing {} connections, {} are still active",
                remove.len(),
                self.update_endpoints.len() - remove.len()
            );
            for i in remove.into_iter().rev() {
                self.update_endpoints.remove(i);
            }
        }
    }
    pub fn sync(&mut self, artists: Vec<Artist>, albums: Vec<Album>, songs: Vec<Song>) {
        self.artists = artists.iter().map(|v| (v.id, v.clone())).collect();
        self.albums = albums.iter().map(|v| (v.id, v.clone())).collect();
        self.songs = songs.iter().map(|v| (v.id, v.clone())).collect();
    }
}

impl Database {
    pub fn songs(&self) -> &HashMap<SongId, Song> {
        &self.songs
    }
    pub fn albums(&self) -> &HashMap<AlbumId, Album> {
        &self.albums
    }
    pub fn artists(&self) -> &HashMap<ArtistId, Artist> {
        &self.artists
    }
    pub fn covers(&self) -> &HashMap<CoverId, Cover> {
        &self.covers
    }
    /// you should probably use a Command to do this...
    pub fn songs_mut(&mut self) -> &mut HashMap<SongId, Song> {
        &mut self.songs
    }
    /// you should probably use a Command to do this...
    pub fn albums_mut(&mut self) -> &mut HashMap<AlbumId, Album> {
        &mut self.albums
    }
    /// you should probably use a Command to do this...
    pub fn artists_mut(&mut self) -> &mut HashMap<ArtistId, Artist> {
        &mut self.artists
    }
    /// you should probably use a Command to do this...
    pub fn covers_mut(&mut self) -> &mut HashMap<CoverId, Cover> {
        &mut self.covers
    }
}

#[derive(Clone, Debug)]
pub struct Cover {
    pub location: DatabaseLocation,
    pub data: Arc<Mutex<(bool, Option<(Instant, Vec<u8>)>)>>,
}
impl Cover {
    pub fn get_bytes<O>(
        &self,
        path: impl FnOnce(&DatabaseLocation) -> PathBuf,
        conv: impl FnOnce(&Vec<u8>) -> O,
    ) -> Option<O> {
        let mut data = loop {
            let data = self.data.lock().unwrap();
            if data.0 {
                drop(data);
                std::thread::sleep(Duration::from_secs(1));
            } else {
                break data;
            }
        };
        if let Some((accessed, data)) = &mut data.1 {
            *accessed = Instant::now();
            Some(conv(&data))
        } else {
            match std::fs::read(path(&self.location)) {
                Ok(bytes) => {
                    data.1 = Some((Instant::now(), bytes));
                    Some(conv(&data.1.as_ref().unwrap().1))
                }
                Err(_) => None,
            }
        }
    }
}
impl ToFromBytes for Cover {
    fn to_bytes<T>(&self, s: &mut T) -> Result<(), std::io::Error>
    where
        T: Write,
    {
        self.location.to_bytes(s)
    }
    fn from_bytes<T>(s: &mut T) -> Result<Self, std::io::Error>
    where
        T: std::io::Read,
    {
        Ok(Self {
            location: ToFromBytes::from_bytes(s)?,
            data: Arc::new(Mutex::new((false, None))),
        })
    }
}
