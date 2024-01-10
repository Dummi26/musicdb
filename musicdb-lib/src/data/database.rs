use std::{
    collections::HashMap,
    fs::{self, File},
    io::{BufReader, Read, Write},
    path::PathBuf,
    sync::{mpsc, Arc, Mutex},
    time::{Duration, Instant},
};

use colorize::AnsiColor;

use crate::{load::ToFromBytes, server::Command};

use super::{
    album::Album,
    artist::Artist,
    queue::{Queue, QueueContent, ShuffleState},
    song::Song,
    AlbumId, ArtistId, CoverId, DatabaseLocation, SongId,
};

pub struct Database {
    /// the directory that contains the dbfile, backups, statistics, ...
    pub db_dir: PathBuf,
    /// the path to the file used to save/load the data. empty if database is in client mode.
    pub db_file: PathBuf,
    /// the path to the directory containing the actual music and cover image files
    pub lib_directory: PathBuf,
    artists: HashMap<ArtistId, Artist>,
    albums: HashMap<AlbumId, Album>,
    songs: HashMap<SongId, Song>,
    covers: HashMap<CoverId, Cover>,
    /// clients can access files in this directory if they know the relative path.
    /// can be used to embed custom images in tags of songs/albums/artists.
    /// None -> no access
    /// Some(None) -> access to lib_directory
    /// Some(Some(path)) -> access to path
    pub custom_files: Option<Option<PathBuf>>,
    pub queue: Queue,
    /// if the database receives an update, it will inform all of its clients so they can stay in sync.
    /// this is a list containing all the clients.
    pub update_endpoints: Vec<UpdateEndpoint>,
    /// true if a song is/should be playing
    pub playing: bool,
    pub command_sender: Option<mpsc::Sender<Command>>,
    pub remote_server_as_song_file_source:
        Option<Arc<Mutex<crate::server::get::Client<Box<dyn ClientIo>>>>>,
    /// only relevant for clients. true if init is done
    client_is_init: bool,

