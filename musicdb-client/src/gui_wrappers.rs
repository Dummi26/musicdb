use speedy2d::window::VirtualKeyCode;

/// requires `keyboard_events_watch = true`
pub struct Hotkey {
    /// 4 * (ignore, pressed): 10 (or 11, but 0b11111111 -> never) -> doesn't matter, 01 -> must be pressed, 00 -> must not be pressed
    /// logo alt shift ctrl
    pub modifiers: u8,
    pub key: VirtualKeyCode,
}
#[allow(unused)]
impl Hotkey {
    pub fn triggered(
        &self,
        modifiers: speedy2d::window::ModifiersState,
        down: bool,
        key: Option<speedy2d::window::VirtualKeyCode>,
    ) -> bool {
        if self.modifiers == u8::MAX {
            return false;
        }
        down == false
            && key.is_some_and(|v| v == self.key)
            && (self.modifiers & 0b10 == 1 || (self.modifiers & 0b01 == 1) == modifiers.ctrl())
            && (self.modifiers & 0b1000 == 1 || (self.modifiers & 0b0100 == 1) == modifiers.shift())
            && (self.modifiers & 0b100000 == 1
                || (self.modifiers & 0b010000 == 1) == modifiers.alt())
            && (self.modifiers & 0b10000000 == 1
                || (self.modifiers & 0b01000000 == 1) == modifiers.logo())
    }
    /// unlike noshift, this ignores the shift modifier
    pub fn new_key(key: VirtualKeyCode) -> Self {
        Self::new(0b1000, key)
    }
    /// requires the key to be pressed without any modifiers
    pub fn new_noshift(key: VirtualKeyCode) -> Self {
        Self::new(0, key)
    }
    pub fn new_shift(key: VirtualKeyCode) -> Self {
        Self::new(0b0100, key)
    }
    pub fn new_ctrl(key: VirtualKeyCode) -> Self {
        Self::new(0b01, key)
    }
    pub fn new_ctrl_shift(key: VirtualKeyCode) -> Self {
        Self::new(0b0101, key)
    }
    pub fn new_alt(key: VirtualKeyCode) -> Self {
        Self::new(0b010000, key)
    }
    pub fn new_alt_shift(key: VirtualKeyCode) -> Self {
        Self::new(0b010100, key)
    }
    pub fn new_ctrl_alt(key: VirtualKeyCode) -> Self {
        Self::new(0b010001, key)
    }
    pub fn new_ctrl_alt_shift(key: VirtualKeyCode) -> Self {
        Self::new(0b010101, key)
    }
    pub fn new(modifiers: u8, key: VirtualKeyCode) -> Self {
        Hotkey { modifiers, key }
    }
}
