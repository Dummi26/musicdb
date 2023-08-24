use std::{
    collections::{HashMap, HashSet},
    fs::{self, FileType},
    io::Write,
    ops::IndexMut,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use id3::TagLike;
use musicdb_lib::data::{
    album::{self, Album},
    artist::Artist,
    database::{Cover, Database},
    song::Song,
    DatabaseLocation, GeneralData,
};

fn main() {
    // arg parsing
    let lib_dir = if let Some(arg) = std::env::args().nth(1) {
        arg
    } else {
        eprintln!("usage: musicdb-filldb <library root>");
        std::process::exit(1);
    };
    eprintln!("Library: {lib_dir}. press enter to start. result will be saved in 'dbfile'.");
    std::io::stdin().read_line(&mut String::new()).unwrap();
    // start
    eprintln!("finding files...");
    let files = get_all_files_in_dir(&lib_dir);
    let files_count = files.len();
    eprintln!("found {files_count} files, reading metadata...");
    let mut songs = Vec::new();
    for (i, file) in files.into_iter().enumerate() {
        let mut newline = OnceNewline::new();
        eprint!("\r{}/{}", i + 1, files_count);
        _ = std::io::stderr().flush();
        if let Some("mp3") = file.extension().and_then(|ext_os| ext_os.to_str()) {
            match id3::Tag::read_from_path(&file) {
                Err(e) => {
                    newline.now();
                    eprintln!("[{file:?}] error reading id3 tag: {e}");
                }
                Ok(tag) => songs.push((file, tag)),
            }
        }
    }
    eprintln!("\nloaded metadata of {} files.", songs.len());
    let mut database = Database::new_empty(PathBuf::from("dbfile"), PathBuf::from(&lib_dir));
    eprintln!("searching for artists...");
    let mut artists = HashMap::new();
    for song in songs {
        let (artist_id, album_id) =
            if let Some(artist) = song.1.album_artist().or_else(|| song.1.artist()) {
                let artist_id = if !artists.contains_key(artist) {
                    let artist_id = database.add_artist_new(Artist {
                        id: 0,
                        name: artist.to_string(),
                        cover: None,
                        albums: vec![],
                        singles: vec![],
                        general: GeneralData::default(),
                    });
                    artists.insert(artist.to_string(), (artist_id, HashMap::new()));
                    eprintln!("Artist #{artist_id}: {artist}");
                    artist_id
                } else {
                    artists.get(artist).unwrap().0
                };
                if let Some(album) = song.1.album() {
                    let (_, albums) = artists.get_mut(artist).unwrap();
                    let album_id = if !albums.contains_key(album) {
                        let album_id = database.add_album_new(Album {
                            id: 0,
                            artist: Some(artist_id),
                            name: album.to_string(),
                            cover: None,
                            songs: vec![],
                            general: GeneralData::default(),
                        });
                        albums.insert(
                            album.to_string(),
                            (album_id, song.0.parent().map(|dir| dir.to_path_buf())),
                        );
                        eprintln!("Album #{album_id}: {album}");
                        album_id
                    } else {
                        let album = albums.get_mut(album).unwrap();
                        if album
                            .1
                            .as_ref()
                            .is_some_and(|dir| Some(dir.as_path()) != song.0.parent())
                        {
                            // album directory is inconsistent
                            album.1 = None;
                        }
                        album.0
                    };
                    (Some(artist_id), Some(album_id))
                } else {
                    (Some(artist_id), None)
                }
            } else {
                (None, None)
            };
        let path = song.0.strip_prefix(&lib_dir).unwrap();
        let title = song
            .1
            .title()
            .map(|title| title.to_string())
            .unwrap_or_else(|| song.0.file_stem().unwrap().to_string_lossy().into_owned());
        let song_id = database.add_song_new(Song {
            id: 0,
            title: title.clone(),
            location: DatabaseLocation {
                rel_path: path.to_path_buf(),
            },
            album: album_id,
            artist: artist_id,
            more_artists: vec![],
            cover: None,
            general: GeneralData::default(),
            cached_data: Arc::new(Mutex::new(None)),
        });
        eprintln!("Song #{song_id}: \"{title}\" @ {path:?}");
    }
    eprintln!("searching for covers...");
    for (artist, (_artist_id, albums)) in &artists {
        for (album, (album_id, album_dir)) in albums {
            if let Some(album_dir) = album_dir {
                let mut cover = None;
                if let Ok(files) = fs::read_dir(album_dir) {
                    for file in files {
                        if let Ok(file) = file {
                            if let Ok(metadata) = file.metadata() {
                                if metadata.is_file() {
                                    let path = file.path();
                                    if matches!(
                                        path.extension().and_then(|v| v.to_str()),
                                        Some("png" | "jpg" | "jpeg")
                                    ) {
                                        if cover.is_none()
                                            || cover
                                                .as_ref()
                                                .is_some_and(|(_, size)| *size < metadata.len())
                                        {
                                            cover = Some((path, metadata.len()));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if let Some((path, _)) = cover {
                    let rel_path = path.strip_prefix(&lib_dir).unwrap().to_path_buf();
                    let cover_id = database.add_cover_new(Cover {
                        location: DatabaseLocation {
                            rel_path: rel_path.clone(),
                        },
                        data: Arc::new(Mutex::new((false, None))),
                    });
                    eprintln!("Cover #{cover_id}: {artist} - {album} -> {rel_path:?}");
                    database.albums_mut().get_mut(album_id).unwrap().cover = Some(cover_id);
                }
            }
        }
    }
    eprintln!("saving dbfile...");
    database.save_database(None).unwrap();
    eprintln!("done!");
}

fn get_all_files_in_dir(dir: impl AsRef<Path>) -> Vec<PathBuf> {
    let mut files = Vec::new();
    _ = all_files_in_dir(&dir, &mut files);
    files
}
fn all_files_in_dir(dir: impl AsRef<Path>, vec: &mut Vec<PathBuf>) -> Result<(), std::io::Error> {
    for path in fs::read_dir(dir)?
        .filter_map(|possible_entry| possible_entry.ok())
        .map(|entry| entry.path())
    {
        if all_files_in_dir(&path, vec).is_err() {
            vec.push(path);
        }
    }
    Ok(())
}

struct OnceNewline(bool);
impl OnceNewline {
    pub fn new() -> Self {
        Self(true)
    }
    pub fn now(&mut self) {
        if std::mem::replace(&mut self.0, false) {
            eprintln!();
        }
    }
}
