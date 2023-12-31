use std::{
    io::{BufReader, Write},
    net::{SocketAddr, TcpStream},
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
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
    server::{get, Command},
};
use speedy2d::color::Color;
#[cfg(feature = "speedy2d")]
mod gui;
#[cfg(feature = "speedy2d")]
mod gui_anim;
#[cfg(feature = "speedy2d")]
mod gui_base;
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
}

fn get_config_file_path() -> PathBuf {
    directories::ProjectDirs::from("", "", "musicdb-client")
        .unwrap()
        .config_dir()
        .to_path_buf()
}

fn main() {
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
    let con_thread = {
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
                        player.update(&mut db);
                    }
                }
                let update = Command::from_bytes(&mut con).unwrap();
                #[cfg(feature = "playback")]
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
        #[cfg(feature = "speedy2d")]
        Mode::Gui => {
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
        #[cfg(feature = "playback")]
        Mode::SyncplayerLocal { .. } | Mode::SyncplayerNetwork => {
            con_thread.join().unwrap();
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

pub(crate) fn color_scale(c: Color, r: f32, g: f32, b: f32, new_alpha: Option<f32>) -> Color {
    Color::from_rgba(c.r() * r, c.g() * g, c.b() * b, new_alpha.unwrap_or(c.a()))
}
