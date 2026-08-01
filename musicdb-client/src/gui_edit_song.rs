use std::time::Instant;

use musicdb_lib::{
    data::{ArtistId, song::Song},
    server::{Action, Req},
};
use speedy2d::{color::Color, dimen::Vec2, shape::Rectangle};

use crate::{
    color_scale,
    gui::{GuiAction, GuiElem, GuiElemCfg, GuiElemChildren},
    gui_anim::AnimationController,
    gui_base::{Button, Panel, ScrollBox},
    gui_edit_any::{ELEM_HEIGHT, EditorForAnyTagAdder, EditorForAnyTagInList, SpacerForScrollBox},
    gui_text::{Label, TextField},
};

// TODO: Fix bug where after selecting an artist you can't mouse-click the buttons anymore (to change it)

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
#[allow(clippy::enum_variant_names)]
pub enum Event {
    Close,
    Apply,
    SetArtist(String, Option<ArtistId>),
    GeneralEvent(super::gui_edit_any::Event),
}
impl From<super::gui_edit_any::Event> for Event {
    fn from(value: super::gui_edit_any::Event) -> Self {
        Self::GeneralEvent(value)
    }
}

pub struct EditorForSongElems {
    c_title: TextField,
    c_artist: EditorForSongArtistChooser,
    c_album: Label,
    c_tags: Vec<EditorForAnyTagInList>,
    c_new_tag: EditorForAnyTagAdder<Event>,
    c_spacers: [SpacerForScrollBox; 4],
}
impl GuiElemChildren for EditorForSongElems {
    fn iter(&mut self) -> Box<dyn Iterator<Item = &mut dyn crate::gui::GuiElem> + '_> {
        Box::new(
            [
                self.c_title.elem_mut(),
                self.c_artist.elem_mut(),
                self.c_album.elem_mut(),
            ]
            .into_iter()
            .chain(self.c_tags.iter_mut().map(|e| e.elem_mut()))
            .chain(std::iter::once(self.c_new_tag.elem_mut())),
        )
    }
    fn len(&self) -> usize {
        3 + self.c_tags.len() + 1 + self.c_spacers.len()
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
                        "(todo...)".to_owned(),
                        Color::GRAY,
                        None,
                        Vec2::new(0.0, 0.5),
                    ),
                    c_tags: {
                        let mut tags = Vec::new();
                        for song in songs.iter() {
                            for tag in song.general.tags.iter() {
                                if !tags.contains(&tag.as_str()) {
                                    tags.push(tag.as_str());
                                }
                            }
                        }
                        tags.into_iter()
                            .enumerate()
                            .map(|(i, tag)| {
                                EditorForAnyTagInList::new(
                                    tag.to_owned(),
                                    i,
                                    sender.clone(),
                                    GuiElemCfg::default(),
                                )
                            })
                            .collect()
                    },
                    c_new_tag: EditorForAnyTagAdder::new(sender.clone()),
                    c_spacers: [
                        SpacerForScrollBox::new(),
                        SpacerForScrollBox::new(),
                        SpacerForScrollBox::new(),
                        SpacerForScrollBox::new(),
                    ],
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
        while let Ok(e) = self.event_recv.try_recv() {
            match e {
                Event::Close => info.actions.push(GuiAction::Do(Box::new(|gui| {
                    gui.gui.c_editing_songs = None;
                    gui.gui.set_normal_ui_enabled(true);
                }))),
                Event::Apply => {
                    let mut actions = Vec::new();
                    for song in self.songs.iter() {
                        let mut song = song.clone();

                        let new_title = self
                            .c_scrollbox
                            .children
                            .c_title
                            .c_input
                            .content
                            .get_text()
                            .trim();
                        if !new_title.is_empty() {
                            song.title = new_title.to_owned();
                        }

                        if let Some(artist_id) = self.c_scrollbox.children.c_artist.chosen_id {
                            song.artist = artist_id;
                            song.album = None;
                        }
                        actions.push(Action::ModifySong(song, Req::none()));
                    }
                    if actions.len() == 1 {
                        info.actions
                            .push(GuiAction::SendToServer(actions.pop().unwrap()));
                    } else if actions.len() > 1 {
                        info.actions
                            .push(GuiAction::SendToServer(Action::Multiple(actions)));
                    }
                }
                Event::SetArtist(name, id) => {
                    self.c_scrollbox.children.c_artist.chosen_id = id;
                    self.c_scrollbox.children.c_artist.last_search = name.to_lowercase();
                    self.c_scrollbox
                        .children
                        .c_artist
                        .open_prog
                        .set_target(info.time, 1.0);
                    *self
                        .c_scrollbox
                        .children
                        .c_artist
                        .c_name
                        .c_input
                        .content
                        .text() = name;
                    self.c_scrollbox
                        .children
                        .c_artist
                        .config_mut()
                        .redraw_once();
                }
                Event::GeneralEvent(e) => {
                    use super::gui_edit_any::Event as GeneralEvent;
                    match e {
                        GeneralEvent::RemoveTag(tag) => {
                            for song in self.songs.iter_mut() {
                                if let Some(i) = song.general.tags.iter().position(|t| *t == tag) {
                                    song.general.tags.remove(i);
                                }
                            }
                            if let Some(i) = (&self.c_scrollbox.children.c_tags)
                                .into_iter()
                                .position(|e| e.tag == tag)
                            {
                                self.c_scrollbox.children.c_tags.remove(i);
                                self.c_scrollbox.config_mut().redraw_once();
                            }
                        }
                        GeneralEvent::AddTag(tag) => {
                            self.c_scrollbox.children.c_new_tag.clear(info.time);
                            for song in self.songs.iter_mut() {
                                if !song.general.tags.contains(&tag) {
                                    song.general.tags.push(tag.clone());
                                }
                            }
                            if !(&self.c_scrollbox.children.c_tags)
                                .into_iter()
                                .any(|e| e.tag == tag)
                            {
                                self.c_scrollbox.children_heights.insert(
                                    3 + self.c_scrollbox.children.c_tags.len(),
                                    ELEM_HEIGHT,
                                );
                                let i = self.c_scrollbox.children.c_tags.len();
                                self.c_scrollbox
                                    .children
                                    .c_tags
                                    .push(EditorForAnyTagInList::new(
                                        tag,
                                        i,
                                        self.event_sender.clone(),
                                        GuiElemCfg::default(),
                                    ));
                                self.c_scrollbox.config_mut().redraw_once();
                            }
                        }
                    }
                }
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
        if let Ok(val) = self
            .c_scrollbox
            .children
            .c_artist
            .open_prog
            .update(info.time, false)
        {
            if let Some(v) = self.c_scrollbox.children_heights.get_mut(1) {
                *v = ELEM_HEIGHT * val as f32;
                self.c_scrollbox.config_mut().redraw_once();
            }
            if let Some(h) = &info.helper {
                h.request_redraw();
            }
        }
        if let Ok(val) = self
            .c_scrollbox
            .children
            .c_new_tag
            .open_prog
            .update(info.time, false)
        {
            if let Some(v) = self
                .c_scrollbox
                .children_heights
                .get_mut(3 + self.c_scrollbox.children.c_tags.len())
            {
                *v = ELEM_HEIGHT * val as f32;
                self.c_scrollbox.config_mut().redraw_once();
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
    open_prog: AnimationController,
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
            open_prog: AnimationController::new(1.0, 1.0, 4.0),
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
        let picker_enabled = self.open_prog.value(info.time) > 1.0;
        self.c_picker.config_mut().enabled = picker_enabled;
        if picker_enabled {
            let split = 1.0 / self.open_prog.value(info.time) as f32;
            self.c_name.config_mut().pos = Rectangle::from_tuples((0.0, 0.0), (1.0, split));
            self.c_picker.config_mut().pos = Rectangle::from_tuples((0.0, split), (1.0, 1.0));
        } else {
            self.c_name.config_mut().pos = Rectangle::from_tuples((0.0, 0.0), (1.0, 1.0));
        }

        let search = self.c_name.c_input.content.get_text().to_lowercase();
        let search_changed = self.last_search != search;
        if self.config.redraw() || search_changed {
            *self.c_name.c_input.content.color() = if self.chosen_id.is_some() {
                Color::GREEN
            } else {
                Color::WHITE
            };
            if search_changed {
                self.chosen_id = None;
                if search.is_empty() {
                    self.open_prog.set_target(info.time, 1.0);
                } else {
                    self.open_prog.set_target(info.time, self.expand_to as f64);
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
            self.c_picker.config_mut().redraw_once();
            self.last_search = search;
            self.config.redrawn();
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
