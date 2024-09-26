use std::{fmt::Display, rc::Rc, sync::Arc};

use musicdb_lib::data::CoverId;
use speedy2d::{
    color::Color,
    dimen::Vec2,
    font::{FormattedTextBlock, TextLayout, TextOptions},
    image::ImageHandle,
    shape::Rectangle,
    window::{ModifiersState, MouseButton},
};

use crate::gui::{EventInfo, GuiAction, GuiElem, GuiElemCfg, GuiServerImage};

/*

Some basic structs to use everywhere,
except they are all text-related.

*/

#[derive(Clone)]
pub struct Label {
    config: GuiElemCfg,
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

#[allow(unused)]
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
impl GuiElem for Label {
    fn config(&self) -> &GuiElemCfg {
        &self.config
    }
    fn config_mut(&mut self) -> &mut GuiElemCfg {
        &mut self.config
    }
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut dyn GuiElem> + '_> {
        Box::new([].into_iter())
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
    pub c_input: Label,
    pub c_hint: Label,
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
            c_input: Label::new(
                GuiElemCfg::default(),
                text,
                color_input,
                None,
                Vec2::new(0.0, 0.5),
            ),
            c_hint: Label::new(
                if text_is_empty {
                    GuiElemCfg::default()
                } else {
                    GuiElemCfg::default().disabled()
                },
                hint,
                color_hint,
                None,
                Vec2::new(0.0, 0.5),
            ),
            on_changed: None,
            on_changed_mut: None,
        }
    }
}
impl GuiElem for TextField {
    fn config(&self) -> &GuiElemCfg {
        &self.config
    }
    fn config_mut(&mut self) -> &mut GuiElemCfg {
        &mut self.config
    }
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut dyn GuiElem> + '_> {
        Box::new([self.c_input.elem_mut(), self.c_hint.elem_mut()].into_iter())
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
    fn mouse_pressed(&mut self, e: &mut EventInfo, _button: MouseButton) -> Vec<GuiAction> {
        if e.take() {
            self.config.request_keyboard_focus = true;
            vec![GuiAction::ResetKeyboardFocus]
        } else {
            vec![]
        }
    }
    fn char_focus(
        &mut self,
        e: &mut EventInfo,
        modifiers: ModifiersState,
        key: char,
    ) -> Vec<GuiAction> {
        if !(modifiers.ctrl() || modifiers.alt() || modifiers.logo())
            && !key.is_control()
            && e.take()
        {
            let content = &mut self.c_input.content;
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
                self.c_hint.config_mut().enabled = false;
            }
        }
        vec![]
    }
    fn key_focus(
        &mut self,
        e: &mut EventInfo,
        modifiers: ModifiersState,
        down: bool,
        key: Option<speedy2d::window::VirtualKeyCode>,
        _scan: speedy2d::window::KeyScancode,
    ) -> Vec<GuiAction> {
        if down
            && !(modifiers.alt() || modifiers.logo())
            && key == Some(speedy2d::window::VirtualKeyCode::Backspace)
            && e.take()
        {
            let content = &mut self.c_input.content;
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
                    self.c_hint.config_mut().enabled = true;
                }
            }
        }
        vec![]
    }
}

#[derive(Clone)]
pub enum AdvancedContent {
    Text(Content),
    Image {
        source: ImageSource,
        handle: Option<Option<ImageHandle>>,
    },
}
#[derive(Clone)]
pub enum ImageSource {
    Cover(CoverId),
    CustomFile(String),
}
impl AdvancedContent {
    pub fn will_redraw(&self) -> bool {
        match self {
            Self::Text(c) => c.will_redraw(),
            Self::Image { source: _, handle } => handle.is_none(),
        }
    }
}
impl Display for AdvancedContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text(c) => write!(f, "{}", c.text),
            Self::Image { .. } => Ok(()),
        }
    }
}

