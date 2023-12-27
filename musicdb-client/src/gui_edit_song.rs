use std::{
    sync::{atomic::AtomicU8, Arc},
    time::Instant,
};

use musicdb_lib::data::{song::Song, ArtistId};
use speedy2d::{color::Color, dimen::Vec2, shape::Rectangle};

use crate::{
    color_scale,
    gui::{GuiAction, GuiElem, GuiElemCfg, GuiElemChildren},
    gui_anim::AnimationController,
    gui_base::{Button, Panel, ScrollBox},
    gui_text::{Label, TextField},
};

// TODO: Fix bug where after selecting an artist you can't mouse-click the buttons anymore (to change it)

const ELEM_HEIGHT: f32 = 32.0;

pub struct EditorForSongs {
    config: GuiElemCfg,
    songs: Vec<Song>,
    c_title: Label,
    c_scrollbox: ScrollBox<EditorForSongElems>,
    c_buttons: Panel<[Button<[Label; 1]>; 2]>,
    c_background: Panel<()>,
    created: Option<Instant>,
    event_sender: std::sync::mpsc::Sender<Event>,
    event_recv: std::sync::mpsc::Receiver<Event>,
}
pub enum Event {
    Close,
    Apply,
    SetArtist(String, Option<ArtistId>),
}
pub struct EditorForSongElems {
    c_title: TextField,
    c_artist: EditorForSongArtistChooser,
    c_album: Label,
}
impl GuiElemChildren for EditorForSongElems {
    fn iter(&mut self) -> Box<dyn Iterator<Item = &mut dyn crate::gui::GuiElem> + '_> {
        Box::new(
            [
                self.c_title.elem_mut(),
                self.c_artist.elem_mut(),
                self.c_album.elem_mut(),
            ]
            .into_iter(),
        )
    }
    fn len(&self) -> usize {
        3
    }
}

impl EditorForSongs {
    pub fn new(songs: Vec<Song>) -> Self {
        let (sender, recv) = std::sync::mpsc::channel();
        Self {
            config: GuiElemCfg::at(Rectangle::from_tuples((0.0, 1.0), (1.0, 2.0))),
            c_title: Label::new(
                GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.0), (1.0, 0.05))),
                format!("Editing {} songs", songs.len()),
                Color::LIGHT_GRAY,
                None,
                Vec2::new(0.5, 0.5),
            ),
            c_scrollbox: ScrollBox::new(
                GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.05), (1.0, 0.95))),
                crate::gui_base::ScrollBoxSizeUnit::Pixels,
                EditorForSongElems {
                    c_title: TextField::new(
                        GuiElemCfg::default(),
                        format!(
                            "Title ({})",
                            songs
                                .iter()
                                .enumerate()
                                .map(|(i, s)| format!(
                                    "{}{}",
                                    if i == 0 { "" } else { ", " },
                                    s.title
                                ))
                                .collect::<String>()
                        ),
                        color_scale(Color::MAGENTA, 0.6, 0.6, 0.6, Some(0.75)),
                        Color::MAGENTA,
                    ),
                    c_artist: EditorForSongArtistChooser::new(sender.clone()),
                    c_album: Label::new(
                        GuiElemCfg::default(),
                        format!("(todo...)"),
                        Color::GRAY,
                        None,
                        Vec2::new(0.0, 0.5),
                    ),
                },
                vec![],
                ELEM_HEIGHT,
            ),
            c_buttons: Panel::new(
                GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.95), (1.0, 1.0))),
                [
                    {
                        let sender = sender.clone();
                        Button::new(
                            GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.0), (0.5, 1.0))),
                            move |_| {
                                sender.send(Event::Close).unwrap();
                                vec![]
                            },
                            [Label::new(
                                GuiElemCfg::default(),
                                "Close".to_owned(),
                                Color::WHITE,
                                None,
                                Vec2::new(0.5, 0.5),
                            )],
                        )
                    },
                    {
                        let sender = sender.clone();
                        Button::new(
                            GuiElemCfg::at(Rectangle::from_tuples((0.5, 0.0), (1.0, 1.0))),
                            move |_| {
                                sender.send(Event::Apply).unwrap();
                                vec![]
                            },
                            [Label::new(
                                GuiElemCfg::default(),
                                "Apply".to_owned(),
                                Color::WHITE,
                                None,
                                Vec2::new(0.5, 0.5),
                            )],
                        )
                    },
                ],
            ),
            c_background: Panel::with_background(GuiElemCfg::default(), (), Color::BLACK),
            created: Some(Instant::now()),
            songs,
            event_sender: sender,
            event_recv: recv,
        }
    }
}

