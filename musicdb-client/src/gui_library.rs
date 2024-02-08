use std::{
    cmp::Ordering,
    collections::HashSet,
    sync::Arc,
    sync::{
        atomic::{AtomicBool, AtomicUsize},
        mpsc, Mutex,
    },
    time::Instant,
};

use musicdb_lib::data::{
    album::Album,
    artist::Artist,
    database::Database,
    queue::{Queue, QueueContent},
    song::Song,
    AlbumId, ArtistId, GeneralData, SongId,
};
use regex::{Regex, RegexBuilder};
use speedy2d::{
    color::Color,
    dimen::Vec2,
    shape::Rectangle,
    window::{MouseButton, VirtualKeyCode},
};

use crate::{
    gui::{
        Dragging, DrawInfo, GuiAction, GuiConfig, GuiElem, GuiElemCfg, GuiElemChildren,
        GuiElemWrapper,
    },
    gui_anim::AnimationController,
    gui_base::{Button, Panel, ScrollBox},
    gui_text::{self, AdvancedLabel, Label, TextField},
};

use self::selected::Selected;

/*

This is responsible for showing the library,
with Regex search and drag-n-drop.

*/

pub struct LibraryBrowser {
    config: GuiElemCfg,
    pub c_search_artist: TextField,
    pub c_search_album: TextField,
    pub c_search_song: TextField,
    pub c_scroll_box: ScrollBox<Vec<ListElement>>,
    pub c_filter_button: Button<[Label; 1]>,
    pub c_filter_panel: FilterPanel,
    pub c_selected_counter_panel: Panel<[Label; 1]>,
    // - - -
    library_sorted: Vec<(ArtistId, Vec<SongId>, Vec<(AlbumId, Vec<SongId>)>)>,
    library_filtered: Vec<(
        ArtistId,
        Vec<(SongId, f32)>,
        Vec<(AlbumId, Vec<(SongId, f32)>, f32)>,
        f32,
    )>,
    selected: Selected,
    // - - -
    search_artist: String,
    search_artist_regex: Option<Regex>,
    search_album: String,
    search_album_regex: Option<Regex>,
    search_song: String,
    search_song_regex: Option<Regex>,
    filter_target_state: Arc<AtomicBool>,
    filter_state: AnimationController<f32>,
    library_updated: bool,
    search_settings_changed: Arc<AtomicBool>,
    search_is_case_sensitive: Arc<AtomicBool>,
    search_was_case_sensitive: bool,
    search_prefer_start_matches: Arc<AtomicBool>,
    search_prefers_start_matches: bool,
    filter_songs: Arc<Mutex<Filter>>,
    filter_albums: Arc<Mutex<Filter>>,
    filter_artists: Arc<Mutex<Filter>>,
    do_something_receiver: mpsc::Receiver<Box<dyn FnOnce(&mut Self)>>,
    selected_popup_state: (f32, usize, usize, usize),
}
fn search_regex_new(pat: &str, case_insensitive: bool) -> Result<Option<Regex>, regex::Error> {
    if pat.is_empty() {
        Ok(None)
    } else {
        Ok(Some(
            RegexBuilder::new(pat)
                .unicode(true)
                .case_insensitive(case_insensitive)
                .build()?,
        ))
    }
}
const LP_LIB1: f32 = 0.1;
const LP_LIB2: f32 = 1.0;
const LP_LIB1S: f32 = 0.4;
impl LibraryBrowser {
    pub fn new(config: GuiElemCfg) -> Self {
        let c_search_artist = TextField::new(
            GuiElemCfg::at(Rectangle::from_tuples((0.01, 0.01), (0.45, 0.05))),
            "artist".to_string(),
            Color::GRAY,
            Color::WHITE,
        );
        let c_search_album = TextField::new(
            GuiElemCfg::at(Rectangle::from_tuples((0.55, 0.01), (0.99, 0.05))),
            "album".to_string(),
            Color::GRAY,
            Color::WHITE,
        );
        let c_search_song = TextField::new(
            GuiElemCfg::at(Rectangle::from_tuples((0.01, 0.06), (0.99, 0.1))),
            "song".to_string(),
            Color::GRAY,
            Color::WHITE,
        );
        let library_scroll_box = ScrollBox::new(
            GuiElemCfg::at(Rectangle::from_tuples((0.0, LP_LIB1), (1.0, LP_LIB2))),
            crate::gui_base::ScrollBoxSizeUnit::Pixels,
            vec![],
            vec![],
            0.0,
        );
        let (do_something_sender, do_something_receiver) = mpsc::channel();
        let search_settings_changed = Arc::new(AtomicBool::new(false));
        let search_was_case_sensitive = false;
        let search_is_case_sensitive = Arc::new(AtomicBool::new(search_was_case_sensitive));
        let search_prefers_start_matches = true;
        let search_prefer_start_matches = Arc::new(AtomicBool::new(search_prefers_start_matches));
        let filter_target_state = Arc::new(AtomicBool::new(false));
        let fts = Arc::clone(&filter_target_state);
        let c_filter_button = Button::new(
            GuiElemCfg::at(Rectangle::from_tuples((0.46, 0.01), (0.54, 0.05))),
            move |_| {
                fts.store(
                    !fts.load(std::sync::atomic::Ordering::Relaxed),
                    std::sync::atomic::Ordering::Relaxed,
                );
                vec![]
            },
            [Label::new(
                GuiElemCfg::default(),
                "more".to_owned(),
                Color::GRAY,
                None,
                Vec2::new(0.5, 0.5),
            )],
        );
        let filter_songs = Arc::new(Mutex::new(Filter {
            and: true,
            filters: vec![],
        }));
        let filter_albums = Arc::new(Mutex::new(Filter {
            and: true,
            filters: vec![],
        }));
        let filter_artists = Arc::new(Mutex::new(Filter {
            and: true,
            filters: vec![],
        }));
        let selected = Selected::new(Arc::clone(&search_settings_changed));
        Self {
            config: config.w_keyboard_watch(),
            c_search_artist,
            c_search_album,
            c_search_song,
            c_scroll_box: library_scroll_box,
            c_filter_button,
            c_filter_panel: FilterPanel::new(
                Arc::clone(&search_settings_changed),
                Arc::clone(&search_is_case_sensitive),
                Arc::clone(&search_prefer_start_matches),
                Arc::clone(&filter_songs),
                Arc::clone(&filter_albums),
                Arc::clone(&filter_artists),
                selected.clone(),
                do_something_sender.clone(),
            ),
            c_selected_counter_panel: Panel::with_background(
                GuiElemCfg::default().disabled(),
                [Label::new(
                    GuiElemCfg::default(),
                    String::new(),
                    Color::LIGHT_GRAY,
                    None,
                    Vec2::new(0.5, 0.5),
                )],
                Color::from_rgba(0.0, 0.0, 0.0, 0.8),
            ),
            // - - -
            library_sorted: vec![],
            library_filtered: vec![],
            selected,
            // - - -
            search_artist: String::new(),
            search_artist_regex: None,
            search_album: String::new(),
            search_album_regex: None,
            search_song: String::new(),
            search_song_regex: None,
            filter_target_state,
            filter_state: AnimationController::new(0.0, 0.0, 0.25, 25.0, 0.1, 0.2, Instant::now()),
            library_updated: true,
            search_settings_changed,
            search_is_case_sensitive,
            search_was_case_sensitive,
            search_prefer_start_matches,
            search_prefers_start_matches,
            filter_songs,
            filter_albums,
            filter_artists,
            do_something_receiver,
            selected_popup_state: (0.0, 0, 0, 0),
        }
    }
    pub fn selected_add_all(&self) {
        self.selected.view_mut(|sel| {
            for (id, singles, albums, _) in &self.library_filtered {
                sel.0.insert(*id);
                for (s, _) in singles {
                    sel.2.insert(*s);
                }
                for (id, album, _) in albums {
                    sel.1.insert(*id);
                    for (s, _) in album {
                        sel.2.insert(*s);
                    }
                }
            }
        })
    }
    pub fn selected_add_songs(&self) {
        self.selected.view_mut(|sel| {
            for (_, singles, albums, _) in &self.library_filtered {
                for (s, _) in singles {
                    sel.2.insert(*s);
                }
                for (_, album, _) in albums {
                    for (s, _) in album {
                        sel.2.insert(*s);
                    }
                }
            }
        })
    }
    pub fn selected_add_albums(&self) {
        self.selected.view_mut(|sel| {
            for (_, _, albums, _) in &self.library_filtered {
                for (id, album, _) in albums {
                    sel.1.insert(*id);
                    for (s, _) in album {
                        sel.2.insert(*s);
                    }
                }
            }
        })
    }
}
impl GuiElem for LibraryBrowser {
    fn config(&self) -> &GuiElemCfg {
        &self.config
    }
    fn config_mut(&mut self) -> &mut GuiElemCfg {
        &mut self.config
    }
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut dyn GuiElem> + '_> {
        Box::new(
            [
                self.c_search_artist.elem_mut(),
                self.c_search_album.elem_mut(),
                self.c_search_song.elem_mut(),
                self.c_scroll_box.elem_mut(),
                self.c_filter_button.elem_mut(),
                self.c_filter_panel.elem_mut(),
                self.c_selected_counter_panel.elem_mut(),
            ]
            .into_iter(),
        )
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
    fn draw_rev(&self) -> bool {
        false
    }
    fn draw(&mut self, info: &mut DrawInfo, _g: &mut speedy2d::Graphics2D) {
        loop {
            if let Ok(action) = self.do_something_receiver.try_recv() {
                action(self);
            } else {
                break;
            }
        }
        // search
        let mut search_changed = false;
        let mut rebuild_regex = false;
        if self
            .search_settings_changed
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            search_changed = true;
            self.search_settings_changed
                .store(false, std::sync::atomic::Ordering::Relaxed);
            let case_sensitive = self
                .search_is_case_sensitive
                .load(std::sync::atomic::Ordering::Relaxed);
            if self.search_was_case_sensitive != case_sensitive {
                self.search_was_case_sensitive = case_sensitive;
                rebuild_regex = true;
            }
            let pref_start = self
                .search_prefer_start_matches
                .load(std::sync::atomic::Ordering::Relaxed);
            if self.search_prefers_start_matches != pref_start {
                self.search_prefers_start_matches = pref_start;
            }
        }
        {
            let v = &mut self.c_search_artist.c_input.content;
            if rebuild_regex || v.will_redraw() && self.search_artist != *v.get_text() {
                search_changed = true;
                self.search_artist = v.get_text().clone();
                self.search_artist_regex =
                    search_regex_new(&self.search_artist, !self.search_was_case_sensitive)
                        .ok()
                        .flatten();
                *v.color() = if self.search_artist_regex.is_some() {
                    Color::WHITE
                } else {
                    Color::RED
                };
            }
        }
        {
            let v = &mut self.c_search_album.c_input.content;
            if rebuild_regex || v.will_redraw() && self.search_album != *v.get_text() {
                search_changed = true;
                self.search_album = v.get_text().clone();
                self.search_album_regex =
                    search_regex_new(&self.search_album, !self.search_was_case_sensitive)
                        .ok()
                        .flatten();
                *v.color() = if self.search_album_regex.is_some() {
                    Color::WHITE
                } else {
                    Color::RED
                };
            }
        }
        {
            let v = &mut self.c_search_song.c_input.content;
            if rebuild_regex || v.will_redraw() && self.search_song != *v.get_text() {
                search_changed = true;
                self.search_song = v.get_text().clone();
                self.search_song_regex =
                    search_regex_new(&self.search_song, !self.search_was_case_sensitive)
                        .ok()
                        .flatten();
                *v.color() = if self.search_song_regex.is_some() {
                    Color::WHITE
                } else {
                    Color::RED
                };
            }
        }
        // filter panel
        let filter_target_state = self
            .filter_target_state
            .load(std::sync::atomic::Ordering::Relaxed);
        self.filter_state.target = if filter_target_state { 1.0 } else { 0.0 };
        if self.filter_state.update(info.time, info.high_performance) {
            if let Some(h) = &info.helper {
                h.request_redraw();
            }
            let y = LP_LIB1 + (LP_LIB1S - LP_LIB1) * self.filter_state.value;
            self.c_scroll_box.config_mut().pos =
                Rectangle::new(Vec2::new(0.0, y), Vec2::new(1.0, LP_LIB2));
            let filter_panel = &mut self.c_filter_panel;
            filter_panel.config_mut().pos =
                Rectangle::new(Vec2::new(0.0, LP_LIB1), Vec2::new(1.0, y));
            filter_panel.config.enabled = self.filter_state.value > 0.0;
        }
        // -
        if self.library_updated {
            self.library_updated = false;
            self.update_local_library(&info.database, |(_, a), (_, b)| a.name.cmp(&b.name));
            search_changed = true;
        }
        if search_changed {
            fn filter(
                s: &LibraryBrowser,
                pat: &str,
                regex: &Option<Regex>,
                search_text: &String,
                filter: &Filter,
                search_gd: &GeneralData,
            ) -> f32 {
                if !filter.passes(search_gd) {
                    return 0.0;
                };
                if let Some(r) = regex {
                    if s.search_prefers_start_matches {
                        r.find_iter(pat)
                            .map(|m| match pat[0..m.start()].chars().rev().next() {
                                // found at the start of h, reaches to the end (whole pattern is part of the match)
                                None if m.end() == pat.len() => 6.0,
                                // found at start of h
                                None => 4.0,
                                Some(ch) if ch.is_whitespace() => {
                                    match pat[m.end()..].chars().next() {
                                        // whole word matches
                                        None => 5.0,
                                        Some(ch) if ch.is_whitespace() => 5.0,
                                        // found after whitespace in h
                                        Some(_) => 3.0,
                                    }
                                }
                                // found somewhere else in h
                                _ => 2.0,
                            })
                            .fold(0.0, f32::max)
                    } else {
                        if r.is_match(pat) {
                            2.0
                        } else {
                            0.0
                        }
                    }
                } else if search_text.is_empty() {
                    1.0
                } else {
                    0.0
                }
            }
            let allow_singles = self.search_album.is_empty()
                && self.filter_albums.lock().unwrap().filters.is_empty();
            self.filter_local_library(
                &info.database,
                |s, artist| {
                    filter(
                        s,
                        &artist.name,
                        &s.search_artist_regex,
                        &s.search_artist,
                        &s.filter_artists.lock().unwrap(),
                        &artist.general,
                    )
                },
                |s, album| {
                    filter(
                        s,
                        &album.name,
                        &s.search_album_regex,
                        &s.search_album,
                        &s.filter_albums.lock().unwrap(),
                        &album.general,
                    )
                },
                |s, song| {
                    if song.album.is_some() || allow_singles {
                        filter(
                            s,
                            &song.title,
                            &s.search_song_regex,
                            &s.search_song,
                            &s.filter_songs.lock().unwrap(),
                            &song.general,
                        )
                    } else {
                        0.0
                    }
                },
            );
            // selected
            {
                let (artists, albums, songs) = self
                    .selected
                    .view(|sel| (sel.0.len(), sel.1.len(), sel.2.len()));
                if self.selected_popup_state.1 != artists
                    || self.selected_popup_state.2 != albums
                    || self.selected_popup_state.3 != songs
                {
                    self.selected_popup_state.1 = artists;
                    self.selected_popup_state.2 = albums;
                    self.selected_popup_state.3 = songs;
                    if artists > 0 || albums > 0 || songs > 0 {
                        if let Some(text) = match (artists, albums, songs) {
                            (0, 0, 0) => None,

                            (0, 0, 1) => Some(format!("1 song selected")),
                            (0, 0, s) => Some(format!("{s} songs selected")),

                            (0, 1, 0) => Some(format!("1 album selected")),
                            (0, al, 0) => Some(format!("{al} albums selected")),

                            (1, 0, 0) => Some(format!("1 artist selected")),
                            (ar, 0, 0) => Some(format!("{ar} artists selected")),

                            (0, 1, 1) => Some(format!("1 song and 1 album selected")),
                            (0, 1, s) => Some(format!("{s} songs and 1 album selected")),
                            (0, al, 1) => Some(format!("1 song and {al} albums selected")),
                            (0, al, s) => Some(format!("{s} songs and {al} albums selected")),

                            (1, 0, 1) => Some(format!("1 song and 1 artist selected")),
                            (1, 0, s) => Some(format!("{s} songs and 1 artist selected")),
                            (ar, 0, 1) => Some(format!("1 song and {ar} artists selected")),
                            (ar, 0, s) => Some(format!("{s} songs and {ar} artists selected")),

                            (1, 1, 0) => Some(format!("1 album and 1 artist selected")),
                            (1, al, 0) => Some(format!("{al} albums and 1 artist selected")),
                            (ar, 1, 0) => Some(format!("1 album and {ar} artists selected")),
                            (ar, al, 0) => Some(format!("{al} albums and {ar} artists selected")),

                            (1, 1, 1) => Some(format!("1 song, 1 album and 1 artist selected")),
                            (1, 1, s) => Some(format!("{s} songs, 1 album and 1 artist selected")),
                            (1, al, 1) => {
                                Some(format!("1 song, {al} albums and 1 artist selected"))
                            }
                            (ar, 1, 1) => {
                                Some(format!("1 song, 1 album and {ar} artists selected"))
                            }
                            (1, al, s) => {
                                Some(format!("{s} songs, {al} albums and 1 artist selected"))
                            }
                            (ar, 1, s) => {
                                Some(format!("{s} songs, 1 album and {ar} artists selected"))
                            }
                            (ar, al, 1) => {
                                Some(format!("1 song, {al} albums and {ar} artist selected"))
                            }
                            (ar, al, s) => {
                                Some(format!("{s} songs, {al} albums and {ar} artists selected"))
                            }
                        } {
                            *self.c_selected_counter_panel.children[0].content.text() = text;
                        }
                    } else {
                    }
                }
            }
            self.config.redraw = true;
        }
        // selected popup
        {
            let mut redraw = false;
            if self.selected_popup_state.1 > 0
                || self.selected_popup_state.2 > 0
                || self.selected_popup_state.3 > 0
            {
                if self.selected_popup_state.0 != 1.0 {
                    redraw = true;
                    self.c_selected_counter_panel.config_mut().enabled = true;
                    self.selected_popup_state.0 = 0.3 + 0.7 * self.selected_popup_state.0;
                    if self.selected_popup_state.0 > 0.99 {
                        self.selected_popup_state.0 = 1.0;
                    }
                }
            } else {
                if self.selected_popup_state.0 != 0.0 {
                    redraw = true;
                    self.selected_popup_state.0 = 0.7 * self.selected_popup_state.0;
                    if self.selected_popup_state.0 < 0.01 {
                        self.selected_popup_state.0 = 0.0;
                        self.c_selected_counter_panel.config_mut().enabled = false;
                    }
                }
            }
            if redraw {
                self.c_selected_counter_panel.config_mut().pos = Rectangle::from_tuples(
                    (0.0, 1.0 - 0.05 * self.selected_popup_state.0),
                    (1.0, 1.0),
                );
                if let Some(h) = &info.helper {
                    h.request_redraw();
                }
            }
        }
        if self.config.redraw || info.pos.size() != self.config.pixel_pos.size() {
            self.config.redraw = false;
            self.update_ui(&info.database, info.line_height);
        }
    }
    fn updated_library(&mut self) {
        self.library_updated = true;
    }
    fn key_watch(
        &mut self,
        modifiers: speedy2d::window::ModifiersState,
        down: bool,
        key: Option<VirtualKeyCode>,
        _scan: speedy2d::window::KeyScancode,
    ) -> Vec<GuiAction> {
        if down && crate::gui::hotkey_deselect_all(&modifiers, key) {
            self.selected.clear();
        }
        if down && crate::gui::hotkey_select_all(&modifiers, key) {
            self.selected_add_all();
        }
        if down && crate::gui::hotkey_select_albums(&modifiers, key) {
            self.selected_add_albums();
        }
        if down && crate::gui::hotkey_select_songs(&modifiers, key) {
            self.selected_add_songs();
        }
        vec![]
    }
}
impl LibraryBrowser {
    /// Sets `self.library_sorted` based on the contents of the `Database`.
    fn update_local_library(
        &mut self,
        db: &Database,
        sort_artists: impl FnMut(&(&ArtistId, &Artist), &(&ArtistId, &Artist)) -> Ordering,
    ) {
        let mut artists = db.artists().iter().collect::<Vec<_>>();
        artists.sort_unstable_by(sort_artists);
        self.library_sorted = artists
            .into_iter()
            .map(|(ar_id, artist)| {
                let singles = artist.singles.iter().map(|id| *id).collect();
                let albums = artist
                    .albums
                    .iter()
                    .map(|id| {
                        let songs = if let Some(album) = db.albums().get(id) {
                            album.songs.iter().map(|id| *id).collect()
                        } else {
                            eprintln!("[warn] No album with id {id} found in db!");
                            vec![]
                        };
                        (*id, songs)
                    })
                    .collect();
                (*ar_id, singles, albums)
            })
            .collect();
    }
    /// Sets `self.library_filtered` using the value of `self.library_sorted` and filter functions.
    /// Return values of the filter functions:
    /// 0.0 -> don't show
    /// 1.0 -> neutral
    /// anything else -> priority (determines how things will be sorted)
    /// Album Value = max(Song Values) * AlbumFilterVal
    /// Artist Value = max(Album Values) * ArtistFilterVal
    fn filter_local_library(
        &mut self,
        db: &Database,
        filter_artist: impl Fn(&Self, &Artist) -> f32,
        filter_album: impl Fn(&Self, &Album) -> f32,
        filter_song: impl Fn(&Self, &Song) -> f32,
    ) {
        let mut a = vec![];
        for (artist_id, singles, albums) in self.library_sorted.iter() {
            if let Some(artist) = db.artists().get(artist_id) {
                let mut filterscore_artist = filter_artist(self, artist);
                if filterscore_artist > 0.0 {
                    let mut max_score_in_artist = 0.0;
                    if filterscore_artist > 0.0 {
                        let mut s = singles
                            .iter()
                            .filter_map(|song_id| {
                                if let Some(song) = db.songs().get(song_id) {
                                    let filterscore_song = filter_song(self, song);
                                    if filterscore_song > 0.0 {
                                        if filterscore_song > max_score_in_artist {
                                            max_score_in_artist = filterscore_song;
                                        }
                                        return Some((*song_id, filterscore_song));
                                    }
                                }
                                None
                            })
                            .collect::<Vec<_>>();
                        s.sort_by(|(.., a), (.., b)| b.partial_cmp(a).unwrap_or(Ordering::Equal));
                        let mut al = albums
                            .iter()
                            .filter_map(|(album_id, songs)| {
                                if let Some(album) = db.albums().get(album_id) {
                                    let mut filterscore_album = filter_album(self, album);
                                    if filterscore_album > 0.0 {
                                        let mut max_score_in_album = 0.0;
                                        let mut s = songs
                                            .iter()
                                            .filter_map(|song_id| {
                                                if let Some(song) = db.songs().get(song_id) {
                                                    let filterscore_song = filter_song(self, song);
                                                    if filterscore_song > 0.0 {
                                                        if filterscore_song > max_score_in_album {
                                                            max_score_in_album = filterscore_song;
                                                        }
                                                        return Some((*song_id, filterscore_song));
                                                    }
                                                }
                                                None
                                            })
                                            .collect::<Vec<_>>();
                                        s.sort_by(|(.., a), (.., b)| {
                                            b.partial_cmp(a).unwrap_or(Ordering::Equal)
                                        });
                                        filterscore_album *= max_score_in_album;
                                        if filterscore_album > 0.0 {
                                            if filterscore_album > max_score_in_artist {
                                                max_score_in_artist = filterscore_album;
                                            }
                                            return Some((*album_id, s, filterscore_album));
                                        }
                                    }
                                }
                                None
                            })
                            .collect::<Vec<_>>();
                        al.sort_by(|(.., a), (.., b)| b.partial_cmp(a).unwrap_or(Ordering::Equal));
                        filterscore_artist *= max_score_in_artist;
                        if filterscore_artist > 0.0 {
                            a.push((*artist_id, s, al, filterscore_artist));
                        }
                    }
                }
            }
        }
        a.sort_by(|(.., a), (.., b)| b.partial_cmp(a).unwrap_or(Ordering::Equal));
        self.library_filtered = a;
    }
    /// Sets the contents of the `ScrollBox` based on `self.library_filtered`.
    fn update_ui(&mut self, db: &Database, line_height: f32) {
        let mut elems = vec![];
        let mut elemh = vec![];
        for (artist_id, singles, albums, _artist_filterscore) in self.library_filtered.iter() {
            let (e, h) = self.build_ui_element_artist(*artist_id, db, line_height);
            elems.push(e);
            elemh.push(h);
            for (song_id, _song_filterscore) in singles {
                let (e, h) = self.build_ui_element_song(*song_id, db, line_height);
                elems.push(e);
                elemh.push(h);
            }
            for (album_id, songs, _album_filterscore) in albums {
                let (e, h) = self.build_ui_element_album(*album_id, db, line_height);
                elems.push(e);
                elemh.push(h);
                for (song_id, _song_filterscore) in songs {
                    let (e, h) = self.build_ui_element_song(*song_id, db, line_height);
                    elems.push(e);
                    elemh.push(h);
                }
            }
        }
        let library_scroll_box = &mut self.c_scroll_box;
        library_scroll_box.children = elems;
        library_scroll_box.children_heights = elemh;
        library_scroll_box.config_mut().redraw = true;
    }
    fn build_ui_element_artist(&self, id: ArtistId, db: &Database, h: f32) -> (ListElement, f32) {
        (
            ListElement::Artist(ListArtist::new(
                GuiElemCfg::default(),
                id,
                if let Some(v) = db.artists().get(&id) {
                    v.name.to_owned()
                } else {
                    format!("[ Artist #{id} ]")
                },
                self.selected.clone(),
            )),
            h * 2.5,
        )
    }
    fn build_ui_element_album(&self, id: ArtistId, db: &Database, h: f32) -> (ListElement, f32) {
        let (name, duration) = if let Some(v) = db.albums().get(&id) {
            let duration = v
                .songs
                .iter()
                .filter_map(|id| db.get_song(id))
                .map(|s| s.duration_millis)
                .fold(0, u64::saturating_add)
                / 1000;
            (
                v.name.to_owned(),
                if duration >= 60 * 60 {
                    format!(
                        "  {}:{:0>2}:{:0>2}",
                        duration / (60 * 60),
                        (duration / 60) % 60,
                        duration % 60
                    )
                } else {
                    format!("  {}:{:0>2}", duration / 60, duration % 60)
                },
            )
        } else {
            (format!("[ Album #{id} ]"), String::new())
        };
        (
            ListElement::Album(ListAlbum::new(
                GuiElemCfg::default(),
                id,
                name,
                duration,
                self.selected.clone(),
            )),
            h * 1.5,
        )
    }
    fn build_ui_element_song(&self, id: ArtistId, db: &Database, h: f32) -> (ListElement, f32) {
        let (name, duration) = if let Some(v) = db.songs().get(&id) {
            let duration = v.duration_millis / 1000;
            (
                v.title.to_owned(),
                format!("  {}:{:0>2}", duration / 60, duration % 60),
            )
        } else {
            (format!("[ Song #{id} ]"), String::new())
        };
        (
            ListElement::Song(ListSong::new(
                GuiElemCfg::default(),
                id,
                name,
                duration,
                self.selected.clone(),
            )),
            h,
        )
    }
}

