use std::rc::Rc;

use speedy2d::{
    color::Color,
    dimen::Vec2,
    font::{FormattedTextBlock, TextLayout, TextOptions},
    shape::Rectangle,
    window::{ModifiersState, MouseButton},
};

use crate::gui::{GuiAction, GuiElem, GuiElemCfg, GuiElemTrait};

/*

Some basic structs to use everywhere,
except they are all text-related.

*/

#[derive(Clone)]
pub struct Label {
    config: GuiElemCfg,
    children: Vec<GuiElem>,
    pub content: Content,
    pub pos: Vec2,
}
#[derive(Clone)]
pub struct Content {
    text: String,
    color: Color,
    background: Option<Color>,
    formatted: Option<Rc<FormattedTextBlock>>,
}
impl Content {
    pub fn new(text: String, color: Color) -> Self {
        Self {
            text: if text.starts_with(' ') {
                text.replacen(' ', "\u{00A0}", 1)
            } else {
                text
            },
            color,
            background: None,
            formatted: None,
        }
    }
    pub fn get_text(&self) -> &String {
        &self.text
    }
    pub fn get_color(&self) -> &Color {
        &self.color
    }
    /// causes text layout reset
    pub fn text(&mut self) -> &mut String {
        self.formatted = None;
        &mut self.text
    }
    pub fn color(&mut self) -> &mut Color {
        &mut self.color
    }
    /// returns true if the text needs to be redrawn, probably because it was changed.
    pub fn will_redraw(&self) -> bool {
        self.formatted.is_none()
    }
}
impl Label {
    pub fn new(
        config: GuiElemCfg,
        text: String,
        color: Color,
        background: Option<Color>,
        pos: Vec2,
    ) -> Self {
        Self {
            config,
            children: vec![],
            content: Content {
                text,
                color,
                background,
                formatted: None,
            },
            pos,
        }
    }
}
impl GuiElemTrait for Label {
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
    fn draw(&mut self, info: &mut crate::gui::DrawInfo, g: &mut speedy2d::Graphics2D) {
        if self.config.pixel_pos.size() != info.pos.size() {
            // resize
            self.content.formatted = None;
        }
        let text = if let Some(text) = &self.content.formatted {
            text
        } else {
            let l = info
                .font
                .layout_text(&self.content.text, 1.0, TextOptions::new());
            let l = info.font.layout_text(
                &self.content.text,
                (info.pos.width() / l.width()).min(info.pos.height() / l.height()),
                TextOptions::new(),
            );
            self.content.formatted = Some(l);
            self.content.formatted.as_ref().unwrap()
        };
        let top_left = Vec2::new(
            info.pos.top_left().x + self.pos.x * (info.pos.width() - text.width()),
            info.pos.top_left().y + self.pos.y * (info.pos.height() - text.height()),
        );
        if let Some(bg) = self.content.background {
            g.draw_rectangle(
                Rectangle::new(
                    top_left,
                    Vec2::new(top_left.x + text.width(), top_left.y + text.height()),
                ),
                bg,
            );
        }
        g.draw_text(top_left, self.content.color, text);
    }
}

// TODO! this, but requires keyboard events first

/// a single-line text field for users to type text into.
pub struct TextField {
    config: GuiElemCfg,
    pub children: Vec<GuiElem>,
    pub on_changed: Option<Box<dyn FnMut(&str)>>,
    pub on_changed_mut: Option<Box<dyn FnMut(&mut Self, String)>>,
}
impl TextField {
    pub fn new(config: GuiElemCfg, hint: String, color_hint: Color, color_input: Color) -> Self {
        Self::new_adv(config, String::new(), hint, color_hint, color_input)
    }
    pub fn new_adv(
        config: GuiElemCfg,
        text: String,
        hint: String,
        color_hint: Color,
        color_input: Color,
    ) -> Self {
        let text_is_empty = text.is_empty();
        Self {
            config: config.w_mouse().w_keyboard_focus(),
            children: vec![
                GuiElem::new(Label::new(
                    GuiElemCfg::default(),
                    text,
                    color_input,
                    None,
                    Vec2::new(0.0, 0.5),
                )),
                GuiElem::new(Label::new(
                    if text_is_empty {
                        GuiElemCfg::default()
                    } else {
                        GuiElemCfg::default().disabled()
                    },
                    hint,
                    color_hint,
                    None,
                    Vec2::new(0.0, 0.5),
                )),
            ],
            on_changed: None,
            on_changed_mut: None,
        }
    }
    pub fn label_input(&self) -> &Label {
        self.children[0].inner.any().downcast_ref().unwrap()
    }
    pub fn label_input_mut(&mut self) -> &mut Label {
        self.children[0].inner.any_mut().downcast_mut().unwrap()
    }
    pub fn label_hint(&self) -> &Label {
        self.children[1].inner.any().downcast_ref().unwrap()
    }
    pub fn label_hint_mut(&mut self) -> &mut Label {
        self.children[1].inner.any_mut().downcast_mut().unwrap()
    }
}
impl GuiElemTrait for TextField {
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
    fn draw(&mut self, info: &mut crate::gui::DrawInfo, g: &mut speedy2d::Graphics2D) {
        let (t, c) = if info.has_keyboard_focus {
            (3.0, Color::WHITE)
        } else {
            (1.0, Color::GRAY)
        };
        g.draw_line(info.pos.top_left(), info.pos.top_right(), t, c);
        g.draw_line(info.pos.bottom_left(), info.pos.bottom_right(), t, c);
        g.draw_line(info.pos.top_left(), info.pos.bottom_left(), t, c);
        g.draw_line(info.pos.top_right(), info.pos.bottom_right(), t, c);
    }
    fn mouse_pressed(&mut self, _button: MouseButton) -> Vec<GuiAction> {
        self.config.request_keyboard_focus = true;
        vec![GuiAction::ResetKeyboardFocus]
    }
    fn char_focus(&mut self, modifiers: ModifiersState, key: char) -> Vec<GuiAction> {
        if !(modifiers.ctrl() || modifiers.alt() || modifiers.logo()) && !key.is_control() {
            let content = &mut self.children[0].try_as_mut::<Label>().unwrap().content;
            let was_empty = content.get_text().is_empty();
            content.text().push(key);
            if let Some(f) = &mut self.on_changed {
                f(content.get_text());
            }
            if let Some(mut f) = self.on_changed_mut.take() {
                let text = content.get_text().clone();
                f(self, text);
                self.on_changed_mut = Some(f);
            }
            if was_empty {
                self.children[1].inner.config_mut().enabled = false;
            }
        }
        vec![]
    }
    fn key_focus(
        &mut self,
        modifiers: ModifiersState,
        down: bool,
        key: Option<speedy2d::window::VirtualKeyCode>,
        _scan: speedy2d::window::KeyScancode,
    ) -> Vec<GuiAction> {
        if down
            && !(modifiers.alt() || modifiers.logo())
            && key == Some(speedy2d::window::VirtualKeyCode::Backspace)
        {
            let content = &mut self.children[0].try_as_mut::<Label>().unwrap().content;
            if !content.get_text().is_empty() {
                if modifiers.ctrl() {
                    for s in [true, false, true] {
                        while !content.get_text().is_empty()
                            && content.get_text().ends_with(' ') == s
                        {
                            content.text().pop();
                        }
                    }
                } else {
                    content.text().pop();
                }
                let is_now_empty = content.get_text().is_empty();
                if let Some(f) = &mut self.on_changed {
                    f(content.get_text());
                }
                if let Some(mut f) = self.on_changed_mut.take() {
                    let text = content.get_text().clone();
                    f(self, text);
                    self.on_changed_mut = Some(f);
                }
                if is_now_empty {
                    self.children[1].inner.config_mut().enabled = true;
                }
            }
        }
        vec![]
    }
}
impl Clone for TextField {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            children: self.children.clone(),
            on_changed: None,
            on_changed_mut: None,
        }
    }
}

