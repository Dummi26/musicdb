use std::time::{Duration, Instant};

use speedy2d::{color::Color, dimen::Vec2, shape::Rectangle, Graphics2D};

use crate::{
    gui::{DrawInfo, GuiAction, GuiElem, GuiElemCfg, GuiElemTrait},
    gui_base::{Button, Panel},
    gui_library::LibraryBrowser,
    gui_playback::{CurrentSong, PlayPauseToggle},
    gui_queue::QueueViewer,
    gui_settings::Settings,
    gui_text::Label,
};

/*

The root gui element.
Contains the Library, Queue, StatusBar, and sometimes Settings elements.
Resizes these elements to show/hide the settings menu and to smoothly switch to/from idle mode.

*/

/// calculates f(p), where f(x) = 3x^2 - 2x^3, because
/// f(0) = 0
/// f(0.5) = 0.5
/// f(1) = 1
/// f'(0) = f'(1) = 0
/// -> smooth animation, fast to calculate
pub fn transition(p: f32) -> f32 {
    3.0 * p * p - 2.0 * p * p * p
}

#[derive(Clone)]
pub struct GuiScreen {
    config: GuiElemCfg,
    /// 0: StatusBar / Idle display
    /// 1: Settings
    /// 2: Panel for Main view
    children: Vec<GuiElem>,
    pub idle: (bool, Option<Instant>),
    pub settings: (bool, Option<Instant>),
    pub last_interaction: Instant,
    idle_timeout: Option<f64>,
    pub prev_mouse_pos: Vec2,
}
impl GuiScreen {
    pub fn new(
        config: GuiElemCfg,
        line_height: f32,
        scroll_sensitivity_pixels: f64,
        scroll_sensitivity_lines: f64,
        scroll_sensitivity_pages: f64,
    ) -> Self {
        Self {
            config: config.w_keyboard_watch(),
            children: vec![
                GuiElem::new(StatusBar::new(
                    GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.9), (1.0, 1.0))),
                    true,
                )),
                GuiElem::new(Settings::new(
                    GuiElemCfg::default().disabled(),
                    line_height,
                    scroll_sensitivity_pixels,
                    scroll_sensitivity_lines,
                    scroll_sensitivity_pages,
                )),
                GuiElem::new(Panel::new(
                    GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.0), (1.0, 0.9))),
                    vec![
                        GuiElem::new(Button::new(
                            GuiElemCfg::at(Rectangle::from_tuples((0.5, 0.0), (0.75, 0.1))),
                            |_| vec![GuiAction::OpenSettings(true)],
                            vec![GuiElem::new(Label::new(
                                GuiElemCfg::default(),
                                "Settings".to_string(),
                                Color::WHITE,
                                None,
                                Vec2::new(0.5, 0.5),
                            ))],
                        )),
                        GuiElem::new(Button::new(
                            GuiElemCfg::at(Rectangle::from_tuples((0.75, 0.0), (1.0, 0.1))),
                            |_| vec![GuiAction::Exit],
                            vec![GuiElem::new(Label::new(
                                GuiElemCfg::default(),
                                "Exit".to_string(),
                                Color::WHITE,
                                None,
                                Vec2::new(0.5, 0.5),
                            ))],
                        )),
                        GuiElem::new(LibraryBrowser::new(GuiElemCfg::at(Rectangle::from_tuples(
                            (0.0, 0.0),
                            (0.5, 1.0),
                        )))),
                        GuiElem::new(QueueViewer::new(GuiElemCfg::at(Rectangle::from_tuples(
                            (0.5, 0.1),
                            (1.0, 1.0),
                        )))),
                    ],
                )),
            ],
            idle: (false, None),
            settings: (false, None),
            last_interaction: Instant::now(),
            idle_timeout: Some(60.0),
            prev_mouse_pos: Vec2::ZERO,
        }
    }
    fn get_prog(v: &mut (bool, Option<Instant>), seconds: f32) -> f32 {
        if let Some(since) = &mut v.1 {
            let prog = since.elapsed().as_secs_f32() / seconds;
            if prog >= 1.0 {
                v.1 = None;
                if v.0 {
                    1.0
                } else {
                    0.0
                }
            } else {
                if v.0 {
                    prog
                } else {
                    1.0 - prog
                }
            }
        } else if v.0 {
            1.0
        } else {
            0.0
        }
    }
    fn not_idle(&mut self) {
        self.last_interaction = Instant::now();
        if self.idle.0 {
            self.idle = (false, Some(Instant::now()));
        }
    }
    fn idle_check(&mut self) {
        if !self.idle.0 {
            if let Some(dur) = &self.idle_timeout {
                if self.last_interaction.elapsed().as_secs_f64() > *dur {
                    self.idle = (true, Some(Instant::now()));
                }
            }
        }
    }
}
impl GuiElemTrait for GuiScreen {
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
    fn key_watch(
        &mut self,
        _modifiers: speedy2d::window::ModifiersState,
        _down: bool,
        _key: Option<speedy2d::window::VirtualKeyCode>,
        _scan: speedy2d::window::KeyScancode,
    ) -> Vec<GuiAction> {
        self.not_idle();
        vec![]
    }
    fn draw(&mut self, info: &mut DrawInfo, g: &mut Graphics2D) {
        // idle stuff
        if self.prev_mouse_pos != info.mouse_pos {
            self.prev_mouse_pos = info.mouse_pos;
            self.not_idle();
        } else if !self.idle.0 && self.config.pixel_pos.size() != info.pos.size() {
            // resizing prevents idle, but doesn't un-idle
            self.not_idle();
        }
        self.idle_check();
        // request_redraw for animations
        if self.idle.1.is_some() | self.settings.1.is_some() {
            if let Some(h) = &info.helper {
                h.request_redraw()
            }
        }
        // animations: idle
        if self.idle.1.is_some() {
            let seconds = if self.idle.0 { 2.0 } else { 0.5 };
            let p1 = Self::get_prog(&mut self.idle, seconds);
            if !self.idle.0 || self.idle.1.is_none() {
                if let Some(h) = &info.helper {
                    h.set_cursor_visible(!self.idle.0);
                    for el in self.children.iter_mut().skip(1) {
                        el.inner.config_mut().enabled = !self.idle.0;
                    }
                }
            }
            let p = transition(p1);
            self.children[0].inner.config_mut().pos =
                Rectangle::from_tuples((0.0, 0.9 - 0.9 * p), (1.0, 1.0));
            self.children[0]
                .inner
                .any_mut()
                .downcast_mut::<StatusBar>()
                .unwrap()
                .idle_mode = p1;
        }
        // animations: settings
        if self.settings.1.is_some() {
            let p1 = Self::get_prog(&mut self.settings, 0.3);
            let p = transition(p1);
            let cfg = self.children[1].inner.config_mut();
            cfg.enabled = p > 0.0;
            cfg.pos = Rectangle::from_tuples((0.0, 0.9 - 0.9 * p), (1.0, 0.9));
        }
        // set idle timeout (only when settings are open)
        if self.settings.0 || self.settings.1.is_some() {
            self.idle_timeout = self.children[1]
                .inner
                .any()
                .downcast_ref::<Settings>()
                .unwrap()
                .get_timeout_val();
        }
    }
}

