use std::{
    fmt::{Debug, Display},
    sync::{Arc, Mutex, RwLock},
};

use mers_lib::{
    data::{self, Data, MersData, MersType, Type},
    info::Info,
    prelude_extend_config::Config,
};
use musicdb_lib::{
    data::{
        album::Album,
        artist::Artist,
        database::Database,
        queue::{Queue, QueueContent},
        song::Song,
    },
    server::Command,
};

pub fn add(
    cfg: Config,
    db: &Arc<Mutex<Database>>,
    cmd: &Arc<impl Fn(Command) + Sync + Send + 'static>,
) -> Config {
    macro_rules! func {
        ($out:expr, $run:expr) => {
            Data::new(data::function::Function {
                info: Arc::new(Info::neverused()),
                info_check: Arc::new(Mutex::new(Info::neverused())),
                out: Arc::new($out),
                run: Arc::new($run),
                inner_statements: None,
            })
        };
    }
    cfg.with_list()
        .add_type(MusicDbIdT.to_string(), Ok(Arc::new(MusicDbIdT)))
        .add_var(
            "queue_get_current_song".to_owned(),
            func!(
                |a, _| {
                    if a.is_included_in(&Type::empty_tuple()) {
                        Ok(Type::newm(vec![
                            Arc::new(MusicDbIdT),
                            Arc::new(data::tuple::TupleT(vec![])),
                        ]))
                    } else {
                        Err(format!("Function argument must be `()`.").into())
                    }
                },
                {
                    let db = Arc::clone(db);
                    move |_, _| match db.lock().unwrap().queue.get_current_song() {
                        Some(id) => Data::new(MusicDbId(*id)),
                        None => Data::empty_tuple(),
                    }
                }
            ),
        )
        .add_var(
            "queue_get_next_song".to_owned(),
            func!(
                |a, _| {
                    if a.is_included_in(&Type::empty_tuple()) {
                        Ok(Type::newm(vec![
                            Arc::new(MusicDbIdT),
                            Arc::new(data::tuple::TupleT(vec![])),
                        ]))
                    } else {
                        Err(format!("Function argument must be `()`.").into())
                    }
                },
                {
                    let db = Arc::clone(db);
                    move |_, _| match db.lock().unwrap().queue.get_next_song() {
                        Some(id) => Data::new(MusicDbId(*id)),
                        None => Data::empty_tuple(),
                    }
                }
            ),
        )
        .add_var(
            "queue_get_elem".to_owned(),
            func!(
                |a, _| {
                    if a.is_included_in(&mers_lib::program::configs::with_list::ListT(Type::new(
                        data::int::IntT,
                    ))) {
                        Ok(gen_queue_elem_type())
                    } else {
                        Err(format!("Function argument must be `List<Int>`.").into())
                    }
                },
                {
                    let db = Arc::clone(db);
                    move |a, _| {
                        let a = int_list_to_usize_vec(&a);
                        if let Some(elem) = db.lock().unwrap().queue.get_item_at_index(&a, 0) {
                            gen_queue_elem(elem)
                        } else {
                            Data::empty_tuple()
                        }
                    }
                }
            ),
        )
        .add_var(
            "queue_add_song".to_owned(),
            func!(
                |a, _| {
                    if a.is_included_in(&data::tuple::TupleT(vec![
                        Type::new(mers_lib::program::configs::with_list::ListT(Type::new(
                            data::int::IntT,
                        ))),
                        Type::new(MusicDbIdT),
                    ])) {
                        Ok(Type::empty_tuple())
                    } else {
                        Err(format!("Function argument must be `(List<Int>, MusicDbId)`.").into())
                    }
                },
                {
                    let cmd = cmd.clone();
                    move |a, _| {
                        let a = a.get();
                        let a = &a.as_any().downcast_ref::<data::tuple::Tuple>().unwrap().0;
                        let path = int_list_to_usize_vec(&a[0]);
                        let song_id = a[1].get().as_any().downcast_ref::<MusicDbId>().unwrap().0;
                        cmd(Command::QueueAdd(
                            path,
                            vec![QueueContent::Song(song_id).into()],
                        ));
                        Data::empty_tuple()
                    }
                }
            ),
        )
        .add_var(
            "all_songs".to_owned(),
            func!(
                |a, _| {
                    if a.is_zero_tuple() {
                        Ok(Type::new(mers_lib::program::configs::with_list::ListT(
                            gen_song_type(),
                        )))
                    } else {
                        Err(format!("Function argument must be `()`.").into())
                    }
                },
                {
                    let db = Arc::clone(db);
                    move |_, _| {
                        Data::new(mers_lib::program::configs::with_list::List(
                            db.lock()
                                .unwrap()
                                .songs()
                                .values()
                                .map(|s| Arc::new(RwLock::new(gen_song(s))))
                                .collect(),
                        ))
                    }
                }
            ),
        )
        .add_var(
            "get_song".to_owned(),
            func!(
                |a, _| {
                    if a.is_included_in(&MusicDbIdT) {
                        Ok(Type::newm(vec![
                            Arc::new(gen_song_type()),
                            Arc::new(data::tuple::TupleT(vec![])),
                        ]))
                    } else {
                        Err(format!("Function argument must be `MusicDbId`.").into())
                    }
                },
                {
                    let db = Arc::clone(db);
                    move |a, _| {
                        let id = a.get().as_any().downcast_ref::<MusicDbId>().unwrap().0;
                        match db.lock().unwrap().get_song(&id) {
                            Some(song) => gen_song(song),
                            None => Data::empty_tuple(),
                        }
                    }
                }
            ),
        )
        .add_var(
            "get_album".to_owned(),
            func!(
                |a, _| {
                    if a.is_included_in(&MusicDbIdT) {
                        Ok(Type::newm(vec![
                            Arc::new(gen_album_type()),
                            Arc::new(data::tuple::TupleT(vec![])),
                        ]))
                    } else {
                        Err(format!("Function argument must be `MusicDbId`.").into())
                    }
                },
                {
                    let db = Arc::clone(db);
                    move |a, _| {
                        let id = a.get().as_any().downcast_ref::<MusicDbId>().unwrap().0;
                        match db.lock().unwrap().albums().get(&id) {
                            Some(album) => gen_album(album),
                            None => Data::empty_tuple(),
                        }
                    }
                }
            ),
        )
        .add_var(
            "get_artist".to_owned(),
            func!(
                |a, _| {
                    if a.is_included_in(&MusicDbIdT) {
                        Ok(Type::newm(vec![
                            Arc::new(gen_artist_type()),
                            Arc::new(data::tuple::TupleT(vec![])),
                        ]))
                    } else {
                        Err(format!("Function argument must be `MusicDbId`.").into())
                    }
                },
                {
                    let db = Arc::clone(db);
                    move |a, _| {
                        let id = a.get().as_any().downcast_ref::<MusicDbId>().unwrap().0;
                        match db.lock().unwrap().artists().get(&id) {
                            Some(artist) => gen_artist(artist),
                            None => Data::empty_tuple(),
                        }
                    }
                }
            ),
        )
        .add_var(
            "get_song_tags".to_owned(),
            func!(
                |a, _| {
                    if a.is_included_in(&MusicDbIdT) {
                        Ok(Type::newm(vec![
                            Arc::new(mers_lib::program::configs::with_list::ListT(Type::new(
                                data::string::StringT,
                            ))),
                            Arc::new(data::tuple::TupleT(vec![])),
                        ]))
                    } else {
                        Err(format!("Function argument must be `MusicDbId`.").into())
                    }
                },
                {
                    let db = Arc::clone(db);
                    move |a, _| {
                        let id = a.get().as_any().downcast_ref::<MusicDbId>().unwrap().0;
                        match db.lock().unwrap().get_song(&id) {
                            Some(song) => Data::new(mers_lib::program::configs::with_list::List(
                                song.general
                                    .tags
                                    .iter()
                                    .map(|t| {
                                        Arc::new(RwLock::new(Data::new(data::string::String(
                                            t.clone(),
                                        ))))
                                    })
                                    .collect(),
                            )),
                            None => Data::empty_tuple(),
                        }
                    }
                }
            ),
        )
        .add_var(
            "get_album_tags".to_owned(),
            func!(
                |a, _| {
                    if a.is_included_in(&MusicDbIdT) {
                        Ok(Type::newm(vec![
                            Arc::new(mers_lib::program::configs::with_list::ListT(Type::new(
                                data::string::StringT,
                            ))),
                            Arc::new(data::tuple::TupleT(vec![])),
                        ]))
                    } else {
                        Err(format!("Function argument must be `MusicDbId`.").into())
                    }
                },
                {
                    let db = Arc::clone(db);
                    move |a, _| {
                        let id = a.get().as_any().downcast_ref::<MusicDbId>().unwrap().0;
                        match db.lock().unwrap().albums().get(&id) {
                            Some(album) => Data::new(mers_lib::program::configs::with_list::List(
                                album
                                    .general
                                    .tags
                                    .iter()
                                    .map(|t| {
                                        Arc::new(RwLock::new(Data::new(data::string::String(
                                            t.clone(),
                                        ))))
                                    })
                                    .collect(),
                            )),
                            None => Data::empty_tuple(),
                        }
                    }
                }
            ),
        )
        .add_var(
            "get_artist_tags".to_owned(),
            func!(
                |a, _| {
                    if a.is_included_in(&MusicDbIdT) {
                        Ok(Type::newm(vec![
                            Arc::new(mers_lib::program::configs::with_list::ListT(Type::new(
                                data::string::StringT,
                            ))),
                            Arc::new(data::tuple::TupleT(vec![])),
                        ]))
                    } else {
                        Err(format!("Function argument must be `MusicDbId`.").into())
                    }
                },
                {
                    let db = Arc::clone(db);
                    move |a, _| {
                        let id = a.get().as_any().downcast_ref::<MusicDbId>().unwrap().0;
                        match db.lock().unwrap().artists().get(&id) {
                            Some(artist) => Data::new(mers_lib::program::configs::with_list::List(
                                artist
                                    .general
                                    .tags
                                    .iter()
                                    .map(|t| {
                                        Arc::new(RwLock::new(Data::new(data::string::String(
                                            t.clone(),
                                        ))))
                                    })
                                    .collect(),
                            )),
                            None => Data::empty_tuple(),
                        }
                    }
                }
            ),
        )
}

