use std::{
    sync::{
        atomic::{AtomicU32, AtomicU64},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use colorize::AnsiColor;

use super::database::Database;

// CacheManage will never uncache the currently playing song or the song that will be played next.

pub struct CacheManager {
    /// Amount of bytes. If free system memory drops below this number, initiate cleanup.
    pub min_avail_mem: Arc<AtomicU64>,
    /// Amount of bytes. If free system memory is greater than this number, consider caching more songs.
    pub max_avail_mem: Arc<AtomicU64>,
    pub songs_to_cache: Arc<AtomicU32>,
    #[allow(unused)]
    thread: Arc<JoinHandle<()>>,
}

impl CacheManager {
    pub fn new(database: Arc<Mutex<Database>>) -> Self {
        let min_avail_mem = Arc::new(AtomicU64::new(1024 * 1024 * 1024));
        let max_avail_mem = Arc::new(AtomicU64::new(1024 * 1024 * 2048));
        // if < 2, does the same as 2.
        let songs_to_cache = Arc::new(AtomicU32::new(10));
        Self {
            min_avail_mem: Arc::clone(&min_avail_mem),
            max_avail_mem: Arc::clone(&max_avail_mem),
            songs_to_cache: Arc::clone(&songs_to_cache),
            thread: Arc::new(thread::spawn(move || {
                let sleep_dur_long = Duration::from_secs(20);
                let sleep_dur_short = Duration::from_secs(1);
                let mut si = sysinfo::System::new_with_specifics(
                    sysinfo::RefreshKind::new()
                        .with_memory(sysinfo::MemoryRefreshKind::new().with_ram()),
                );
                eprintln!("[{}] Starting CacheManager", "INFO".cyan());
                let mut sleep_short = true;
                loop {
                    thread::sleep(if sleep_short {
                        sleep_dur_short
                    } else {
                        sleep_dur_long
                    });
                    sleep_short = false;
                    si.refresh_memory_specifics(sysinfo::MemoryRefreshKind::new().with_ram());
                    let available_memory = si.available_memory();
                    let min_avail_mem = min_avail_mem.load(std::sync::atomic::Ordering::Relaxed);
                    let low_memory = available_memory < min_avail_mem;
                    let max_avail_mem = max_avail_mem.load(std::sync::atomic::Ordering::Relaxed);
                    let songs_to_cache = songs_to_cache.load(std::sync::atomic::Ordering::Relaxed);

                    // let db_lock_start_time = Instant::now();

                    let db = database.lock().unwrap();

                    let (queue_current_song, queue_next_song, ids_to_cache) = if songs_to_cache <= 2
                    {
                        let queue_current_song = db.queue.get_current_song().copied();
                        let queue_next_song = db.queue.get_next_song().copied();

                        (
                            queue_current_song,
                            queue_next_song,
                            match (queue_current_song, queue_next_song) {
                                (None, None) => vec![],
                                (Some(a), None) | (None, Some(a)) => vec![a],
                                (Some(a), Some(b)) => {
                                    if a == b {
                                        vec![a]
                                    } else {
                                        vec![a, b]
                                    }
                                }
                            },
                        )
                    } else {
                        let mut queue = db.queue.clone();

                        let queue_current_song = queue.get_current_song().copied();
                        queue.advance_index_inner();
                        let queue_next_song = queue.get_current_song().copied();

                        let mut ids_to_cache = queue_current_song
                            .into_iter()
                            .chain(queue_next_song)
                            .collect::<Vec<_>>();

                        for _ in 2..songs_to_cache {
                            queue.advance_index_inner();
                            if let Some(id) = queue.get_current_song() {
                                if !ids_to_cache.contains(id) {
                                    ids_to_cache.push(*id);
                                }
                            } else {
                                break;
                            }
                        }

                        (queue_current_song, queue_next_song, ids_to_cache)
                    };

                    if low_memory {
                        let mut found = false;
                        for (id, song) in db.songs().iter() {
                            if !ids_to_cache.contains(id) {
                                if let Ok(true) = song.uncache_data() {
                                    eprintln!(
                                        "[{}] CacheManager :: Uncached bytes for song '{}'.",
                                        "INFO".cyan(),
                                        song.title
                                    );
                                    found = true;
                                    break;
                                }
                            }
                        }
                        if !found {
                            // also uncache songs that should be cached, but not current/next song
                            for id in ids_to_cache.iter().rev() {
                                if !(queue_current_song.is_some_and(|i| i == *id)
                                    || queue_next_song.is_some_and(|i| i == *id))
                                {
                                    if let Some(song) = db.get_song(id) {
                                        if let Ok(true) = song.uncache_data() {
                                            eprintln!(
                                                "[{}] CacheManager :: Uncached bytes for song '{}'.",
                                                "INFO".cyan(),
                                                song.title
                                            );
                                            found = true;
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        if found {
                            // uncache more songs
                            sleep_short = true;
                        }
                    } else if available_memory > max_avail_mem {
                        // we have some memory left, maybe cache a song (or cache multiple if we know their byte-sizes)
                        for song in &ids_to_cache {
                            if let Some(song) = db.get_song(song) {
                                match song.cache_data_start_thread_or_say_already_running(&db) {
                                    Err(false) => (),
                                    // thread already running, don't start a second one (otherwise we may load too many songs, using too much memory, causing a cache-uncache-cycle)
                                    Err(true) => {
                                        break;
                                    }
                                    Ok(()) => {
                                        eprintln!(
                                            "[{}] CacheManager :: Start caching bytes for song '{}'.",
                                            "INFO".cyan(),
                                            song.title
                                        );
                                        sleep_short = true;
                                        break;
                                    }
                                }
                            }
                        }
                    }

                    if let Some(song_id) = queue_next_song {
                        if let Some(song) = db.get_song(&song_id) {
                            if song.cache_data_start_thread(&db) {
                                eprintln!(
                                    "[{}] CacheManager :: Start caching bytes for next song, '{}'.",
                                    "INFO".cyan(),
                                    song.title
                                );
                            }
                        }
                    }
                }
            })),
        }
    }
    /// Songs will be removed from cache if `available_memory < min_avail_mem`.
    /// New songs will only be cached if `available_memory > max_avail_mem`.
    /// `min` and `max` in MiB (1024*1024 Bytes)
    pub fn set_memory_mib(&self, min: u64, max: u64) {
        self.min_avail_mem
            .store(1024 * 1024 * min, std::sync::atomic::Ordering::Relaxed);
        self.max_avail_mem
            .store(1024 * 1024 * max, std::sync::atomic::Ordering::Relaxed);
    }

    /// How many songs to load ahead of time. `< 2` behaves like `2`.
    /// Songs will be cached slowly over time.
    /// New songs will only be cached if `available_memory > max_avail_mem`.
    pub fn set_cache_songs_count(&self, count: u32) {
        self.songs_to_cache
            .store(count, std::sync::atomic::Ordering::Relaxed);
    }
}
