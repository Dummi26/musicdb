use std::{
    io::{BufRead, BufReader, Read, Write},
    sync::{mpsc, Arc, Mutex},
};

use crate::data::database::Database;

use super::Command;

pub fn handle_main<T: Read + Write>(
    db: Arc<Mutex<Database>>,
    con: &mut BufReader<T>,
    command_sender: &mpsc::Sender<Command>,
) -> Result<(), std::io::Error> {
    loop {
        let mut command = String::new();
        con.read_line(&mut command)?;
        let command = command.trim();
        let (command, args) = command.split_once(' ').unwrap_or((command, ""));
        match command {
            "goodbye" => return Ok(()),
            "list-artists" => {
                for (id, artist) in db.lock().unwrap().artists().iter() {
                    writeln!(con.get_mut(), "#{id}:{}", artist.name)?;
                }
                writeln!(con.get_mut(), "---")?;
            }
            "list-albums" => {
                for (id, album) in db.lock().unwrap().albums().iter() {
                    writeln!(con.get_mut(), "#{id}:{}", album.name)?;
                }
                writeln!(con.get_mut(), "---")?;
            }
            "list-songs" => {
                for (id, song) in db.lock().unwrap().songs().iter() {
                    writeln!(con.get_mut(), "#{id}:{}", song.title)?;
                }
                writeln!(con.get_mut(), "---")?;
            }
            _ => writeln!(con.get_mut(), "err: no such command")?,
        }
    }
}
