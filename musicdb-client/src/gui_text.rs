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
            text,
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
#[derive(Clone)]
pub struct TextField {
    config: GuiElemCfg,
    pub children: Vec<GuiElem>,
}
impl TextField {
    pub fn new(config: GuiElemCfg, hint: String, color_hint: Color, color_input: Color) -> Self {
        Self {
            config: config.w_mouse().w_keyboard_focus(),
            children: vec![
                GuiElem::new(Label::new(
                    GuiElemCfg::default(),
                    String::new(),
                    color_input,
                    None,
                    Vec2::new(0.0, 0.5),
                )),
                GuiElem::new(Label::new(
                    GuiElemCfg::default(),
                    hint,
                    color_hint,
                    None,
                    Vec2::new(0.0, 0.5),
                )),
            ],
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
    fn mouse_pressed(&mut self, button: MouseButton) -> Vec<GuiAction> {
        self.config.request_keyboard_focus = true;
        vec![GuiAction::ResetKeyboardFocus]
    }
    fn char_focus(&mut self, modifiers: ModifiersState, key: char) -> Vec<GuiAction> {
        if !(modifiers.ctrl() || modifiers.alt() || modifiers.logo()) && !key.is_control() {
            let content = &mut self.children[0].try_as_mut::<Label>().unwrap().content;
            let was_empty = content.get_text().is_empty();
            content.text().push(key);
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
                if content.get_text().is_empty() {
                    self.children[1].inner.config_mut().enabled = true;
                }
            }
        }
        vec![]
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
    pub content: Vec<(Content, f32, f32)>,
    /// the position from where content drawing starts.
    /// recalculated when layouting is performed.
    content_pos: Vec2,
    content_height: f32,
}
impl AdvancedLabel {
    pub fn new(config: GuiElemCfg, align: Vec2, content: Vec<(Content, f32, f32)>) -> Self {
        Self {
            config,
            children: vec![],
            align,
            content,
            content_pos: Vec2::ZERO,
            content_height: 0.0,
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
            || self.content.iter().any(|(c, _, _)| c.will_redraw())
        {
            self.config.redraw = false;
            let mut len = 0.0;
            let mut height = 0.0;
            for (c, scale, _) in &self.content {
                let mut size = info
                    .font
                    .layout_text(&c.text, 1.0, TextOptions::new())
                    .size();
                len += size.x * scale;
                if size.y * scale > height {
                    height = size.y * scale;
                }
            }
            if len > 0.0 && height > 0.0 {
                let scale1 = info.pos.width() / len;
                let scale2 = info.pos.height() / height;
                let scale;
                self.content_pos = if scale1 < scale2 {
                    // use all available width
                    scale = scale1;
                    self.content_height = height * scale;
                    let pad = info.pos.height() - self.content_height;
                    Vec2::new(0.0, pad * self.align.y)
                } else {
                    // use all available height
                    scale = scale2;
                    self.content_height = info.pos.height();
                    let pad = info.pos.width() - len * scale;
                    Vec2::new(pad * self.align.x, 0.0)
                };
                for (c, s, _) in &mut self.content {
                    c.formatted = Some(info.font.layout_text(
                        &c.text,
                        scale * (*s),
                        TextOptions::new(),
                    ));
                }
            }
        }
        let pos_y = info.pos.top_left().y + self.content_pos.y;
        let mut pos_x = info.pos.top_left().x + self.content_pos.x;
        for (c, _, h) in &self.content {
            if let Some(f) = &c.formatted {
                let y = pos_y + (self.content_height - f.height()) * h;
                g.draw_text(Vec2::new(pos_x, y), c.color, f);
                pos_x += f.width();
            }
        }
    }
}
