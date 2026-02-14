//! Inter-thread messages for the render architecture.

use serde::{Deserialize, Serialize};

/// Monotonic frame identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FrameId(pub u64);

impl FrameId {
    #[must_use]
    pub fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

/// Damage rectangle.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DamageRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Messages sent to the chrome rendering thread.
#[derive(Debug, Clone)]
pub enum ChromeMessage {
    /// Render a new frame of window chrome.
    RenderFrame {
        frame_id: FrameId,
        width: u32,
        height: u32,
        damage: Vec<DamageRect>,
    },
    /// Window was resized.
    Resize { width: u32, height: u32 },
    /// Update window title.
    SetTitle { title: String },
    /// Theme changed.
    ThemeChanged,
    /// Shutdown the chrome thread.
    Shutdown,
}

/// Messages sent to the content rendering thread.
#[derive(Debug, Clone)]
pub enum ContentMessage {
    /// Render content for a frame.
    RenderFrame {
        frame_id: FrameId,
        viewport_width: u32,
        viewport_height: u32,
        damage: Vec<DamageRect>,
    },
    /// Scroll position changed.
    Scroll { x: f64, y: f64 },
    /// Content invalidated (needs full repaint).
    Invalidate,
    /// Shutdown the content thread.
    Shutdown,
}

/// Frame completion notification.
#[derive(Debug, Clone)]
pub struct FrameComplete {
    pub frame_id: FrameId,
    /// Rendering time in microseconds.
    pub render_time_us: u64,
    /// Whether the frame was dropped (rendered too late).
    pub dropped: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_id() {
        let f = FrameId(0);
        assert_eq!(f.next(), FrameId(1));
        assert!(FrameId(2) > FrameId(1));
    }
}
