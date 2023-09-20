use std::{
    rc::Rc,
    sync::{atomic::AtomicBool, Arc},
};

use musicdb_lib::data::{database::Database, AlbumId, ArtistId, SongId};
use regex::{Regex, RegexBuilder};
use speedy2d::{
    color::Color,
    dimen::Vec2,
    shape::Rectangle,
    window::{MouseButton, VirtualKeyCode},
};

use crate::{
    gui::{Dragging, DrawInfo, GuiAction, GuiElem, GuiElemCfg, GuiElemTrait},
    gui_base::{Button, Panel, ScrollBox},
    gui_edit::GuiEdit,
    gui_text::{Label, TextField},
    gui_wrappers::WithFocusHotkey,
};

/*

This is responsible for showing the library,
with Regex search and drag-n-drop.

*/

#[derive(Clone)]
pub struct LibraryBrowser {
    config: GuiElemCfg,
    pub children: Vec<GuiElem>,
    search_artist: String,
    search_artist_regex: Option<Regex>,
    search_album: String,
    search_album_regex: Option<Regex>,
    search_song: String,
    search_song_regex: Option<Regex>,
    filter_target_state: Rc<AtomicBool>,
    filter_state: f32,
    search_is_case_sensitive: Rc<AtomicBool>,
    search_was_case_sensitive: bool,
}
fn search_regex_new(pat: &str, case_insensitive: bool) -> Result<Regex, regex::Error> {
    RegexBuilder::new(pat)
        .unicode(true)
        .case_insensitive(case_insensitive)
        .build()
}
const LP_LIB1: f32 = 0.1;
const LP_LIB2: f32 = 1.0;
const LP_LIB1S: f32 = 0.4;
impl LibraryBrowser {
    pub fn new(config: GuiElemCfg) -> Self {
        let search_artist = TextField::new(
            GuiElemCfg::at(Rectangle::from_tuples((0.01, 0.01), (0.45, 0.05))),
            "artist".to_string(),
            Color::GRAY,
            Color::WHITE,
        );
        let search_album = TextField::new(
            GuiElemCfg::at(Rectangle::from_tuples((0.55, 0.01), (0.99, 0.05))),
            "album".to_string(),
            Color::GRAY,
            Color::WHITE,
        );
        let search_song = WithFocusHotkey::new_ctrl(
            VirtualKeyCode::F,
            TextField::new(
                GuiElemCfg::at(Rectangle::from_tuples((0.01, 0.06), (0.99, 0.1))),
                "song".to_string(),
                Color::GRAY,
                Color::WHITE,
            ),
        );
        let library_scroll_box = ScrollBox::new(
            GuiElemCfg::at(Rectangle::from_tuples((0.0, LP_LIB1), (1.0, LP_LIB2))),
            crate::gui_base::ScrollBoxSizeUnit::Pixels,
            vec![],
        );
        let filter_target_state = Rc::new(AtomicBool::new(false));
        let fts = Rc::clone(&filter_target_state);
        let filter_button = Button::new(
            GuiElemCfg::at(Rectangle::from_tuples((0.46, 0.01), (0.54, 0.05))),
            move |_| {
                fts.store(
                    !fts.load(std::sync::atomic::Ordering::Relaxed),
                    std::sync::atomic::Ordering::Relaxed,
                );
                vec![]
            },
            vec![GuiElem::new(Label::new(
                GuiElemCfg::default(),
                "more".to_owned(),
                Color::GRAY,
                None,
                Vec2::new(0.5, 0.5),
            ))],
        );
        let search_is_case_sensitive = Rc::new(AtomicBool::new(false));
        Self {
            config,
            children: vec![
                GuiElem::new(search_artist),
                GuiElem::new(search_album),
                GuiElem::new(search_song),
                GuiElem::new(library_scroll_box),
                GuiElem::new(filter_button),
                GuiElem::new(FilterPanel::new(Rc::clone(&search_is_case_sensitive))),
            ],
            search_artist: String::new(),
            search_artist_regex: None,
            search_album: String::new(),
            search_album_regex: None,
            search_song: String::new(),
            search_song_regex: None,
            filter_target_state,
            filter_state: 0.0,
            search_is_case_sensitive,
            search_was_case_sensitive: false,
        }
    }
}
impl GuiElemTrait for LibraryBrowser {
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
    fn draw(&mut self, info: &mut DrawInfo, _g: &mut speedy2d::Graphics2D) {
        // search
        let mut search_changed = false;
        let mut rebuild_regex = false;
        let case_sensitive = self
            .search_is_case_sensitive
            .load(std::sync::atomic::Ordering::Relaxed);
        if self.search_was_case_sensitive != case_sensitive {
            self.search_was_case_sensitive = case_sensitive;
            rebuild_regex = true;
        }
        {
            let v = &mut self.children[0].try_as_mut::<TextField>().unwrap().children[0]
                .try_as_mut::<Label>()
                .unwrap()
                .content;
            if rebuild_regex || v.will_redraw() && self.search_artist != *v.get_text() {
                search_changed = true;
                self.search_artist = v.get_text().clone();
                self.search_artist_regex =
                    search_regex_new(&self.search_artist, !case_sensitive).ok();
                *v.color() = if self.search_artist_regex.is_some() {
                    Color::WHITE
                } else {
                    Color::RED
                };
            }
        }
        {
            let v = &mut self.children[1].try_as_mut::<TextField>().unwrap().children[0]
                .try_as_mut::<Label>()
                .unwrap()
                .content;
            if rebuild_regex || v.will_redraw() && self.search_album != *v.get_text() {
                search_changed = true;
                self.search_album = v.get_text().clone();
                self.search_album_regex =
                    search_regex_new(&self.search_album, !case_sensitive).ok();
                *v.color() = if self.search_album_regex.is_some() {
                    Color::WHITE
                } else {
                    Color::RED
                };
            }
        }
        {
            let v = &mut self.children[2]
                .try_as_mut::<WithFocusHotkey<TextField>>()
                .unwrap()
                .inner
                .children[0]
                .try_as_mut::<Label>()
                .unwrap()
                .content;
            if rebuild_regex || v.will_redraw() && self.search_song != *v.get_text() {
                search_changed = true;
                self.search_song = v.get_text().clone();
                self.search_song_regex = search_regex_new(&self.search_song, !case_sensitive).ok();
                *v.color() = if self.search_song_regex.is_some() {
                    Color::WHITE
                } else {
                    Color::RED
                };
            }
        }
        // filter
        let filter_target_state = self
            .filter_target_state
            .load(std::sync::atomic::Ordering::Relaxed);
        let draw_filter = if filter_target_state && self.filter_state != 1.0 {
            if let Some(h) = &info.helper {
                h.request_redraw();
            }
            self.filter_state += (1.0 - self.filter_state) * 0.2;
            if self.filter_state > 0.999 {
                self.filter_state = 1.0;
            }
            true
        } else if !filter_target_state && self.filter_state != 0.0 {
            if let Some(h) = &info.helper {
                h.request_redraw();
            }
            self.filter_state *= 0.8;
            if self.filter_state < 0.001 {
                self.filter_state = 0.0;
            }
            true
        } else {
            false
        };
        if draw_filter {
            let y = LP_LIB1 + (LP_LIB1S - LP_LIB1) * self.filter_state;
            self.children[3]
                .try_as_mut::<ScrollBox>()
                .unwrap()
                .config_mut()
                .pos = Rectangle::new(Vec2::new(0.0, y), Vec2::new(1.0, LP_LIB2));
            let filter_panel = self.children[5].try_as_mut::<FilterPanel>().unwrap();
            filter_panel.config_mut().pos =
                Rectangle::new(Vec2::new(0.0, LP_LIB1), Vec2::new(1.0, y));
            filter_panel.config.enabled = self.filter_state > 0.0;
        }
        // -
        if self.config.redraw || search_changed || info.pos.size() != self.config.pixel_pos.size() {
            self.config.redraw = false;
            self.update_list(&info.database, info.line_height);
        }
    }
    fn updated_library(&mut self) {
        self.config.redraw = true;
    }
}
impl LibraryBrowser {
    fn update_list(&mut self, db: &Database, line_height: f32) {
        let song_height = line_height;
        let artist_height = song_height * 3.0;
        let album_height = song_height * 2.0;
        // sort artists by name
        let mut artists = db.artists().iter().collect::<Vec<_>>();
        artists.sort_by_key(|v| &v.1.name);
        let mut gui_elements = vec![];
        for (artist_id, artist) in artists {
            if self.search_artist.is_empty()
                || self
                    .search_artist_regex
                    .as_ref()
                    .is_some_and(|regex| regex.is_match(&artist.name))
            {
                let mut artist_gui = Some((
                    GuiElem::new(ListArtist::new(
                        GuiElemCfg::default(),
                        *artist_id,
                        artist.name.clone(),
                    )),
                    artist_height,
                ));
                if self.search_album.is_empty() {
                    for song_id in &artist.singles {
                        if let Some(song) = db.songs().get(song_id) {
                            if self.search_song.is_empty()
                                || self
                                    .search_song_regex
                                    .as_ref()
                                    .is_some_and(|regex| regex.is_match(&song.title))
                            {
                                if let Some(g) = artist_gui.take() {
                                    gui_elements.push(g);
                                }
                                gui_elements.push((
                                    GuiElem::new(ListSong::new(
                                        GuiElemCfg::default(),
                                        *song_id,
                                        song.title.clone(),
                                    )),
                                    song_height,
                                ));
                            }
                        }
                    }
                }
                for album_id in &artist.albums {
                    if let Some(album) = db.albums().get(album_id) {
                        if self.search_album.is_empty()
                            || self
                                .search_album_regex
                                .as_ref()
                                .is_some_and(|regex| regex.is_match(&album.name))
                        {
                            let mut album_gui = Some((
                                GuiElem::new(ListAlbum::new(
                                    GuiElemCfg::default(),
                                    *album_id,
                                    album.name.clone(),
                                )),
                                album_height,
                            ));
                            for song_id in &album.songs {
                                if let Some(song) = db.songs().get(song_id) {
                                    if self.search_song.is_empty()
                                        || self
                                            .search_song_regex
                                            .as_ref()
                                            .is_some_and(|regex| regex.is_match(&song.title))
                                    {
                                        if let Some(g) = artist_gui.take() {
                                            gui_elements.push(g);
                                        }
                                        if let Some(g) = album_gui.take() {
                                            gui_elements.push(g);
                                        }
                                        gui_elements.push((
                                            GuiElem::new(ListSong::new(
                                                GuiElemCfg::default(),
                                                *song_id,
                                                song.title.clone(),
                                            )),
                                            song_height,
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        let scroll_box = self.children[3].try_as_mut::<ScrollBox>().unwrap();
        scroll_box.children = gui_elements;
        scroll_box.config_mut().redraw = true;
    }
}

#[derive(Clone)]
struct ListArtist {
    config: GuiElemCfg,
    id: ArtistId,
    children: Vec<GuiElem>,
    mouse: bool,
    mouse_pos: Vec2,
}
impl ListArtist {
    pub fn new(config: GuiElemCfg, id: ArtistId, name: String) -> Self {
        let label = Label::new(
            GuiElemCfg::default(),
            name,
            Color::from_int_rgb(81, 24, 125),
            None,
            Vec2::new(0.0, 0.5),
        );
        Self {
            config: config.w_mouse(),
            id,
            children: vec![GuiElem::new(label)],
            mouse: false,
            mouse_pos: Vec2::ZERO,
        }
    }
}
impl GuiElemTrait for ListArtist {
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
    fn draw(&mut self, info: &mut DrawInfo, _g: &mut speedy2d::Graphics2D) {
        if self.mouse {
            if info.pos.contains(info.mouse_pos) {
                return;
            } else {
                self.mouse = false;
            }
        }
        self.mouse_pos = Vec2::new(
            info.mouse_pos.x - self.config.pixel_pos.top_left().x,
            info.mouse_pos.y - self.config.pixel_pos.top_left().y,
        );
    }
    fn mouse_down(&mut self, button: MouseButton) -> Vec<GuiAction> {
        if button == MouseButton::Left {
            self.mouse = true;
            let mouse_pos = self.mouse_pos;
            let w = self.config.pixel_pos.width();
            let h = self.config.pixel_pos.height();
            let mut el = GuiElem::new(self.clone());
            vec![GuiAction::SetDragging(Some((
                Dragging::Artist(self.id),
                Some(Box::new(move |i, g| {
                    let sw = i.pos.width();
                    let sh = i.pos.height();
                    let x = (i.mouse_pos.x - mouse_pos.x) / sw;
                    let y = (i.mouse_pos.y - mouse_pos.y) / sh;
                    el.inner.config_mut().pos =
                        Rectangle::from_tuples((x, y), (x + w / sw, y + h / sh));
                    el.draw(i, g)
                })),
            )))]
        } else {
            vec![]
        }
    }
    fn mouse_up(&mut self, button: MouseButton) -> Vec<GuiAction> {
        if self.mouse && button == MouseButton::Left {
            self.mouse = false;
            vec![GuiAction::OpenEditPanel(GuiElem::new(GuiEdit::new(
                GuiElemCfg::default(),
                crate::gui_edit::Editable::Artist(vec![self.id]),
            )))]
        } else {
            vec![]
        }
    }
}

#[derive(Clone)]
struct ListAlbum {
    config: GuiElemCfg,
    id: AlbumId,
    children: Vec<GuiElem>,
    mouse: bool,
    mouse_pos: Vec2,
}
impl ListAlbum {
    pub fn new(config: GuiElemCfg, id: AlbumId, name: String) -> Self {
        let label = Label::new(
            GuiElemCfg::default(),
            name,
            Color::from_int_rgb(8, 61, 47),
            None,
            Vec2::new(0.0, 0.5),
        );
        Self {
            config: config.w_mouse(),
            id,
            children: vec![GuiElem::new(label)],
            mouse: false,
            mouse_pos: Vec2::ZERO,
        }
    }
}
impl GuiElemTrait for ListAlbum {
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
    fn draw(&mut self, info: &mut DrawInfo, _g: &mut speedy2d::Graphics2D) {
        if self.mouse {
            if info.pos.contains(info.mouse_pos) {
                return;
            } else {
                self.mouse = false;
            }
        }
        self.mouse_pos = Vec2::new(
            info.mouse_pos.x - self.config.pixel_pos.top_left().x,
            info.mouse_pos.y - self.config.pixel_pos.top_left().y,
        );
    }
    fn mouse_down(&mut self, button: MouseButton) -> Vec<GuiAction> {
        if button == MouseButton::Left {
            self.mouse = true;
            let mouse_pos = self.mouse_pos;
            let w = self.config.pixel_pos.width();
            let h = self.config.pixel_pos.height();
            let mut el = GuiElem::new(self.clone());
            vec![GuiAction::SetDragging(Some((
                Dragging::Album(self.id),
                Some(Box::new(move |i, g| {
                    let sw = i.pos.width();
                    let sh = i.pos.height();
                    let x = (i.mouse_pos.x - mouse_pos.x) / sw;
                    let y = (i.mouse_pos.y - mouse_pos.y) / sh;
                    el.inner.config_mut().pos =
                        Rectangle::from_tuples((x, y), (x + w / sw, y + h / sh));
                    el.draw(i, g)
                })),
            )))]
        } else {
            vec![]
        }
    }
    fn mouse_up(&mut self, button: MouseButton) -> Vec<GuiAction> {
        if self.mouse && button == MouseButton::Left {
            self.mouse = false;
            vec![GuiAction::OpenEditPanel(GuiElem::new(GuiEdit::new(
                GuiElemCfg::default(),
                crate::gui_edit::Editable::Album(vec![self.id]),
            )))]
        } else {
            vec![]
        }
    }
}

#[derive(Clone)]
struct ListSong {
    config: GuiElemCfg,
    id: SongId,
    children: Vec<GuiElem>,
    mouse: bool,
    mouse_pos: Vec2,
}
impl ListSong {
    pub fn new(config: GuiElemCfg, id: SongId, name: String) -> Self {
        let label = Label::new(
            GuiElemCfg::default(),
            name,
            Color::from_int_rgb(175, 175, 175),
            None,
            Vec2::new(0.0, 0.5),
        );
        Self {
            config: config.w_mouse(),
            id,
            children: vec![GuiElem::new(label)],
            mouse: false,
            mouse_pos: Vec2::ZERO,
        }
    }
}
impl GuiElemTrait for ListSong {
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
    fn draw(&mut self, info: &mut DrawInfo, _g: &mut speedy2d::Graphics2D) {
        if self.mouse {
            if info.pos.contains(info.mouse_pos) {
                return;
            } else {
                self.mouse = false;
            }
        }
        self.mouse_pos = Vec2::new(
            info.mouse_pos.x - self.config.pixel_pos.top_left().x,
            info.mouse_pos.y - self.config.pixel_pos.top_left().y,
        );
    }
    fn mouse_down(&mut self, button: MouseButton) -> Vec<GuiAction> {
        if button == MouseButton::Left {
            self.mouse = true;
            let mouse_pos = self.mouse_pos;
            let w = self.config.pixel_pos.width();
            let h = self.config.pixel_pos.height();
            let mut el = GuiElem::new(self.clone());
            vec![GuiAction::SetDragging(Some((
                Dragging::Song(self.id),
                Some(Box::new(move |i, g| {
                    let sw = i.pos.width();
                    let sh = i.pos.height();
                    let x = (i.mouse_pos.x - mouse_pos.x) / sw;
                    let y = (i.mouse_pos.y - mouse_pos.y) / sh;
                    el.inner.config_mut().pos =
                        Rectangle::from_tuples((x, y), (x + w / sw, y + h / sh));
                    el.draw(i, g)
                })),
            )))]
        } else {
            vec![]
        }
    }
    fn mouse_up(&mut self, button: MouseButton) -> Vec<GuiAction> {
        if self.mouse && button == MouseButton::Left {
            self.mouse = false;
            vec![GuiAction::OpenEditPanel(GuiElem::new(GuiEdit::new(
                GuiElemCfg::default(),
                crate::gui_edit::Editable::Song(vec![self.id]),
            )))]
        } else {
            vec![]
        }
    }
}

#[derive(Clone)]
struct FilterPanel {
    config: GuiElemCfg,
    children: Vec<GuiElem>,
    line_height: f32,
}
const FP_CASESENS_N: &'static str = "Switch to case-sensitive search";
const FP_CASESENS_Y: &'static str = "Switch to case-insensitive search";
impl FilterPanel {
    pub fn new(search_is_case_sensitive: Rc<AtomicBool>) -> Self {
        let is_case_sensitive = search_is_case_sensitive.load(std::sync::atomic::Ordering::Relaxed);
        Self {
            config: GuiElemCfg::default().disabled(),
            children: vec![GuiElem::new(ScrollBox::new(
                GuiElemCfg::default(),
                crate::gui_base::ScrollBoxSizeUnit::Pixels,
                vec![
                    (
                        GuiElem::new(Button::new(
                            GuiElemCfg::default(),
                            move |button| {
                                let is_case_sensitive = !search_is_case_sensitive
                                    .load(std::sync::atomic::Ordering::Relaxed);
                                search_is_case_sensitive
                                    .store(is_case_sensitive, std::sync::atomic::Ordering::Relaxed);
                                *button
                                    .children()
                                    .next()
                                    .unwrap()
                                    .try_as_mut::<Label>()
                                    .unwrap()
                                    .content
                                    .text() = if is_case_sensitive {
                                    FP_CASESENS_Y.to_owned()
                                } else {
                                    FP_CASESENS_N.to_owned()
                                };
                                vec![]
                            },
                            vec![GuiElem::new(Label::new(
                                GuiElemCfg::default(),
                                if is_case_sensitive {
                                    FP_CASESENS_Y.to_owned()
                                } else {
                                    FP_CASESENS_N.to_owned()
                                },
                                Color::GRAY,
                                None,
                                Vec2::new(0.5, 0.5),
                            ))],
                        )),
                        1.0,
                    ),
                    (
                        GuiElem::new(Button::new(
                            GuiElemCfg::default(),
                            |button| {
                                let text = button
                                    .children()
                                    .next()
                                    .unwrap()
                                    .try_as_mut::<Label>()
                                    .unwrap()
                                    .content
                                    .text();
                                *text = if text.len() > 20 {
                                    "Click for RegEx help".to_owned()
                                } else {
                                    "Click to close RegEx help\ntest\nyay".to_owned()
                                };
                                vec![]
                            },
                            vec![GuiElem::new(Label::new(
                                GuiElemCfg::default(),
                                "Click for RegEx help".to_owned(),
                                Color::GRAY,
                                None,
                                Vec2::new(0.5, 0.0),
                            ))],
                        )),
                        1.0,
                    ),
                ],
            ))],
            line_height: 0.0,
        }
    }
}
impl GuiElemTrait for FilterPanel {
    fn draw(&mut self, info: &mut DrawInfo, _g: &mut speedy2d::Graphics2D) {
        if info.line_height != self.line_height {
            for (_, h) in &mut self.children[0].try_as_mut::<ScrollBox>().unwrap().children {
                *h = info.line_height;
            }
            self.line_height = info.line_height;
        }
    }
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
}
