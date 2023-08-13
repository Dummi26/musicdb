mod web;

use std::{
    path::PathBuf,
    process::exit,
    sync::{Arc, Mutex},
    thread,
};

use musicdb_lib::server::{run_server, Command};

use musicdb_lib::data::database::Database;

/*

# Exit codes

0 => exited as requested by the user
1 => exit after printing help message
3 => error parsing cli arguments
10 => tried to start with a path that caused some io::Error
11 => tried to start with a path that does not exist (--init prevents this)

*/

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let mut tcp_addr = None;
    let mut web_addr = None;
    let mut lib_dir_for_init = None;
    let database = if let Some(path_s) = args.next() {
        loop {
            if let Some(arg) = args.next() {
                if arg.starts_with("--") {
                    match &arg[2..] {
                        "init" => {
                            if let Some(lib_dir) = args.next() {
                                lib_dir_for_init = Some(lib_dir);
                            } else {
                                eprintln!(
                                    "[EXIT]
missing argument: --init <lib path>"
                                );
                                exit(3);
                            }
                        }
                        "tcp" => {
                            if let Some(addr) = args.next() {
                                if let Ok(addr) = addr.parse() {
                                    tcp_addr = Some(addr)
                                } else {
                                    eprintln!(
                                        "[EXIT]
bad argument: --tcp <addr:port>: couldn't parse <addr:port>"
                                    );
                                    exit(3);
                                }
                            } else {
                                eprintln!(
                                    "[EXIT]
missing argument: --tcp <addr:port>"
                                );
                                exit(3);
                            }
                        }
                        "web" => {
                            if let Some(addr) = args.next() {
                                if let Ok(addr) = addr.parse() {
                                    web_addr = Some(addr)
                                } else {
                                    eprintln!(
                                        "[EXIT]
bad argument: --web <addr:port>: couldn't parse <addr:port>"
                                    );
                                    exit(3);
                                }
                            } else {
                                eprintln!(
                                    "[EXIT]
missing argument: --web <addr:port>"
                                );
                                exit(3);
                            }
                        }
                        o => {
                            eprintln!(
                                "[EXIT]
Unknown long argument --{o}"
                            );
                            exit(3);
                        }
                    }
                } else if arg.starts_with("-") {
                    match &arg[1..] {
                        o => {
                            eprintln!(
                                "[EXIT]
Unknown short argument -{o}"
                            );
                            exit(3);
                        }
                    }
                } else {
                    eprintln!(
                        "[EXIT]
Argument didn't start with - or -- ({arg})."
                    );
                    exit(3);
                }
            } else {
                break;
            }
        }
        let path = PathBuf::from(&path_s);
        match path.try_exists() {
            Ok(exists) => {
                if let Some(lib_directory) = lib_dir_for_init {
                    Database::new_empty(path, lib_directory.into())
                } else if exists {
                    Database::load_database(path).unwrap()
                } else {
                    eprintln!(
                        "[EXIT]
The provided path does not exist."
                    );
                    exit(11);
                }
            }
            Err(e) => {
                eprintln!(
                    "[EXIT]
Error getting information about the provided path '{path_s}': {e}"
                );
                exit(10);
            }
        }
    } else {
        eprintln!(
            "[EXIT]
musicdb - help
musicdb <path to database file> <options> <options> <...>
options:
  --init <lib directory>
  --tcp <addr:port>
  --web <addr:port>
this help was shown because no arguments were provided."
        );
        exit(1);
    };
    // database.add_song_new(Song::new(
    //     "Amaranthe/Manifest/02 Make It Better.mp3".into(),
    //     "Make It Better".to_owned(),
    //     None,
    //     None,
    //     vec![],
    //     None,
    // ));
    // let mut player = Player::new();
    // eprintln!("[info] database.songs: {:?}", database.songs());
    // database.save_database(Some("/tmp/dbfile".into())).unwrap();
    // eprintln!("{}", database.get_song(&0).unwrap());
    // database.queue.add_to_end(QueueContent::Song(1).into());
    // player.update_and_restart_playing_song(&database);
    let database = Arc::new(Mutex::new(database));
    if tcp_addr.is_some() || web_addr.is_some() {
        if let Some(addr) = web_addr {
            let (s, mut r) = tokio::sync::mpsc::channel(2);
            let db = Arc::clone(&database);
            thread::spawn(move || run_server(database, tcp_addr, Some(s)));
            if let Some(sender) = r.recv().await {
                web::main(db, sender, addr).await;
            }
        } else {
            run_server(database, tcp_addr, None);
        }
    } else {
        eprintln!("nothing to do, not starting the server.");
    }
    // std::io::stdin().read_line(&mut String::new()).unwrap();
    // dbg!(Update::from_bytes(&mut BufReader::new(
    //     TcpStream::connect("127.0.0.1:26314".parse::<SocketAddr>().unwrap()).unwrap()
    // )));
}