enum ListElement {
    Artist(ListArtist),
    Album(ListAlbum),
    Song(ListSong),
}
impl GuiElemWrapper for ListElement {
    fn as_elem(&self) -> &dyn GuiElem {
        match self {
            Self::Artist(v) => v,
            Self::Album(v) => v,
            Self::Song(v) => v,
        }
    }
    fn as_elem_mut(&mut self) -> &mut dyn GuiElem {
        match self {
            Self::Artist(v) => v,
            Self::Album(v) => v,
            Self::Song(v) => v,
        }
    }
}

struct ListArtist {
    config: GuiElemCfg,
    id: ArtistId,
    children: Vec<Box<dyn GuiElem>>,
    mouse: bool,
    mouse_pos: Vec2,
    selected: Selected,
    sel: bool,
}
impl ListArtist {
    pub fn new(mut config: GuiElemCfg, id: ArtistId, name: String, selected: Selected) -> Self {
        let label = Label::new(
            GuiElemCfg::default(),
            name,
            Color::from_int_rgb(81, 24, 125),
            None,
            Vec2::new(0.0, 0.5),
        );
        config.redraw = true;
        Self {
            config: config.w_mouse(),
            id,
            children: vec![Box::new(label)],
            mouse: false,
            mouse_pos: Vec2::ZERO,
            selected,
            sel: false,
        }
    }
}
impl GuiElem for ListArtist {
    fn config(&self) -> &GuiElemCfg {
        &self.config
    }
    fn config_mut(&mut self) -> &mut GuiElemCfg {
        &mut self.config
    }
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut dyn GuiElem> + '_> {
        Box::new(self.children.iter_mut().map(|v| v.elem_mut()))
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
    fn draw(&mut self, info: &mut DrawInfo, _g: &mut speedy2d::Graphics2D) {
        if self.config.redraw {
            self.config.redraw = false;
            let sel = self.selected.contains_artist(&self.id);
            if sel != self.sel {
                self.sel = sel;
                if sel {
                    self.children.push(Box::new(Panel::with_background(
                        GuiElemCfg::default(),
                        (),
                        Color::from_rgba(1.0, 1.0, 1.0, 0.2),
                    )));
                } else {
                    self.children.pop();
                }
            }
        }
        if self.mouse {
            if info.pos.contains(info.mouse_pos) {
                return;
            } else {
                self.mouse = false;
                if self.sel {
                    let selected = self.selected.clone();
                    info.actions.push(GuiAction::Do(Box::new(move |gui| {
                        let q = selected.as_queue(
                            &gui.gui.c_main_view.children.library_browser,
                            &gui.database.lock().unwrap(),
                        );
                        gui.exec_gui_action(GuiAction::SetDragging(Some((
                            Dragging::Queues(q),
                            None,
                        ))));
                    })));
                }
            }
        }
        self.mouse_pos = Vec2::new(
            info.mouse_pos.x - info.pos.top_left().x,
            info.mouse_pos.y - info.pos.top_left().y,
        );
    }
    fn mouse_down(&mut self, button: MouseButton) -> Vec<GuiAction> {
        if button == MouseButton::Left {
            self.mouse = true;
            if self.sel {
                vec![]
            } else {
                vec![GuiAction::SetDragging(Some((
                    Dragging::Artist(self.id),
                    None,
                )))]
            }
        } else {
            vec![]
        }
    }
    fn mouse_up(&mut self, button: MouseButton) -> Vec<GuiAction> {
        if self.mouse && button == MouseButton::Left {
            self.mouse = false;
            self.config.redraw = true;
            if !self.sel {
                self.selected.insert_artist(self.id);
            } else {
                self.selected.remove_artist(&self.id);
            }
        }
        vec![]
    }
}

