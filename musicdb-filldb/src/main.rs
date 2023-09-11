use std::{
    collections::HashMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use id3::TagLike;
use musicdb_lib::data::{
    album::Album,
    artist::Artist,
    database::{Cover, Database},
    song::Song,
    CoverId, DatabaseLocation, GeneralData,
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
    let unknown_artist = database.add_artist_new(Artist {
        id: 0,
        name: format!("<unknown>"),
        cover: None,
        albums: vec![],
        singles: vec![],
        general: GeneralData::default(),
    });
    eprintln!("searching for artists...");
    let mut artists = HashMap::new();
    for song in songs {
        let (artist_id, album_id) = if let Some(artist) = song
            .1
            .album_artist()
            .or_else(|| song.1.artist())
            .filter(|v| !v.trim().is_empty())
        {
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
                artist_id
            } else {
                artists.get(artist).unwrap().0
            };
            if let Some(album) = song.1.album() {
                let (_, albums) = artists.get_mut(artist).unwrap();
                let album_id = if !albums.contains_key(album) {
                    let album_id = database.add_album_new(Album {
                        id: 0,
                        artist: artist_id,
                        name: album.to_string(),
                        cover: None,
                        songs: vec![],
                        general: GeneralData::default(),
                    });
                    albums.insert(
                        album.to_string(),
                        (album_id, song.0.parent().map(|dir| dir.to_path_buf())),
                    );
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
                (artist_id, Some(album_id))
            } else {
                (artist_id, None)
            }
        } else {
            (unknown_artist, None)
        };
        let path = song.0.strip_prefix(&lib_dir).unwrap();
        let title = song
            .1
            .title()
            .map_or(None, |title| {
                if title.trim().is_empty() {
                    None
                } else {
                    Some(title.to_string())
                }
            })
            .unwrap_or_else(|| song.0.file_stem().unwrap().to_string_lossy().into_owned());
        database.add_song_new(Song {
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
    }
    eprintln!("searching for covers...");
    let mut single_images = HashMap::new();
    for (i1, (_artist, (artist_id, albums))) in artists.iter().enumerate() {
        eprint!("\rartist {}/{}", i1 + 1, artists.len());
        for (_album, (album_id, album_dir)) in albums {
            if let Some(album_dir) = album_dir {
                if let Some(cover_id) = get_cover(&mut database, &lib_dir, album_dir) {
                    database.albums_mut().get_mut(album_id).unwrap().cover = Some(cover_id);
                }
            }
        }
        if let Some(artist) = database.artists().get(artist_id) {
            for song in artist.singles.clone() {
                if let Some(dir) = AsRef::<Path>::as_ref(&lib_dir)
                    .join(&database.songs().get(&song).unwrap().location.rel_path)
                    .parent()
                {
                    let cover_id = if let Some(cover_id) = single_images.get(dir) {
                        Some(*cover_id)
                    } else if let Some(cover_id) = get_cover(&mut database, &lib_dir, dir) {
                        single_images.insert(dir.to_owned(), cover_id);
                        Some(cover_id)
                    } else {
                        None
                    };
                    let song = database.songs_mut().get_mut(&song).unwrap();
                    song.cover = cover_id;
                }
            }
        }
    }
    eprintln!();
    if let Some(uka) = database.artists().get(&unknown_artist) {
        if uka.albums.is_empty() && uka.singles.is_empty() {
            database.artists_mut().remove(&unknown_artist);
        } else {
            eprintln!("Added the <unknown> artist as a fallback!");
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

fn get_cover(database: &mut Database, lib_dir: &str, abs_dir: impl AsRef<Path>) -> Option<CoverId> {
    let mut cover = None;
    if let Ok(files) = fs::read_dir(abs_dir) {
        for file in files {
            if let Ok(file) = file {
                if let Ok(metadata) = file.metadata() {
                    if metadata.is_file() {
                        let path = file.path();
                        if path.extension().and_then(|v| v.to_str()).is_some_and(|v| {
                            matches!(v.to_lowercase().as_str(), "png" | "jpg" | "jpeg")
                        }) {
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
        Some(database.add_cover_new(Cover {
            location: DatabaseLocation {
                rel_path: rel_path.clone(),
            },
            data: Arc::new(Mutex::new((false, None))),
        }))
    } else {
        None
    }
}
