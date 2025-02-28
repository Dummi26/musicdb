#[cfg(feature = "playback-via-playback-rs")]
pub mod playback_rs;
#[cfg(feature = "playback-via-rodio")]
pub mod rodio;
#[cfg(feature = "playback-via-sleep")]
pub mod sleep;
#[cfg(feature = "playback-via-playback-rs")]
pub type PlayerBackendFeat<T> = playback_rs::PlayerBackendPlaybackRs<T>;
#[cfg(feature = "playback-via-rodio")]
pub type PlayerBackendFeat<T> = rodio::PlayerBackendRodio<T>;

use std::{collections::HashMap, ffi::OsStr, sync::Arc};

use crate::{
    data::{
        database::Database,
        song::{CachedData, Song},
        SongId,
    },
    server::Action,
};

pub struct Player<T: PlayerBackend<SongCustomData>> {
    cached: HashMap<SongId, CachedData>,
    pub backend: T,
    allow_sending_commands: bool,
}

pub struct SongCustomData {
    load_duration: bool,
}
pub trait PlayerBackend<T> {
    /// load the next song from its bytes
    fn load_next_song(
        &mut self,
        id: SongId,
        song: &Song,
        filename: &OsStr,
        bytes: Arc<Vec<u8>>,
        load_duration: bool,
        custom_data: T,
    );

    /// pause playback. if `resume` is called, resume the song where it was paused.
    fn pause(&mut self);
    /// stop playback. if `resume` is called, restart the song.
    fn stop(&mut self);
    /// used after pause or stop.
    /// does nothing if no song was playing, song was cleared, or a song is already playing.
    fn resume(&mut self);

    /// stop and discard the currently playing song, then set the next song as the current one.
    /// `play` decides whether the next song should start playing or not.
    fn next(&mut self, play: bool, load_duration: bool);
    /// stop and discard the currently playing and next song.
    /// calling `resume` after this was called but before a new song was loaded does nothing.
    fn clear(&mut self);

    /// Should be `true` after calling `resume()` or `next(true)` if `current_song().is_some()`
    fn playing(&self) -> bool;

    /// - `None` before a song is loaded or after `clear` was called,
    /// - `Some((id, false))` while loading (if loading is done on a separate thread),
    /// - `Some((id, true))` if a song is loaded and ready to be played (or loading failed)
    /// performance notes: must be fast, as it is called repeatedly
    fn current_song(&self) -> Option<(SongId, bool, &T)>;
    /// like `has_current_song`.
    /// performance notes: must be fast, as it is called repeatedly
    fn next_song(&self) -> Option<(SongId, bool, &T)>;

    fn gen_data_mut(&mut self) -> (Option<&mut T>, Option<&mut T>);

    // if true, call `song_finished` more often. if false, song_finished may also be a constant `false`, but events should be sent directly to the server instead.
    // this **must be constant**: it cannot change after the backend was constructed
    fn song_finished_polling(&self) -> bool;

    /// true if the currently playing song has finished playing.
    /// this may return a constant `false` if the playback thread automatically sends a `NextSong` command to the database when this happens.
    /// performance notes: must be fast, as it is called repeatedly
    fn song_finished(&self) -> bool;

    /// If possible, return the current song's duration in milliseconds.
    /// It could also just return `None`.
    /// If `load_duration` was `false` in either `load_next_song` or `next`, for performance reasons,
    /// this should probably return `None` (unless getting the duration is virtually free).
    /// `load_duration` can be ignored if you don't want to load the duration anyway, it's just there to prevent you from loading the duration if it won't be used anyway
    fn current_song_duration(&self) -> Option<u64>;

    /// If known, get the current playback position in the song, in milliseconds.
    fn current_song_playback_position(&self) -> Option<u64>;
}

impl<T: PlayerBackend<SongCustomData>> Player<T> {
    pub fn new(backend: T) -> Self {
        Self {
            cached: HashMap::new(),
            backend,
            allow_sending_commands: true,
        }
    }
    pub fn new_client(backend: T) -> Self {
        Self {
            cached: HashMap::new(),
            backend,
            allow_sending_commands: false,
        }
    }
    pub fn handle_action(&mut self, action: &Action) {
        match action {
            Action::Resume => self.resume(),
            Action::Pause => self.pause(),
            Action::Stop => self.stop(),
            _ => {}
        }
    }
    pub fn pause(&mut self) {
        self.backend.pause();
    }
    pub fn resume(&mut self) {
        self.backend.resume();
    }
    pub fn stop(&mut self) {
        self.backend.stop();
    }

