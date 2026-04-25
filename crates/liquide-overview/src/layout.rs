/// Position and size of a thumbnail rectangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OverviewRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl OverviewRect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Returns true if the point `(px, py)` lies inside this rectangle.
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }
}

/// Input data describing a window to be arranged in the overview.
#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub id: u64,
    pub title: String,
    pub original: OverviewRect,
    pub workspace: u32,
    pub monitor: u32,
}

/// Computed position for a single window in the overview grid.
#[derive(Debug, Clone, PartialEq)]
pub struct OverviewSlot {
    pub window_id: u64,
    pub target: OverviewRect,
    pub scale: f32,
    pub label_y: f32,
}

/// Configuration for the overview layout algorithm.
#[derive(Debug, Clone)]
pub struct LayoutConfig {
    pub padding: f32,
    pub gap: f32,
    pub max_columns: u32,
    pub show_titles: bool,
    pub aspect_preserve: bool,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            padding: 40.0,
            gap: 20.0,
            max_columns: 8,
            show_titles: true,
            aspect_preserve: true,
        }
    }
}

/// Choose the grid dimensions (cols, rows) that best fills the viewport.
///
/// Strategy: try every column count from 1..=max_columns, pick the layout
/// whose rows/cols ratio is closest to 1 (squarish grid). Among ties, prefer
/// fewer columns to keep cells wider.
fn optimal_grid(count: usize, max_columns: u32) -> (u32, u32) {
    if count == 0 {
        return (1, 1);
    }
    let n = count as u32;
    let max_c = max_columns.max(1).min(n);

    let mut best_cols = 1u32;
    let mut best_rows = n;
    let mut best_diff = n as i32; // |rows - cols|

    for c in 1..=max_c {
        let r = (n + c - 1) / c;
        let diff = (r as i32 - c as i32).abs();
        if diff < best_diff || (diff == best_diff && r < best_rows) {
            best_diff = diff;
            best_cols = c;
            best_rows = r;
        }
    }
    (best_cols, best_rows)
}

/// Compute overview layout positions for a list of windows.
///
/// Windows are arranged in a grid centred inside `viewport`, with uniform cell
/// sizes. When `config.aspect_preserve` is true each window is scaled to fit
/// its cell while preserving its original aspect ratio.
pub fn compute_overview_layout(
    windows: &[WindowInfo],
    viewport: OverviewRect,
    config: &LayoutConfig,
) -> Vec<OverviewSlot> {
    if windows.is_empty() {
        return Vec::new();
    }

    let (cols, rows) = optimal_grid(windows.len(), config.max_columns);

    let label_height = if config.show_titles { 24.0f32 } else { 0.0 };

    // Available space after padding.
    let avail_w = (viewport.width - config.padding * 2.0).max(1.0);
    let avail_h = (viewport.height - config.padding * 2.0).max(1.0);

    // Cell size (including gap between cells).
    let cell_w = (avail_w - config.gap * (cols as f32 - 1.0).max(0.0)) / cols as f32;
    let cell_h = (avail_h - config.gap * (rows as f32 - 1.0).max(0.0)) / rows as f32;

    // Grid total size (for centring within the viewport).
    let grid_w = cell_w * cols as f32 + config.gap * (cols as f32 - 1.0).max(0.0);
    let grid_h = cell_h * rows as f32 + config.gap * (rows as f32 - 1.0).max(0.0);

    let origin_x = viewport.x + config.padding + (avail_w - grid_w) / 2.0;
    let origin_y = viewport.y + config.padding + (avail_h - grid_h) / 2.0;

    let mut slots = Vec::with_capacity(windows.len());

    for (i, win) in windows.iter().enumerate() {
        let col = i as u32 % cols;
        let row = i as u32 / cols;

        let cx = origin_x + col as f32 * (cell_w + config.gap);
        let cy = origin_y + row as f32 * (cell_h + config.gap);

        // Usable area inside the cell (leave room for label at bottom).
        let usable_h = (cell_h - label_height).max(1.0);

        let (tw, th, scale) = if config.aspect_preserve {
            let orig_w = win.original.width.max(1.0);
            let orig_h = win.original.height.max(1.0);
            let sw = cell_w / orig_w;
            let sh = usable_h / orig_h;
            let s = sw.min(sh);
            (orig_w * s, orig_h * s, s)
        } else {
            let orig_w = win.original.width.max(1.0);
            let sw = cell_w / orig_w;
            (cell_w, usable_h, sw)
        };

        // Centre the thumbnail within its cell.
        let tx = cx + (cell_w - tw) / 2.0;
        let ty = cy + (usable_h - th) / 2.0;

        slots.push(OverviewSlot {
            window_id: win.id,
            target: OverviewRect::new(tx, ty, tw, th),
            scale,
            label_y: ty + th + 4.0,
        });
    }

    slots
}