struct ListAlbum {
    config: GuiElemCfg,
    id: AlbumId,
    children: Vec<Box<dyn GuiElem>>,
    mouse: bool,
    mouse_pos: Vec2,
    selected: Selected,
    sel: bool,
}
impl ListAlbum {
    pub fn new(
        mut config: GuiElemCfg,
        id: AlbumId,
        name: String,
        half_sized_info: String,
        selected: Selected,
    ) -> Self {
        let label = AdvancedLabel::new(
            GuiElemCfg::default(),
            Vec2::new(0.0, 0.5),
            vec![vec![
                (
                    gui_text::AdvancedContent::Text(gui_text::Content::new(
                        name,
                        Color::from_int_rgb(8, 61, 47),
                    )),
                    1.0,
                    1.0,
                ),
                (
                    gui_text::AdvancedContent::Text(gui_text::Content::new(
                        half_sized_info,
                        Color::GRAY,
                    )),
                    0.5,
                    1.0,
                ),
            ]],
        );
        config.redraw = true;
        Self {
            config: config.w_mouse(),
            id,
            children: vec![Box::new(label)],
            mouse: false,
            mouse_pos: Vec2::ZERO,
            selected,
            sel: false,
        }
    }
}
impl GuiElem for ListAlbum {
    fn config(&self) -> &GuiElemCfg {
        &self.config
    }
    fn config_mut(&mut self) -> &mut GuiElemCfg {
        &mut self.config
    }
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut dyn GuiElem> + '_> {
        Box::new(self.children.iter_mut().map(|v| v.elem_mut()))
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
    fn draw(&mut self, info: &mut DrawInfo, _g: &mut speedy2d::Graphics2D) {
        if self.config.redraw {
            self.config.redraw = false;
            let sel = self.selected.contains_album(&self.id);
            if sel != self.sel {
                self.sel = sel;
                if sel {
                    self.children.push(Box::new(Panel::with_background(
                        GuiElemCfg::default(),
                        (),
                        Color::from_rgba(1.0, 1.0, 1.0, 0.2),
                    )));
                } else {
                    self.children.pop();
                }
            }
        }
        if self.mouse {
            if info.pos.contains(info.mouse_pos) {
                return;
            } else {
                self.mouse = false;
                if self.sel {
                    let selected = self.selected.clone();
                    info.actions.push(GuiAction::Do(Box::new(move |gui| {
                        let q = selected.as_queue(
                            &gui.gui.c_main_view.children.library_browser,
                            &gui.database.lock().unwrap(),
                        );
                        gui.exec_gui_action(GuiAction::SetDragging(Some((
                            Dragging::Queues(q),
                            None,
                        ))));
                    })));
                }
            }
        }
        self.mouse_pos = Vec2::new(
            info.mouse_pos.x - info.pos.top_left().x,
            info.mouse_pos.y - info.pos.top_left().y,
        );
    }
    fn mouse_down(&mut self, button: MouseButton) -> Vec<GuiAction> {
        if button == MouseButton::Left {
            self.mouse = true;
            if self.sel {
                vec![]
            } else {
                vec![GuiAction::SetDragging(Some((
                    Dragging::Album(self.id),
                    None,
                )))]
            }
        } else {
            vec![]
        }
    }
    fn mouse_up(&mut self, button: MouseButton) -> Vec<GuiAction> {
        if self.mouse && button == MouseButton::Left {
            self.mouse = false;
            self.config.redraw = true;
            if !self.sel {
                self.selected.insert_album(self.id);
            } else {
                self.selected.remove_album(&self.id);
            }
        }
        vec![]
    }
}

