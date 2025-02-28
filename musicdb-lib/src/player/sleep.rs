use std::{
    ffi::OsStr,
    sync::Arc,
    time::{Duration, Instant},
};

use crate::{
    data::{song::Song, SongId},
    server::Command,
};

use super::PlayerBackend;

pub struct PlayerBackendSleep<T> {
    current: Option<(SongId, u64, T)>,
    next: Option<(SongId, u64, T)>,
    /// unused, but could be used to do something smarter than polling at some point
    #[allow(unused)]
    command_sender: Option<std::sync::mpsc::Sender<(Command, Option<u64>)>>,
    finished: SongFinished,
}
#[derive(Debug)]
enum SongFinished {
    Never,
    In(Duration),
    At(Instant),
}

impl<T> PlayerBackendSleep<T> {
    pub fn new(
        command_sender: Option<std::sync::mpsc::Sender<(Command, Option<u64>)>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            current: None,
            next: None,
            command_sender,
            finished: SongFinished::Never,
        })
    }
    fn set_finished(&mut self, play: bool) {
        self.finished = if let Some((_, duration, _)) = &self.current {
            let duration = Duration::from_millis(*duration);
            if play {
                SongFinished::At(Instant::now() + duration)
            } else {
                SongFinished::In(duration)
            }
        } else {
            SongFinished::Never
        };
    }
}

impl<T> PlayerBackend<T> for PlayerBackendSleep<T> {
    fn load_next_song(
        &mut self,
        id: SongId,
        song: &Song,
        _filename: &OsStr,
        _bytes: Arc<Vec<u8>>,
        _load_duration: bool,
        custom_data: T,
    ) {
        self.next = Some((id, song.duration_millis, custom_data));
    }
    fn pause(&mut self) {
        match self.finished {
            SongFinished::Never | SongFinished::In(_) => {}
            SongFinished::At(time) => {
                self.finished = SongFinished::In(time.saturating_duration_since(Instant::now()));
            }
        }
    }
    fn stop(&mut self) {
        self.set_finished(false);
    }
    fn resume(&mut self) {
        match self.finished {
            SongFinished::Never | SongFinished::At(_) => {}
            SongFinished::In(dur) => {
                self.finished = SongFinished::At(Instant::now() + dur);
            }
        }
    }
    fn next(&mut self, play: bool, _load_duration: bool) {
        self.current = self.next.take();
        self.set_finished(play);
    }
    fn clear(&mut self) {
        self.current = None;
        self.next = None;
        self.finished = SongFinished::Never;
    }
    fn playing(&self) -> bool {
        match self.finished {
            SongFinished::Never => false,
            SongFinished::In(_) => false,
            SongFinished::At(_) => true,
        }
    }
    fn current_song(&self) -> Option<(SongId, bool, &T)> {
        self.current
            .as_ref()
            .map(|(id, _, custom)| (*id, true, custom))
    }
    fn next_song(&self) -> Option<(SongId, bool, &T)> {
        self.next
            .as_ref()
            .map(|(id, _, custom)| (*id, true, custom))
    }
    fn gen_data_mut(&mut self) -> (Option<&mut T>, Option<&mut T>) {
        (
            self.current.as_mut().map(|(_, _, t)| t),
            self.next.as_mut().map(|(_, _, t)| t),
        )
    }
    fn song_finished_polling(&self) -> bool {
        true
    }
    fn song_finished(&self) -> bool {
        match self.finished {
            SongFinished::Never => true,
            SongFinished::In(dur) => dur <= Duration::ZERO,
            SongFinished::At(time) => time <= Instant::now(),
        }
    }
    fn current_song_duration(&self) -> Option<u64> {
        self.current
            .as_ref()
            .map(|(_, dur, _)| *dur)
            .filter(|dur| *dur > 0)
    }
    fn current_song_playback_position(&self) -> Option<u64> {
        if let Some(duration) = self.current_song_duration() {
            match self.finished {
                SongFinished::Never => None,
                SongFinished::In(dur) => Some(duration.saturating_sub(dur.as_millis() as u64)),
                SongFinished::At(time) => Some(duration.saturating_sub(
                    time.saturating_duration_since(Instant::now()).as_millis() as u64,
                )),
            }
        } else {
            None
        }
    }
}