impl GuiElem for EditorForSongs {
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut dyn GuiElem> + '_> {
        Box::new(
            [
                self.c_title.elem_mut(),
                self.c_scrollbox.elem_mut(),
                self.c_buttons.elem_mut(),
                self.c_background.elem_mut(),
            ]
            .into_iter(),
        )
    }
    fn draw(&mut self, info: &mut crate::gui::DrawInfo, g: &mut speedy2d::Graphics2D) {
        loop {
            match self.event_recv.try_recv() {
                Ok(e) => match e {
                    Event::Close => info.actions.push(GuiAction::Do(Box::new(|gui| {
                        gui.gui.c_editing_songs = None;
                        gui.gui.set_normal_ui_enabled(true);
                    }))),
                    Event::Apply => eprintln!("TODO: Apply"),
                    Event::SetArtist(name, id) => {
                        self.c_scrollbox.children.c_artist.chosen_id = id;
                        self.c_scrollbox.children.c_artist.last_search = name.to_lowercase();
                        self.c_scrollbox.children.c_artist.open_prog.target = 1.0;
                        *self
                            .c_scrollbox
                            .children
                            .c_artist
                            .c_name
                            .c_input
                            .content
                            .text() = name;
                        self.c_scrollbox.children.c_artist.config_mut().redraw = true;
                    }
                },
                Err(_) => break,
            }
        }
        // animation
        if let Some(created) = &self.created {
            if let Some(h) = &info.helper {
                h.request_redraw();
            }
            let open_prog = created.elapsed().as_secs_f32() / 0.5;
            if open_prog >= 1.0 {
                self.created = None;
                self.config.pos = Rectangle::from_tuples((0.0, 0.0), (1.0, 1.0));
                info.actions.push(GuiAction::Do(Box::new(|gui| {
                    gui.gui.set_normal_ui_enabled(false);
                })));
            } else {
                let offset = 1.0 - open_prog;
                let offset = offset * offset;
                self.config.pos = Rectangle::from_tuples((0.0, offset), (1.0, 1.0 + offset));
            }
        }
        // artist sel
        if self
            .c_scrollbox
            .children
            .c_artist
            .open_prog
            .update(Instant::now(), false)
        {
            if let Some(v) = self.c_scrollbox.children_heights.get_mut(1) {
                *v = ELEM_HEIGHT * self.c_scrollbox.children.c_artist.open_prog.value;
                self.c_scrollbox.config_mut().redraw = true;
            }
            if let Some(h) = &info.helper {
                h.request_redraw();
            }
        }
    }
    fn config(&self) -> &GuiElemCfg {
        &self.config
    }
    fn config_mut(&mut self) -> &mut GuiElemCfg {
        &mut self.config
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn elem(&self) -> &dyn GuiElem {
        self
    }
    fn elem_mut(&mut self) -> &mut dyn GuiElem {
        self
    }
}

