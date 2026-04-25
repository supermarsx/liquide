use crate::effects::Rect;

/// Zones for quarter / half tiling when dragging to screen edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileZone {
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Maximize,
}

/// A translucent preview rectangle shown while dragging a window.
pub struct DragPreview {
    pub active: bool,
    pub target_rect: Rect,
    pub opacity: f32,
    pub show_outline: bool,
    fade_target: f32,
    fade_speed: f32, // opacity units per millisecond
}

impl DragPreview {
    pub fn new() -> Self {
        Self {
            active: false,
            target_rect: Rect::new(0.0, 0.0, 0.0, 0.0),
            opacity: 0.0,
            show_outline: true,
            fade_target: 0.0,
            fade_speed: 0.004, // ~250 ms full fade at 1.0 range
        }
    }

    /// Show the drag preview at `target`.
    pub fn begin(&mut self, target: Rect) {
        self.active = true;
        self.target_rect = target;
        self.fade_target = 0.35;
    }

    /// Update the preview position while dragging.
    pub fn update(&mut self, new_target: Rect) {
        self.target_rect = new_target;
    }

    /// Hide the preview (starts fade-out).
    pub fn end(&mut self) {
        self.fade_target = 0.0;
    }

    /// Advance the opacity animation by `dt_ms` milliseconds.
    pub fn tick(&mut self, dt_ms: f32) {
        let step = self.fade_speed * dt_ms;
        if self.opacity < self.fade_target {
            self.opacity = (self.opacity + step).min(self.fade_target);
        } else if self.opacity > self.fade_target {
            self.opacity = (self.opacity - step).max(self.fade_target);
        }
        if !self.active && self.opacity <= 0.0 {
            self.opacity = 0.0;
        }
        if self.fade_target <= 0.0 && self.opacity <= 0.0 {
            self.active = false;
        }
    }

    /// Current preview rectangle, if visible.
    pub fn current_rect(&self) -> Option<Rect> {
        if self.active || self.opacity > 0.0 {
            Some(self.target_rect)
        } else {
            None
        }
    }

    /// Current opacity of the preview overlay.
    pub fn current_opacity(&self) -> f32 {
        self.opacity
    }
}

impl Default for DragPreview {
    fn default() -> Self {
        Self::new()
    }
}

/// Shows a translucent blue rectangle when the cursor is dragged to a screen
/// edge, indicating where the window will tile.
pub struct TilePreview;

impl TilePreview {
    /// Determine which tile zone (if any) the cursor is in.  `threshold` is
    /// the number of pixels from the screen edge that triggers tiling.
    /// Corner zones are detected when the cursor is within `threshold` of two
    /// perpendicular edges simultaneously.
    pub fn check_tile_zone(
        cursor_x: f32,
        cursor_y: f32,
        screen: Rect,
        threshold: f32,
    ) -> Option<TileZone> {
        let at_left = cursor_x - screen.x < threshold;
        let at_right = (screen.x + screen.width) - cursor_x < threshold;
        let at_top = cursor_y - screen.y < threshold;
        let at_bottom = (screen.y + screen.height) - cursor_y < threshold;

        match (at_left, at_right, at_top, at_bottom) {
            (true, _, true, _) => Some(TileZone::TopLeft),
            (_, true, true, _) => Some(TileZone::TopRight),
            (true, _, _, true) => Some(TileZone::BottomLeft),
            (_, true, _, true) => Some(TileZone::BottomRight),
            (true, _, _, _) => Some(TileZone::Left),
            (_, true, _, _) => Some(TileZone::Right),
            (_, _, true, _) => Some(TileZone::Maximize),
            (_, _, _, true) => Some(TileZone::Bottom),
            _ => None,
        }
    }

