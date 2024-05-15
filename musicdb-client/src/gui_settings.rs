use std::sync::{atomic::AtomicBool, Arc, Mutex};

use musicdb_lib::server::Command;
use speedy2d::{
    color::Color,
    dimen::Vec2,
    shape::Rectangle,
    window::{KeyScancode, ModifiersState, MouseButton, VirtualKeyCode},
    Graphics2D,
};

use crate::{
    gui::{
        DrawInfo, GuiAction, GuiElem, GuiElemCfg, GuiElemChildren, KeyAction, KeyActionId,
        KeyBinding,
    },
    gui_base::{Button, Panel, ScrollBox, Slider},
    gui_text::{AdvancedContent, AdvancedLabel, Content, Label},
};

pub struct Settings {
    pub config: GuiElemCfg,
    pub c_scroll_box: ScrollBox<SettingsContent>,
    c_background: Panel<()>,
}
impl Settings {
    pub fn new(
        mut config: GuiElemCfg,
        no_animations: bool,
        line_height: f32,
        scroll_sensitivity_pixels: f64,
        scroll_sensitivity_lines: f64,
        scroll_sensitivity_pages: f64,
    ) -> Self {
        config.redraw = true;
        Self {
            config,
            c_scroll_box: ScrollBox::new(
                GuiElemCfg::default(),
                crate::gui_base::ScrollBoxSizeUnit::Pixels,
                SettingsContent::new(
                    no_animations,
                    line_height,
                    scroll_sensitivity_pixels,
                    scroll_sensitivity_lines,
                    scroll_sensitivity_pages,
                ),
                vec![],
                0.0,
            ),
            c_background: Panel::with_background(GuiElemCfg::default().w_mouse(), (), Color::BLACK),
        }
    }
    pub fn get_timeout_val(&self) -> Option<f64> {
        let v = self.c_scroll_box.children.idle_time.children.1.val;
        if v > 0.0 {
            Some(v * v)
        } else {
            None
        }
    }
}
pub struct SettingsContent {
    pub back_button: Button<[Label; 1]>,
    pub opacity: Panel<(Label, Slider)>,
    pub performance_toggle: Panel<(Label, Button<[Label; 1]>)>,
    pub line_height: Panel<(Label, Slider)>,
    pub scroll_sensitivity: Panel<(Label, Slider)>,
    pub idle_time: Panel<(Label, Slider)>,
    pub save_button: Button<[Label; 1]>,
    pub add_new_songs_button: Button<[Label; 1]>,
    pub keybinds: Vec<Panel<(AdvancedLabel, KeybindInput)>>,
    pub keybinds_should_be_updated: Arc<AtomicBool>,
    pub keybinds_updated: bool,
    pub keybinds_updater: Arc<Mutex<Option<Vec<Panel<(AdvancedLabel, KeybindInput)>>>>>,
}
impl GuiElemChildren for SettingsContent {
    fn iter(&mut self) -> Box<dyn Iterator<Item = &mut dyn GuiElem> + '_> {
        Box::new(
            [
                self.back_button.elem_mut(),
                self.opacity.elem_mut(),
                self.performance_toggle.elem_mut(),
                self.line_height.elem_mut(),
                self.scroll_sensitivity.elem_mut(),
                self.idle_time.elem_mut(),
                self.save_button.elem_mut(),
                self.add_new_songs_button.elem_mut(),
            ]
            .into_iter()
            .chain(self.keybinds.iter_mut().map(|v| v.elem_mut())),
        )
    }
    fn len(&self) -> usize {
        8 + self.keybinds.len()
    }
}
pub struct KeybindInput {
    config: GuiElemCfg,
    c_label: Label,
    id: KeyActionId,
    changing: bool,
    keybinds_should_be_updated: Arc<AtomicBool>,
    has_keyboard_focus: bool,
}
impl KeybindInput {
    pub fn new(
        config: GuiElemCfg,
        id: KeyActionId,
        _action: &KeyAction,
        binding: Option<KeyBinding>,
        keybinds_should_be_updated: Arc<AtomicBool>,
    ) -> Self {
        Self {
            config: config.w_keyboard_focus().w_mouse(),
            c_label: Label::new(
                GuiElemCfg::default(),
                if let Some(b) = &binding {
                    format!(
                        "{}{}{}{}{:?}",
                        if b.get_ctrl() { "Ctrl+" } else { "" },
                        if b.get_alt() { "Alt+" } else { "" },
                        if b.get_shift() { "Shift+" } else { "" },
                        if b.get_meta() { "Meta+" } else { "" },
                        b.key,
                    )
                } else {
                    format!("")
                },
                Color::WHITE,
                None,
                Vec2::new(0.5, 0.5),
            ),
            id,
            changing: false,
            keybinds_should_be_updated,
            has_keyboard_focus: false,
        }
    }
}
impl GuiElem for KeybindInput {
    fn mouse_pressed(&mut self, button: MouseButton) -> Vec<GuiAction> {
        if let MouseButton::Left = button {
            if !self.has_keyboard_focus {
                self.changing = true;
                self.config.request_keyboard_focus = true;
            } else {
                self.changing = false;
            }
            vec![GuiAction::ResetKeyboardFocus]
        } else {
            vec![]
        }
    }
    fn key_focus(
        &mut self,
        modifiers: ModifiersState,
        down: bool,
        key: Option<VirtualKeyCode>,
        _scan: KeyScancode,
    ) -> Vec<GuiAction> {
        if self.changing && down {
            if let Some(key) = key {
                if !matches!(
                    key,
                    VirtualKeyCode::LControl
                        | VirtualKeyCode::RControl
                        | VirtualKeyCode::LShift
                        | VirtualKeyCode::RShift
                        | VirtualKeyCode::LAlt
                        | VirtualKeyCode::RAlt
                        | VirtualKeyCode::LWin
                        | VirtualKeyCode::RWin
                ) {
                    self.changing = false;
                    let bind = KeyBinding::new(&modifiers, key);
                    self.keybinds_should_be_updated
                        .store(true, std::sync::atomic::Ordering::Relaxed);
                    vec![
                        GuiAction::SetKeybind(self.id, Some(bind)),
                        GuiAction::ResetKeyboardFocus,
                    ]
                } else {
                    vec![]
                }
            } else {
                vec![]
            }
        } else {
            vec![]
        }
    }
    fn draw(&mut self, info: &mut DrawInfo, g: &mut Graphics2D) {
        self.has_keyboard_focus = info.has_keyboard_focus;
        if info.has_keyboard_focus && self.changing {
            let half_width = 2.0;
            let thickness = 2.0 * half_width;
            g.draw_line(
                Vec2::new(info.pos.top_left().x, info.pos.top_left().y + half_width),
                Vec2::new(
                    info.pos.bottom_right().x,
                    info.pos.top_left().y + half_width,
                ),
                thickness,
                Color::WHITE,
            );
            g.draw_line(
                Vec2::new(
                    info.pos.top_left().x,
                    info.pos.bottom_right().y - half_width,
                ),
                Vec2::new(
                    info.pos.bottom_right().x,
                    info.pos.bottom_right().y - half_width,
                ),
                thickness,
                Color::WHITE,
            );
            g.draw_line(
                Vec2::new(info.pos.top_left().x + half_width, info.pos.top_left().y),
                Vec2::new(
                    info.pos.top_left().x + half_width,
                    info.pos.bottom_right().y,
                ),
                thickness,
                Color::WHITE,
            );
            g.draw_line(
                Vec2::new(
                    info.pos.bottom_right().x - half_width,
                    info.pos.top_left().y,
                ),
                Vec2::new(
                    info.pos.bottom_right().x - half_width,
                    info.pos.bottom_right().y,
                ),
                thickness,
                Color::WHITE,
            );
        }
    }
    fn config(&self) -> &GuiElemCfg {
        &self.config
    }
    fn config_mut(&mut self) -> &mut GuiElemCfg {
        &mut self.config
    }
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut dyn GuiElem> + '_> {
        Box::new([self.c_label.elem_mut()].into_iter())
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
impl SettingsContent {
    pub fn new(
        high_performance: bool,
        line_height: f32,
        _scroll_sensitivity_pixels: f64,
        scroll_sensitivity_lines: f64,
        _scroll_sensitivity_pages: f64,
    ) -> Self {
        Self {
            back_button: Button::new(
                GuiElemCfg::at(Rectangle::from_tuples((0.75, 0.0), (1.0, 1.0))),
                |_| vec![GuiAction::OpenSettings(false)],
                [Label::new(
                    GuiElemCfg::default(),
                    "Back".to_string(),
                    Color::WHITE,
                    None,
                    Vec2::new(0.5, 0.5),
                )],
            ),
            opacity: Panel::new(
                GuiElemCfg::default(),
                (
                    Label::new(
                        GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.0), (0.33, 1.0))),
                        "Settings panel opacity".to_string(),
                        Color::WHITE,
                        None,
                        Vec2::new(0.9, 0.5),
                    ),
                    {
                        let mut s = Slider::new_labeled(
                            GuiElemCfg::at(Rectangle::from_tuples((0.33, 0.0), (1.0, 1.0))),
                            0.0,
                            1.0,
                            1.0,
                            |slider, label, _info| {
                                if slider.val_changed() {
                                    *label.content.text() = format!("{:.0}%", slider.val * 100.0);
                                }
                            },
                        );
                        s.val_changed_subs.push(false);
                        s
                    },
                ),
            ),
            performance_toggle: Panel::new(
                GuiElemCfg::default(),
                (
                    Label::new(
                        GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.0), (0.33, 1.0))),
                        "Power Saver".to_string(),
                        Color::WHITE,
                        None,
                        Vec2::new(1.0, 0.5),
                    ),
                    Button::new(
                        GuiElemCfg::at(Rectangle::from_tuples((0.75, 0.0), (1.0, 1.0))),
                        |b| {
                            let text = b.children[0].content.text();
                            let ad = if text.starts_with("On") {
                                *text = "Off".to_string();
                                false
                            } else {
                                *text = "On".to_string();
                                true
                            };
                            vec![GuiAction::SetHighPerformance(ad)]
                        },
                        [Label::new(
                            GuiElemCfg::default(),
                            if high_performance { "On" } else { "Off" }.to_string(),
                            Color::WHITE,
                            None,
                            Vec2::new(0.5, 0.5),
                        )],
                    ),
                ),
            ),
            line_height: Panel::new(
                GuiElemCfg::default(),
                (
                    Label::new(
                        GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.0), (0.33, 1.0))),
                        "Line Height / Text Size".to_string(),
                        Color::WHITE,
                        None,
                        Vec2::new(0.9, 0.5),
                    ),
                    Slider::new_labeled(
                        GuiElemCfg::at(Rectangle::from_tuples((0.33, 0.0), (1.0, 1.0))),
                        16.0,
                        80.0,
                        line_height as _,
                        |slider, label, info| {
                            if slider.val_changed() {
                                *label.content.text() = format!("line height: {:.0}", slider.val);
                                let h = slider.val as _;
                                info.actions.push(GuiAction::SetLineHeight(h));
                            }
                        },
                    ),
                ),
            ),
            scroll_sensitivity: Panel::new(
                GuiElemCfg::default(),
                (
                    Label::new(
                        GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.0), (0.33, 1.0))),
                        "Scroll Sensitivity".to_string(),
                        Color::WHITE,
                        None,
                        Vec2::new(0.9, 0.5),
                    ),
                    Slider::new_labeled(
                        GuiElemCfg::at(Rectangle::from_tuples((0.33, 0.0), (1.0, 1.0))),
                        0.0,
                        12.0,
                        scroll_sensitivity_lines,
                        |slider, label, info| {
                            if slider.val_changed() {
                                *label.content.text() = format!("{:.1}", slider.val);
                                let h = slider.val as _;
                                info.actions.push(GuiAction::Do(Box::new(move |gui| {
                                    gui.scroll_lines_multiplier = h
                                })));
                            }
                        },
                    ),
                ),
            ),
            idle_time: Panel::new(
                GuiElemCfg::default(),
                (
                    Label::new(
                        GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.0), (0.33, 1.0))),
                        "Idle time".to_string(),
                        Color::WHITE,
                        None,
                        Vec2::new(0.9, 0.5),
                    ),
                    Slider::new_labeled(
                        GuiElemCfg::at(Rectangle::from_tuples((0.33, 0.0), (1.0, 1.0))),
                        0.0,
                        (60.0f64 * 60.0).sqrt(),
                        60.0f64.sqrt(),
                        |slider, label, info| {
                            if slider.val_changed() {
                                *label.content.text() = if slider.val > 0.0 {
                                    let mut s = String::new();
                                    let seconds = (slider.val * slider.val) as u64;
                                    let hours = seconds / 3600;
                                    let seconds = seconds % 3600;
                                    let minutes = seconds / 60;
                                    let seconds = seconds % 60;
                                    if hours > 0 {
                                        s = hours.to_string();
                                        s.push_str("h ");
                                    }
                                    if minutes > 0 || hours > 0 && seconds > 0 {
                                        s.push_str(&minutes.to_string());
                                        s.push_str("m ");
                                    }
                                    if hours == 0 && minutes < 10 && (seconds > 0 || minutes == 0) {
                                        s.push_str(&seconds.to_string());
                                        s.push_str("s");
                                    } else if s.ends_with(" ") {
                                        s.pop();
                                    }
                                    s
                                } else {
                                    "no timeout".to_string()
                                }
                            };
                            let h = slider.val as _;
                            if slider.val_changed() {
                                info.actions.push(GuiAction::Do(Box::new(move |gui| {
                                    gui.scroll_lines_multiplier = h
                                })));
                            }
                        },
                    ),
                ),
            ),
            save_button: Button::new(
                GuiElemCfg::default(),
                |_| vec![GuiAction::SendToServer(Command::Save)],
                [Label::new(
                    GuiElemCfg::default(),
                    "Server: Save Changes".to_string(),
                    Color::WHITE,
                    None,
                    Vec2::new(0.5, 0.5),
                )],
            ),
            add_new_songs_button: Button::new(
                GuiElemCfg::default(),
                |_| vec![GuiAction::OpenAddSongsMenu],
                [Label::new(
                    GuiElemCfg::default(),
                    "search for new songs".to_string(),
                    Color::WHITE,
                    None,
                    Vec2::new(0.5, 0.5),
                )],
            ),
            keybinds: vec![],
            keybinds_should_be_updated: Arc::new(AtomicBool::new(true)),
            keybinds_updated: false,
            keybinds_updater: Arc::new(Mutex::new(None)),
        }
    }
    pub fn draw(&mut self, info: &mut DrawInfo) -> bool {
        if !self.keybinds_updated
            && self
                .keybinds_should_be_updated
                .load(std::sync::atomic::Ordering::Relaxed)
        {
            self.keybinds_updated = true;
            self.keybinds_should_be_updated
                .store(false, std::sync::atomic::Ordering::Relaxed);
            let updater = Arc::clone(&self.keybinds_updater);
            let keybinds_should_be_updated = Arc::clone(&self.keybinds_should_be_updated);
            info.actions.push(GuiAction::Do(Box::new(move |gui| {
                *updater.lock().unwrap() =
                    Some(build_keybind_elems(gui, &keybinds_should_be_updated))
            })))
        }
        if self.keybinds_updated {
            if let Some(keybinds) = self.keybinds_updater.lock().unwrap().take() {
                self.keybinds_updated = false;
                self.keybinds = keybinds;
                if let Some(h) = &info.helper {
                    h.request_redraw();
                }
                true
            } else {
                false
            }
        } else {
            false
        }
    }
}
impl GuiElem for Settings {
    fn config(&self) -> &GuiElemCfg {
        &self.config
    }
    fn config_mut(&mut self) -> &mut GuiElemCfg {
        &mut self.config
    }
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut dyn GuiElem> + '_> {
        Box::new([self.c_scroll_box.elem_mut(), self.c_background.elem_mut()].into_iter())
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
    fn draw(&mut self, info: &mut DrawInfo, _g: &mut Graphics2D) {
        if self.c_scroll_box.children.draw(info) {
            self.c_scroll_box.config_mut().redraw = true;
        }
        let scrollbox = &mut self.c_scroll_box;
        let background = &mut self.c_background;
        let settings_opacity_slider = &mut scrollbox.children.opacity.children.1;
        if settings_opacity_slider.val_changed_subs[0] {
            settings_opacity_slider.val_changed_subs[0] = false;
            let color = background.background.as_mut().unwrap();
            *color = Color::from_rgba(
                color.r(),
                color.g(),
                color.b(),
                settings_opacity_slider.val as _,
            );
        }
        if self.config.redraw {
            self.config.redraw = false;
            scrollbox.config_mut().redraw = true;
            if scrollbox.children_heights.len() == scrollbox.children.len() {
                for (i, h) in scrollbox.children_heights.iter_mut().enumerate() {
                    *h = if i == 0 || i >= 8 {
                        info.line_height * 2.0
                    } else {
                        info.line_height
                    };
                }
            } else {
                // try again next frame (scrollbox will autofill the children_heights vec)
                self.config.redraw = true;
            }
        }
    }
}

