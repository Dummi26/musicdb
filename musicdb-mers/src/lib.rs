use std::{
    fmt::{Debug, Display},
    sync::{Arc, Mutex, RwLock},
};

pub use mers_lib;

use mers_lib::{
    data::{self, function::Function, Data, MersData, MersType, Type},
    prelude_extend_config::Config,
};
use musicdb_lib::{
    data::{
        album::Album,
        artist::Artist,
        database::Database,
        queue::{Queue, QueueContent, QueueFolder},
        song::Song,
    },
    server::Command,
};

pub fn add(
    mut cfg: Config,
    db: &Arc<Mutex<Database>>,
    cmd: &Arc<impl Fn(Command) + Sync + Send + 'static>,
    after_db_cmd: &Arc<Mutex<Option<Box<dyn FnMut(Command) + Send + Sync + 'static>>>>,
) -> Config {
    /// handle commands received from server (for handler functions)
    /// `T` can be used to return generated data to avoid calculating something twice if one event may call multiple handlers.
    fn handle<T>(
        handler: &Arc<RwLock<Data>>,
        gen: impl FnOnce() -> (Data, T),
    ) -> Option<(T, Data)> {
        if let Some(func) = handler
            .read()
            .unwrap()
            .get()
            .as_any()
            .downcast_ref::<Function>()
        {
            let (data, t) = gen();
            Some((t, func.run_immut(data).ok()?))
        } else {
            None
        }
    }
    let handler_resume = Arc::new(RwLock::new(Data::empty_tuple()));
    let handler_pause = Arc::new(RwLock::new(Data::empty_tuple()));
    let handler_next_song = Arc::new(RwLock::new(Data::empty_tuple()));
    let handler_queue_changed = Arc::new(RwLock::new(Data::empty_tuple()));
    let handler_library_changed = Arc::new(RwLock::new(Data::empty_tuple()));
    let handler_notification_received = Arc::new(RwLock::new(Data::empty_tuple()));
    {
        *after_db_cmd.lock().unwrap() = Some({
            let handler_resume = Arc::clone(&handler_resume);
            let handler_pause = Arc::clone(&handler_pause);
            let handler_next_song = Arc::clone(&handler_next_song);
            let handler_queue_changed = Arc::clone(&handler_queue_changed);
            let handler_library_changed = Arc::clone(&handler_library_changed);
            let handler_notification_received = Arc::clone(&handler_notification_received);
            Box::new(move |cmd| match cmd {
                Command::Resume => {
                    handle(&handler_resume, move || (Data::empty_tuple(), ()));
                }
                Command::Pause | Command::Stop => {
                    handle(&handler_pause, move || (Data::empty_tuple(), ()));
                }
                Command::NextSong => {
                    handle(&handler_next_song, move || (Data::empty_tuple(), ()));
                    handle(&handler_queue_changed, move || (Data::empty_tuple(), ()));
                }
                Command::SyncDatabase(..) => {
                    handle(&handler_library_changed, move || (Data::empty_tuple(), ()));
                }
                Command::QueueUpdate(..)
                | Command::QueueAdd(..)
                | Command::QueueInsert(..)
                | Command::QueueRemove(..)
                | Command::QueueMove(..)
                | Command::QueueMoveInto(..)
                | Command::QueueGoto(..)
                | Command::QueueShuffle(..)
                | Command::QueueSetShuffle(..)
                | Command::QueueUnshuffle(..) => {
                    handle(&handler_queue_changed, move || (Data::empty_tuple(), ()));
                }
                Command::AddSong(_)
                | Command::AddAlbum(_)
                | Command::AddArtist(_)
                | Command::AddCover(_)
                | Command::ModifySong(_)
                | Command::ModifyAlbum(_)
                | Command::ModifyArtist(_)
                | Command::RemoveSong(_)
                | Command::RemoveAlbum(_)
                | Command::RemoveArtist(_) => {
                    handle(&handler_library_changed, move || (Data::empty_tuple(), ()));
                }
                Command::SetSongDuration(..) => {
                    handle(&handler_library_changed, move || (Data::empty_tuple(), ()));
                }
                Command::TagSongFlagSet(..)
                | Command::TagSongFlagUnset(..)
                | Command::TagAlbumFlagSet(..)
                | Command::TagAlbumFlagUnset(..)
                | Command::TagArtistFlagSet(..)
                | Command::TagArtistFlagUnset(..)
                | Command::TagSongPropertySet(..)
                | Command::TagSongPropertyUnset(..)
                | Command::TagAlbumPropertySet(..)
                | Command::TagAlbumPropertyUnset(..)
                | Command::TagArtistPropertySet(..)
                | Command::TagArtistPropertyUnset(..) => {
                    handle(&handler_library_changed, move || (Data::empty_tuple(), ()));
                }
                Command::InitComplete => (),
                Command::Save => (),
                Command::ErrorInfo(title, body) => {
                    handle(&handler_notification_received, move || {
                        (
                            Data::new(data::tuple::Tuple(vec![
                                Data::new(data::string::String(title)),
                                Data::new(data::string::String(body)),
                            ])),
                            (),
                        )
                    });
                }
            })
        });
    }
    // MusicDb type
    cfg = cfg
        .with_list()
        .add_type(MusicDbIdT.to_string(), Ok(Arc::new(Type::new(MusicDbIdT))));
    // handler setters
    for (name, handler, in_type) in [
        ("resume", handler_resume, Type::empty_tuple()),
        ("pause", handler_pause, Type::empty_tuple()),
        ("next_song", handler_next_song, Type::empty_tuple()),
        (
            "library_changed",
            handler_library_changed,
            Type::empty_tuple(),
        ),
        ("queue_changed", handler_queue_changed, Type::empty_tuple()),
        (
            "notification_received",
            handler_notification_received,
            Type::new(data::tuple::TupleT(vec![
                Type::new(data::string::StringT),
                Type::new(data::string::StringT),
            ])),
        ),
    ] {
        cfg = cfg.add_var(
            format!("handle_event_{name}"),
            Function::new_generic(
                move |a| {
                    if a.types.iter().all(|a| {
                        Type::newm(vec![Arc::clone(a)]).is_zero_tuple()
                            || a.as_any()
                                .downcast_ref::<data::function::FunctionT>()
                                .is_some_and(|a| a.o(&in_type).is_ok_and(|opt| opt.is_zero_tuple()))
                    }) {
                        Ok(Type::empty_tuple())
                    } else {
                        Err(format!("Handler function must be `{in_type} -> ()`").into())
                    }
                },
                move |a, _| {
                    *handler.write().unwrap() = a;
                    Ok(Data::empty_tuple())
                },
            ),
        );
    }
    // actions
    cfg.add_var(
        "send_server_notification".to_owned(),
        Function::new_generic(
            |a| {
                if a.is_included_in_single(&data::string::StringT) {
                    Ok(Type::empty_tuple())
                } else {
                    Err(format!("Function argument must be `String`.").into())
                }
            },
            {
                let cmd = Arc::clone(cmd);
                move |a, _| {
                    cmd(Command::ErrorInfo(
                        String::new(),
                        a.get()
                            .as_any()
                            .downcast_ref::<data::string::String>()
                            .unwrap()
                            .0
                            .clone(),
                    ));
                    Ok(Data::empty_tuple())
                }
            },
        ),
    )
    .add_var(
        "resume".to_owned(),
        Function::new_generic(
            |a| {
                if a.is_included_in(&Type::empty_tuple()) {
                    Ok(Type::empty_tuple())
                } else {
                    Err(format!("Function argument must be `()`.").into())
                }
            },
            {
                let cmd = Arc::clone(cmd);
                move |_, _| {
                    cmd(Command::Resume);
                    Ok(Data::empty_tuple())
                }
            },
        ),
    )
    .add_var(
        "pause".to_owned(),
        Function::new_generic(
            |a| {
                if a.is_included_in(&Type::empty_tuple()) {
                    Ok(Type::empty_tuple())
                } else {
                    Err(format!("Function argument must be `()`.").into())
                }
            },
            {
                let cmd = Arc::clone(cmd);
                move |_, _| {
                    cmd(Command::Pause);
                    Ok(Data::empty_tuple())
                }
            },
        ),
    )
    .add_var(
        "stop".to_owned(),
        Function::new_generic(
            |a| {
                if a.is_included_in(&Type::empty_tuple()) {
                    Ok(Type::empty_tuple())
                } else {
                    Err(format!("Function argument must be `()`.").into())
                }
            },
            {
                let cmd = Arc::clone(cmd);
                move |_, _| {
                    cmd(Command::Stop);
                    Ok(Data::empty_tuple())
                }
            },
        ),
    )
    .add_var(
        "next_song".to_owned(),
        Function::new_generic(
            |a| {
                if a.is_included_in(&Type::empty_tuple()) {
                    Ok(Type::empty_tuple())
                } else {
                    Err(format!("Function argument must be `()`.").into())
                }
            },
            {
                let cmd = Arc::clone(cmd);
                move |_, _| {
                    cmd(Command::NextSong);
                    Ok(Data::empty_tuple())
                }
            },
        ),
    )
    .add_var(
        "get_playing".to_owned(),
        Function::new_generic(
            |a| {
                if a.is_included_in(&Type::empty_tuple()) {
                    Ok(data::bool::bool_type())
                } else {
                    Err(format!("Function argument must be `()`.").into())
                }
            },
            {
                let db = Arc::clone(db);
                move |_, _| Ok(Data::new(data::bool::Bool(db.lock().unwrap().playing)))
            },
        ),
    )
    .add_var(
        "queue_get_current_song".to_owned(),
        Function::new_generic(
            |a| {
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
                move |_, _| {
                    Ok(match db.lock().unwrap().queue.get_current_song() {
                        Some(id) => Data::new(MusicDbId(*id)),
                        None => Data::empty_tuple(),
                    })
                }
            },
        ),
    )
    .add_var(
        "queue_get_next_song".to_owned(),
        Function::new_generic(
            |a| {
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
                move |_, _| {
                    Ok(match db.lock().unwrap().queue.get_next_song() {
                        Some(id) => Data::new(MusicDbId(*id)),
                        None => Data::empty_tuple(),
                    })
                }
            },
        ),
    )
    .add_var(
        "queue_get_elem".to_owned(),
        Function::new_generic(
            |a| {
                if a.is_included_in_single(&mers_lib::program::configs::with_list::ListT(
                    Type::new(data::int::IntT),
                )) {
                    Ok(gen_queue_elem_type_or_empty_tuple())
                } else {
                    Err(format!("Function argument must be `List<Int>`.").into())
                }
            },
            {
                let db = Arc::clone(db);
                move |a, _| {
                    let a = int_list_to_usize_vec(&a);
                    Ok(
                        if let Some(elem) = db.lock().unwrap().queue.get_item_at_index(&a, 0) {
                            gen_queue_elem(elem)
                        } else {
                            Data::empty_tuple()
                        },
                    )
                }
            },
        ),
    )
    .add_var(
        "queue_goto".to_owned(),
        Function::new_generic(
            |a| {
                if a.is_included_in_single(&mers_lib::program::configs::with_list::ListT(
                    Type::new(data::int::IntT),
                )) {
                    Ok(Type::empty_tuple())
                } else {
                    Err(format!("Function argument must be `List<Int>`.").into())
                }
            },
            {
                let cmd = Arc::clone(cmd);
                move |a, _| {
                    cmd(Command::QueueGoto(int_list_to_usize_vec(&a)));
                    Ok(Data::empty_tuple())
                }
            },
        ),
    )
    .add_var(
        "queue_clear".to_owned(),
        Function::new_generic(
            |a| {
                if a.is_included_in(&Type::empty_tuple()) {
                    Ok(Type::empty_tuple())
                } else {
                    Err(format!("Function argument must be `()`.").into())
                }
            },
            {
                let cmd = Arc::clone(cmd);
                move |_, _| {
                    cmd(Command::QueueUpdate(
                        vec![],
                        QueueContent::Folder(QueueFolder::default()).into(),
                    ));
                    Ok(Data::empty_tuple())
                }
            },
        ),
    )
    // TODO: `queue_add`, which takes any queue element as defined in `gen_queue_elem_type`
    // .add_var(
    //     "queue_add_song".to_owned(),
    //     Function::new_generic(
    //         |a| {
    //             if a.is_included_in_single(&data::tuple::TupleT(vec![
    //                 Type::new(mers_lib::program::configs::with_list::ListT(Type::new(
    //                     data::int::IntT,
    //                 ))),
    //                 Type::new(MusicDbIdT),
    //             ])) {
    //                 Ok(Type::empty_tuple())
    //             } else {
    //                 Err(format!("Function argument must be `(List<Int>, MusicDbId)`.").into())
    //             }
    //         },
    //         {
    //             let cmd = Arc::clone(cmd);
    //             move |a, _| {
    //                 let a = a.get();
    //                 let a = &a.as_any().downcast_ref::<data::tuple::Tuple>().unwrap().0;
    //                 let path = int_list_to_usize_vec(&a[0]);
    //                 let song_id = a[1].get().as_any().downcast_ref::<MusicDbId>().unwrap().0;
    //                 cmd(Command::QueueAdd(
    //                     path,
    //                     vec![QueueContent::Song(song_id).into()],
    //                 ));
    //                 Ok(Data::empty_tuple())
    //             }
    //         },
    //     ),
    // )
    // .add_var(
    //     "queue_add_loop".to_owned(),
    //     Function::new_generic(
    //         |a| {
    //             if a.is_included_in_single(&data::tuple::TupleT(vec![
    //                 Type::new(mers_lib::program::configs::with_list::ListT(Type::new(
    //                     data::int::IntT,
    //                 ))),
    //                 Type::new(data::int::IntT),
    //             ])) {
    //                 Ok(Type::empty_tuple())
    //             } else {
    //                 Err(format!("Function argument must be `(List<Int>, Int)`.").into())
    //             }
    //         },
    //         {
    //             let cmd = Arc::clone(cmd);
    //             move |a, _| {
    //                 let a = a.get();
    //                 let a = &a.as_any().downcast_ref::<data::tuple::Tuple>().unwrap().0;
    //                 let path = int_list_to_usize_vec(&a[0]);
    //                 let repeat_count = a[1]
    //                     .get()
    //                     .as_any()
    //                     .downcast_ref::<data::int::Int>()
    //                     .unwrap()
    //                     .0;
    //                 cmd(Command::QueueAdd(
    //                     path,
    //                     vec![QueueContent::Loop(
    //                         repeat_count.max(0) as _,
    //                         0,
    //                         Box::new(QueueContent::Folder(QueueFolder::default()).into()),
    //                     )
    //                     .into()],
    //                 ));
    //                 Ok(Data::empty_tuple())
    //             }
    //         },
    //     ),
    // )
    // .add_var(
    //     "queue_add_folder".to_owned(),
    //     Function::new_generic(
    //         |a| {
    //             if a.is_included_in_single(&data::tuple::TupleT(vec![
    //                 Type::new(mers_lib::program::configs::with_list::ListT(Type::new(
    //                     data::int::IntT,
    //                 ))),
    //                 Type::new(data::string::StringT),
    //             ])) {
    //                 Ok(Type::empty_tuple())
    //             } else {
    //                 Err(format!("Function argument must be `(List<Int>, String)`.").into())
    //             }
    //         },
    //         {
    //             let cmd = Arc::clone(cmd);
    //             move |a, _| {
    //                 let a = a.get();
    //                 let a = &a.as_any().downcast_ref::<data::tuple::Tuple>().unwrap().0;
    //                 let path = int_list_to_usize_vec(&a[0]);
    //                 let name = a[1]
    //                     .get()
    //                     .as_any()
    //                     .downcast_ref::<data::string::String>()
    //                     .unwrap()
    //                     .0
    //                     .clone();
    //                 cmd(Command::QueueAdd(
    //                     path,
    //                     vec![QueueContent::Folder(QueueFolder {
    //                         index: 0,
    //                         content: vec![],
    //                         name,
    //                         order: None,
    //                     })
    //                     .into()],
    //                 ));
    //                 Ok(Data::empty_tuple())
    //             }
    //         },
    //     ),
    // )
    .add_var(
        "all_songs".to_owned(),
        Function::new_generic(
            |a| {
                if a.is_zero_tuple() {
                    Ok(Type::new(mers_lib::program::configs::with_list::ListT(
                        Type::new(gen_song_type()),
                    )))
                } else {
                    Err(format!("Function argument must be `()`.").into())
                }
            },
            {
                let db = Arc::clone(db);
                move |_, _| {
                    Ok(Data::new(mers_lib::program::configs::with_list::List(
                        db.lock()
                            .unwrap()
                            .songs()
                            .values()
                            .map(|s| Arc::new(RwLock::new(gen_song(s))))
                            .collect(),
                    )))
                }
            },
        ),
    )
    .add_var(
        "get_song".to_owned(),
        Function::new_generic(
            |a| {
                if a.is_included_in_single(&MusicDbIdT) {
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
                    Ok(match db.lock().unwrap().get_song(&id) {
                        Some(song) => gen_song(song),
                        None => Data::empty_tuple(),
                    })
                }
            },
        ),
    )
    .add_var(
        "get_album".to_owned(),
        Function::new_generic(
            |a| {
                if a.is_included_in_single(&MusicDbIdT) {
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
                    Ok(match db.lock().unwrap().albums().get(&id) {
                        Some(album) => gen_album(album),
                        None => Data::empty_tuple(),
                    })
                }
            },
        ),
    )
    .add_var(
        "get_artist".to_owned(),
        Function::new_generic(
            |a| {
                if a.is_included_in_single(&MusicDbIdT) {
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
                    Ok(match db.lock().unwrap().artists().get(&id) {
                        Some(artist) => gen_artist(artist),
                        None => Data::empty_tuple(),
                    })
                }
            },
        ),
    )
    .add_var(
        "get_song_tags".to_owned(),
        Function::new_generic(
            |a| {
                if a.is_included_in_single(&MusicDbIdT) {
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
                    Ok(match db.lock().unwrap().get_song(&id) {
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
                    })
                }
            },
        ),
    )
    .add_var(
        "get_album_tags".to_owned(),
        Function::new_generic(
            |a| {
                if a.is_included_in_single(&MusicDbIdT) {
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
                    Ok(match db.lock().unwrap().albums().get(&id) {
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
                    })
                }
            },
        ),
    )
    .add_var(
        "get_artist_tags".to_owned(),
        Function::new_generic(
            |a| {
                if a.is_included_in_single(&MusicDbIdT) {
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
                    Ok(match db.lock().unwrap().artists().get(&id) {
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
                    })
                }
            },
        ),
    )
}

