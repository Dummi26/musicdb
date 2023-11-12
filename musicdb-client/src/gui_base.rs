use std::{sync::Arc, time::Instant};

use speedy2d::{color::Color, dimen::Vec2, shape::Rectangle, window::MouseButton};

use crate::{
    gui::{DrawInfo, GuiAction, GuiElem, GuiElemCfg, GuiElemChildren},
    gui_text::Label,
};

/*

Some basic structs to use everywhere.
Mostly containers for other GuiElems.

*/

/// A simple container for zero, one, or multiple child GuiElems. Can optionally fill the background with a color.
pub struct Panel<C: GuiElemChildren> {
    config: GuiElemCfg,
    pub children: C,
    pub background: Option<Color>,
}
impl<C: GuiElemChildren> Panel<C> {
    pub fn new(config: GuiElemCfg, children: C) -> Self {
        Self {
            config,
            children,
            background: None,
        }
    }
    pub fn with_background(config: GuiElemCfg, children: C, background: Color) -> Self {
        Self {
            config,
            children,
            background: Some(background),
        }
    }
}
impl<C: GuiElemChildren + 'static> GuiElem for Panel<C> {
    fn config(&self) -> &GuiElemCfg {
        &self.config
    }
    fn config_mut(&mut self) -> &mut GuiElemCfg {
        &mut self.config
    }
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut dyn GuiElem> + '_> {
        self.children.iter()
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
    fn draw(&mut self, info: &mut DrawInfo, g: &mut speedy2d::Graphics2D) {
        if let Some(c) = self.background {
            g.draw_rectangle(info.pos.clone(), c);
        }
    }
}

#[derive(Clone)]
pub struct Square<T: GuiElem> {
    config: GuiElemCfg,
    pub inner: T,
}
#[allow(unused)]
impl<T: GuiElem> Square<T> {
    pub fn new(mut config: GuiElemCfg, inner: T) -> Self {
        config.redraw = true;
        Self { config, inner }
    }
}
impl<T: GuiElem + 'static> GuiElem for Square<T> {
    fn config(&self) -> &GuiElemCfg {
        &self.config
    }
    fn config_mut(&mut self) -> &mut GuiElemCfg {
        &mut self.config
    }
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut dyn GuiElem> + '_> {
        Box::new([self.inner.elem_mut()].into_iter())
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
    fn draw(&mut self, info: &mut DrawInfo, _g: &mut speedy2d::Graphics2D) {
        if info.pos.size() != self.config.pixel_pos.size() {
            self.config.redraw = true;
        }
        if self.config.redraw {
            self.config.redraw = false;
            if info.pos.width() > info.pos.height() {
                let w = 0.5 * info.pos.height() / info.pos.width();
                self.inner.config_mut().pos =
                    Rectangle::from_tuples((0.5 - w, 0.0), (0.5 + w, 1.0));
            } else {
                let h = 0.5 * info.pos.width() / info.pos.height();
                self.inner.config_mut().pos =
                    Rectangle::from_tuples((0.0, 0.5 - h), (1.0, 0.5 + h));
            }
        }
    }
}

