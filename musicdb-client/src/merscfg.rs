use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU8},
        mpsc::Sender,
        Arc, Mutex, RwLock,
    },
    time::Duration,
};

use musicdb_lib::{data::database::Database, server::Command};
use musicdb_mers::mers_lib::{
    data::{Data, Type},
    errors::CheckError,
    info::DisplayInfo,
};
use speedy2d::{color::Color, dimen::Vec2, shape::Rectangle, window::UserEventSender};

use crate::{
    gui::{Gui, GuiAction, GuiConfig, GuiElem, GuiElemCfg, GuiEvent},
    gui_base::Panel,
    gui_notif::{NotifInfo, NotifOverlay},
    gui_text::Label,
    textcfg::TextBuilder,
};

pub struct OptFunc(pub Option<musicdb_mers::mers_lib::data::function::Function>);
impl OptFunc {
    pub fn none() -> Self {
        Self(None)
    }
    pub fn some(func: musicdb_mers::mers_lib::data::function::Function) -> Self {
        Self(Some(func))
    }
    fn run(&self) {
        if let Some(func) = &self.0 {
            func.run_immut(Data::empty_tuple());
        }
    }
}

/// mers code must return an object `{}` with hook functions.
/// All hook functions will be called with `()` as their argument,
/// and their return value will be ignored.
///
/// Values:
/// - `is_playing`
/// - `is_idle`
/// - `window_size_in_pixels`
/// - `idle_screen_cover_aspect_ratio`
///
/// Functions:
/// - `idle_start`
/// - `idle_stop`
/// - `idle_prevent`
/// - `send_notification`
/// - `set_idle_screen_cover_pos`
/// - `set_idle_screen_artist_image_pos`
/// - `set_idle_screen_top_text_pos`
/// - `set_idle_screen_side_text_1_pos`
/// - `set_idle_screen_side_text_2_pos`
/// - `set_statusbar_text_format`
/// - `set_idle_screen_top_text_format`
/// - `set_idle_screen_side_text_1_format`
/// - `set_idle_screen_side_text_2_format`
pub struct MersCfg {
    pub source_file: PathBuf,
    pub database: Arc<Mutex<Database>>,
    // - - handler functions - -
    pub func_before_draw: OptFunc,
    pub func_library_updated: OptFunc,
    pub func_queue_updated: OptFunc,
    // - - globals that aren't functions - -
    pub var_is_idle: Arc<RwLock<Data>>,
    pub var_window_size_in_pixels: Arc<RwLock<Data>>,
    pub var_idle_screen_cover_aspect_ratio: Arc<RwLock<Data>>,
    // - - results from running functions - -
    pub channel_gui_actions: (
        std::sync::mpsc::Sender<Command>,
        std::sync::mpsc::Receiver<Command>,
    ),
    pub updated_playing_status: Arc<AtomicU8>,
    pub updated_idle_status: Arc<AtomicU8>,
    pub updated_idle_screen_cover_pos: Arc<Updatable<Option<Rectangle>>>,
    pub updated_idle_screen_artist_image_pos: Arc<Updatable<Option<Rectangle>>>,
    pub updated_idle_screen_top_text_pos: Arc<Updatable<Rectangle>>,
    pub updated_idle_screen_side_text_1_pos: Arc<Updatable<Rectangle>>,
    pub updated_idle_screen_side_text_2_pos: Arc<Updatable<Rectangle>>,
    pub updated_idle_screen_playback_buttons_pos: Arc<Updatable<Rectangle>>,
    pub updated_statusbar_text_format: Arc<Updatable<TextBuilder>>,
    pub updated_idle_screen_top_text_format: Arc<Updatable<TextBuilder>>,
    pub updated_idle_screen_side_text_1_format: Arc<Updatable<TextBuilder>>,
    pub updated_idle_screen_side_text_2_format: Arc<Updatable<TextBuilder>>,
}

