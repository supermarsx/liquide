//! Event types for UI interaction.

use std::fmt;

/// Mouse button identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Keyboard modifier keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub super_key: bool,
}

impl Modifiers {
    pub const NONE: Self = Self {
        shift: false,
        ctrl: false,
        alt: false,
        super_key: false,
    };
}

/// Keyboard key representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    Char(char),
    Enter,
    Escape,
    Tab,
    Backspace,
    Delete,
    Space,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Home,
    End,
    PageUp,
    PageDown,
    F(u8), // F1–F12
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Key::Char(c) => write!(f, "{c}"),
            Key::F(n) => write!(f, "F{n}"),
            other => write!(f, "{other:?}"),
        }
    }
}

/// A UI event dispatched to widgets.
#[derive(Debug, Clone)]
pub enum Event {
    /// Mouse moved to (x, y) in widget-local coordinates.
    MouseMove { x: f32, y: f32 },
    /// Mouse button pressed.
    MouseDown { x: f32, y: f32, button: MouseButton },
    /// Mouse button released.
    MouseUp { x: f32, y: f32, button: MouseButton },
    /// Mouse entered the widget bounds.
    MouseEnter,
    /// Mouse left the widget bounds.
    MouseLeave,
    /// Scroll wheel.
    Scroll { dx: f32, dy: f32 },
    /// Key pressed.
    KeyDown { key: Key, modifiers: Modifiers },
    /// Key released.
    KeyUp { key: Key, modifiers: Modifiers },
    /// Text input from IME / character entry.
    TextInput { text: String },
    /// Widget gained keyboard focus.
    FocusIn,
    /// Widget lost keyboard focus.
    FocusOut,
    /// Widget was resized.
    Resize { width: f32, height: f32 },
}

/// How a widget responds to an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventResponse {
    /// Event was consumed — stop propagation.
    Consumed,
    /// Event was not handled — continue bubbling.
    Ignored,
    /// Request focus for this widget.
    RequestFocus,
    /// Release focus from this widget.
    ReleaseFocus,
}