pub struct ScrollBox<C: GuiElemChildren> {
    config: GuiElemCfg,
    pub children: C,
    pub children_heights: Vec<f32>,
    pub size_unit: ScrollBoxSizeUnit,
    pub scroll_target: f32,
    pub scroll_display: f32,
    height_bottom: f32,
    /// 0.max(height_bottom - 1)
    max_scroll: f32,
    last_height_px: f32,
    mouse_in_scrollbar: bool,
    mouse_scrolling: bool,
    mouse_scroll_margin_right: f32,
}
#[derive(Clone)]
#[allow(unused)]
pub enum ScrollBoxSizeUnit {
    Relative,
    Pixels,
}
impl<C: GuiElemChildren> ScrollBox<C> {
    pub fn new(
        config: GuiElemCfg,
        size_unit: ScrollBoxSizeUnit,
        children: C,
        children_heights: Vec<f32>,
    ) -> Self {
        Self {
            config: config.w_scroll().w_mouse(),
            children,
            children_heights,
            size_unit,
            scroll_target: 0.0,
            scroll_display: 0.0,
            /// the y-position of the bottom edge of the last element (i.e. the total height)
            height_bottom: 0.0,
            max_scroll: 0.0,
            last_height_px: 0.0,
            mouse_in_scrollbar: false,
            mouse_scrolling: false,
            mouse_scroll_margin_right: 0.0,
        }
    }
}
impl<C: GuiElemChildren + 'static> GuiElem for ScrollBox<C> {
    fn config(&self) -> &GuiElemCfg {
        &self.config
    }
    fn config_mut(&mut self) -> &mut GuiElemCfg {
        &mut self.config
    }
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut dyn GuiElem> + '_> {
        self.children.iter()
    }
    fn draw_rev(&self) -> bool {
        false
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
    fn draw(&mut self, info: &mut DrawInfo, g: &mut speedy2d::Graphics2D) {
        if self.config.pixel_pos.size() != info.pos.size() {
            self.config.redraw = true;
        }
        // smooth scrolling animation
        if self.scroll_target > self.max_scroll {
            self.scroll_target = self.max_scroll;
        } else if self.scroll_target < 0.0 {
            self.scroll_target = 0.0;
        }
        if self.scroll_target != self.scroll_display {
            self.config.redraw = true;
            if info.no_animations {
                self.scroll_display = self.scroll_target;
            } else {
                self.scroll_display = 0.2 * self.scroll_target + 0.8 * self.scroll_display;
                if (self.scroll_display - self.scroll_target).abs() < 1.0 / info.pos.height() {
                    self.scroll_display = self.scroll_target;
                } else if let Some(h) = &info.helper {
                    h.request_redraw();
                }
            }
        }
        // recalculate positions
        if self.config.redraw {
            // adjust height vector length if necessary
            if self.children_heights.len() != self.children.len() {
                let target = self.children.len();
                while self.children_heights.len() < target {
                    self.children_heights.push(0.0);
                }
                while self.children_heights.len() > target {
                    self.children_heights.pop();
                }
            }
            //
            self.mouse_scroll_margin_right = info.line_height * 0.2;
            let max_x = 1.0 - self.mouse_scroll_margin_right / info.pos.width();
            self.config.redraw = false;
            let mut y_pos = -self.scroll_display;
            for (e, h) in self.children.iter().zip(self.children_heights.iter()) {
                let h_rel = self.size_unit.to_rel(*h, info.pos.height());
                let y_rel = self.size_unit.to_rel(y_pos, info.pos.height());
                if y_rel + h_rel >= 0.0 && y_rel <= 1.0 {
                    let cfg = e.config_mut();
                    cfg.enabled = true;
                    cfg.pos = Rectangle::new(
                        Vec2::new(cfg.pos.top_left().x, 0.0f32.max(y_rel)),
                        Vec2::new(
                            cfg.pos.bottom_right().x.min(max_x),
                            1.0f32.min(y_rel + h_rel),
                        ),
                    );
                } else {
                    e.config_mut().enabled = false;
                }
                y_pos += *h;
            }
            self.height_bottom = y_pos + self.scroll_display;
            self.max_scroll =
                0.0f32.max(self.height_bottom - self.size_unit.from_rel(0.75, info.pos.height()));
        }
        // scroll bar
        self.mouse_in_scrollbar = info.mouse_pos.y >= info.pos.top_left().y
            && info.mouse_pos.y <= info.pos.bottom_right().y
            && info.mouse_pos.x <= info.pos.bottom_right().x
            && info.mouse_pos.x >= (info.pos.bottom_right().x - self.mouse_scroll_margin_right);
        if self.mouse_scrolling {
            self.scroll_target = (self.max_scroll * (info.mouse_pos.y - info.pos.top_left().y)
                / info.pos.height())
            .max(0.0)
            .min(self.max_scroll);
        }
        if self.mouse_in_scrollbar
            || self.mouse_scrolling
            || (self.scroll_display - self.scroll_target).abs()
                > self.size_unit.from_abs(1.0, info.pos.height())
        {
            let y1 = info.pos.top_left().y
                + info.pos.height() * self.scroll_display.min(self.scroll_target) / self.max_scroll
                - 1.0;
            let y2 = info.pos.top_left().y
                + info.pos.height() * self.scroll_display.max(self.scroll_target) / self.max_scroll
                + 1.0;
            g.draw_rectangle(
                Rectangle::from_tuples(
                    (
                        info.pos.bottom_right().x - self.mouse_scroll_margin_right,
                        y1,
                    ),
                    (info.pos.bottom_right().x, y2),
                ),
                Color::WHITE,
            );
        }
    }
    fn mouse_wheel(&mut self, diff: f32) -> Vec<crate::gui::GuiAction> {
        self.scroll_target = (self.scroll_target
            - self.size_unit.from_abs(diff as f32, self.last_height_px))
        .max(0.0);
        Vec::with_capacity(0)
    }
    fn mouse_down(&mut self, button: MouseButton) -> Vec<GuiAction> {
        if button == MouseButton::Left && self.mouse_in_scrollbar {
            self.mouse_scrolling = true;
        }
        vec![]
    }
    fn mouse_up(&mut self, button: MouseButton) -> Vec<GuiAction> {
        if button == MouseButton::Left {
            self.mouse_scrolling = false;
        }
        vec![]
    }
}
impl ScrollBoxSizeUnit {
    fn to_rel(&self, val: f32, draw_height: f32) -> f32 {
        match self {
            Self::Relative => val,
            Self::Pixels => val / draw_height,
        }
    }
    fn from_rel(&self, val: f32, draw_height: f32) -> f32 {
        match self {
            Self::Relative => val,
            Self::Pixels => val * draw_height,
        }
    }
    fn from_abs(&self, val: f32, draw_height: f32) -> f32 {
        match self {
            Self::Relative => val / draw_height,
            Self::Pixels => val,
        }
    }
}