fn gen_song_type() -> Type {
    Type::new(data::object::ObjectT(vec![
        ("id".to_owned(), Type::new(MusicDbIdT)),
        ("title".to_owned(), Type::new(data::string::StringT)),
        (
            "album".to_owned(),
            Type::newm(vec![
                Arc::new(MusicDbIdT),
                Arc::new(data::tuple::TupleT(vec![])),
            ]),
        ),
        ("artist".to_owned(), Type::new(MusicDbIdT)),
        (
            "cover".to_owned(),
            Type::newm(vec![
                Arc::new(MusicDbIdT),
                Arc::new(data::tuple::TupleT(vec![])),
            ]),
        ),
    ]))
}
fn gen_song(song: &Song) -> Data {
    Data::new(data::object::Object(vec![
        ("id".to_owned(), Data::new(MusicDbId(song.id))),
        (
            "title".to_owned(),
            Data::new(data::string::String(song.title.clone())),
        ),
        (
            "album".to_owned(),
            if let Some(album) = song.album {
                Data::new(MusicDbId(album))
            } else {
                Data::empty_tuple()
            },
        ),
        ("artist".to_owned(), Data::new(MusicDbId(song.artist))),
        (
            "cover".to_owned(),
            if let Some(cover) = song.cover {
                Data::new(MusicDbId(cover))
            } else {
                Data::empty_tuple()
            },
        ),
    ]))
}
fn gen_album_type() -> Type {
    Type::new(data::object::ObjectT(vec![
        ("id".to_owned(), Type::new(MusicDbIdT)),
        ("name".to_owned(), Type::new(data::string::StringT)),
        ("artist".to_owned(), Type::new(MusicDbIdT)),
        (
            "cover".to_owned(),
            Type::newm(vec![
                Arc::new(MusicDbIdT),
                Arc::new(data::tuple::TupleT(vec![])),
            ]),
        ),
        (
            "songs".to_owned(),
            Type::new(mers_lib::program::configs::with_list::ListT(Type::new(
                MusicDbIdT,
            ))),
        ),
    ]))
}
fn gen_album(album: &Album) -> Data {
    Data::new(data::object::Object(vec![
        ("id".to_owned(), Data::new(MusicDbId(album.id))),
        (
            "name".to_owned(),
            Data::new(data::string::String(album.name.clone())),
        ),
        ("artist".to_owned(), Data::new(MusicDbId(album.artist))),
        (
            "cover".to_owned(),
            if let Some(cover) = album.cover {
                Data::new(MusicDbId(cover))
            } else {
                Data::empty_tuple()
            },
        ),
        (
            "songs".to_owned(),
            Data::new(mers_lib::program::configs::with_list::List(
                album
                    .songs
                    .iter()
                    .map(|id| Arc::new(RwLock::new(Data::new(MusicDbId(*id)))))
                    .collect(),
            )),
        ),
    ]))
}
fn gen_artist_type() -> Type {
    Type::new(data::object::ObjectT(vec![
        ("id".to_owned(), Type::new(MusicDbIdT)),
        ("name".to_owned(), Type::new(data::string::StringT)),
        (
            "cover".to_owned(),
            Type::newm(vec![
                Arc::new(MusicDbIdT),
                Arc::new(data::tuple::TupleT(vec![])),
            ]),
        ),
        (
            "albums".to_owned(),
            Type::new(mers_lib::program::configs::with_list::ListT(Type::new(
                MusicDbIdT,
            ))),
        ),
        (
            "singles".to_owned(),
            Type::new(mers_lib::program::configs::with_list::ListT(Type::new(
                MusicDbIdT,
            ))),
        ),
    ]))
}
fn gen_artist(artist: &Artist) -> Data {
    Data::new(data::object::Object(vec![
        ("id".to_owned(), Data::new(MusicDbId(artist.id))),
        (
            "name".to_owned(),
            Data::new(data::string::String(artist.name.clone())),
        ),
        (
            "cover".to_owned(),
            if let Some(cover) = artist.cover {
                Data::new(MusicDbId(cover))
            } else {
                Data::empty_tuple()
            },
        ),
        (
            "albums".to_owned(),
            Data::new(mers_lib::program::configs::with_list::List(
                artist
                    .albums
                    .iter()
                    .map(|id| Arc::new(RwLock::new(Data::new(MusicDbId(*id)))))
                    .collect(),
            )),
        ),
        (
            "singles".to_owned(),
            Data::new(mers_lib::program::configs::with_list::List(
                artist
                    .singles
                    .iter()
                    .map(|id| Arc::new(RwLock::new(Data::new(MusicDbId(*id)))))
                    .collect(),
            )),
        ),
    ]))
}

