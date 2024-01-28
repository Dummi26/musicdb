use std::{
    cmp::Ordering,
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
    song::{CachedData, Song},
    CoverId, DatabaseLocation, GeneralData,
};

fn main() {
    // arg parsing
    let mut args = std::env::args().skip(1);
    let lib_dir = if let Some(arg) = args.next() {
        arg
    } else {
        eprintln!("usage: musicdb-filldb <library root> [--help] [--skip-duration] [--custom-files <path>] [... (see --help)]");
        std::process::exit(1);
    };
    let mut bad_arg = false;
    let mut skip_duration = false;
    let mut custom_files = None;
    let mut artist_txt = false;
    let mut artist_img = false;
    loop {
        match args.next() {
            None => break,
            Some(arg) => match arg.as_str() {
                "--help" => {
                    eprintln!("--skip-duration: Don't try to figure out the songs duration from file contents. This means mp3 files with the Duration field unset will have a duration of 0.");
                    eprintln!("--custom-files <path>: server will use <path> as its custom-files directory.");
                    eprintln!("--cf-artist-txt: For each artist, check for an <artist>.txt file. If it exists, add each line as a tag to that artist.");
                    eprintln!("--cf-artist-img: For each artist, check for an <artist>.{{jpg,png,...}} file. If it exists, add ImageExt=<extension> tag to the artist, so the image can be loaded by clients later.");
                    return;
                }
                "--skip-duration" => skip_duration = true,
                "--custom-files" => {
                    if let Some(path) = args.next() {
                        custom_files = Some(PathBuf::from(path));
                    } else {
                        bad_arg = true;
                        eprintln!("--custom-files <path> :: missing <path>!");
                    }
                }
                "--cf-artist-txt" => artist_txt = true,
                "--cf-artist-img" => artist_img = true,
                arg => {
                    bad_arg = true;
                    eprintln!("Unknown argument: {arg}");
                }
            },
        }
    }
    if bad_arg {
        return;
    }
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
        if let Ok(metadata) = file.metadata() {
            _ = std::io::stderr().flush();
            if let Some("mp3") = file.extension().and_then(|ext_os| ext_os.to_str()) {
                match id3::Tag::read_from_path(&file) {
                    Err(e) => {
                        newline.now();
                        eprintln!("[{file:?}] error reading id3 tag: {e}");
                    }
                    Ok(tag) => songs.push((file, metadata, tag)),
                }
            }
        } else {
            newline.now();
            eprintln!("[err] couldn't get metadata of file {:?}, skipping", file);
        }
    }
    eprintln!("\nloaded metadata of {} files.", songs.len());
    let mut database = Database::new_empty_in_dir(PathBuf::from("."), PathBuf::from(&lib_dir));
    let unknown_artist = database.add_artist_new(Artist {
        id: 0,
        name: format!("<unknown>"),
        cover: None,
        albums: vec![],
        singles: vec![],
        general: GeneralData::default(),
    });
    eprintln!(
        "searching for artists and adding songs... (this will be much faster with --skip-duration because it avoids loading and decoding all the mp3 files)"
    );
    let mut artists = HashMap::new();
    let len = songs.len();
    let mut prev_perc = 999;
    songs.sort_by(|(path1, _, tags1), (path2, _, tags2)| {
        // Sort by Disc->Track->Path
        if let (Some(d1), Some(d2)) = (tags1.disc(), tags2.disc()) {
            d1.cmp(&d2)
        } else {
            Ordering::Equal
        }
        .then_with(|| {
            if let (Some(t1), Some(t2)) = (tags1.track(), tags2.track()) {
                t1.cmp(&t2)
            } else {
                Ordering::Equal
            }
            .then_with(|| path1.cmp(&path2))
        })
    });
    for (i, (song_path, song_file_metadata, song_tags)) in songs.into_iter().enumerate() {
        let perc = i * 100 / len;
        if perc != prev_perc {
            eprint!("{perc: >2}%\r");
            _ = std::io::stderr().lock().flush();
            prev_perc = perc;
        }
        let mut general = GeneralData::default();
        match (song_tags.track(), song_tags.total_tracks()) {
            (None, None) => {}
            (Some(n), Some(t)) => general.tags.push(format!("TrackNr={n}/{t}")),
            (Some(n), None) => general.tags.push(format!("TrackNr={n}")),
            (None, Some(t)) => general.tags.push(format!("TrackNr=?/{t}")),
        }
        match (song_tags.disc(), song_tags.total_discs()) {
            (None, None) => {}
            (Some(n), Some(t)) => general.tags.push(format!("DiscNr={n}/{t}")),
            (Some(n), None) => general.tags.push(format!("DiscNr={n}")),
            (None, Some(t)) => general.tags.push(format!("DiscNr=?/{t}")),
        }
        if let Some(year) = song_tags.year() {
            general.tags.push(format!("Year={year}"));
        }
        if let Some(genre) = song_tags.genre_parsed() {
            general.tags.push(format!("Genre={genre}"));
        }
        let (artist_id, album_id) = if let Some(artist) = song_tags
            .album_artist()
            .filter(|v| !v.trim().is_empty())
            .or_else(|| song_tags.artist().filter(|v| !v.trim().is_empty()))
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
            if let Some(album) = song_tags.album().filter(|a| !a.trim().is_empty()) {
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
                        (album_id, song_path.parent().map(|dir| dir.to_path_buf())),
                    );
                    album_id
                } else {
                    let album = albums.get_mut(album).unwrap();
                    if album
                        .1
                        .as_ref()
                        .is_some_and(|dir| Some(dir.as_path()) != song_path.parent())
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
        let path = song_path.strip_prefix(&lib_dir).unwrap();
        let title = song_tags
            .title()
            .map_or(None, |title| {
                if title.trim().is_empty() {
                    None
                } else {
                    Some(title.to_string())
                }
            })
            .unwrap_or_else(|| {
                song_path
                    .file_stem()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            });
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
            file_size: song_file_metadata.len(),
            duration_millis: if let Some(dur) = song_tags.duration() {
                dur as u64
            } else {
                if skip_duration {
                    eprintln!(
                        "Duration of song {:?} not found in tags, using 0 instead!",
                        song_path
                    );
                    0
                } else {
                    match mp3_duration::from_path(&song_path) {
                        Ok(dur) => dur.as_millis().min(u64::MAX as _) as u64,
                        Err(e) => {
                            eprintln!("Duration of song {song_path:?} not found in tags and can't be determined from the file contents either ({e}). Using duration 0 instead.");
                            0
                        }
                    }
                }
            },
            general,
            cached_data: CachedData(Arc::new(Mutex::new((None, None)))),
        });
    }
    eprintln!("searching for covers...");
    let mut multiple_cover_options = vec![];
    let mut single_images = HashMap::new();
    for (i1, (_artist, (artist_id, albums))) in artists.iter().enumerate() {
        eprint!("\rartist {}/{}", i1 + 1, artists.len());
        for (_album, (album_id, album_dir)) in albums {
            if let Some(album_dir) = album_dir {
                if let Some(cover_id) = get_cover(
                    &mut database,
                    &lib_dir,
                    album_dir,
                    &mut multiple_cover_options,
                ) {
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
                    } else if let Some(cover_id) =
                        get_cover(&mut database, &lib_dir, dir, &mut multiple_cover_options)
                    {
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
    if !multiple_cover_options.is_empty() {
        eprintln!("> Found more than one cover in the following directories: ");
        for dir in multiple_cover_options {
            eprintln!(">> {}", dir.to_string_lossy());
        }
        eprintln!("> Default behavior is using the largest image file found.");
    }
    if let Some(uka) = database.artists().get(&unknown_artist) {
        if uka.albums.is_empty() && uka.singles.is_empty() {
            database.artists_mut().remove(&unknown_artist);
        } else {
            eprintln!("Added the <unknown> artist as a fallback!");
        }
    }
    if let Some(custom_files) = custom_files {
        if artist_txt {
            eprintln!("[info] Searching for <artist>.txt files in custom-files dir...");
            let l = database.artists().len();
            let mut c = 0;
            for (i, artist) in database.artists_mut().values_mut().enumerate() {
                if let Ok(info) =
                    fs::read_to_string(custom_files.join(format!("{}.txt", artist.name)))
                {
                    c += 1;
                    for line in info.lines() {
                        artist.general.tags.push(line.to_owned());
                    }
                }
                eprint!(" {}/{l} ({c})\r", i + 1);
            }
            eprintln!();
        }
        if artist_img {
            eprintln!("[info] Searching for <artist>.{{png,jpg,...}} files in custom-files dir...");
            match fs::read_dir(&custom_files) {
                Err(e) => {
                    eprintln!("Can't read custom-files dir {custom_files:?}: {e}");
                }
                Ok(ls) => {
                    let mut files = HashMap::new();
                    for entry in ls {
                        if let Ok(entry) = entry {
                            let p = entry.path();
                            if let Some(base) = p.file_stem().and_then(|v| v.to_str()) {
                                if let Some(ext) = entry
                                    .path()
                                    .extension()
                                    .and_then(|v| v.to_str())
                                    .filter(|v| {
                                        matches!(v.to_lowercase().as_str(), "png" | "jpg" | "jpeg")
                                    })
                                {
                                    if let Some(old) = files.insert(base.to_owned(), ext.to_owned())
                                    {
                                        eprintln!("[warn] Not using file {base}.{old}, because {base}.{ext} was found.");
                                    }
                                }
                            }
                        }
                    }
                    for artist in database.artists_mut().values_mut() {
                        if let Some(ext) = files.get(&artist.name) {
                            artist.general.tags.push(format!("ImageExt={ext}"));
                        }
                    }
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

fn get_cover(
    database: &mut Database,
    lib_dir: &str,
    abs_dir: impl AsRef<Path>,
    multiple_options_list: &mut Vec<PathBuf>,
) -> Option<CoverId> {
    let mut multiple = false;
    let mut cover = None;
    if let Ok(files) = fs::read_dir(&abs_dir) {
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
                                if cover.is_some() {
                                    multiple = true;
                                }
                                cover = Some((path, metadata.len()));
                            }
                        }
                    }
                }
            }
        }
    }
    if multiple {
        multiple_options_list.push(abs_dir.as_ref().to_path_buf());
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