    /// Compute the target rectangle for a tile zone, respecting `gap` between
    /// the tiled window and the screen edges / other halves.
    pub fn zone_rect(zone: TileZone, screen: Rect, gap: f32) -> Rect {
        let half_w = (screen.width - gap * 3.0) / 2.0;
        let half_h = (screen.height - gap * 3.0) / 2.0;
        let x0 = screen.x + gap;
        let y0 = screen.y + gap;

        match zone {
            TileZone::Left => Rect::new(x0, y0, half_w, screen.height - gap * 2.0),
            TileZone::Right => Rect::new(x0 + half_w + gap, y0, half_w, screen.height - gap * 2.0),
            TileZone::Top => Rect::new(x0, y0, screen.width - gap * 2.0, half_h),
            TileZone::Bottom => Rect::new(x0, y0 + half_h + gap, screen.width - gap * 2.0, half_h),
            TileZone::TopLeft => Rect::new(x0, y0, half_w, half_h),
            TileZone::TopRight => Rect::new(x0 + half_w + gap, y0, half_w, half_h),
            TileZone::BottomLeft => Rect::new(x0, y0 + half_h + gap, half_w, half_h),
            TileZone::BottomRight => {
                Rect::new(x0 + half_w + gap, y0 + half_h + gap, half_w, half_h)
            }
            TileZone::Maximize => {
                Rect::new(x0, y0, screen.width - gap * 2.0, screen.height - gap * 2.0)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen() -> Rect {
        Rect::new(0.0, 0.0, 1920.0, 1080.0)
    }

    // ── TileZone detection ──────────────────────────────────────────

    #[test]
    fn tile_zone_left() {
        let z = TilePreview::check_tile_zone(3.0, 540.0, screen(), 10.0);
        assert_eq!(z, Some(TileZone::Left));
    }

    #[test]
    fn tile_zone_right() {
        let z = TilePreview::check_tile_zone(1917.0, 540.0, screen(), 10.0);
        assert_eq!(z, Some(TileZone::Right));
    }

    #[test]
    fn tile_zone_top_maximizes() {
        let z = TilePreview::check_tile_zone(960.0, 3.0, screen(), 10.0);
        assert_eq!(z, Some(TileZone::Maximize));
    }

    #[test]
    fn tile_zone_bottom() {
        let z = TilePreview::check_tile_zone(960.0, 1077.0, screen(), 10.0);
        assert_eq!(z, Some(TileZone::Bottom));
    }

    #[test]
    fn tile_zone_top_left() {
        let z = TilePreview::check_tile_zone(3.0, 3.0, screen(), 10.0);
        assert_eq!(z, Some(TileZone::TopLeft));
    }

    #[test]
    fn tile_zone_top_right() {
        let z = TilePreview::check_tile_zone(1917.0, 3.0, screen(), 10.0);
        assert_eq!(z, Some(TileZone::TopRight));
    }

    #[test]
    fn tile_zone_bottom_left() {
        let z = TilePreview::check_tile_zone(3.0, 1077.0, screen(), 10.0);
        assert_eq!(z, Some(TileZone::BottomLeft));
    }

    #[test]
    fn tile_zone_bottom_right() {
        let z = TilePreview::check_tile_zone(1917.0, 1077.0, screen(), 10.0);
        assert_eq!(z, Some(TileZone::BottomRight));
    }

    #[test]
    fn tile_zone_center_is_none() {
        let z = TilePreview::check_tile_zone(960.0, 540.0, screen(), 10.0);
        assert_eq!(z, None);
    }

    // ── zone_rect ───────────────────────────────────────────────────

    #[test]
    fn zone_rect_left_right_no_overlap() {
        let gap = 8.0;
        let left = TilePreview::zone_rect(TileZone::Left, screen(), gap);
        let right = TilePreview::zone_rect(TileZone::Right, screen(), gap);
        assert!(left.x + left.width <= right.x);
        assert!((left.width - right.width).abs() < 1e-3);
    }

    #[test]
    fn zone_rect_top_bottom_no_overlap() {
        let gap = 8.0;
        let top = TilePreview::zone_rect(TileZone::Top, screen(), gap);
        let bottom = TilePreview::zone_rect(TileZone::Bottom, screen(), gap);
        assert!(top.y + top.height <= bottom.y);
        assert!((top.height - bottom.height).abs() < 1e-3);
    }

    #[test]
    fn zone_rect_maximize_fills_screen() {
        let gap = 8.0;
        let r = TilePreview::zone_rect(TileZone::Maximize, screen(), gap);
        assert!((r.width - (1920.0 - 16.0)).abs() < 1e-3);
        assert!((r.height - (1080.0 - 16.0)).abs() < 1e-3);
    }

    #[test]
    fn zone_rect_quarter_tiles_no_overlap() {
        let gap = 8.0;
        let tl = TilePreview::zone_rect(TileZone::TopLeft, screen(), gap);
        let tr = TilePreview::zone_rect(TileZone::TopRight, screen(), gap);
        let bl = TilePreview::zone_rect(TileZone::BottomLeft, screen(), gap);
        let br = TilePreview::zone_rect(TileZone::BottomRight, screen(), gap);

        // Horizontal: TL and TR don't overlap
        assert!(tl.x + tl.width <= tr.x);
        // Vertical: TL and BL don't overlap
        assert!(tl.y + tl.height <= bl.y);
        // Same width
        assert!((tl.width - br.width).abs() < 1e-3);
        // Same height
        assert!((tl.height - br.height).abs() < 1e-3);
    }

    // ── DragPreview ─────────────────────────────────────────────────

    #[test]
    fn drag_preview_begin_activates() {
        let mut dp = DragPreview::new();
        assert!(!dp.active);
        dp.begin(Rect::new(100.0, 100.0, 800.0, 600.0));
        assert!(dp.active);
    }

    #[test]
    fn drag_preview_fades_in() {
        let mut dp = DragPreview::new();
        dp.begin(Rect::new(100.0, 100.0, 800.0, 600.0));
        assert!((dp.opacity - 0.0).abs() < 1e-5);
        dp.tick(500.0); // large dt to ensure it reaches target
        assert!(dp.opacity > 0.0);
    }

    #[test]
    fn drag_preview_end_fades_out() {
        let mut dp = DragPreview::new();
        dp.begin(Rect::new(100.0, 100.0, 800.0, 600.0));
        dp.tick(500.0); // fade in
        dp.end();
        dp.tick(500.0); // fade out
        assert!((dp.opacity - 0.0).abs() < 1e-5);
        assert!(!dp.active);
    }

    #[test]
    fn drag_preview_update_changes_rect() {
        let mut dp = DragPreview::new();
        dp.begin(Rect::new(0.0, 0.0, 100.0, 100.0));
        dp.update(Rect::new(50.0, 50.0, 200.0, 200.0));
        let r = dp.current_rect().unwrap();
        assert!((r.x - 50.0).abs() < 1e-5);
        assert!((r.width - 200.0).abs() < 1e-5);
    }

    #[test]
    fn drag_preview_current_rect_none_when_inactive() {
        let dp = DragPreview::new();
        assert!(dp.current_rect().is_none());
    }

    #[test]
    fn drag_preview_default() {
        let dp = DragPreview::default();
        assert!(!dp.active);
        assert!((dp.opacity - 0.0).abs() < 1e-5);
    }
}
