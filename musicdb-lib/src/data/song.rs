use std::{
    fmt::Display,
    io::{Read, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
    thread::JoinHandle,
};

use colorize::AnsiColor;

use crate::load::ToFromBytes;

use super::{
    database::{ClientIo, Database},
    AlbumId, ArtistId, CoverId, DatabaseLocation, GeneralData, SongId,
};

#[derive(Clone, Debug)]
pub struct Song {
    pub id: SongId,
    pub location: DatabaseLocation,
    pub title: String,
    pub album: Option<AlbumId>,
    pub artist: ArtistId,
    pub more_artists: Vec<ArtistId>,
    pub cover: Option<CoverId>,
    pub file_size: u64,
    /// song duration in milliseconds
    pub duration_millis: u64,
    pub general: GeneralData,
    /// None => No cached data
    /// Some(Err) => No cached data yet, but a thread is working on loading it.
    /// Some(Ok(data)) => Cached data is available.
    pub cached_data: CachedData,
}
impl Song {
    pub fn new(
        location: DatabaseLocation,
        title: String,
        album: Option<AlbumId>,
        artist: ArtistId,
        more_artists: Vec<ArtistId>,
        cover: Option<CoverId>,
        file_size: u64,
        duration_millis: u64,
    ) -> Self {
        Self {
            id: 0,
            location,
            title,
            album,
            artist,
            more_artists,
            cover,
            file_size,
            duration_millis,
            general: GeneralData::default(),
            cached_data: CachedData(Arc::new(Mutex::new((None, None)))),
        }
    }
    pub fn uncache_data(&self) -> Result<bool, ()> {
        let mut cached = self.cached_data.0.lock().unwrap();
        match cached.0.take() {
            Some(Ok(_data)) => Ok(true),
            Some(Err(thread)) => {
                if thread.is_finished() {
                    // get value from thread and drop it
                    _ = thread.join();
                    Ok(true)
                } else {
                    // thread is still running...
                    cached.0 = Some(Err(thread));
                    Err(())
                }
            }
            None => Ok(false),
        }
    }
    /// If no data is cached yet and no caching thread is running, starts a thread to cache the data.
    pub fn cache_data_start_thread(&self, db: &Database) -> bool {
        self.cache_data_start_thread_or_say_already_running(db)
            .is_ok()
    }
    pub fn cache_data_start_thread_or_say_already_running(
        &self,
        db: &Database,
    ) -> Result<(), bool> {
        let mut cd = self.cached_data.0.lock().unwrap();
        match cd.0.as_ref() {
            None => (),
            Some(Err(t)) => return Err(!t.is_finished()),
            Some(Ok(_)) => return Err(false),
        };
        let src = if let Some(dlcon) = &db.remote_server_as_song_file_source {
            Err((self.id, Arc::clone(dlcon)))
        } else {
            Ok(db.get_path(&self.location))
        };
        cd.0 = Some(Err(std::thread::spawn(move || {
            let data = Self::load_data(src)?;
            Some(Arc::new(data))
        })));
        Ok(())
    }
    /// If the song's data is cached, returns the number of bytes.
    pub fn has_cached_data(&self) -> Option<usize> {
        if let Some(Ok(v)) = self.cached_data.0.lock().unwrap().0.as_ref() {
            Some(v.len())
        } else {
            None
        }
    }
    /// Gets the cached data, if available.
    /// If a thread is running to load the data, it is not awaited.
    /// This function doesn't block.
    pub fn cached_data(&self) -> Option<Arc<Vec<u8>>> {
        if let Some(Ok(v)) = self.cached_data.0.lock().unwrap().0.as_ref() {
            Some(Arc::clone(v))
        } else {
            None
        }
    }
    /// Gets or loads the cached data.
    /// If a thread is running to load the data, it *is* awaited.
    /// This function will block until the data is loaded.
    /// If it still returns none, some error must have occured.
    pub fn cached_data_now(&self, db: &Database) -> Option<Arc<Vec<u8>>> {
        let mut cd = self.cached_data.0.lock().unwrap();
        cd.0 = match cd.0.take() {
            None => {
                let src = if let Some(dlcon) = &db.remote_server_as_song_file_source {
                    Err((self.id, Arc::clone(dlcon)))
                } else {
                    Ok(db.get_path(&self.location))
                };
                if let Some(v) = Self::load_data(src) {
                    Some(Ok(Arc::new(v)))
                } else {
                    None
                }
            }
            Some(Err(t)) => match t.join() {
                Err(_e) => None,
                Ok(Some(v)) => Some(Ok(v)),
                Ok(None) => None,
            },
            Some(Ok(v)) => Some(Ok(v)),
        };
        if let Some(Ok(v)) = &cd.0 {
            cd.1 = Some(v.len());
        }
        drop(cd);
        self.cached_data()
    }
    fn load_data(
        src: Result<
            PathBuf,
            (
                SongId,
                Arc<Mutex<crate::server::get::Client<Box<dyn ClientIo>>>>,
            ),
        >,
    ) -> Option<Vec<u8>> {
        match src {
            Ok(path) => {
                eprintln!("[{}] loading song from {:?}", "INFO".cyan(), path);
                match std::fs::read(&path) {
                    Ok(v) => {
                        eprintln!("[{}] loaded song from {:?}", "INFO".green(), path);
                        Some(v)
                    }
                    Err(e) => {
                        eprintln!("[{}] error loading {:?}: {e:?}", "ERR!".red(), path);
                        None
                    }
                }
            }
            Err((id, dlcon)) => {
                eprintln!("[{}] loading song {id}", "INFO".cyan());
                match dlcon
                    .lock()
                    .unwrap()
                    .song_file(id)
                    .expect("problem with downloader connection...")
                {
                    Ok(data) => Some(data),
                    Err(e) => {
                        eprintln!("[{}] error loading song {id}: {e}", "ERR!".red());
                        None
                    }
                }
            }
        }
    }
}
impl Display for Song {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.title)?;
        match self.album {
            Some(album) => write!(f, " (by {} on {album})", self.artist)?,
            None => write!(f, " (by {})", self.artist)?,
        }
        Ok(())
    }
}

