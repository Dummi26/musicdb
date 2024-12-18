use std::time::Instant;

use musicdb_lib::{
    data::queue::{QueueContent, QueueFolder},
    server::{Action, Req},
};
use speedy2d::{color::Color, dimen::Vec2, shape::Rectangle, window::VirtualKeyCode, Graphics2D};
use uianimator::{default_animator_f64_quadratic::DefaultAnimatorF64Quadratic, Animator};

use crate::{
    gui::{
        DrawInfo, EventInfo, GuiAction, GuiElem, GuiElemCfg, GuiElemChildren, KeyAction,
        KeyBinding, SpecificGuiElem,
    },
    gui_base::{Button, Panel},
    gui_edit_song::EditorForSongs,
    gui_idle_display::IdleDisplay,
    gui_library::LibraryBrowser,
    gui_notif::NotifOverlay,
    gui_queue::QueueViewer,
    gui_settings::Settings,
    gui_song_adder::SongAdder,
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
    pub c_notif_overlay: NotifOverlay,
    pub c_idle_display: IdleDisplay,
    pub c_editing_songs: Option<EditorForSongs>,
    pub c_status_bar: StatusBar,
    pub c_settings: Settings,
    pub c_song_adder: Option<SongAdder>,
    pub c_main_view: Panel<MainView>,
    pub c_context_menu: Option<Box<dyn GuiElem>>,
    pub idle: DefaultAnimatorF64Quadratic,
    pub idle_prev_val: f32,
    // pub settings: (bool, Option<Instant>),
    pub settings: (bool, Option<Instant>),
    pub last_interaction: Instant,
    idle_timeout: Option<f64>,
    pub prev_mouse_pos: Vec2,
    pub hotkey: Hotkey,
}
pub struct MainView {
    pub button_clear_queue: Button<[Label; 1]>,
    pub button_settings: Button<[Label; 1]>,
    pub button_exit: Button<[Label; 1]>,
    pub library_browser: LibraryBrowser,
    pub queue_viewer: QueueViewer,
}
impl GuiElemChildren for MainView {
    fn iter(&mut self) -> Box<dyn Iterator<Item = &mut dyn GuiElem> + '_> {
        Box::new(
            [
                self.button_clear_queue.elem_mut(),
                self.button_settings.elem_mut(),
                self.button_exit.elem_mut(),
                self.library_browser.elem_mut(),
                self.queue_viewer.elem_mut(),
            ]
            .into_iter(),
        )
    }
    fn len(&self) -> usize {
        5
    }
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
            config: config.w_keyboard_watch().w_mouse(),
            c_notif_overlay,
            c_status_bar: StatusBar::new(GuiElemCfg::at(Rectangle::from_tuples(
                (0.0, 0.9),
                (1.0, 1.0),
            ))),
            c_editing_songs: None,
            c_idle_display: IdleDisplay::new(GuiElemCfg::default().disabled()),
            c_settings: Settings::new(
                GuiElemCfg::default().disabled(),
                no_animations,
                line_height,
                scroll_sensitivity_pixels,
                scroll_sensitivity_lines,
                scroll_sensitivity_pages,
            ),
            c_song_adder: None,
            c_main_view: Panel::new(
                GuiElemCfg::at(Rectangle::from_tuples((0.0, 0.0), (1.0, 0.9))),
                MainView {
                    button_clear_queue: Button::new(
                        GuiElemCfg::at(Rectangle::from_tuples((0.5, 0.0), (0.75, 0.03))),
                        |_| {
                            vec![GuiAction::SendToServer(Action::QueueUpdate(
                                vec![],
                                musicdb_lib::data::queue::QueueContent::Folder(
                                    musicdb_lib::data::queue::QueueFolder::default(),
                                )
                                .into(),
                                Req::none(),
                            ))]
                        },
                        [Label::new(
                            GuiElemCfg::default(),
                            "Clear Queue".to_string(),
                            Color::WHITE,
                            None,
                            Vec2::new(0.5, 0.5),
                        )],
                    ),
                    button_settings: Button::new(
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
                    button_exit: Button::new(
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
                    library_browser: LibraryBrowser::new(GuiElemCfg::at(Rectangle::from_tuples(
                        (0.0, 0.0),
                        (0.5, 1.0),
                    ))),
                    queue_viewer: QueueViewer::new(GuiElemCfg::at(Rectangle::from_tuples(
                        (0.5, 0.03),
                        (1.0, 1.0),
                    ))),
                },
            ),
            c_context_menu: None,
            hotkey: Hotkey::new_noshift(VirtualKeyCode::Escape),
            idle: DefaultAnimatorF64Quadratic::new(0.0, 0.67),
            idle_prev_val: 0.0,
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
    pub fn force_idle(&mut self) {
        self.idle.set_target(1.0, Instant::now());
    }
    pub fn not_idle(&mut self) {
        let time = Instant::now();
        self.last_interaction = time;
        if self.idle.target() > 0.0 {
            if self.idle.get_value(time) < 1.0 {
                self.idle.set_target(0.0, time);
            } else {
                self.c_idle_display.c_idle_exit_hint.config_mut().enabled = true;
            }
        }
    }
    pub fn unidle(&mut self) {
        self.not_idle();
        self.c_idle_display.c_idle_exit_hint.config_mut().enabled = false;
        self.idle.set_target(0.0, Instant::now());
    }
    fn idle_check(&mut self) {
        if self.idle.target() == 0.0 {
            if let Some(dur) = &self.idle_timeout {
                if self.last_interaction.elapsed().as_secs_f64() > *dur {
                    self.idle.set_target(1.0, Instant::now());
                }
            }
        }
    }

    pub fn set_normal_ui_enabled(&mut self, enabled: bool) {
        self.c_status_bar.config_mut().enabled = enabled;
        // self.c_settings.config_mut().enabled = enabled;
        self.c_main_view.config_mut().enabled = enabled;
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
            self.c_context_menu.iter_mut().map(|v| v.elem_mut()).chain(
                [
                    self.c_notif_overlay.elem_mut(),
                    self.c_idle_display.elem_mut(),
                ]
                .into_iter()
                .chain(self.c_editing_songs.as_mut().map(|v| v.elem_mut()))
                .chain(self.c_song_adder.as_mut().map(|v| v.elem_mut()).into_iter())
                .chain([
                    self.c_status_bar.elem_mut(),
                    self.c_settings.elem_mut(),
                    self.c_main_view.elem_mut(),
                ]),
            ),
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
        e: &mut EventInfo,
        modifiers: speedy2d::window::ModifiersState,
        down: bool,
        key: Option<speedy2d::window::VirtualKeyCode>,
        _scan: speedy2d::window::KeyScancode,
    ) -> Vec<GuiAction> {
        if down {
            // don't `e.take()` here (it might cause keys the user wanted to type to go missing + it would break the next if statement)
            self.not_idle();
        }
        if self.hotkey.triggered(modifiers, down, key) && e.take() {
            self.config.request_keyboard_focus = true;
            vec![GuiAction::ResetKeyboardFocus]
        // } else if down && matches!(key, Some(VirtualKeyCode::Space)) && e.take() {
        // MOVED TO CHAR_WATCH !!!
        } else {
            vec![]
        }
    }
    fn char_watch(
        &mut self,
        e: &mut EventInfo,
        modifiers: speedy2d::window::ModifiersState,
        key: char,
    ) -> Vec<GuiAction> {
        if key == ' ' && !(modifiers.ctrl() || modifiers.alt() || modifiers.logo()) && e.take() {
            vec![GuiAction::Build(Box::new(|db| {
                vec![GuiAction::SendToServer(if db.playing {
                    Action::Pause
                } else {
                    Action::Resume
                })]
            }))]
        } else {
            vec![]
        }
    }
    fn mouse_down(
        &mut self,
        _e: &mut EventInfo,
        _button: speedy2d::window::MouseButton,
    ) -> Vec<GuiAction> {
        self.not_idle();
        vec![]
    }
    fn draw(&mut self, info: &mut DrawInfo, _g: &mut Graphics2D) {
        if self.config.init {
            info.actions.extend([
                GuiAction::AddKeybind(
                    Some((KeyBinding::ctrl(VirtualKeyCode::Q), true)),
                    KeyAction {
                        category: "General".to_owned(),
                        title: "Quit".to_owned(),
                        description: "Closes the application".to_owned(),
                        action: Box::new(|| vec![GuiAction::Exit]),
                        enabled: true,
                    },
                    Box::new(|_| {}),
                ),
                GuiAction::AddKeybind(
                    Some((KeyBinding::ctrl(VirtualKeyCode::I), true)),
                    KeyAction {
                        category: "General".to_owned(),
                        title: "Idle".to_owned(),
                        description: "Opens the idle display".to_owned(),
                        action: Box::new(|| vec![GuiAction::ForceIdle]),
                        enabled: true,
                    },
                    Box::new(|_| {}),
                ),
                GuiAction::AddKeybind(
                    Some((KeyBinding::ctrl(VirtualKeyCode::F), true)),
                    KeyAction {
                        category: "Library".to_owned(),
                        title: "Search songs".to_owned(),
                        description: "moves keyboard focus to the song search".to_owned(),
                        action: Box::new(|| {
                            vec![GuiAction::SetFocused(SpecificGuiElem::SearchSong)]
                        }),
                        enabled: true,
                    },
                    Box::new(|_| {}),
                ),
            ]);
        }
        // idle stuff
        if self.prev_mouse_pos != info.mouse_pos {
            self.prev_mouse_pos = info.mouse_pos;
            self.not_idle();
        } else if self.idle.target() == 0.0 && self.config.pixel_pos.size() != info.pos.size() {
            // resizing prevents idle, but doesn't un-idle
            self.not_idle();
        }
        if !(!info.database.playing
            || matches!(info.database.queue.content(), QueueContent::Folder(QueueFolder { content: v, .. }) if v.is_empty()))
        {
            // skip idle_check if paused or queue is empty
            self.idle_check();
        }
        // show/hide idle_exit_hint
        let idle_exit_anim = if self.c_idle_display.c_idle_exit_hint.config().enabled {
            let hide = info
                .time
                .duration_since(self.last_interaction)
                .as_secs_f32()
                / 3.0;
            let cv = if hide >= 1.0 {
                self.c_idle_display.c_idle_exit_hint.config_mut().enabled = false;
                false
            } else {
                let v = hide * hide;
                let w = 0.15;
                let h = 0.05;
                let dx = w * v;
                let dy = h * v;
                self.c_idle_display.c_idle_exit_hint.config_mut().pos =
                    Rectangle::from_tuples((-dx, -dy), (w - dx, h - dy));
                true
            };
            if let Some(h) = &info.helper {
                h.set_cursor_visible(cv);
            }
            cv
        } else {
            false
        };
        // request_redraw for animations
        let idle_value = self.idle.get_value(Instant::now()) as f32;
        let idle_changed = self.idle_prev_val != idle_value;
        if idle_changed || idle_exit_anim || self.settings.1.is_some() {
            self.idle_prev_val = idle_value;
            if let Some(h) = &info.helper {
                h.request_redraw()
            }
        }
        // animations: idle
        if idle_changed {
            let enable_normal_ui = idle_value < 1.0;
            self.set_normal_ui_enabled(enable_normal_ui);
            if let Some(h) = &info.helper {
                h.set_cursor_visible(enable_normal_ui);
            }
            let idcfg = self.c_idle_display.config_mut();
            let top = 1.0 - idle_value;
            let bottom = top + 1.0;
            idcfg.pos = Rectangle::from_tuples((0.0, top), (1.0, bottom));
            idcfg.enabled = idle_value > 0.0;
            self.c_status_bar.idle_mode = idle_value;
            self.c_idle_display.idle_mode = idle_value;
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
}