fn gen_queue_elem_type() -> Type {
    Type::newm(vec![
        Arc::new(data::tuple::TupleT(vec![])),
        Arc::new(data::object::ObjectT(vec![
            ("enabled".to_owned(), Type::new(data::bool::BoolT)),
            ("song".to_owned(), Type::new(MusicDbIdT)),
        ])),
        Arc::new(data::object::ObjectT(vec![
            ("enabled".to_owned(), Type::new(data::bool::BoolT)),
            (
                "loop".to_owned(),
                Type::new(data::object::ObjectT(vec![
                    ("total".to_owned(), Type::new(data::int::IntT)),
                    ("done".to_owned(), Type::new(data::int::IntT)),
                ])),
            ),
        ])),
        Arc::new(data::object::ObjectT(vec![
            ("enabled".to_owned(), Type::new(data::bool::BoolT)),
            ("random".to_owned(), Type::empty_tuple()),
        ])),
        Arc::new(data::object::ObjectT(vec![
            ("enabled".to_owned(), Type::new(data::bool::BoolT)),
            (
                "folder".to_owned(),
                Type::new(data::object::ObjectT(vec![
                    ("index".to_owned(), Type::new(data::int::IntT)),
                    ("length".to_owned(), Type::new(data::int::IntT)),
                    ("name".to_owned(), Type::new(data::string::StringT)),
                ])),
            ),
        ])),
        Arc::new(data::object::ObjectT(vec![
            ("enabled".to_owned(), Type::new(data::bool::BoolT)),
            ("shuffle".to_owned(), Type::empty_tuple()),
        ])),
    ])
}
fn gen_queue_elem(queue_elem: &Queue) -> Data {
    Data::new(data::object::Object(vec![
        (
            "enabled".to_owned(),
            Data::new(data::bool::Bool(queue_elem.enabled())),
        ),
        match queue_elem.content() {
            QueueContent::Song(id) => ("song".to_owned(), Data::new(MusicDbId(*id))),
            QueueContent::Loop(total, done, _inner) => (
                "loop".to_owned(),
                Data::new(data::object::Object(vec![
                    ("total".to_owned(), Data::new(data::int::Int(*total as _))),
                    ("done".to_owned(), Data::new(data::int::Int(*done as _))),
                ])),
            ),
            QueueContent::Random(_) => ("random".to_owned(), Data::empty_tuple()),
            QueueContent::Folder(index, inner, name) => (
                "folder".to_owned(),
                Data::new(data::object::Object(vec![
                    ("index".to_owned(), Data::new(data::int::Int(*index as _))),
                    (
                        "length".to_owned(),
                        Data::new(data::int::Int(inner.len() as _)),
                    ),
                    (
                        "name".to_owned(),
                        Data::new(data::string::String(name.clone())),
                    ),
                ])),
            ),
            QueueContent::Shuffle { inner: _, state: _ } => {
                ("shuffle".to_owned(), Data::empty_tuple())
            }
        },
    ]))
}

