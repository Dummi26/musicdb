use rand::prelude::SliceRandom;
use std::{
    collections::{BTreeSet, HashMap},
    fs::{self, File},
    io::{BufReader, Read, Write},
    path::{Path, PathBuf},
    sync::{mpsc, Arc, Mutex},
    time::{Duration, Instant},
};

use colorize::AnsiColor;
use rand::thread_rng;

use crate::{
    load::ToFromBytes,
    server::{Action, Command, Commander, Req},
};

use super::{
    album::Album,
    artist::Artist,
    queue::{Queue, QueueContent, QueueFolder},
    song::Song,
    AlbumId, ArtistId, CoverId, DatabaseLocation, SongId,
};

pub struct Database {
    pub seq: Commander,
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
    pub update_endpoints: Vec<(u64, UpdateEndpoint)>,
    pub update_endpoints_id: u64,
    /// true if a song is/should be playing
    pub playing: bool,
    pub command_sender: Option<mpsc::Sender<(Command, Option<u64>)>>,
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
    Custom(Box<dyn FnMut(&Command) + Send>),
    CustomArc(Box<dyn FnMut(Arc<Command>) + Send>),
    CustomBytes(Box<dyn FnMut(&[u8]) + Send>),
}

impl Database {
    pub fn is_client(&self) -> bool {
        self.db_file.as_os_str().is_empty()
    }
    pub fn is_client_init(&self) -> bool {
        self.client_is_init
    }
    pub fn get_path(&self, location: &DatabaseLocation) -> PathBuf {
        Self::get_path_nodb(&self.lib_directory, location)
    }
    pub fn get_path_nodb(lib_directory: &impl AsRef<Path>, location: &DatabaseLocation) -> PathBuf {
        lib_directory.as_ref().join(&location.rel_path)
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
    pub fn update_song(&mut self, mut song: Song) -> Result<Song, ()> {
        if let Some(prev_song) = self.songs.remove(&song.id) {
            self.modified_data();
            if song.album != prev_song.album || song.artist != prev_song.artist {
                // remove previous song from album/artist
                if let Some(a) = prev_song.album {
                    if let Some(a) = self.albums.get_mut(&a) {
                        if let Some(i) = a.songs.iter().position(|s| *s == song.id) {
                            a.songs.remove(i);
                        } else {
                            eprintln!(
                                "[{}] Couldn't remove Song {} from previous album, because the album with the ID {} didn't contain that song.",
                                "WARN".yellow(),
                                song.id,
                                a.id
                            );
                        }
                    } else {
                        eprintln!(
                            "[{}] Couldn't remove Song {} from previous album, because no album with the ID {} was found.",
                            "ERR!".red(),
                            song.id,
                                        a
                        );
                    }
                } else {
                    if let Some(a) = self.artists.get_mut(&prev_song.artist) {
                        if let Some(i) = a.singles.iter().position(|s| *s == song.id) {
                            a.singles.remove(i);
                        } else {
                            eprintln!("[{}] Couldn't remove Song {} from Artist {} singles, because that song wasn't found in that artist.", "WARN".yellow(), song.id, prev_song.artist);
                        }
                    } else {
                        eprintln!("[{}] Couldn't remove Song {} from Artist {} singles, because that artist wasn't found.", "ERR!".red(), song.id, prev_song.artist);
                    }
                }
                // add new song to album/artist
                if let Some(a) = song.album {
                    if let Some(a) = self.albums.get_mut(&a) {
                        if song.artist != a.artist {
                            eprintln!("[{}] Changing song's artist because it doesn't match the specified album's artist.", "WARN".yellow());
                            song.artist = a.artist;
                        }
                        if !a.songs.contains(&song.id) {
                            a.songs.push(song.id);
                        }
                    } else {
                        eprintln!(
                            "[{}] Couldn't add Song {} to new album, because no album with the ID {} was found.",
                            "ERR!".red(),
                            song.id, a
                        );
                    }
                } else {
                    if let Some(a) = self.artists.get_mut(&song.artist) {
                        if !a.singles.contains(&song.id) {
                            a.singles.push(song.id);
                        }
                    } else {
                        eprintln!("[{}] Couldn't add Song {} to Artist {} singles, because that artist wasn't found.", "ERR!".red(), song.id, song.artist);
                    }
                }
            }

            self.songs.insert(song.id, song);
            Ok(prev_song)
        } else {
            eprintln!(
                "[{}] Couldn't update Song {}, because no song with that ID exists.",
                "WARN".yellow(),
                song.id,
            );
            Err(())
        }
    }
    pub fn update_album(&mut self, album: Album) -> Result<Album, ()> {
        if let Some(prev_album) = self.albums.remove(&album.id) {
            // some checks
            let new_songs = album.songs.iter().copied().collect::<BTreeSet<_>>();
            let prev_songs = album.songs.iter().copied().collect::<BTreeSet<_>>();
            // check if we would end up with songs that aren't referenced anywhere, and, if yes, don't do anything.
            if prev_songs.difference(&new_songs).next().is_some() {
                eprintln!("[{}] Can't update Album {} because some songs that used to be in this album are not included in the new data.", "ERR!".red(), album.id);
                return Err(());
            }

            // change artist
            if prev_album.artist != album.artist {
                // remove album from previous artist
                if let Some(prev_artist) = self.artists.get_mut(&prev_album.artist) {
                    if let Some(i) = prev_artist.albums.iter().position(|a| *a != prev_album.id) {
                        prev_artist.albums.remove(i);
                    } else {
                        eprintln!(
                            "[{}] Couldn't remove Album {} from Artist {}, because it was not listed as an album in that artist.",
                            "ERR!".red(),
                            prev_album.id,
                            prev_album.artist
                        );
                    }
                } else {
                    eprintln!(
                            "[{}] Couldn't remove Album {} from Artist {}, because no artist with that ID exists.",
                            "ERR!".red(),
                            prev_album.id,
                            prev_album.artist
                        );
                }
                // add album to new artist
                if let Some(artist) = self.artists.get_mut(&album.artist) {
                    if !artist.albums.contains(&album.id) {
                        artist.albums.push(album.id);
                    } else {
                        eprintln!(
                            "[{}] Couldn't add Album {} to Artist {}, because the album was already added (this should never happen...).",
                            "WARN".yellow(),
                            album.id,
                            album.artist
                        );
                    }
                } else {
                    eprintln!(
                            "[{}] Couldn't add Album {} to Artist {}, because no artist with that ID exists.",
                            "ERR!".red(),
                            album.id,
                            album.artist
                        );
                }
                // change artist of songs in album (if album artist is changed AND album has gotten more songs, this will be done twice for some songs, but that is okay)
                for song in &album.songs {
                    if let Some(song) = self.songs.get_mut(song) {
                        song.artist = album.artist;
                    } else {
                        eprintln!(
                            "[{}] Couldn't change Song {} artist to Artist {}, because no song with that ID exists (changing because album artist was changed).",
                            "ERR!".red(),
                            song,
                            album.artist
                        );
                    }
                }
            }

            // change artist & album of songs that were previously not in this album
            for song in new_songs.difference(&prev_songs) {
                if let Some(song) = self.songs.get_mut(song) {
                    // change song's artist to that of this album
                    song.artist = album.artist;
                    // if song was previously in another album, remove it from that album
                    // it will be added to this new album because its id is already in `album`, so we don't need to do anything to achieve that.
                    if let Some(prev_album) = song.album {
                        if prev_album != album.id {
                            // remove song from its previous album
                            if let Some(prev_album) = self.albums.get_mut(&prev_album) {
                                if let Some(i) = prev_album.songs.iter().position(|s| *s == song.id)
                                {
                                    prev_album.songs.remove(i);
                                } else {
                                    eprintln!(
                                        "[{}] Couldn't remove Song {} from its previous album, Album {}, because no song with that ID exists in that album.",
                                        "WARN".yellow(),
                                        song.id,
                                        prev_album.id
                                    );
                                }
                            } else {
                                eprintln!(
                                        "[{}] Couldn't remove Song {} from its previous album, Album {}, because no album with that ID exists.",
                                        "WARN".yellow(),
                                        song.id,
                                        prev_album
                                    );
                            }
                        }
                    }
                } else {
                    eprintln!(
                        "[{}] Couldn't remove Song {} from its previous album because no song with that ID exists.",
                        "ERR!".red(),
                        *song,
                    );
                }
            }

            self.albums.insert(album.id, album);
            self.modified_data();
            Ok(prev_album)
        } else {
            eprintln!(
                "[{}] Couldn't update Album {}, because no album with that ID exists.",
                "WARN".yellow(),
                album.id,
            );
            Err(())
        }
    }
    pub fn update_artist(&mut self, artist: Artist) -> Result<Artist, ()> {
        if let Some(prev_artist) = self.artists.remove(&artist.id) {
            self.modified_data();

            let prev_albums = prev_artist.albums.iter().copied().collect::<BTreeSet<_>>();
            let new_albums = artist.albums.iter().copied().collect::<BTreeSet<_>>();
            if prev_albums.difference(&new_albums).next().is_some() {
                eprintln!("[{}] Can't update Artist {} because some albums that used to be in this artist are not included in the new data.", "ERR!".red(), artist.id);
                return Err(());
            }

            let prev_singles = prev_artist.singles.iter().copied().collect::<BTreeSet<_>>();
            let new_singles = artist.singles.iter().copied().collect::<BTreeSet<_>>();
            if prev_singles.difference(&new_singles).next().is_some() {
                eprintln!("[{}] Can't update Artist {} because some singles that used to be in this artist are not included in the new data.", "ERR!".red(), artist.id);
                return Err(());
            }

            // change artist of newly added albums and their songs
            for album in new_albums.difference(&prev_albums) {
                if let Some(album) = self.albums.get_mut(album) {
                    if let Some(a) = self.artists.get_mut(&album.artist) {
                        if let Some(i) = a.albums.iter().position(|a| *a == album.id) {
                            a.albums.remove(i);
                        } else {
                            eprintln!("[{}] Couldn't remove Album {} from Artist {} because that artist doesn't contain that album.", "ERR!".red(), album.id, album.artist);
                        }
                    } else {
                        eprintln!("[{}] Couldn't remove Album {} from Artist {} because that artist doesn't exist.", "ERR!".red(), album.id, album.artist);
                    }
                    album.artist = artist.id;
                    for song in &album.songs {
                        if let Some(song) = self.songs.get_mut(song) {
                            song.artist = artist.id;
                        } else {
                            eprintln!("[{}] Couldn't change Song {} artist to Artist {} because no song with that ID exists (should change because song is newly added to Album {}).", "ERR!".red(), song, artist.id, album.id);
                        }
                    }
                } else {
                    eprintln!("[{}] Couldn't move Album {} to Artist {} because no album with that ID exists.", "ERR!".red(), album, artist.id);
                }
            }

            // change artist of new singles
            for song in new_singles.difference(&prev_singles) {
                if let Some(song) = self.songs.get_mut(song) {
                    // remove song from previous album or artist
                    if let Some(a) = &song.album {
                        if let Some(a) = self.albums.get_mut(a) {
                            if let Some(i) = a.songs.iter().position(|s| *s == song.id) {
                                a.songs.remove(i);
                            } else {
                                eprintln!("[{}] Couldn't remove Song {} from Album {} because the album doesn't contain that song.", "ERR!".red(), song.id, a.id);
                            }
                        } else {
                            eprintln!("[{}] Couldn't remove Song {} from Album {} because no album with that ID exists.", "ERR!".red(), song.id, a);
                        }
                    } else {
                        if let Some(a) = self.artists.get_mut(&song.artist) {
                            if let Some(i) = a.singles.iter().position(|s| *s == song.id) {
                                a.singles.remove(i);
                            } else {
                                eprintln!("[{}] Couldn't remove Song {} from Artist {} because the artist doesn't contain that song.", "ERR!".red(), song.id, a.id);
                            }
                        } else {
                            eprintln!("[{}] Couldn't remove Song {} from Artist {} because no artist with that ID exists.", "ERR!".red(), song.id, song.artist);
                        }
                    }
                    song.artist = artist.id;
                } else {
                    eprintln!("[{}] Couldn't move Song {} to Artist {} singles because no song with that ID exists.", "ERR!".red(), song, artist.id);
                }
            }

            self.artists.insert(artist.id, artist);
            Ok(prev_artist)
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
        self.seq
            .pack(Action::SyncDatabase(
                self.artists().iter().map(|v| v.1.clone()).collect(),
                self.albums().iter().map(|v| v.1.clone()).collect(),
                self.songs().iter().map(|v| v.1.clone()).collect(),
            ))
            .to_bytes(con)?;
        self.seq
            .pack(Action::QueueUpdate(vec![], self.queue.clone(), Req::none()))
            .to_bytes(con)?;
        if self.playing {
            self.seq.pack(Action::Resume).to_bytes(con)?;
        }
        // this allows clients to find out when init_connection is done.
        self.seq.pack(Action::InitComplete).to_bytes(con)?;
        // is initialized now - client can receive updates after this point.
        // NOTE: Don't write to connection anymore - the db will dispatch updates on its own.
        // we just need to handle commands (receive from the connection).
        Ok(())
    }

    /// `apply_action_unchecked_seq(command.action)` if `command.seq` is correct or `0xFF`
    pub fn apply_command(&mut self, mut command: Command, client: Option<u64>) {
        if command.seq != self.seq.seq() && command.seq != 0xFF {
            if let Some(client) = client {
                for (udepid, udep) in &mut self.update_endpoints {
                    if client == *udepid {
                        let mut reqs = command.action.get_req_if_some();
                        if reqs.is_empty() {
                            reqs.push(Req::none());
                        }
                        for req in reqs {
                            let denied = Action::Denied(req).cmd(0xFFu8);
                            match udep {
                                UpdateEndpoint::Bytes(w) => {
                                    let _ = w.write(&denied.to_bytes_vec());
                                }
                                UpdateEndpoint::CmdChannel(w) => {
                                    let _ = w.send(Arc::new(denied));
                                }
                                UpdateEndpoint::Custom(w) => w(&denied),
                                UpdateEndpoint::CustomArc(w) => w(Arc::new(denied)),
                                UpdateEndpoint::CustomBytes(w) => w(&denied.to_bytes_vec()),
                            }
                        }
                        return;
                    }
                }
            }
            eprintln!(
                "Invalid sequence number: got {} but expected {}.",
                command.seq,
                self.seq.seq()
            );
            return;
        }
        self.apply_action_unchecked_seq(command.action, client)
    }
    pub fn apply_action_unchecked_seq(&mut self, mut action: Action, client: Option<u64>) {
        if !self.is_client() {
            if let Action::ErrorInfo(t, _) = &mut action {
                // clients can send ErrorInfo to the server and it will show up on other clients,
                // BUT only the server can set the Title of the ErrorInfo.
                t.clear();
            }
        }
        // some commands shouldn't be broadcast. these will broadcast a different command in their specific implementation.
        match &action {
            // Will broadcast `QueueSetShuffle`
            Action::QueueShuffle(_) => (),
            Action::NextSong if self.queue.is_almost_empty() => (),
            Action::Pause if !self.playing => (),
            Action::Resume if self.playing => (),
            // will be broadcast individually
            Action::Multiple(_) => (),
            // since db.update_endpoints is empty for clients, this won't cause unwanted back and forth
            _ => action = self.broadcast_update(action, client),
        }
        match action {
            Action::Resume => self.playing = true,
            Action::Pause => self.playing = false,
            Action::Stop => self.playing = false,
            Action::NextSong => {
                if !Queue::advance_index_db(self) {
                    // end of queue
                    self.apply_action_unchecked_seq(Action::Pause, client);
                    self.queue.init();
                }
            }
            Action::Save => {
                if let Err(e) = self.save_database(None) {
                    eprintln!("[{}] Couldn't save: {e}", "ERR!".red());
                }
            }
            Action::SyncDatabase(a, b, c) => self.sync(a, b, c),
            Action::QueueUpdate(index, new_data, _) => {
                if let Some(v) = self.queue.get_item_at_index_mut(&index, 0) {
                    *v = new_data;
                }
            }
            Action::QueueAdd(index, new_data, _) => {
                if let Some(v) = self.queue.get_item_at_index_mut(&index, 0) {
                    v.add_to_end(new_data, false);
                }
            }
            Action::QueueInsert(index, pos, new_data, _) => {
                if let Some(v) = self.queue.get_item_at_index_mut(&index, 0) {
                    v.insert(new_data, pos, false);
                }
            }
            Action::QueueRemove(index) => {
                self.queue.remove_by_index(&index, 0);
            }
            Action::QueueMove(index_from, mut index_to) => 'queue_move: {
                if index_to.len() == 0 || index_to.starts_with(&index_from) {
                    break 'queue_move;
                }
                // if same parent path, perform folder move operation instead
                if index_from[0..index_from.len() - 1] == index_to[0..index_to.len() - 1] {
                    if let Some(parent) = self
                        .queue
                        .get_item_at_index_mut(&index_from[0..index_from.len() - 1], 0)
                    {
                        if let QueueContent::Folder(folder) = parent.content_mut() {
                            let i1 = index_from[index_from.len() - 1];
                            let mut i2 = index_to[index_to.len() - 1];
                            if i2 > i1 {
                                i2 -= 1;
                            }
                            // this preserves "is currently active queue element" status
                            folder.move_elem(i1, i2);
                            break 'queue_move;
                        }
                    }
                }
                // otherwise, remove then insert
                let was_current = self.queue.is_current(&index_from);
                if let Some(elem) = self.queue.remove_by_index(&index_from, 0) {
                    if index_to.len() >= index_from.len()
                        && index_to.starts_with(&index_from[0..index_from.len() - 1])
                        && index_to[index_from.len() - 1] > index_from[index_from.len() - 1]
                    {
                        index_to[index_from.len() - 1] -= 1;
                    }
                    if let Some(parent) = self
                        .queue
                        .get_item_at_index_mut(&index_to[0..index_to.len() - 1], 0)
                    {
                        parent.insert(vec![elem], index_to[index_to.len() - 1], true);
                        if was_current {
                            self.queue.set_index_inner(&index_to, 0, vec![], true);
                        }
                    }
                }
            }
            Action::QueueMoveInto(index_from, mut parent_to) => 'queue_move_into: {
                if parent_to.starts_with(&index_from) {
                    break 'queue_move_into;
                }
                // remove then insert
                let was_current = self.queue.is_current(&index_from);
                if let Some(elem) = self.queue.remove_by_index(&index_from, 0) {
                    if parent_to.len() >= index_from.len()
                        && parent_to.starts_with(&index_from[0..index_from.len() - 1])
                        && parent_to[index_from.len() - 1] > index_from[index_from.len() - 1]
                    {
                        parent_to[index_from.len() - 1] -= 1;
                    }
                    if let Some(parent) = self.queue.get_item_at_index_mut(&parent_to, 0) {
                        if let Some(i) = parent.add_to_end(vec![elem], true) {
                            if was_current {
                                parent_to.push(i);
                                self.queue.set_index_inner(&parent_to, 0, vec![], true);
                            }
                        }
                    }
                }
            }
            Action::QueueGoto(index) => Queue::set_index_db(self, &index),
            Action::QueueShuffle(path) => {
                if let Some(elem) = self.queue.get_item_at_index_mut(&path, 0) {
                    if let QueueContent::Folder(QueueFolder {
                        index: _,
                        content,
                        name: _,
                        order: _,
                    }) = elem.content_mut()
                    {
                        let mut ord: Vec<usize> = (0..content.len()).collect();
                        ord.shuffle(&mut thread_rng());
                        self.apply_action_unchecked_seq(Action::QueueSetShuffle(path, ord), client);
                    } else {
                        eprintln!("(QueueShuffle) QueueElement at {path:?} not a folder!");
                    }
                } else {
                    eprintln!("(QueueShuffle) No QueueElement at {path:?}");
                }
            }
            Action::QueueSetShuffle(path, ord) => {
                if let Some(elem) = self.queue.get_item_at_index_mut(&path, 0) {
                    if let QueueContent::Folder(QueueFolder {
                        index,
                        content,
                        name: _,
                        order,
                    }) = elem.content_mut()
                    {
                        if ord.len() == content.len() {
                            if let Some(ni) = ord.iter().position(|v| *v == *index) {
                                *index = ni;
                            }
                            *order = Some(ord);
                        } else {
                            eprintln!(
                                "[warn] can't QueueSetShuffle - length of new ord ({}) is not the same as length of content ({})!",
                                ord.len(),
                                content.len()
                            );
                        }
                    } else {
                        eprintln!(
                            "[warn] can't QueueSetShuffle - element at path {path:?} isn't a folder"
                        );
                    }
                } else {
                    eprintln!(
                        "[{}] can't QueueSetShuffle - no element at path {path:?}",
                        "WARN".yellow()
                    );
                }
            }
            Action::QueueUnshuffle(path) => {
                if let Some(elem) = self.queue.get_item_at_index_mut(&path, 0) {
                    if let QueueContent::Folder(QueueFolder {
                        index,
                        content: _,
                        name: _,
                        order,
                    }) = elem.content_mut()
                    {
                        if let Some(ni) = order.as_ref().and_then(|v| v.get(*index).copied()) {
                            *index = ni;
                        }
                        *order = None;
                    }
                }
            }
            Action::AddSong(song, _) => {
                self.add_song_new(song);
            }
            Action::AddAlbum(album, _) => {
                self.add_album_new(album);
            }
            Action::AddArtist(artist, _) => {
                self.add_artist_new(artist);
            }
            Action::AddCover(cover, _) => _ = self.add_cover_new(cover),
            Action::ModifySong(song, _) => {
                _ = self.update_song(song);
            }
            Action::ModifyAlbum(album, _) => {
                _ = self.update_album(album);
            }
            Action::ModifyArtist(artist, _) => {
                _ = self.update_artist(artist);
            }
            Action::RemoveSong(song) => {
                _ = self.remove_song(song);
            }
            Action::RemoveAlbum(album) => {
                _ = self.remove_album(album);
            }
            Action::RemoveArtist(artist) => {
                _ = self.remove_artist(artist);
            }
            Action::TagSongFlagSet(id, tag) => {
                if let Some(v) = self.get_song_mut(&id) {
                    if !v.general.tags.contains(&tag) {
                        v.general.tags.push(tag);
                    }
                }
            }
            Action::TagSongFlagUnset(id, tag) => {
                if let Some(v) = self.get_song_mut(&id) {
                    if let Some(i) = v.general.tags.iter().position(|v| v == &tag) {
                        v.general.tags.remove(i);
                    }
                }
            }
            Action::TagAlbumFlagSet(id, tag) => {
                if let Some(v) = self.albums.get_mut(&id) {
                    if !v.general.tags.contains(&tag) {
                        v.general.tags.push(tag);
                    }
                }
            }
            Action::TagAlbumFlagUnset(id, tag) => {
                if let Some(v) = self.albums.get_mut(&id) {
                    if let Some(i) = v.general.tags.iter().position(|v| v == &tag) {
                        v.general.tags.remove(i);
                    }
                }
            }
            Action::TagArtistFlagSet(id, tag) => {
                if let Some(v) = self.artists.get_mut(&id) {
                    if !v.general.tags.contains(&tag) {
                        v.general.tags.push(tag);
                    }
                }
            }
            Action::TagArtistFlagUnset(id, tag) => {
                if let Some(v) = self.artists.get_mut(&id) {
                    if let Some(i) = v.general.tags.iter().position(|v| v == &tag) {
                        v.general.tags.remove(i);
                    }
                }
            }
            Action::TagSongPropertySet(id, key, val) => {
                if let Some(v) = self.get_song_mut(&id) {
                    let new = format!("{key}{val}");
                    if let Some(v) = v.general.tags.iter_mut().find(|v| v.starts_with(&key)) {
                        *v = new;
                    } else {
                        v.general.tags.push(new);
                    }
                }
            }
            Action::TagSongPropertyUnset(id, key) => {
                if let Some(v) = self.get_song_mut(&id) {
                    let tags = std::mem::replace(&mut v.general.tags, vec![]);
                    v.general.tags = tags.into_iter().filter(|v| !v.starts_with(&key)).collect();
                }
            }
            Action::TagAlbumPropertySet(id, key, val) => {
                if let Some(v) = self.albums.get_mut(&id) {
                    let new = format!("{key}{val}");
                    if let Some(v) = v.general.tags.iter_mut().find(|v| v.starts_with(&key)) {
                        *v = new;
                    } else {
                        v.general.tags.push(new);
                    }
                }
            }
            Action::TagAlbumPropertyUnset(id, key) => {
                if let Some(v) = self.albums.get_mut(&id) {
                    let tags = std::mem::replace(&mut v.general.tags, vec![]);
                    v.general.tags = tags.into_iter().filter(|v| !v.starts_with(&key)).collect();
                }
            }
            Action::TagArtistPropertySet(id, key, val) => {
                if let Some(v) = self.artists.get_mut(&id) {
                    let new = format!("{key}{val}");
                    if let Some(v) = v.general.tags.iter_mut().find(|v| v.starts_with(&key)) {
                        *v = new;
                    } else {
                        v.general.tags.push(new);
                    }
                }
            }
            Action::TagArtistPropertyUnset(id, key) => {
                if let Some(v) = self.artists.get_mut(&id) {
                    let tags = std::mem::replace(&mut v.general.tags, vec![]);
                    v.general.tags = tags.into_iter().filter(|v| !v.starts_with(&key)).collect();
                }
            }
            Action::SetSongDuration(id, duration) => {
                if let Some(song) = self.get_song_mut(&id) {
                    song.duration_millis = duration;
                }
            }
            Action::Multiple(actions) => {
                for action in actions {
                    self.apply_action_unchecked_seq(action, client);
                }
            }
            Action::InitComplete => {
                self.client_is_init = true;
            }
            Action::ErrorInfo(..) => {}
            Action::Denied(..) => {}
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
            seq: Commander::new(true),
            db_dir: PathBuf::new(),
            db_file: PathBuf::new(),
            lib_directory: PathBuf::new(),
            artists: HashMap::new(),
            albums: HashMap::new(),
            songs: HashMap::new(),
            covers: HashMap::new(),
            custom_files: None,
            queue: QueueContent::Folder(QueueFolder::default()).into(),
            update_endpoints: vec![],
            update_endpoints_id: 0,
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
            seq: Commander::new(false),
            db_dir: dir,
            db_file: path,
            lib_directory: lib_dir,
            artists: HashMap::new(),
            albums: HashMap::new(),
            songs: HashMap::new(),
            covers: HashMap::new(),
            custom_files: None,
            queue: QueueContent::Folder(QueueFolder::default()).into(),
            update_endpoints: vec![],
            update_endpoints_id: 0,
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
            seq: Commander::new(false),
            db_dir: dir,
            db_file: path,
            lib_directory,
            artists: ToFromBytes::from_bytes(&mut file)?,
            albums: ToFromBytes::from_bytes(&mut file)?,
            songs: ToFromBytes::from_bytes(&mut file)?,
            covers: ToFromBytes::from_bytes(&mut file)?,
            custom_files: None,
            queue: QueueContent::Folder(QueueFolder::default()).into(),
            update_endpoints: vec![],
            update_endpoints_id: 0,
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
    pub fn broadcast_update(&mut self, update: Action, client: Option<u64>) -> Action {
        match update {
            Action::InitComplete => return update,
            _ => {}
        }
        if !self.is_client() {
            self.seq.inc();
        }
        let mut update = self.seq.pack(update);
        let reqs = update.action.take_req_all();
        let mut remove = vec![];
        let mut bytes = None;
        let mut arc = None;
        for (i, (udepid, udep)) in self.update_endpoints.iter_mut().enumerate() {
            if reqs.iter().any(|r| r.is_some()) && client.is_some_and(|v| *udepid == v) {
                update.action.put_req_all(reqs.clone());
                match udep {
                    UpdateEndpoint::Bytes(writer) => {
                        if writer.write_all(&update.to_bytes_vec()).is_err() {
                            remove.push(i);
                        }
                    }
                    UpdateEndpoint::CmdChannel(sender) => {
                        if sender.send(Arc::new(update.clone())).is_err() {
                            remove.push(i);
                        }
                    }
                    UpdateEndpoint::Custom(func) => func(&update),
                    UpdateEndpoint::CustomArc(func) => func(Arc::new(update.clone())),
                    UpdateEndpoint::CustomBytes(func) => {
                        if bytes.is_none() {
                            bytes = Some(update.to_bytes_vec());
                        }
                        func(bytes.as_ref().unwrap())
                    }
                }
                update.action.take_req_all();
            }
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
                UpdateEndpoint::Custom(func) => func(&update),
                UpdateEndpoint::CustomArc(func) => {
                    if arc.is_none() {
                        arc = Some(Arc::new(update.clone()));
                    }
                    func(Arc::clone(arc.as_ref().unwrap()))
                }
                UpdateEndpoint::CustomBytes(func) => {
                    if bytes.is_none() {
                        bytes = Some(update.to_bytes_vec());
                    }
                    func(bytes.as_ref().unwrap())
                }
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
        update.action.put_req_all(reqs);
        update.action
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
impl PartialEq for Cover {
    fn eq(&self, other: &Self) -> bool {
        self.location == other.location
    }
}
impl Cover {
    pub fn get_bytes_from_file<O>(
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
