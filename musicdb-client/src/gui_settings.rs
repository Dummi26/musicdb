use speedy2d::{color::Color, dimen::Vec2, shape::Rectangle, Graphics2D};

use crate::{
    gui::{DrawInfo, GuiAction, GuiElemCfg, GuiElemTrait},
    gui_base::{Button, Panel, ScrollBox, Slider},
    gui_text::Label,
};

pub struct Settings {
    pub config: GuiElemCfg,
    pub children: Vec<Box<dyn GuiElemTrait>>,
}
impl Settings {
    pub fn new(
        mut config: GuiElemCfg,
        line_height: f32,
        scroll_sensitivity_pixels: f64,
        scroll_sensitivity_lines: f64,
        scroll_sensitivity_pages: f64,
    ) -> Self {
        config.redraw = true;
        Self {
            config,
            children: vec![
                Box::new(ScrollBox::new(
                    GuiElemCfg::default(),
                    crate::gui_base::ScrollBoxSizeUnit::Pixels,
                    vec![
                        (
                            Box::new(Button::new(
                                GuiElemCfg::at(Rectangle::from_tuples((0.75, 0.0), (1.0, 1.0))),
                                |btn| vec![GuiAction::OpenSettings(false)],
                                vec![Box::new(Label::new(
                                    GuiElemCfg::default(),
                                    "Back".to_string(),
                                    Color::WHITE,
                                    None,
                                    Vec2::new(0.5, 0.5),
                                ))],
                            )),
                            0.0,
                        ),
                        (
                            Box::new(Panel::new(
                                GuiElemCfg::default(),
                                vec![
                                    Box::new(Label::new(
                                        GuiElemCfg::at(Rectangle::from_tuples(
                                            (0.0, 0.0),
                                            (0.33, 1.0),
                                        )),
                                        "Settings panel opacity".to_string(),
                                        Color::WHITE,
                                        None,
                                        Vec2::new(0.9, 0.5),
                                    )),
                                    Box::new({
                                        let mut s = Slider::new_labeled(
                                            GuiElemCfg::at(Rectangle::from_tuples(
                                                (0.33, 0.0),
                                                (1.0, 1.0),
                                            )),
                                            0.0,
                                            1.0,
                                            1.0,
                                            |slider, label, _info| {
                                                if slider.val_changed() {
                                                    *label.content.text() =
                                                        format!("{:.0}%", slider.val * 100.0);
                                                }
                                            },
                                        );
                                        s.val_changed_subs.push(false);
                                        s
                                    }),
                                ],
                            )),
                            0.0,
                        ),
                        (
                            Box::new(Panel::new(
                                GuiElemCfg::default(),
                                vec![
                                    Box::new(Label::new(
                                        GuiElemCfg::at(Rectangle::from_tuples(
                                            (0.0, 0.0),
                                            (0.33, 1.0),
                                        )),
                                        "Line Height / Text Size".to_string(),
                                        Color::WHITE,
                                        None,
                                        Vec2::new(0.9, 0.5),
                                    )),
                                    Box::new(Slider::new_labeled(
                                        GuiElemCfg::at(Rectangle::from_tuples(
                                            (0.33, 0.0),
                                            (1.0, 1.0),
                                        )),
                                        16.0,
                                        80.0,
                                        line_height as _,
                                        |slider, label, info| {
                                            if slider.val_changed() {
                                                *label.content.text() =
                                                    format!("line height: {:.0}", slider.val);
                                                let h = slider.val as _;
                                                info.actions.push(GuiAction::SetLineHeight(h));
                                            }
                                        },
                                    )),
                                ],
                            )),
                            0.0,
                        ),
                        (
                            Box::new(Panel::new(
                                GuiElemCfg::default(),
                                vec![
                                    Box::new(Label::new(
                                        GuiElemCfg::at(Rectangle::from_tuples(
                                            (0.0, 0.0),
                                            (0.33, 1.0),
                                        )),
                                        "Scroll Sensitivity".to_string(),
                                        Color::WHITE,
                                        None,
                                        Vec2::new(0.9, 0.5),
                                    )),
                                    Box::new(Slider::new_labeled(
                                        GuiElemCfg::at(Rectangle::from_tuples(
                                            (0.33, 0.0),
                                            (1.0, 1.0),
                                        )),
                                        0.0,
                                        12.0,
                                        scroll_sensitivity_lines,
                                        |slider, label, info| {
                                            if slider.val_changed() {
                                                *label.content.text() =
                                                    format!("{:.1}", slider.val);
                                                let h = slider.val as _;
                                                info.actions.push(GuiAction::Do(Box::new(
                                                    move |gui| gui.scroll_lines_multiplier = h,
                                                )));
                                            }
                                        },
                                    )),
                                ],
                            )),
                            0.0,
                        ),
                        (
                            Box::new(Panel::new(
                                GuiElemCfg::default(),
                                vec![
                                    Box::new(Label::new(
                                        GuiElemCfg::at(Rectangle::from_tuples(
                                            (0.0, 0.0),
                                            (0.33, 1.0),
                                        )),
                                        "Idle time".to_string(),
                                        Color::WHITE,
                                        None,
                                        Vec2::new(0.9, 0.5),
                                    )),
                                    Box::new(Slider::new_labeled(
                                        GuiElemCfg::at(Rectangle::from_tuples(
                                            (0.33, 0.0),
                                            (1.0, 1.0),
                                        )),
                                        0.0,
                                        (60.0f64 * 60.0 * 6.0).sqrt(),
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
                                                    if hours == 0
                                                        && minutes < 10
                                                        && (seconds > 0 || minutes == 0)
                                                    {
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
                                                info.actions.push(GuiAction::Do(Box::new(
                                                    move |gui| gui.scroll_lines_multiplier = h,
                                                )));
                                            }
                                        },
                                    )),
                                ],
                            )),
                            0.0,
                        ),
                    ],
                )),
                Box::new(Panel::with_background(
                    GuiElemCfg::default().w_mouse(),
                    vec![],
                    Color::BLACK,
                )),
            ],
        }
    }
    pub fn get_timeout_val(&self) -> Option<f64> {
        let v = self.children[0]
            .any()
            .downcast_ref::<ScrollBox>()
            .unwrap()
            .children[4]
            .0
            .any()
            .downcast_ref::<Panel>()
            .unwrap()
            .children[1]
            .any()
            .downcast_ref::<Slider>()
            .unwrap()
            .val;
        if v > 0.0 {
            Some(v * v)
        } else {
            None
        }
    }
}
impl GuiElemTrait for Settings {
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
    fn draw(&mut self, info: &mut DrawInfo, _g: &mut Graphics2D) {
        let (rest, background) = self.children.split_at_mut(1);
        let scrollbox = rest[0].any_mut().downcast_mut::<ScrollBox>().unwrap();
        let settings_opacity_slider = scrollbox.children[1]
            .0
            .any_mut()
            .downcast_mut::<Panel>()
            .unwrap()
            .children[1]
            .any_mut()
            .downcast_mut::<Slider>()
            .unwrap();
        if settings_opacity_slider.val_changed_subs[0] {
            settings_opacity_slider.val_changed_subs[0] = false;
            let color = background[0]
                .any_mut()
                .downcast_mut::<Panel>()
                .unwrap()
                .background
                .as_mut()
                .unwrap();
            *color = Color::from_rgba(
                color.r(),
                color.g(),
                color.b(),
                settings_opacity_slider.val as _,
            );
        }
        if self.config.redraw {
            self.config.redraw = false;
            for (i, (_, h)) in scrollbox.children.iter_mut().enumerate() {
                *h = if i == 0 {
                    info.line_height * 2.0
                } else {
                    info.line_height
                };
            }
        }
    }
}
