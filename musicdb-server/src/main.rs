mod web;

use std::{
    net::SocketAddr,
    path::PathBuf,
    process::exit,
    sync::{Arc, Mutex},
    thread,
};

use clap::Parser;
use musicdb_lib::server::run_server;

use musicdb_lib::data::database::Database;

#[derive(Parser, Debug)]
struct Args {
    /// The file which contains information about the songs in your library
    #[arg()]
    dbfile: PathBuf,
    /// The path containing your actual library.
    #[arg()]
    lib_dir: PathBuf,
    /// skip reading the `dbfile` (because it doesn't exist yet)
    #[arg(long)]
    init: bool,
    /// optional address for tcp connections to the server
    #[arg(long)]
    tcp: Option<SocketAddr>,
    /// optional address on which to start a website which can be used on devices without `musicdb-client` to control playback.
    /// requires the `assets/` folder to be present!
    #[arg(long)]
    web: Option<SocketAddr>,

    /// allow clients to access files in this directory, or the lib_dir if not specified.
    #[arg(long)]
    custom_files: Option<Option<PathBuf>>,
}

#[tokio::main]
async fn main() {
    // parse args
    let args = Args::parse();
    let mut database = if args.init {
        Database::new_empty(args.dbfile, args.lib_dir)
    } else {
        match Database::load_database(args.dbfile.clone(), args.lib_dir.clone()) {
            Ok(db) => db,
            Err(e) => {
                eprintln!("Couldn't load database!");
                eprintln!("  dbfile: {:?}", args.dbfile);
                eprintln!("  libdir: {:?}", args.lib_dir);
                eprintln!("  err: {}", e);
                exit(1);
            }
        }
    };
    database.custom_files = args.custom_files;
    // database can be shared by multiple threads using Arc<Mutex<_>>
    let database = Arc::new(Mutex::new(database));
    if args.tcp.is_some() || args.web.is_some() {
        if let Some(addr) = &args.web {
            let (s, mut r) = tokio::sync::mpsc::channel(2);
            let db = Arc::clone(&database);
            thread::spawn(move || run_server(database, args.tcp, Some(s)));
            if let Some(sender) = r.recv().await {
                web::main(db, sender, *addr).await;
            }
        } else {
            run_server(database, args.tcp, None);
        }
    } else {
        eprintln!("nothing to do, not starting the server.");
    }
}
