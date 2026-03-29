use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap},
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::SystemTime,
};

use id3::TagLike;
use musicdb_lib::data::{
    CoverId, DatabaseLocation, GeneralData,
    album::Album,
    artist::Artist,
    database::{Cover, Database},
    song::Song,
};

fn main() {
    // arg parsing
    let mut args = std::env::args().skip(1);
    let lib_dir = if let Some(arg) = args.next() {
        arg
    } else {
        eprintln!(
            "usage: musicdb-filldb <library root> [--help] [--skip-duration] [--custom-files <path>] [... (see --help)]"
        );
        std::process::exit(1);
    };
    let mut bad_arg = false;
    let mut dbdir = ".".to_owned();
    let mut skip_duration = false;
    let mut verbosity = 0;
    let mut custom_files = None;
    let mut artist_img = false;
    let mut export_custom_files = None;

    let mut stats = false;

    let mut year_counts = BTreeMap::new();
    let mut genre_counts = BTreeMap::new();

    loop {
        match args.next() {
            None => break,
            Some(arg) => match arg.as_str() {
                "-v" | "--verbose" => verbosity += 1,
                "--help" => {
                    eprintln!(
                        "-v, --verbose: Generate more warnings (can be specified multiple times)"
                    );
                    eprintln!("--dbdir <path>: Save dbfile in the <path> directory (default: `.`)");
                    eprintln!(
                        "--skip-duration: Don't try to figure out the songs duration from file contents. This means mp3 files with the Duration field unset will have a duration of 0."
                    );
                    eprintln!(
                        "--custom-files <path>: Server will use <path> as its custom-files directory. Additional data is loaded from here."
                    );
                    eprintln!(
                        "--cf-artist-img: For each artist, check for an <artist>.{{jpg,png,...}} file. If it exists, add ImageExt=<extension> tag to the artist, so the image can be loaded by clients later."
                    );
                    eprintln!(
                        "--export-custom-files <path>: Create <path> as a directory containing metadata from the *existing* dbfile, so that it can be loaded again using --custom-files <same-path>."
                    );
                    eprintln!("--stats, --statistics: Output statistics before exiting.");
                    return;
                }
                "--stats" | "--statistics" => stats = true,
                "--dbdir" => {
                    if let Some(dir) = args.next() {
                        dbdir = dir;
                    } else {
                        bad_arg = true;
                        eprintln!("--dbdir <path> :: missing <path>!");
                    }
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
                "--cf-artist-img" => artist_img = true,
                "--export-custom-files" => {
                    if let Some(path) = args.next() {
                        export_custom_files = Some(PathBuf::from(path));
                    } else {
                        bad_arg = true;
                        eprintln!("--export-custom-files <path> :: missing <path>!");
                    }
                }
                arg => {
                    bad_arg = true;
                    eprintln!("Unknown argument: {arg}");
                }
            },
        }
    }
    if export_custom_files.is_some() && (skip_duration || custom_files.is_some() || artist_img) {
        bad_arg = true;
        eprintln!("--export-custom-files :: incompatible with other arguments except --dbdir!");
    }
    if bad_arg {
        return;
    }
    if let Some(path) = export_custom_files {
        export_to_custom_files_dir(dbdir, path);
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
    let mut database = Database::new_empty_in_dir(PathBuf::from(dbdir), PathBuf::from(&lib_dir));
    let unknown_artist = database.add_artist_new(Artist {
        id: 0,
        name: "<unknown>".to_owned(),
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
        match (tags1.disc(), tags2.disc()) {
            (Some(d1), Some(d2)) => d1.cmp(&d2),
            (Some(_), None) => Ordering::Greater,
            (None, Some(_)) => Ordering::Less,
            (None, None) => Ordering::Equal,
        }
        .then_with(|| {
            match (tags1.track(), tags2.track()) {
                (Some(t1), Some(t2)) => t1.cmp(&t2),
                (Some(_), None) => Ordering::Greater,
                (None, Some(_)) => Ordering::Less,
                (None, None) => Ordering::Equal,
            }
            .then_with(|| path1.cmp(path2))
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
            (Some(n), Some(t)) => general.tags.push(format!("SRCFILE:TrackNr={n}/{t}")),
            (Some(n), None) => general.tags.push(format!("SRCFILE:TrackNr={n}")),
            (None, Some(t)) => general.tags.push(format!("SRCFILE:TrackNr=?/{t}")),
        }
        match (song_tags.disc(), song_tags.total_discs()) {
            (None, None) => {}
            (Some(n), Some(t)) => general.tags.push(format!("SRCFILE:DiscNr={n}/{t}")),
            (Some(n), None) => general.tags.push(format!("SRCFILE:DiscNr={n}")),
            (None, Some(t)) => general.tags.push(format!("SRCFILE:DiscNr=?/{t}")),
        }
        if let Some(year) = song_tags.year() {
            general.tags.push(format!("SRCFILE:Year={year}"));
            if stats {
                *year_counts.entry(year).or_insert(0) += 1;
            }
        } else if verbosity > 0 {
            eprintln!("Missing year tag for file {}.", song_path.display());
        }
        if let Some(genre) = song_tags.genre_parsed() {
            general.tags.push(format!("SRCFILE:Genre={genre}"));
            if stats {
                *genre_counts.entry(genre.into_owned()).or_insert(0) += 1;
            }
        } else if verbosity > 0 {
            eprintln!("Missing genre tag for file {}.", song_path.display());
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
            .and_then(|title| {
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
        database.add_song_new(Song::new(
            DatabaseLocation {
                rel_path: path.to_path_buf(),
            },
            match song_path.metadata() {
                Ok(v) => match v.modified() {
                    Ok(v) => if let Ok(time) = v.duration_since(SystemTime::UNIX_EPOCH) {
                        Some(time.as_secs())
                    } else {
                        eprintln!(
                            "LastModified time of song {:?} is before the UNIX-EPOCH, setting `None`.",
                            song_path
                        );
                        None
                    },
                    Err(e) => {
                        eprintln!(
                            "LastModified time of song {:?} not available: {e}.",
                            song_path
                        );
                        None
                    }
                }
                Err(e) => {
                    eprintln!(
                        "LastModified time of song {:?} could not be read: {e}.",
                        song_path
                    );
                    None
                }
            },
            title.clone(),
            album_id,
            artist_id,
            vec![],
            None,
            song_file_metadata.len(),
            if let Some(dur) = song_tags.duration() {
                dur as u64
            } else {
                if skip_duration {
                    if verbosity > 0 {
                        eprintln!(
                            "Duration of song {:?} not found in tags, using 0 instead!",
                            song_path
                        );
                    }
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
        ));
    }
    {
        let (artists, albums, songs) = database.artists_albums_songs_mut();
        fn unsrcfile(tags: &mut Vec<String>) {
            let srcfile_tags = tags
                .iter()
                .filter_map(|tag| tag.strip_prefix("SRCFILE:"))
                .map(|tag| tag.to_owned())
                .collect::<Vec<_>>();
            for tag in srcfile_tags {
                if !tags.contains(&tag) {
                    tags.push(tag.to_owned());
                }
            }
        }
        for v in artists.values_mut() {
            unsrcfile(&mut v.general.tags);
        }
        for v in albums.values_mut() {
            unsrcfile(&mut v.general.tags);
        }
        for v in songs.values_mut() {
            unsrcfile(&mut v.general.tags);
        }
    }
    eprintln!("searching for covers...");
    let mut multiple_cover_options = vec![];
    let mut single_images = HashMap::new();
    for (i1, (_artist, (artist_id, albums))) in artists.iter().enumerate() {
        eprint!("\rartist {}/{}", i1 + 1, artists.len());
        for (album_id, album_dir) in albums.values() {
            if let Some(album_dir) = album_dir
                && let Some(cover_id) = get_cover(
                    &mut database,
                    &lib_dir,
                    album_dir,
                    &mut multiple_cover_options,
                )
            {
                database.albums_mut().get_mut(album_id).unwrap().cover = Some(cover_id);
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
        if artist_img {
            eprintln!("[info] Searching for <artist>.{{png,jpg,...}} files in custom-files dir...");
            match fs::read_dir(&custom_files) {
                Err(e) => {
                    eprintln!("Can't read custom-files dir {custom_files:?}: {e}");
                }
                Ok(ls) => {
                    let mut files = HashMap::new();
                    for entry in ls.flatten() {
                        let p = entry.path();
                        if let Some(base) = p.file_stem().and_then(|v| v.to_str())
                            && let Some(ext) = entry
                                .path()
                                .extension()
                                .and_then(|v| v.to_str())
                                .filter(|v| {
                                    matches!(v.to_lowercase().as_str(), "png" | "jpg" | "jpeg")
                                })
                            && let Some(old) = files.insert(base.to_owned(), ext.to_owned())
                        {
                            eprintln!(
                                "[warn] Not using file {base}.{old}, because {base}.{ext} was found."
                            );
                        }
                    }
                    for artist in database.artists_mut().values_mut() {
                        if let Some(ext) = files.get(&artist.name) {
                            artist.general.tags.push(format!("SRCFILE:ImageExt={ext}"));
                            artist.general.tags.push(format!("ImageExt={ext}"));
                        }
                    }
                }
            }
        }
        eprintln!(
            "[info] Searching for <artist>.tags, <artist>.d/<album>.tags, <artist>.d/singles.d/<song>.tags, <artist>.d/<album>.d/<song>.tags in custom-files dir..."
        );
        let l = database.artists().len() + database.albums().len() + database.songs().len();
        let mut cc = 0;
        let mut c = 0;
        let (artists, albums, songs) = database.artists_albums_songs_mut();
        fn push_tags(info: &str, tags: &mut Vec<String>) {
            for line in info.lines() {
                let tag = normalized_str_to_tag(line);
                if !tags.contains(&tag) {
                    tags.push(tag);
                }
            }
        }
        for artist in artists.values_mut() {
            // <artist>.tags
            cc += 1;
            if let Ok(info) = fs::read_to_string(custom_files.join(format!(
                "{}.tags",
                normalize_to_file_path_component_for_custom_files(&artist.name)
            ))) {
                c += 1;
                push_tags(&info, &mut artist.general.tags);
            }
            // <artist>.d/
            let dir = custom_files.join(format!(
                "{}.d",
                normalize_to_file_path_component_for_custom_files(&artist.name)
            ));
            if fs::metadata(&dir).is_ok_and(|meta| meta.is_dir()) {
                // <artist>.d/singles/
                {
                    let dir = dir.join("singles");
                    for song in artist.singles.iter() {
                        // <artist>.d/singles/<song>.tags
                        cc += 1;
                        if let Some(song) = songs.get_mut(song)
                            && let Ok(info) = fs::read_to_string(dir.join(format!(
                                "{}.tags",
                                normalize_to_file_path_component_for_custom_files(&song.title)
                            )))
                        {
                            c += 1;
                            push_tags(&info, &mut song.general.tags);
                        }
                    }
                }
                for album in artist.albums.iter() {
                    eprint!(" {cc}/{l} ({c})\r");
                    cc += 1;
                    if let Some(album) = albums.get_mut(album) {
                        // <artist>.d/<album>.tags
                        if let Ok(info) = fs::read_to_string(dir.join(format!(
                            "{}.tags",
                            normalize_to_file_path_component_for_custom_files(&album.name)
                        ))) {
                            c += 1;
                            push_tags(&info, &mut album.general.tags);
                        }
                        // <artist>.d/<album>.d/
                        let dir = dir.join(format!(
                            "{}.d",
                            normalize_to_file_path_component_for_custom_files(&album.name)
                        ));
                        for song in album.songs.iter() {
                            cc += 1;
                            if let Some(song) = songs.get_mut(song) {
                                // <artist>.d/<album>.d/<song>.tags
                                if let Ok(info) = fs::read_to_string(dir.join(format!(
                                    "{}.tags",
                                    normalize_to_file_path_component_for_custom_files(&song.title)
                                ))) {
                                    c += 1;
                                    push_tags(&info, &mut song.general.tags);
                                }
                            }
                        }
                    }
                }
            } else {
                cc += artist.albums.len();
                for album in artist.albums.iter() {
                    if let Some(album) = albums.get(album) {
                        cc += album.songs.len();
                    }
                }
            }
            eprint!(" {cc}/{l} ({c})\r");
        }
        eprintln!();
    }
    eprintln!("saving dbfile...");
    database.save_database(None).unwrap();
    eprintln!("done!");
    if stats {
        eprintln!();
        eprintln!("=== Genre Statistics ===");
        let mut genre_counts = genre_counts
            .iter()
            .map(|(genre, count)| (genre, *count))
            .collect::<Vec<_>>();
        genre_counts.sort_by(|(_, count_1), (_, count_2)| count_2.cmp(count_1));
        for (genre, count) in genre_counts {
            eprintln!("{genre}: {count}");
        }
        eprintln!();
        eprintln!("=== Year Statistics ===");
        if let Some((&min_year, _)) = year_counts.first_key_value()
            && let Some((&max_year, _)) = year_counts.last_key_value()
        {
            let width = year_counts
                .values()
                .copied()
                .max()
                .unwrap_or(0)
                .to_string()
                .len();
            for i in min_year / 10..=max_year / 10 {
                let start = 10 * i;
                let end = 10 * (1 + i);
                let total = year_counts
                    .range(start..end)
                    .map(|(_, c)| *c)
                    .sum::<usize>()
                    .to_string();
                eprint!(
                    "{}-{} | ∑: {}{}",
                    start,
                    end - 1,
                    " ".repeat(width + 1 - total.len()),
                    total,
                );
                for y in start..end {
                    let count = year_counts.get(&y).copied().unwrap_or(0);
                    let (pre, post) = if count == 0 {
                        ("\x1b[90m", "\x1b[0m")
                    } else {
                        ("", "")
                    };
                    let count = count.to_string();
                    eprint!(
                        ", {}{y}: {}{}{}",
                        pre,
                        " ".repeat(width - count.len()),
                        count,
                        post
                    );
                }
                eprintln!();
            }
        }
    }
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
        for file in files.flatten() {
            let path = file.path();
            if let Ok(metadata) = path.metadata()
                && metadata.is_file()
                && path
                    .extension()
                    .and_then(|v| v.to_str())
                    .is_some_and(|v| matches!(v.to_lowercase().as_str(), "png" | "jpg" | "jpeg"))
                && (cover.is_none()
                    || cover
                        .as_ref()
                        .is_some_and(|(_, size)| *size < metadata.len()))
            {
                if cover.is_some() {
                    multiple = true;
                }
                cover = Some((path, metadata.len()));
            }
        }
    }
    if multiple {
        multiple_options_list.push(abs_dir.as_ref().to_path_buf());
    }
    if let Some((path, _)) = cover {
        let rel_path = path.strip_prefix(lib_dir).unwrap().to_path_buf();
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

fn normalize_to_file_path_component_for_custom_files(str: &str) -> String {
    str.replace('%', "%p")
        .replace('\0', "%0")
        .replace('/', "%s")
        .replace('\\', "%S")
        .replace('\t', "%t")
        .replace('\r', "%r")
        .replace('\n', "%n")
}

// may NOT set \! to a valid escape sequence, as this is used to
// identify db-internal "tags" such as the Song-/Album-/Artist-ID
fn normalize_tag_to_str(str: &str) -> String {
    str.replace('\\', "\\S")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}
fn normalized_str_to_tag(str: &str) -> String {
    str.replace(['\n', '\r'], "")
        .replace("\\n", "\n")
        .replace("\\r", "\r")
        .replace("\\S", "\\")
}

fn export_to_custom_files_dir(dbdir: String, path: PathBuf) {
    let database = Database::load_database_from_dir(dbdir.into(), PathBuf::new()).unwrap();
    for (artist_id, artist) in database.artists().iter() {
        export_custom_files_tags(
            &artist.general.tags,
            &gen_internals(Some(*artist_id), artist.cover),
            &path.join(format!(
                "{}.tags",
                normalize_to_file_path_component_for_custom_files(&artist.name)
            )),
        );
        let dir = path.join(format!(
            "{}.d",
            normalize_to_file_path_component_for_custom_files(&artist.name)
        ));
        {
            let dir = dir.join("singles");
            for song_id in artist.singles.iter() {
                if let Some(song) = database.songs().get(song_id) {
                    export_custom_files_tags(
                        &song.general.tags,
                        &gen_internals(Some(*song_id), song.cover),
                        &dir.join(format!(
                            "{}.tags",
                            normalize_to_file_path_component_for_custom_files(&song.title,)
                        )),
                    );
                }
            }
        }
        for album_id in artist.albums.iter() {
            if let Some(album) = database.albums().get(album_id) {
                export_custom_files_tags(
                    &album.general.tags,
                    &gen_internals(Some(*album_id), album.cover),
                    &dir.join(format!(
                        "{}.tags",
                        normalize_to_file_path_component_for_custom_files(&album.name,)
                    )),
                );
                let dir = dir.join(format!(
                    "{}.d",
                    normalize_to_file_path_component_for_custom_files(&album.name,)
                ));
                for song_id in album.songs.iter() {
                    if let Some(song) = database.songs().get(song_id) {
                        export_custom_files_tags(
                            &song.general.tags,
                            &gen_internals(Some(*song_id), song.cover),
                            &dir.join(format!(
                                "{}.tags",
                                normalize_to_file_path_component_for_custom_files(&song.title,)
                            )),
                        );
                    }
                }
            }
        }
    }
}
fn export_custom_files_tags(tags: &[String], internals: &[String], path: &Path) {
    if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
        let mut normalized_tags = None;
        fn mk_normalized_tags<'a>(
            normalized_tags: &'a mut Option<String>,
            tags: &[String],
            internals: &[String],
        ) -> &'a String {
            &*normalized_tags.get_or_insert_with(|| {
                let mut tags = Vec::from(tags);
                let mut rm = Vec::new();
                for (i, srcfile_tag_stripped) in tags
                    .iter()
                    .enumerate()
                    .filter_map(|(i, tag)| Some((i, tag.strip_prefix("SRCFILE:")?)))
                {
                    match rm.binary_search(&i) {
                        Ok(_) => {}
                        Err(v) => rm.insert(v, i),
                    }
                    if let Some(i) = tags.iter().position(|tag| tag == srcfile_tag_stripped) {
                        // There is a tag which just repeats the information
                        // which is already present in the source (audio) file.
                        // We do not want to save this information, so that,
                        // if the audio file is replaced in the future, its new
                        // information is used by musicdb, and musicdb-internal
                        // information is only used if it was changed to be different
                        // from the source file by the user.
                        match rm.binary_search(&i) {
                            Ok(_) => {}
                            Err(v) => rm.insert(v, i),
                        }
                    }
                }
                for i in rm.into_iter().rev() {
                    tags.remove(i);
                }
                tags.iter()
                    .map(|tag| normalize_tag_to_str(tag) + "\n")
                    .chain(
                        internals
                            .iter()
                            .map(|tag| format!("\\!{}\n", normalize_tag_to_str(tag))),
                    )
                    .collect::<String>()
            })
        }
        let allow_write = match fs::exists(path) {
            Err(e) => {
                eprintln!("Cannot check for {}, skipping. Error: {e}", path.display());
                false
            }
            Ok(false) => true,
            Ok(true) => {
                if fs::read_to_string(path).is_ok_and(|file| {
                    file == *mk_normalized_tags(&mut normalized_tags, tags, internals)
                }) {
                    // file contains the same tags as database, don't write,
                    // but don't create backup either
                    false
                } else {
                    let backup_path = path.with_file_name(format!("{file_name}.backup"));
                    match fs::exists(&backup_path) {
                        Err(e) => {
                            eprintln!(
                                "Cannot check for {}, skipping {}. Error: {e}",
                                backup_path.display(),
                                path.display()
                            );
                            false
                        }
                        Ok(true) => {
                            eprintln!(
                                "Backup {} exists, skipping {}.",
                                backup_path.display(),
                                path.display()
                            );
                            false
                        }
                        Ok(false) => {
                            if let Err(e) = fs::rename(path, &backup_path) {
                                eprintln!(
                                    "Failed to move previous file/dir {} to {}: {e}",
                                    path.display(),
                                    backup_path.display()
                                );
                                false
                            } else {
                                true
                            }
                        }
                    }
                }
            }
        };
        if allow_write && !mk_normalized_tags(&mut normalized_tags, tags, internals).is_empty() {
            if let Some(p) = path.parent()
                && let Err(e) = fs::create_dir_all(p)
            {
                eprintln!(
                    "Could not create directory to contain {}: {e}",
                    path.display()
                );
            }
            if let Err(e) = fs::write(
                path,
                mk_normalized_tags(&mut normalized_tags, tags, internals),
            ) {
                eprintln!("Could not save {}: {e}", path.display());
            }
        }
    } else {
        eprintln!(
            "[ERR] Somehow created a non-unicode path {path:?}! This should not have happened!"
        );
    }
}

// TODO: load these tags, infer album and artist id from parent directories in the structure (will probably happen with no further changes required)
fn gen_internals(id: Option<u64>, cover: Option<CoverId>) -> Vec<String> {
    [
        id.map(|id| format!("id={id}")),
        cover.map(|id| format!("cover={id}")),
    ]
    .into_iter()
    .flatten()
    .collect()
}
