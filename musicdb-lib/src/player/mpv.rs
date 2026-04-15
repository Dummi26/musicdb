use std::{
    ffi::OsStr,
    io::Write,
    os::unix::net::UnixStream,
    process::Stdio,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use crate::{
    data::{SongId, song::Song},
    server::{Action, Command},
};

use super::PlayerBackend;

const IPC_QUIT: &[u8] = b"{\"command\":[\"quit\"]}\n";
const IPC_PAUSE: &[u8] = b"{\"command\":[\"set_property\",\"pause\",true]}\n";
const IPC_RESUME: &[u8] = b"{\"command\":[\"set_property\",\"pause\",false]}\n";
const IPC_STOP: &[u8] =
    b"{\"command\":[\"set_property\",\"pause\",true]}\n{\"command\":[\"seek\",0,\"absolute\"]}\n";

pub struct PlayerBackendMpv<T> {
    id: (u32, u8),
    current: Option<(SongId, Option<(UnixStream, bool, Arc<AtomicBool>)>, T)>,
    next: Option<(SongId, Option<(UnixStream, bool, Arc<AtomicBool>)>, T)>,
    /// unused, but could be used to do something smarter than polling at some point
    #[allow(unused)]
    command_sender: Option<std::sync::mpsc::Sender<(Command, Option<u64>)>>,
}

impl<T> PlayerBackendMpv<T> {
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
            id: (std::process::id(), 0),
            current: None,
            next: None,
            command_sender,
        })
    }
}

impl<T> PlayerBackend<T> for PlayerBackendMpv<T> {
    fn load_next_song(
        &mut self,
        id: SongId,
        _song: &Song,
        _filename: &OsStr,
        bytes: Arc<Vec<u8>>,
        _load_duration: bool,
        custom_data: T,
    ) {
        if let Some((_, Some((mut ipc, _, quit)), _)) = self.next.take() {
            quit.store(true, Ordering::Release);
            ipc.write_all(IPC_QUIT).ok();
        }
        self.id.1 = 1 + (self.id.1 % 9);
        let ipc_path = format!("/tmp/musicdb-server-mpv-ipc-{:X}-{}", self.id.0, self.id.1);
        match std::process::Command::new("mpv")
            .args(["--no-config", "--no-video", "--pause"])
            .arg(format!("--input-ipc-server={ipc_path}"))
            .args(["--no-terminal", "--cache=yes", "--cache-on-disk=no", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(mut proc) => {
                let quit = Arc::new(AtomicBool::new(false));
                for i in 1..=34 {
                    std::thread::sleep(Duration::from_millis(100 * i));
                    if let Ok(ipc) = UnixStream::connect(&ipc_path) {
                        self.next = Some((id, Some((ipc, false, Arc::clone(&quit))), custom_data));
                        break;
                    }
                }
                if self.next.is_some()
                    && let Some(mut stdin) = proc.stdin.take()
                {
                    let s = self.command_sender.clone();
                    std::thread::spawn(move || {
                        stdin.write_all(&bytes).ok();
                        drop(stdin);
                        match proc.wait() {
                            Ok(status) => {
                                if quit.load(Ordering::Acquire) {
                                    return;
                                }
                                quit.store(true, Ordering::Release);
                                if let Some(s) = &s {
                                    if status.success() {
                                        eprintln!("mpv exited, success");
                                        s.send((Action::NextSong.cmd(0xFFu8), None)).unwrap();
                                    } else {
                                        s.send((
                                            Action::ErrorInfo(
                                                "mpv process crashed!".to_owned(),
                                                format!(
                                                    "Exit code: {}",
                                                    status
                                                        .code()
                                                        .map(|n| n.to_string())
                                                        .unwrap_or("unknown".to_owned())
                                                ),
                                            )
                                            .cmd(0xFFu8),
                                            None,
                                        ))
                                        .unwrap();
                                    }
                                }
                            }
                            Err(e) => {
                                if quit.load(Ordering::Acquire) {
                                    return;
                                }
                                quit.store(true, Ordering::Release);
                                if let Some(s) = &s {
                                    s.send((
                                        Action::ErrorInfo(
                                            "Error waiting for mpv to exit!".to_owned(),
                                            format!("Error: {e}"),
                                        )
                                        .cmd(0xFFu8),
                                        None,
                                    ))
                                    .unwrap();
                                }
                            }
                        }
                    });
                } else {
                    proc.kill().ok();
                    if let Some(s) = &self.command_sender {
                        s.send((
                            Action::ErrorInfo(
                                "Error waiting for mpv to start!".to_owned(),
                                "Could not get process' stdin or could not connect to ipc socket."
                                    .to_owned(),
                            )
                            .cmd(0xFFu8),
                            None,
                        ))
                        .unwrap();
                    }
                }
            }
            Err(e) => {
                if let Some(s) = &self.command_sender {
                    s.send((
                        Action::ErrorInfo(
                            "Error starting mpv process!".to_owned(),
                            format!("Error: {e}"),
                        )
                        .cmd(0xFFu8),
                        None,
                    ))
                    .unwrap();
                }
            }
        }
    }
    fn pause(&mut self) {
        if let Some((_, Some((ipc, playing, _)), _)) = &mut self.current {
            ipc.write_all(IPC_PAUSE).ok();
            ipc.flush().ok();
            *playing = false;
        }
    }
    fn stop(&mut self) {
        if let Some((_, Some((ipc, playing, _)), _)) = &mut self.current {
            ipc.write_all(IPC_STOP).ok();
            ipc.flush().ok();
            *playing = false;
        }
    }
    fn resume(&mut self) {
        if let Some((_, Some((ipc, playing, _)), _)) = &mut self.current {
            ipc.write_all(IPC_RESUME).ok();
            ipc.flush().ok();
            *playing = true;
        }
    }
    fn next(&mut self, play: bool, _load_duration: bool) {
        if let Some((_, Some((mut ipc, _, quit)), _)) = self.current.take() {
            quit.store(true, Ordering::Release);
            ipc.write_all(IPC_QUIT).ok();
        }
        self.current = self.next.take();
        if play {
            self.resume();
        }
    }
    fn clear(&mut self) {
        self.next(false, false);
        self.next(false, false);
    }
    fn playing(&self) -> bool {
        if let Some((_, Some((_, playing, _)), _)) = self.current {
            playing
        } else {
            false
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
        self.command_sender.is_none()
    }
    fn song_finished(&self) -> bool {
        if self.command_sender.is_none()
            && let Some((_, Some((_, _, quit)), _)) = &self.current
        {
            quit.load(Ordering::Relaxed)
        } else {
            false
        }
    }
    fn current_song_duration(&self) -> Option<u64> {
        None
    }
    fn current_song_playback_position(&self) -> Option<u64> {
        None
    }
}
