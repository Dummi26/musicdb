use std::{
    collections::{HashMap, HashSet},
    sync::mpsc,
};

use musicdb_lib::{
    data::{
        album::Album, artist::Artist, queue::QueueContent, song::Song, AlbumId, ArtistId, CoverId,
        SongId,
    },
    server::Command,
};
use speedy2d::{color::Color, dimen::Vec2, shape::Rectangle};

use crate::{
    gui::{Dragging, DrawInfo, GuiAction, GuiElemCfg, GuiElemTrait},
    gui_base::{Button, Panel, ScrollBox},
    gui_text::{Label, TextField},
};

pub struct GuiEdit {
    config: GuiElemCfg,
    children: Vec<Box<dyn GuiElemTrait>>,
    editable: Editable,
    editing: Editing,
    reload: bool,
    rebuild_main: bool,
    rebuild_changes: bool,
    send: bool,
    apply_change: mpsc::Sender<Box<dyn FnOnce(&mut Self)>>,
    change_recv: mpsc::Receiver<Box<dyn FnOnce(&mut Self)>>,
}
#[derive(Clone)]
pub enum Editable {
    Artist(Vec<ArtistId>),
    Album(Vec<AlbumId>),
    Song(Vec<SongId>),
}
#[derive(Clone)]
pub enum Editing {
    NotLoaded,
    Artist(Vec<Artist>, Vec<ArtistChange>),
    Album(Vec<Album>, Vec<AlbumChange>),
    Song(Vec<Song>, Vec<SongChange>),
}
#[derive(Clone)]
pub enum ArtistChange {
    SetName(String),
    SetCover(Option<CoverId>),
    AddAlbum(AlbumId),
}
#[derive(Clone)]
pub enum AlbumChange {
    SetName(String),
    SetCover(Option<ArtistId>),
    RemoveSong(SongId),
    AddSong(SongId),
}
#[derive(Clone)]
pub enum SongChange {
    SetTitle(String),
    SetCover(Option<ArtistId>),
}

