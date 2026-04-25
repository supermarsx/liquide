//! UI event system for keyboard, mouse, focus, and resize events.

use std::fmt;

/// A mouse button identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    /// Primary (left) button.
    Left,
    /// Secondary (right) button.
    Right,
    /// Middle button (scroll wheel click).
    Middle,
    /// Back navigation button.
    Back,
    /// Forward navigation button.
    Forward,
}

impl fmt::Display for MouseButton {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Left => write!(f, "Left"),
            Self::Right => write!(f, "Right"),
            Self::Middle => write!(f, "Middle"),
            Self::Back => write!(f, "Back"),
            Self::Forward => write!(f, "Forward"),
        }
    }
}

/// Keyboard key codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    // Letters
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,

    // Digits
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,

    // Function keys
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,

    // Special keys
    Enter,
    Escape,
    Tab,
    Space,
    Backspace,
    Delete,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
}

impl fmt::Display for KeyCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

/// Modifier key state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modifiers {
    /// Whether the Shift key is pressed.
    pub shift: bool,
    /// Whether the Ctrl key is pressed.
    pub ctrl: bool,
    /// Whether the Alt key is pressed.
    pub alt: bool,
    /// Whether the Super (OS/Windows/Command) key is pressed.
    pub super_key: bool,
}

impl Modifiers {
    /// No modifiers pressed.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// Whether the Ctrl modifier is active.
    #[must_use]
    pub fn has_ctrl(&self) -> bool {
        self.ctrl
    }

    /// Whether the Shift modifier is active.
    #[must_use]
    pub fn has_shift(&self) -> bool {
        self.shift
    }

    /// Whether the Alt modifier is active.
    #[must_use]
    pub fn has_alt(&self) -> bool {
        self.alt
    }
}

/// A UI event dispatched to widgets.
#[derive(Debug, Clone)]
pub enum UiEvent {
    /// Mouse cursor moved to a position.
    MouseMove {
        /// X coordinate.
        x: f32,
        /// Y coordinate.
        y: f32,
    },
    /// Mouse button pressed.
    MouseDown {
        /// X coordinate.
        x: f32,
        /// Y coordinate.
        y: f32,
        /// Which button was pressed.
        button: MouseButton,
    },
    /// Mouse button released.
    MouseUp {
        /// X coordinate.
        x: f32,
        /// Y coordinate.
        y: f32,
        /// Which button was released.
        button: MouseButton,
    },
    /// Mouse cursor entered the widget.
    MouseEnter,
    /// Mouse cursor left the widget.
    MouseLeave,
    /// Scroll wheel event.
    Scroll {
        /// Horizontal scroll delta.
        dx: f32,
        /// Vertical scroll delta.
        dy: f32,
    },
    /// Key pressed.
    KeyDown {
        /// Key that was pressed.
        key: KeyCode,
        /// Active modifier keys.
        modifiers: Modifiers,
    },
    /// Key released.
    KeyUp {
        /// Key that was released.
        key: KeyCode,
        /// Active modifier keys.
        modifiers: Modifiers,
    },
    /// Text input from the keyboard (after IME processing).
    TextInput {
        /// The input text.
        text: String,
    },
    /// Widget gained focus.
    FocusIn,
    /// Widget lost focus.
    FocusOut,
    /// Widget was resized.
    Resize {
        /// New width.
        width: f32,
        /// New height.
        height: f32,
    },
}

/// Event propagation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventPropagation {
    /// Event bubbles up from child to parent.
    Bubble,
    /// Event is captured from parent down to child.
    Capture,
    /// Event is delivered directly to the target widget only.
    Direct,
}