pub struct Button<C: GuiElemChildren> {
    config: GuiElemCfg,
    pub children: C,
    action: Arc<dyn Fn(&mut Self) -> Vec<GuiAction> + 'static>,
}
impl<C: GuiElemChildren> Button<C> {
    /// automatically adds w_mouse to config
    pub fn new<F: Fn(&mut Self) -> Vec<GuiAction> + 'static>(
        config: GuiElemCfg,
        action: F,
        children: C,
    ) -> Self {
        Self {
            config: config.w_mouse(),
            children,
            action: Arc::new(action),
        }
    }
}
impl<C: GuiElemChildren + 'static> GuiElem for Button<C> {
    fn config(&self) -> &GuiElemCfg {
        &self.config
    }
    fn config_mut(&mut self) -> &mut GuiElemCfg {
        &mut self.config
    }
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut dyn GuiElem> + '_> {
        self.children.iter()
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
    fn mouse_pressed(&mut self, button: MouseButton) -> Vec<GuiAction> {
        if button == MouseButton::Left {
            (self.action.clone())(self)
        } else {
            vec![]
        }
    }
    fn draw(&mut self, info: &mut crate::gui::DrawInfo, g: &mut speedy2d::Graphics2D) {
        let mouse_down = self.config.mouse_down.0;
        let contains = info.pos.contains(info.mouse_pos);
        g.draw_rectangle(
            info.pos.clone(),
            if mouse_down && contains {
                Color::from_rgb(0.25, 0.25, 0.25)
            } else if contains || mouse_down {
                Color::from_rgb(0.15, 0.15, 0.15)
            } else {
                Color::from_rgb(0.1, 0.1, 0.1)
            },
        );
    }
}