struct ListSong {
    config: GuiElemCfg,
    id: SongId,
    children: Vec<Box<dyn GuiElem>>,
    mouse: bool,
    mouse_pos: Vec2,
    selected: Selected,
    sel: bool,
}
impl ListSong {
    pub fn new(
        mut config: GuiElemCfg,
        id: SongId,
        name: String,
        duration: String,
        selected: Selected,
    ) -> Self {
        let label = AdvancedLabel::new(
            GuiElemCfg::default(),
            Vec2::new(0.0, 0.5),
            vec![vec![
                (
                    gui_text::AdvancedContent::Text(gui_text::Content::new(
                        name,
                        Color::from_int_rgb(175, 175, 175),
                    )),
                    1.0,
                    1.0,
                ),
                (
                    gui_text::AdvancedContent::Text(gui_text::Content::new(duration, Color::GRAY)),
                    0.6,
                    1.0,
                ),
            ]],
        );
        config.redraw = true;
        Self {
            config: config.w_mouse(),
            id,
            children: vec![Box::new(label)],
            mouse: false,
            mouse_pos: Vec2::ZERO,
            selected,
            sel: false,
        }
    }
}
impl GuiElem for ListSong {
    fn config(&self) -> &GuiElemCfg {
        &self.config
    }
    fn config_mut(&mut self) -> &mut GuiElemCfg {
        &mut self.config
    }
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut dyn GuiElem> + '_> {
        Box::new(self.children.iter_mut().map(|v| v.elem_mut()))
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
    fn draw(&mut self, info: &mut DrawInfo, _g: &mut speedy2d::Graphics2D) {
        if self.config.redraw {
            self.config.redraw = false;
            let sel = self.selected.contains_song(&self.id);
            if sel != self.sel {
                self.sel = sel;
                if sel {
                    self.children.push(Box::new(Panel::with_background(
                        GuiElemCfg::default(),
                        (),
                        Color::from_rgba(1.0, 1.0, 1.0, 0.2),
                    )));
                } else {
                    self.children.pop();
                }
            }
        }
        if self.mouse {
            if info.pos.contains(info.mouse_pos) {
                return;
            } else {
                self.mouse = false;
                if self.sel {
                    let selected = self.selected.clone();
                    info.actions.push(GuiAction::Do(Box::new(move |gui| {
                        let q = selected.as_queue(
                            &gui.gui.c_main_view.children.library_browser,
                            &gui.database.lock().unwrap(),
                        );
                        gui.exec_gui_action(GuiAction::SetDragging(Some((
                            Dragging::Queues(q),
                            None,
                        ))));
                    })));
                }
            }
        }
        self.mouse_pos = Vec2::new(
            info.mouse_pos.x - info.pos.top_left().x,
            info.mouse_pos.y - info.pos.top_left().y,
        );
    }
    fn mouse_down(&mut self, button: MouseButton) -> Vec<GuiAction> {
        if button == MouseButton::Left {
            self.mouse = true;
            if self.sel {
                vec![]
            } else {
                vec![GuiAction::SetDragging(Some((
                    Dragging::Song(self.id),
                    None,
                )))]
            }
        } else {
            vec![]
        }
    }
    fn mouse_up(&mut self, button: MouseButton) -> Vec<GuiAction> {
        if self.mouse && button == MouseButton::Left {
            self.mouse = false;
            self.config.redraw = true;
            if !self.sel {
                self.selected.insert_song(self.id);
            } else {
                self.selected.remove_song(&self.id);
            }
        }
        vec![]
    }
    fn mouse_pressed(&mut self, button: MouseButton) -> Vec<GuiAction> {
        if button == MouseButton::Right {
            let id = self.id;
            vec![GuiAction::Build(Box::new(move |db| {
                if let Some(me) = db.songs().get(&id) {
                    let me = me.clone();
                    vec![GuiAction::ContextMenu(Some(vec![Box::new(Button::new(
                        GuiElemCfg::default(),
                        move |_| vec![GuiAction::EditSongs(vec![me.clone()])],
                        [Label::new(
                            GuiElemCfg::default(),
                            format!("Edit"),
                            Color::WHITE,
                            None,
                            Vec2::new_y(0.5),
                        )],
                    ))]))]
                } else {
                    vec![]
                }
            }))]
        } else {
            vec![]
        }
    }
}

