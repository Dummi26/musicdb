// #![allow(unused)]

use std::{
    io::{BufReader, Write},
    net::TcpStream,
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
};

use clap::{Parser, Subcommand};
#[cfg(feature = "speedy2d")]
use gui::GuiEvent;
#[cfg(feature = "playback")]
use musicdb_lib::data::cache_manager::CacheManager;
#[cfg(feature = "playback")]
use musicdb_lib::player::{Player, PlayerBackendFeat};
use musicdb_lib::{
    data::{
        database::{ClientIo, Database},
        CoverId, SongId,
    },
    load::ToFromBytes,
    server::Command,
};
#[cfg(feature = "speedy2d")]
use speedy2d::color::Color;
#[cfg(feature = "speedy2d")]
mod gui;
#[cfg(feature = "speedy2d")]
mod gui_anim;
#[cfg(feature = "speedy2d")]
mod gui_base;
#[cfg(feature = "speedy2d")]
mod gui_edit_song;
#[cfg(feature = "speedy2d")]
mod gui_idle_display;
#[cfg(feature = "speedy2d")]
mod gui_library;
#[cfg(feature = "speedy2d")]
mod gui_notif;
#[cfg(feature = "speedy2d")]
mod gui_playback;
#[cfg(feature = "speedy2d")]
mod gui_playpause;
#[cfg(feature = "speedy2d")]
mod gui_queue;
#[cfg(feature = "speedy2d")]
mod gui_screen;
#[cfg(feature = "speedy2d")]
mod gui_settings;
#[cfg(feature = "speedy2d")]
mod gui_song_adder;
#[cfg(feature = "speedy2d")]
mod gui_statusbar;
#[cfg(feature = "speedy2d")]
mod gui_text;
#[cfg(feature = "speedy2d")]
mod gui_wrappers;
#[cfg(feature = "merscfg")]
mod merscfg;
#[cfg(feature = "speedy2d")]
mod textcfg;

#[derive(Parser, Debug)]
struct Args {
    /// the address to be used for the tcp connection to the server
    addr: String,
    /// what to do
    #[command(subcommand)]
    mode: Mode,
}

#[derive(Subcommand, Debug, Clone)]
enum Mode {
    /// graphical user interface
    #[cfg(feature = "speedy2d")]
    Gui,
    /// graphical user interface + syncplayer using local files (syncplayer-network can be enabled in settings)
    #[cfg(feature = "speedy2d")]
    #[cfg(feature = "playback")]
    GuiSyncplayerLocal { lib_dir: PathBuf },
    /// graphical user interface + syncplayer (syncplayer-network can be toggled in settings)
    #[cfg(feature = "speedy2d")]
    #[cfg(feature = "playback")]
    GuiSyncplayerNetwork,
    /// play in sync with the server, but load the songs from a local copy of the lib-dir
    #[cfg(feature = "playback")]
    SyncplayerLocal { lib_dir: PathBuf },
    /// play in sync with the server, and fetch the songs from it too. slower than the local variant for obvious reasons
    #[cfg(feature = "playback")]
    SyncplayerNetwork,
    #[cfg(feature = "mers")]
    RunMers { path: PathBuf },
}

fn get_config_file_path() -> PathBuf {
    directories::ProjectDirs::from("", "", "musicdb-client")
        .unwrap()
        .config_dir()
        .to_path_buf()
}

