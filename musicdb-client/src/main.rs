use std::{
    io::{BufReader, Write},
    net::{SocketAddr, TcpStream},
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
};

use clap::{Parser, Subcommand};
#[cfg(feature = "speedy2d")]
use gui::GuiEvent;
#[cfg(feature = "playback")]
use musicdb_lib::player::Player;
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
    addr: SocketAddr,
    /// what to do
    #[command(subcommand)]
    mode: Mode,
}

#[derive(Subcommand, Debug, Clone)]
enum Mode {
    #[cfg(feature = "speedy2d")]
    /// graphical user interface
    Gui,
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
    let addr = args.addr;
    let mut con = TcpStream::connect(addr).unwrap();
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
        let mers_after_db_updated_action = Arc::clone(&mers_after_db_updated_action);
        let mode = mode.clone();
        let database = Arc::clone(&database);
        let mut con = con.try_clone().unwrap();
        // this is all you need to keep the db in sync
        thread::spawn(move || {
            #[cfg(feature = "playback")]
            let mut player =
                if matches!(mode, Mode::SyncplayerLocal { .. } | Mode::SyncplayerNetwork) {
                    Some(Player::new().unwrap().without_sending_commands())
                } else {
                    None
                };
            #[allow(unused_labels)]
            'ifstatementworkaround: {
                // use if+break instead of if-else because we can't #[cfg(feature)] the if statement,
                // since we want the else part to run if the feature is disabled
                #[cfg(feature = "playback")]
                if let Mode::SyncplayerLocal { lib_dir } = mode {
                    let mut db = database.lock().unwrap();
                    db.lib_directory = lib_dir;
                    break 'ifstatementworkaround;
                }
                let mut db = database.lock().unwrap();
                let client_con: Box<dyn ClientIo> = Box::new(TcpStream::connect(addr).unwrap());
                db.remote_server_as_song_file_source = Some(Arc::new(Mutex::new(
                    musicdb_lib::server::get::Client::new(BufReader::new(client_con)).unwrap(),
                )));
            }
            loop {
                #[cfg(feature = "playback")]
                if let Some(player) = &mut player {
                    let mut db = database.lock().unwrap();
                    if db.is_client_init() {
                        // command_sender does nothing. if a song finishes, we don't want to move to the next song, we want to wait for the server to send the NextSong event.
                        player.update(&mut db, &Arc::new(|_| {}));
                    }
                }
                let update = Command::from_bytes(&mut con).unwrap();
                #[cfg(feature = "playback")]
                if let Some(player) = &mut player {
                    player.handle_command(&update);
                }
                #[cfg(any(feature = "mers", feature = "merscfg"))]
                if let Some(action) = &mut *mers_after_db_updated_action.lock().unwrap() {
                    database.lock().unwrap().apply_command(update.clone());
                    action(update);
                } else {
                    database.lock().unwrap().apply_command(update);
                }
                #[cfg(feature = "speedy2d")]
                if let Some(v) = &*update_gui_sender.lock().unwrap() {
                    v.send_event(GuiEvent::Refresh).unwrap();
                }
            }
        })
    };
    match mode {
        #[cfg(feature = "speedy2d")]
        Mode::Gui => {
            {
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
                    musicdb_lib::server::get::Client::new(BufReader::new(
                        TcpStream::connect(addr).expect("opening get client connection"),
                    ))
                    .expect("initializing get client connection"),
                    sender,
                    #[cfg(feature = "merscfg")]
                    &mers_after_db_updated_action,
                )
            };
        }
        #[cfg(feature = "playback")]
        Mode::SyncplayerLocal { .. } | Mode::SyncplayerNetwork => {
            con_thread.join().unwrap();
        }
        #[cfg(feature = "mers")]
        Mode::RunMers { path } => {
            let mut src = mers_lib::prelude_compile::Source::new_from_file(path).unwrap();
            let srca = Arc::new(src.clone());
            let con = Mutex::new(con);
            let (mut i1, mut i2, mut i3) = musicdb_mers::add(
                mers_lib::prelude_compile::Config::new().bundle_std(),
                &database,
                &Arc::new(move |cmd: Command| cmd.to_bytes(&mut *con.lock().unwrap()).unwrap()),
                &mers_after_db_updated_action,
            )
            .infos();
            let program = mers_lib::prelude_compile::parse(&mut src, &srca)
                .unwrap()
                .compile(&mut i1, mers_lib::prelude_compile::CompInfo::default())
                .unwrap();
            match program.check(&mut i3, None) {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("{e}");
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
            program.run(&mut i2);
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