#[derive(Clone)]
pub struct StatusBar {
    config: GuiElemCfg,
    children: Vec<GuiElem>,
    idle_mode: f32,
}
impl StatusBar {
    pub fn new(config: GuiElemCfg, playing: bool) -> Self {
        Self {
            config,
            children: vec![
                GuiElem::new(CurrentSong::new(GuiElemCfg::at(Rectangle::new(
                    Vec2::ZERO,
                    Vec2::new(0.8, 1.0),
                )))),
                GuiElem::new(PlayPauseToggle::new(
                    GuiElemCfg::at(Rectangle::from_tuples((0.85, 0.0), (0.95, 1.0))),
                    false,
                )),
                GuiElem::new(Panel::with_background(
                    GuiElemCfg::default(),
                    vec![],
                    Color::BLACK,
                )),
            ],
            idle_mode: 0.0,
        }
    }
}
impl GuiElemTrait for StatusBar {
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
    fn draw(&mut self, info: &mut DrawInfo, g: &mut Graphics2D) {
        if self.idle_mode < 1.0 {
            g.draw_line(
                info.pos.top_left(),
                info.pos.top_right(),
                2.0,
                Color::from_rgba(1.0, 1.0, 1.0, 1.0 - self.idle_mode),
            );
        }
    }
}
