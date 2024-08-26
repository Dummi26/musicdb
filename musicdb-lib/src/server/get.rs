use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Instant, SystemTime},
};

use crate::data::{database::Database, CoverId, SongId};

pub struct Client<T: Write + Read>(BufReader<T>);
impl<T: Write + Read> Client<T> {
    pub fn new(mut con: BufReader<T>) -> std::io::Result<Self> {
        writeln!(con.get_mut(), "get")?;
        con.get_mut().flush()?;
        Ok(Self(con))
    }
    pub fn cover_bytes(&mut self, id: CoverId) -> Result<Result<Vec<u8>, String>, std::io::Error> {
        writeln!(
            self.0.get_mut(),
            "{}",
            con_get_encode_string(&format!("cover-bytes\n{id}"))
        )?;
        self.0.get_mut().flush()?;
        let mut response = String::new();
        self.0.read_line(&mut response)?;
        if response.starts_with("len: ") {
            if let Ok(len) = response[4..].trim().parse() {
                let mut bytes = vec![0; len];
                self.0.read_exact(&mut bytes)?;
                Ok(Ok(bytes))
            } else {
                Ok(Err(response))
            }
        } else {
            Ok(Err(response))
        }
    }
    pub fn song_file(&mut self, id: SongId) -> Result<Result<Vec<u8>, String>, std::io::Error> {
        writeln!(
            self.0.get_mut(),
            "{}",
            con_get_encode_string(&format!("song-file\n{id}",))
        )?;
        self.0.get_mut().flush()?;
        let mut response = String::new();
        self.0.read_line(&mut response)?;
        if response.starts_with("len: ") {
            if let Ok(len) = response[4..].trim().parse() {
                let mut bytes = vec![0; len];
                self.0.read_exact(&mut bytes)?;
                Ok(Ok(bytes))
            } else {
                Ok(Err(response))
            }
        } else {
            Ok(Err(response))
        }
    }
    pub fn custom_file(&mut self, path: &str) -> Result<Result<Vec<u8>, String>, std::io::Error> {
        writeln!(
            self.0.get_mut(),
            "{}",
            con_get_encode_string(&format!("custom-file\n{path}",))
        )?;
        self.0.get_mut().flush()?;
        let mut response = String::new();
        self.0.read_line(&mut response)?;
        if response.starts_with("len: ") {
            if let Ok(len) = response[4..].trim().parse() {
                let mut bytes = vec![0; len];
                self.0.read_exact(&mut bytes)?;
                Ok(Ok(bytes))
            } else {
                Ok(Err(response))
            }
        } else {
            Ok(Err(response))
        }
    }
    pub fn song_file_by_path(
        &mut self,
        path: &str,
    ) -> Result<Result<Vec<u8>, String>, std::io::Error> {
        writeln!(
            self.0.get_mut(),
            "{}",
            con_get_encode_string(&format!("song-file-by-path\n{path}",))
        )?;
        self.0.get_mut().flush()?;
        let mut response = String::new();
        self.0.read_line(&mut response)?;
        if response.starts_with("len: ") {
            if let Ok(len) = response[4..].trim().parse() {
                let mut bytes = vec![0; len];
                self.0.read_exact(&mut bytes)?;
                Ok(Ok(bytes))
            } else {
                Ok(Err(response))
            }
        } else {
            Ok(Err(response))
        }
    }
    /// find songs which have no last-modified time, whose time has changed, whose files have been removed or whose files could not be read due to another error.
    pub fn find_songs_with_changed_files(
        &mut self,
    ) -> Result<
        Result<
            (
                Vec<SongId>,
                Vec<(SongId, u64)>,
                Vec<SongId>,
                Vec<(SongId, String)>,
            ),
            String,
        >,
        std::io::Error,
    > {
        writeln!(
            self.0.get_mut(),
            "{}",
            con_get_encode_string("find-songs-with-changed-files")
        )?;
        self.0.get_mut().flush()?;
        loop {
            let mut response = String::new();
            self.0.read_line(&mut response)?;
            let len_line = response.trim();
            if len_line.starts_with('%') {
                eprintln!(
                    "[Find Songs With Changed Files] Status: {}",
                    len_line[1..].trim()
                );
            } else {
                let mut read_list = || -> std::io::Result<Result<Vec<String>, String>> {
                    if len_line.starts_with("len: ") {
                        if let Ok(len) = len_line[4..].trim().parse() {
                            let mut out = Vec::with_capacity(len);
                            for _ in 0..len {
                                let mut line = String::new();
                                self.0.read_line(&mut line)?;
                                let line = line.trim_end_matches(['\n', '\r']);
                                out.push(line.trim().to_owned());
                            }
                            Ok(Ok(out))
                        } else {
                            Ok(Err(format!("bad len in len-line: {len_line}")))
                        }
                    } else {
                        Ok(Err(format!("bad len-line: {len_line}")))
                    }
                };
                break Ok(Ok((
                    match read_list()? {
                        Ok(v) => match v
                            .into_iter()
                            .map(|v| v.trim().parse::<SongId>().map_err(|e| (v, e.to_string())))
                            .collect()
                        {
                            Ok(v) => v,
                            Err((s, e)) => {
                                return Ok(Err(format!("error parsing songid(notime) '{s}': {e}")))
                            }
                        },
                        Err(e) => return Ok(Err(e)),
                    },
                    match read_list()? {
                        Ok(v) => match v
                            .into_iter()
                            .map(|v| {
                                v.trim()
                                    .split_once(':')
                                    .ok_or_else(|| format!("missing colon"))
                                    .and_then(|(i, t)| {
                                        Ok((
                                            i.parse::<SongId>().map_err(|e| e.to_string())?,
                                            t.parse::<u64>().map_err(|e| e.to_string())?,
                                        ))
                                    })
                                    .map_err(|e| (v, e))
                            })
                            .collect()
                        {
                            Ok(v) => v,
                            Err((s, e)) => {
                                return Ok(Err(format!("error parsing songid+time '{s}': {e}")))
                            }
                        },
                        Err(e) => return Ok(Err(e)),
                    },
                    match read_list()? {
                        Ok(v) => match v
                            .into_iter()
                            .map(|v| v.trim().parse::<SongId>().map_err(|e| (v, e)))
                            .collect()
                        {
                            Ok(v) => v,
                            Err((s, e)) => {
                                return Ok(Err(format!("error parsing songid(deleted) '{s}': {e}")))
                            }
                        },
                        Err(e) => return Ok(Err(e)),
                    },
                    match read_list()? {
                        Ok(v) => match v
                            .into_iter()
                            .map(|v| {
                                v.trim()
                                    .split_once(':')
                                    .ok_or_else(|| format!("missing colon"))
                                    .and_then(|(i, t)| {
                                        Ok((
                                            i.parse::<SongId>().map_err(|e| e.to_string())?,
                                            t.to_owned(),
                                        ))
                                    })
                                    .map_err(|e| (v, e))
                            })
                            .collect()
                        {
                            Ok(v) => v,
                            Err((s, e)) => {
                                return Ok(Err(format!("error parsing songid+error '{s}': {e}")))
                            }
                        },
                        Err(e) => return Ok(Err(e)),
                    },
                )));
            };
        }
    }
    /// tell the server to search for files that are not in its song database.
    ///
    /// ## `extensions`:
    /// If `None`, the server uses a default set of music-related extensions (`[".mp3", ...]`).
    /// If `Some([])`, allow all extensions, even ones like `.jpg` and files without extensions.
    /// If `Some(...)`, only allow the specified extensions. Note: These are actually suffixes, for example `mp3` would allow a file named `test_mp3`, while `.mp3` would only allow `test.mp3`.
    /// Because of this, you usually want to include the `.` before the extension, and double extensions like `.tar.gz` are also supported.
    ///
    /// For each file, returns a boolean error flag indicating, if `true`, that the path was invalid (not UTF-8 or contained a newline).
    pub fn find_unused_song_files(
        &mut self,
        extensions: Option<&[&str]>,
    ) -> Result<Result<Vec<(String, bool)>, String>, std::io::Error> {
        let mut str = "find-unused-song-files".to_owned();
        if let Some(extensions) = extensions {
            if extensions.is_empty() {
                str.push_str("\nextensions");
            } else {
                str.push_str("\nextensions=");
                for (i, ext) in extensions.iter().enumerate() {
                    if i > 0 {
                        str.push(':');
                    }
                    str.push_str(ext);
                }
            }
        }
        writeln!(self.0.get_mut(), "{}", con_get_encode_string(&str))?;
        self.0.get_mut().flush()?;
        loop {
            let mut response = String::new();
            self.0.read_line(&mut response)?;
            let len_line = response.trim();
            if len_line.starts_with('%') {
                eprintln!("[Find Unused Song Files] Status: {}", len_line[1..].trim());
            } else {
                break if len_line.starts_with("len: ") {
                    if let Ok(len) = len_line[4..].trim().parse() {
                        let mut out = Vec::with_capacity(len);
                        for _ in 0..len {
                            let mut line = String::new();
                            self.0.read_line(&mut line)?;
                            let line = line.trim_end_matches(['\n', '\r']);
                            if line.starts_with('#') {
                                out.push((line[1..].to_owned(), false))
                            } else if line.starts_with('!') {
                                out.push((line[1..].to_owned(), true))
                            } else {
                                return Ok(Err(format!("bad line-format: {line}")));
                            }
                        }
                        Ok(Ok(out))
                    } else {
                        Ok(Err(format!("bad len in len-line: {len_line}")))
                    }
                } else {
                    Ok(Err(format!("bad len-line: {len_line}")))
                };
            };
        }
    }
}

