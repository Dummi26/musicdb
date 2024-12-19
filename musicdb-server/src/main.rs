#[cfg(feature = "website")]
mod web;

use std::{
    io::{BufReader, Write},
    net::{SocketAddr, TcpStream},
    path::PathBuf,
    process::exit,
    sync::{Arc, Mutex},
};

use clap::{Parser, Subcommand};
use musicdb_lib::{load::ToFromBytes, server::run_server_caching_thread_opt};

use musicdb_lib::data::database::Database;

#[derive(Parser, Debug)]
struct Args {
    /// optional address for tcp connections to the server
    #[arg(long)]
    tcp: Option<SocketAddr>,
    /// optional address on which to start a website which can be used on devices without `musicdb-client` to control playback.
    /// requires the `assets/` folder to be present!
    #[arg(long)]
    web: Option<SocketAddr>,
    /// play audio instead of acting like a server
    #[arg(long)]
    play_audio: bool,

    /// allow clients to access files in this directory, or the lib_dir if not specified.
    ///
    /// if source=remote and this is not specified, the remote server's files will be used.
    /// if source=remote and a path is specified, the local files will be used.
    /// if source=remote and no path is specified, custom files are disabled even if the remote server has them enabled.
    #[arg(long)]
    custom_files: Option<Option<PathBuf>>,

    /// Use an extra background thread to cache more songs ahead of time. Useful for remote filesystems or very slow disks. If more than this many MiB of system memory are available, cache more songs.
    #[arg(long, value_name = "max_avail_mem_in_mib")]
    advanced_cache: Option<u64>,
    /// Only does something if `--advanced-cache` is used. If available system memory drops below this amount (in MiB), remove songs from cache.
    #[arg(long, value_name = "min_avail_mem_in_mib", default_value_t = 1024)]
    advanced_cache_min_mem: u64,
    /// Only does something if `--advanced-cache` is used. CacheManager will cache the current, next, ..., songs in the queue, but at most this many songs.
    #[arg(long, value_name = "number_of_songs", default_value_t = 10)]
    advanced_cache_song_lookahead_limit: u32,

    // db and song file source
    #[command(subcommand)]
    source: Source,
}
#[derive(Subcommand, Debug)]
enum Source {
    Local {
        /// The directory which contains information about the songs in your library
        #[arg()]
        db_dir: PathBuf,
        /// The path containing your actual library.
        #[arg()]
        lib_dir: PathBuf,
        /// skip reading the dbfile (because it doesn't exist yet)
        #[arg(long)]
        init: bool,
    },
    Remote {
        /// The address of another musicdb-server from where to load the songs
        #[arg()]
        addr: SocketAddr,
    },
}
// struct Args {
//     /// The directory which contains information about the songs in your library
//     #[arg()]
//     db_dir: PathBuf,
//     /// The path containing your actual library.
//     #[arg()]
//     lib_dir: PathBuf,
//     /// skip reading the dbfile (because it doesn't exist yet)
//     #[arg(long)]
//     init: bool,
//     /// optional address for tcp connections to the server
//     #[arg(long)]
//     tcp: Option<SocketAddr>,
//     /// optional address on which to start a website which can be used on devices without `musicdb-client` to control playback.
//     /// requires the `assets/` folder to be present!
//     #[arg(long)]
//     web: Option<SocketAddr>,

//     /// allow clients to access files in this directory, or the lib_dir if not specified.
//     #[arg(long)]
//     custom_files: Option<Option<PathBuf>>,

//     /// Use an extra background thread to cache more songs ahead of time. Useful for remote filesystems or very slow disks. If more than this many MiB of system memory are available, cache more songs.
//     #[arg(long, value_name = "max_avail_mem_in_mib")]
//     advanced_cache: Option<u64>,
//     /// Only does something if `--advanced-cache` is used. If available system memory drops below this amount (in MiB), remove songs from cache.
//     #[arg(long, value_name = "min_avail_mem_in_mib", default_value_t = 1024)]
//     advanced_cache_min_mem: u64,
//     /// Only does something if `--advanced-cache` is used. CacheManager will cache the current, next, ..., songs in the queue, but at most this many songs.
//     #[arg(long, value_name = "number_of_songs", default_value_t = 10)]
//     advanced_cache_song_lookahead_limit: u32,
// }

