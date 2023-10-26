use std::{
    cmp::Ordering,
    collections::HashSet,
    sync::Arc,
    sync::{
        atomic::{AtomicBool, AtomicUsize},
        mpsc, Mutex,
    },
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
    gui::{Dragging, DrawInfo, GuiAction, GuiElem, GuiElemCfg, GuiElemTrait},
    gui_base::{Button, Panel, ScrollBox},
    gui_text::{Label, TextField},
    gui_wrappers::WithFocusHotkey,
};

use self::selected::Selected;

/*

This is responsible for showing the library,
with Regex search and drag-n-drop.

*/

pub struct LibraryBrowser {
    config: GuiElemCfg,
    pub children: Vec<GuiElem>,
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
    filter_state: f32,
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
}
impl Clone for LibraryBrowser {
    fn clone(&self) -> Self {
        Self::new(self.config.clone())
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
            for (artist, singles, albums, _) in &lb.library_filtered {
                let artist_selected = sel_artists.contains(artist);
                let mut local_artist_owned = vec![];
                let mut local_artist = if artist_selected {
                    &mut local_artist_owned
                } else {
                    &mut out
                };
                for (song, _) in singles {
                    let song_selected = sel_songs.contains(song);
                    if song_selected {
                        local_artist.push(QueueContent::Song(*song).into());
                    }
                }
                for (album, songs, _) in albums {
                    let album_selected = sel_albums.contains(album);
                    let mut local_album_owned = vec![];
                    let local_album = if album_selected {
                        &mut local_album_owned
                    } else {
                        &mut local_artist
                    };
                    for (song, _) in songs {
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
        let (do_something_sender, do_something_receiver) = mpsc::channel();
        let search_settings_changed = Arc::new(AtomicBool::new(false));
        let search_was_case_sensitive = false;
        let search_is_case_sensitive = Arc::new(AtomicBool::new(search_was_case_sensitive));
        let search_prefers_start_matches = true;
        let search_prefer_start_matches = Arc::new(AtomicBool::new(search_prefers_start_matches));
        let filter_target_state = Arc::new(AtomicBool::new(false));
        let fts = Arc::clone(&filter_target_state);
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
            children: vec![
                GuiElem::new(search_artist),
                GuiElem::new(search_album),
                GuiElem::new(search_song),
                GuiElem::new(library_scroll_box),
                GuiElem::new(filter_button),
                GuiElem::new(FilterPanel::new(
                    Arc::clone(&search_settings_changed),
                    Arc::clone(&search_is_case_sensitive),
                    Arc::clone(&search_prefer_start_matches),
                    Arc::clone(&filter_songs),
                    Arc::clone(&filter_albums),
                    Arc::clone(&filter_artists),
                    selected.clone(),
                    do_something_sender.clone(),
                )),
            ],
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
            filter_state: 0.0,
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
            let v = &mut self.children[0].try_as_mut::<TextField>().unwrap().children[0]
                .try_as_mut::<Label>()
                .unwrap()
                .content;
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
            let v = &mut self.children[1].try_as_mut::<TextField>().unwrap().children[0]
                .try_as_mut::<Label>()
                .unwrap()
                .content;
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
            self.config.redraw = true;
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
        scan: speedy2d::window::KeyScancode,
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
        for (artist_id, singles, albums, _artist_filterscore) in self.library_filtered.iter() {
            elems.push(self.build_ui_element_artist(*artist_id, db, line_height));
            for (song_id, _song_filterscore) in singles {
                elems.push(self.build_ui_element_song(*song_id, db, line_height));
            }
            for (album_id, songs, _album_filterscore) in albums {
                elems.push(self.build_ui_element_album(*album_id, db, line_height));
                for (song_id, _song_filterscore) in songs {
                    elems.push(self.build_ui_element_song(*song_id, db, line_height));
                }
            }
        }
        let library_scroll_box = self.children[3].try_as_mut::<ScrollBox>().unwrap();
        library_scroll_box.children = elems;
        library_scroll_box.config_mut().redraw = true;
    }
    fn build_ui_element_artist(&self, id: ArtistId, db: &Database, h: f32) -> (GuiElem, f32) {
        (
            GuiElem::new(ListArtist::new(
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
    fn build_ui_element_album(&self, id: ArtistId, db: &Database, h: f32) -> (GuiElem, f32) {
        (
            GuiElem::new(ListAlbum::new(
                GuiElemCfg::default(),
                id,
                if let Some(v) = db.albums().get(&id) {
                    v.name.to_owned()
                } else {
                    format!("[ Album #{id} ]")
                },
                self.selected.clone(),
            )),
            h * 1.5,
        )
    }
    fn build_ui_element_song(&self, id: ArtistId, db: &Database, h: f32) -> (GuiElem, f32) {
        (
            GuiElem::new(ListSong::new(
                GuiElemCfg::default(),
                id,
                if let Some(v) = db.songs().get(&id) {
                    v.title.to_owned()
                } else {
                    format!("[ Song #{id} ]")
                },
                self.selected.clone(),
            )),
            h,
        )
    }
}

#[derive(Clone)]
struct ListArtist {
    config: GuiElemCfg,
    id: ArtistId,
    children: Vec<GuiElem>,
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
            children: vec![GuiElem::new(label)],
            mouse: false,
            mouse_pos: Vec2::ZERO,
            selected,
            sel: false,
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
        if self.config.redraw {
            self.config.redraw = false;
            let sel = self.selected.contains_artist(&self.id);
            if sel != self.sel {
                self.sel = sel;
                if sel {
                    self.children.push(GuiElem::new(Panel::with_background(
                        GuiElemCfg::default(),
                        vec![],
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
                            gui.gui
                                .inner
                                .children()
                                .nth(3)
                                .unwrap()
                                .inner
                                .children()
                                .nth(2)
                                .unwrap()
                                .try_as()
                                .unwrap(),
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
            let mouse_pos = self.mouse_pos;
            let w = self.config.pixel_pos.width();
            let h = self.config.pixel_pos.height();
            let mut el = GuiElem::new(self.clone());
            if self.sel {
                vec![]
            } else {
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

#[derive(Clone)]
struct ListAlbum {
    config: GuiElemCfg,
    id: AlbumId,
    children: Vec<GuiElem>,
    mouse: bool,
    mouse_pos: Vec2,
    selected: Selected,
    sel: bool,
}
impl ListAlbum {
    pub fn new(mut config: GuiElemCfg, id: AlbumId, name: String, selected: Selected) -> Self {
        let label = Label::new(
            GuiElemCfg::default(),
            name,
            Color::from_int_rgb(8, 61, 47),
            None,
            Vec2::new(0.0, 0.5),
        );
        config.redraw = true;
        Self {
            config: config.w_mouse(),
            id,
            children: vec![GuiElem::new(label)],
            mouse: false,
            mouse_pos: Vec2::ZERO,
            selected,
            sel: false,
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
        if self.config.redraw {
            self.config.redraw = false;
            let sel = self.selected.contains_album(&self.id);
            if sel != self.sel {
                self.sel = sel;
                if sel {
                    self.children.push(GuiElem::new(Panel::with_background(
                        GuiElemCfg::default(),
                        vec![],
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
                            gui.gui
                                .inner
                                .children()
                                .nth(3)
                                .unwrap()
                                .inner
                                .children()
                                .nth(2)
                                .unwrap()
                                .try_as()
                                .unwrap(),
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
            let mouse_pos = self.mouse_pos;
            let w = self.config.pixel_pos.width();
            let h = self.config.pixel_pos.height();
            let mut el = GuiElem::new(self.clone());
            if self.sel {
                vec![]
            } else {
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

#[derive(Clone)]
struct ListSong {
    config: GuiElemCfg,
    id: SongId,
    children: Vec<GuiElem>,
    mouse: bool,
    mouse_pos: Vec2,
    selected: Selected,
    sel: bool,
}
impl ListSong {
    pub fn new(mut config: GuiElemCfg, id: SongId, name: String, selected: Selected) -> Self {
        let label = Label::new(
            GuiElemCfg::default(),
            name,
            Color::from_int_rgb(175, 175, 175),
            None,
            Vec2::new(0.0, 0.5),
        );
        config.redraw = true;
        Self {
            config: config.w_mouse(),
            id,
            children: vec![GuiElem::new(label)],
            mouse: false,
            mouse_pos: Vec2::ZERO,
            selected,
            sel: false,
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
        if self.config.redraw {
            self.config.redraw = false;
            let sel = self.selected.contains_song(&self.id);
            if sel != self.sel {
                self.sel = sel;
                if sel {
                    self.children.push(GuiElem::new(Panel::with_background(
                        GuiElemCfg::default(),
                        vec![],
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
                            gui.gui
                                .inner
                                .children()
                                .nth(3)
                                .unwrap()
                                .inner
                                .children()
                                .nth(2)
                                .unwrap()
                                .try_as()
                                .unwrap(),
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
            let mouse_pos = self.mouse_pos;
            let w = self.config.pixel_pos.width();
            let h = self.config.pixel_pos.height();
            let mut el = GuiElem::new(self.clone());
            if self.sel {
                vec![]
            } else {
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
}

#[derive(Clone)]
struct FilterPanel {
    config: GuiElemCfg,
    children: Vec<GuiElem>,
    search_settings_changed: Arc<AtomicBool>,
    tab: usize,
    new_tab: Arc<AtomicUsize>,
    line_height: f32,
    filter_songs: Arc<Mutex<Filter>>,
    filter_albums: Arc<Mutex<Filter>>,
    filter_artists: Arc<Mutex<Filter>>,
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
        let tab_main = GuiElem::new(ScrollBox::new(
            GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.0), (VSPLIT, 1.0))),
            crate::gui_base::ScrollBoxSizeUnit::Pixels,
            vec![
                (
                    GuiElem::new(Button::new(
                        GuiElemCfg::default(),
                        move |button| {
                            let v = !search_is_case_sensitive
                                .load(std::sync::atomic::Ordering::Relaxed);
                            search_is_case_sensitive.store(v, std::sync::atomic::Ordering::Relaxed);
                            ssc1.store(true, std::sync::atomic::Ordering::Relaxed);
                            *button
                                .children()
                                .next()
                                .unwrap()
                                .try_as_mut::<Label>()
                                .unwrap()
                                .content
                                .text() = if v {
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
                        move |button| {
                            let v = !search_prefer_start_matches
                                .load(std::sync::atomic::Ordering::Relaxed);
                            search_prefer_start_matches
                                .store(v, std::sync::atomic::Ordering::Relaxed);
                            ssc2.store(true, std::sync::atomic::Ordering::Relaxed);
                            *button
                                .children()
                                .next()
                                .unwrap()
                                .try_as_mut::<Label>()
                                .unwrap()
                                .content
                                .text() = if v {
                                FP_PREFSTART_Y.to_owned()
                            } else {
                                FP_PREFSTART_N.to_owned()
                            };
                            vec![]
                        },
                        vec![GuiElem::new(Label::new(
                            GuiElemCfg::default(),
                            if prefer_start_matches {
                                FP_PREFSTART_Y.to_owned()
                            } else {
                                FP_PREFSTART_N.to_owned()
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
                        move |_| {
                            let mut sel = sel3.clear();
                            vec![]
                        },
                        vec![GuiElem::new(Label::new(
                            GuiElemCfg::default(),
                            "deselect all".to_owned(),
                            Color::GRAY,
                            None,
                            Vec2::new(0.5, 0.5),
                        ))],
                    )),
                    1.0,
                ),
                (
                    GuiElem::new(Panel::new(
                        GuiElemCfg::default(),
                        vec![
                            GuiElem::new(Button::new(
                                GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.0), (0.5, 1.0))),
                                {
                                    let dss = do_something_sender.clone();
                                    move |_| {
                                        dss.send(Box::new(|s| s.selected_add_all())).unwrap();
                                        vec![]
                                    }
                                },
                                vec![GuiElem::new(Label::new(
                                    GuiElemCfg::default(),
                                    "select all".to_owned(),
                                    Color::GRAY,
                                    None,
                                    Vec2::new(0.5, 0.5),
                                ))],
                            )),
                            GuiElem::new(Button::new(
                                GuiElemCfg::at(Rectangle::from_tuples((0.55, 0.0), (0.75, 1.0))),
                                {
                                    let dss = do_something_sender.clone();
                                    move |_| {
                                        dss.send(Box::new(|s| s.selected_add_songs())).unwrap();
                                        vec![]
                                    }
                                },
                                vec![GuiElem::new(Label::new(
                                    GuiElemCfg::default(),
                                    "songs".to_owned(),
                                    Color::GRAY,
                                    None,
                                    Vec2::new(0.5, 0.5),
                                ))],
                            )),
                            GuiElem::new(Button::new(
                                GuiElemCfg::at(Rectangle::from_tuples((0.8, 0.0), (1.0, 1.0))),
                                {
                                    let dss = do_something_sender.clone();
                                    move |_| {
                                        dss.send(Box::new(|s| s.selected_add_albums())).unwrap();
                                        vec![]
                                    }
                                },
                                vec![GuiElem::new(Label::new(
                                    GuiElemCfg::default(),
                                    "albums".to_owned(),
                                    Color::GRAY,
                                    None,
                                    Vec2::new(0.5, 0.5),
                                ))],
                            )),
                        ],
                    )),
                    1.0,
                ),
            ],
        ));
        let tab_filters_songs = GuiElem::new(ScrollBox::new(
            GuiElemCfg::at(Rectangle::from_tuples((VSPLIT, 0.0), (1.0, 1.0))),
            crate::gui_base::ScrollBoxSizeUnit::Pixels,
            vec![],
        ));
        let tab_filters_albums = GuiElem::new(ScrollBox::new(
            GuiElemCfg::at(Rectangle::from_tuples((VSPLIT, 0.0), (1.0, 1.0))).disabled(),
            crate::gui_base::ScrollBoxSizeUnit::Pixels,
            vec![],
        ));
        let tab_filters_artists = GuiElem::new(ScrollBox::new(
            GuiElemCfg::at(Rectangle::from_tuples((VSPLIT, 0.0), (1.0, 1.0))).disabled(),
            crate::gui_base::ScrollBoxSizeUnit::Pixels,
            vec![],
        ));
        let new_tab = Arc::new(AtomicUsize::new(0));
        let set_tab_1 = Arc::clone(&new_tab);
        let set_tab_2 = Arc::clone(&new_tab);
        let set_tab_3 = Arc::clone(&new_tab);
        const HEIGHT: f32 = 0.1;
        Self {
            config: GuiElemCfg::default().disabled(),
            children: vec![
                GuiElem::new(Panel::new(
                    GuiElemCfg::at(Rectangle::from_tuples((VSPLIT, 0.0), (1.0, HEIGHT))),
                    vec![
                        GuiElem::new(Button::new(
                            GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.0), (0.33, 1.0))),
                            move |_| {
                                set_tab_1.store(0, std::sync::atomic::Ordering::Relaxed);
                                vec![]
                            },
                            vec![GuiElem::new(Label::new(
                                GuiElemCfg::default(),
                                "Filter Songs".to_owned(),
                                Color::GRAY,
                                None,
                                Vec2::new(0.5, 0.5),
                            ))],
                        )),
                        GuiElem::new(Button::new(
                            GuiElemCfg::at(Rectangle::from_tuples((0.33, 0.0), (0.67, 1.0))),
                            move |_| {
                                set_tab_2.store(1, std::sync::atomic::Ordering::Relaxed);
                                vec![]
                            },
                            vec![GuiElem::new(Label::new(
                                GuiElemCfg::default(),
                                "Filter Albums".to_owned(),
                                Color::GRAY,
                                None,
                                Vec2::new(0.5, 0.5),
                            ))],
                        )),
                        GuiElem::new(Button::new(
                            GuiElemCfg::at(Rectangle::from_tuples((0.67, 0.0), (1.0, 1.0))),
                            move |_| {
                                set_tab_3.store(2, std::sync::atomic::Ordering::Relaxed);
                                vec![]
                            },
                            vec![GuiElem::new(Label::new(
                                GuiElemCfg::default(),
                                "Filter Artists".to_owned(),
                                Color::GRAY,
                                None,
                                Vec2::new(0.5, 0.5),
                            ))],
                        )),
                    ],
                )),
                GuiElem::new(Panel::new(
                    GuiElemCfg::at(Rectangle::from_tuples((0.0, HEIGHT), (1.0, 1.0))),
                    vec![tab_filters_songs, tab_filters_albums, tab_filters_artists],
                )),
                tab_main,
            ],
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
        filter: &Arc<Mutex<Filter>>,
        line_height: f32,
        on_change: &Arc<impl Fn(bool) + 'static>,
        path: Vec<usize>,
    ) -> Vec<(GuiElem, f32)> {
        let f0 = Arc::clone(filter);
        let oc0 = Arc::clone(on_change);
        let f1 = Arc::clone(filter);
        let f2 = Arc::clone(filter);
        let oc1 = Arc::clone(on_change);
        let oc2 = Arc::clone(on_change);
        let mut children = vec![
            GuiElem::new(Button::new(
                GuiElemCfg::default(),
                move |_| {
                    f0.lock().unwrap().filters.clear();
                    oc0(true);
                    vec![]
                },
                vec![GuiElem::new(Label::new(
                    GuiElemCfg::default(),
                    "clear filters".to_owned(),
                    Color::LIGHT_GRAY,
                    None,
                    Vec2::new(0.5, 0.5),
                ))],
            )),
            GuiElem::new(Button::new(
                GuiElemCfg::default(),
                move |_| {
                    f1.lock()
                        .unwrap()
                        .filters
                        .push(FilterType::TagEq("Fav".to_owned()));
                    oc1(true);
                    vec![]
                },
                vec![GuiElem::new(Label::new(
                    GuiElemCfg::default(),
                    "must have tag".to_owned(),
                    Color::GRAY,
                    None,
                    Vec2::new(0.5, 0.5),
                ))],
            )),
            GuiElem::new(Button::new(
                GuiElemCfg::default(),
                move |_| {
                    f2.lock().unwrap().filters.push(FilterType::TagWithValueInt(
                        "Year=".to_owned(),
                        1990,
                        2000,
                    ));
                    oc2(true);
                    vec![]
                },
                vec![GuiElem::new(Label::new(
                    GuiElemCfg::default(),
                    "tag with integer value between (min) and (max)".to_owned(),
                    Color::GRAY,
                    None,
                    Vec2::new(0.5, 0.5),
                ))],
            )),
        ];
        Self::build_filter_editor(
            &filter.lock().unwrap(),
            filter,
            &mut children,
            0.0,
            0.05,
            on_change,
            path,
        );
        children.into_iter().map(|v| (v, line_height)).collect()
    }
    fn build_filter_editor(
        filter: &Filter,
        mutex: &Arc<Mutex<Filter>>,
        children: &mut Vec<GuiElem>,
        mut indent: f32,
        indent_by: f32,
        on_change: &Arc<impl Fn(bool) + 'static>,
        path: Vec<usize>,
    ) {
        if filter.filters.len() > 1 {
            let mx = Arc::clone(mutex);
            let oc = Arc::clone(on_change);
            let p = path.clone();
            children.push(GuiElem::new(Button::new(
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
                vec![GuiElem::new(Label::new(
                    GuiElemCfg::default(),
                    if filter.and { "AND" } else { "OR" }.to_owned(),
                    Color::WHITE,
                    None,
                    Vec2::new(0.5, 0.5),
                ))],
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
                    children.push(GuiElem::new(Label::new(
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
                    children.push(GuiElem::new(Panel::new(
                        GuiElemCfg::at(Rectangle::from_tuples((indent, 0.0), (1.0, 1.0))),
                        vec![
                            GuiElem::new(Label::new(
                                GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.0), (0.1, 1.0))),
                                "=".to_owned(),
                                Color::WHITE,
                                None,
                                Vec2::new(0.5, 0.5),
                            )),
                            GuiElem::new(tf),
                        ],
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
                    children.push(GuiElem::new(Panel::new(
                        GuiElemCfg::at(Rectangle::from_tuples((indent, 0.0), (1.0, 1.0))),
                        vec![
                            GuiElem::new(Label::new(
                                GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.0), (0.1, 1.0))),
                                ">".to_owned(),
                                Color::WHITE,
                                None,
                                Vec2::new(0.5, 0.5),
                            )),
                            GuiElem::new(tf),
                        ],
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
                    children.push(GuiElem::new(Panel::new(
                        GuiElemCfg::at(Rectangle::from_tuples((indent, 0.0), (1.0, 1.0))),
                        vec![
                            GuiElem::new(Label::new(
                                GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.0), (0.1, 1.0))),
                                "..".to_owned(),
                                Color::WHITE,
                                None,
                                Vec2::new(0.5, 0.5),
                            )),
                            GuiElem::new(tf),
                            GuiElem::new(tf1),
                            GuiElem::new(tf2),
                        ],
                    )));
                }
            }
        }
    }
}
impl GuiElemTrait for FilterPanel {
    fn draw(&mut self, info: &mut DrawInfo, _g: &mut speedy2d::Graphics2D) {
        // set line height
        if info.line_height != self.line_height {
            let sb = self.children[2].try_as_mut::<ScrollBox>().unwrap();
            for (_, h) in &mut sb.children {
                *h = info.line_height;
            }
            for c in &mut self.children[1].inner.children() {
                if let Some(sb) = c.try_as_mut::<ScrollBox>() {
                    for (_, h) in &mut sb.children {
                        *h = info.line_height;
                    }
                }
            }
            self.line_height = info.line_height;
        }
        // maybe switch tabs
        let mut new_tab = self.new_tab.load(std::sync::atomic::Ordering::Relaxed);
        let mut load_tab = false;
        if new_tab != usize::MAX {
            self.new_tab
                .store(usize::MAX, std::sync::atomic::Ordering::Relaxed);
            load_tab = true;
            if new_tab == usize::MAX {
                self.new_tab
                    .store(self.tab, std::sync::atomic::Ordering::Relaxed);
                new_tab = self.tab;
            } else {
                self.children[1]
                    .inner
                    .children()
                    .nth(self.tab)
                    .unwrap()
                    .inner
                    .config_mut()
                    .enabled = false;
                self.children[1]
                    .inner
                    .children()
                    .nth(new_tab)
                    .unwrap()
                    .inner
                    .config_mut()
                    .enabled = true;
                *self.children[0]
                    .inner
                    .children()
                    .nth(self.tab)
                    .unwrap()
                    .try_as_mut::<Button>()
                    .unwrap()
                    .children[0]
                    .try_as_mut::<Label>()
                    .unwrap()
                    .content
                    .color() = Color::GRAY;
                *self.children[0]
                    .inner
                    .children()
                    .nth(new_tab)
                    .unwrap()
                    .try_as_mut::<Button>()
                    .unwrap()
                    .children[0]
                    .try_as_mut::<Label>()
                    .unwrap()
                    .content
                    .color() = Color::WHITE;
                self.tab = new_tab;
            }
        }
        // load tab
        if load_tab {
            match new_tab {
                0 | 1 | 2 => {
                    let sb = self.children[1]
                        .inner
                        .children()
                        .nth(new_tab)
                        .unwrap()
                        .try_as_mut::<ScrollBox>()
                        .unwrap();
                    let ssc = Arc::clone(&self.search_settings_changed);
                    let my_tab = new_tab;
                    let ntab = Arc::clone(&self.new_tab);
                    sb.children = Self::build_filter(
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
                    );
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
struct Filter {
    and: bool,
    filters: Vec<FilterType>,
}
enum FilterType {
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
