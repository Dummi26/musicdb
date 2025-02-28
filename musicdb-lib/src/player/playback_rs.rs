use std::{ffi::OsStr, io::Cursor, path::Path, sync::Arc, time::Duration};

use playback_rs::Hint;

use crate::{
    data::{song::Song, SongId},
    server::{Action, Command},
};

use super::PlayerBackend;

pub struct PlayerBackendPlaybackRs<T> {
    player: playback_rs::Player,
    current: Option<(SongId, Option<playback_rs::Song>, T)>,
    next: Option<(SongId, Option<playback_rs::Song>, T)>,
    command_sender: Option<std::sync::mpsc::Sender<(Command, Option<u64>)>>,
}

impl<T> PlayerBackendPlaybackRs<T> {
    pub fn new(
        command_sender: std::sync::mpsc::Sender<(Command, Option<u64>)>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Self::new_with_optional_command_sending(Some(command_sender))
    }
    pub fn new_without_command_sending() -> Result<Self, Box<dyn std::error::Error>> {
        Self::new_with_optional_command_sending(None)
    }
    pub fn new_with_optional_command_sending(
        command_sender: Option<std::sync::mpsc::Sender<(Command, Option<u64>)>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            player: playback_rs::Player::new(None)?,
            current: None,
            next: None,
            command_sender,
        })
    }
}

impl<T> PlayerBackend<T> for PlayerBackendPlaybackRs<T> {
    fn load_next_song(
        &mut self,
        id: SongId,
        _song: &Song,
        filename: &OsStr,
        bytes: Arc<Vec<u8>>,
        _load_duration: bool,
        custom_data: T,
    ) {
        let mut hint = Hint::new();
        if let Some(ext) = Path::new(filename).extension().and_then(OsStr::to_str) {
            hint.with_extension(ext);
        }
        let reader = Box::new(Cursor::new(ArcVec(bytes)));
        let loaded_song = match playback_rs::Song::new(reader, &hint, None) {
            Ok(v) => Some(v),
            Err(e) => {
                if let Some(s) = &self.command_sender {
                    s.send((
                        Action::ErrorInfo(
                            format!("Couldn't decode song #{id}!"),
                            format!("Error: {e}"),
                        )
                        .cmd(0xFFu8),
                        None,
                    ))
                    .unwrap();
                }
                None
            }
        };
        // if let Some(song) = &loaded_song {
        //     if self.player.has_current_song() {
        //         if let Err(e) = self.player.play_song_next(song, None) {
        //             if let Some(s) = &self.command_sender {
        //                 s.send(Command::ErrorInfo(
        //                     format!("Couldn't preload song #{id}!"),
        //                     format!("Error: {e}"),
        //                 ))
        //                 .unwrap();
        //             }
        //         }
        //     }
        // }
        self.next = Some((id, loaded_song, custom_data));
    }
    fn pause(&mut self) {
        self.player.set_playing(false);
    }
    fn stop(&mut self) {
        self.pause();
        self.player.seek(Duration::ZERO);
    }
    fn resume(&mut self) {
        self.player.set_playing(true);
    }
    fn next(&mut self, play: bool, _load_duration: bool) {
        self.pause();
        self.player.stop();
        self.player.skip();
        self.current = self.next.take();
        if let Some((id, song, _)) = &self.current {
            if let Some(song) = song {
                if let Err(e) = self.player.play_song_now(song, None) {
                    if let Some(s) = &self.command_sender {
                        s.send((
                            Action::ErrorInfo(
                                format!("Couldn't play song #{id}!"),
                                format!("Error: {e}"),
                            )
                            .cmd(0xFFu8),
                            None,
                        ))
                        .unwrap();
                        s.send((Action::NextSong.cmd(0xFFu8), None)).unwrap();
                    }
                } else {
                    self.player.set_playing(play);
                }
            } else if let Some(s) = &self.command_sender {
                s.send((Action::NextSong.cmd(0xFFu8), None)).unwrap();
            }
        }
    }
    fn clear(&mut self) {
        // remove next song
        let _ = self.player.force_remove_next_song();
        // remove current song
        let _ = self.player.force_remove_next_song();
        self.current = None;
        self.next = None;
    }
    fn playing(&self) -> bool {
        self.player.is_playing()
    }
    fn current_song(&self) -> Option<(SongId, bool, &T)> {
        self.current.as_ref().map(|v| (v.0, true, &v.2))
    }
    fn next_song(&self) -> Option<(SongId, bool, &T)> {
        self.next.as_ref().map(|v| (v.0, true, &v.2))
    }
    fn gen_data_mut(&mut self) -> (Option<&mut T>, Option<&mut T>) {
        (
            self.current.as_mut().map(|v| &mut v.2),
            self.next.as_mut().map(|v| &mut v.2),
        )
    }
    fn song_finished_polling(&self) -> bool {
        true
    }
    fn song_finished(&self) -> bool {
        self.current.is_some() && !self.player.has_current_song()
    }
    fn current_song_duration(&self) -> Option<u64> {
        self.player
            .get_playback_position()
            .map(|v| v.1.as_millis() as _)
    }
    fn current_song_playback_position(&self) -> Option<u64> {
        self.player
            .get_playback_position()
            .map(|v| v.0.as_millis() as _)
    }
}

pub struct ArcVec(pub Arc<Vec<u8>>);
impl AsRef<[u8]> for ArcVec {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref().as_ref()
    }
}