fn int_list_to_usize_vec(a: &Data) -> Vec<usize> {
    a.get()
        .as_any()
        .downcast_ref::<mers_lib::program::configs::with_list::List>()
        .unwrap()
        .0
        .iter()
        .map(|v| {
            v.read()
                .unwrap()
                .get()
                .as_any()
                .downcast_ref::<data::int::Int>()
                .unwrap()
                .0
                .abs() as usize
        })
        .collect()
}

#[derive(Clone, Copy)]
pub struct MusicDbId(u64);

#[derive(Clone, Copy)]
pub struct MusicDbIdT;

impl MersData for MusicDbId {
    fn as_type(&self) -> Type {
        Type::new(MusicDbIdT)
    }
    fn is_eq(&self, other: &dyn MersData) -> bool {
        if let Some(other) = other.as_any().downcast_ref::<Self>() {
            self.0 == other.0
        } else {
            false
        }
    }
    fn clone(&self) -> Box<dyn MersData> {
        Box::new(*self)
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn mut_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn to_any(self) -> Box<dyn std::any::Any> {
        Box::new(self)
    }
}
impl MersType for MusicDbIdT {
    fn is_same_type_as(&self, other: &dyn MersType) -> bool {
        other.as_any().is::<Self>()
    }
    fn is_included_in_single(&self, target: &dyn MersType) -> bool {
        target.as_any().is::<Self>()
    }
    fn subtypes(&self, acc: &mut Type) {
        acc.add(Arc::new(*self))
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn mut_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn to_any(self) -> Box<dyn std::any::Any> {
        Box::new(self)
    }
}

impl Display for MusicDbId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl Debug for MusicDbId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self}")
    }
}
impl Display for MusicDbIdT {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MusicDbId")
    }
}
impl Debug for MusicDbIdT {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self}")
    }
}
