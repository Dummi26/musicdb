use std::{collections::BTreeSet, sync::Arc, time::Instant};

use musicdb_lib::data::{ArtistId, GeneralData};
use speedy2d::{Graphics2D, color::Color, dimen::Vec2, shape::Rectangle};

use crate::{
    gui::{DrawInfo, GuiElem, GuiElemCfg},
    gui_anim::AnimationController,
    gui_base::{Button, Panel, ScrollBox},
    gui_text::{Label, TextField},
};

pub const ELEM_HEIGHT: f32 = 32.0;

pub enum Event {
    AddTag(String),
    RemoveTag(String),
}
impl Event {
    pub fn handle<'a, T: From<Event> + 'static>(
        self,
        general: impl Iterator<Item = &'a mut GeneralData>,
        c_tags: &mut Vec<EditorForAnyTagInList<T>>,
        c_new_tag: &mut EditorForAnyTagAdder<T>,
        c_scrollbox_children_heights: (usize, &mut Vec<f32>),
        event_sender: &std::sync::mpsc::Sender<T>,
        time: Instant,
    ) -> bool {
        let mut redraw_c_scrollbox = false;
        match self {
            Event::AddTag(tag) => {
                c_new_tag.clear(time);
                for general in general {
                    if !general.tags.contains(&tag) {
                        general.tags.push(tag.clone());
                    }
                }
                c_scrollbox_children_heights
                    .1
                    .insert(c_scrollbox_children_heights.0 + c_tags.len(), ELEM_HEIGHT);
                let i = c_tags.len();
                c_tags.push(EditorForAnyTagInList::new(
                    tag,
                    i,
                    event_sender.clone(),
                    GuiElemCfg::default(),
                ));
                redraw_c_scrollbox = true;
            }
            Event::RemoveTag(tag) => {
                for general in general {
                    general.tags.retain(|t| t.as_str() != tag.as_str());
                }
                let mut min_i = c_tags.len();
                while let Some(i) = c_tags.iter().rposition(|t| t.tag.as_str() == tag.as_str()) {
                    min_i = i;
                    c_tags.remove(i);
                    for t in &mut c_tags[i..] {
                        t.index -= 1;
                    }
                }
                for t in &mut c_tags[min_i..] {
                    t.changed_index();
                    redraw_c_scrollbox = true;
                }
            }
        }
        redraw_c_scrollbox
    }
}

pub fn apply<'a, T: From<Event> + 'static>(
    general: impl Iterator<Item = &'a mut GeneralData>,
    c_tags: &mut [EditorForAnyTagInList<T>],
) {
    for general in general {
        for tag in c_tags.iter_mut() {
            for t in (general.tags.iter_mut()).filter(|t| t.as_str() == tag.tag.as_str()) {
                *t = tag.text_field().c_input.content.get_text().to_owned();
            }
        }
    }
}

pub struct EditorForAnyTagInList<T: From<Event> + 'static> {
    config: GuiElemCfg,
    pub tag: String,
    pub index: usize,
    panel: Panel<(TextField, Button<[IconDelete; 1]>)>,
    sender: std::sync::mpsc::Sender<T>,
}

impl<T: From<Event> + 'static> EditorForAnyTagInList<T> {
    pub fn new(
        tag: String,
        index: usize,
        sender: std::sync::mpsc::Sender<T>,
        config: GuiElemCfg,
    ) -> Self {
        let mut tag_text = TextField::new(
            GuiElemCfg::default(),
            tag.clone(),
            Color::DARK_GRAY,
            Color::WHITE,
        );
        tag_text.set_text(tag.clone());
        tag_text.on_changed_mut = {
            Some(Box::new(move |tag_text, text| {
                *tag_text.c_input.content.color() =
                    if text.as_str() != tag_text.c_hint.content.get_text().as_str() {
                        Color::CYAN
                    } else {
                        Color::WHITE
                    };
            }))
        };
        let rm_button = Button::new(
            GuiElemCfg::default(),
            {
                let tag = tag.clone();
                let sender = sender.clone();
                move |btn| {
                    btn.disable();
                    sender.send(Event::RemoveTag(tag.clone()).into()).unwrap();
                    vec![]
                }
            },
            [IconDelete::new(GuiElemCfg::default())],
        );
        let panel = Panel::new(GuiElemCfg::default(), (tag_text, rm_button));
        let mut s = Self {
            config,
            tag,
            index,
            panel,
            sender,
        };
        s.changed_index();
        s
    }
    fn changed_index(&mut self) {
        self.config_mut().redraw_once();
        self.panel.background = Some(match self.index % 2 {
            1 => Color::from_rgba(0.0, 1.0, 0.0, 0.1),
            _ => Color::from_rgba(0.0, 0.0, 1.0, 0.14),
        });
        self.panel.config_mut().redraw_once();
    }
    fn row(&self) -> &(TextField, Button<[IconDelete; 1]>) {
        &self.panel.children
    }
    fn row_mut(&mut self) -> &mut (TextField, Button<[IconDelete; 1]>) {
        &mut self.panel.children
    }
    fn text_field(&self) -> &TextField {
        &self.panel.children.0
    }
    fn rm_button(&self) -> &Button<[IconDelete; 1]> {
        &self.panel.children.1
    }
    fn text_field_mut(&mut self) -> &mut TextField {
        &mut self.panel.children.0
    }
    fn rm_button_mut(&mut self) -> &mut Button<[IconDelete; 1]> {
        &mut self.panel.children.1
    }
}

