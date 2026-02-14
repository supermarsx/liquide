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
        Self { style: FrameStyle::default() }
    }

    pub fn with_style(style: FrameStyle) -> Self {
        Self { style }
    }

    /// Hit-test a point against the resize handles.
    /// Returns which resize edge (if any) the point is on.
    pub fn hit_test_resize(&self, x: f32, y: f32, win_x: f32, win_y: f32, win_w: f32, win_h: f32) -> ResizeEdge {
        let tol = self.style.resize_tolerance;
        let left = win_x;
        let right = win_x + win_w;
        let top = win_y;
        let bottom = win_y + win_h;

        let on_left = x >= left - tol && x < left + tol;
        let on_right = x >= right - tol && x < right + tol;
        let on_top = y >= top - tol && y < top + tol;
        let on_bottom = y >= bottom - tol && y < bottom + tol;

        // Corners
        if on_left && on_top { return ResizeEdge::TopLeft; }
        if on_right && on_top { return ResizeEdge::TopRight; }
        if on_left && on_bottom { return ResizeEdge::BottomLeft; }
        if on_right && on_bottom { return ResizeEdge::BottomRight; }

        // Edges
        if on_left { return ResizeEdge::Left; }
        if on_right { return ResizeEdge::Right; }
        if on_top { return ResizeEdge::Top; }
        if on_bottom { return ResizeEdge::Bottom; }

        ResizeEdge::None
    }
}

impl Default for WindowFrame {
    fn default() -> Self { Self::new() }
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
    pub fn is_some(&self) -> bool { *self != ResizeEdge::None }
}