pub fn build_keybind_elems(
    gui: &crate::gui::Gui,
    keybinds_should_be_updated: &Arc<AtomicBool>,
) -> Vec<Panel<(AdvancedLabel, KeybindInput)>> {
    let split = 0.75;
    let mut list = gui
        .key_actions
        .iter()
        .map(|(a, b)| (a, b, None))
        .collect::<Vec<_>>();
    for (binding, action) in gui.keybinds.iter() {
        list[action.get_index()].2 = Some(*binding);
    }
    list.sort_by_key(|(_, v, _)| &v.category);
    list.into_iter()
        .map(|(id, v, binding)| {
            Panel::new(
                GuiElemCfg::default(),
                (
                    AdvancedLabel::new(
                        GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.0), (split, 1.0))),
                        Vec2::new(1.0, 0.5),
                        vec![
                            vec![(
                                AdvancedContent::Text(Content::new(
                                    format!("{}", v.title),
                                    if v.enabled {
                                        Color::WHITE
                                    } else {
                                        Color::LIGHT_GRAY
                                    },
                                )),
                                1.0,
                                1.0,
                            )],
                            vec![(
                                AdvancedContent::Text(Content::new(
                                    format!("{}", v.description),
                                    if v.enabled {
                                        Color::LIGHT_GRAY
                                    } else {
                                        Color::GRAY
                                    },
                                )),
                                0.5,
                                1.0,
                            )],
                        ],
                    ),
                    KeybindInput::new(
                        GuiElemCfg::at(Rectangle::from_tuples((split, 0.0), (1.0, 1.0))),
                        id,
                        v,
                        binding,
                        Arc::clone(keybinds_should_be_updated),
                    ),
                ),
            )
        })
        .collect()
}