    pub fn update(&mut self, db: &mut Database) {
        self.update_uncache_opt(db, true)
    }
    /// never uncache songs (this is something the CacheManager has to do if you decide to use this function)
    pub fn update_dont_uncache(&mut self, db: &mut Database) {
        self.update_uncache_opt(db, false)
    }
    pub fn update_uncache_opt(&mut self, db: &mut Database, allow_uncaching: bool) {
        if self.allow_sending_commands {
            if self.allow_sending_commands && self.backend.song_finished() {
                db.apply_action_unchecked_seq(Action::NextSong, None);
            }
        }

        let queue_current_song = db.queue.get_current_song().copied();
        let queue_next_song = db.queue.get_next_song().copied();

        match (self.backend.current_song().map(|v| v.0), queue_current_song) {
            (None, None) => (),
            (Some(a), Some(b)) if a == b => (),
            (_, Some(id)) => {
                if self.backend.next_song().map(|v| v.0) == queue_current_song {
                    let load_duration = self
                        .backend
                        .next_song()
                        .is_some_and(|(_, _, t)| t.load_duration);
                    self.backend.next(db.playing, load_duration);
                    if self.allow_sending_commands && load_duration {
                        if let Some(dur) = self.backend.current_song_duration() {
                            db.apply_action_unchecked_seq(Action::SetSongDuration(id, dur), None)
                        }
                    }
                } else if let Some(song) = db.get_song(&id) {
                    self.cached.insert(id, song.cached_data().clone());
                    if let Some(bytes) = song
                        .cached_data()
                        .get_data_or_maybe_start_thread(db, song)
                        .or_else(|| song.cached_data().cached_data_await())
                    {
                        let load_duration = song.duration_millis == 0;
                        self.backend.load_next_song(
                            id,
                            song,
                            song.location
                                .rel_path
                                .file_name()
                                .unwrap_or_else(|| OsStr::new("")),
                            bytes,
                            load_duration,
                            SongCustomData { load_duration },
                        );
                        self.backend.next(db.playing, load_duration);
                        if self.allow_sending_commands && load_duration {
                            if let Some(dur) = self.backend.current_song_duration() {
                                db.apply_action_unchecked_seq(
                                    Action::SetSongDuration(id, dur),
                                    None,
                                )
                            }
                        }
                    } else {
                        // only show an error if the user tries to play the song.
                        // otherwise, the error might be spammed.
                        if self.allow_sending_commands && db.playing {
                            db.apply_action_unchecked_seq(
                                Action::ErrorInfo(
                                    format!("Couldn't load bytes for song {id}"),
                                    format!(
                                        "Song: {}\nby {:?} on {:?}",
                                        song.title, song.artist, song.album
                                    ),
                                ),
                                None,
                            );
                            db.apply_action_unchecked_seq(Action::NextSong, None);
                        }
                        self.backend.clear();
                    }
                } else {
                    self.backend.clear();
                }
            }
            (Some(_), None) => self.backend.clear(),
        }
        match (self.backend.next_song().map(|v| v.0), queue_next_song) {
            (None, None) => (),
            (Some(a), Some(b)) if a == b => (),
            (_, Some(id)) => {
                if let Some(song) = db.get_song(&id) {
                    self.cached.insert(id, song.cached_data().clone());
                    if let Some(bytes) =
                        song.cached_data().get_data_or_maybe_start_thread(&db, song)
                    {
                        let load_duration = song.duration_millis == 0;
                        self.backend.load_next_song(
                            id,
                            song,
                            song.location
                                .rel_path
                                .file_name()
                                .unwrap_or_else(|| OsStr::new("")),
                            bytes,
                            load_duration,
                            SongCustomData { load_duration },
                        );
                    }
                }
            }
            (Some(_), None) => (),
        }
        if db.playing != self.backend.playing() {
            if db.playing {
                self.backend.resume();
            } else {
                self.backend.pause();
            }
        }

        if allow_uncaching {
            for (&id, cd) in &self.cached {
                if Some(id) != queue_current_song && Some(id) != queue_next_song {
                    if let Ok(_) = cd.uncache_data() {
                        self.cached.remove(&id);
                        break;
                    }
                }
            }
        }
    }
}
