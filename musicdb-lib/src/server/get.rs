use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
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
    /// tell the server to search for files that are not in its song database.
    ///
    /// ## `extensions`:
    /// If `None`, the server uses a default set of music-related extensions (`[".mp3", ...]`).
    /// If `Some([])`, allow all extensions, even ones like `.jpg` and files without extensions.
    /// If `Some(...)`, only allow the specified extensions. Note: These are actually suffixes, for example `mp3` would allow a file named `test_mp3`, while `.mp3` would only allow `test.mp3`.
    /// Because of this, you usually want to include the `.` before the extension, and double extensions like `.tar.gz` are also supported.
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
        let mut response = String::new();
        self.0.read_line(&mut response)?;
        let len_line = response.trim();
        if len_line.starts_with("len: ") {
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
                        if let Some(cover) = request
                            .next()
                            .and_then(|id| id.parse().ok())
                            .and_then(|id| db.lock().unwrap().covers().get(&id).cloned())
                        {
                            if let Some(v) = cover.get_bytes(
                                |p| db.lock().unwrap().get_path(p),
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
                    }
                    "song-file" => {
                        if let Some(bytes) =
                            request
                                .next()
                                .and_then(|id| id.parse().ok())
                                .and_then(|id| {
                                    let db = db.lock().unwrap();
                                    db.get_song(&id).and_then(|song| song.cached_data_now(&db))
                                })
                        {
                            writeln!(connection.get_mut(), "len: {}", bytes.len())?;
                            connection.get_mut().write_all(&bytes)?;
                        } else {
                            writeln!(connection.get_mut(), "no data")?;
                        }
                    }
                    "custom-file" => {
                        if let Some(bytes) = request.next().and_then(|path| {
                            let db = db.lock().unwrap();
                            let mut parent = match &db.custom_files {
                                None => None,
                                Some(None) => Some(db.lib_directory.clone()),
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
                        }) {
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
                            &FindUnusedSongFilesConfig {
                                extensions: extensions
                                    .unwrap_or_else(|| Some(vec![".mp3".to_owned()])),
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
    cfg: &FindUnusedSongFilesConfig,
) -> Vec<PathBuf> {
    let mut files = vec![];
    find_unused_song_files_internal(db, path, &"", cfg, &mut files, &mut vec![], true);
    files
}

struct FindUnusedSongFilesConfig {
    extensions: Option<Vec<String>>,
}

fn find_unused_song_files_internal(
    db: &Arc<Mutex<Database>>,
    path: &impl AsRef<Path>,
    rel_path: &impl AsRef<Path>,
    cfg: &FindUnusedSongFilesConfig,
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
        for song in db.songs().values() {
            if let Some(i) = files_buf
                .iter()
                .position(|path| path == &song.location.rel_path)
            {
                files_buf.remove(i);
            }
        }
        unused_files.extend(std::mem::replace(files_buf, vec![]).into_iter());
    }
}
