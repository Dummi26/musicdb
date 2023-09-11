use std::{
    eprintln, fs,
    io::{BufReader, Write},
    net::{SocketAddr, TcpStream},
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use gui::GuiEvent;
use musicdb_lib::{
    data::{
        album::Album,
        artist::Artist,
        database::{ClientIo, Cover, Database},
        queue::QueueContent,
        song::Song,
        CoverId, DatabaseLocation, GeneralData, SongId,
    },
    load::ToFromBytes,
    player::Player,
    server::{get, Command},
};
#[cfg(feature = "speedy2d")]
mod gui;
#[cfg(feature = "speedy2d")]
mod gui_base;
#[cfg(feature = "speedy2d")]
mod gui_edit;
#[cfg(feature = "speedy2d")]
mod gui_library;
#[cfg(feature = "speedy2d")]
mod gui_playback;
#[cfg(feature = "speedy2d")]
mod gui_queue;
#[cfg(feature = "speedy2d")]
mod gui_screen;
#[cfg(feature = "speedy2d")]
mod gui_settings;
#[cfg(feature = "speedy2d")]
mod gui_text;
#[cfg(feature = "speedy2d")]
mod gui_wrappers;

#[derive(Clone, Copy)]
enum Mode {
    Cli,
    Gui,
    SyncPlayer,
    SyncPlayerWithoutData,
}

fn get_config_file_path() -> PathBuf {
    directories::ProjectDirs::from("", "", "musicdb-client")
        .unwrap()
        .config_dir()
        .to_path_buf()
    // if let Ok(config_home) = std::env::var("XDG_CONFIG_HOME") {
    //     let mut config_home: PathBuf = config_home.into();
    //     config_home.push("musicdb-client");
    //     config_home
    // } else if let Ok(home) = std::env::var("HOME") {
    //     let mut config_home: PathBuf = home.into();
    //     config_home.push(".config");
    //     config_home.push("musicdb-client");
    //     config_home
    // } else {
    //     eprintln!("No config directory!");
    //     std::process::exit(24);
    // }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mode = match args.next().as_ref().map(|v| v.trim()) {
        Some("cli") => Mode::Cli,
        Some("gui") => Mode::Gui,
        Some("syncplayer") => Mode::SyncPlayer,
        Some("syncplayernd") => Mode::SyncPlayerWithoutData,
        _ => {
            println!("Run with argument <cli/gui/syncplayer/syncplayernd>!");
            return;
        }
    };
    let addr = args.next().unwrap_or("127.0.0.1:26314".to_string());
    let addr = addr.parse::<SocketAddr>().unwrap();
    let mut con = TcpStream::connect(addr).unwrap();
    writeln!(con, "main").unwrap();
    let database = Arc::new(Mutex::new(Database::new_clientside()));
    #[cfg(feature = "speedy2d")]
    let update_gui_sender: Arc<Mutex<Option<speedy2d::window::UserEventSender<GuiEvent>>>> =
        Arc::new(Mutex::new(None));
    #[cfg(feature = "speedy2d")]
    let sender = Arc::clone(&update_gui_sender);
    let con_thread = {
        let database = Arc::clone(&database);
        let mut con = con.try_clone().unwrap();
        // this is all you need to keep the db in sync
        thread::spawn(move || {
            let mut player = if matches!(mode, Mode::SyncPlayer | Mode::SyncPlayerWithoutData) {
                Some(Player::new().unwrap())
            } else {
                None
            };
            if matches!(mode, Mode::SyncPlayerWithoutData) {
                let mut db = database.lock().unwrap();
                let client_con: Box<dyn ClientIo> = Box::new(TcpStream::connect(addr).unwrap());
                db.remote_server_as_song_file_source = Some(Arc::new(Mutex::new(
                    musicdb_lib::server::get::Client::new(BufReader::new(client_con)).unwrap(),
                )));
            };
            loop {
                if let Some(player) = &mut player {
                    let mut db = database.lock().unwrap();
                    if !db.lib_directory.as_os_str().is_empty() {
                        player.update(&mut db);
                    }
                }
                let update = Command::from_bytes(&mut con).unwrap();
                if let Some(player) = &mut player {
                    player.handle_command(&update);
                }
                database.lock().unwrap().apply_command(update);
                #[cfg(feature = "speedy2d")]
                if let Some(v) = &*update_gui_sender.lock().unwrap() {
                    v.send_event(GuiEvent::Refresh).unwrap();
                }
            }
        })
    };
    match mode {
        Mode::Cli => {
            Looper {
                con: &mut con,
                database: &database,
            }
            .cmd_loop();
        }
        Mode::Gui => {
            #[cfg(feature = "speedy2d")]
            {
                let occasional_refresh_sender = Arc::clone(&sender);
                thread::spawn(move || loop {
                    std::thread::sleep(Duration::from_secs(1));
                    if let Some(v) = &*occasional_refresh_sender.lock().unwrap() {
                        v.send_event(GuiEvent::Refresh).unwrap();
                    }
                });
                gui::main(
                    database,
                    con,
                    get::Client::new(BufReader::new(
                        TcpStream::connect(addr).expect("opening get client connection"),
                    ))
                    .expect("initializing get client connection"),
                    sender,
                )
            };
        }
        Mode::SyncPlayer | Mode::SyncPlayerWithoutData => {
            con_thread.join().unwrap();
        }
    }
}
struct Looper<'a> {
    pub con: &'a mut TcpStream,
    pub database: &'a Arc<Mutex<Database>>,
}
impl<'a> Looper<'a> {
    pub fn cmd_loop(&mut self) {
        loop {
            println!();
            let line = self.read_line(" > enter a command (help for help)");
            let line = line.trim();
            match line {
                "resume" => Command::Resume,
                "pause" => Command::Pause,
                "stop" => Command::Stop,
                "next" => Command::NextSong,
                "set-lib-dir" => {
                    let line = self.read_line("Enter the new (absolute) library directory, or leave empty to abort");
                    if !line.is_empty() {
                        Command::SetLibraryDirectory(line.into())
                    } else {
                        continue;
                    }
                },
                "add-song" => {
                    let song = Song {
                        id: 0,
                        location: self.read_line("The songs file is located, relative to the library root, at...").into(),
                        title: self.read_line("The songs title is..."),
                        album: self.read_line_ido("The song is part of the album with the id... (empty for None)"),
                        artist: self.read_line_id("The song is made by the artist with the id..."),
                        more_artists: accumulate(|| self.read_line_ido("The song is made with support by other artist, one of which has the id... (will ask repeatedly; leave empty once done)")),
                        cover: self.read_line_ido("The song should use the cover with the id... (empty for None - will default to album or artist cover, if available)"),
                        general: GeneralData::default(),
                        cached_data: Arc::new(Mutex::new(None)),
                    };
                    println!("You are about to add the following song to the database:");
                    println!("  + {song}");
                    if self.read_line("Are you sure? (type 'yes' to continue)").to_lowercase().trim() == "yes" {
                            Command::AddSong(song)
                    } else {
                        println!("[-] Aborted - no event will be sent to the database.");
                        continue;
                    }
                },
                "update-song" => {
                    let song_id = self.read_line_id("The ID of the song is...");
                    if let Some(mut song) = self.database.lock().unwrap().get_song(&song_id).cloned() {
                        println!("You are now editing the song {song}.");
                        loop {
                            match self.read_line("What do you want to edit? (title/album/artist/location or done)").to_lowercase().trim() {
                                "done" => break,
                                "title" => {
                                    println!("prev: '{}'", song.title);
                                    song.title = self.read_line("");
                                }
                                "album" => {
                                    println!("prev: '{}'", song.album.map_or(String::new(), |v| v.to_string()));
                                    song.album = self.read_line_ido("");
                                }
                                "artist" => {
                                    println!("prev: '{}'", song.artist);
                                    song.artist = self.read_line_id("");
                                }
                                "location" => {
                                    println!("prev: '{:?}'", song.location);
                                    song.location = self.read_line("").into();
                                }
                                _ => println!("[-] must be title/album/artist/location or done"),
                            }
                        }
                        println!("You are about to update the song:");
                        println!("  + {song}");
                        if self.read_line("Are you sure? (type 'yes' to continue)").to_lowercase().trim() == "yes" {
                            Command::ModifySong(song)
                        } else {
                            println!("[-] Aborted - no event will be sent to the database.");
                            continue;
                        }
                    } else {
                        println!("[-] No song with that ID found, aborting.");
                        continue;
                    }
                }
                "queue-clear" => Command::QueueUpdate(vec![], QueueContent::Folder(0, vec![], String::new()).into()),
                "queue-add-to-end" => Command::QueueAdd(vec![], QueueContent::Song(self.read_line_id("The ID of the song that should be added to the end of the queue is...")).into()),
                "save" => Command::Save,
                "status" => {
                    let db = self.database.lock().unwrap();
                    println!("DB contains {} songs:", db.songs().len());
                    for song in db.songs().values() {
                        println!("> [{}]: {}", song.id, song);
                    }
                    println!("Queue: {:?}, then {:?}", db.queue.get_current(), db.queue.get_next());
                    continue;
                }
                "exit" => {
                    println!("<< goodbye");
                    break;
                }
                _ => {
                    println!("Type 'exit' to exit, 'status' to see the db, 'resume', 'pause', 'stop', 'next', 'queue-clear', 'queue-add-to-end', 'add-song', 'add-album', 'add-artist', 'update-song', 'update-album', 'update-artist', 'set-lib-dir', or 'save' to control playback or update the db.");
                    continue;
                }
            }
            .to_bytes(self.con)
            .unwrap();
        }
    }