pub struct EditorForSongArtistChooser {
    config: GuiElemCfg,
    event_sender: std::sync::mpsc::Sender<Event>,
    /// `1.0` = collapsed, `self.expand_to` = expanded (shows `c_picker` of height 7-1=6)
    open_prog: AnimationController<f32>,
    expand_to: f32,
    chosen_id: Option<ArtistId>,
    c_name: TextField,
    c_picker: ScrollBox<Vec<Button<[Label; 1]>>>,
    last_search: String,
}
impl EditorForSongArtistChooser {
    pub fn new(event_sender: std::sync::mpsc::Sender<Event>) -> Self {
        let expand_to = 7.0;
        Self {
            config: GuiElemCfg::default(),
            event_sender,
            open_prog: AnimationController::new(1.0, 1.0, 0.3, 8.0, 0.5, 0.6, Instant::now()),
            expand_to,
            chosen_id: None,
            c_name: TextField::new(
                GuiElemCfg::default(),
                "artist".to_owned(),
                Color::DARK_GRAY,
                Color::WHITE,
            ),
            c_picker: ScrollBox::new(
                GuiElemCfg::default().disabled(),
                crate::gui_base::ScrollBoxSizeUnit::Pixels,
                vec![],
                vec![],
                ELEM_HEIGHT,
            ),
            last_search: String::from("\n"),
        }
    }
}
impl GuiElem for EditorForSongArtistChooser {
    fn draw(&mut self, info: &mut crate::gui::DrawInfo, _g: &mut speedy2d::Graphics2D) {
        let picker_enabled = self.open_prog.value > 1.0;
        self.c_picker.config_mut().enabled = picker_enabled;
        if picker_enabled {
            let split = 1.0 / self.open_prog.value;
            self.c_name.config_mut().pos = Rectangle::from_tuples((0.0, 0.0), (1.0, split));
            self.c_picker.config_mut().pos = Rectangle::from_tuples((0.0, split), (1.0, 1.0));
        } else {
            self.c_name.config_mut().pos = Rectangle::from_tuples((0.0, 0.0), (1.0, 1.0));
        }

        let search = self.c_name.c_input.content.get_text().to_lowercase();
        let search_changed = &self.last_search != &search;
        if self.config.redraw || search_changed {
            *self.c_name.c_input.content.color() = if self.chosen_id.is_some() {
                Color::GREEN
            } else {
                Color::WHITE
            };
            if search_changed {
                self.chosen_id = None;
                self.open_prog.target = self.expand_to;
                if search.is_empty() {
                    self.open_prog.target = 1.0;
                }
            }
            let artists = info
                .database
                .artists()
                .values()
                .filter(|artist| artist.name.to_lowercase().contains(&search))
                // .take(self.open_prog.value as _)
                .map(|artist| (artist.name.clone(), artist.id))
                .collect::<Vec<_>>();
            let chosen_id = self.chosen_id;
            self.c_picker.children = artists
                .iter()
                .map(|a| {
                    let sender = self.event_sender.clone();
                    let name = a.0.clone();
                    let id = a.1;
                    Button::new(
                        GuiElemCfg::default(),
                        move |_| {
                            sender
                                .send(Event::SetArtist(name.clone(), Some(id)))
                                .unwrap();
                            vec![]
                        },
                        [Label::new(
                            GuiElemCfg::default(),
                            a.0.clone(),
                            if chosen_id.is_some_and(|c| c == a.1) {
                                Color::WHITE
                            } else {
                                Color::LIGHT_GRAY
                            },
                            None,
                            Vec2::new(0.0, 0.5),
                        )],
                    )
                })
                .collect();
            self.c_picker.config_mut().redraw = true;
            self.last_search = search;
        }
    }
    fn config(&self) -> &GuiElemCfg {
        &self.config
    }
    fn config_mut(&mut self) -> &mut GuiElemCfg {
        &mut self.config
    }
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut dyn GuiElem> + '_> {
        Box::new([self.c_name.elem_mut(), self.c_picker.elem_mut()].into_iter())
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn elem(&self) -> &dyn GuiElem {
        self
    }
    fn elem_mut(&mut self) -> &mut dyn GuiElem {
        self
    }
}
