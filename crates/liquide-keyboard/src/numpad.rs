//! Numpad and NumLock handling.
//!
//! Maps numeric keypad keycodes to either numeric characters or navigation
//! keys depending on the NumLock state, following standard XKB behavior.

/// Navigation keys produced by numpad when NumLock is off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NavKey {
    Home,
    End,
    PageUp,
    PageDown,
    Up,
    Down,
    Left,
    Right,
    Insert,
    Delete,
}

/// Output from numpad key translation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumpadOutput {
    /// A character (digit or decimal point).
    Char(char),
    /// A navigation key.
    NavigationKey(NavKey),
    /// Not a numpad key or no output.
    None,
}

/// Standard numpad keycodes (evdev / AT set 1 scancodes).
pub const KP_0: u32 = 82;
pub const KP_1: u32 = 79;
pub const KP_2: u32 = 80;
pub const KP_3: u32 = 81;
pub const KP_4: u32 = 75;
pub const KP_5: u32 = 76;
pub const KP_6: u32 = 77;
pub const KP_7: u32 = 71;
pub const KP_8: u32 = 72;
pub const KP_9: u32 = 73;
pub const KP_DECIMAL: u32 = 83;
pub const KP_ENTER: u32 = 96;
pub const KP_ADD: u32 = 78;
pub const KP_SUBTRACT: u32 = 74;
pub const KP_MULTIPLY: u32 = 55;
pub const KP_DIVIDE: u32 = 98;

/// Tracks the NumLock state and translates numpad keycodes.
#[derive(Debug, Clone)]
pub struct NumpadState {
    /// Whether NumLock is currently active.
    pub num_lock: bool,
}

impl NumpadState {
    /// Create a new numpad state. NumLock defaults to on.
    pub fn new() -> Self {
        Self { num_lock: true }
    }

    /// Create with a specific initial NumLock state.
    pub fn with_num_lock(num_lock: bool) -> Self {
        Self { num_lock }
    }

    /// Toggle NumLock on/off.
    pub fn toggle_num_lock(&mut self) {
        self.num_lock = !self.num_lock;
    }

    /// Translate a numpad keycode to its output.
    ///
    /// When NumLock is on, digit keys produce characters. When off, they
    /// produce navigation keys. Operator keys (+, -, *, /) always produce
    /// characters regardless of NumLock state.
    pub fn translate(&self, keycode: u32) -> NumpadOutput {
        numpad_translate(keycode, self.num_lock)
    }
}

impl Default for NumpadState {
    fn default() -> Self {
        Self::new()
    }
}

/// Translate a numpad keycode given the NumLock state.
///
/// Returns `NumpadOutput::None` for non-numpad keycodes.
pub fn numpad_translate(keycode: u32, num_lock_on: bool) -> NumpadOutput {
    // Operator keys always produce characters.
    match keycode {
        KP_ADD => return NumpadOutput::Char('+'),
        KP_SUBTRACT => return NumpadOutput::Char('-'),
        KP_MULTIPLY => return NumpadOutput::Char('*'),
        KP_DIVIDE => return NumpadOutput::Char('/'),
        KP_ENTER => return NumpadOutput::Char('\n'),
        _ => {}
    }

    if num_lock_on {
        match keycode {
            KP_0 => NumpadOutput::Char('0'),
            KP_1 => NumpadOutput::Char('1'),
            KP_2 => NumpadOutput::Char('2'),
            KP_3 => NumpadOutput::Char('3'),
            KP_4 => NumpadOutput::Char('4'),
            KP_5 => NumpadOutput::Char('5'),
            KP_6 => NumpadOutput::Char('6'),
            KP_7 => NumpadOutput::Char('7'),
            KP_8 => NumpadOutput::Char('8'),
            KP_9 => NumpadOutput::Char('9'),
            KP_DECIMAL => NumpadOutput::Char('.'),
            _ => NumpadOutput::None,
        }
    } else {
        match keycode {
            KP_0 => NumpadOutput::NavigationKey(NavKey::Insert),
            KP_1 => NumpadOutput::NavigationKey(NavKey::End),
            KP_2 => NumpadOutput::NavigationKey(NavKey::Down),
            KP_3 => NumpadOutput::NavigationKey(NavKey::PageDown),
            KP_4 => NumpadOutput::NavigationKey(NavKey::Left),
            KP_5 => NumpadOutput::None, // KP_5 with no NumLock has no standard nav
            KP_6 => NumpadOutput::NavigationKey(NavKey::Right),
            KP_7 => NumpadOutput::NavigationKey(NavKey::Home),
            KP_8 => NumpadOutput::NavigationKey(NavKey::Up),
            KP_9 => NumpadOutput::NavigationKey(NavKey::PageUp),
            KP_DECIMAL => NumpadOutput::NavigationKey(NavKey::Delete),
            _ => NumpadOutput::None,
        }
    }
}