    /// If `Some`, contains the first time and the last time data was modified.
    /// When the DB is saved, this is reset to `None` to represent that nothing was modified.
    pub times_data_modified: Option<(Instant, Instant)>,
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
    pub fn is_client(&self) -> bool {
        self.db_file.as_os_str().is_empty()
    }
    pub fn is_client_init(&self) -> bool {
        self.client_is_init
    }
    pub fn get_path(&self, location: &DatabaseLocation) -> PathBuf {
        self.lib_directory.join(&location.rel_path)
    }
    fn modified_data(&mut self) {
        let now = Instant::now();
        if let Some((_first, last)) = &mut self.times_data_modified {
            *last = now;
        } else {
            self.times_data_modified = Some((now, now));
        }
    }
    // NOTE: just use `songs` directly? not sure yet...
    pub fn get_song(&self, song: &SongId) -> Option<&Song> {
        self.songs.get(song)
    }
    pub fn get_song_mut(&mut self, song: &SongId) -> Option<&mut Song> {
        self.modified_data();
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
            if let Some(artist) = self.artists.get_mut(&artist) {
                artist.singles.push(id);
            }
        }
        id
    }
    /// used internally
    pub fn add_song_new_nomagic(&mut self, mut song: Song) -> SongId {
        self.modified_data();
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
        self.modified_data();
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
        if let Some(artist) = self.artists.get_mut(&artist) {
            artist.albums.push(id);
        }
        id
    }
    /// used internally
    fn add_album_new_nomagic(&mut self, mut album: Album) -> AlbumId {
        self.modified_data();
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
        self.modified_data();
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
            let old = std::mem::replace(prev_song, song);
            self.modified_data();
            Ok(old)
        } else {
            Err(())
        }
    }
    pub fn update_album(&mut self, album: Album) -> Result<Album, ()> {
        if let Some(prev_album) = self.albums.get_mut(&album.id) {
            let old = std::mem::replace(prev_album, album);
            self.modified_data();
            Ok(old)
        } else {
            Err(())
        }
    }
    pub fn update_artist(&mut self, artist: Artist) -> Result<Artist, ()> {
        if let Some(prev_artist) = self.artists.get_mut(&artist.id) {
            let old = std::mem::replace(prev_artist, artist);
            self.modified_data();
            Ok(old)
        } else {
            Err(())
        }
    }
    /// [NOT RECOMMENDED - use add_song_new or update_song instead!] inserts the song into the database.
    /// uses song.id. If another song with that ID exists, it is replaced and Some(other_song) is returned.
    /// If no other song exists, the song will be added to the database with the given ID and None is returned.
    pub fn update_or_add_song(&mut self, song: Song) -> Option<Song> {
        self.modified_data();
        self.songs.insert(song.id, song)
    }

    pub fn remove_song(&mut self, song: SongId) -> Option<Song> {
        if let Some(removed) = self.songs.remove(&song) {
            self.modified_data();
            Some(removed)
        } else {
            None
        }
    }
    pub fn remove_album(&mut self, song: SongId) -> Option<Song> {
        if let Some(removed) = self.songs.remove(&song) {
            self.modified_data();
            Some(removed)
        } else {
            None
        }
    }
    pub fn remove_artist(&mut self, song: SongId) -> Option<Song> {
        if let Some(removed) = self.songs.remove(&song) {
            self.modified_data();
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
        // this allows clients to find out when init_connection is done.
        Command::InitComplete.to_bytes(con)?;
        // is initialized now - client can receive updates after this point.
        // NOTE: Don't write to connection anymore - the db will dispatch updates on its own.
        // we just need to handle commands (receive from the connection).
        Ok(())
    }

    pub fn apply_command(&mut self, mut command: Command) {
        if !self.is_client() {
            if let Command::ErrorInfo(t, _) = &mut command {
                // clients can send ErrorInfo to the server and it will show up on other clients,
                // BUT only the server can set the Title of the ErrorInfo.
                t.clear();
            }
        }
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
                    eprintln!("[{}] Couldn't save: {e}", "ERR!".red());
                }
            }
            Command::SyncDatabase(a, b, c) => self.sync(a, b, c),
            Command::QueueUpdate(index, new_data) => {
                let mut actions = vec![];
                if let Some(v) = self.queue.get_item_at_index_mut(&index, 0, &mut actions) {
                    *v = new_data;
                }
                Queue::handle_actions(self, actions);
            }
            Command::QueueAdd(index, new_data) => {
                let mut actions = vec![];
                if let Some(v) = self.queue.get_item_at_index_mut(&index, 0, &mut actions) {
                    v.add_to_end(new_data, index, &mut actions);
                }
                Queue::handle_actions(self, actions);
            }
            Command::QueueInsert(index, pos, new_data) => {
                let mut actions = vec![];
                if let Some(v) = self.queue.get_item_at_index_mut(&index, 0, &mut actions) {
                    v.insert(new_data, pos, index, &mut actions);
                }
                Queue::handle_actions(self, actions);
            }
            Command::QueueRemove(index) => {
                self.queue.remove_by_index(&index, 0);
            }
            Command::QueueGoto(index) => Queue::set_index_db(self, &index),
            Command::QueueSetShuffle(path, order) => {
                let mut actions = vec![];
                if let Some(elem) = self.queue.get_item_at_index_mut(&path, 0, &mut actions) {
                    if let QueueContent::Shuffle { inner, state } = elem.content_mut() {
                        if let QueueContent::Folder(_, v, _) = inner.content_mut() {
                            let mut o = std::mem::replace(v, vec![])
                                .into_iter()
                                .map(|v| Some(v))
                                .collect::<Vec<_>>();
                            for &i in order.iter() {
                                if let Some(a) = o.get_mut(i).and_then(Option::take) {
                                    v.push(a);
                                } else {
                                    eprintln!("[{}] Can't properly apply requested order to Queue/Shuffle: no element at index {i}. Index may be out of bounds or used twice. Len: {}, Order: {order:?}.", "WARN".yellow(), v.len());
                                }
                            }
                        }
                        *state = ShuffleState::Shuffled;
                    } else {
                        eprintln!(
                            "[warn] can't QueueSetShuffle - element at path {path:?} isn't Shuffle"
                        );
                    }
                } else {
                    eprintln!(
                        "[{}] can't QueueSetShuffle - no element at path {path:?}",
                        "WARN".yellow()
                    );
                }
                Queue::handle_actions(self, actions);
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
            Command::TagSongFlagSet(id, tag) => {
                if let Some(v) = self.get_song_mut(&id) {
                    if !v.general.tags.contains(&tag) {
                        v.general.tags.push(tag);
                    }
                }
            }
            Command::TagSongFlagUnset(id, tag) => {
                if let Some(v) = self.get_song_mut(&id) {
                    if let Some(i) = v.general.tags.iter().position(|v| v == &tag) {
                        v.general.tags.remove(i);
                    }
                }
            }
            Command::TagAlbumFlagSet(id, tag) => {
                if let Some(v) = self.albums.get_mut(&id) {
                    if !v.general.tags.contains(&tag) {
                        v.general.tags.push(tag);
                    }
                }
            }
            Command::TagAlbumFlagUnset(id, tag) => {
                if let Some(v) = self.albums.get_mut(&id) {
                    if let Some(i) = v.general.tags.iter().position(|v| v == &tag) {
                        v.general.tags.remove(i);
                    }
                }
            }
            Command::TagArtistFlagSet(id, tag) => {
                if let Some(v) = self.artists.get_mut(&id) {
                    if !v.general.tags.contains(&tag) {
                        v.general.tags.push(tag);
                    }
                }
            }
            Command::TagArtistFlagUnset(id, tag) => {
                if let Some(v) = self.artists.get_mut(&id) {
                    if let Some(i) = v.general.tags.iter().position(|v| v == &tag) {
                        v.general.tags.remove(i);
                    }
                }
            }
            Command::TagSongPropertySet(id, key, val) => {
                if let Some(v) = self.get_song_mut(&id) {
                    let new = format!("{key}{val}");
                    if let Some(v) = v.general.tags.iter_mut().find(|v| v.starts_with(&key)) {
                        *v = new;
                    } else {
                        v.general.tags.push(new);
                    }
                }
            }
            Command::TagSongPropertyUnset(id, key) => {
                if let Some(v) = self.get_song_mut(&id) {
                    let tags = std::mem::replace(&mut v.general.tags, vec![]);
                    v.general.tags = tags.into_iter().filter(|v| !v.starts_with(&key)).collect();
                }
            }
            Command::TagAlbumPropertySet(id, key, val) => {
                if let Some(v) = self.albums.get_mut(&id) {
                    let new = format!("{key}{val}");
                    if let Some(v) = v.general.tags.iter_mut().find(|v| v.starts_with(&key)) {
                        *v = new;
                    } else {
                        v.general.tags.push(new);
                    }
                }
            }
            Command::TagAlbumPropertyUnset(id, key) => {
                if let Some(v) = self.albums.get_mut(&id) {
                    let tags = std::mem::replace(&mut v.general.tags, vec![]);
                    v.general.tags = tags.into_iter().filter(|v| !v.starts_with(&key)).collect();
                }
            }
            Command::TagArtistPropertySet(id, key, val) => {
                if let Some(v) = self.artists.get_mut(&id) {
                    let new = format!("{key}{val}");
                    if let Some(v) = v.general.tags.iter_mut().find(|v| v.starts_with(&key)) {
                        *v = new;
                    } else {
                        v.general.tags.push(new);
                    }
                }
            }
            Command::TagArtistPropertyUnset(id, key) => {
                if let Some(v) = self.artists.get_mut(&id) {
                    let tags = std::mem::replace(&mut v.general.tags, vec![]);
                    v.general.tags = tags.into_iter().filter(|v| !v.starts_with(&key)).collect();
                }
            }
            Command::SetSongDuration(id, duration) => {
                if let Some(song) = self.get_song_mut(&id) {
                    song.duration_millis = duration;
                }
            }
            Command::InitComplete => {
                self.client_is_init = true;
            }
            Command::ErrorInfo(..) => {}
        }
    }
}