fn main() {
    #[cfg(not(feature = "speedy2d"))]
    #[cfg(not(feature = "mers"))]
    #[cfg(not(feature = "playback"))]
    compile_error!("None of the optional features are enabled. Without at least one of these, the application is useless! See Cargo.toml for info.");
    // parse args
    let args = Args::parse();
    // start
    let addr = args.addr.clone();
    let mut con = TcpStream::connect(&addr).unwrap();
    let mode = args.mode;
    writeln!(con, "main").unwrap();
    let database = Arc::new(Mutex::new(Database::new_clientside()));
    #[cfg(feature = "speedy2d")]
    let update_gui_sender: Arc<Mutex<Option<speedy2d::window::UserEventSender<GuiEvent>>>> =
        Arc::new(Mutex::new(None));
    #[cfg(feature = "speedy2d")]
    let sender = Arc::clone(&update_gui_sender);
    #[cfg(any(feature = "mers", feature = "merscfg"))]
    let mers_after_db_updated_action: Arc<
        Mutex<Option<Box<dyn FnMut(Command) + Send + Sync + 'static>>>,
    > = Arc::new(Mutex::new(None));
    let con_thread = {
        #[cfg(any(feature = "mers", feature = "merscfg"))]
        let mers_after_db_updated_action = Arc::clone(&mers_after_db_updated_action);
        let mode = mode.clone();
        let database = Arc::clone(&database);
        let mut con = con.try_clone().unwrap();
        // this is all you need to keep the db in sync
        let addr = addr.clone();
        thread::spawn(move || {
            #[cfg(feature = "playback")]
            #[cfg(not(feature = "speedy2d"))]
            let is_syncplayer =
                matches!(mode, Mode::SyncplayerLocal { .. } | Mode::SyncplayerNetwork);
            #[cfg(feature = "playback")]
            #[cfg(feature = "speedy2d")]
            let is_syncplayer = matches!(
                mode,
                Mode::SyncplayerLocal { .. }
                    | Mode::SyncplayerNetwork
                    | Mode::GuiSyncplayerLocal { .. }
                    | Mode::GuiSyncplayerNetwork
            );
            #[cfg(feature = "playback")]
            #[allow(unused)]
            let mut cache_manager = None;
            #[cfg(feature = "playback")]
            let mut player = if is_syncplayer {
                let cm = CacheManager::new(Arc::clone(&database));
                cm.set_memory_mib(1024, 2048);
                cm.set_cache_songs_count(20);
                cache_manager = Some(cm);
                Some(Player::new_client(
                    PlayerBackendFeat::new_without_command_sending().unwrap(),
                ))
            } else {
                None
            };
            #[allow(unused_labels)]
            'ifstatementworkaround: {
                // use if+break instead of if-else because we can't #[cfg(feature)] the if statement,
                // since we want the else part to run if the feature is disabled
                #[cfg(feature = "playback")]
                let lib_dir = match &mode {
                    Mode::SyncplayerLocal { lib_dir } => Some(lib_dir.clone()),
                    #[cfg(feature = "speedy2d")]
                    Mode::GuiSyncplayerLocal { lib_dir } => Some(lib_dir.clone()),
                    _ => None,
                };
                #[cfg(feature = "playback")]
                if let Some(lib_dir) = lib_dir {
                    let mut db = database.lock().unwrap();
                    db.lib_directory = lib_dir;
                    break 'ifstatementworkaround;
                }
                #[cfg(feature = "speedy2d")]
                if matches!(mode, Mode::Gui) {
                    // gui does this in the main thread
                    break 'ifstatementworkaround;
                }
                let mut db = database.lock().unwrap();
                let client_con: Box<dyn ClientIo> = Box::new(TcpStream::connect(&addr).unwrap());
                db.remote_server_as_song_file_source = Some(Arc::new(Mutex::new(
                    musicdb_lib::server::get::Client::new(BufReader::new(client_con)).unwrap(),
                )));
            }
            loop {
                let command = Command::from_bytes(&mut con).unwrap();
                let mut db = database.lock().unwrap();
                let action = db.seq.recv(command);
                #[cfg(feature = "playback")]
                if let Some(player) = &mut player {
                    player.handle_action(&action);
                }
                #[allow(unused_labels)]
                'feature_if: {
                    #[cfg(any(feature = "mers", feature = "merscfg"))]
                    if let Some(action) = &mut *mers_after_db_updated_action.lock().unwrap() {
                        db.apply_command(action.clone());
                        action(action);
                        break 'feature_if;
                    }
                    db.apply_action_unchecked_seq(action, None);
                }
                #[cfg(feature = "playback")]
                if let Some(player) = &mut player {
                    player.update_dont_uncache(&mut *db);
                }
                drop(db);
                #[cfg(feature = "speedy2d")]
                if let Some(v) = &*update_gui_sender.lock().unwrap() {
                    v.send_event(GuiEvent::Refresh).unwrap();
                }
            }
        })
    };
    macro_rules! gui_modes {
        () => {{
            let get_con: Arc<Mutex<musicdb_lib::server::get::Client<Box<dyn ClientIo + 'static>>>> =
                Arc::new(Mutex::new(
                    musicdb_lib::server::get::Client::new(BufReader::new(Box::new(
                        TcpStream::connect(&addr).expect("opening get client connection"),
                    ) as _))
                    .expect("initializing get client connection"),
                ));
            #[allow(unused_labels)]
            'anotherifstatement: {
                #[cfg(feature = "playback")]
                if let Mode::GuiSyncplayerLocal { lib_dir } = mode {
                    database.lock().unwrap().lib_directory = lib_dir;
                    break 'anotherifstatement;
                }
                #[cfg(feature = "playback")]
                if let Mode::GuiSyncplayerNetwork = mode {
                    break 'anotherifstatement;
                }
                // if not using syncplayer-local
                database.lock().unwrap().remote_server_as_song_file_source =
                    Some(Arc::clone(&get_con));
            }
            let occasional_refresh_sender = Arc::clone(&sender);
            thread::spawn(move || loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
                if let Some(v) = &*occasional_refresh_sender.lock().unwrap() {
                    v.send_event(GuiEvent::Refresh).unwrap();
                }
            });
            gui::main(
                database,
                con,
                get_con,
                sender,
                #[cfg(feature = "merscfg")]
                &mers_after_db_updated_action,
            )
        }};
    }
    match mode {
        #[cfg(feature = "speedy2d")]
        #[cfg(feature = "playback")]
        Mode::Gui | Mode::GuiSyncplayerLocal { .. } | Mode::GuiSyncplayerNetwork => gui_modes!(),
        #[cfg(feature = "speedy2d")]
        #[cfg(not(feature = "playback"))]
        Mode::Gui => gui_modes!(),
        #[cfg(feature = "playback")]
        Mode::SyncplayerLocal { .. } | Mode::SyncplayerNetwork => {
            con_thread.join().unwrap();
        }
        #[cfg(feature = "mers")]
        Mode::RunMers { path } => {
            let mut src =
                musicdb_mers::mers_lib::prelude_compile::Source::new_from_file(path).unwrap();
            let srca = Arc::new(src.clone());
            let con = Mutex::new(con);
            let (mut i1, mut i2, mut i3) = musicdb_mers::add(
                musicdb_mers::mers_lib::prelude_compile::Config::new().bundle_std(),
                &database,
                &Arc::new(move |cmd: Command| cmd.to_bytes(&mut *con.lock().unwrap()).unwrap()),
                &mers_after_db_updated_action,
            )
            .infos();
            let program = match musicdb_mers::mers_lib::prelude_compile::parse(&mut src, &srca) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("{}", e.display_term());
                    std::process::exit(60);
                }
            };
            let program = match program.compile(&mut i1, Default::default()) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("{}", e.display_term());
                    std::process::exit(60);
                }
            };
            match program.check(&mut i3, None) {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("{}", e.display_term());
                    std::process::exit(60);
                }
            };
            // wait until db is synced
            let dur = std::time::Duration::from_secs_f32(0.1);
            loop {
                std::thread::sleep(dur);
                if database.lock().unwrap().is_client_init() {
                    break;
                }
            }
            if let Err(e) = program.run(&mut i2) {
                eprintln!("{}", e.display_term());
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

#[cfg(feature = "speedy2d")]
pub(crate) fn color_scale(c: Color, r: f32, g: f32, b: f32, new_alpha: Option<f32>) -> Color {
    Color::from_rgba(c.r() * r, c.g() * g, c.b() * b, new_alpha.unwrap_or(c.a()))
}
