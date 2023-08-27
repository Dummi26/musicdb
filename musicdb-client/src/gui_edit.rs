use std::sync::{atomic::AtomicBool, mpsc, Arc};

use musicdb_lib::{
    data::{album::Album, artist::Artist, song::Song, AlbumId, ArtistId, SongId},
    server::Command,
};
use speedy2d::{color::Color, dimen::Vec2, shape::Rectangle};

use crate::{
    gui::{GuiAction, GuiElem, GuiElemCfg, GuiElemTrait},
    gui_base::{Button, Panel, ScrollBox},
    gui_text::{Label, TextField},
};

pub struct GuiEdit {
    config: GuiElemCfg,
    children: Vec<GuiElem>,
    editable: Editable,
    editing: Editing,
    reload: bool,
    send: bool,
    apply_change: mpsc::Sender<Box<dyn FnOnce(&mut Self)>>,
    change_recv: mpsc::Receiver<Box<dyn FnOnce(&mut Self)>>,
}
#[derive(Clone)]
pub enum Editable {
    Artist(ArtistId),
    Album(AlbumId),
    Song(SongId),
}
#[derive(Clone)]
pub enum Editing {
    NotLoaded,
    Artist(Artist),
    Album(Album),
    Song(Song),
}
impl GuiEdit {
    pub fn new(config: GuiElemCfg, edit: Editable) -> Self {
        let (apply_change, change_recv) = mpsc::channel();
        let ac1 = apply_change.clone();
        let ac2 = apply_change.clone();
        Self {
            config,
            editable: edit,
            editing: Editing::NotLoaded,
            reload: true,
            send: false,
            apply_change,
            change_recv,
            children: vec![
                GuiElem::new(Button::new(
                    GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.95), (0.33, 1.0))),
                    |_| vec![GuiAction::CloseEditPanel],
                    vec![GuiElem::new(Label::new(
                        GuiElemCfg::default(),
                        "Back".to_string(),
                        Color::WHITE,
                        None,
                        Vec2::new(0.5, 0.5),
                    ))],
                )),
                GuiElem::new(Button::new(
                    GuiElemCfg::at(Rectangle::from_tuples((0.33, 0.95), (0.67, 1.0))),
                    move |_| {
                        _ = ac1.send(Box::new(|s| s.reload = true));
                        vec![]
                    },
                    vec![GuiElem::new(Label::new(
                        GuiElemCfg::default(),
                        "Reload".to_string(),
                        Color::WHITE,
                        None,
                        Vec2::new(0.5, 0.5),
                    ))],
                )),
                GuiElem::new(Button::new(
                    GuiElemCfg::at(Rectangle::from_tuples((0.67, 0.95), (1.0, 1.0))),
                    move |_| {
                        _ = ac2.send(Box::new(|s| s.send = true));
                        vec![]
                    },
                    vec![GuiElem::new(Label::new(
                        GuiElemCfg::default(),
                        "Send".to_string(),
                        Color::WHITE,
                        None,
                        Vec2::new(0.5, 0.5),
                    ))],
                )),
            ],
        }
    }
}
impl GuiElemTrait for GuiEdit {
    fn config(&self) -> &GuiElemCfg {
        &self.config
    }
    fn config_mut(&mut self) -> &mut GuiElemCfg {
        &mut self.config
    }
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut GuiElem> + '_> {
        Box::new(self.children.iter_mut())
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn clone_gui(&self) -> Box<dyn GuiElemTrait> {
        Box::new(self.clone())
    }
    fn draw(&mut self, info: &mut crate::gui::DrawInfo, g: &mut speedy2d::Graphics2D) {
        loop {
            if let Ok(func) = self.change_recv.try_recv() {
                func(self);
            } else {
                break;
            }
        }
        if self.send {
            self.send = false;
            match &self.editing {
                Editing::NotLoaded => {}
                Editing::Artist(v) => info
                    .actions
                    .push(GuiAction::SendToServer(Command::ModifyArtist(v.clone()))),
                Editing::Album(v) => info
                    .actions
                    .push(GuiAction::SendToServer(Command::ModifyAlbum(v.clone()))),
                Editing::Song(v) => info
                    .actions
                    .push(GuiAction::SendToServer(Command::ModifySong(v.clone()))),
            }
        }
        if self.reload {
            self.reload = false;
            self.editing = match &self.editable {
                Editable::Artist(id) => {
                    if let Some(v) = info.database.artists().get(id).cloned() {
                        Editing::Artist(v)
                    } else {
                        Editing::NotLoaded
                    }
                }
                Editable::Album(id) => {
                    if let Some(v) = info.database.albums().get(id).cloned() {
                        Editing::Album(v)
                    } else {
                        Editing::NotLoaded
                    }
                }
                Editable::Song(id) => {
                    if let Some(v) = info.database.songs().get(id).cloned() {
                        Editing::Song(v)
                    } else {
                        Editing::NotLoaded
                    }
                }
            };
            self.config.redraw = true;
        }
        if self.config.redraw {
            self.config.redraw = false;
            let scrollbox = if self.children.len() > 3 {
                let o = self.children.pop();
                while self.children.len() > 3 {
                    self.children.pop();
                }
                o
            } else {
                None
            };
            match &self.editing {
                Editing::NotLoaded => {
                    self.children.push(GuiElem::new(Label::new(
                        GuiElemCfg::default(),
                        "nothing here".to_string(),
                        Color::WHITE,
                        None,
                        Vec2::new(0.5, 0.5),
                    )));
                }
                Editing::Artist(artist) => {
                    self.children.push(GuiElem::new(Label::new(
                        GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.0), (0.8, 0.08))),
                        artist.name.clone(),
                        Color::WHITE,
                        None,
                        Vec2::new(0.1, 0.5),
                    )));
                    self.children.push(GuiElem::new(Label::new(
                        GuiElemCfg::at(Rectangle::from_tuples((0.8, 0.0), (1.0, 0.04))),
                        "Artist".to_string(),
                        Color::WHITE,
                        None,
                        Vec2::new(0.8, 0.5),
                    )));
                    self.children.push(GuiElem::new(Label::new(
                        GuiElemCfg::at(Rectangle::from_tuples((0.8, 0.04), (1.0, 0.08))),
                        format!("#{}", artist.id),
                        Color::WHITE,
                        None,
                        Vec2::new(0.8, 0.5),
                    )));
                    let mut elems = vec![];
                    elems.push((
                        GuiElem::new(Panel::new(
                            GuiElemCfg::default(),
                            vec![
                                GuiElem::new(Label::new(
                                    GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.0), (0.6, 1.0))),
                                    format!(
                                        "{} album{}",
                                        artist.albums.len(),
                                        if artist.albums.len() != 1 { "s" } else { "" }
                                    ),
                                    Color::LIGHT_GRAY,
                                    None,
                                    Vec2::new(0.0, 0.5),
                                )),
                                GuiElem::new(TextField::new(
                                    GuiElemCfg::at(Rectangle::from_tuples((0.6, 0.0), (0.8, 1.0))),
                                    "id".to_string(),
                                    Color::DARK_GRAY,
                                    Color::WHITE,
                                )),
                                GuiElem::new(Button::new(
                                    GuiElemCfg::at(Rectangle::from_tuples((0.8, 0.0), (1.0, 1.0))),
                                    {
                                        let apply_change = self.apply_change.clone();
                                        let my_index = elems.len();
                                        move |_| {
                                            _ = apply_change.send(Box::new(move |s| {
                                                s.config.redraw = true;
                                                if let Ok(id) = s
                                                    .children
                                                    .last_mut()
                                                    .unwrap()
                                                    .inner
                                                    .any_mut()
                                                    .downcast_mut::<ScrollBox>()
                                                    .unwrap()
                                                    .children[my_index]
                                                    .0
                                                    .inner
                                                    .children()
                                                    .nth(1)
                                                    .unwrap()
                                                    .inner
                                                    .children()
                                                    .next()
                                                    .unwrap()
                                                    .inner
                                                    .any()
                                                    .downcast_ref::<Label>()
                                                    .unwrap()
                                                    .content
                                                    .get_text()
                                                    .parse::<AlbumId>()
                                                {
                                                    if let Editing::Artist(artist) = &mut s.editing
                                                    {
                                                        artist.albums.push(id);
                                                    }
                                                }
                                            }));
                                            vec![]
                                        }
                                    },
                                    vec![GuiElem::new(Label::new(
                                        GuiElemCfg::default(),
                                        "add".to_string(),
                                        Color::LIGHT_GRAY,
                                        None,
                                        Vec2::new(0.0, 0.5),
                                    ))],
                                )),
                            ],
                        )),
                        info.line_height,
                    ));
                    for &album in &artist.albums {
                        elems.push((
                            GuiElem::new(Button::new(
                                GuiElemCfg::default(),
                                move |_| {
                                    vec![GuiAction::OpenEditPanel(GuiElem::new(GuiEdit::new(
                                        GuiElemCfg::default(),
                                        Editable::Album(album),
                                    )))]
                                },
                                vec![GuiElem::new(Label::new(
                                    GuiElemCfg::default(),
                                    if let Some(a) = info.database.albums().get(&album) {
                                        format!("Album: {}", a.name)
                                    } else {
                                        format!("Album #{album}")
                                    },
                                    Color::WHITE,
                                    None,
                                    Vec2::new(0.0, 0.5),
                                ))],
                            )),
                            info.line_height,
                        ));
                    }
                    self.children.push(if let Some(mut sb) = scrollbox {
                        if let Some(s) = sb.inner.any_mut().downcast_mut::<ScrollBox>() {
                            s.children = elems;
                            s.config_mut().redraw = true;
                            sb
                        } else {
                            GuiElem::new(ScrollBox::new(
                                GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.08), (1.0, 1.0))),
                                crate::gui_base::ScrollBoxSizeUnit::Pixels,
                                elems,
                            ))
                        }
                    } else {
                        GuiElem::new(ScrollBox::new(
                            GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.08), (1.0, 1.0))),
                            crate::gui_base::ScrollBoxSizeUnit::Pixels,
                            elems,
                        ))
                    });
                }
                Editing::Album(album) => {
                    self.children.push(GuiElem::new(Label::new(
                        GuiElemCfg::default(),
                        format!("Album: {}", album.name),
                        Color::WHITE,
                        None,
                        Vec2::new(0.5, 0.5),
                    )));
                }
                Editing::Song(song) => {
                    self.children.push(GuiElem::new(Label::new(
                        GuiElemCfg::default(),
                        format!("Song: {}", song.title),
                        Color::WHITE,
                        None,
                        Vec2::new(0.5, 0.5),
                    )));
                }
            }
        };
        match self.editing {
            _ => {}
        }
    }
}

impl Clone for GuiEdit {
    fn clone(&self) -> Self {
        Self::new(self.config.clone(), self.editable.clone())
    }
}
