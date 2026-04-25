//! Window frame — border, shadow, resize handle styling.

use serde::{Deserialize, Serialize};

/// Frame styling parameters.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FrameStyle {
    pub border_width: f32,
    pub corner_radius: f32,
    pub shadow_blur: f32,
    pub shadow_offset_y: f32,
    pub shadow_opacity: f32,
    /// Resize edge tolerance in pixels.
    pub resize_tolerance: f32,
}

impl Default for FrameStyle {
    fn default() -> Self {
        Self {
            border_width: 1.0,
            corner_radius: 10.0,
            shadow_blur: 12.0,
            shadow_offset_y: 4.0,
            shadow_opacity: 0.25,
            resize_tolerance: 8.0,
        }
    }
}

/// Window frame — manages the border decorations and resize handles.
pub struct WindowFrame {
    pub style: FrameStyle,
}

impl WindowFrame {
    pub fn new() -> Self {
        Self {
            style: FrameStyle::default(),
        }
    }

    pub fn with_style(style: FrameStyle) -> Self {
        Self { style }
    }

    /// Hit-test a point against the resize handles.
    /// Returns which resize edge (if any) the point is on. `dpi_scale`
    /// grows the tolerance on HiDPI displays so the hit region scales
    /// with the visual pixel size of the handles.
    pub fn hit_test_resize(
        &self,
        x: f32,
        y: f32,
        win_x: f32,
        win_y: f32,
        win_w: f32,
        win_h: f32,
    ) -> ResizeEdge {
        self.hit_test_resize_scaled(x, y, win_x, win_y, win_w, win_h, 1.0)
    }

    /// DPI-aware variant of `hit_test_resize`.
    pub fn hit_test_resize_scaled(
        &self,
        x: f32,
        y: f32,
        win_x: f32,
        win_y: f32,
        win_w: f32,
        win_h: f32,
        dpi_scale: f32,
    ) -> ResizeEdge {
        let tol = self.style.resize_tolerance * dpi_scale.max(0.25);
        let left = win_x;
        let right = win_x + win_w;
        let top = win_y;
        let bottom = win_y + win_h;

        let on_left = x >= left - tol && x < left + tol;
        let on_right = x >= right - tol && x < right + tol;
        let on_top = y >= top - tol && y < top + tol;
        let on_bottom = y >= bottom - tol && y < bottom + tol;

        // Corners
        if on_left && on_top {
            return ResizeEdge::TopLeft;
        }
        if on_right && on_top {
            return ResizeEdge::TopRight;
        }
        if on_left && on_bottom {
            return ResizeEdge::BottomLeft;
        }
        if on_right && on_bottom {
            return ResizeEdge::BottomRight;
        }

        // Edges
        if on_left {
            return ResizeEdge::Left;
        }
        if on_right {
            return ResizeEdge::Right;
        }
        if on_top {
            return ResizeEdge::Top;
        }
        if on_bottom {
            return ResizeEdge::Bottom;
        }

        ResizeEdge::None
    }
}

impl Default for WindowFrame {
    fn default() -> Self {
        Self::new()
    }
}

/// Resize edge zones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResizeEdge {
    None,
    Top,
    Bottom,
    Left,
    Right,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl ResizeEdge {
    pub fn is_some(&self) -> bool {
        *self != ResizeEdge::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_style_default() {
        let s = FrameStyle::default();
        assert_eq!(s.border_width, 1.0);
        assert_eq!(s.corner_radius, 10.0);
        assert_eq!(s.resize_tolerance, 8.0);
    }

    #[test]
    fn test_hit_test_none_inside() {
        let frame = WindowFrame::new();
        // Point well inside the window
        let edge = frame.hit_test_resize(200.0, 200.0, 100.0, 100.0, 400.0, 300.0);
        assert_eq!(edge, ResizeEdge::None);
        assert!(!edge.is_some());
    }

    #[test]
    fn test_hit_test_left_edge() {
        let frame = WindowFrame::new();
        let edge = frame.hit_test_resize(100.0, 250.0, 100.0, 100.0, 400.0, 300.0);
        assert_eq!(edge, ResizeEdge::Left);
        assert!(edge.is_some());
    }

    #[test]
    fn test_hit_test_right_edge() {
        let frame = WindowFrame::new();
        let edge = frame.hit_test_resize(500.0, 250.0, 100.0, 100.0, 400.0, 300.0);
        assert_eq!(edge, ResizeEdge::Right);
    }

    #[test]
    fn test_hit_test_top_edge() {
        let frame = WindowFrame::new();
        let edge = frame.hit_test_resize(300.0, 100.0, 100.0, 100.0, 400.0, 300.0);
        assert_eq!(edge, ResizeEdge::Top);
    }

    #[test]
    fn test_hit_test_bottom_edge() {
        let frame = WindowFrame::new();
        let edge = frame.hit_test_resize(300.0, 400.0, 100.0, 100.0, 400.0, 300.0);
        assert_eq!(edge, ResizeEdge::Bottom);
    }

    #[test]
    fn test_hit_test_top_left_corner() {
        let frame = WindowFrame::new();
        let edge = frame.hit_test_resize(100.0, 100.0, 100.0, 100.0, 400.0, 300.0);
        assert_eq!(edge, ResizeEdge::TopLeft);
    }

    #[test]
    fn test_hit_test_top_right_corner() {
        let frame = WindowFrame::new();
        let edge = frame.hit_test_resize(500.0, 100.0, 100.0, 100.0, 400.0, 300.0);
        assert_eq!(edge, ResizeEdge::TopRight);
    }

    #[test]
    fn test_hit_test_bottom_left_corner() {
        let frame = WindowFrame::new();
        let edge = frame.hit_test_resize(100.0, 400.0, 100.0, 100.0, 400.0, 300.0);
        assert_eq!(edge, ResizeEdge::BottomLeft);
    }

    #[test]
    fn test_hit_test_bottom_right_corner() {
        let frame = WindowFrame::new();
        let edge = frame.hit_test_resize(500.0, 400.0, 100.0, 100.0, 400.0, 300.0);
        assert_eq!(edge, ResizeEdge::BottomRight);
    }
}
