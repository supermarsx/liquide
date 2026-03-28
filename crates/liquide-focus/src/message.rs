//! Window messages — a typed, platform-agnostic message vocabulary.
//!
//! Every interaction between the desktop shell and a window is expressed as a
//! [`WindowMessage`].  Messages carry just enough data to describe the event
//! without binding to any particular platform ABI.

use serde::{Deserialize, Serialize};

use crate::types::WindowId;

/// Mouse button identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    X1,
    X2,
}

/// Keyboard modifier flags (bitmask-style but stored as a struct for clarity).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
}

impl Modifiers {
    /// No modifiers held.
    pub const NONE: Self = Self {
        shift: false,
        ctrl: false,
        alt: false,
        meta: false,
    };

    /// Returns `true` when no modifier keys are held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        !self.shift && !self.ctrl && !self.alt && !self.meta
    }
}

/// Min/max size constraints reported by a window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MinMaxInfo {
    pub min_width: u32,
    pub min_height: u32,
    pub max_width: u32,
    pub max_height: u32,
}

impl Default for MinMaxInfo {
    fn default() -> Self {
        Self {
            min_width: 0,
            min_height: 0,
            max_width: u32::MAX,
            max_height: u32::MAX,
        }
    }
}

/// A window-level message.
///
/// Messages are the sole communication channel between the desktop shell and
/// individual window instances.  They cover lifecycle, input, paint, drag-drop,
/// and configuration changes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WindowMessage {
    // -- Lifecycle --
    /// The window has been created.
    Created,
    /// Request the window to paint itself.
    Paint,
    /// The window should close (user or programmatic).
    Close,
    /// Final tear-down — the window is being destroyed.
    Destroy,
    /// Make the window visible.
    Show,
    /// Hide the window.
    Hide,

    // -- Activation / focus --
    /// The window is being activated (brought to foreground).
    Activate,
    /// The window is being deactivated.
    Deactivate,
    /// Keyboard focus gained.
    FocusGained,
    /// Keyboard focus lost.
    FocusLost,

    // -- Geometry --
    /// The window has been resized to (width, height) in logical pixels.
    Resize { width: u32, height: u32 },
    /// The window has been moved to (x, y) in desktop coordinates.
    Move { x: i32, y: i32 },
    /// Combined resize + reposition (e.g. tiling layout change).
    Configure { width: u32, height: u32 },
    /// Query the window for its size constraints.
    MinMaxInfo,

    // -- Keyboard --
    /// A key was pressed.
    KeyDown { keycode: u32, modifiers: Modifiers },
    /// A key was released.
    KeyUp { keycode: u32, modifiers: Modifiers },
    /// A character has been composed (after dead-key / IME processing).
    CharInput(char),

    // -- Mouse --
    /// The pointer moved to (x, y) relative to the window's client area.
    MouseMove { x: f64, y: f64 },
    /// A mouse button was pressed at (x, y).
    MouseDown { button: MouseButton, x: f64, y: f64 },
    /// A mouse button was released at (x, y).
    MouseUp { button: MouseButton, x: f64, y: f64 },
    /// The scroll wheel moved by `delta` (positive = up/right).
    MouseWheel { delta: f64 },
    /// The pointer entered the window's client area.
    MouseEnter,
    /// The pointer left the window's client area.
    MouseLeave,

    // -- Drag and drop --
    /// A drag operation entered the window.
    DragEnter,
    /// A drag operation is hovering over the window.
    DragOver,
    /// A drag operation left the window.
    DragLeave,
    /// A drop occurred on the window.
    Drop,

    // -- Timers --
    /// A timer with the given ID has fired.
    Timer(u64),

    // -- Environment --
    /// The desktop theme has changed.
    ThemeChanged,
    /// The display DPI / scale factor changed.
    DpiChanged { scale: f64 },
    /// The window's visual style properties have changed.
    StyleChanged,
}

impl WindowMessage {
    /// Returns `true` for messages that relate to pointer input.
    #[must_use]
    pub fn is_mouse(&self) -> bool {
        matches!(
            self,
            Self::MouseMove { .. }
                | Self::MouseDown { .. }
                | Self::MouseUp { .. }
                | Self::MouseWheel { .. }
                | Self::MouseEnter
                | Self::MouseLeave
        )
    }

    /// Returns `true` for messages that relate to keyboard input.
    #[must_use]
    pub fn is_keyboard(&self) -> bool {
        matches!(
            self,
            Self::KeyDown { .. } | Self::KeyUp { .. } | Self::CharInput(_)
        )
    }

    /// Returns `true` for lifecycle messages (Created, Close, Destroy, Show, Hide).
    #[must_use]
    pub fn is_lifecycle(&self) -> bool {
        matches!(
            self,
            Self::Created | Self::Close | Self::Destroy | Self::Show | Self::Hide
        )
    }

    /// Returns `true` for drag-and-drop messages.
    #[must_use]
    pub fn is_drag_drop(&self) -> bool {
        matches!(
            self,
            Self::DragEnter | Self::DragOver | Self::DragLeave | Self::Drop
        )
    }
}

/// Message dispatch priority.
///
/// The message queue drains `High` messages before `Normal`, and `Normal`
/// before `Low`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum MessagePriority {
    /// Processed first (e.g. input, timer).
    High = 0,
    /// Default priority.
    Normal = 1,
    /// Background / deferred work.
    Low = 2,
}

impl Default for MessagePriority {
    fn default() -> Self {
        Self::Normal
    }
}

/// A message addressed to a specific window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageTarget {
    /// The window that should receive this message.
    pub window_id: WindowId,
    /// The message itself.
    pub message: WindowMessage,
    /// Dispatch priority.
    pub priority: MessagePriority,
}

impl MessageTarget {
    /// Convenience constructor with `Normal` priority.
    #[must_use]
    pub fn new(window_id: WindowId, message: WindowMessage) -> Self {
        Self {
            window_id,
            message,
            priority: MessagePriority::Normal,
        }
    }

    /// Convenience constructor with explicit priority.
    #[must_use]
    pub fn with_priority(
        window_id: WindowId,
        message: WindowMessage,
        priority: MessagePriority,
    ) -> Self {
        Self {
            window_id,
            message,
            priority,
        }
    }
}