impl MersCfg {
    pub fn new(path: PathBuf, database: Arc<Mutex<Database>>) -> Self {
        Self {
            source_file: path,
            database,

            func_before_draw: OptFunc::none(),
            func_library_updated: OptFunc::none(),
            func_queue_updated: OptFunc::none(),

            var_is_idle: Arc::new(RwLock::new(Data::new(
                musicdb_mers::mers_lib::data::bool::Bool(false),
            ))),
            var_window_size_in_pixels: Arc::new(RwLock::new(Data::new(
                musicdb_mers::mers_lib::data::tuple::Tuple(vec![
                    Data::new(musicdb_mers::mers_lib::data::int::Int(0)),
                    Data::new(musicdb_mers::mers_lib::data::int::Int(0)),
                ]),
            ))),
            var_idle_screen_cover_aspect_ratio: Arc::new(RwLock::new(Data::new(
                musicdb_mers::mers_lib::data::float::Float(0.0),
            ))),

            channel_gui_actions: std::sync::mpsc::channel(),
            updated_playing_status: Arc::new(AtomicU8::new(0)),
            updated_idle_status: Arc::new(AtomicU8::new(0)),
            updated_idle_screen_cover_pos: Arc::new(Updatable::new()),
            updated_idle_screen_artist_image_pos: Arc::new(Updatable::new()),
            updated_idle_screen_top_text_pos: Arc::new(Updatable::new()),
            updated_idle_screen_side_text_1_pos: Arc::new(Updatable::new()),
            updated_idle_screen_side_text_2_pos: Arc::new(Updatable::new()),
            updated_idle_screen_playback_buttons_pos: Arc::new(Updatable::new()),
            updated_statusbar_text_format: Arc::new(Updatable::new()),
            updated_idle_screen_top_text_format: Arc::new(Updatable::new()),
            updated_idle_screen_side_text_1_format: Arc::new(Updatable::new()),
            updated_idle_screen_side_text_2_format: Arc::new(Updatable::new()),
        }
    }
    fn custom_globals(
        &self,
        cfg: musicdb_mers::mers_lib::prelude_extend_config::Config,
        db: &Arc<Mutex<Database>>,
        event_sender: Arc<UserEventSender<GuiEvent>>,
        notif_sender: Sender<
            Box<dyn FnOnce(&NotifOverlay) -> (Box<dyn GuiElem>, NotifInfo) + Send>,
        >,
        after_db_cmd: &Arc<Mutex<Option<Box<dyn FnMut(Command) + Send + Sync + 'static>>>>,
    ) -> musicdb_mers::mers_lib::prelude_extend_config::Config {
        let cmd_es = event_sender.clone();
        let cmd_ga = self.channel_gui_actions.0.clone();
        fn get_type(d: &RwLock<Data>) -> Type {
            d.read().unwrap().get().as_type()
        }
        musicdb_mers::add(
            cfg,
            db,
            &Arc::new(move |cmd| {
                cmd_ga.send(cmd).unwrap();
                cmd_es.send_event(GuiEvent::RefreshMers).unwrap();
            }),
            after_db_cmd,
        )
        .add_var_from_arc(
            "is_idle".to_owned(),
            Arc::clone(&self.var_is_idle),
            get_type(&self.var_is_idle),
        )
        .add_var_from_arc(
            "window_size_in_pixels".to_owned(),
            Arc::clone(&self.var_window_size_in_pixels),
            get_type(&self.var_window_size_in_pixels),
        )
        .add_var_from_arc(
            "idle_screen_cover_aspect_ratio".to_owned(),
            Arc::clone(&self.var_idle_screen_cover_aspect_ratio),
            get_type(&self.var_idle_screen_cover_aspect_ratio),
        )
        .add_var("playback_resume".to_owned(),{
            let es = event_sender.clone();
            let v = Arc::clone(&self.updated_playing_status);
            musicdb_mers::mers_lib::data::function::Function::new_generic(
                |a, i| {
                    if a.is_zero_tuple() {
                        Ok(Type::empty_tuple())
                    } else {
                        Err(format!("Can't call `playback_resume` with argument of type `{}` (must be `()`).", a.with_info(i)).into())
                    }
                },
                move |_, _| {
                    v.store(1, std::sync::atomic::Ordering::Relaxed);
                    es.send_event(GuiEvent::Refresh).unwrap();
                    Ok(Data::empty_tuple())
                },
            )
        })
        .add_var("playback_pause".to_owned(),{
            let es = event_sender.clone();
            let v = Arc::clone(&self.updated_playing_status);
            musicdb_mers::mers_lib::data::function::Function::new_generic(
                |a, i| {
                    if a.is_zero_tuple() {
                        Ok(Type::empty_tuple())
                    } else {
                        Err(format!("Can't call `playback_pause` with argument of type `{}` (must be `()`).", a.with_info(i)).into())
                    }
                },
                move |_, _| {
                    v.store(2, std::sync::atomic::Ordering::Relaxed);
                    es.send_event(GuiEvent::Refresh).unwrap();
                    Ok(Data::empty_tuple())
                },
            )
        })
        .add_var("playback_stop".to_owned(),{
            let es = event_sender.clone();
            let v = Arc::clone(&self.updated_playing_status);
            musicdb_mers::mers_lib::data::function::Function::new_generic(
                |a,i| {
                    if a.is_zero_tuple() {
                        Ok(Type::empty_tuple())
                    } else {
                        Err(format!("Can't call `playback_stop` with argument of type `{}` (must be `()`).", a.with_info(i)).into())
                    }
                },
                move |_, _| {
                    v.store(3, std::sync::atomic::Ordering::Relaxed);
                    es.send_event(GuiEvent::Refresh).unwrap();
                    Ok(Data::empty_tuple())
                },
            )
        })
        .add_var("idle_start".to_owned(),{
            let es = event_sender.clone();
            let v = Arc::clone(&self.updated_idle_status);
            musicdb_mers::mers_lib::data::function::Function::new_generic(
                |a, i| {
                    if a.is_zero_tuple() {
                        Ok(Type::empty_tuple())
                    } else {
                        Err(format!("Can't call `idle_start` with argument of type `{}` (must be `()`).",a.with_info(i)).into())
                    }
                },
                move |_, _| {
                    v.store(1, std::sync::atomic::Ordering::Relaxed);
                    es.send_event(GuiEvent::Refresh).unwrap();
                    Ok(Data::empty_tuple())
                },
            )
        })
        .add_var("idle_stop".to_owned(),{
            let es = event_sender.clone();
            let v = Arc::clone(&self.updated_idle_status);
            musicdb_mers::mers_lib::data::function::Function::new_generic(
                |a,i| {
                    if a.is_zero_tuple() {
                        Ok(Type::empty_tuple())
                    } else {
                        Err(format!("Can't call `idle_stop` with argument of type `{}` (must be `()`).", a.with_info(i)).into())
                    }
                },
                move |_, _| {
                    v.store(2, std::sync::atomic::Ordering::Relaxed);
                    es.send_event(GuiEvent::Refresh).unwrap();
                    Ok(Data::empty_tuple())
                },
            )
        })
        .add_var("idle_prevent".to_owned(),{
            let es = event_sender.clone();
            let v = Arc::clone(&self.updated_idle_status);
            musicdb_mers::mers_lib::data::function::Function::new_generic(
                |a, i| {
                    if a.is_zero_tuple() {
                        Ok(Type::empty_tuple())
                    } else {
                        Err(format!("Can't call `idle_prevent` with argument of type `{}` (must be `()`).", a.with_info(i)).into())
                    }
                },
                move |_, _| {
                    v.store(3, std::sync::atomic::Ordering::Relaxed);
                    es.send_event(GuiEvent::Refresh).unwrap();
                    Ok(Data::empty_tuple())
                },
            )
        })
        .add_var("send_notification".to_owned(),{
            let es = event_sender.clone();
            musicdb_mers::mers_lib::data::function::Function::new_generic(
                |a, i| {
                    if a.is_included_in_single(&musicdb_mers::mers_lib::data::tuple::TupleT(vec![
                        musicdb_mers::mers_lib::data::Type::new(musicdb_mers::mers_lib::data::string::StringT),
                        musicdb_mers::mers_lib::data::Type::new(musicdb_mers::mers_lib::data::string::StringT),
                        musicdb_mers::mers_lib::data::Type::newm(vec![
                            Arc::new(musicdb_mers::mers_lib::data::int::IntT(musicdb_mers::mers_lib::data::int::INT_MIN,musicdb_mers::mers_lib::data::int::INT_MAX)),
                            Arc::new(musicdb_mers::mers_lib::data::float::FloatT)
                        ]),
                    ])) {
                        Ok(Type::empty_tuple())
                    } else {
                        Err(format!("Can't call `send_notification` with argument of type `{}` (must be `String`).", a.with_info(i)).into())
                    }
                },
                move |a, _| {
                    let a = a.get();
                    let t = &a.as_any().downcast_ref::<musicdb_mers::mers_lib::data::tuple::Tuple>().unwrap().0;
                    let title = t[0].get().as_any().downcast_ref::<musicdb_mers::mers_lib::data::string::String>().unwrap().0.clone();
                    let text = t[1].get().as_any().downcast_ref::<musicdb_mers::mers_lib::data::string::String>().unwrap().0.clone();
                    let t = t[2].get();
                    let duration = t.as_any().downcast_ref::<musicdb_mers::mers_lib::data::int::Int>().map(|s| Duration::from_secs(s.0.max(0) as _)).unwrap_or_else(|| Duration::from_secs_f64(t.as_any().downcast_ref::<musicdb_mers::mers_lib::data::float::Float>().unwrap().0));
                    notif_sender
                        .send(Box::new(move |_| {
                            (
                                Box::new(Panel::with_background(
                                    GuiElemCfg::default(),
                                    (
                                        Label::new(
                                            GuiElemCfg::at(Rectangle::from_tuples(
                                                (0.25, 0.0),
                                                (0.75, 0.5),
                                            )),
                                            title,
                                            Color::WHITE,
                                            None,
                                            Vec2::new(0.5, 0.0),
                                        ),
                                        Label::new(
                                            GuiElemCfg::at(Rectangle::from_tuples(
                                                (0.0, 0.5),
                                                (1.0, 1.0),
                                            )),
                                            text,
                                            Color::WHITE,
                                            None,
                                            Vec2::new(0.5, 1.0),
                                        ),
                                    ),
                                    Color::from_rgba(0.0, 0.0, 0.0, 0.8),
                                )),
                                NotifInfo::new(duration),
                            )
                        }))
                        .unwrap();
                    es.send_event(GuiEvent::Refresh).unwrap();
                    Ok(Data::empty_tuple())
                },
            )
        })
        .add_var("set_idle_screen_cover_pos".to_owned(),{
            let es = event_sender.clone();
            let update = Arc::clone(&self.updated_idle_screen_cover_pos);
            musicdb_mers::mers_lib::data::function::Function::new_generic(
                |a, i| {
                    if a.is_included_in(&musicdb_mers::mers_lib::data::Type::newm(vec![
                        Arc::new(musicdb_mers::mers_lib::data::tuple::TupleT(vec![])),
                        Arc::new(musicdb_mers::mers_lib::data::tuple::TupleT(vec![
                            musicdb_mers::mers_lib::data::Type::new(musicdb_mers::mers_lib::data::float::FloatT),
                            musicdb_mers::mers_lib::data::Type::new(musicdb_mers::mers_lib::data::float::FloatT),
                            musicdb_mers::mers_lib::data::Type::new(musicdb_mers::mers_lib::data::float::FloatT),
                            musicdb_mers::mers_lib::data::Type::new(musicdb_mers::mers_lib::data::float::FloatT),
                        ]))
                    ])) {
                        Ok(Type::empty_tuple())
                    } else {
                        Err(format!("Can't call `set_idle_screen_cover_pos` with argument of type `{}` (must be `()` or `(Float, Float, Float, Float)`).", a.with_info(i)).into())
                    }
                },
                move |a, _| {
                    let a = a.get();
                    let mut vals = a.as_any().downcast_ref::<musicdb_mers::mers_lib::data::tuple::Tuple>().unwrap().0.iter().map(|v| v.get().as_any().downcast_ref::<musicdb_mers::mers_lib::data::float::Float>().unwrap().0);
                    update.update(
                    if vals.len() >= 4 {
                        Some(Rectangle::from_tuples((vals.next().unwrap() as _, vals.next().unwrap() as _), (vals.next().unwrap() as _, vals.next().unwrap() as _)))
                    } else { None });
                    es.send_event(GuiEvent::Refresh).unwrap();
                    Ok(Data::empty_tuple())
                },
            )
        }).add_var("set_idle_screen_artist_image_pos".to_owned(),{
            let es = event_sender.clone();
            let update = Arc::clone(&self.updated_idle_screen_artist_image_pos);
            musicdb_mers::mers_lib::data::function::Function::new_generic(
                |a, i| {
                    if a.is_included_in(&musicdb_mers::mers_lib::data::Type::newm(vec![
                        Arc::new(musicdb_mers::mers_lib::data::tuple::TupleT(vec![])),
                        Arc::new(musicdb_mers::mers_lib::data::tuple::TupleT(vec![
                            musicdb_mers::mers_lib::data::Type::new(musicdb_mers::mers_lib::data::float::FloatT),
                            musicdb_mers::mers_lib::data::Type::new(musicdb_mers::mers_lib::data::float::FloatT),
                            musicdb_mers::mers_lib::data::Type::new(musicdb_mers::mers_lib::data::float::FloatT),
                            musicdb_mers::mers_lib::data::Type::new(musicdb_mers::mers_lib::data::float::FloatT),
                        ]))
                    ])) {
                        Ok(Type::empty_tuple())
                    } else {
                        Err(format!("Can't call `set_idle_screen_artist_image_pos` with argument of type `{}` (must be `()` or `(Float, Float, Float, Float)`).", a.with_info(i)).into())
                    }
                },
                move |a, _| {
                    let a = a.get();
                    let mut vals = a.as_any().downcast_ref::<musicdb_mers::mers_lib::data::tuple::Tuple>().unwrap().0.iter().map(|v| v.get().as_any().downcast_ref::<musicdb_mers::mers_lib::data::float::Float>().unwrap().0);
                    update.update(
                    if vals.len() >= 4 {
                        Some(Rectangle::from_tuples((vals.next().unwrap() as _, vals.next().unwrap() as _), (vals.next().unwrap() as _, vals.next().unwrap() as _)))
                    } else { None });
                    es.send_event(GuiEvent::Refresh).unwrap();
                    Ok(Data::empty_tuple())
                },
            )
        })
        .add_var("set_idle_screen_top_text_pos".to_owned(), gen_set_pos_func("set_idle_screen_top_text_pos", Arc::clone(&event_sender), Arc::clone(&self.updated_idle_screen_top_text_pos)))
        .add_var("set_idle_screen_side_text_1_pos".to_owned(), gen_set_pos_func("set_idle_screen_side_text_1_pos", Arc::clone(&event_sender), Arc::clone(&self.updated_idle_screen_side_text_1_pos)))
        .add_var("set_idle_screen_side_text_2_pos".to_owned(), gen_set_pos_func("set_idle_screen_side_text_2_pos", Arc::clone(&event_sender), Arc::clone(&self.updated_idle_screen_side_text_2_pos)))
        .add_var("set_idle_screen_playback_buttons_pos".to_owned(), gen_set_pos_func("set_idle_screen_playback_buttons_pos", Arc::clone(&event_sender), Arc::clone(&self.updated_idle_screen_playback_buttons_pos)))

        .add_var("set_statusbar_text_format".to_owned(),{
            let es = event_sender.clone();
            let update = Arc::clone(&self.updated_statusbar_text_format);
            musicdb_mers::mers_lib::data::function::Function::new_generic(
                |a, i| {
                    if a.is_included_in_single(&musicdb_mers::mers_lib::data::string::StringT) {
                        Ok(Type::newm(vec![
                            Arc::new(musicdb_mers::mers_lib::data::tuple::TupleT(vec![])),
                            Arc::new(musicdb_mers::mers_lib::data::string::StringT),
                        ]))
                    } else {
                        Err(format!("Can't call `set_statusbar_text_format` with argument of type `{}` (must be `String`).", a.with_info(i)).into())
                    }
                },
                move |a, _| {
                    let a = a.get();
                    let o = match a.as_any().downcast_ref::<musicdb_mers::mers_lib::data::string::String>().unwrap().0.parse() {
                        Ok(v) => {
                            update.update(v);
                            Data::empty_tuple()
                        }
                        Err(e) => musicdb_mers::mers_lib::data::Data::new(musicdb_mers::mers_lib::data::string::String(e.to_string())),
                    };
                    es.send_event(GuiEvent::Refresh).unwrap();
                    Ok(o)
                },
            )
        })
        .add_var("set_idle_screen_top_text_format".to_owned(),{
            let es = event_sender.clone();
            let update = Arc::clone(&self.updated_idle_screen_top_text_format);
            musicdb_mers::mers_lib::data::function::Function::new_generic(
                |a, i| {
                    if a.is_included_in_single(&musicdb_mers::mers_lib::data::string::StringT) {
                        Ok(Type::newm(vec![
                            Arc::new(musicdb_mers::mers_lib::data::tuple::TupleT(vec![])),
                            Arc::new(musicdb_mers::mers_lib::data::string::StringT),
                        ]))
                    } else {
                        Err(format!("Can't call `set_idle_screen_top_text_format` with argument of type `{}` (must be `String`).", a.with_info(i)).into())
                    }
                },
                move |a, _| {
                    let a = a.get();
                    let o = match a.as_any().downcast_ref::<musicdb_mers::mers_lib::data::string::String>().unwrap().0.parse() {
                        Ok(v) => {
                            update.update(v);
                            Data::empty_tuple()
                        }
                        Err(e) => musicdb_mers::mers_lib::data::Data::new(musicdb_mers::mers_lib::data::string::String(e.to_string())),
                    };
                    es.send_event(GuiEvent::Refresh).unwrap();
                    Ok(o)
                },
            )
        }).add_var("set_idle_screen_side_text_1_format".to_owned(),{
            let es = event_sender.clone();
            let update = Arc::clone(&self.updated_idle_screen_side_text_1_format);
            musicdb_mers::mers_lib::data::function::Function::new_generic(
                |a, i| {
                    if a.is_included_in_single(&musicdb_mers::mers_lib::data::string::StringT) {
                        Ok(Type::newm(vec![
                            Arc::new(musicdb_mers::mers_lib::data::tuple::TupleT(vec![])),
                            Arc::new(musicdb_mers::mers_lib::data::string::StringT),
                        ]))
                    } else {
                        Err(format!("Can't call `set_idle_screen_side_text_1_format` with argument of type `{}` (must be `String`).", a.with_info(i)).into())
                    }
                },
                move |a, _| {
                    let a = a.get();
                    let o = match a.as_any().downcast_ref::<musicdb_mers::mers_lib::data::string::String>().unwrap().0.parse() {
                        Ok(v) => {
                            update.update(v);
                            Data::empty_tuple()
                        }
                        Err(e) => musicdb_mers::mers_lib::data::Data::new(musicdb_mers::mers_lib::data::string::String(e.to_string())),
                    };
                    es.send_event(GuiEvent::Refresh).unwrap();
                    Ok(o)
                },
            )
        }).add_var("set_idle_screen_side_text_2_format".to_owned(),{
            let es = event_sender.clone();
            let update = Arc::clone(&self.updated_idle_screen_side_text_2_format);
            musicdb_mers::mers_lib::data::function::Function::new_generic(
                |a, i| {
                    if a.is_included_in_single(&musicdb_mers::mers_lib::data::string::StringT) {
                        Ok(Type::newm(vec![
                            Arc::new(musicdb_mers::mers_lib::data::tuple::TupleT(vec![])),
                            Arc::new(musicdb_mers::mers_lib::data::string::StringT),
                        ]))
                    } else {
                        Err(format!("Can't call `set_idle_screen_side_text_2_format` with argument of type `{}` (must be `String`).", a.with_info(i)).into())
                    }
                },
                move |a, _| {
                    let a = a.get();
                    let o = match a.as_any().downcast_ref::<musicdb_mers::mers_lib::data::string::String>().unwrap().0.parse() {
                        Ok(v) => {
                            update.update(v);
                            Data::empty_tuple()
                        }
                        Err(e) => musicdb_mers::mers_lib::data::Data::new(musicdb_mers::mers_lib::data::string::String(e.to_string())),
                    };
                    es.send_event(GuiEvent::Refresh).unwrap();
                    Ok(o)
                },
            )
        })
        // .add_type("Song".to_owned(), Ok(Arc::new(musicdb_mers::mers_lib::data::object::ObjectT(vec![
        //         ("id".to_owned(), Type::new(musicdb_mers::mers_lib::data::int::IntT)),
        //         ("title".to_owned(), Type::new(musicdb_mers::mers_lib::data::string::StringT)),
        //         ("album".to_owned(), Type::new(musicdb_mers::mers_lib::data::string::StringT)),
        //         ("artist".to_owned(), Type::new(musicdb_mers::mers_lib::data::string::StringT)),
        //     ]))))
    }

