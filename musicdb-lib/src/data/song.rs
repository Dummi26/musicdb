use std::{
    fmt::Display,
    io::{Read, Write},
    mem::replace,
    path::PathBuf,
    sync::{Arc, Mutex},
    thread::JoinHandle,
    time::Instant,
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
    pub file_last_modified_unix_timestamp: Option<u64>,
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
        file_last_modified_unix_timestamp: Option<u64>,
        title: String,
        album: Option<AlbumId>,
        artist: ArtistId,
        more_artists: Vec<ArtistId>,
        cover: Option<CoverId>,
        file_size: u64,
        duration_millis: u64,
        general: GeneralData,
    ) -> Self {
        Self {
            id: 0,
            location,
            file_last_modified_unix_timestamp,
            title,
            album,
            artist,
            more_artists,
            cover,
            file_size,
            duration_millis,
            general,
            cached_data: CachedData(Arc::new(Mutex::new((Err(None), None)))),
        }
    }

    pub fn cached_data(&self) -> &CachedData {
        &self.cached_data
    }
}
impl CachedData {
    pub fn uncache_data(&self) -> Result<bool, ()> {
        let mut cached = self.0.lock().unwrap();
        match replace(&mut cached.0, Err(None)) {
            Ok(Ok(_data)) => Ok(true),
            Ok(Err(thread)) => {
                if thread.is_finished() {
                    // get value from thread and drop it
                    _ = thread.join();
                    Ok(true)
                } else {
                    // thread is still running...
                    cached.0 = Ok(Err(thread));
                    Err(())
                }
            }
            Err(e) => {
                cached.0 = Err(e);
                Ok(false)
            }
        }
    }
    /// If no data is cached yet and no caching thread is running, starts a thread to cache the data.
    pub fn cache_data_start_thread(&self, db: &Database, song: &Song) -> bool {
        self.cache_data_start_thread_or_say_already_running(db, song) == Ok(true)
    }
    /// Ok(true) => thread started,
    /// Ok(false) => data was already loaded
    pub fn cache_data_start_thread_or_say_already_running(
        &self,
        db: &Database,
        song: &Song,
    ) -> Result<bool, bool> {
        self.get_data_or_start_thread_and_say_already_running(db, |_| false, || true, song)
    }
    /// gets the data if available, or, if no thread is running, starts a thread to get the data.
    /// if a thread is running, was started, or recently encountered an error, `None` is returned, otherwise `Some(data)`.
    pub fn get_data_or_maybe_start_thread(
        &self,
        db: &Database,
        song: &Song,
    ) -> Option<Arc<Vec<u8>>> {
        self.get_data_or_start_thread_and_say_already_running(
            db,
            |data| Some(Arc::clone(data)),
            || None,
            song,
        )
        .ok()
        .and_then(|v| v)
    }
    /// `Err(true)` if a thread is already running,
    /// `Ok(get_data(data))` if there is data,
    /// `Ok(started())` if a thread was started,
    /// `Err(false)` otherwise (i.e. loading data failed recently, 60 second cooldown between retries is active).
    pub fn get_data_or_start_thread_and_say_already_running<T>(
        &self,
        db: &Database,
        get_data: impl FnOnce(&Arc<Vec<u8>>) -> T,
        started: impl FnOnce() -> T,
        song: &Song,
    ) -> Result<T, bool> {
        let mut cd = self.0.lock().unwrap();
        match cd.0.as_mut() {
            Err(Some(i)) if i.elapsed().as_secs_f32() < 60.0 => return Err(false),
            Err(_) => (),
            Ok(Err(t)) => {
                if t.is_finished() {
                    if let Some(bytes) = replace(&mut cd.0, Err(None))
                        .unwrap()
                        .unwrap_err()
                        .join()
                        .unwrap()
                    {
                        cd.0 = Ok(Ok(bytes));
                        return Ok(get_data(cd.0.as_ref().unwrap().as_ref().unwrap()));
                    } else {
                        cd.0 = Err(Some(Instant::now()));
                        return Err(false);
                    }
                } else {
                    return Err(true);
                }
            }
            Ok(Ok(bytes)) => return Ok(get_data(&*bytes)),
        };
        let src = if let Some(dlcon) = &db.remote_server_as_song_file_source {
            Err((song.id, Arc::clone(dlcon)))
        } else {
            Ok(db.get_path(&song.location))
        };
        cd.0 = Ok(Err(std::thread::spawn(move || {
            let data = Self::load_data(src)?;
            Some(Arc::new(data))
        })));
        Ok(started())
    }
    /// If the song's data is cached, returns the number of bytes.
    pub fn has_cached_data(&self) -> Option<usize> {
        if let Ok(Ok(v)) = self.0.lock().unwrap().0.as_ref() {
            Some(v.len())
        } else {
            None
        }
    }
    /// Gets the cached data, if available.
    /// If a thread is running to load the data, it is not awaited.
    /// This function doesn't block.
    pub fn cached_data(&self) -> Option<Arc<Vec<u8>>> {
        if let Ok(Ok(v)) = self.0.lock().unwrap().0.as_ref() {
            Some(Arc::clone(v))
        } else {
            None
        }
    }
    /// Gets the cached data, if available.
    /// If a thread is running to load the data, it is awaited.
    /// This function doesn't block.
    pub fn cached_data_await(&self) -> Option<Arc<Vec<u8>>> {
        let mut cd = self.0.lock().unwrap();
        let (out, next) = match replace(&mut cd.0, Err(None)) {
            Ok(Ok(bytes)) => (Some(Arc::clone(&bytes)), Ok(Ok(bytes))),
            Ok(Err(t)) => {
                if let Some(bytes) = t.join().unwrap() {
                    (Some(Arc::clone(&bytes)), Ok(Ok(bytes)))
                } else {
                    (None, Err(Some(Instant::now())))
                }
            }
            Err(e) => (None, Err(e)),
        };
        cd.0 = next;
        out
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
            Some(album) => write!(f, " ({} by {} on {album})", self.id, self.artist)?,
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
        self.file_last_modified_unix_timestamp.to_bytes(s)?;
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
            file_last_modified_unix_timestamp: ToFromBytes::from_bytes(s)?,
            title: ToFromBytes::from_bytes(s)?,
            album: ToFromBytes::from_bytes(s)?,
            artist: ToFromBytes::from_bytes(s)?,
            more_artists: ToFromBytes::from_bytes(s)?,
            cover: ToFromBytes::from_bytes(s)?,
            file_size: ToFromBytes::from_bytes(s)?,
            duration_millis: ToFromBytes::from_bytes(s)?,
            general: ToFromBytes::from_bytes(s)?,
            cached_data: CachedData(Arc::new(Mutex::new((Err(None), None)))),
        })
    }
}

#[derive(Debug)]
pub struct CachedData(
    pub  Arc<
        Mutex<(
            Result<Result<Arc<Vec<u8>>, JoinHandle<Option<Arc<Vec<u8>>>>>, Option<Instant>>,
            Option<usize>,
        )>,
    >,
);
impl Clone for CachedData {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}
