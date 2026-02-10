//! Unified input event types.

use serde::{Deserialize, Serialize};

use crate::keyboard::KeyEvent;
use crate::mouse::MouseEvent;
use crate::touch::TouchEvent;

/// A unified input event.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum InputEvent {
    Keyboard(KeyEvent),
    Mouse(MouseEvent),
    Touch(TouchEvent),
}

impl InputEvent {
    /// Get the timestamp of this event in microseconds.
    #[must_use]
    pub fn timestamp_us(&self) -> u64 {
        match self {
            Self::Keyboard(e) => e.timestamp_us,
            Self::Mouse(_) => 0, // Mouse events don't carry timestamps individually
            Self::Touch(e) => e.timestamp_us,
        }
    }

    /// Check if this is a keyboard event.
    #[must_use]
    pub fn is_keyboard(&self) -> bool {
        matches!(self, Self::Keyboard(_))
    }

    /// Check if this is a mouse event.
    #[must_use]
    pub fn is_mouse(&self) -> bool {
        matches!(self, Self::Mouse(_))
    }

    /// Check if this is a touch event.
    #[must_use]
    pub fn is_touch(&self) -> bool {
        matches!(self, Self::Touch(_))
    }
}

impl std::fmt::Display for InputEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Keyboard(ke) => write!(f, "Key({} {})", ke.key, ke.state),
            Self::Mouse(me) => match me {
                crate::mouse::MouseEvent::Move { x, y } => write!(f, "MouseMove({x}, {y})"),
                crate::mouse::MouseEvent::Button { button, state, .. } => write!(f, "MouseButton({button} {state})"),
                crate::mouse::MouseEvent::Scroll { axis, delta, .. } => write!(f, "Scroll({axis:?} {delta})"),
                crate::mouse::MouseEvent::Enter { x, y } => write!(f, "MouseEnter({x}, {y})"),
                crate::mouse::MouseEvent::Leave => write!(f, "MouseLeave"),
            },
            Self::Touch(te) => write!(f, "Touch({} id={})", te.phase, te.point.id),
        }
    }
}

/// Identifies where an input event came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventSource {
    pub surface_id: u64,
    pub device_id: u32,
}

impl EventSource {
    /// Create a new event source.
    #[must_use]
    pub fn new(surface_id: u64, device_id: u32) -> Self {
        Self { surface_id, device_id }
    }
}

/// A complete input packet with source and sequence info.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct InputPacket {
    pub event: InputEvent,
    pub source: EventSource,
    pub sequence: u64,
}

impl InputPacket {
    /// Create a new input packet.
    #[must_use]
    pub fn new(event: InputEvent, source: EventSource, sequence: u64) -> Self {
        Self { event, source, sequence }
    }
}