    pub fn run(gui_cfg: &mut GuiConfig, gui: &mut Gui, run: impl FnOnce(&Self) -> &OptFunc) {
        // prepare vars
        *gui_cfg.merscfg.var_window_size_in_pixels.write().unwrap() =
            musicdb_mers::mers_lib::data::Data::new(musicdb_mers::mers_lib::data::tuple::Tuple(
                vec![
                    musicdb_mers::mers_lib::data::Data::new(
                        musicdb_mers::mers_lib::data::int::Int(gui.size.x as _),
                    ),
                    musicdb_mers::mers_lib::data::Data::new(
                        musicdb_mers::mers_lib::data::int::Int(gui.size.y as _),
                    ),
                ],
            ));
        *gui_cfg
            .merscfg
            .var_idle_screen_cover_aspect_ratio
            .write()
            .unwrap() =
            musicdb_mers::mers_lib::data::Data::new(musicdb_mers::mers_lib::data::float::Float(
                gui.gui.c_idle_display.cover_aspect_ratio.value as _,
            ));

        // run
        run(&gui_cfg.merscfg).run();

        loop {
            if let Ok(a) = gui_cfg.merscfg.channel_gui_actions.1.try_recv() {
                gui.exec_gui_action(GuiAction::SendToServer(a));
            } else {
                break;
            }
        }

        // apply updates

        match gui_cfg
            .merscfg
            .updated_playing_status
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            0 => {}
            v => {
                match v {
                    1 => gui.exec_gui_action(GuiAction::SendToServer(Command::Resume)),
                    2 => gui.exec_gui_action(GuiAction::SendToServer(Command::Pause)),
                    3 => gui.exec_gui_action(GuiAction::SendToServer(Command::Stop)),
                    _ => {}
                }
                gui_cfg
                    .merscfg
                    .updated_playing_status
                    .store(0, std::sync::atomic::Ordering::Relaxed);
            }
        }