pub fn handle_one_connection_as_get(
    db: Arc<Mutex<Database>>,
    connection: &mut BufReader<impl Read + Write>,
) -> Result<(), std::io::Error> {
    loop {
        let mut line = String::new();
        if connection.read_line(&mut line).is_ok() {
            if line.is_empty() {
                return Ok(());
            }
            let request = con_get_decode_line(&line);
            let mut request = request.lines();
            if let Some(req) = request.next() {
                match req {
                    "cover-bytes" => {
                        if let Some(cover_id) = request.next().and_then(|id| id.parse().ok()) {
                            let dbl = db.lock().unwrap();
                            if let Some(get_con) = &dbl.remote_server_as_song_file_source {
                                if let Some(bytes) = get_con
                                    .lock()
                                    .unwrap()
                                    .cover_bytes(cover_id)
                                    .ok()
                                    .and_then(Result::ok)
                                {
                                    writeln!(connection.get_mut(), "len: {}", bytes.len())?;
                                    connection.get_mut().write_all(&bytes)?;
                                } else {
                                    writeln!(connection.get_mut(), "no")?;
                                }
                            } else if let Some(cover) = dbl.covers().get(&cover_id) {
                                if let Some(v) = cover.get_bytes_from_file(
                                    |p| dbl.get_path(p),
                                    |bytes| {
                                        writeln!(connection.get_mut(), "len: {}", bytes.len())?;
                                        connection.get_mut().write_all(bytes)?;
                                        Ok::<(), std::io::Error>(())
                                    },
                                ) {
                                    v?;
                                } else {
                                    writeln!(connection.get_mut(), "no data")?;
                                }
                            } else {
                                writeln!(connection.get_mut(), "no cover")?;
                            }
                        } else {
                            writeln!(connection.get_mut(), "bad id")?;
                        }
                    }
                    "song-file" => {
                        if let Some(bytes) =
                            request
                                .next()
                                .and_then(|id| id.parse().ok())
                                .and_then(|id| {
                                    let db = db.lock().unwrap();
                                    if let Some(song) = db.get_song(&id) {
                                        let cd = song.cached_data();
                                        if let Some(data) =
                                            cd.get_data_or_maybe_start_thread(&db, song)
                                        {
                                            Some(data)
                                        } else {
                                            let cd = cd.clone();
                                            drop(db);
                                            cd.cached_data_await()
                                        }
                                    } else {
                                        None
                                    }
                                })
                        {
                            writeln!(connection.get_mut(), "len: {}", bytes.len())?;
                            connection.get_mut().write_all(&bytes)?;
                        } else {
                            writeln!(connection.get_mut(), "no data")?;
                        }
                    }
                    "custom-file" => {
                        if let Some(bytes) =
                            request.next().and_then(|path| 'load_custom_file_data: {
                                let db = db.lock().unwrap();
                                let mut parent = match &db.custom_files {
                                    None => {
                                        if let Some(con) = &db.remote_server_as_song_file_source {
                                            if let Ok(Ok(data)) =
                                                con.lock().unwrap().custom_file(path)
                                            {
                                                break 'load_custom_file_data Some(data);
                                            } else {
                                                None
                                            }
                                        } else {
                                            None
                                        }
                                    }
                                    // if a remote source is present, this means we should ignore it. if no remote source is present, use the lib_dir.
                                    Some(None) => {
                                        if db.remote_server_as_song_file_source.is_none() {
                                            Some(db.lib_directory.clone())
                                        } else {
                                            None
                                        }
                                    }
                                    Some(Some(p)) => Some(p.clone()),
                                };
                                // check for malicious paths [TODO: Improve]
                                if Path::new(path).is_absolute() {
                                    parent = None;
                                }
                                if let Some(parent) = parent {
                                    let path = parent.join(path);
                                    if path.starts_with(parent) {
                                        fs::read(path).ok()
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            })
                        {
                            writeln!(connection.get_mut(), "len: {}", bytes.len())?;
                            connection.get_mut().write_all(&bytes)?;
                        } else {
                            writeln!(connection.get_mut(), "no data")?;
                        }
                    }
                    "song-file-by-path" => {
                        if let Some(bytes) = request.next().and_then(|path| {
                            let db = db.lock().unwrap();
                            let mut parent = Some(db.lib_directory.clone());
                            // check for malicious paths [TODO: Improve]
                            if Path::new(path).is_absolute() {
                                parent = None;
                            }
                            if let Some(parent) = parent {
                                let path = parent.join(path);
                                if path.starts_with(parent) {
                                    fs::read(path).ok()
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        }) {
                            writeln!(connection.get_mut(), "len: {}", bytes.len())?;
                            connection.get_mut().write_all(&bytes)?;
                        } else {
                            writeln!(connection.get_mut(), "no data")?;
                        }
                    }
                    "find-songs-with-changed-files" => {
                        let db_lock = db.lock().unwrap();
                        let lib_directory = db_lock.lib_directory.clone();
                        let all_songs = db_lock
                            .songs()
                            .iter()
                            .map(|(id, song)| {
                                (
                                    *id,
                                    song.location.clone(),
                                    song.file_last_modified_unix_timestamp.clone(),
                                )
                            })
                            .collect::<Vec<_>>();
                        drop(db_lock);
                        let (
                            mut songs_no_time,
                            mut songs_new_time,
                            mut songs_removed,
                            mut songs_err,
                        ) = (vec![], vec![], vec![], vec![]);
                        for (id, location, last_modified) in all_songs {
                            let path = Database::get_path_nodb(&lib_directory, &location);
                            match path.try_exists() {
                                Ok(true) => match path.metadata() {
                                    Ok(metadata) => {
                                        let time = metadata.modified().ok().and_then(|time| {
                                            time.duration_since(SystemTime::UNIX_EPOCH)
                                                .ok()
                                                .map(|v| v.as_secs())
                                        });
                                        if last_modified.is_none() || time != last_modified {
                                            if let Some(time) = time {
                                                songs_new_time.push((id, time));
                                            } else {
                                                songs_no_time.push(id);
                                            }
                                        }
                                    }
                                    Err(e) => songs_err.push((id, e.to_string())),
                                },
                                Ok(false) => songs_removed.push(id),
                                Err(e) => songs_err.push((id, e.to_string())),
                            }
                        }
                        write_list(
                            connection.get_mut(),
                            songs_no_time.len(),
                            songs_no_time.into_iter().map(|id| (id, None)),
                        )?;
                        write_list(
                            connection.get_mut(),
                            songs_new_time.len(),
                            songs_new_time
                                .into_iter()
                                .map(|(id, t)| (id, Some(format!("{t}")))),
                        )?;
                        write_list(
                            connection.get_mut(),
                            songs_removed.len(),
                            songs_removed.into_iter().map(|id| (id, None)),
                        )?;
                        write_list(
                            connection.get_mut(),
                            songs_err.len(),
                            songs_err
                                .into_iter()
                                .map(|(id, e)| (id, Some(format!("{e}")))),
                        )?;
                        fn write_list(
                            connection: &mut impl Write,
                            len: usize,
                            list: impl IntoIterator<Item = (u64, Option<String>)>,
                        ) -> std::io::Result<()> {
                            writeln!(connection, "len: {}", len)?;
                            for (song, data) in list {
                                if let Some(data) = data {
                                    writeln!(connection, "{song}:{data}")?;
                                } else {
                                    writeln!(connection, "{song}")?;
                                }
                            }
                            Ok(())
                        }
                    }
                    "find-unused-song-files" => {
                        // configure search
                        let mut extensions = None;
                        loop {
                            if let Some(line) = request.next() {
                                if let Some((key, value)) = line.split_once("=") {
                                    match key.trim() {
                                        "extensions" => {
                                            extensions = Some(Some(
                                                value
                                                    .split(':')
                                                    .map(|v| v.trim().to_owned())
                                                    .collect::<Vec<_>>(),
                                            ))
                                        }
                                        _ => (),
                                    }
                                } else {
                                    match line.trim() {
                                        "extensions" => extensions = Some(None),
                                        _ => (),
                                    }
                                }
                            } else {
                                break;
                            }
                        }
                        // search
                        let lib_dir = db.lock().unwrap().lib_directory.clone();
                        let unused = find_unused_song_files(
                            &db,
                            &lib_dir,
                            &mut FindUnusedSongFilesConfig {
                                extensions: extensions
                                    .unwrap_or_else(|| Some(vec![".mp3".to_owned()])),
                                w: connection.get_mut(),
                                last_write: Instant::now(),
                                new: 0,
                                songs: 0,
                            },
                        );
                        writeln!(connection.get_mut(), "len: {}", unused.len())?;
                        for path in unused {
                            if let Some(path) = path.to_str().filter(|v| !v.contains('\n')) {
                                writeln!(connection.get_mut(), "#{path}")?;
                            } else {
                                let path = path.to_string_lossy().replace('\n', "");
                                writeln!(connection.get_mut(), "!{path}")?;
                            }
                        }
                    }
                    _ => {}
                }
            }
        } else {
            return Ok(());
        }
    }
}

pub fn con_get_decode_line(line: &str) -> String {
    let mut o = String::new();
    let mut chars = line.chars();
    loop {
        match chars.next() {
            Some('\\') => match chars.next() {
                Some('n') => o.push('\n'),
                Some('r') => o.push('\r'),
                Some('\\') => o.push('\\'),
                Some(ch) => o.push(ch),
                None => break,
            },
            Some(ch) => o.push(ch),
            None => break,
        }
    }
    o
}
pub fn con_get_encode_string(line: &str) -> String {
    let mut o = String::new();
    for ch in line.chars() {
        match ch {
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            _ => o.push(ch),
        }
    }
    o
}

fn find_unused_song_files(
    db: &Arc<Mutex<Database>>,
    path: &impl AsRef<Path>,
    cfg: &mut FindUnusedSongFilesConfig,
) -> Vec<PathBuf> {
    let mut files = vec![];
    find_unused_song_files_internal(db, path, &"", cfg, &mut files, &mut vec![], true);
    files
}

struct FindUnusedSongFilesConfig<'a> {
    extensions: Option<Vec<String>>,
    w: &'a mut dyn Write,
    last_write: Instant,
    new: usize,
    songs: usize,
}

fn find_unused_song_files_internal(
    db: &Arc<Mutex<Database>>,
    path: &impl AsRef<Path>,
    rel_path: &impl AsRef<Path>,
    cfg: &mut FindUnusedSongFilesConfig,
    unused_files: &mut Vec<PathBuf>,
    files_buf: &mut Vec<PathBuf>,
    is_final: bool,
) {
    if let Ok(rd) = std::fs::read_dir(path.as_ref()) {
        for entry in rd {
            if let Ok(entry) = entry {
                if let Ok(file_type) = entry.file_type() {
                    let path = entry.path();
                    let rel_path = rel_path.as_ref().join(entry.file_name());
                    if file_type.is_dir() {
                        find_unused_song_files_internal(
                            db,
                            &path,
                            &rel_path,
                            cfg,
                            unused_files,
                            files_buf,
                            false,
                        );
                    } else if file_type.is_file() {
                        if match &cfg.extensions {
                            None => true,
                            Some(exts) => {
                                if let Some(name) = path.file_name().and_then(|v| v.to_str()) {
                                    exts.iter().any(|ext| name.ends_with(ext))
                                } else {
                                    false
                                }
                            }
                        } {
                            files_buf.push(rel_path);
                        }
                    }
                }
            }
        }
    }
    if (is_final && files_buf.len() > 0) || files_buf.len() > 50 {
        let db = db.lock().unwrap();
        cfg.songs += files_buf.len();
        for song in db.songs().values() {
            if let Some(i) = files_buf
                .iter()
                .position(|path| path == &song.location.rel_path)
            {
                files_buf.remove(i);
            }
        }
        cfg.new += files_buf.len();
        unused_files.extend(std::mem::replace(files_buf, vec![]).into_iter());
        if cfg.last_write.elapsed().as_secs_f32() > 1.0 {
            cfg.last_write = Instant::now();
            _ = writeln!(cfg.w, "%{}/{}", cfg.new, cfg.songs);
        }
    }
}
