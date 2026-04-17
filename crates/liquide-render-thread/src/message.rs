//! Inter-thread messages for the render architecture.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use liquide_compositor::scene::FlatNode;

/// Monotonic frame identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FrameId(pub u64);

impl FrameId {
    #[must_use]
    pub fn next(self) -> Self {
        Self(self.0.wrapping_add(1))
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
pub enum ChromeMessage {
    /// Render a new frame of window chrome.
    RenderFrame {
        frame_id: FrameId,
        width: u32,
        height: u32,
        damage: Vec<DamageRect>,
        /// Flattened scene nodes for the chrome region (decorations).
        nodes: Vec<FlatNode>,
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
pub enum ContentMessage {
    /// Render content for a frame.
    RenderFrame {
        frame_id: FrameId,
        viewport_width: u32,
        viewport_height: u32,
        damage: Vec<DamageRect>,
        /// Flattened scene nodes for the content region.
        nodes: Vec<FlatNode>,
    },
    /// Scroll position changed.
    Scroll { x: f64, y: f64 },
    /// Content invalidated (needs full repaint).
    Invalidate,
    /// Shutdown the content thread.
    Shutdown,
}

/// Frame completion notification.
#[derive(Clone)]
pub struct FrameComplete {
    pub frame_id: FrameId,
    /// Rendering time in microseconds.
    pub render_time_us: u64,
    /// Whether the frame was dropped (rendered too late).
    pub dropped: bool,
    /// Rendered pixel data (BGRA8, may be None if no rendering occurred).
    pub pixels: Option<Arc<Vec<u8>>>,
    /// Width of the rendered framebuffer.
    pub width: u32,
    /// Height of the rendered framebuffer.
    pub height: u32,
    /// Stride (bytes per row) of the rendered framebuffer.
    pub stride: u32,
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

    #[test]
    fn test_frame_id_wrapping() {
        let f = FrameId(u64::MAX);
        assert_eq!(f.next(), FrameId(0));
    }

    #[test]
    fn test_frame_id_equality() {
        assert_eq!(FrameId(5), FrameId(5));
        assert_ne!(FrameId(5), FrameId(6));
    }

    #[test]
    fn test_damage_rect_fields() {
        let rect = DamageRect { x: 10, y: 20, width: 100, height: 200 };
        assert_eq!(rect.x, 10);
        assert_eq!(rect.y, 20);
        assert_eq!(rect.width, 100);
        assert_eq!(rect.height, 200);
    }

    #[test]
    fn test_frame_complete_clone() {
        let fc = FrameComplete {
            frame_id: FrameId(42),
            render_time_us: 16000,
            dropped: false,
            pixels: Some(Arc::new(vec![0u8; 100])),
            width: 10,
            height: 10,
            stride: 40,
        };
        let cloned = fc.clone();
        assert_eq!(cloned.frame_id, FrameId(42));
        assert_eq!(cloned.render_time_us, 16000);
        assert!(!cloned.dropped);
        assert!(cloned.pixels.is_some());
    }
}