pub struct Slider {
    pub config: GuiElemCfg,
    pub children: Vec<Box<dyn GuiElem>>,
    pub slider_pos: Rectangle,
    pub min: f64,
    pub max: f64,
    pub val: f64,
    val_changed: bool,
    pub val_changed_subs: Vec<bool>,
    /// if true, the display should be visible.
    pub display: bool,
    /// if Some, the display is in a transition period.
    /// you can set this to None to indicate that the transition has finished, but this is not required.
    pub display_since: Option<Instant>,
    pub on_update: Arc<dyn Fn(&mut Self, &mut DrawInfo)>,
}
#[allow(unused)]
impl Slider {
    /// returns true if the value of the slider has changed since the last time this function was called.
    /// this is usually used by the closure responsible for directly handling updates. if you wish to check for changes
    /// from outside, push a `false` to `val_changed_subs` and remember your index.
    /// when the value changes, this will be set to `true`. don't forget to reset it to `false` again if you find it set to `true`,
    /// or your code will run every time.
    pub fn val_changed(&mut self) -> bool {
        if self.val_changed {
            self.val_changed = false;
            true
        } else {
            false
        }
    }
    pub fn val_changed_peek(&self) -> bool {
        self.val_changed
    }
    pub fn new<F: Fn(&mut Self, &mut DrawInfo) + 'static>(
        config: GuiElemCfg,
        slider_pos: Rectangle,
        min: f64,
        max: f64,
        val: f64,
        children: Vec<Box<dyn GuiElem>>,
        on_update: F,
    ) -> Self {
        Self {
            config: config.w_mouse().w_scroll(),
            children,
            slider_pos,
            min,
            max,
            val,
            val_changed: true,
            val_changed_subs: vec![],
            display: false,
            display_since: None,
            on_update: Arc::new(on_update),
        }
    }
    pub fn new_labeled<F: Fn(&mut Self, &mut Label, &mut DrawInfo) + 'static>(
        config: GuiElemCfg,
        min: f64,
        max: f64,
        val: f64,
        mktext: F,
    ) -> Self {
        Self::new(
            config,
            Rectangle::new(Vec2::ZERO, Vec2::new(1.0, 1.0)),
            min,
            max,
            val,
            vec![Box::new(Label::new(
                GuiElemCfg::default(),
                String::new(),
                Color::WHITE,
                // Some(Color::from_int_rgba(0, 0, 0, 150)),
                None,
                Vec2::new(0.5, 1.0),
            ))],
            move |s, i| {
                if s.display || s.display_since.is_some() {
                    let mut label = s.children.pop().unwrap();
                    if let Some(l) = label.any_mut().downcast_mut::<Label>() {
                        let display_state = if let Some(since) =
                            s.display_since.map(|v| v.elapsed().as_secs_f64() / 0.2)
                        {
                            if since >= 1.0 {
                                s.display_since = None;
                                if s.display {
                                    1.0
                                } else {
                                    0.0
                                }
                            } else {
                                if let Some(h) = &i.helper {
                                    h.request_redraw();
                                }
                                s.config.redraw = true;
                                if s.display {
                                    since
                                } else {
                                    1.0 - since
                                }
                            }
                        } else {
                            1.0
                        };
                        let display_state =
                            (1.0 - (1.0 - display_state) * (1.0 - display_state)) as _;
                        if display_state == 0.0 {
                            l.config_mut().enabled = false;
                        } else {
                            l.pos.x = ((s.val - s.min) / (s.max - s.min)) as _;
                            *l.content.color() = Color::from_rgba(0.8, 0.8, 0.8, display_state);
                            let cfg = l.config_mut();
                            cfg.enabled = true;
                            let label_height = i.line_height / i.pos.height();
                            cfg.pos = Rectangle::from_tuples(
                                (0.05, 1.0 - label_height - display_state),
                                (0.95, 1.0 - display_state),
                            );
                            mktext(s, l, i);
                        }
                    }
                    s.children.push(label);
                }
            },
        )
    }
}
impl GuiElem for Slider {
    fn config(&self) -> &GuiElemCfg {
        &self.config
    }
    fn config_mut(&mut self) -> &mut GuiElemCfg {
        &mut self.config
    }
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut dyn GuiElem> + '_> {
        Box::new(self.children.iter_mut().map(|v| v.elem_mut()))
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
    fn draw(&mut self, info: &mut DrawInfo, g: &mut speedy2d::Graphics2D) {
        if self.display != (self.config.mouse_down.0 || info.pos.contains(info.mouse_pos)) {
            self.display = !self.display;
            self.display_since = Some(Instant::now());
            self.config.redraw = true;
        }
        let dot_size = (info.pos.height() * 0.9).min(info.pos.width() * 0.25);
        let y_mid_line = 0.5 * (info.pos.top_left().y + info.pos.bottom_right().y);
        let line_radius = dot_size * 0.25;
        let line_pos = Rectangle::from_tuples(
            (info.pos.top_left().x + dot_size, y_mid_line - line_radius),
            (
                info.pos.bottom_right().x - dot_size,
                y_mid_line + line_radius,
            ),
        );
        let line_left = line_pos.top_left().x;
        let line_width = line_pos.width();
        if self.config.mouse_down.0 {
            self.val = self.min
                + (self.max - self.min)
                    * 1.0f64.min(0.0f64.max(
                        (info.mouse_pos.x - line_pos.top_left().x) as f64 / line_pos.width() as f64,
                    ));
            self.val_changed = true;
            for v in &mut self.val_changed_subs {
                *v = true;
            }
            self.config.redraw = true;
        }
        let line_color = Color::from_int_rgb(50, 50, 100);
        g.draw_circle(
            Vec2::new(line_pos.top_left().x, y_mid_line),
            line_radius,
            line_color,
        );
        g.draw_circle(
            Vec2::new(line_pos.bottom_right().x, y_mid_line),
            line_radius,
            line_color,
        );
        g.draw_rectangle(line_pos, line_color);
        g.draw_circle(
            Vec2::new(
                line_left
                    + (line_width as f64 * (self.val - self.min) / (self.max - self.min)) as f32,
                y_mid_line,
            ),
            0.5 * dot_size,
            Color::CYAN,
        );
        if self.config.redraw {
            self.config.redraw = false;
            (Arc::clone(&self.on_update))(self, info);
        }
    }
}
