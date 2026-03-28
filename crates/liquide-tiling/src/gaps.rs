//! Tiling gap configuration.

use liquide_compositor::geometry::Rect;

/// Spacing configuration for tiled windows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TilingGaps {
    /// Gap between adjacent windows (pixels).
    pub inner: f32,
    /// Gap from screen edges (pixels).
    pub outer: f32,
    /// When true, disable all gaps when only one window is tiled.
    pub smart_gaps: bool,
}

impl Default for TilingGaps {
    fn default() -> Self {
        Self {
            inner: 8.0,
            outer: 8.0,
            smart_gaps: true,
        }
    }
}

impl TilingGaps {
    /// Create gap config with uniform inner and outer gap.
    #[must_use]
    pub fn uniform(gap: f32) -> Self {
        Self {
            inner: gap,
            outer: gap,
            smart_gaps: true,
        }
    }

    /// Create gap config with no gaps at all.
    #[must_use]
    pub fn none() -> Self {
        Self {
            inner: 0.0,
            outer: 0.0,
            smart_gaps: false,
        }
    }

    /// Compute the usable work area after subtracting outer gaps.
    #[must_use]
    pub fn usable_area(&self, work_area: Rect) -> Rect {
        Rect::new(
            work_area.x + self.outer,
            work_area.y + self.outer,
            (work_area.width - 2.0 * self.outer).max(0.0),
            (work_area.height - 2.0 * self.outer).max(0.0),
        )
    }

    /// Return the effective gaps for a given window count. If smart_gaps is
    /// enabled and there is only one window, all gaps are zero.
    #[must_use]
    pub fn effective(&self, window_count: usize) -> TilingGaps {
        if self.smart_gaps && window_count <= 1 {
            TilingGaps::none()
        } else {
            *self
        }
    }
}