        match gui_cfg
            .merscfg
            .updated_idle_status
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            0 => {}
            v => {
                match v {
                    1 => gui.gui.force_idle(),
                    2 => gui.gui.unidle(),
                    3 => gui.gui.not_idle(),
                    _ => {}
                }
                gui_cfg
                    .merscfg
                    .updated_idle_status
                    .store(0, std::sync::atomic::Ordering::Relaxed);
            }
        }

        if let Some(maybe_rect) = gui_cfg.merscfg.updated_idle_screen_cover_pos.take_val() {
            gui.gui.c_idle_display.cover_pos = maybe_rect;
        }
        if let Some(maybe_rect) = gui_cfg
            .merscfg
            .updated_idle_screen_artist_image_pos
            .take_val()
        {
            gui.gui.c_idle_display.artist_image_pos = maybe_rect;
        }
        if let Some(maybe_rect) = gui_cfg.merscfg.updated_idle_screen_top_text_pos.take_val() {
            gui.gui.c_idle_display.c_top_label.config_mut().pos = maybe_rect;
        }
        if let Some(maybe_rect) = gui_cfg
            .merscfg
            .updated_idle_screen_side_text_1_pos
            .take_val()
        {
            gui.gui.c_idle_display.c_side1_label.config_mut().pos = maybe_rect;
        }
        if let Some(maybe_rect) = gui_cfg
            .merscfg
            .updated_idle_screen_side_text_2_pos
            .take_val()
        {
            gui.gui.c_idle_display.c_side2_label.config_mut().pos = maybe_rect;
        }
        if let Some(maybe_rect) = gui_cfg
            .merscfg
            .updated_idle_screen_playback_buttons_pos
            .take_val()
        {
            gui.gui.c_idle_display.c_buttons.config_mut().pos = maybe_rect;
            gui.gui.c_idle_display.c_buttons_custom_pos = true;
        }
        if let Some(fmt) = gui_cfg.merscfg.updated_statusbar_text_format.take_val() {
            gui_cfg.status_bar_text = fmt;
            gui.gui.c_status_bar.force_reset_texts = true;
        }
        if let Some(fmt) = gui_cfg
            .merscfg
            .updated_idle_screen_top_text_format
            .take_val()
        {
            gui_cfg.idle_top_text = fmt;
            gui.gui.c_idle_display.force_reset_texts = true;
        }
        if let Some(fmt) = gui_cfg
            .merscfg
            .updated_idle_screen_side_text_1_format
            .take_val()
        {
            gui_cfg.idle_side1_text = fmt;
            gui.gui.c_idle_display.force_reset_texts = true;
        }
        if let Some(fmt) = gui_cfg
            .merscfg
            .updated_idle_screen_side_text_2_format
            .take_val()
        {
            gui_cfg.idle_side2_text = fmt;
            gui.gui.c_idle_display.force_reset_texts = true;
        }
    }

    pub fn load(
        &mut self,
        event_sender: Arc<UserEventSender<GuiEvent>>,
        notif_sender: Sender<
            Box<dyn FnOnce(&NotifOverlay) -> (Box<dyn GuiElem>, NotifInfo) + Send>,
        >,
        after_db_cmd: &Arc<Mutex<Option<Box<dyn FnMut(Command) + Send + Sync + 'static>>>>,
    ) -> std::io::Result<Result<Result<(), (String, Option<CheckError>)>, CheckError>> {
        let src = musicdb_mers::mers_lib::prelude_compile::Source::new_from_file(
            self.source_file.clone(),
        )?;
        Ok(self.load2(src, event_sender, notif_sender, after_db_cmd))
    }
    fn load2(
        &mut self,
        mut src: musicdb_mers::mers_lib::prelude_compile::Source,
        event_sender: Arc<UserEventSender<GuiEvent>>,
        notif_sender: Sender<
            Box<dyn FnOnce(&NotifOverlay) -> (Box<dyn GuiElem>, NotifInfo) + Send>,
        >,
        after_db_cmd: &Arc<Mutex<Option<Box<dyn FnMut(Command) + Send + Sync + 'static>>>>,
    ) -> Result<Result<(), (String, Option<CheckError>)>, CheckError> {
        let srca = Arc::new(src.clone());
        let (mut i1, mut i2, mut i3) = self
            .custom_globals(
                musicdb_mers::mers_lib::prelude_extend_config::Config::new().bundle_std(),
                &self.database,
                event_sender,
                notif_sender,
                after_db_cmd,
            )
            .infos();
        let compiled = musicdb_mers::mers_lib::prelude_compile::parse(&mut src, &srca)?
            .compile(&mut i1, Default::default())?;
        let _ = compiled.check(&mut i3, None)?;
        let out = compiled.run(&mut i2)?;
        Ok(self.load3(out, i2.display_info()))
    }
    fn load3(
        &mut self,
        out: musicdb_mers::mers_lib::data::Data,
        i: DisplayInfo,
    ) -> Result<(), (String, Option<CheckError>)> {
        if let Some(obj) = out
            .get()
            .as_any()
            .downcast_ref::<musicdb_mers::mers_lib::data::object::Object>()
        {
            for (name, val) in obj.iter() {
                let name = i.get_object_field_name(*name).to_string();
                let name = name.as_str();
                match name {
                    "before_draw" => {
                        self.func_before_draw = OptFunc::some(check_handler(name, val)?);
                    }
                    "library_updated" => {
                        self.func_library_updated = OptFunc::some(check_handler(name, val)?);
                    }
                    "queue_updated" => {
                        self.func_queue_updated = OptFunc::some(check_handler(name, val)?);
                    }
                    name => {
                        eprintln!("merscfg: ignoring unexpected field named '{name}'.")
                    }
                }
            }
        } else {
            return Err((String::from("mers config file must return an object! (optional) fields of the object: `before_draw`, `library_updated`, `queue_updated`."), None));
        }
        Ok(())
    }
}

