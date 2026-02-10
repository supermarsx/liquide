//! Keyboard types: key codes, modifiers, key events.

use serde::{Deserialize, Serialize};

/// Physical/logical key code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeyCode {
    A, B, C, D, E, F, G, H, I, J, K, L, M,
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    Digit0, Digit1, Digit2, Digit3, Digit4,
    Digit5, Digit6, Digit7, Digit8, Digit9,
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    Escape, Enter, Tab, Backspace, Space,
    ArrowUp, ArrowDown, ArrowLeft, ArrowRight,
    Home, End, PageUp, PageDown,
    Insert, Delete,
    CapsLock, NumLock, ScrollLock,
    PrintScreen, Pause,
    LeftShift, RightShift,
    LeftCtrl, RightCtrl,
    LeftAlt, RightAlt,
    LeftSuper, RightSuper,
    Comma, Period, Slash, Semicolon, Quote,
    BracketLeft, BracketRight, Backslash,
    Minus, Equal, Grave,
    ContextMenu,
}

/// Modifier key flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Modifiers(u8);

impl Modifiers {
    pub const SHIFT: u8 = 0x01;
    pub const CTRL: u8 = 0x02;
    pub const ALT: u8 = 0x04;
    pub const SUPER: u8 = 0x08;
    pub const CAPS_LOCK: u8 = 0x10;
    pub const NUM_LOCK: u8 = 0x20;

    /// Create empty modifiers.
    #[must_use]
    pub fn new() -> Self {
        Self(0)
    }

    /// Create modifiers from raw bits.
    #[must_use]
    pub fn from_bits(bits: u8) -> Self {
        Self(bits)
    }

    /// Get raw bits.
    #[must_use]
    pub fn bits(self) -> u8 {
        self.0
    }

    /// Check if shift is active.
    #[must_use]
    pub fn shift(self) -> bool {
        self.0 & Self::SHIFT != 0
    }

    /// Check if ctrl is active.
    #[must_use]
    pub fn ctrl(self) -> bool {
        self.0 & Self::CTRL != 0
    }

    /// Check if alt is active.
    #[must_use]
    pub fn alt(self) -> bool {
        self.0 & Self::ALT != 0
    }

    /// Check if super/meta key is active.
    #[must_use]
    pub fn super_key(self) -> bool {
        self.0 & Self::SUPER != 0
    }

    /// Check if no modifiers are active.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Check if this contains a specific modifier flag.
    #[must_use]
    pub fn contains(self, flag: u8) -> bool {
        self.0 & flag != 0
    }
}

impl std::ops::BitOr for Modifiers {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for Modifiers {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl std::ops::BitAnd for Modifiers {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl std::fmt::Display for KeyCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::fmt::Display for KeyState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pressed => write!(f, "pressed"),
            Self::Released => write!(f, "released"),
            Self::Repeat => write!(f, "repeat"),
        }
    }
}

impl std::fmt::Display for Modifiers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut parts = Vec::new();
        if self.shift() { parts.push("Shift"); }
        if self.ctrl() { parts.push("Ctrl"); }
        if self.alt() { parts.push("Alt"); }
        if self.super_key() { parts.push("Super"); }
        if self.contains(Self::CAPS_LOCK) { parts.push("CapsLock"); }
        if self.contains(Self::NUM_LOCK) { parts.push("NumLock"); }
        if parts.is_empty() {
            write!(f, "(none)")
        } else {
            write!(f, "{}", parts.join("+"))
        }
    }
}

/// Key press state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeyState {
    Pressed,
    Released,
    Repeat,
}

/// A keyboard event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyEvent {
    pub key: KeyCode,
    pub state: KeyState,
    pub modifiers: Modifiers,
    pub scancode: u32,
    pub timestamp_us: u64,
}

impl KeyEvent {
    /// Create a new key event.
    #[must_use]
    pub fn new(key: KeyCode, state: KeyState, modifiers: Modifiers, scancode: u32, timestamp_us: u64) -> Self {
        Self { key, state, modifiers, scancode, timestamp_us }
    }
}