impl GuiEdit {
    pub fn new(config: GuiElemCfg, edit: Editable) -> Self {
        let (apply_change, change_recv) = mpsc::channel();
        let ac1 = apply_change.clone();
        let ac2 = apply_change.clone();
        Self {
            config: config.w_drag_target(),
            editable: edit,
            editing: Editing::NotLoaded,
            reload: true,
            rebuild_main: true,
            rebuild_changes: true,
            send: false,
            apply_change,
            change_recv,
            children: vec![
                Box::new(ScrollBox::new(
                    GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.0), (1.0, 0.6))),
                    crate::gui_base::ScrollBoxSizeUnit::Pixels,
                    vec![],
                )),
                Box::new(ScrollBox::new(
                    GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.6), (1.0, 0.9))),
                    crate::gui_base::ScrollBoxSizeUnit::Pixels,
                    vec![],
                )),
                Box::new(Button::new(
                    GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.95), (0.33, 1.0))),
                    |_| vec![GuiAction::CloseEditPanel],
                    vec![Box::new(Label::new(
                        GuiElemCfg::default(),
                        "Back".to_string(),
                        Color::WHITE,
                        None,
                        Vec2::new(0.5, 0.5),
                    ))],
                )),
                Box::new(Button::new(
                    GuiElemCfg::at(Rectangle::from_tuples((0.33, 0.95), (0.67, 1.0))),
                    move |_| {
                        _ = ac1.send(Box::new(|s| s.reload = true));
                        vec![]
                    },
                    vec![Box::new(Label::new(
                        GuiElemCfg::default(),
                        "Reload".to_string(),
                        Color::WHITE,
                        None,
                        Vec2::new(0.5, 0.5),
                    ))],
                )),
                Box::new(Button::new(
                    GuiElemCfg::at(Rectangle::from_tuples((0.67, 0.95), (1.0, 1.0))),
                    move |_| {
                        _ = ac2.send(Box::new(|s| s.send = true));
                        vec![]
                    },
                    vec![Box::new(Label::new(
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
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut dyn GuiElemTrait> + '_> {
        Box::new(self.children.iter_mut().map(|v| v.as_mut()))
    }
    fn any(&self) -> &dyn std::any::Any {
        self
    }
    fn any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn elem(&self) -> &dyn GuiElemTrait {
        self
    }
    fn elem_mut(&mut self) -> &mut dyn GuiElemTrait {
        self
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
            self.rebuild_main = true;
            self.rebuild_changes = true;
            self.config.redraw = true;
            match &mut self.editing {
                Editing::NotLoaded => {}
                Editing::Artist(v, changes) => {
                    for change in changes.iter() {
                        match change {
                            ArtistChange::SetName(n) => {
                                for artist in v.iter_mut() {
                                    artist.name = n.clone();
                                    info.actions.push(GuiAction::SendToServer(
                                        Command::ModifyArtist(artist.clone()),
                                    ));
                                }
                            }
                            ArtistChange::SetCover(c) => {
                                for artist in v.iter_mut() {
                                    artist.cover = c.clone();
                                    info.actions.push(GuiAction::SendToServer(
                                        Command::ModifyArtist(artist.clone()),
                                    ));
                                }
                            }
                            ArtistChange::AddAlbum(id) => {
                                // use the first artist for the artist fields
                                let mut editing = v.first().unwrap().clone();
                                if let Some(album) = info.database.albums().get(id) {
                                    let mut album = album.clone();
                                    // find the previous artist for this album and remove them
                                    if let Some(prev) = info.database.artists().get(&album.artist) {
                                        let mut prev = prev.clone();
                                        if let Some(i) = prev.albums.iter().position(|v| v == id) {
                                            prev.albums.remove(i);
                                            info.actions.push(GuiAction::SendToServer(
                                                Command::ModifyArtist(prev),
                                            ));
                                        }
                                    }
                                    // update the artist field on the album so it points to the new artist
                                    album.artist = editing.id;
                                    info.actions
                                        .push(GuiAction::SendToServer(Command::ModifyAlbum(album)));
                                    // add the album to the artist we are editing
                                    if !editing.albums.contains(id) {
                                        editing.albums.push(*id);
                                        info.actions.push(GuiAction::SendToServer(
                                            Command::ModifyArtist(editing),
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
                Editing::Album(v, changes) => {
                    for v in v {
                        let mut v = v.clone();
                        for change in changes.iter() {
                            todo!()
                        }
                        info.actions
                            .push(GuiAction::SendToServer(Command::ModifyAlbum(v)));
                    }
                }
                Editing::Song(v, changes) => {
                    for v in v {
                        let mut v = v.clone();
                        for change in changes.iter() {
                            todo!()
                        }
                        info.actions
                            .push(GuiAction::SendToServer(Command::ModifySong(v)));
                    }
                }
            }
        }
        if self.reload {
            self.reload = false;
            let prev = std::mem::replace(&mut self.editing, Editing::NotLoaded);
            self.editing = match &self.editable {
                Editable::Artist(id) => {
                    let v = id
                        .iter()
                        .filter_map(|id| info.database.artists().get(id).cloned())
                        .collect::<Vec<_>>();
                    if !v.is_empty() {
                        Editing::Artist(
                            v,
                            if let Editing::Artist(_, c) = prev {
                                c
                            } else {
                                vec![]
                            },
                        )
                    } else {
                        Editing::NotLoaded
                    }
                }
                Editable::Album(id) => {
                    let v = id
                        .iter()
                        .filter_map(|id| info.database.albums().get(id).cloned())
                        .collect::<Vec<_>>();
                    if !v.is_empty() {
                        Editing::Album(
                            v,
                            if let Editing::Album(_, c) = prev {
                                c
                            } else {
                                vec![]
                            },
                        )
                    } else {
                        Editing::NotLoaded
                    }
                }
                Editable::Song(id) => {
                    let v = id
                        .iter()
                        .filter_map(|id| info.database.songs().get(id).cloned())
                        .collect::<Vec<_>>();
                    if !v.is_empty() {
                        Editing::Song(
                            v,
                            if let Editing::Song(_, c) = prev {
                                c
                            } else {
                                vec![]
                            },
                        )
                    } else {
                        Editing::NotLoaded
                    }
                }
            };
            self.config.redraw = true;
            self.rebuild_main = true;
            self.rebuild_changes = true;
        }
        if let Some(sb) = self.children[0].any_mut().downcast_mut::<ScrollBox>() {
            for (c, _) in sb.children.iter() {
                if let Some(p) = c
                    .any()
                    .downcast_ref::<Panel>()
                    .and_then(|p| p.children.get(0))
                    .and_then(|e| e.any().downcast_ref::<TextField>())
                {
                    if p.label_input().content.will_redraw() {
                        if let Some((key, _)) = p.label_hint().content.get_text().split_once(':') {
                            match (&mut self.editing, key) {
                                (Editing::Artist(_, changes), "name") => {
                                    let mut c = changes.iter_mut();
                                    loop {
                                        if let Some(c) = c.next() {
                                            if let ArtistChange::SetName(n) = c {
                                                *n = p.label_input().content.get_text().clone();
                                                break;
                                            }
                                        } else {
                                            changes.push(ArtistChange::SetName(
                                                p.label_input().content.get_text().clone(),
                                            ));
                                            break;
                                        }
                                    }
                                    self.rebuild_changes = true;
                                }
                                (Editing::Artist(_, changes), "cover") => {
                                    let mut c = changes.iter_mut();
                                    loop {
                                        if let Some(c) = c.next() {
                                            if let ArtistChange::SetCover(n) = c {
                                                *n =
                                                    p.label_input().content.get_text().parse().ok();
                                                break;
                                            }
                                        } else {
                                            changes.push(ArtistChange::SetCover(
                                                p.label_input().content.get_text().parse().ok(),
                                            ));
                                            break;
                                        }
                                    }
                                    self.rebuild_changes = true;
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
        if self.rebuild_main {
            self.rebuild_main = false;
            self.rebuild_main(info);
        }
        if self.rebuild_changes {
            self.rebuild_changes = false;
            self.rebuild_changes(info);
        }
        if self.config.redraw {
            self.config.redraw = false;
            if let Some(sb) = self.children[0].any_mut().downcast_mut::<ScrollBox>() {
                for c in sb.children.iter_mut() {
                    c.1 = info.line_height;
                }
            }
        }
    }
    fn dragged(&mut self, dragged: Dragging) -> Vec<GuiAction> {
        let dragged = match dragged {
            Dragging::Artist(_) | Dragging::Album(_) | Dragging::Song(_) | Dragging::Queues(_) => {
                dragged
            }
            Dragging::Queue(q) => match q.content() {
                QueueContent::Song(id) => Dragging::Song(*id),
                _ => Dragging::Queue(q),
            },
        };
        match dragged {
            Dragging::Artist(id) => {
                if let Editing::Artist(a, _) = &self.editing {
                    self.editable = Editable::Artist(a.iter().map(|v| v.id).chain([id]).collect())
                }
            }
            Dragging::Album(id) => {
                if let Editing::Album(a, _) = &self.editing {
                    self.editable = Editable::Album(a.iter().map(|v| v.id).chain([id]).collect())
                }
            }
            Dragging::Song(id) => {
                if let Editing::Song(a, _) = &self.editing {
                    self.editable = Editable::Song(a.iter().map(|v| v.id).chain([id]).collect())
                }
            }
            Dragging::Queue(_) => return vec![],
            Dragging::Queues(_) => return vec![],
        }
        self.reload = true;
        vec![]
    }
}
impl GuiEdit {
    fn rebuild_main(&mut self, info: &mut DrawInfo) {
        if let Some(sb) = self.children[0].any_mut().downcast_mut::<ScrollBox>() {
            sb.children.clear();
            sb.config_mut().redraw = true;
            match &self.editing {
                Editing::NotLoaded => {}
                Editing::Artist(v, _) => {
                    // name
                    let mut names = v
                        .iter()
                        .map(|v| &v.name)
                        .collect::<HashSet<_>>()
                        .into_iter()
                        .collect::<Vec<_>>();
                    names.sort_unstable();
                    let name = if names.len() == 1 {
                        format!("name: {}", names[0])
                    } else {
                        let mut name = format!("name: {}", names[0]);
                        for n in names.iter().skip(1) {
                            name.push_str(" / ");
                            name.push_str(n);
                        }
                        name
                    };
                    sb.children.push((
                        Box::new(Panel::new(
                            GuiElemCfg::default(),
                            vec![Box::new(TextField::new(
                                GuiElemCfg::default(),
                                name,
                                Color::LIGHT_GRAY,
                                Color::WHITE,
                            ))],
                        )),
                        info.line_height,
                    ));
                    // cover
                    let covers = v.iter().filter_map(|v| v.cover).collect::<Vec<_>>();
                    let cover = if covers.is_empty() {
                        format!("cover: None")
                    } else {
                        let mut cover = format!("cover: {}", covers[0]);
                        for c in covers.iter().skip(1) {
                            cover.push('/');
                            cover.push_str(&format!("{c}"));
                        }
                        cover
                    };
                    sb.children.push((
                        Box::new(Panel::new(
                            GuiElemCfg::default(),
                            vec![Box::new(TextField::new(
                                GuiElemCfg::default(),
                                cover,
                                Color::LIGHT_GRAY,
                                Color::WHITE,
                            ))],
                        )),
                        info.line_height,
                    ));
                    // albums
                    let mut albums = HashMap::new();
                    for v in v {
                        for album in &v.albums {
                            if let Some(count) = albums.get_mut(album) {
                                *count += 1;
                            } else {
                                albums.insert(*album, 1);
                            }
                        }
                    }
                    {
                        fn get_id(s: &mut GuiEdit) -> Option<AlbumId> {
                            s.children[0]
                                .children()
                                .collect::<Vec<_>>()
                                .into_iter()
                                .rev()
                                .nth(2)
                                .unwrap()
                                .any_mut()
                                .downcast_mut::<Panel>()
                                .unwrap()
                                .children()
                                .next()
                                .unwrap()
                                .any_mut()
                                .downcast_mut::<TextField>()
                                .unwrap()
                                .label_input()
                                .content
                                .get_text()
                                .parse()
                                .ok()
                        }
                        let add_button = {
                            let apply_change = self.apply_change.clone();
                            Box::new(Button::new(
                                GuiElemCfg::at(Rectangle::from_tuples((0.9, 0.0), (1.0, 1.0))),
                                move |_| {
                                    _ = apply_change.send(Box::new(move |s| {
                                        if let Some(album_id) = get_id(s) {
                                            if let Editing::Artist(_, c) = &mut s.editing {
                                                if let Some(i) = c.iter().position(|c| {
                                                    matches!(c, ArtistChange::AddAlbum(id) if *id == album_id)
                                                }) {
                                                    c.remove(i);
                                                }
                                                c.push(ArtistChange::AddAlbum(album_id));
                                                s.rebuild_changes = true;
                                            }
                                        }
                                    }));
                                    vec![]
                                },
                                vec![Box::new(Label::new(
                                    GuiElemCfg::default(),
                                    format!("add"),
                                    Color::GREEN,
                                    None,
                                    Vec2::new(0.5, 0.5),
                                ))],
                            ))
                        };
                        let name = Box::new(TextField::new(
                            GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.0), (0.9, 1.0))),
                            "add album by id".to_string(),
                            Color::LIGHT_GRAY,
                            Color::WHITE,
                        ));
                        sb.children.push((
                            Box::new(Panel::new(GuiElemCfg::default(), vec![name, add_button])),
                            info.line_height * 2.0,
                        ));
                    }
                    for (album_id, count) in albums {
                        let album = info.database.albums().get(&album_id);
                        let name = Box::new(Button::new(
                            GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.0), (1.0, 1.0))),
                            move |_| {
                                vec![GuiAction::OpenEditPanel(Box::new(GuiEdit::new(
                                    GuiElemCfg::default(),
                                    Editable::Album(vec![album_id]),
                                )))]
                            },
                            vec![Box::new(Label::new(
                                GuiElemCfg::default(),
                                if let Some(a) = album {
                                    a.name.clone()
                                } else {
                                    format!("#{album_id}")
                                },
                                Color::WHITE,
                                None,
                                Vec2::new(0.0, 0.5),
                            ))],
                        ));
                        sb.children.push((
                            Box::new(Panel::new(GuiElemCfg::default(), vec![name])),
                            info.line_height,
                        ));
                    }
                }
                Editing::Album(v, _) => {}
                Editing::Song(v, _) => {}
            }
        }
    }
    fn rebuild_changes(&mut self, info: &mut DrawInfo) {
        if let Some(sb) = self.children[1].any_mut().downcast_mut::<ScrollBox>() {
            sb.children.clear();
            sb.config_mut().redraw = true;
            match &self.editing {
                Editing::NotLoaded => {}
                Editing::Artist(_, a) => {
                    for (i, v) in a.iter().enumerate() {
                        let text = match v {
                            ArtistChange::SetName(v) => format!("set name to \"{v}\""),
                            ArtistChange::SetCover(c) => {
                                if let Some(c) = c {
                                    format!("set cover to {c}")
                                } else {
                                    "remove cover".to_string()
                                }
                            }
                            ArtistChange::AddAlbum(v) => format!("add album {v}"),
                        };
                        let s = self.apply_change.clone();
                        sb.children.push((
                            Box::new(Button::new(
                                GuiElemCfg::default(),
                                move |_| {
                                    _ = s.send(Box::new(move |s| {
                                        if !s.rebuild_changes {
                                            if let Editing::Artist(_, v) = &mut s.editing {
                                                if i < v.len() {
                                                    v.remove(i);
                                                }
                                                s.rebuild_changes = true;
                                            }
                                        }
                                    }));
                                    vec![]
                                },
                                vec![Box::new(Label::new(
                                    GuiElemCfg::default(),
                                    text,
                                    Color::WHITE,
                                    None,
                                    Vec2::new(0.0, 0.5),
                                ))],
                            )),
                            info.line_height,
                        ));
                    }
                }
                _ => todo!(),
            }
        }
    }
}

impl Clone for GuiEdit {
    fn clone(&self) -> Self {
        Self::new(self.config.clone(), self.editable.clone())
    }
}