fn gen_song_type() -> data::object::ObjectT {
    data::object::ObjectT(vec![
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
    ])
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
fn gen_album_type() -> data::object::ObjectT {
    data::object::ObjectT(vec![
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
    ])
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
fn gen_artist_type() -> data::object::ObjectT {
    data::object::ObjectT(vec![
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
    ])
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

fn gen_queue_elem_type_or_empty_tuple() -> Type {
    Type::newm(vec![
        Arc::new(data::tuple::TupleT(vec![])),
        Arc::new(data::object::ObjectT(vec![
            ("enabled".to_owned(), data::bool::bool_type()),
            ("song".to_owned(), Type::new(MusicDbIdT)),
        ])),
        Arc::new(data::object::ObjectT(vec![
            ("enabled".to_owned(), data::bool::bool_type()),
            (
                "loop".to_owned(),
                Type::new(data::object::ObjectT(vec![
                    ("total".to_owned(), Type::new(data::int::IntT)),
                    ("done".to_owned(), Type::new(data::int::IntT)),
                ])),
            ),
        ])),
        Arc::new(data::object::ObjectT(vec![
            ("enabled".to_owned(), data::bool::bool_type()),
            ("random".to_owned(), Type::empty_tuple()),
        ])),
        Arc::new(data::object::ObjectT(vec![
            ("enabled".to_owned(), data::bool::bool_type()),
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
            ("enabled".to_owned(), data::bool::bool_type()),
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
            QueueContent::Folder(folder) => (
                "folder".to_owned(),
                Data::new(data::object::Object(vec![
                    (
                        "index".to_owned(),
                        Data::new(data::int::Int(folder.index as _)),
                    ),
                    (
                        "length".to_owned(),
                        Data::new(data::int::Int(folder.content.len() as _)),
                    ),
                    (
                        "name".to_owned(),
                        Data::new(data::string::String(folder.name.clone())),
                    ),
                ])),
            ),
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
    fn is_included_in(&self, target: &dyn MersType) -> bool {
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
