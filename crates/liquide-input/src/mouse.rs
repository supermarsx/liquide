//! Mouse/pointer types: buttons, scroll, movement events.

use serde::{Deserialize, Serialize};

/// Mouse button identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
    Other(u8),
}

/// Button press state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ButtonState {
    Pressed,
    Released,
}

/// Scroll axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ScrollAxis {
    Vertical,
    Horizontal,
}

/// A mouse/pointer event.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum MouseEvent {
    Move {
        x: f32,
        y: f32,
    },
    Button {
        button: MouseButton,
        state: ButtonState,
        x: f32,
        y: f32,
    },
    Scroll {
        axis: ScrollAxis,
        delta: f32,
        x: f32,
        y: f32,
    },
    Enter {
        x: f32,
        y: f32,
    },
    Leave,
}

impl std::fmt::Display for MouseButton {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Left => write!(f, "Left"),
            Self::Right => write!(f, "Right"),
            Self::Middle => write!(f, "Middle"),
            Self::Back => write!(f, "Back"),
            Self::Forward => write!(f, "Forward"),
            Self::Other(n) => write!(f, "Button({n})"),
        }
    }
}

impl std::fmt::Display for ButtonState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pressed => write!(f, "pressed"),
            Self::Released => write!(f, "released"),
        }
    }
}
