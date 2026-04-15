use std::{io::Write, net::TcpStream};

use musicdb_lib::{
    data::{
        database::Database,
        queue::{QueueContent, QueueFolder},
    },
    load::ToFromBytes,
    server::{Action, Command, Req},
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
    db.seq
        .pack(Action::Multiple(vec![
            Action::QueueUpdate(
                vec![],
                QueueContent::Folder(QueueFolder {
                    content: db
                        .songs()
                        .keys()
                        .map(|id| QueueContent::Song(*id).into())
                        .collect(),
                    ..Default::default()
                })
                .into(),
                Req::none(),
            ),
            Action::QueueShuffle(vec![], 0),
        ]))
        .to_bytes(&mut con)
        .unwrap();
}
