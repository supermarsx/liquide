//! Touch input types.

use serde::{Deserialize, Serialize};

/// Phase of a touch event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TouchPhase {
    Begin,
    Move,
    End,
    Cancel,
}

/// A single touch contact point.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TouchPoint {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub pressure: f32,
}

impl TouchPoint {
    /// Create a new touch point.
    #[must_use]
    pub fn new(id: u32, x: f32, y: f32, pressure: f32) -> Self {
        Self { id, x, y, pressure }
    }
}

/// A touch event.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TouchEvent {
    pub phase: TouchPhase,
    pub point: TouchPoint,
    pub timestamp_us: u64,
}

impl TouchEvent {
    /// Create a new touch event.
    #[must_use]
    pub fn new(phase: TouchPhase, point: TouchPoint, timestamp_us: u64) -> Self {
        Self { phase, point, timestamp_us }
    }
}

impl std::fmt::Display for TouchPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Begin => write!(f, "begin"),
            Self::Move => write!(f, "move"),
            Self::End => write!(f, "end"),
            Self::Cancel => write!(f, "cancel"),
        }
    }
}