// file saving/loading

impl Database {
    /// TODO!
    fn panic(&self, msg: &str) -> ! {
        // custom panic handler
        // make a backup
        // exit
        panic!("DatabasePanic: {msg}");
    }
    /// Database is also used for clients, to keep things consistent.
    /// A client database doesn't need any storage paths and won't perform autosaves.
    pub fn new_clientside() -> Self {
        Self {
            db_dir: PathBuf::new(),
            db_file: PathBuf::new(),
            lib_directory: PathBuf::new(),
            artists: HashMap::new(),
            albums: HashMap::new(),
            songs: HashMap::new(),
            covers: HashMap::new(),
            custom_files: None,
            queue: QueueContent::Folder(0, vec![], String::new()).into(),
            update_endpoints: vec![],
            playing: false,
            command_sender: None,
            remote_server_as_song_file_source: None,
            client_is_init: false,
            times_data_modified: None,
        }
    }
    pub fn new_empty_in_dir(dir: PathBuf, lib_dir: PathBuf) -> Self {
        let path = dir.join("dbfile");
        Self {
            db_dir: dir,
            db_file: path,
            lib_directory: lib_dir,
            artists: HashMap::new(),
            albums: HashMap::new(),
            songs: HashMap::new(),
            covers: HashMap::new(),
            custom_files: None,
            queue: QueueContent::Folder(0, vec![], String::new()).into(),
            update_endpoints: vec![],
            playing: false,
            command_sender: None,
            remote_server_as_song_file_source: None,
            client_is_init: false,
            times_data_modified: None,
        }
    }
    pub fn load_database_from_dir(
        dir: PathBuf,
        lib_directory: PathBuf,
    ) -> Result<Self, std::io::Error> {
        let path = dir.join("dbfile");
        let mut file = BufReader::new(File::open(&path)?);
        eprintln!("[{}] loading library from {file:?}", "INFO".cyan());
        let s = Self {
            db_dir: dir,
            db_file: path,
            lib_directory,
            artists: ToFromBytes::from_bytes(&mut file)?,
            albums: ToFromBytes::from_bytes(&mut file)?,
            songs: ToFromBytes::from_bytes(&mut file)?,
            covers: ToFromBytes::from_bytes(&mut file)?,
            custom_files: None,
            queue: QueueContent::Folder(0, vec![], String::new()).into(),
            update_endpoints: vec![],
            playing: false,
            command_sender: None,
            remote_server_as_song_file_source: None,
            client_is_init: false,
            times_data_modified: None,
        };
        eprintln!("[{}] loaded library", "INFO".green());
        Ok(s)
    }
    /// saves the database's contents. save path can be overridden
    pub fn save_database(&mut self, path: Option<PathBuf>) -> Result<PathBuf, std::io::Error> {
        let path = if let Some(p) = path {
            p
        } else {
            self.db_file.clone()
        };
        // if no path is set (client mode), do nothing
        if path.as_os_str().is_empty() {
            return Ok(path);
        }
        eprintln!("[{}] saving db to {path:?}", "INFO".cyan());
        if path.try_exists()? {
            let backup_name = format!(
                "dbfile-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
            );
            if let Err(e) = fs::rename(&path, self.db_dir.join(&backup_name)) {
                eprintln!(
                    "[{}] Couldn't move previous dbfile to {backup_name}!",
                    "ERR!".red()
                );
                return Err(e);
            }
        }
        let mut file = fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&path)?;
        self.artists.to_bytes(&mut file)?;
        self.albums.to_bytes(&mut file)?;
        self.songs.to_bytes(&mut file)?;
        self.covers.to_bytes(&mut file)?;
        eprintln!("[{}] saved db", "INFO".green());
        // all changes saved, data no longer modified
        self.times_data_modified = None;
        Ok(path)
    }
    pub fn broadcast_update(&mut self, update: &Command) {
        match update {
            Command::InitComplete => return,
            _ => {}
        }
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
        self.modified_data();
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
        self.modified_data();
        &mut self.songs
    }
    /// you should probably use a Command to do this...
    pub fn albums_mut(&mut self) -> &mut HashMap<AlbumId, Album> {
        self.modified_data();
        &mut self.albums
    }
    /// you should probably use a Command to do this...
    pub fn artists_mut(&mut self) -> &mut HashMap<ArtistId, Artist> {
        self.modified_data();
        &mut self.artists
    }
    /// you should probably use a Command to do this...
    pub fn covers_mut(&mut self) -> &mut HashMap<CoverId, Cover> {
        self.modified_data();
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
