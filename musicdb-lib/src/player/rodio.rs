use std::{ffi::OsStr, sync::Arc};

use rc_u8_reader::ArcU8Reader;
use rodio::{decoder::DecoderError, Decoder, OutputStream, OutputStreamHandle, Sink, Source};

use crate::{
    data::SongId,
    server::{Action, Command},
};

use super::PlayerBackend;

pub struct PlayerBackendRodio<T> {
    #[allow(unused)]
    output_stream: OutputStream,
    #[allow(unused)]
    output_stream_handle: OutputStreamHandle,
    sink: Sink,
    stopped: bool,
    current: Option<(SongId, Arc<Vec<u8>>, Option<u128>, T)>,
    next: Option<(SongId, Arc<Vec<u8>>, Option<MyDecoder>, T)>,
    command_sender: Option<std::sync::mpsc::Sender<(Command, Option<u64>)>>,
}

impl<T> PlayerBackendRodio<T> {
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
        let (output_stream, output_stream_handle) = rodio::OutputStream::try_default()?;
        let sink = Sink::try_new(&output_stream_handle)?;
        Ok(Self {
            output_stream,
            output_stream_handle,
            sink,
            stopped: true,
            current: None,
            next: None,
            command_sender,
        })
    }
}

impl<T> PlayerBackend<T> for PlayerBackendRodio<T> {
    fn load_next_song(
        &mut self,
        id: SongId,
        _filename: &OsStr,
        bytes: Arc<Vec<u8>>,
        _load_duration: bool,
        custom_data: T,
    ) {
        let decoder = decoder_from_bytes(Arc::clone(&bytes));
        if let Err(e) = &decoder {
            if let Some(s) = &self.command_sender {
                s.send((
                    Action::ErrorInfo(
                        format!("Couldn't decode song #{id}!"),
                        format!("Error: '{e}'"),
                    )
                    .cmd(0xFFu8),
                    None,
                ))
                .unwrap();
            }
        }
        self.next = Some((id, bytes, decoder.ok(), custom_data));
    }
    fn pause(&mut self) {
        self.sink.pause();
    }
    fn stop(&mut self) {
        if !self.stopped {
            self.sink.clear();
            if let Some((_, bytes, _, _)) = &self.current {
                if let Ok(decoder) = decoder_from_bytes(Arc::clone(bytes)) {
                    self.sink.append(decoder);
                }
            }
        }
    }
    fn resume(&mut self) {
        self.stopped = false;
        self.sink.play();
    }
    fn next(&mut self, play: bool, load_duration: bool) {
        self.stopped = false;
        self.sink.clear();
        self.current = self
            .next
            .take()
            .map(|(id, bytes, mut decoder, custom_data)| {
                let duration = if let Some(decoder) = decoder.take() {
                    let duration = if load_duration {
                        dbg!(decoder.total_duration().map(|v| v.as_millis()))
                    } else {
                        None
                    };
                    self.sink.append(decoder);
                    if play {
                        self.sink.play();
                    }
                    duration
                } else {
                    None
                };
                (id, bytes, duration, custom_data)
            });
    }
    fn clear(&mut self) {
        self.sink.clear();
    }
    fn playing(&self) -> bool {
        !(self.sink.is_paused() || self.sink.empty())
    }
    fn current_song(&self) -> Option<(SongId, bool, &T)> {
        self.current.as_ref().map(|(id, _, _, t)| (*id, true, t))
    }
    fn next_song(&self) -> Option<(SongId, bool, &T)> {
        self.next.as_ref().map(|(id, _, _, t)| (*id, true, t))
    }
    fn gen_data_mut(&mut self) -> (Option<&mut T>, Option<&mut T>) {
        (
            self.current.as_mut().map(|(_, _, _, t)| t),
            self.next.as_mut().map(|(_, _, _, t)| t),
        )
    }
    fn song_finished_polling(&self) -> bool {
        true
    }
    fn song_finished(&self) -> bool {
        self.current.is_some() && self.sink.empty()
    }
    fn current_song_duration(&self) -> Option<u64> {
        self.current
            .as_ref()
            .and_then(|(_, _, dur, _)| dur.map(|v| v as _))
    }
    fn current_song_playback_position(&self) -> Option<u64> {
        None
    }
}

type MyDecoder = Decoder<ArcU8Reader<Vec<u8>>>;

fn decoder_from_bytes(bytes: Arc<Vec<u8>>) -> Result<MyDecoder, DecoderError> {
    Decoder::new(ArcU8Reader::new(Arc::clone(&bytes))).map(|decoder| decoder)
}
