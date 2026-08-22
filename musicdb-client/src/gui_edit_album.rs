use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use musicdb_lib::{
    data::{ArtistId, album::Album},
    server::{Action, Req},
};
use speedy2d::{color::Color, dimen::Vec2, shape::Rectangle};

use crate::{
    color_scale,
    gui::{GuiAction, GuiElem, GuiElemCfg, GuiElemChildren},
    gui_base::{Button, Panel, ScrollBox},
    gui_edit_any::{
        self, ELEM_HEIGHT, EditorArtistChooser, EditorForAnyTagAdder, EditorForAnyTagInList,
        SpacerForScrollBox,
    },
    gui_screen::EditorForAny,
    gui_text::{Label, TextField},
};

// TODO: Fix bug where after selecting an artist you can't mouse-click the buttons anymore (to change it)

pub struct EditorForAlbums {
    config: GuiElemCfg,
    albums: Vec<Album>,
    c_title: Label,
    c_scrollbox: ScrollBox<EditorForAlbumElems>,
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

pub struct EditorForAlbumElems {
    c_title: TextField,
    c_artist: EditorArtistChooser,
    c_tags: Vec<EditorForAnyTagInList<Event>>,
    c_new_tag: EditorForAnyTagAdder<Event>,
    c_spacers: [SpacerForScrollBox; 4],
}
impl GuiElemChildren for EditorForAlbumElems {
    fn iter(&mut self) -> Box<dyn Iterator<Item = &mut dyn crate::gui::GuiElem> + '_> {
        Box::new(
            [self.c_title.elem_mut(), self.c_artist.elem_mut()]
                .into_iter()
                .chain(self.c_tags.iter_mut().map(|e| e.elem_mut()))
                .chain(std::iter::once(self.c_new_tag.elem_mut())),
        )
    }
    fn len(&self) -> usize {
        2 + self.c_tags.len() + 1 + self.c_spacers.len()
    }
}

impl EditorForAlbums {
    pub fn new(albums: Vec<Album>) -> Self {
        Self::new_internal(albums, true)
    }
    pub fn new_instant(albums: Vec<Album>) -> Self {
        Self::new_internal(albums, false)
    }
    fn new_internal(albums: Vec<Album>, open_animation: bool) -> Self {
        let (sender, recv) = std::sync::mpsc::channel();
        Self {
            config: GuiElemCfg::at(Rectangle::from_tuples((0.0, 1.0), (1.0, 2.0))),
            c_title: Label::new(
                GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.0), (1.0, 0.05))),
                format!("Editing {} albums", albums.len()),
                Color::LIGHT_GRAY,
                None,
                Vec2::new(0.5, 0.5),
            ),
            c_scrollbox: ScrollBox::new(
                GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.05), (1.0, 0.95))),
                crate::gui_base::ScrollBoxSizeUnit::Pixels,
                EditorForAlbumElems {
                    c_title: TextField::new(
                        GuiElemCfg::default(),
                        format!(
                            "Name ({})",
                            albums
                                .iter()
                                .enumerate()
                                .map(|(i, s)| format!(
                                    "{}{}",
                                    if i == 0 { "" } else { ", " },
                                    s.name
                                ))
                                .collect::<String>()
                        ),
                        color_scale(Color::MAGENTA, 0.6, 0.6, 0.6, Some(0.75)),
                        Color::MAGENTA,
                    ),
                    c_artist: {
                        let sender = sender.clone();
                        EditorArtistChooser::new(Arc::new(move |name, id| {
                            sender.send(Event::SetArtist(name, Some(id))).unwrap()
                        }))
                    },
                    c_tags: {
                        let mut tags = Vec::new();
                        for album in albums.iter() {
                            for tag in album.general.tags.iter() {
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
            created: Some(
                (open_animation.then(Instant::now))
                    .unwrap_or_else(|| Instant::now() - Duration::from_secs(5)),
            ),
            albums,
            event_sender: sender,
            event_recv: recv,
        }
    }
}

impl GuiElem for EditorForAlbums {
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
                    gui.gui.c_editing = EditorForAny::None;
                    gui.gui.set_normal_ui_enabled(true);
                }))),
                Event::Apply => {
                    let mut actions = Vec::new();
                    gui_edit_any::apply(
                        self.albums.iter_mut().map(|v| &mut v.general),
                        &mut self.c_scrollbox.children.c_tags,
                    );
                    for album in self.albums.iter() {
                        let mut album = album.clone();

                        let new_title = self
                            .c_scrollbox
                            .children
                            .c_title
                            .c_input
                            .content
                            .get_text()
                            .trim();
                        if !new_title.is_empty() {
                            album.name = new_title.to_owned();
                        }

                        if let Some(artist_id) = self.c_scrollbox.children.c_artist.chosen_id {
                            album.artist = artist_id;
                        }
                        actions.push(Action::ModifyAlbum(album, Req::none()));
                    }
                    if actions.len() == 1 {
                        info.actions
                            .push(GuiAction::SendToServer(actions.pop().unwrap()));
                    } else if actions.len() > 1 {
                        info.actions
                            .push(GuiAction::SendToServer(Action::Multiple(actions)));
                    }
                    *self = Self::new_instant(std::mem::take(&mut self.albums));
                }
                Event::SetArtist(name, id) => {
                    self.c_scrollbox
                        .children
                        .c_artist
                        .set_artist(name, id, info.time);
                }
                Event::GeneralEvent(e) => {
                    if e.handle(
                        self.albums.iter_mut().map(|v| &mut v.general),
                        &mut self.c_scrollbox.children.c_tags,
                        &mut self.c_scrollbox.children.c_new_tag,
                        (2, &mut self.c_scrollbox.children_heights),
                        &self.event_sender,
                        info.time,
                    ) {
                        self.c_scrollbox.config_mut().redraw_once();
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
        if self
            .c_scrollbox
            .children
            .c_artist
            .draw(self.c_scrollbox.children_heights.get_mut(1), info.time)
        {
            self.c_scrollbox.config_mut().redraw_once();
            if let Some(h) = &info.helper {
                h.request_redraw();
            }
        }
        // tag sel
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
                .get_mut(2 + self.c_scrollbox.children.c_tags.len())
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
