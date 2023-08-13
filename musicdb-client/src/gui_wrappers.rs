use speedy2d::{
    window::{MouseButton, VirtualKeyCode},
    Graphics2D,
};

use crate::gui::{DrawInfo, GuiAction, GuiElem, GuiElemCfg, GuiElemTrait};

#[derive(Clone)]
pub struct WithFocusHotkey<T: GuiElemTrait + Clone> {
    pub inner: T,
    /// 4 * (ignore, pressed): 10 or 11 -> doesn't matter, 01 -> must be pressed, 00 -> must not be pressed
    /// logo alt shift ctrl
    pub modifiers: u8,
    pub key: VirtualKeyCode,
}
impl<T: GuiElemTrait + Clone> WithFocusHotkey<T> {
    /// unlike noshift, this ignores the shift modifier
    pub fn new_key(key: VirtualKeyCode, inner: T) -> WithFocusHotkey<T> {
        Self::new(0b1000, key, inner)
    }
    /// requires the key to be pressed without any modifiers
    pub fn new_noshift(key: VirtualKeyCode, inner: T) -> WithFocusHotkey<T> {
        Self::new(0, key, inner)
    }
    pub fn new_shift(key: VirtualKeyCode, inner: T) -> WithFocusHotkey<T> {
        Self::new(0b0100, key, inner)
    }
    pub fn new_ctrl(key: VirtualKeyCode, inner: T) -> WithFocusHotkey<T> {
        Self::new(0b01, key, inner)
    }
    pub fn new_ctrl_shift(key: VirtualKeyCode, inner: T) -> WithFocusHotkey<T> {
        Self::new(0b0101, key, inner)
    }
    pub fn new_alt(key: VirtualKeyCode, inner: T) -> WithFocusHotkey<T> {
        Self::new(0b010000, key, inner)
    }
    pub fn new_alt_shift(key: VirtualKeyCode, inner: T) -> WithFocusHotkey<T> {
        Self::new(0b010100, key, inner)
    }
    pub fn new_ctrl_alt(key: VirtualKeyCode, inner: T) -> WithFocusHotkey<T> {
        Self::new(0b010001, key, inner)
    }
    pub fn new_ctrl_alt_shift(key: VirtualKeyCode, inner: T) -> WithFocusHotkey<T> {
        Self::new(0b010101, key, inner)
    }
    pub fn new(modifiers: u8, key: VirtualKeyCode, mut inner: T) -> WithFocusHotkey<T> {
        inner.config_mut().keyboard_events_watch = true;
        WithFocusHotkey {
            inner,
            modifiers,
            key,
        }
    }
}
impl<T: Clone + 'static> GuiElemTrait for WithFocusHotkey<T>
where
    T: GuiElemTrait,
{
    fn config(&self) -> &GuiElemCfg {
        self.inner.config()
    }
    fn config_mut(&mut self) -> &mut GuiElemCfg {
        self.inner.config_mut()
    }
    fn children(&mut self) -> Box<dyn Iterator<Item = &mut GuiElem> + '_> {
        self.inner.children()
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
        self.inner.draw(info, g)
    }
    fn mouse_down(&mut self, button: MouseButton) -> Vec<GuiAction> {
        self.inner.mouse_down(button)
    }
    fn mouse_up(&mut self, button: MouseButton) -> Vec<GuiAction> {
        self.inner.mouse_up(button)
    }
    fn mouse_pressed(&mut self, button: MouseButton) -> Vec<GuiAction> {
        self.inner.mouse_pressed(button)
    }
    fn mouse_wheel(&mut self, diff: f32) -> Vec<GuiAction> {
        self.inner.mouse_wheel(diff)
    }
    fn char_watch(
        &mut self,
        modifiers: speedy2d::window::ModifiersState,
        key: char,
    ) -> Vec<GuiAction> {
        self.inner.char_watch(modifiers, key)
    }
    fn char_focus(
        &mut self,
        modifiers: speedy2d::window::ModifiersState,
        key: char,
    ) -> Vec<GuiAction> {
        self.inner.char_focus(modifiers, key)
    }
    fn key_watch(
        &mut self,
        modifiers: speedy2d::window::ModifiersState,
        down: bool,
        key: Option<speedy2d::window::VirtualKeyCode>,
        scan: speedy2d::window::KeyScancode,
    ) -> Vec<GuiAction> {
        let hotkey = down == false
            && key.is_some_and(|v| v == self.key)
            && (self.modifiers & 0b10 == 1 || (self.modifiers & 0b01 == 1) == modifiers.ctrl())
            && (self.modifiers & 0b1000 == 1
                || (self.modifiers & 0b0100 == 1) == modifiers.shift())
            && (self.modifiers & 0b100000 == 1
                || (self.modifiers & 0b010000 == 1) == modifiers.alt())
            && (self.modifiers & 0b10000000 == 1
                || (self.modifiers & 0b01000000 == 1) == modifiers.logo());
        let mut o = self.inner.key_watch(modifiers, down, key, scan);
        if hotkey {
            self.config_mut().request_keyboard_focus = true;
            o.push(GuiAction::ResetKeyboardFocus);
        }
        o
    }
    fn key_focus(
        &mut self,
        modifiers: speedy2d::window::ModifiersState,
        down: bool,
        key: Option<speedy2d::window::VirtualKeyCode>,
        scan: speedy2d::window::KeyScancode,
    ) -> Vec<GuiAction> {
        self.inner.key_focus(modifiers, down, key, scan)
    }
    fn dragged(&mut self, dragged: crate::gui::Dragging) -> Vec<GuiAction> {
        self.inner.dragged(dragged)
    }
    fn updated_library(&mut self) {
        self.inner.updated_library()
    }
    fn updated_queue(&mut self) {
        self.inner.updated_queue()
    }
}
