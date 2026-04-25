//! Workspace layout and geometry.
//!
//! This module computes workspace positions for overview / expose views and
//! animated slide transitions between workspaces. Inspired by GNOME Shell's
//! workspace layout logic.

use serde::{Deserialize, Serialize};

// ── Rect ─────────────────────────────────────────────────────────────

/// Axis-aligned rectangle used for screen bounds, thumbnails, etc.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    pub fn new(x: i32, y: i32, w: u32, h: u32) -> Self {
        Self { x, y, w, h }
    }

    /// Returns true if this rect intersects another.
    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.x + other.w as i32
            && self.x + self.w as i32 > other.x
            && self.y < other.y + other.h as i32
            && self.y + self.h as i32 > other.y
    }

    /// Area in pixels.
    pub fn area(&self) -> u64 {
        self.w as u64 * self.h as u64
    }
}

// ── WorkspaceLayout ──────────────────────────────────────────────────

/// Describes how workspaces are arranged spatially (for transitions and
/// overview rendering).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkspaceLayout {
    /// Workspaces arranged in a grid with the given number of columns and
    /// rows.
    Grid { cols: usize, rows: usize },
    /// Workspaces in a single horizontal strip.
    HorizontalStrip,
    /// Workspaces in a single vertical strip.
    VerticalStrip,
    /// All workspaces occupy the same position (only one visible at a time).
    Single,
}

impl Default for WorkspaceLayout {
    fn default() -> Self {
        Self::HorizontalStrip
    }
}

// ── Workspace position ───────────────────────────────────────────────

/// Compute the logical (x, y) position of a workspace at `index` within the
/// given layout and screen size. The origin is (0, 0) for workspace 0.
///
/// For `HorizontalStrip`, workspaces are placed side by side horizontally.
/// For `VerticalStrip`, they stack vertically.
/// For `Grid`, they fill column-major (left to right, top to bottom).
/// For `Single`, all positions are (0, 0).
pub fn workspace_position(
    index: usize,
    layout: WorkspaceLayout,
    screen_width: u32,
    screen_height: u32,
) -> (i32, i32) {
    match layout {
        WorkspaceLayout::HorizontalStrip => (index as i32 * screen_width as i32, 0),
        WorkspaceLayout::VerticalStrip => (0, index as i32 * screen_height as i32),
        WorkspaceLayout::Grid { cols, .. } => {
            let col = index % cols;
            let row = index / cols;
            (
                col as i32 * screen_width as i32,
                row as i32 * screen_height as i32,
            )
        }
        WorkspaceLayout::Single => (0, 0),
    }
}

// ── Overview grid ────────────────────────────────────────────────────

/// Compute miniature rectangles for an overview / expose view.
///
/// Returns one [`Rect`] per workspace, positioned within `screen_width` x
/// `screen_height` with margins and gaps. The grid dimensions are chosen
/// automatically to approximate the screen aspect ratio.
pub fn overview_grid(workspace_count: usize, screen_width: u32, screen_height: u32) -> Vec<Rect> {
    if workspace_count == 0 {
        return Vec::new();
    }

    let sw = screen_width as f64;
    let sh = screen_height as f64;

    let cols = optimal_columns(workspace_count, sw, sh);
    let rows = (workspace_count + cols - 1) / cols;

    // 5% outer margin, 2% gap between cells.
    let margin_x = (sw * 0.05) as i32;
    let margin_y = (sh * 0.05) as i32;
    let gap_x = (sw * 0.02) as i32;
    let gap_y = (sh * 0.02) as i32;

    let usable_w = screen_width as i32 - 2 * margin_x - (cols as i32 - 1).max(0) * gap_x;
    let usable_h = screen_height as i32 - 2 * margin_y - (rows as i32 - 1).max(0) * gap_y;

    let cell_w = (usable_w / cols as i32).max(1) as u32;
    let cell_h = (usable_h / rows as i32).max(1) as u32;

    let mut result = Vec::with_capacity(workspace_count);
    for i in 0..workspace_count {
        let col = i % cols;
        let row = i / cols;
        let x = margin_x + col as i32 * (cell_w as i32 + gap_x);
        let y = margin_y + row as i32 * (cell_h as i32 + gap_y);
        result.push(Rect::new(x, y, cell_w, cell_h));
    }

    result
}

