//! Horizontal slot positioning within the status bar.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Horizontal slot within the status bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StatusBarSlot {
    Left,
    Center,
    Right,
}

impl fmt::Display for StatusBarSlot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Left => write!(f, "Left"),
            Self::Center => write!(f, "Center"),
            Self::Right => write!(f, "Right"),
        }
    }
}