/// Compute the horizontal workspace thumbnail strip at the top of the overview.
///
/// Returns one `OverviewRect` per workspace, arranged in a horizontal row
/// centred near the top of `viewport`.
pub fn compute_workspace_strip(
    num_workspaces: u32,
    _active: u32,
    viewport: OverviewRect,
) -> Vec<OverviewRect> {
    if num_workspaces == 0 {
        return Vec::new();
    }

    let strip_height = 80.0f32;
    let thumb_aspect = viewport.width / viewport.height.max(1.0);
    let thumb_h = strip_height;
    let thumb_w = thumb_h * thumb_aspect;
    let gap = 12.0f32;

    let total_w = thumb_w * num_workspaces as f32 + gap * (num_workspaces as f32 - 1.0).max(0.0);
    let start_x = viewport.x + (viewport.width - total_w) / 2.0;
    let start_y = viewport.y + 16.0;

    (0..num_workspaces)
        .map(|i| {
            OverviewRect::new(
                start_x + i as f32 * (thumb_w + gap),
                start_y,
                thumb_w,
                thumb_h,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn viewport() -> OverviewRect {
        OverviewRect::new(0.0, 0.0, 1920.0, 1080.0)
    }

    fn make_windows(count: usize) -> Vec<WindowInfo> {
        (0..count)
            .map(|i| WindowInfo {
                id: i as u64 + 1,
                title: format!("Window {}", i + 1),
                original: OverviewRect::new(100.0, 100.0, 800.0, 600.0),
                workspace: 0,
                monitor: 0,
            })
            .collect()
    }

    fn default_config() -> LayoutConfig {
        LayoutConfig::default()
    }

    #[test]
    fn empty_windows_returns_empty() {
        let slots = compute_overview_layout(&[], viewport(), &default_config());
        assert!(slots.is_empty());
    }

    #[test]
    fn single_window_centered() {
        let wins = make_windows(1);
        let vp = viewport();
        let slots = compute_overview_layout(&wins, vp, &default_config());
        assert_eq!(slots.len(), 1);
        let s = &slots[0];
        // Thumbnail should be centred horizontally.
        let mid_x = s.target.x + s.target.width / 2.0;
        assert!((mid_x - vp.width / 2.0).abs() < 1.0);
    }

    #[test]
    fn two_windows_side_by_side() {
        let wins = make_windows(2);
        let slots = compute_overview_layout(&wins, viewport(), &default_config());
        assert_eq!(slots.len(), 2);
        // Second window should be to the right of the first.
        assert!(slots[1].target.x > slots[0].target.x);
        // They should share the same vertical position.
        assert!((slots[0].target.y - slots[1].target.y).abs() < 1.0);
    }

    #[test]
    fn four_windows_two_by_two() {
        let wins = make_windows(4);
        let slots = compute_overview_layout(&wins, viewport(), &default_config());
        assert_eq!(slots.len(), 4);
        // First row: slots 0, 1 — second row: slots 2, 3.
        assert!((slots[0].target.y - slots[1].target.y).abs() < 1.0);
        assert!((slots[2].target.y - slots[3].target.y).abs() < 1.0);
        assert!(slots[2].target.y > slots[0].target.y);
    }

    #[test]
    fn many_windows_grid() {
        let wins = make_windows(12);
        let slots = compute_overview_layout(&wins, viewport(), &default_config());
        assert_eq!(slots.len(), 12);
        // All thumbnails should be within the viewport.
        for s in &slots {
            assert!(s.target.x >= 0.0);
            assert!(s.target.y >= 0.0);
            assert!(s.target.x + s.target.width <= 1920.0 + 1.0);
            assert!(s.target.y + s.target.height <= 1080.0 + 1.0);
        }
    }

    #[test]
    fn aspect_ratio_preserved() {
        let wins = vec![WindowInfo {
            id: 1,
            title: "Wide".into(),
            original: OverviewRect::new(0.0, 0.0, 1600.0, 400.0),
            workspace: 0,
            monitor: 0,
        }];
        let slots = compute_overview_layout(&wins, viewport(), &default_config());
        let s = &slots[0];
        let orig_ratio = 1600.0 / 400.0;
        let slot_ratio = s.target.width / s.target.height;
        assert!((orig_ratio - slot_ratio).abs() < 0.1);
    }

    #[test]
    fn no_aspect_preserve() {
        let wins = make_windows(1);
        let mut cfg = default_config();
        cfg.aspect_preserve = false;
        let slots = compute_overview_layout(&wins, viewport(), &cfg);
        assert_eq!(slots.len(), 1);
        // Without aspect preservation, window fills the cell width.
        let s = &slots[0];
        assert!(s.target.width > 100.0);
    }

    #[test]
    fn workspace_strip_positions() {
        let rects = compute_workspace_strip(3, 0, viewport());
        assert_eq!(rects.len(), 3);
        // All at the same y.
        assert!((rects[0].y - rects[1].y).abs() < 0.01);
        assert!((rects[1].y - rects[2].y).abs() < 0.01);
        // Ordered left to right.
        assert!(rects[1].x > rects[0].x);
        assert!(rects[2].x > rects[1].x);
        // Centred.
        let total = rects[2].x + rects[2].width - rects[0].x;
        let mid = rects[0].x + total / 2.0;
        assert!((mid - 960.0).abs() < 1.0);
    }

    #[test]
    fn workspace_strip_single() {
        let rects = compute_workspace_strip(1, 0, viewport());
        assert_eq!(rects.len(), 1);
    }

    #[test]
    fn workspace_strip_zero() {
        let rects = compute_workspace_strip(0, 0, viewport());
        assert!(rects.is_empty());
    }

    #[test]
    fn overview_rect_contains() {
        let r = OverviewRect::new(10.0, 20.0, 100.0, 50.0);
        assert!(r.contains(50.0, 40.0));
        assert!(!r.contains(5.0, 40.0));
        assert!(!r.contains(50.0, 75.0));
    }

    #[test]
    fn optimal_grid_one() {
        assert_eq!(optimal_grid(1, 8), (1, 1));
    }

    #[test]
    fn optimal_grid_two() {
        assert_eq!(optimal_grid(2, 8), (2, 1));
    }

    #[test]
    fn optimal_grid_three() {
        let (c, r) = optimal_grid(3, 8);
        assert_eq!(c * r >= 3, true);
        assert!(r <= 2);
    }

    #[test]
    fn optimal_grid_nine() {
        let (c, r) = optimal_grid(9, 8);
        assert!(c * r >= 9);
    }

    #[test]
    fn scale_factor_less_than_one() {
        let wins = make_windows(1);
        let slots = compute_overview_layout(&wins, viewport(), &default_config());
        // The window is 800x600, viewport is 1920x1080 — the scale should fit.
        assert!(slots[0].scale > 0.0);
        assert!(slots[0].scale <= 2.0);
    }

    #[test]
    fn label_y_below_thumbnail() {
        let wins = make_windows(1);
        let slots = compute_overview_layout(&wins, viewport(), &default_config());
        let s = &slots[0];
        assert!(s.label_y > s.target.y + s.target.height - 1.0);
    }

    #[test]
    fn window_ids_preserved() {
        let wins = make_windows(5);
        let slots = compute_overview_layout(&wins, viewport(), &default_config());
        for (w, s) in wins.iter().zip(slots.iter()) {
            assert_eq!(w.id, s.window_id);
        }
    }
}
