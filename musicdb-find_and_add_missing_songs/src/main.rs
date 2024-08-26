use std::{
    io::{BufReader, Write},
    net::{SocketAddr, TcpStream},
};

use clap::{Parser, Subcommand};
use musicdb_lib::{
    data::database::{ClientIo, Database},
    load::ToFromBytes,
    server::Command,
};

#[derive(Parser)]
struct Args {
    addr: SocketAddr,
    #[clap(subcommand)]
    action: Action,
}
#[derive(Subcommand)]
enum Action {
    ListChangedSongs,
    FindUnusedSongFiles,
}

fn main() {
    let args = Args::parse();
    match args.action {
        Action::ListChangedSongs => {
            let addr = &args.addr;
            eprintln!("Address: {addr}, connecting...");
            let mut db_con = TcpStream::connect(addr).unwrap();
            writeln!(db_con, "main").unwrap();
            let client_con: Box<dyn ClientIo> = Box::new(TcpStream::connect(addr).unwrap());
            let mut client =
                musicdb_lib::server::get::Client::new(BufReader::new(client_con)).unwrap();
            let mut db = Database::new_clientside();
            eprint!("Loading");
            let _ = std::io::stderr().flush();
            loop {
                eprint!(".");
                let _ = std::io::stderr().flush();
                db.apply_command(Command::from_bytes(&mut db_con).unwrap());
                if db.is_client_init() {
                    eprintln!(" done");
                    break;
                }
            }
            eprintln!("Asking server to search for changed songs.");
            eprintln!(
                "Depending on the size of your library and your hardware, this may take a while."
            );
            let (songs_no_time, songs_new_time, songs_removed, songs_error) =
                client.find_songs_with_changed_files().unwrap().unwrap();
            eprintln!("-------------------------");
            if !songs_no_time.is_empty() {
                eprintln!(
                    "Songs with no last-modified time ({}):",
                    songs_no_time.len()
                );
                for song in songs_no_time {
                    if let Some(song) = db.get_song(&song) {
                        eprintln!("-{song}: {:?}", song.location.rel_path)
                    } else {
                        eprintln!("-{song}!")
                    }
                }
            }
            if !songs_new_time.is_empty() {
                eprintln!(
                    "Songs with a different last-modified time ({}):",
                    songs_new_time.len()
                );
                for (song, new_time) in songs_new_time {
                    if let Some(song) = db.get_song(&song) {
                        if let Some(old_time) = song.file_last_modified_unix_timestamp {
                            eprintln!(
                                "-{song}: {:?} : {old_time}->{new_time}",
                                song.location.rel_path
                            )
                        } else {
                            eprintln!("-{song}: {:?} : !->{new_time}", song.location.rel_path)
                        }
                    } else {
                        eprintln!("-{song}!")
                    }
                }
            }
            if !songs_removed.is_empty() {
                eprintln!(
                    "Songs with locations that do not exist ({}):",
                    songs_removed.len()
                );
                for song in songs_removed {
                    if let Some(song) = db.get_song(&song) {
                        eprintln!("-{song}: {:?}", song.location.rel_path)
                    } else {
                        eprintln!("-{song}!")
                    }
                }
            }
            if !songs_error.is_empty() {
                eprintln!(
                    "Songs with a different last-modified time ({}):",
                    songs_error.len()
                );
                for (song, error) in songs_error {
                    if let Some(song) = db.get_song(&song) {
                        eprintln!("-{song}: {:?} : {error}", song.location.rel_path)
                    } else {
                        eprintln!("-{song}!")
                    }
                }
            }
        }
        Action::FindUnusedSongFiles => {
            let addr = &args.addr;
            eprintln!("Address: {addr}, connecting...");
            let client_con: Box<dyn ClientIo> = Box::new(TcpStream::connect(addr).unwrap());
            eprintln!("Connected. Initializing...");
            let mut client =
                musicdb_lib::server::get::Client::new(BufReader::new(client_con)).unwrap();
            eprintln!("Asking server to search for unused song files in the library.");
            eprintln!(
                "Depending on the size of your library and your hardware, this may take a while."
            );
            let unused = client
                .find_unused_song_files(Some(&[".mp3", ".wma"]))
                .unwrap()
                .unwrap();
            eprintln!("-------------------------");
            for (i, (unused, bad_path)) in unused.iter().enumerate() {
                if *bad_path {
                    println!(
                        "(bad path): {unused} (original path contained newlines or wasn't unicode)"
                    );
                } else {
                    println!("#{i}: {unused}");
                }
            }
        }
    }
}