/// Choose the number of columns that makes each cell closest to the
/// screen's aspect ratio.
fn optimal_columns(count: usize, area_w: f64, area_h: f64) -> usize {
    if count <= 1 {
        return 1;
    }
    let target_ratio = area_w / area_h;
    let mut best_cols = 1usize;
    let mut best_err = f64::MAX;

    for c in 1..=count {
        let r = (count + c - 1) / c;
        let cell_ratio = (area_w / c as f64) / (area_h / r as f64);
        let err = (cell_ratio - target_ratio).abs();
        if err < best_err {
            best_err = err;
            best_cols = c;
        }
    }
    best_cols
}

// ── Transition offset ────────────────────────────────────────────────

/// Compute the (x, y) pixel offset for an animated slide transition
/// between workspace `from_index` and `to_index`.
///
/// `progress` is in [0.0, 1.0]: 0.0 = fully showing `from`, 1.0 = fully
/// showing `to`.
///
/// The return value should be applied as a translation to the entire
/// workspace strip / scene so that the viewport slides from `from` to `to`.
pub fn transition_offset(
    from_index: usize,
    to_index: usize,
    progress: f64,
    layout: WorkspaceLayout,
    screen_width: u32,
    screen_height: u32,
) -> (f64, f64) {
    let progress = progress.clamp(0.0, 1.0);

    let (from_x, from_y) = workspace_position(from_index, layout, screen_width, screen_height);
    let (to_x, to_y) = workspace_position(to_index, layout, screen_width, screen_height);

    let dx = (to_x - from_x) as f64;
    let dy = (to_y - from_y) as f64;

    // The offset is negative because we translate the *scene* in the
    // opposite direction of the workspace we want to see.
    let offset_x = -(from_x as f64 + dx * progress);
    let offset_y = -(from_y as f64 + dy * progress);

    (offset_x, offset_y)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Rect ────────────────────────────────────────────────────────

    #[test]
    fn rect_intersects() {
        let a = Rect::new(0, 0, 100, 100);
        let b = Rect::new(50, 50, 100, 100);
        assert!(a.intersects(&b));
        assert!(b.intersects(&a));
    }

    #[test]
    fn rect_no_intersect() {
        let a = Rect::new(0, 0, 100, 100);
        let b = Rect::new(200, 200, 50, 50);
        assert!(!a.intersects(&b));
    }

    #[test]
    fn rect_adjacent_no_intersect() {
        let a = Rect::new(0, 0, 100, 100);
        let b = Rect::new(100, 0, 100, 100);
        assert!(!a.intersects(&b));
    }

    #[test]
    fn rect_area() {
        assert_eq!(Rect::new(0, 0, 1920, 1080).area(), 1920 * 1080);
    }

    // ── workspace_position ──────────────────────────────────────────

    #[test]
    fn horizontal_strip_positions() {
        let p0 = workspace_position(0, WorkspaceLayout::HorizontalStrip, 1920, 1080);
        let p1 = workspace_position(1, WorkspaceLayout::HorizontalStrip, 1920, 1080);
        let p2 = workspace_position(2, WorkspaceLayout::HorizontalStrip, 1920, 1080);
        assert_eq!(p0, (0, 0));
        assert_eq!(p1, (1920, 0));
        assert_eq!(p2, (3840, 0));
    }

    #[test]
    fn vertical_strip_positions() {
        let p0 = workspace_position(0, WorkspaceLayout::VerticalStrip, 1920, 1080);
        let p1 = workspace_position(1, WorkspaceLayout::VerticalStrip, 1920, 1080);
        assert_eq!(p0, (0, 0));
        assert_eq!(p1, (0, 1080));
    }

    #[test]
    fn grid_positions() {
        let layout = WorkspaceLayout::Grid { cols: 2, rows: 2 };
        let p0 = workspace_position(0, layout, 1920, 1080);
        let p1 = workspace_position(1, layout, 1920, 1080);
        let p2 = workspace_position(2, layout, 1920, 1080);
        let p3 = workspace_position(3, layout, 1920, 1080);
        assert_eq!(p0, (0, 0));
        assert_eq!(p1, (1920, 0));
        assert_eq!(p2, (0, 1080));
        assert_eq!(p3, (1920, 1080));
    }

    #[test]
    fn single_layout_all_zero() {
        assert_eq!(
            workspace_position(0, WorkspaceLayout::Single, 1920, 1080),
            (0, 0)
        );
        assert_eq!(
            workspace_position(5, WorkspaceLayout::Single, 1920, 1080),
            (0, 0)
        );
    }

    // ── overview_grid ───────────────────────────────────────────────

    #[test]
    fn overview_grid_correct_count() {
        let grid = overview_grid(4, 1920, 1080);
        assert_eq!(grid.len(), 4);
    }

    #[test]
    fn overview_grid_empty() {
        assert!(overview_grid(0, 1920, 1080).is_empty());
    }

    #[test]
    fn overview_grid_single_is_large() {
        let grid = overview_grid(1, 1920, 1080);
        assert_eq!(grid.len(), 1);
        assert!(grid[0].w > 1920 / 2);
        assert!(grid[0].h > 1080 / 2);
    }

    #[test]
    fn overview_grid_no_overlap() {
        let grid = overview_grid(6, 1920, 1080);
        for i in 0..grid.len() {
            for j in (i + 1)..grid.len() {
                assert!(
                    !grid[i].intersects(&grid[j]),
                    "Thumbnails {} and {} overlap: {:?} vs {:?}",
                    i,
                    j,
                    grid[i],
                    grid[j]
                );
            }
        }
    }

    #[test]
    fn overview_grid_all_positive_size() {
        let grid = overview_grid(9, 1920, 1080);
        for r in &grid {
            assert!(r.w > 0);
            assert!(r.h > 0);
        }
    }

    // ── transition_offset ───────────────────────────────────────────

    #[test]
    fn transition_start_shows_from() {
        let (ox, oy) = transition_offset(0, 1, 0.0, WorkspaceLayout::HorizontalStrip, 1920, 1080);
        // At progress 0, offset should position viewport at workspace 0.
        assert!((ox - 0.0).abs() < 1.0);
        assert!((oy - 0.0).abs() < 1.0);
    }

    #[test]
    fn transition_end_shows_to() {
        let (ox, _oy) = transition_offset(0, 1, 1.0, WorkspaceLayout::HorizontalStrip, 1920, 1080);
        // At progress 1, offset should position viewport at workspace 1.
        assert!((ox - (-1920.0)).abs() < 1.0);
    }

    #[test]
    fn transition_midpoint() {
        let (ox, _oy) = transition_offset(0, 1, 0.5, WorkspaceLayout::HorizontalStrip, 1920, 1080);
        assert!((ox - (-960.0)).abs() < 1.0);
    }

    #[test]
    fn transition_vertical() {
        let (_ox, oy) = transition_offset(0, 1, 1.0, WorkspaceLayout::VerticalStrip, 1920, 1080);
        assert!((oy - (-1080.0)).abs() < 1.0);
    }

    #[test]
    fn transition_clamps_progress() {
        let (ox1, _) = transition_offset(0, 1, -0.5, WorkspaceLayout::HorizontalStrip, 1920, 1080);
        let (ox2, _) = transition_offset(0, 1, 0.0, WorkspaceLayout::HorizontalStrip, 1920, 1080);
        assert!((ox1 - ox2).abs() < 1.0);
    }

    #[test]
    fn transition_same_workspace() {
        let (ox, oy) = transition_offset(2, 2, 0.5, WorkspaceLayout::HorizontalStrip, 1920, 1080);
        // from == to, so offset should be at workspace 2's position.
        let expected_x = -(2.0 * 1920.0);
        assert!((ox - expected_x).abs() < 1.0);
        assert!((oy - 0.0).abs() < 1.0);
    }
}
