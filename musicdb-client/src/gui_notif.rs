use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

use speedy2d::{color::Color, dimen::Vector2, shape::Rectangle};

use crate::gui::{GuiElem, GuiElemCfg, GuiElemTrait};

/// This should be added on top of overything else and set to fullscreen.
/// It will respond to notification events.
pub struct NotifOverlay {
    config: GuiElemCfg,
    notifs: Vec<(GuiElem, NotifInfo)>,
    light: Option<(Instant, Color)>,
    receiver: mpsc::Receiver<Box<dyn FnOnce(&Self) -> (GuiElem, NotifInfo) + Send>>,
}

impl NotifOverlay {
    pub fn new() -> (
        Self,
        mpsc::Sender<Box<dyn FnOnce(&Self) -> (GuiElem, NotifInfo) + Send>>,
    ) {
        let (sender, receiver) = mpsc::channel();
        (
            Self {
                config: GuiElemCfg::default(),
                notifs: vec![],
                light: None,
                receiver,
            },
            sender,
        )
    }

    fn check_notifs(&mut self) {
        let mut adjust_heights = false;
        let mut remove = Vec::with_capacity(0);
        for (i, (gui, info)) in self.notifs.iter_mut().enumerate() {
            match info.time {
                NotifInfoTime::Pending => {
                    if self.light.is_none() {
                        let now = Instant::now();
                        info.time = NotifInfoTime::FadingIn(now);
                        if let Some(color) = info.color {
                            self.light = Some((now, color));
                        }
                        adjust_heights = true;
                        gui.inner.config_mut().enabled = true;
                    }
                }
                NotifInfoTime::FadingIn(since) => {
                    adjust_heights = true;
                    let p = since.elapsed().as_secs_f32() / 0.25;
                    if p >= 1.0 {
                        info.time = NotifInfoTime::Displayed(Instant::now());
                        info.progress = 0.0;
                    } else {
                        info.progress = p;
                    }
                }
                NotifInfoTime::Displayed(since) => {
                    let p = since.elapsed().as_secs_f32() / info.duration.as_secs_f32();
                    if p >= 1.0 {
                        info.time = NotifInfoTime::FadingOut(Instant::now());
                        info.progress = 0.0;
                    } else {
                        info.progress = p;
                    }
                }
                NotifInfoTime::FadingOut(since) => {
                    adjust_heights = true;
                    let p = since.elapsed().as_secs_f32() / 0.25;
                    if p >= 1.0 {
                        remove.push(i);
                    } else {
                        info.progress = p;
                    }
                }
            }
        }
        for index in remove.into_iter().rev() {
            self.notifs.remove(index);
        }
        if adjust_heights {
            self.adjust_heights();
        }
    }

    fn adjust_heights(&mut self) {
        let screen_size = self.config.pixel_pos.size();
        let width = 0.3;
        let left = 0.5 - (0.5 * width);
        let right = 0.5 + (0.5 * width);
        let height = 0.2 * width * screen_size.x / screen_size.y;
        let space = 0.05 / 0.2 * height;
        let mut y = 0.0;
        for (gui, info) in self.notifs.iter_mut() {
            y += space;
            let pos_y = if matches!(info.time, NotifInfoTime::FadingOut(..)) {
                let v = y - (height + y) * info.progress * info.progress;
                // for notifs below this one
                y -= (height + space) * crate::gui_screen::transition(info.progress);
                v
            } else if matches!(info.time, NotifInfoTime::FadingIn(..)) {
                -height + (height + y) * (1.0 - (1.0 - info.progress) * (1.0 - info.progress))
            } else {
                y
            };
            y += height;
            gui.inner.config_mut().pos =
                Rectangle::from_tuples((left, pos_y), (right, pos_y + height));
        }
    }
}

#[derive(Clone)]
pub struct NotifInfo {
    time: NotifInfoTime,
    duration: Duration,
    /// when the notification is first shown on screen,
    /// light up the edges of the screen/window
    /// in this color (usually red for important things)
    color: Option<Color>,
    /// used for fade-out animation
    progress: f32,
}
#[derive(Clone)]
enum NotifInfoTime {
    Pending,
    FadingIn(Instant),
    Displayed(Instant),
    FadingOut(Instant),
}
impl NotifInfo {
    pub fn new(duration: Duration) -> Self {
        Self {
            time: NotifInfoTime::Pending,
            duration,
            color: None,
            progress: 0.0,
        }
    }
    pub fn with_highlight(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }
}

impl Clone for NotifOverlay {
    fn clone(&self) -> Self {
        Self::new().0
    }
}

impl GuiElemTrait for NotifOverlay {
    fn draw(&mut self, info: &mut crate::gui::DrawInfo, g: &mut speedy2d::Graphics2D) {
        if let Ok(notif) = self.receiver.try_recv() {
            let mut n = notif(self);
            n.0.inner.config_mut().enabled = false;
            self.notifs.push(n);
        }
        self.check_notifs();
        // light
        if let Some((since, color)) = self.light {
            let p = since.elapsed().as_secs_f32() / 0.5;
            if p >= 1.0 {
                self.light = None;
            } else {
                let f = p * 2.0 - 1.0;
                let f = 1.0 - f * f;
                let color = Color::from_rgba(color.r(), color.g(), color.b(), color.a() * f);
                let Vector2 { x: x1, y: y1 } = *info.pos.top_left();
                let Vector2 { x: x2, y: y2 } = *info.pos.bottom_right();
                let width = info.pos.width() * 0.01;
                g.draw_rectangle(Rectangle::from_tuples((x1, y1), (x1 + width, y2)), color);
                g.draw_rectangle(Rectangle::from_tuples((x2 - width, y1), (x2, y2)), color);
            }
        }
        // redraw
        if !self.notifs.is_empty() {
            if let Some(h) = &info.helper {
                h.request_redraw();
            }
        }
    }
    fn draw_rev(&self) -> bool {
        true
    }

    fn config(&self) -> &GuiElemCfg {
        &self.config
    }
    fn config_mut(&mut self) -> &mut GuiElemCfg {
        &mut self.config
    }
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut GuiElem> + '_> {
        Box::new(self.notifs.iter_mut().map(|(v, _)| v))
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