    pub fn read_line(&mut self, q: &str) -> String {
        loop {
            if !q.is_empty() {
                println!("{q}");
            }
            let mut line = String::new();
            std::io::stdin().read_line(&mut line).unwrap();
            while line.ends_with('\n') || line.ends_with('\r') {
                line.pop();
            }
            if line.trim() == "#" {
                self.cmd_loop();
            } else {
                return line;
            }
        }
    }

    pub fn read_line_id(&mut self, q: &str) -> u64 {
        loop {
            if let Ok(v) = self.read_line(q).trim().parse() {
                return v;
            } else {
                println!("[-] Must be a positive integer.");
            }
        }
    }
    pub fn read_line_ido(&mut self, q: &str) -> Option<u64> {
        loop {
            let line = self.read_line(q);
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            if let Ok(v) = line.parse() {
                return Some(v);
            } else {
                println!("[-] Must be a positive integer or nothing for None.");
            }
        }
    }
}
pub fn accumulate<F: FnMut() -> Option<T>, T>(mut f: F) -> Vec<T> {
    let mut o = vec![];
    loop {
        if let Some(v) = f() {
            o.push(v);
        } else {
            break;
        }
    }
    o
}

fn get_cover(song: SongId, database: &Database) -> Option<CoverId> {
    let song = database.get_song(&song)?;
    if let Some(v) = song.cover {
        Some(v)
    } else {
        database.albums().get(song.album.as_ref()?)?.cover
    }
}
