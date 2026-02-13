//! Window decoration hit-testing.

use liquide_compositor::geometry::Rect;
use serde::{Deserialize, Serialize};

/// Window decoration style parameters.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DecorationStyle {
    pub title_bar_height: f32,
    pub border_width: f32,
    pub corner_radius: f32,
    pub button_size: f32,
    /// Resize edge tolerance in pixels (larger = easier to grab).
    pub resize_tolerance: f32,
}

impl Default for DecorationStyle {
    fn default() -> Self {
        Self {
            title_bar_height: 30.0,
            border_width: 1.0,
            corner_radius: 8.0,
            button_size: 16.0,
            resize_tolerance: 8.0, // 8px hit zone for edges
        }
    }
}

/// Zones of a decorated window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HitZone {
    TitleBar,
    CloseButton,
    MinimizeButton,
    MaximizeButton,
    AlwaysOnTopButton,
    ResizeTop,
    ResizeBottom,
    ResizeLeft,
    ResizeRight,
    ResizeTopLeft,
    ResizeTopRight,
    ResizeBottomLeft,
    ResizeBottomRight,
    Client,
    Outside,
}

impl std::fmt::Display for HitZone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

/// Hit-test a point against a decorated window.
///
/// The window bounds represent the client area. The title bar sits above
/// the client area, and resize borders extend `border_width` outside.
#[must_use]
pub fn hit_test_decoration(
    window_bounds: Rect,
    style: &DecorationStyle,
    x: f32,
    y: f32,
) -> HitZone {
    let bw = style.resize_tolerance; // Use larger tolerance for easier resizing
    let tbh = style.title_bar_height;
    let btn = style.button_size;

    // Expanded bounds (including resize borders)
    let outer = Rect::new(
        window_bounds.x - bw,
        window_bounds.y - tbh - bw,
        window_bounds.width + bw * 2.0,
        window_bounds.height + tbh + bw * 2.0,
    );

    if x < outer.x || x >= outer.x + outer.width || y < outer.y || y >= outer.y + outer.height {
        return HitZone::Outside;
    }

    let left = window_bounds.x;
    let right = window_bounds.x + window_bounds.width;
    let top = window_bounds.y - tbh;
    let bottom = window_bounds.y + window_bounds.height;

    // Title bar buttons (check first for priority over resize corners)
    if y >= top && y < window_bounds.y {
        let close_x = right - btn - 4.0;
        let max_x = close_x - btn - 4.0;
        let min_x = max_x - btn - 4.0;
        let aot_x = min_x - btn - 4.0;
        let btn_y_center = top + tbh / 2.0;

        if x >= close_x && x < close_x + btn && (y - btn_y_center).abs() < btn / 2.0 {
            return HitZone::CloseButton;
        }
        if x >= max_x && x < max_x + btn && (y - btn_y_center).abs() < btn / 2.0 {
            return HitZone::MaximizeButton;
        }
        if x >= min_x && x < min_x + btn && (y - btn_y_center).abs() < btn / 2.0 {
            return HitZone::MinimizeButton;
        }
        if x >= aot_x && x < aot_x + btn && (y - btn_y_center).abs() < btn / 2.0 {
            return HitZone::AlwaysOnTopButton;
        }
    }

    let corner_size = bw * 2.5; // Larger corner zones

    // Resize corners (prioritize corners over edges)
    if x < left + corner_size && y < top + corner_size {
        return HitZone::ResizeTopLeft;
    }
    if x >= right - corner_size && y < top + corner_size {
        return HitZone::ResizeTopRight;
    }
    if x < left + corner_size && y >= bottom - corner_size {
        return HitZone::ResizeBottomLeft;
    }
    if x >= right - corner_size && y >= bottom - corner_size {
        return HitZone::ResizeBottomRight;
    }

    // Resize borders
    if x < left {
        return HitZone::ResizeLeft;
    }
    if x >= right {
        return HitZone::ResizeRight;
    }
    if y < top {
        return HitZone::ResizeTop;
    }
    if y >= bottom {
        return HitZone::ResizeBottom;
    }

    // Title bar area (if not a button)
    if y < window_bounds.y {
        return HitZone::TitleBar;
    }

    HitZone::Client
}
