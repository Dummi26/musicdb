use std::time::Instant;

use musicdb_lib::{data::queue::QueueContent, server::Command};
use speedy2d::{color::Color, dimen::Vec2, shape::Rectangle, window::VirtualKeyCode, Graphics2D};

use crate::{
    gui::{DrawInfo, GuiAction, GuiElem, GuiElemCfg},
    gui_anim::AnimationController,
    gui_base::{Button, Panel},
    gui_idle_display::IdleDisplay,
    gui_library::LibraryBrowser,
    gui_notif::NotifOverlay,
    gui_queue::QueueViewer,
    gui_settings::Settings,
    gui_statusbar::StatusBar,
    gui_text::Label,
    gui_wrappers::Hotkey,
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

pub struct GuiScreen {
    config: GuiElemCfg,
    c_notif_overlay: NotifOverlay,
    c_idle_display: IdleDisplay,
    c_status_bar: StatusBar,
    c_settings: Settings,
    c_main_view: Panel<(
        Button<[Label; 1]>,
        Button<[Label; 1]>,
        LibraryBrowser,
        QueueViewer,
        Button<[Label; 1]>,
    )>,
    pub c_context_menu: Option<Box<dyn GuiElem>>,
    pub idle: AnimationController<f32>,
    // pub settings: (bool, Option<Instant>),
    pub settings: (bool, Option<Instant>),
    pub last_interaction: Instant,
    idle_timeout: Option<f64>,
    pub prev_mouse_pos: Vec2,
    pub hotkey: Hotkey,
}
impl GuiScreen {
    pub fn new(
        config: GuiElemCfg,
        c_notif_overlay: NotifOverlay,
        no_animations: bool,
        line_height: f32,
        scroll_sensitivity_pixels: f64,
        scroll_sensitivity_lines: f64,
        scroll_sensitivity_pages: f64,
    ) -> Self {
        Self {
            config: config.w_keyboard_watch().w_mouse().w_keyboard_focus(),
            c_notif_overlay,
            c_status_bar: StatusBar::new(GuiElemCfg::at(Rectangle::from_tuples(
                (0.0, 0.9),
                (1.0, 1.0),
            ))),
            c_idle_display: IdleDisplay::new(GuiElemCfg::default().disabled()),
            c_settings: Settings::new(
                GuiElemCfg::default().disabled(),
                no_animations,
                line_height,
                scroll_sensitivity_pixels,
                scroll_sensitivity_lines,
                scroll_sensitivity_pages,
            ),
            c_main_view: Panel::new(
                GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.0), (1.0, 0.9))),
                (
                    Button::new(
                        GuiElemCfg::at(Rectangle::from_tuples((0.75, 0.0), (0.875, 0.03))),
                        |_| vec![GuiAction::OpenSettings(true)],
                        [Label::new(
                            GuiElemCfg::default(),
                            "Settings".to_string(),
                            Color::WHITE,
                            None,
                            Vec2::new(0.5, 0.5),
                        )],
                    ),
                    Button::new(
                        GuiElemCfg::at(Rectangle::from_tuples((0.875, 0.0), (1.0, 0.03))),
                        |_| vec![GuiAction::Exit],
                        [Label::new(
                            GuiElemCfg::default(),
                            "Exit".to_string(),
                            Color::WHITE,
                            None,
                            Vec2::new(0.5, 0.5),
                        )],
                    ),
                    LibraryBrowser::new(GuiElemCfg::at(Rectangle::from_tuples(
                        (0.0, 0.0),
                        (0.5, 1.0),
                    ))),
                    QueueViewer::new(GuiElemCfg::at(Rectangle::from_tuples(
                        (0.5, 0.03),
                        (1.0, 1.0),
                    ))),
                    Button::new(
                        GuiElemCfg::at(Rectangle::from_tuples((0.5, 0.0), (0.75, 0.03))),
                        |_| {
                            vec![GuiAction::SendToServer(
                                musicdb_lib::server::Command::QueueUpdate(
                                    vec![],
                                    musicdb_lib::data::queue::QueueContent::Folder(
                                        0,
                                        vec![],
                                        String::new(),
                                    )
                                    .into(),
                                ),
                            )]
                        },
                        [Label::new(
                            GuiElemCfg::default(),
                            "Clear Queue".to_string(),
                            Color::WHITE,
                            None,
                            Vec2::new(0.5, 0.5),
                        )],
                    ),
                ),
            ),
            c_context_menu: None,
            hotkey: Hotkey::new_noshift(VirtualKeyCode::Escape),
            idle: AnimationController::new(0.0, 0.0, 0.01, 1.0, 0.8, 0.6, Instant::now()),
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
        self.idle.target = 0.0;
    }
    fn idle_check(&mut self) {
        if self.idle.target == 0.0 {
            if let Some(dur) = &self.idle_timeout {
                if self.last_interaction.elapsed().as_secs_f64() > *dur {
                    self.idle.target = 1.0;
                }
            }
        }
    }
}
impl GuiElem for GuiScreen {
    fn config(&self) -> &GuiElemCfg {
        &self.config
    }
    fn config_mut(&mut self) -> &mut GuiElemCfg {
        &mut self.config
    }
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut dyn GuiElem> + '_> {
        Box::new(
            [
                self.c_notif_overlay.elem_mut(),
                self.c_idle_display.elem_mut(),
                self.c_status_bar.elem_mut(),
                self.c_settings.elem_mut(),
                self.c_main_view.elem_mut(),
            ]
            .into_iter()
            .chain(self.c_context_menu.as_mut().map(|v| v.elem_mut())),
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
    fn key_watch(
        &mut self,
        modifiers: speedy2d::window::ModifiersState,
        down: bool,
        key: Option<speedy2d::window::VirtualKeyCode>,
        _scan: speedy2d::window::KeyScancode,
    ) -> Vec<GuiAction> {
        self.not_idle();
        if self.hotkey.triggered(modifiers, down, key) {
            self.config.request_keyboard_focus = true;
            vec![GuiAction::ResetKeyboardFocus]
        } else {
            vec![]
        }
    }
    fn mouse_down(&mut self, _button: speedy2d::window::MouseButton) -> Vec<GuiAction> {
        self.not_idle();
        vec![]
    }
    fn draw(&mut self, info: &mut DrawInfo, _g: &mut Graphics2D) {
        // idle stuff
        if self.prev_mouse_pos != info.mouse_pos {
            self.prev_mouse_pos = info.mouse_pos;
            self.not_idle();
        } else if self.idle.target == 0.0 && self.config.pixel_pos.size() != info.pos.size() {
            // resizing prevents idle, but doesn't un-idle
            self.not_idle();
        }
        if !(!info.database.playing
            || matches!(info.database.queue.content(), QueueContent::Folder(_, v, _) if v.is_empty()))
        {
            // skip idle_check if paused or queue is empty
            self.idle_check();
        }
        // request_redraw for animations
        let idle_changed = self.idle.update(info.time, info.no_animations);
        if idle_changed || self.settings.1.is_some() {
            if let Some(h) = &info.helper {
                h.request_redraw()
            }
        }
        // animations: idle
        if idle_changed {
            let enable_normal_ui = self.idle.value < 1.0;
            self.c_main_view.config_mut().enabled = enable_normal_ui;
            self.c_settings.config_mut().enabled = enable_normal_ui;
            self.c_status_bar.config_mut().enabled = enable_normal_ui;
            if let Some(h) = &info.helper {
                h.set_cursor_visible(enable_normal_ui);
            }
            let idcfg = self.c_idle_display.config_mut();
            let top = 1.0 - self.idle.value;
            let bottom = top + 1.0;
            idcfg.pos = Rectangle::from_tuples((0.0, top), (1.0, bottom));
            idcfg.enabled = self.idle.value > 0.0;
            self.c_status_bar.idle_mode = self.idle.value;
            self.c_idle_display.idle_mode = self.idle.value;
        }
        // animations: settings
        if self.settings.1.is_some() {
            let p1 = Self::get_prog(&mut self.settings, 0.3);
            let p = transition(p1);
            let cfg = self.c_settings.config_mut();
            cfg.enabled = p > 0.0;
            cfg.pos = Rectangle::from_tuples((0.0, 0.9 - 0.9 * p), (1.0, 0.9));
        }
        // set idle timeout (only when settings are open)
        if self.settings.0 || self.settings.1.is_some() {
            self.idle_timeout = self.c_settings.get_timeout_val();
        }
    }
    fn key_focus(
        &mut self,
        _modifiers: speedy2d::window::ModifiersState,
        down: bool,
        key: Option<speedy2d::window::VirtualKeyCode>,
        _scan: speedy2d::window::KeyScancode,
    ) -> Vec<GuiAction> {
        if down && matches!(key, Some(VirtualKeyCode::Space)) {
            vec![GuiAction::Build(Box::new(|db| {
                vec![GuiAction::SendToServer(if db.playing {
                    Command::Pause
                } else {
                    Command::Resume
                })]
            }))]
        } else if down && matches!(key, Some(VirtualKeyCode::F8)) {
            vec![GuiAction::SendToServer(Command::ErrorInfo(
                "".to_owned(),
                "tEsT".to_owned(),
            ))]
        } else {
            vec![]
        }
    }
}
