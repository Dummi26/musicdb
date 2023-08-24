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
        database::{Cover, Database},
        queue::QueueContent,
        song::Song,
        DatabaseLocation, GeneralData,
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

enum Mode {
    Cli,
    Gui,
    SyncPlayer,
    FillDb,
}

fn get_config_file_path() -> PathBuf {
    if let Ok(config_home) = std::env::var("XDG_CONFIG_HOME") {
        let mut config_home: PathBuf = config_home.into();
        config_home.push("musicdb-client");
        config_home
    } else if let Ok(home) = std::env::var("HOME") {
        let mut config_home: PathBuf = home.into();
        config_home.push(".config");
        config_home.push("musicdb-client");
        config_home
    } else {
        eprintln!("No config directory!");
        std::process::exit(24);
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mode = match args.next().as_ref().map(|v| v.trim()) {
        Some("cli") => Mode::Cli,
        Some("gui") => Mode::Gui,
        Some("syncplayer") => Mode::SyncPlayer,
        Some("filldb") => Mode::FillDb,
        _ => {
            println!("Run with argument <cli/gui/syncplayer/filldb>!");
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
    let wants_player = matches!(mode, Mode::SyncPlayer);
    let con_thread = {
        let database = Arc::clone(&database);
        let mut con = con.try_clone().unwrap();
        // this is all you need to keep the db in sync
        thread::spawn(move || {
            let mut player = if wants_player {
                Some(Player::new().unwrap())
            } else {
                None
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
        Mode::SyncPlayer => {
            con_thread.join().unwrap();
        }
        Mode::FillDb => {
            // wait for init
            let dir = loop {
                let db = database.lock().unwrap();
                if !db.lib_directory.as_os_str().is_empty() {
                    break db.lib_directory.clone();
                }
                drop(db);
                std::thread::sleep(Duration::from_millis(300));
            };
            eprintln!("
  WARN: This will add all audio files in the lib-dir to the library, even if they were already added!
        lib-dir: {:?}
        If you really want to continue, type Yes.", dir);
            let mut line = String::new();
            std::io::stdin().read_line(&mut line).unwrap();
            if line.trim().to_lowercase() == "yes" {
                let mut covers = 0;
                for artist in fs::read_dir(&dir)
                    .expect("reading lib-dir")
                    .filter_map(|v| v.ok())
                {
                    if let Ok(albums) = fs::read_dir(artist.path()) {
                        let artist_name = artist.file_name().to_string_lossy().into_owned();
                        let mut artist_id = None;
                        for album in albums.filter_map(|v| v.ok()) {
                            if let Ok(songs) = fs::read_dir(album.path()) {
                                let album_name = album.file_name().to_string_lossy().into_owned();
                                let mut album_id = None;
                                let mut songs: Vec<_> = songs.filter_map(|v| v.ok()).collect();
                                songs.sort_unstable_by_key(|v| v.file_name());
                                let cover = songs.iter().map(|entry| entry.path()).find(|path| {
                                    path.extension().is_some_and(|ext| {
                                        ext.to_str().is_some_and(|ext| {
                                            matches!(
                                                ext.to_lowercase().trim(),
                                                "png" | "jpg" | "jpeg"
                                            )
                                        })
                                    })
                                });
                                for song in songs {
                                    match song.path().extension().map(|v| v.to_str()) {
                                        Some(Some(
                                            "mp3" | "wav" | "wma" | "aac" | "flac" | "m4a" | "m4p"
                                            | "ogg" | "oga" | "mogg" | "opus" | "tta",
                                        )) => {
                                            println!("> {:?}", song.path());
                                            let song_name =
                                                song.file_name().to_string_lossy().into_owned();
                                            println!(
                                                "  {}  -  {}  -  {}",
                                                song_name, artist_name, album_name
                                            );
                                            // get artist id
                                            let artist_id = if let Some(v) = artist_id {
                                                v
                                            } else {
                                                let mut adding_artist = false;
                                                loop {
                                                    let db = database.lock().unwrap();
                                                    let artists = db
                                                        .artists()
                                                        .iter()
                                                        .filter(|(_, v)| v.name == artist_name)
                                                        .collect::<Vec<_>>();
                                                    if artists.len() > 1 {
                                                        eprintln!("Choosing the first of {} artists named {}.", artists.len(), artist_name);
                                                    }
                                                    if let Some((id, _)) = artists.first() {
                                                        artist_id = Some(**id);
                                                        break **id;
                                                    } else {
                                                        drop(db);
                                                        if !adding_artist {
                                                            adding_artist = true;
                                                            Command::AddArtist(Artist {
                                                                id: 0,
                                                                name: artist_name.clone(),
                                                                cover: None,
                                                                albums: vec![],
                                                                singles: vec![],
                                                                general: GeneralData::default(),
                                                            })
                                                            .to_bytes(&mut con)
                                                            .expect(
                                                                "sending AddArtist to db failed",
                                                            );
                                                        }
                                                        std::thread::sleep(Duration::from_millis(
                                                            300,
                                                        ));
                                                    };
                                                }
                                            };
                                            // get album id
                                            let album_id = if let Some(v) = album_id {
                                                v
                                            } else {
                                                let mut adding_album = false;
                                                loop {
                                                    let db = database.lock().unwrap();
                                                    let albums = db
                                                        .artists()
                                                        .get(&artist_id)
                                                        .expect("artist_id not valid (bug)")
                                                        .albums
                                                        .iter()
                                                        .filter_map(|v| {
                                                            Some((v, db.albums().get(&v)?))
                                                        })
                                                        .filter(|(_, v)| v.name == album_name)
                                                        .collect::<Vec<_>>();
                                                    if albums.len() > 1 {
                                                        eprintln!("Choosing the first of {} albums named {} by the artist {}.", albums.len(), album_name, artist_name);
                                                    }
                                                    if let Some((id, _)) = albums.first() {
                                                        album_id = Some(**id);
                                                        break **id;
                                                    } else {
                                                        drop(db);
                                                        if !adding_album {
                                                            adding_album = true;
                                                            let cover = if let Some(cover) = &cover
                                                            {
                                                                eprintln!("Adding cover {cover:?}");
                                                                Command::AddCover(Cover {
                                                                    location: DatabaseLocation {
                                                                        rel_path: PathBuf::from(
                                                                            artist.file_name(),
                                                                        )
                                                                        .join(album.file_name())
                                                                        .join(
                                                                            cover
                                                                                .file_name()
                                                                                .unwrap(),
                                                                        ),
                                                                    },
                                                                    data: Arc::new(Mutex::new((
                                                                        false, None,
                                                                    ))),
                                                                })
                                                                .to_bytes(&mut con)
                                                                .expect(
                                                                    "sending AddCover to db failed",
                                                                );
                                                                covers += 1;
                                                                Some(covers - 1)
                                                            } else {
                                                                None
                                                            };
                                                            Command::AddAlbum(Album {
                                                                id: 0,
                                                                name: album_name.clone(),
                                                                artist: Some(artist_id),
                                                                cover,
                                                                songs: vec![],
                                                                general: GeneralData::default(),
                                                            })
                                                            .to_bytes(&mut con)
                                                            .expect("sending AddAlbum to db failed");
                                                        }
                                                        std::thread::sleep(Duration::from_millis(
                                                            300,
                                                        ));
                                                    };
                                                }
                                            };
                                            Command::AddSong(Song::new(
                                                DatabaseLocation {
                                                    rel_path: PathBuf::from(artist.file_name())
                                                        .join(album.file_name())
                                                        .join(song.file_name()),
                                                },
                                                song_name,
                                                Some(album_id),
                                                Some(artist_id),
                                                vec![],
                                                None,
                                            ))
                                            .to_bytes(&mut con)
                                            .expect("sending AddSong to db failed");
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                }
            }
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
                        artist: self.read_line_ido("The song is made by the artist with the id... (empty for None)"),
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
                                    println!("prev: '{}'", song.artist.map_or(String::new(), |v| v.to_string()));
                                    song.artist = self.read_line_ido("");
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