fn check_handler(
    name: &str,
    val: &musicdb_mers::mers_lib::data::Data,
) -> Result<musicdb_mers::mers_lib::data::function::Function, (String, Option<CheckError>)> {
    if let Some(func) = val
        .get()
        .as_any()
        .downcast_ref::<musicdb_mers::mers_lib::data::function::Function>()
    {
        match func.check(&Type::empty_tuple()) {
            Ok(_) => Ok(func.clone()),
            Err(e) => Err((format!("Function '{name}' causes an error:"), Some(e))),
        }
    } else {
        Err((format!("Expected a function for field '{name}'!"), None))
    }
}

fn gen_set_pos_func(
    name: &'static str,
    es: Arc<UserEventSender<GuiEvent>>,
    update: Arc<Updatable<Rectangle>>,
) -> musicdb_mers::mers_lib::data::function::Function {
    musicdb_mers::mers_lib::data::function::Function::new_generic(
        move |a, i| {
            if a.is_included_in(&musicdb_mers::mers_lib::data::Type::newm(vec![Arc::new(
                musicdb_mers::mers_lib::data::tuple::TupleT(vec![
                    musicdb_mers::mers_lib::data::Type::new(
                        musicdb_mers::mers_lib::data::float::FloatT,
                    ),
                    musicdb_mers::mers_lib::data::Type::new(
                        musicdb_mers::mers_lib::data::float::FloatT,
                    ),
                    musicdb_mers::mers_lib::data::Type::new(
                        musicdb_mers::mers_lib::data::float::FloatT,
                    ),
                    musicdb_mers::mers_lib::data::Type::new(
                        musicdb_mers::mers_lib::data::float::FloatT,
                    ),
                ]),
            )])) {
                Ok(Type::empty_tuple())
            } else {
                Err(format!("Can't call `{name}` with argument of type `{}` (must be `(Float, Float, Float, Float)`).", a.with_info(i)).into())
            }
        },
        move |a, _| {
            let a = a.get();
            let mut vals = a
                .as_any()
                .downcast_ref::<musicdb_mers::mers_lib::data::tuple::Tuple>()
                .unwrap()
                .0
                .iter()
                .map(|v| {
                    v.get()
                        .as_any()
                        .downcast_ref::<musicdb_mers::mers_lib::data::float::Float>()
                        .unwrap()
                        .0
                });
            update.update(Rectangle::from_tuples(
                (vals.next().unwrap() as _, vals.next().unwrap() as _),
                (vals.next().unwrap() as _, vals.next().unwrap() as _),
            ));
            es.send_event(GuiEvent::Refresh).unwrap();
            Ok(Data::empty_tuple())
        },
    )
}

pub struct Updatable<T> {
    updated: AtomicBool,
    value: Mutex<Option<T>>,
}
impl<T> Updatable<T> {
    pub fn new() -> Self {
        Self {
            updated: AtomicBool::new(false),
            value: Mutex::new(None),
        }
    }
    pub fn update(&self, val: T) {
        self.updated
            .store(true, std::sync::atomic::Ordering::Relaxed);
        *self.value.lock().unwrap() = Some(val);
    }
    pub fn take_val(&self) -> Option<T> {
        if self.updated.load(std::sync::atomic::Ordering::Relaxed) {
            self.updated
                .store(false, std::sync::atomic::Ordering::Relaxed);
            self.value.lock().unwrap().take()
        } else {
            None
        }
    }
}
impl<T> Updatable<T>
where
    T: Default,
{
    pub fn modify<R>(&self, func: impl FnOnce(&mut T) -> R) -> R {
        self.updated
            .store(true, std::sync::atomic::Ordering::Relaxed);
        let mut val = self.value.lock().unwrap();
        if val.is_none() {
            *val = Some(Default::default());
        }
        func(val.as_mut().unwrap())
    }
}
