use std::{collections::HashSet, io::Write, net::TcpStream};

use musicdb_lib::{
    data::{
        database::Database,
        queue::{Queue, QueueContent},
        SongId,
    },
    load::ToFromBytes,
    server::{Action, Command},
};

fn main() {
    let mut con = TcpStream::connect(
        std::env::args()
            .nth(1)
            .expect("required argument: server address and port"),
    )
    .unwrap();
    writeln!(con, "main").unwrap();
    let mut db = Database::new_clientside();
    while !db.is_client_init() {
        db.apply_action_unchecked_seq(Command::from_bytes(&mut con).unwrap().action, None);
    }
    let mut actions = vec![];
    rev_actions(&mut actions, &db.queue, &mut vec![], &mut HashSet::new());
    actions.reverse();
    eprintln!("Removing {} queue elements", actions.len());
    db.seq
        .pack(Action::Multiple(actions))
        .to_bytes(&mut con)
        .unwrap();
}

fn rev_actions(
    actions: &mut Vec<Action>,
    queue: &Queue,
    path: &mut Vec<usize>,
    seen: &mut HashSet<SongId>,
) {
    match queue.content() {
        QueueContent::Song(id) => {
            if seen.contains(id) {
                actions.push(Action::QueueRemove(path.clone()));
            } else {
                seen.insert(*id);
            }
        }
        QueueContent::Folder(folder) => {
            for (i, queue) in folder.iter().enumerate() {
                path.push(i);
                rev_actions(actions, queue, path, seen);
                path.pop();
            }
        }
        QueueContent::Loop(_, _, inner) => {
            path.push(0);
            rev_actions(actions, &*inner, path, seen);
            path.pop();
        }
    }
}
