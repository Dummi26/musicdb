use speedy2d::{color::Color, dimen::Vec2, shape::Rectangle, Graphics2D};

use crate::{
    gui::{DrawInfo, GuiAction, GuiElem, GuiElemCfg, GuiElemChildren},
    gui_base::{Button, Panel, ScrollBox, Slider},
    gui_text::Label,
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
            ]
            .into_iter(),
        )
    }
    fn len(&self) -> usize {
        6
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
                    *h = if i == 0 {
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