impl<T: From<Event> + 'static> GuiElem for EditorForAnyTagInList<T> {
    fn draw(&mut self, info: &mut DrawInfo, g: &mut Graphics2D) {
        let rm_button_size = (info.pos.height() * 0.8).min(info.pos.width() * 0.33);
        let rm_button_padding = (info.pos.height() - rm_button_size) / 2.0;
        let label_padding = info.pos.height() * 0.05;
        let x_split = (info.pos.width() - rm_button_size) / info.pos.width();
        self.rm_button_mut().config_mut().pos = Rectangle::from_tuples(
            (x_split, rm_button_padding / info.pos.height()),
            (1.0, 1.0 - rm_button_padding / info.pos.height()),
        );
        self.text_field_mut().config_mut().pos = Rectangle::from_tuples(
            (0.0, label_padding / info.pos.height()),
            (x_split, 1.0 - label_padding / info.pos.height()),
        );
    }
    fn config(&self) -> &GuiElemCfg {
        &self.config
    }
    fn config_mut(&mut self) -> &mut GuiElemCfg {
        &mut self.config
    }
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut dyn GuiElem> + '_> {
        Box::new([self.panel.elem_mut()].into_iter())
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

struct IconDelete {
    config: GuiElemCfg,
}
impl IconDelete {
    pub fn new(config: GuiElemCfg) -> Self {
        Self { config }
    }
}
impl GuiElem for IconDelete {
    fn draw(&mut self, info: &mut DrawInfo, g: &mut Graphics2D) {
        let thickness = (info.pos.height() * 0.01).max(1.0);
        g.draw_line(
            *info.pos.top_left(),
            *info.pos.bottom_right(),
            thickness,
            Color::GRAY,
        );
        g.draw_line(
            info.pos.top_right(),
            info.pos.bottom_left(),
            thickness,
            Color::GRAY,
        );
    }
    fn config(&self) -> &GuiElemCfg {
        &self.config
    }
    fn config_mut(&mut self) -> &mut GuiElemCfg {
        &mut self.config
    }
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut dyn GuiElem> + '_> {
        Box::new([].into_iter())
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

pub struct EditorForAnyTagAdder<T: From<Event>> {
    config: GuiElemCfg,
    event_sender: std::sync::mpsc::Sender<T>,
    /// `1.0` = collapsed, `self.expand_to` = expanded (shows `c_picker` of height 7-1=6)
    pub open_prog: AnimationController,
    expand_to: f32,
    c_value: TextField,
    c_picker: ScrollBox<Vec<Button<[Label; 1]>>>,
    last_search: String,
}
impl<T: From<Event> + 'static> EditorForAnyTagAdder<T> {
    pub fn new(event_sender: std::sync::mpsc::Sender<T>) -> Self {
        let expand_to = 7.0;
        Self {
            config: GuiElemCfg::default(),
            event_sender,
            open_prog: AnimationController::new(1.0, 1.0, 4.0),
            expand_to,
            c_value: TextField::new(
                GuiElemCfg::default(),
                "tag".to_owned(),
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
    pub fn clear(&mut self, now: Instant) {
        self.last_search = "\n".to_owned();
        self.c_value.c_input.content.text().clear();
        self.open_prog.set_target(now, 1.0);
        self.config_mut().redraw_once();
    }
}
impl<T: From<Event> + 'static> GuiElem for EditorForAnyTagAdder<T> {
    fn draw(&mut self, info: &mut crate::gui::DrawInfo, _g: &mut speedy2d::Graphics2D) {
        let picker_enabled = self.open_prog.value(info.time) > 1.0;
        self.c_picker.config_mut().enabled = picker_enabled;
        if picker_enabled {
            let split = 1.0 / self.open_prog.value(info.time) as f32;
            self.c_value.config_mut().pos = Rectangle::from_tuples((0.0, 0.0), (1.0, split));
            self.c_picker.config_mut().pos = Rectangle::from_tuples((0.0, split), (1.0, 1.0));
        } else {
            self.c_value.config_mut().pos = Rectangle::from_tuples((0.0, 0.0), (1.0, 1.0));
        }

        let search = self.c_value.c_input.content.get_text().to_lowercase();
        let search_changed = self.last_search != search;
        if self.config.redraw() || search_changed {
            *self.c_value.c_input.content.color() = Color::WHITE;
            if search_changed {
                if search.is_empty() {
                    self.open_prog.set_target(info.time, 1.0);
                } else {
                    self.open_prog.set_target(info.time, self.expand_to as f64);
                }
            }
            let mut tags = (info.database.songs().values())
                .flat_map(|s| s.general.tags.iter())
                .chain((info.database.albums().values()).flat_map(|s| s.general.tags.iter()))
                .chain((info.database.artists().values()).flat_map(|s| s.general.tags.iter()))
                .chain(
                    info.database
                        .songs()
                        .values()
                        .flat_map(|s| s.general.tags.iter()),
                )
                .chain(
                    info.database
                        .songs()
                        .values()
                        .flat_map(|s| s.general.tags.iter()),
                )
                .map(|t| t.as_str())
                .filter_map(|tag| {
                    let low = tag.to_lowercase();
                    if low.starts_with(&search) {
                        Some((1u8, tag))
                    } else if low.contains(&search) {
                        Some((2u8, tag))
                    } else {
                        None
                    }
                })
                .collect::<BTreeSet<_>>();
            if !tags.contains(&(0u8, self.c_value.c_input.content.get_text().as_str())) {
                tags.insert((0u8, self.c_value.c_input.content.get_text().as_str()));
            }
            self.c_picker.children = tags
                .iter()
                .map(|tag| {
                    let sender = self.event_sender.clone();
                    Button::new(
                        GuiElemCfg::default(),
                        {
                            let tag = tag.1.to_owned();
                            move |_| {
                                sender.send(Event::AddTag(tag.clone()).into()).unwrap();
                                vec![]
                            }
                        },
                        [Label::new(
                            GuiElemCfg::default(),
                            tag.1.to_owned(),
                            Color::LIGHT_GRAY,
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
        Box::new([self.c_value.elem_mut(), self.c_picker.elem_mut()].into_iter())
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

pub struct SpacerForScrollBox {
    config: GuiElemCfg,
}
impl SpacerForScrollBox {
    pub fn new() -> Self {
        Self {
            config: GuiElemCfg::default(),
        }
    }
}
impl GuiElem for SpacerForScrollBox {
    fn draw(&mut self, _info: &mut DrawInfo, _g: &mut Graphics2D) {}
    fn config(&self) -> &GuiElemCfg {
        &self.config
    }
    fn config_mut(&mut self) -> &mut GuiElemCfg {
        &mut self.config
    }
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut dyn GuiElem> + '_> {
        Box::new([].into_iter())
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

pub struct EditorArtistChooser {
    config: GuiElemCfg,
    event_sender: Arc<dyn Fn(String, ArtistId)>,
    /// `1.0` = collapsed, `self.expand_to` = expanded (shows `c_picker` of height 7-1=6)
    pub open_prog: AnimationController,
    expand_to: f32,
    pub chosen_id: Option<ArtistId>,
    pub c_name: TextField,
    c_picker: ScrollBox<Vec<Button<[Label; 1]>>>,
    last_search: String,
}
impl EditorArtistChooser {
    pub fn new(event_sender: Arc<dyn Fn(String, ArtistId)>) -> Self {
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
    pub fn set_artist(&mut self, name: String, id: Option<u64>, time: Instant) {
        self.chosen_id = id;
        self.last_search = name.to_lowercase();
        self.open_prog.set_target(time, 1.0);
        *self.c_name.c_input.content.text() = name;
        self.config_mut().redraw_once();
    }
    pub fn draw(&mut self, height: Option<&mut f32>, time: Instant) -> bool {
        if let Ok(val) = self.open_prog.update(time, false) {
            if let Some(v) = height {
                *v = ELEM_HEIGHT * val as f32;
            }
            true
        } else {
            false
        }
    }
}
impl GuiElem for EditorArtistChooser {
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
                    let sender = Arc::clone(&self.event_sender);
                    let name = a.0.clone();
                    let id = a.1;
                    Button::new(
                        GuiElemCfg::default(),
                        move |_| {
                            sender(name.clone(), id);
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
