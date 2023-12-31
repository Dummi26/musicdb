use std::{collections::HashSet, sync::Arc};

use awedio::{
    backends::CpalBackend,
    manager::Manager,
    sounds::wrappers::{AsyncCompletionNotifier, Controller, Pausable},
    Sound,
};
use colorize::AnsiColor;
use rc_u8_reader::ArcU8Reader;

use crate::{
    data::{database::Database, SongId},
    server::Command,
};

pub struct Player {
    /// can be unused, but must be present otherwise audio playback breaks
    #[allow(unused)]
    backend: CpalBackend,
    source: Option<(
        Controller<AsyncCompletionNotifier<Pausable<Box<dyn Sound>>>>,
        tokio::sync::oneshot::Receiver<()>,
    )>,
    manager: Manager,
    current_song_id: SongOpt,
    cached: HashSet<SongId>,
    allow_sending_commands: bool,
}

pub enum SongOpt {
    None,
    Some(SongId),
    /// Will be set to Some or None once handeled
    New(Option<SongId>),
}

impl Player {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let (manager, backend) = awedio::start()?;
        Ok(Self {
            manager,
            backend,
            source: None,
            current_song_id: SongOpt::None,
            cached: HashSet::new(),
            allow_sending_commands: true,
        })
    }
    pub fn without_sending_commands(mut self) -> Self {
        self.allow_sending_commands = false;
        self
    }
    pub fn handle_command(&mut self, command: &Command) {
        match command {
            Command::Resume => self.resume(),
            Command::Pause => self.pause(),
            Command::Stop => self.stop(),
            _ => {}
        }
    }
    pub fn pause(&mut self) {
        if let Some((source, _notif)) = &mut self.source {
            source.set_paused(true);
        }
    }
    pub fn resume(&mut self) {
        if let Some((source, _notif)) = &mut self.source {
            source.set_paused(false);
        } else if let SongOpt::Some(id) = &self.current_song_id {
            // there is no source to resume playback on, but there is a current song
            self.current_song_id = SongOpt::New(Some(*id));
        }
    }
    pub fn stop(&mut self) {
        if let Some((source, _notif)) = &mut self.source {
            source.set_paused(true);
        }
        if let SongOpt::Some(id) | SongOpt::New(Some(id)) = self.current_song_id {
            self.current_song_id = SongOpt::New(Some(id));
        } else {
            self.current_song_id = SongOpt::New(None);
        }
    }
    pub fn update(&mut self, db: &mut Database) {
        macro_rules! apply_command {
            ($cmd:expr) => {
                if self.allow_sending_commands {
                    db.apply_command($cmd);
                }
            };
        }
        if db.playing && self.source.is_none() {
            if let Some(song) = db.queue.get_current_song() {
                // db playing, but no source - initialize a source (via SongOpt::New)
                self.current_song_id = SongOpt::New(Some(*song));
            } else {
                // db.playing, but no song in queue...
                apply_command!(Command::Stop);
            }
        } else if let Some((_source, notif)) = &mut self.source {
            if let Ok(()) = notif.try_recv() {
                // song has finished playing
                apply_command!(Command::NextSong);
                self.current_song_id = SongOpt::New(db.queue.get_current_song().cloned());
            }
        }

        // check the queue's current index
        if let SongOpt::None = self.current_song_id {
            if let Some(id) = db.queue.get_current_song() {
                self.current_song_id = SongOpt::New(Some(*id));
            }
        } else if let SongOpt::Some(l_id) = &self.current_song_id {
            if let Some(id) = db.queue.get_current_song() {
                if *id != *l_id {
                    self.current_song_id = SongOpt::New(Some(*id));
                }
            } else {
                self.current_song_id = SongOpt::New(None);
            }
        }

        // new current song
        if let SongOpt::New(song_opt) = &self.current_song_id {
            // stop playback
            // eprintln!("[play] stopping playback");
            self.manager.clear();
            if let Some(song_id) = song_opt {
                // start playback again
                if let Some(song) = db.get_song(song_id) {
                    // eprintln!("[play] starting playback...");
                    // add our song
                    let ext = match &song.location.rel_path.extension() {
                        Some(s) => s.to_str().unwrap_or(""),
                        None => "",
                    };
                    if db.playing {
                        if let Some(bytes) = song.cached_data_now(db) {
                            self.cached.insert(song.id);
                            match Self::sound_from_bytes(ext, bytes) {
                                Ok(v) => {
                                    let (sound, notif) =
                                        v.pausable().with_async_completion_notifier();
                                    // add it
                                    let (sound, controller) = sound.controllable();
                                    self.source = Some((controller, notif));
                                    // and play it
                                    self.manager.play(Box::new(sound));
                                }
                                Err(e) => {
                                    eprintln!(
                                        "[{}] [player] Can't play, skipping! {e}",
                                        "INFO".blue()
                                    );
                                    apply_command!(Command::NextSong);
                                }
                            }
                        } else {
                            // couldn't load song bytes
                            db.broadcast_update(&Command::ErrorInfo(
                                "NoSongData".to_owned(),
                                format!("Couldn't load song #{}\n({})", song.id, song.title),
                            ));
                            apply_command!(Command::NextSong);
                        }
                    } else {
                        self.source = None;
                        song.cache_data_start_thread(&db);
                        self.cached.insert(song.id);
                    }
                } else {
                    panic!("invalid song ID: current_song_id not found in DB!");
                }
                self.current_song_id = SongOpt::Some(*song_id);
            } else {
                self.current_song_id = SongOpt::None;
            }
            let next_song = db.queue.get_next_song().and_then(|v| db.get_song(v));
            for &id in &self.cached {
                if Some(id) != next_song.map(|v| v.id)
                    && !matches!(self.current_song_id, SongOpt::Some(v) if v == id)
                {
                    if let Some(song) = db.songs().get(&id) {
                        if let Ok(()) = song.uncache_data() {
                            self.cached.remove(&id);
                            break;
                        }
                    } else {
                        self.cached.remove(&id);
                        break;
                    }
                }
            }
            if let Some(song) = next_song {
                song.cache_data_start_thread(&db);
                self.cached.insert(song.id);
            }
        }
    }

    /// partly identical to awedio/src/sounds/open_file.rs open_file_with_reader(), which is a private function I can't access
    fn sound_from_bytes(
        extension: &str,
        bytes: Arc<Vec<u8>>,
    ) -> Result<Box<dyn Sound>, std::io::Error> {
        let reader = ArcU8Reader::new(bytes);
        Ok(match extension {
            "wav" => Box::new(
                awedio::sounds::decoders::WavDecoder::new(reader)
                    .map_err(|_e| std::io::Error::from(std::io::ErrorKind::InvalidData))?,
            ),
            "mp3" => Box::new(awedio::sounds::decoders::Mp3Decoder::new(reader)),
            _ => return Err(std::io::Error::from(std::io::ErrorKind::Unsupported)),
        })
    }
}
