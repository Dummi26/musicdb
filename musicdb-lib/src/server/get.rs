use std::{
    fs,
    io::BufRead,
    io::{BufReader, Read, Write},
    path::Path,
    sync::{Arc, Mutex},
};

use crate::data::{database::Database, CoverId, SongId};

pub struct Client<T: Write + Read>(BufReader<T>);
impl<T: Write + Read> Client<T> {
    pub fn new(mut con: BufReader<T>) -> std::io::Result<Self> {
        writeln!(con.get_mut(), "get")?;
        Ok(Self(con))
    }
    pub fn cover_bytes(&mut self, id: CoverId) -> Result<Result<Vec<u8>, String>, std::io::Error> {
        writeln!(
            self.0.get_mut(),
            "{}",
            con_get_encode_string(&format!("cover-bytes\n{id}"))
        )?;
        let mut response = String::new();
        self.0.read_line(&mut response)?;
        let response = con_get_decode_line(&response);
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
        let mut response = String::new();
        self.0.read_line(&mut response)?;
        let response = con_get_decode_line(&response);
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
        let mut response = String::new();
        self.0.read_line(&mut response)?;
        let response = con_get_decode_line(&response);
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
}

pub fn handle_one_connection_as_get(
    db: Arc<Mutex<Database>>,
    connection: &mut BufReader<impl Read + Write>,
) -> Result<(), std::io::Error> {
    let mut line = String::new();
    loop {
        line.clear();
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
                                fs::read(parent.join(path)).ok()
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