/// More advanced version of `Label`.
/// Allows stringing together multiple `Content`s in one line.
#[derive(Clone)]
pub struct AdvancedLabel {
    config: GuiElemCfg,
    children: Vec<GuiElem>,
    /// 0.0 => align to top/left
    /// 0.5 => center
    /// 1.0 => align to bottom/right
    pub align: Vec2,
    /// (Content, Size-Scale, Height)
    /// Size-Scale and Height should default to 1.0.
    pub content: Vec<Vec<(Content, f32, f32)>>,
    /// the position from where content drawing starts.
    /// recalculated when layouting is performed.
    content_pos: Vec2,
}
impl AdvancedLabel {
    pub fn new(config: GuiElemCfg, align: Vec2, content: Vec<Vec<(Content, f32, f32)>>) -> Self {
        Self {
            config,
            children: vec![],
            align,
            content,
            content_pos: Vec2::ZERO,
        }
    }
}
impl GuiElemTrait for AdvancedLabel {
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
    fn draw(&mut self, info: &mut crate::gui::DrawInfo, g: &mut speedy2d::Graphics2D) {
        if self.config.redraw
            || self.config.pixel_pos.size() != info.pos.size()
            || self
                .content
                .iter()
                .any(|v| v.iter().any(|(c, _, _)| c.will_redraw()))
        {
            self.config.redraw = false;
            let mut max_len = 0.0;
            let mut total_height = 0.0;
            for line in &self.content {
                let mut len = 0.0;
                let mut height = 0.0;
                for (c, scale, _) in line {
                    let size = info
                        .font
                        .layout_text(&c.text, 1.0, TextOptions::new())
                        .size();
                    len += size.x * scale;
                    if size.y * scale > height {
                        height = size.y * scale;
                    }
                }
                if len > max_len {
                    max_len = len;
                }
                total_height += height;
            }
            if max_len > 0.0 && total_height > 0.0 {
                let scale1 = info.pos.width() / max_len;
                let scale2 = info.pos.height() / total_height;
                let scale;
                self.content_pos = if scale1 < scale2 {
                    // use all available width
                    scale = scale1;
                    Vec2::new(
                        0.0,
                        (info.pos.height() - (total_height * scale)) * self.align.y,
                    )
                } else {
                    // use all available height
                    scale = scale2;
                    Vec2::new((info.pos.width() - (max_len * scale)) * self.align.x, 0.0)
                };
                for line in &mut self.content {
                    for (c, s, _) in line {
                        c.formatted = Some(info.font.layout_text(
                            &c.text,
                            scale * (*s),
                            TextOptions::new(),
                        ));
                    }
                }
            }
        }
        let pos_x_start = info.pos.top_left().x + self.content_pos.x;
        let mut pos_y = info.pos.top_left().y + self.content_pos.y;
        for line in &self.content {
            let mut pos_x = pos_x_start;
            let height = line
                .iter()
                .filter_map(|v| v.0.formatted.as_ref())
                .map(|f| f.height())
                .reduce(f32::max)
                .unwrap_or(0.0);
            for (c, _, h) in line {
                if let Some(f) = &c.formatted {
                    let y = pos_y + (height - f.height()) * h;
                    g.draw_text(Vec2::new(pos_x, y), c.color, f);
                    pos_x += f.width();
                }
            }
            pos_y += height;
        }
    }
}