/// More advanced version of `Label`.
/// Allows stringing together multiple `Content`s in one line.
pub struct AdvancedLabel {
    config: GuiElemCfg,
    children: Vec<Box<dyn GuiElem>>,
    /// 0.0 => align to top/left
    /// 0.5 => center
    /// 1.0 => align to bottom/right
    pub align: Vec2,
    /// (Content, Size-Scale, Height)
    /// Size-Scale and Height should default to 1.0.
    pub content: Vec<Vec<(AdvancedContent, f32, f32)>>,
    /// the position from where content drawing starts.
    /// recalculated when layouting is performed.
    content_pos: Vec2,
}
impl AdvancedLabel {
    pub fn new(
        config: GuiElemCfg,
        align: Vec2,
        content: Vec<Vec<(AdvancedContent, f32, f32)>>,
    ) -> Self {
        Self {
            config,
            children: vec![],
            align,
            content,
            content_pos: Vec2::ZERO,
        }
    }
}
impl GuiElem for AdvancedLabel {
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
                    match c {
                        AdvancedContent::Text(c) => {
                            let size = info
                                .font
                                .layout_text(&c.text, 1.0, TextOptions::new())
                                .size();
                            len += size.x * scale;
                            if size.y * scale > height {
                                height = size.y * scale;
                            }
                        }
                        AdvancedContent::Image { source, handle } => {}
                    }
                }
                for (c, scale, _) in line {
                    match c {
                        AdvancedContent::Text(_) => {}
                        AdvancedContent::Image { source, handle } => {
                            if let Some(Some(handle)) = handle {
                                let size = handle.size().into_f32();
                                len += height * size.x / size.y;
                            }
                        }
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
                        match c {
                            AdvancedContent::Text(c) => {
                                c.formatted = Some(info.font.layout_text(
                                    &c.text,
                                    scale * (*s),
                                    TextOptions::new(),
                                ));
                            }
                            AdvancedContent::Image { source, handle } => {
                                if handle.is_none() {
                                    match source {
                                        ImageSource::Cover(id) => {
                                            if let Some(img) = info.covers.get_mut(&id) {
                                                if let Some(img) = img.get_init(g) {
                                                    *handle = Some(Some(img));
                                                } else {
                                                    match img {
                                                        GuiServerImage::Loading(_) => {}
                                                        GuiServerImage::Loaded(_) => {}
                                                        GuiServerImage::Error => {
                                                            *handle = Some(None)
                                                        }
                                                    }
                                                }
                                            } else {
                                                info.covers.insert(
                                                    *id,
                                                    GuiServerImage::new_cover(
                                                        *id,
                                                        Arc::clone(&info.get_con),
                                                    ),
                                                );
                                            }
                                        }
                                        ImageSource::CustomFile(path) => {
                                            if let Some(img) = info.custom_images.get_mut(path) {
                                                if let Some(img) = img.get_init(g) {
                                                    *handle = Some(Some(img));
                                                } else {
                                                    match img {
                                                        GuiServerImage::Loading(_) => {}
                                                        GuiServerImage::Loaded(_) => {}
                                                        GuiServerImage::Error => {
                                                            *handle = Some(None)
                                                        }
                                                    }
                                                }
                                            } else {
                                                info.custom_images.insert(
                                                    path.clone(),
                                                    GuiServerImage::new_custom_file(
                                                        path.clone(),
                                                        Arc::clone(&info.get_con),
                                                    ),
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        let pos_x_start = info.pos.top_left().x + self.content_pos.x;
        let mut pos_y = info.pos.top_left().y + self.content_pos.y;
        for line in &self.content {
            let mut pos_x = pos_x_start;
            let height_div_by = line
                .iter()
                .map(|(_, scale, _)| *scale)
                .reduce(f32::max)
                .unwrap_or(1.0);
            let line_height = line
                .iter()
                .filter_map(|(v, _, _)| {
                    if let AdvancedContent::Text(c) = v {
                        Some(c)
                    } else {
                        None
                    }
                })
                .filter_map(|v| v.formatted.as_ref())
                .map(|f| f.height())
                .reduce(f32::max)
                .unwrap_or(0.0);
            for (c, scale, placement_height) in line {
                // not super accurate, but pretty good
                let rel_scale = f32::min(1.0, scale / height_div_by);
                match c {
                    AdvancedContent::Text(c) => {
                        if let Some(f) = &c.formatted {
                            let y = pos_y + (line_height - f.height()) * placement_height;
                            g.draw_text(Vec2::new(pos_x, y), c.color, f);
                            pos_x += f.width();
                        }
                    }
                    AdvancedContent::Image { source: _, handle } => {
                        if let Some(Some(handle)) = handle {
                            let size = handle.size().into_f32();
                            let h = line_height * rel_scale;
                            let w = h * size.x / size.y;
                            let y = pos_y + (line_height - h) * placement_height;
                            g.draw_rectangle_image(
                                Rectangle::from_tuples((pos_x, y), (pos_x + w, y + h)),
                                handle,
                            );
                            pos_x += w;
                        }
                    }
                }
            }
            pos_y += line_height;
        }
    }
}