fn main() {
    // parse args
    let args = Args::parse();
    let mut remote_source_addr = None;
    let mut database = match args.source {
        Source::Local {
            db_dir,
            lib_dir,
            init,
        } => {
            if init {
                Database::new_empty_in_dir(db_dir, lib_dir)
            } else {
                match Database::load_database_from_dir(db_dir.clone(), lib_dir.clone()) {
                    Ok(db) => db,
                    Err(e) => {
                        eprintln!("Couldn't load database!");
                        eprintln!("  dbfile: {:?}", db_dir);
                        eprintln!("  libdir: {:?}", lib_dir);
                        eprintln!("  err: {}", e);
                        exit(1);
                    }
                }
            }
        }
        Source::Remote { addr } => {
            let mut db = Database::new_clientside();
            db.remote_server_as_song_file_source = Some(Arc::new(Mutex::new(
                musicdb_lib::server::get::Client::new(BufReader::new(Box::new(
                    TcpStream::connect(&addr).unwrap(),
                ) as _))
                .unwrap(),
            )));
            remote_source_addr = Some(addr);
            db
        }
    };
    database.custom_files = args.custom_files;
    // database can be shared by multiple threads using Arc<Mutex<_>>
    let database = Arc::new(Mutex::new(database));
    // thread to communicate with the remote server
    if let Some(addr) = remote_source_addr {
        let database = Arc::clone(&database);
        std::thread::spawn(move || {
            let mut con = TcpStream::connect(addr).unwrap();
            writeln!(con, "main").unwrap();
            loop {
                let mut cmd = musicdb_lib::server::Command::from_bytes(&mut con).unwrap();
                use musicdb_lib::server::Action::*;
                match &cmd.action {
                    // ignore playback and queue commands, and denials
                    Resume | Pause | Stop | NextSong | QueueUpdate(..) | QueueAdd(..)
                    | QueueInsert(..) | QueueRemove(..) | QueueMove(..) | QueueMoveInto(..)
                    | QueueGoto(..) | QueueShuffle(..) | QueueSetShuffle(..)
                    | QueueUnshuffle(..) | Denied(..) => continue,
                    SyncDatabase(..)
                    | AddSong(..)
                    | AddAlbum(..)
                    | AddArtist(..)
                    | AddCover(..)
                    | ModifySong(..)
                    | ModifyAlbum(..)
                    | RemoveSong(..)
                    | RemoveAlbum(..)
                    | RemoveArtist(..)
                    | ModifyArtist(..)
                    | SetSongDuration(..)
                    | TagSongFlagSet(..)
                    | TagSongFlagUnset(..)
                    | TagAlbumFlagSet(..)
                    | TagAlbumFlagUnset(..)
                    | TagArtistFlagSet(..)
                    | TagArtistFlagUnset(..)
                    | TagSongPropertySet(..)
                    | TagSongPropertyUnset(..)
                    | TagAlbumPropertySet(..)
                    | TagAlbumPropertyUnset(..)
                    | TagArtistPropertySet(..)
                    | TagArtistPropertyUnset(..)
                    | InitComplete
                    | Save
                    | ErrorInfo(..) => (),
                }
                cmd.seq = 0xFF;
                database.lock().unwrap().apply_command(cmd, None);
            }
        });
    }
    if args.tcp.is_some() || args.web.is_some() {
        let mem_min = args.advanced_cache_min_mem;
        let cache_limit = args.advanced_cache_song_lookahead_limit;
        let args_tcp = args.tcp;
        let run_server = move |database, sender_sender| {
            run_server_caching_thread_opt(
                database,
                args_tcp,
                sender_sender,
                args.advanced_cache.map(|max| {
                    Box::new(
                        move |cm: &mut musicdb_lib::data::cache_manager::CacheManager| {
                            cm.set_memory_mib(mem_min, max.max(mem_min + 128));
                            cm.set_cache_songs_count(cache_limit);
                        },
                    ) as _
                }),
                args.play_audio,
            );
        };
        if let Some(addr) = &args.web {
            #[cfg(not(feature = "website"))]
            {
                let _ = addr;
                eprintln!("Website support requires the 'website' feature to be enabled when compiling the server!");
                std::process::exit(80);
            }
            #[cfg(feature = "website")]
            {
                let (s, r) = std::sync::mpsc::sync_channel(1);
                let db = Arc::clone(&database);
                std::thread::spawn(move || {
                    run_server(database, Some(Box::new(move |c| s.send(c).unwrap())))
                });
                let sender = r.recv().unwrap();
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(web::main(db, sender, *addr));
            }
        } else {
            run_server(database, None);
        }
    } else {
        eprintln!("nothing to do, not starting the server.");
    }
}