impl ToFromBytes for Song {
    fn to_bytes<T>(&self, s: &mut T) -> Result<(), std::io::Error>
    where
        T: Write,
    {
        self.id.to_bytes(s)?;
        self.location.to_bytes(s)?;
        self.title.to_bytes(s)?;
        self.album.to_bytes(s)?;
        self.artist.to_bytes(s)?;
        self.more_artists.to_bytes(s)?;
        self.cover.to_bytes(s)?;
        self.file_size.to_bytes(s)?;
        self.duration_millis.to_bytes(s)?;
        self.general.to_bytes(s)?;
        Ok(())
    }
    fn from_bytes<T>(s: &mut T) -> Result<Self, std::io::Error>
    where
        T: Read,
    {
        Ok(Self {
            id: ToFromBytes::from_bytes(s)?,
            location: ToFromBytes::from_bytes(s)?,
            title: ToFromBytes::from_bytes(s)?,
            album: ToFromBytes::from_bytes(s)?,
            artist: ToFromBytes::from_bytes(s)?,
            more_artists: ToFromBytes::from_bytes(s)?,
            cover: ToFromBytes::from_bytes(s)?,
            file_size: ToFromBytes::from_bytes(s)?,
            duration_millis: ToFromBytes::from_bytes(s)?,
            general: ToFromBytes::from_bytes(s)?,
            cached_data: CachedData(Arc::new(Mutex::new((None, None)))),
        })
    }
}

#[derive(Clone, Debug)]
pub struct CachedData(
    pub  Arc<
        Mutex<(
            Option<Result<Arc<Vec<u8>>, JoinHandle<Option<Arc<Vec<u8>>>>>>,
            Option<usize>,
        )>,
    >,
);
