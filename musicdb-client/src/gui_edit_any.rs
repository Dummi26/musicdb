use std::{collections::BTreeSet, time::Instant};

use speedy2d::{
    Graphics2D,
    color::Color,
    dimen::{Vec2, Vector2},
    shape::Rectangle,
};

use crate::{
    gui::{DrawInfo, GuiElem, GuiElemCfg},
    gui_anim::AnimationController,
    gui_base::{Button, Panel, ScrollBox},
    gui_text::{Label, TextField},
};

pub const ELEM_HEIGHT: f32 = 32.0;

pub enum Event {
    RemoveTag(String),
    AddTag(String),
}

pub struct EditorForAnyTagInList {
    config: GuiElemCfg,
    pub tag: String,
    panel: Panel<(Label, Button<[IconDelete; 1]>)>,
}

impl EditorForAnyTagInList {
    pub fn new<T: From<Event> + 'static>(
        tag: String,
        index: usize,
        sender: std::sync::mpsc::Sender<T>,
        config: GuiElemCfg,
    ) -> Self {
        let label = Label::new(
            GuiElemCfg::default(),
            tag.clone(),
            Color::WHITE,
            None,
            Vector2::new(0.0, 0.5),
        );
        let rm_button = Button::new(
            GuiElemCfg::default(),
            {
                let tag = tag.clone();
                move |btn| {
                    btn.disable();
                    sender.send(Event::RemoveTag(tag.clone()).into()).unwrap();
                    vec![]
                }
            },
            [IconDelete::new(GuiElemCfg::default())],
        );
        let panel = Panel::with_background(
            GuiElemCfg::default(),
            (label, rm_button),
            match index % 2 {
                1 => Color::from_rgba(0.0, 1.0, 0.0, 0.1),
                _ => Color::from_rgba(0.0, 0.0, 1.0, 0.14),
            },
        );
        Self { config, tag, panel }
    }
    fn row(&self) -> &(Label, Button<[IconDelete; 1]>) {
        &self.panel.children
    }
    fn row_mut(&mut self) -> &mut (Label, Button<[IconDelete; 1]>) {
        &mut self.panel.children
    }
    fn label(&self) -> &Label {
        &self.panel.children.0
    }
    fn rm_button(&self) -> &Button<[IconDelete; 1]> {
        &self.panel.children.1
    }
    fn label_mut(&mut self) -> &mut Label {
        &mut self.panel.children.0
    }
    fn rm_button_mut(&mut self) -> &mut Button<[IconDelete; 1]> {
        &mut self.panel.children.1
    }
}

impl GuiElem for EditorForAnyTagInList {
    fn draw(&mut self, info: &mut DrawInfo, g: &mut Graphics2D) {
        let rm_button_size = (info.pos.height() * 0.8).min(info.pos.width() * 0.33);
        let rm_button_padding = (info.pos.height() - rm_button_size) / 2.0;
        let label_padding = info.pos.height() * 0.05;
        let x_split = (info.pos.width() - rm_button_size) / info.pos.width();
        self.rm_button_mut().config_mut().pos = Rectangle::from_tuples(
            (x_split, rm_button_padding / info.pos.height()),
            (1.0, 1.0 - rm_button_padding / info.pos.height()),
        );
        self.label_mut().config_mut().pos = Rectangle::from_tuples(
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
            let mut tags = info
                .database
                .songs()
                .values()
                .flat_map(|s| s.general.tags.iter())
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
                .filter(|tag| tag.to_lowercase().contains(&search))
                .cloned()
                .collect::<BTreeSet<_>>();
            if !tags.contains(self.c_value.c_input.content.get_text()) {
                tags.insert(self.c_value.c_input.content.get_text().clone());
            }
            self.c_picker.children = tags
                .iter()
                .map(|tag| {
                    let sender = self.event_sender.clone();
                    Button::new(
                        GuiElemCfg::default(),
                        {
                            let tag = tag.clone();
                            move |_| {
                                sender.send(Event::AddTag(tag.clone()).into()).unwrap();
                                vec![]
                            }
                        },
                        [Label::new(
                            GuiElemCfg::default(),
                            tag.clone(),
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
