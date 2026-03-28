//! Snap zone detection for drag-and-drop window tiling.

use liquide_compositor::geometry::Rect;

/// Named snap target when dragging a window to a screen edge or corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SnapTarget {
    /// No snap detected.
    None,
    /// Left half of screen.
    Left,
    /// Right half of screen.
    Right,
    /// Maximize (top edge).
    Top,
    /// Bottom half of screen.
    Bottom,
    /// Top-left quarter.
    TopLeft,
    /// Top-right quarter.
    TopRight,
    /// Bottom-left quarter.
    BottomLeft,
    /// Bottom-right quarter.
    BottomRight,
    /// Center = maximize.
    Center,
}

impl SnapTarget {
    /// Whether this target represents an actual snap (not None).
    #[must_use]
    pub fn is_active(&self) -> bool {
        !matches!(self, SnapTarget::None)
    }
}

/// Snap zone detector.
pub struct SnapZones;

impl SnapZones {
    /// Detect which snap zone the cursor falls into.
    ///
    /// `cursor` is `(x, y)` in screen coordinates. `screen` is the monitor
    /// work area. `threshold` is the pixel distance from an edge that
    /// triggers detection.
    #[must_use]
    pub fn detect_zone(cursor: (f32, f32), screen: Rect, threshold: f32) -> SnapTarget {
        let (cx, cy) = cursor;

        // Check if cursor is within the screen bounds (with some tolerance).
        if cx < screen.x - threshold
            || cx > screen.right() + threshold
            || cy < screen.y - threshold
            || cy > screen.bottom() + threshold
        {
            return SnapTarget::None;
        }

        let near_left = cx - screen.x < threshold;
        let near_right = screen.right() - cx < threshold;
        let near_top = cy - screen.y < threshold;
        let near_bottom = screen.bottom() - cy < threshold;

        // Corners take priority.
        match (near_left, near_right, near_top, near_bottom) {
            (true, _, true, _) => SnapTarget::TopLeft,
            (true, _, _, true) => SnapTarget::BottomLeft,
            (_, true, true, _) => SnapTarget::TopRight,
            (_, true, _, true) => SnapTarget::BottomRight,
            (true, _, _, _) => SnapTarget::Left,
            (_, true, _, _) => SnapTarget::Right,
            (_, _, true, _) => SnapTarget::Top,
            (_, _, _, true) => SnapTarget::Bottom,
            _ => SnapTarget::None,
        }
    }

    /// Compute the preview rectangle for a given snap target.
    ///
    /// Corner targets produce quarter-screen rectangles; edge targets produce
    /// half-screen rectangles; Top and Center produce full-screen (maximize).
    #[must_use]
    pub fn zone_preview(target: SnapTarget, screen: Rect) -> Rect {
        let hw = screen.width / 2.0;
        let hh = screen.height / 2.0;

        match target {
            SnapTarget::None => Rect::ZERO,
            SnapTarget::Left => Rect::new(screen.x, screen.y, hw, screen.height),
            SnapTarget::Right => Rect::new(screen.x + hw, screen.y, hw, screen.height),
            SnapTarget::Top | SnapTarget::Center => screen,
            SnapTarget::Bottom => Rect::new(screen.x, screen.y + hh, screen.width, hh),
            SnapTarget::TopLeft => Rect::new(screen.x, screen.y, hw, hh),
            SnapTarget::TopRight => Rect::new(screen.x + hw, screen.y, hw, hh),
            SnapTarget::BottomLeft => Rect::new(screen.x, screen.y + hh, hw, hh),
            SnapTarget::BottomRight => Rect::new(screen.x + hw, screen.y + hh, hw, hh),
        }
    }
}