struct FilterPanel {
    config: GuiElemCfg,
    c_tab_main: ScrollBox<(
        Button<[Label; 1]>,
        Button<[Label; 1]>,
        Button<[Label; 1]>,
        Panel<[Button<[Label; 1]>; 3]>,
    )>,
    c_tab_filters_songs: ScrollBox<FilterTab>,
    c_tab_filters_albums: ScrollBox<FilterTab>,
    c_tab_filters_artists: ScrollBox<FilterTab>,
    c_tab_change: Panel<[Button<[Label; 1]>; 3]>,
    search_settings_changed: Arc<AtomicBool>,
    tab: usize,
    new_tab: Arc<AtomicUsize>,
    line_height: f32,
    filter_songs: Arc<Mutex<Filter>>,
    filter_albums: Arc<Mutex<Filter>>,
    filter_artists: Arc<Mutex<Filter>>,
}
#[derive(Default)]
struct FilterTab {
    buttons: Vec<Button<[Label; 1]>>,
    filters: Vec<FilterLine>,
}
enum FilterLine {
    Joiner(Button<[Label; 1]>),
    Not(Label),
    TagEq(Panel<(Label, TextField)>),
    TagStartsWith(Panel<(Label, TextField)>),
    TagWithValueInt(Panel<(Label, TextField, TextField, TextField)>),
}
impl GuiElemWrapper for FilterLine {
    fn as_elem(&self) -> &dyn GuiElem {
        match self {
            Self::Joiner(v) => v,
            Self::Not(v) => v,
            Self::TagEq(v) => v,
            Self::TagStartsWith(v) => v,
            Self::TagWithValueInt(v) => v,
        }
    }
    fn as_elem_mut(&mut self) -> &mut dyn GuiElem {
        match self {
            Self::Joiner(v) => v,
            Self::Not(v) => v,
            Self::TagEq(v) => v,
            Self::TagStartsWith(v) => v,
            Self::TagWithValueInt(v) => v,
        }
    }
}
impl GuiElemChildren for FilterTab {
    fn iter(&mut self) -> Box<dyn Iterator<Item = &mut dyn GuiElem> + '_> {
        Box::new(
            self.buttons
                .iter_mut()
                .map(|v| v.elem_mut())
                .chain(self.filters.iter_mut().map(|v| v.elem_mut())),
        )
    }
    fn len(&self) -> usize {
        self.buttons.len() + self.filters.len()
    }
}
const FP_CASESENS_N: &'static str = "search is case-insensitive";
const FP_CASESENS_Y: &'static str = "search is case-sensitive!";
const FP_PREFSTART_N: &'static str = "simple search";
const FP_PREFSTART_Y: &'static str = "will prefer matches at the start of a word";
impl FilterPanel {
    pub fn new(
        search_settings_changed: Arc<AtomicBool>,
        search_is_case_sensitive: Arc<AtomicBool>,
        search_prefer_start_matches: Arc<AtomicBool>,
        filter_songs: Arc<Mutex<Filter>>,
        filter_albums: Arc<Mutex<Filter>>,
        filter_artists: Arc<Mutex<Filter>>,
        selected: Selected,
        do_something_sender: mpsc::Sender<Box<dyn FnOnce(&mut LibraryBrowser)>>,
    ) -> Self {
        let is_case_sensitive = search_is_case_sensitive.load(std::sync::atomic::Ordering::Relaxed);
        let prefer_start_matches =
            search_prefer_start_matches.load(std::sync::atomic::Ordering::Relaxed);
        let ssc1 = Arc::clone(&search_settings_changed);
        let ssc2 = Arc::clone(&search_settings_changed);
        let sel3 = selected.clone();
        const VSPLIT: f32 = 0.4;
        let c_tab_main = ScrollBox::new(
            GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.0), (VSPLIT, 1.0))),
            crate::gui_base::ScrollBoxSizeUnit::Pixels,
            (
                Button::new(
                    GuiElemCfg::default(),
                    move |button| {
                        let v =
                            !search_is_case_sensitive.load(std::sync::atomic::Ordering::Relaxed);
                        search_is_case_sensitive.store(v, std::sync::atomic::Ordering::Relaxed);
                        ssc1.store(true, std::sync::atomic::Ordering::Relaxed);
                        *button
                            .children()
                            .next()
                            .unwrap()
                            .any_mut()
                            .downcast_mut::<Label>()
                            .unwrap()
                            .content
                            .text() = if v {
                            FP_CASESENS_Y.to_owned()
                        } else {
                            FP_CASESENS_N.to_owned()
                        };
                        vec![]
                    },
                    [Label::new(
                        GuiElemCfg::default(),
                        if is_case_sensitive {
                            FP_CASESENS_Y.to_owned()
                        } else {
                            FP_CASESENS_N.to_owned()
                        },
                        Color::GRAY,
                        None,
                        Vec2::new(0.5, 0.5),
                    )],
                ),
                Button::new(
                    GuiElemCfg::default(),
                    move |button| {
                        let v =
                            !search_prefer_start_matches.load(std::sync::atomic::Ordering::Relaxed);
                        search_prefer_start_matches.store(v, std::sync::atomic::Ordering::Relaxed);
                        ssc2.store(true, std::sync::atomic::Ordering::Relaxed);
                        *button
                            .children()
                            .next()
                            .unwrap()
                            .any_mut()
                            .downcast_mut::<Label>()
                            .unwrap()
                            .content
                            .text() = if v {
                            FP_PREFSTART_Y.to_owned()
                        } else {
                            FP_PREFSTART_N.to_owned()
                        };
                        vec![]
                    },
                    [Label::new(
                        GuiElemCfg::default(),
                        if prefer_start_matches {
                            FP_PREFSTART_Y.to_owned()
                        } else {
                            FP_PREFSTART_N.to_owned()
                        },
                        Color::GRAY,
                        None,
                        Vec2::new(0.5, 0.5),
                    )],
                ),
                Button::new(
                    GuiElemCfg::default(),
                    move |_| {
                        sel3.clear();
                        vec![]
                    },
                    [Label::new(
                        GuiElemCfg::default(),
                        "deselect all".to_owned(),
                        Color::GRAY,
                        None,
                        Vec2::new(0.5, 0.5),
                    )],
                ),
                Panel::new(
                    GuiElemCfg::default(),
                    [
                        Button::new(
                            GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.0), (0.5, 1.0))),
                            {
                                let dss = do_something_sender.clone();
                                move |_| {
                                    dss.send(Box::new(|s| s.selected_add_all())).unwrap();
                                    vec![]
                                }
                            },
                            [Label::new(
                                GuiElemCfg::default(),
                                "select all".to_owned(),
                                Color::GRAY,
                                None,
                                Vec2::new(0.5, 0.5),
                            )],
                        ),
                        Button::new(
                            GuiElemCfg::at(Rectangle::from_tuples((0.55, 0.0), (0.75, 1.0))),
                            {
                                let dss = do_something_sender.clone();
                                move |_| {
                                    dss.send(Box::new(|s| s.selected_add_songs())).unwrap();
                                    vec![]
                                }
                            },
                            [Label::new(
                                GuiElemCfg::default(),
                                "songs".to_owned(),
                                Color::GRAY,
                                None,
                                Vec2::new(0.5, 0.5),
                            )],
                        ),
                        Button::new(
                            GuiElemCfg::at(Rectangle::from_tuples((0.8, 0.0), (1.0, 1.0))),
                            {
                                let dss = do_something_sender.clone();
                                move |_| {
                                    dss.send(Box::new(|s| s.selected_add_albums())).unwrap();
                                    vec![]
                                }
                            },
                            [Label::new(
                                GuiElemCfg::default(),
                                "albums".to_owned(),
                                Color::GRAY,
                                None,
                                Vec2::new(0.5, 0.5),
                            )],
                        ),
                    ],
                ),
            ),
            vec![0.0; 10],
            0.0,
        );
        let c_tab_filters_songs = ScrollBox::new(
            GuiElemCfg::at(Rectangle::from_tuples((VSPLIT, HEIGHT), (1.0, 1.0))),
            crate::gui_base::ScrollBoxSizeUnit::Pixels,
            FilterTab::default(),
            vec![],
            0.0,
        );
        let c_tab_filters_albums = ScrollBox::new(
            GuiElemCfg::at(Rectangle::from_tuples((VSPLIT, HEIGHT), (1.0, 1.0))).disabled(),
            crate::gui_base::ScrollBoxSizeUnit::Pixels,
            FilterTab::default(),
            vec![],
            0.0,
        );
        let c_tab_filters_artists = ScrollBox::new(
            GuiElemCfg::at(Rectangle::from_tuples((VSPLIT, HEIGHT), (1.0, 1.0))).disabled(),
            crate::gui_base::ScrollBoxSizeUnit::Pixels,
            FilterTab::default(),
            vec![],
            0.0,
        );
        let new_tab = Arc::new(AtomicUsize::new(0));
        let set_tab_1 = Arc::clone(&new_tab);
        let set_tab_2 = Arc::clone(&new_tab);
        let set_tab_3 = Arc::clone(&new_tab);
        let c_tab_change = Panel::new(
            GuiElemCfg::at(Rectangle::from_tuples((VSPLIT, 0.0), (1.0, HEIGHT))),
            [
                Button::new(
                    GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.0), (0.33, 1.0))),
                    move |_| {
                        set_tab_1.store(0, std::sync::atomic::Ordering::Relaxed);
                        vec![]
                    },
                    [Label::new(
                        GuiElemCfg::default(),
                        "Filter Songs".to_owned(),
                        Color::GRAY,
                        None,
                        Vec2::new(0.5, 0.5),
                    )],
                ),
                Button::new(
                    GuiElemCfg::at(Rectangle::from_tuples((0.33, 0.0), (0.67, 1.0))),
                    move |_| {
                        set_tab_2.store(1, std::sync::atomic::Ordering::Relaxed);
                        vec![]
                    },
                    [Label::new(
                        GuiElemCfg::default(),
                        "Filter Albums".to_owned(),
                        Color::GRAY,
                        None,
                        Vec2::new(0.5, 0.5),
                    )],
                ),
                Button::new(
                    GuiElemCfg::at(Rectangle::from_tuples((0.67, 0.0), (1.0, 1.0))),
                    move |_| {
                        set_tab_3.store(2, std::sync::atomic::Ordering::Relaxed);
                        vec![]
                    },
                    [Label::new(
                        GuiElemCfg::default(),
                        "Filter Artists".to_owned(),
                        Color::GRAY,
                        None,
                        Vec2::new(0.5, 0.5),
                    )],
                ),
            ],
        );
        const HEIGHT: f32 = 0.1;
        Self {
            config: GuiElemCfg::default().disabled(),
            c_tab_main,
            c_tab_filters_songs,
            c_tab_filters_albums,
            c_tab_filters_artists,
            c_tab_change,
            line_height: 0.0,
            search_settings_changed,
            tab: 0,
            new_tab,
            filter_songs,
            filter_albums,
            filter_artists,
        }
    }
    fn build_filter(
        tab: usize,
        filter: &Arc<Mutex<Filter>>,
        line_height: f32,
        on_change: &Arc<impl Fn(bool) + 'static>,
        path: Vec<usize>,
        gui_cfg: &GuiConfig,
    ) -> (FilterTab, Vec<f32>) {
        let f0 = Arc::clone(filter);
        let oc0 = Arc::clone(on_change);
        let mut filters = vec![];
        Self::build_filter_editor(
            &filter.lock().unwrap(),
            filter,
            &mut filters,
            0.0,
            0.05,
            on_change,
            path,
        );
        let ft = FilterTab {
            buttons: [Button::new(
                GuiElemCfg::default(),
                move |_| {
                    f0.lock().unwrap().filters.clear();
                    oc0(true);
                    vec![]
                },
                [Label::new(
                    GuiElemCfg::default(),
                    "clear filters".to_owned(),
                    Color::LIGHT_GRAY,
                    None,
                    Vec2::new(0.5, 0.5),
                )],
            )]
            .into_iter()
            .chain(
                match tab {
                    2 => &gui_cfg.filter_presets_artist,
                    1 => &gui_cfg.filter_presets_album,
                    _ => &gui_cfg.filter_presets_song,
                }
                .iter()
                .cloned()
                .map(|(text, preset)| {
                    let f = Arc::clone(&filter);
                    let oc = Arc::clone(&on_change);
                    Button::new(
                        GuiElemCfg::default(),
                        move |_| {
                            f.lock().unwrap().filters.push(preset.clone());
                            oc(true);
                            vec![]
                        },
                        [Label::new(
                            GuiElemCfg::default(),
                            text,
                            Color::GRAY,
                            None,
                            Vec2::new(0.5, 0.5),
                        )],
                    )
                }),
            )
            .collect(),
            filters,
        };
        let r = 0..ft.len();
        (ft, r.map(|_| line_height).collect())
    }
    fn build_filter_editor(
        filter: &Filter,
        mutex: &Arc<Mutex<Filter>>,
        children: &mut Vec<FilterLine>,
        mut indent: f32,
        indent_by: f32,
        on_change: &Arc<impl Fn(bool) + 'static>,
        path: Vec<usize>,
    ) {
        if filter.filters.len() > 1 {
            let mx = Arc::clone(mutex);
            let oc = Arc::clone(on_change);
            let p = path.clone();
            children.push(FilterLine::Joiner(Button::new(
                GuiElemCfg::at(Rectangle::from_tuples((indent, 0.0), (1.0, 1.0))),
                move |_| {
                    if let Some(f) = match mx.lock().unwrap().get_mut(&p) {
                        Some(Ok(f)) => f.inner_filter(),
                        Some(Err(f)) => Some(f),
                        None => None,
                    } {
                        f.and = !f.and;
                        oc(true);
                    }
                    vec![]
                },
                [Label::new(
                    GuiElemCfg::default(),
                    if filter.and { "AND" } else { "OR" }.to_owned(),
                    Color::WHITE,
                    None,
                    Vec2::new(0.5, 0.5),
                )],
            )));
        }
        indent += indent_by;
        for (i, f) in filter.filters.iter().enumerate() {
            let mut path = path.clone();
            path.push(i);
            match f {
                FilterType::Nested(f) => Self::build_filter_editor(
                    f, mutex, children, indent, indent_by, on_change, path,
                ),
                FilterType::Not(f) => {
                    children.push(FilterLine::Not(Label::new(
                        GuiElemCfg::at(Rectangle::from_tuples((indent, 0.0), (1.0, 1.0))),
                        "NOT".to_owned(),
                        Color::WHITE,
                        None,
                        Vec2::new(0.0, 0.5),
                    )));
                    Self::build_filter_editor(
                        f, mutex, children, indent, indent_by, on_change, path,
                    )
                }
                FilterType::TagEq(v) => {
                    let mut tf = TextField::new_adv(
                        GuiElemCfg::at(Rectangle::from_tuples((0.1, 0.0), (1.0, 1.0))),
                        v.to_owned(),
                        "tag value".to_owned(),
                        Color::GRAY,
                        Color::WHITE,
                    );
                    let mx = Arc::clone(mutex);
                    let oc = Arc::clone(on_change);
                    tf.on_changed = Some(Box::new(move |text| {
                        if let Some(Ok(FilterType::TagEq(v))) = mx.lock().unwrap().get_mut(&path) {
                            *v = text.to_owned();
                            oc(false);
                        }
                    }));
                    children.push(FilterLine::TagEq(Panel::new(
                        GuiElemCfg::at(Rectangle::from_tuples((indent, 0.0), (1.0, 1.0))),
                        (
                            Label::new(
                                GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.0), (0.1, 1.0))),
                                "=".to_owned(),
                                Color::WHITE,
                                None,
                                Vec2::new(0.5, 0.5),
                            ),
                            tf,
                        ),
                    )));
                }
                FilterType::TagStartsWith(v) => {
                    let mut tf = TextField::new_adv(
                        GuiElemCfg::at(Rectangle::from_tuples((0.1, 0.0), (1.0, 1.0))),
                        v.to_owned(),
                        "tag value".to_owned(),
                        Color::GRAY,
                        Color::WHITE,
                    );
                    let mx = Arc::clone(mutex);
                    let oc = Arc::clone(on_change);
                    tf.on_changed = Some(Box::new(move |text| {
                        if let Some(Ok(FilterType::TagStartsWith(v))) =
                            mx.lock().unwrap().get_mut(&path)
                        {
                            *v = text.to_owned();
                            oc(false);
                        }
                    }));
                    children.push(FilterLine::TagStartsWith(Panel::new(
                        GuiElemCfg::at(Rectangle::from_tuples((indent, 0.0), (1.0, 1.0))),
                        (
                            Label::new(
                                GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.0), (0.1, 1.0))),
                                ">".to_owned(),
                                Color::WHITE,
                                None,
                                Vec2::new(0.5, 0.5),
                            ),
                            tf,
                        ),
                    )));
                }
                FilterType::TagWithValueInt(v, min, max) => {
                    let mut tf = TextField::new_adv(
                        GuiElemCfg::at(Rectangle::from_tuples((0.1, 0.0), (0.6, 1.0))),
                        v.to_owned(),
                        "tag value".to_owned(),
                        Color::GRAY,
                        Color::WHITE,
                    );
                    let mx = Arc::clone(mutex);
                    let oc = Arc::clone(on_change);
                    let p = path.clone();
                    tf.on_changed = Some(Box::new(move |text| {
                        if let Some(Ok(FilterType::TagWithValueInt(v, _, _))) =
                            mx.lock().unwrap().get_mut(&p)
                        {
                            *v = text.to_owned();
                            oc(false);
                        }
                    }));
                    let mut tf1 = TextField::new_adv(
                        GuiElemCfg::at(Rectangle::from_tuples((0.6, 0.0), (0.8, 1.0))),
                        min.to_string(),
                        "min".to_owned(),
                        Color::GRAY,
                        Color::WHITE,
                    );
                    let mut tf2 = TextField::new_adv(
                        GuiElemCfg::at(Rectangle::from_tuples((0.8, 0.0), (1.0, 1.0))),
                        max.to_string(),
                        "max".to_owned(),
                        Color::GRAY,
                        Color::WHITE,
                    );
                    let mx = Arc::clone(mutex);
                    let oc = Arc::clone(on_change);
                    let p = path.clone();
                    tf1.on_changed = Some(Box::new(move |text| {
                        if let Ok(n) = text.parse() {
                            if let Some(Ok(FilterType::TagWithValueInt(_, v, _))) =
                                mx.lock().unwrap().get_mut(&p)
                            {
                                *v = n;
                                oc(false);
                            }
                        }
                    }));
                    let mx = Arc::clone(mutex);
                    let oc = Arc::clone(on_change);
                    let p = path.clone();
                    tf2.on_changed = Some(Box::new(move |text| {
                        if let Ok(n) = text.parse() {
                            if let Some(Ok(FilterType::TagWithValueInt(_, _, v))) =
                                mx.lock().unwrap().get_mut(&p)
                            {
                                *v = n;
                                oc(false);
                            }
                        }
                    }));
                    children.push(FilterLine::TagWithValueInt(Panel::new(
                        GuiElemCfg::at(Rectangle::from_tuples((indent, 0.0), (1.0, 1.0))),
                        (
                            Label::new(
                                GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.0), (0.1, 1.0))),
                                "..".to_owned(),
                                Color::WHITE,
                                None,
                                Vec2::new(0.5, 0.5),
                            ),
                            tf,
                            tf1,
                            tf2,
                        ),
                    )));
                }
            }
        }
    }
}
impl GuiElem for FilterPanel {
    fn draw(&mut self, info: &mut DrawInfo, _g: &mut speedy2d::Graphics2D) {
        // set line height
        if info.line_height != self.line_height {
            for h in &mut self.c_tab_main.children_heights {
                *h = info.line_height;
            }
            for h in &mut self.c_tab_filters_songs.children_heights {
                *h = info.line_height;
            }
            for h in &mut self.c_tab_filters_albums.children_heights {
                *h = info.line_height;
            }
            for h in &mut self.c_tab_filters_artists.children_heights {
                *h = info.line_height;
            }
            self.c_tab_main.config_mut().redraw = true;
            self.c_tab_filters_songs.config_mut().redraw = true;
            self.c_tab_filters_albums.config_mut().redraw = true;
            self.c_tab_filters_artists.config_mut().redraw = true;
            self.line_height = info.line_height;
        }
        // maybe switch tabs
        let new_tab = self.new_tab.load(std::sync::atomic::Ordering::Relaxed);
        let mut load_tab = false;
        if new_tab != usize::MAX {
            self.new_tab
                .store(usize::MAX, std::sync::atomic::Ordering::Relaxed);
            load_tab = true;

            match self.tab {
                0 => self.c_tab_filters_songs.config_mut().enabled = false,
                1 => self.c_tab_filters_albums.config_mut().enabled = false,
                2 => self.c_tab_filters_artists.config_mut().enabled = false,
                _ => (),
            }
            match new_tab {
                0 => self.c_tab_filters_songs.config_mut().enabled = true,
                1 => self.c_tab_filters_albums.config_mut().enabled = true,
                2 => self.c_tab_filters_artists.config_mut().enabled = true,
                _ => (),
            }
            *self.c_tab_change.children[self.tab].children[0]
                .content
                .color() = Color::GRAY;
            *self.c_tab_change.children[new_tab].children[0]
                .content
                .color() = Color::WHITE;
            self.tab = new_tab;
        }
        // load tab
        if load_tab {
            match new_tab {
                0 | 1 | 2 => {
                    let sb = match new_tab {
                        0 => &mut self.c_tab_filters_songs,
                        1 => &mut self.c_tab_filters_albums,
                        2 => &mut self.c_tab_filters_artists,
                        _ => unreachable!(),
                    };
                    let ssc = Arc::clone(&self.search_settings_changed);
                    let my_tab = new_tab;
                    let ntab = Arc::clone(&self.new_tab);
                    let (ft, heights) = Self::build_filter(
                        new_tab,
                        match new_tab {
                            0 => &self.filter_songs,
                            1 => &self.filter_albums,
                            2 => &self.filter_artists,
                            _ => unreachable!(),
                        },
                        info.line_height,
                        &Arc::new(move |update_ui| {
                            if update_ui {
                                ntab.store(my_tab, std::sync::atomic::Ordering::Relaxed);
                            }
                            ssc.store(true, std::sync::atomic::Ordering::Relaxed);
                        }),
                        vec![],
                        info.gui_config,
                    );
                    sb.children = ft;
                    sb.children_heights = heights;
                    sb.config_mut().redraw = true;
                }
                _ => {}
            }
        }
    }
    fn config(&self) -> &GuiElemCfg {
        &self.config
    }
    fn config_mut(&mut self) -> &mut GuiElemCfg {
        &mut self.config
    }
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut dyn GuiElem> + '_> {
        Box::new(
            [
                self.c_tab_main.elem_mut(),
                self.c_tab_filters_songs.elem_mut(),
                self.c_tab_filters_albums.elem_mut(),
                self.c_tab_filters_artists.elem_mut(),
                self.c_tab_change.elem_mut(),
            ]
            .into_iter(),
        )
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
#[derive(Clone)]
pub struct Filter {
    and: bool,
    filters: Vec<FilterType>,
}
#[derive(Clone)]
#[allow(unused)]
pub enum FilterType {
    Nested(Filter),
    Not(Filter),
    TagEq(String),
    TagStartsWith(String),
    /// true if the tag is '<String><Integer>' and Integer is between min and max (both inclusive)
    /// note: <String> usually ends with '='.
    TagWithValueInt(String, i32, i32),
}
impl Filter {
    pub fn passes(&self, gd: &GeneralData) -> bool {
        if self.filters.is_empty() {
            return true;
        }
        let mut iter = self.filters.iter().map(|v| v.passes(gd));
        if self.and {
            iter.all(|v| v)
        } else {
            iter.any(|v| v)
        }
    }
    pub fn get_mut(&mut self, path: &[usize]) -> Option<Result<&mut FilterType, &mut Self>> {
        if let Some(i) = path.first() {
            let p = &path[1..];
            if let Some(f) = self.filters.get_mut(*i) {
                f.get_mut(p)
            } else {
                None
            }
        } else {
            Some(Err(self))
        }
    }
}
impl FilterType {
    pub fn passes(&self, gd: &GeneralData) -> bool {
        match self {
            Self::Nested(f) => f.passes(gd),
            Self::Not(f) => !f.passes(gd),
            Self::TagEq(v) => gd.tags.iter().any(|t| t == v),
            Self::TagStartsWith(v) => gd.tags.iter().any(|t| t.starts_with(v)),
            Self::TagWithValueInt(v, min, max) => gd.tags.iter().any(|t| {
                if t.starts_with(v) {
                    if let Ok(val) = t[v.len()..].parse() {
                        *min <= val && val <= *max
                    } else {
                        false
                    }
                } else {
                    false
                }
            }),
        }
    }
    pub fn get_mut(&mut self, path: &[usize]) -> Option<Result<&mut Self, &mut Filter>> {
        if path.is_empty() {
            Some(Ok(self))
        } else {
            if let Some(f) = self.inner_filter() {
                f.get_mut(path)
            } else {
                None
            }
        }
    }
    pub fn inner_filter(&mut self) -> Option<&mut Filter> {
        match self {
            Self::Nested(f) | Self::Not(f) => Some(f),
            Self::TagEq(_) | Self::TagStartsWith(_) | Self::TagWithValueInt(..) => None,
        }
    }
}

mod selected {
    use super::*;
    #[derive(Clone)]
    pub struct Selected(
        // artist, album, songs
        Arc<Mutex<(HashSet<ArtistId>, HashSet<AlbumId>, HashSet<SongId>)>>,
        Arc<AtomicBool>,
    );
    impl Selected {
        pub fn new(update: Arc<AtomicBool>) -> Self {
            Self(Default::default(), update)
        }
        pub fn clear(&self) {
            self.set_to(HashSet::new(), HashSet::new(), HashSet::new())
        }
        pub fn set_to(&self, artists: HashSet<u64>, albums: HashSet<u64>, songs: HashSet<u64>) {
            let mut s = self.0.lock().unwrap();
            s.0 = artists;
            s.1 = albums;
            s.2 = songs;
            self.changed();
        }
        pub fn contains_artist(&self, id: &ArtistId) -> bool {
            self.0.lock().unwrap().0.contains(id)
        }
        pub fn contains_album(&self, id: &AlbumId) -> bool {
            self.0.lock().unwrap().1.contains(id)
        }
        pub fn contains_song(&self, id: &SongId) -> bool {
            self.0.lock().unwrap().2.contains(id)
        }
        pub fn insert_artist(&self, id: ArtistId) -> bool {
            self.changed();
            self.0.lock().unwrap().0.insert(id)
        }
        pub fn insert_album(&self, id: AlbumId) -> bool {
            self.changed();
            self.0.lock().unwrap().1.insert(id)
        }
        pub fn insert_song(&self, id: SongId) -> bool {
            self.changed();
            self.0.lock().unwrap().2.insert(id)
        }
        pub fn remove_artist(&self, id: &ArtistId) -> bool {
            self.changed();
            self.0.lock().unwrap().0.remove(id)
        }
        pub fn remove_album(&self, id: &AlbumId) -> bool {
            self.changed();
            self.0.lock().unwrap().1.remove(id)
        }
        pub fn remove_song(&self, id: &SongId) -> bool {
            self.changed();
            self.0.lock().unwrap().2.remove(id)
        }
        pub fn view<T>(
            &self,
            f: impl FnOnce(&(HashSet<ArtistId>, HashSet<AlbumId>, HashSet<SongId>)) -> T,
        ) -> T {
            f(&self.0.lock().unwrap())
        }
        pub fn view_mut<T>(
            &self,
            f: impl FnOnce(&mut (HashSet<ArtistId>, HashSet<AlbumId>, HashSet<SongId>)) -> T,
        ) -> T {
            let v = f(&mut self.0.lock().unwrap());
            self.changed();
            v
        }
        fn changed(&self) {
            self.1.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        pub fn as_queue(&self, lb: &LibraryBrowser, db: &Database) -> Vec<Queue> {
            let lock = self.0.lock().unwrap();
            let (sel_artists, sel_albums, sel_songs) = &*lock;
            let mut out = vec![];
            for (artist, singles, albums) in &lb.library_sorted {
                let artist_selected = sel_artists.contains(artist);
                let mut local_artist_owned = vec![];
                let mut local_artist = if artist_selected {
                    &mut local_artist_owned
                } else {
                    &mut out
                };
                for song in singles {
                    let song_selected = sel_songs.contains(song);
                    if song_selected {
                        local_artist.push(QueueContent::Song(*song).into());
                    }
                }
                for (album, songs) in albums {
                    let album_selected = sel_albums.contains(album);
                    let mut local_album_owned = vec![];
                    let local_album = if album_selected {
                        &mut local_album_owned
                    } else {
                        &mut local_artist
                    };
                    for song in songs {
                        let song_selected = sel_songs.contains(song);
                        if song_selected {
                            local_album.push(QueueContent::Song(*song).into());
                        }
                    }
                    if album_selected {
                        local_artist.push(
                            QueueContent::Folder(
                                0,
                                local_album_owned,
                                match db.albums().get(album) {
                                    Some(v) => v.name.clone(),
                                    None => "< unknown album >".to_owned(),
                                },
                            )
                            .into(),
                        );
                    }
                }
                if artist_selected {
                    out.push(
                        QueueContent::Folder(
                            0,
                            local_artist_owned,
                            match db.artists().get(artist) {
                                Some(v) => v.name.to_owned(),
                                None => "< unknown artist >".to_owned(),
                            },
                        )
                        .into(),
                    );
                }
            }
            out
        }
    }
}
