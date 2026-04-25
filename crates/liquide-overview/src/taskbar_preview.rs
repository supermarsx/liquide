//! Taskbar hover preview — shows a live thumbnail when the user hovers over a
//! dock or taskbar item, similar to GNOME's window-list tooltip or macOS dock
//! previews.

use crate::layout::OverviewRect;

/// Animation state of the preview tooltip.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PreviewState {
    /// Not visible.
    Hidden,
    /// Fading in. `progress` goes from 0.0 to 1.0.
    FadingIn(f32),
    /// Fully visible.
    Visible,
    /// Fading out. `progress` goes from 0.0 to 1.0.
    FadingOut(f32),
}

/// Layout result for a preview tooltip, positioned relative to an anchor.
#[derive(Debug, Clone, PartialEq)]
pub struct PreviewLayout {
    /// The bounding rect of the entire preview (thumbnail + title + close btn).
    pub bounds: OverviewRect,
    /// Where the thumbnail image is drawn.
    pub thumbnail_rect: OverviewRect,
    /// Where the window title text is drawn.
    pub title_rect: OverviewRect,
    /// Where the close button is drawn.
    pub close_button_rect: OverviewRect,
}

/// Configuration for the taskbar preview.
#[derive(Debug, Clone)]
pub struct PreviewConfig {
    /// Width of the preview thumbnail in pixels.
    pub thumbnail_width: f32,
    /// Height of the preview thumbnail in pixels.
    pub thumbnail_height: f32,
    /// Height of the title bar area below the thumbnail.
    pub title_height: f32,
    /// Padding around the content inside the preview.
    pub padding: f32,
    /// Gap between the anchor (dock item) and the preview.
    pub anchor_gap: f32,
    /// Duration of the fade-in animation in milliseconds.
    pub fade_in_ms: f32,
    /// Duration of the fade-out animation in milliseconds.
    pub fade_out_ms: f32,
    /// Size of the close button (square).
    pub close_button_size: f32,
}

impl Default for PreviewConfig {
    fn default() -> Self {
        Self {
            thumbnail_width: 240.0,
            thumbnail_height: 160.0,
            title_height: 28.0,
            padding: 8.0,
            anchor_gap: 8.0,
            fade_in_ms: 150.0,
            fade_out_ms: 100.0,
            close_button_size: 18.0,
        }
    }
}

/// Taskbar hover preview showing a live thumbnail above a dock/taskbar item.
pub struct TaskbarPreview {
    /// The window currently being previewed, if any.
    pub target_window_id: Option<u64>,
    /// The window title (for display).
    pub title: String,
    /// Current animation state.
    pub state: PreviewState,
    /// The computed layout of the preview tooltip.
    pub layout: Option<PreviewLayout>,
    /// The anchor rect (dock item) the preview is attached to.
    anchor: Option<OverviewRect>,
    /// Screen bounds for clamping.
    screen_width: f32,
    screen_height: f32,
    /// Configuration.
    config: PreviewConfig,
}

impl TaskbarPreview {
    pub fn new(screen_width: f32, screen_height: f32, config: PreviewConfig) -> Self {
        Self {
            target_window_id: None,
            title: String::new(),
            state: PreviewState::Hidden,
            layout: None,
            anchor: None,
            screen_width,
            screen_height,
            config,
        }
    }

    /// Show the preview for a given window, anchored above a dock/taskbar item.
    ///
    /// `anchor_rect` is the bounding box of the dock item that was hovered.
    pub fn show(&mut self, window_id: u64, title: &str, anchor_rect: OverviewRect) {
        // If we're already showing this window, don't restart the animation.
        if self.target_window_id == Some(window_id) && self.state != PreviewState::Hidden {
            return;
        }

        self.target_window_id = Some(window_id);
        self.title = title.to_string();
        self.anchor = Some(anchor_rect);
        self.state = PreviewState::FadingIn(0.0);
        self.layout = Some(self.compute_layout(anchor_rect));
    }

    /// Hide the preview with a fade-out animation.
    pub fn hide(&mut self) {
        match self.state {
            PreviewState::Hidden | PreviewState::FadingOut(_) => {}
            _ => {
                self.state = PreviewState::FadingOut(0.0);
            }
        }
    }

    /// Advance the fade animations by `dt` milliseconds.
    ///
    /// Returns `true` if the preview is still animating and needs redraw.
    pub fn tick(&mut self, dt_ms: f32) -> bool {
        match self.state {
            PreviewState::FadingIn(progress) => {
                let new_progress = progress + dt_ms / self.config.fade_in_ms;
                if new_progress >= 1.0 {
                    self.state = PreviewState::Visible;
                } else {
                    self.state = PreviewState::FadingIn(new_progress);
                }
                true
            }
            PreviewState::FadingOut(progress) => {
                let new_progress = progress + dt_ms / self.config.fade_out_ms;
                if new_progress >= 1.0 {
                    self.state = PreviewState::Hidden;
                    self.target_window_id = None;
                    self.layout = None;
                    self.anchor = None;
                    return false;
                }
                self.state = PreviewState::FadingOut(new_progress);
                true
            }
            _ => false,
        }
    }

    /// Current opacity based on animation state (0.0 = invisible, 1.0 = opaque).
    pub fn opacity(&self) -> f32 {
        match self.state {
            PreviewState::Hidden => 0.0,
            PreviewState::FadingIn(p) => p.clamp(0.0, 1.0),
            PreviewState::Visible => 1.0,
            PreviewState::FadingOut(p) => (1.0 - p).clamp(0.0, 1.0),
        }
    }

    /// Whether the preview is showing (visible or animating).
    pub fn is_active(&self) -> bool {
        self.state != PreviewState::Hidden
    }

    /// Hit-test: returns `true` if the close button was clicked.
    pub fn hit_test_close(&self, x: f32, y: f32) -> bool {
        if let Some(ref layout) = self.layout {
            layout.close_button_rect.contains(x, y)
        } else {
            false
        }
    }

    /// Hit-test: returns `true` if the point is inside the preview thumbnail.
    pub fn hit_test_thumbnail(&self, x: f32, y: f32) -> bool {
        if let Some(ref layout) = self.layout {
            layout.thumbnail_rect.contains(x, y)
        } else {
            false
        }
    }

    /// Update the screen dimensions (e.g., after resolution change).
    pub fn set_screen_size(&mut self, width: f32, height: f32) {
        self.screen_width = width;
        self.screen_height = height;
        // Re-compute layout if active.
        if let Some(anchor) = self.anchor {
            self.layout = Some(self.compute_layout(anchor));
        }
    }

    // ── internal ────────────────────────────────────────────────

    fn compute_layout(&self, anchor: OverviewRect) -> PreviewLayout {
        let pad = self.config.padding;
        let tw = self.config.thumbnail_width;
        let th = self.config.thumbnail_height;
        let title_h = self.config.title_height;
        let close_sz = self.config.close_button_size;

        // Total preview size including padding.
        let total_w = tw + pad * 2.0;
        let total_h = th + title_h + pad * 2.0;

        // Position above the anchor, horizontally centred.
        let mut x = anchor.x + (anchor.width - total_w) / 2.0;
        let mut y = anchor.y - total_h - self.config.anchor_gap;

        // Clamp to screen edges.
        x = x.clamp(0.0, (self.screen_width - total_w).max(0.0));
        y = y.clamp(0.0, (self.screen_height - total_h).max(0.0));

        // If clamped y pushes it into the anchor, try below the anchor.
        if y + total_h > anchor.y - 2.0 {
            let below_y = anchor.y + anchor.height + self.config.anchor_gap;
            if below_y + total_h <= self.screen_height {
                y = below_y;
            }
        }

        let thumb_rect = OverviewRect::new(x + pad, y + pad, tw, th);
        let title_rect = OverviewRect::new(x + pad, y + pad + th, tw - close_sz - 4.0, title_h);
        let close_rect = OverviewRect::new(
            x + pad + tw - close_sz,
            y + pad + th + (title_h - close_sz) / 2.0,
            close_sz,
            close_sz,
        );

        PreviewLayout {
            bounds: OverviewRect::new(x, y, total_w, total_h),
            thumbnail_rect: thumb_rect,
            title_rect: title_rect,
            close_button_rect: close_rect,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_preview() -> TaskbarPreview {
        TaskbarPreview::new(1920.0, 1080.0, PreviewConfig::default())
    }

    fn dock_item_anchor() -> OverviewRect {
        // A dock item near the bottom-centre of the screen.
        OverviewRect::new(910.0, 1020.0, 48.0, 48.0)
    }

    // ── show/hide lifecycle ────────────────────────────────────

    #[test]
    fn starts_hidden() {
        let p = default_preview();
        assert_eq!(p.state, PreviewState::Hidden);
        assert!(!p.is_active());
        assert_eq!(p.opacity(), 0.0);
    }

    #[test]
    fn show_starts_fade_in() {
        let mut p = default_preview();
        p.show(1, "Firefox", dock_item_anchor());
        assert!(matches!(p.state, PreviewState::FadingIn(_)));
        assert!(p.is_active());
        assert_eq!(p.target_window_id, Some(1));
    }

    #[test]
    fn show_same_window_no_restart() {
        let mut p = default_preview();
        p.show(1, "Firefox", dock_item_anchor());
        p.tick(200.0); // complete fade-in
        assert_eq!(p.state, PreviewState::Visible);
        // Showing same window again should not restart animation.
        p.show(1, "Firefox", dock_item_anchor());
        assert_eq!(p.state, PreviewState::Visible);
    }

    #[test]
    fn show_different_window_restarts() {
        let mut p = default_preview();
        p.show(1, "Firefox", dock_item_anchor());
        p.tick(200.0);
        assert_eq!(p.state, PreviewState::Visible);
        // Different window should restart.
        p.show(2, "Terminal", dock_item_anchor());
        assert!(matches!(p.state, PreviewState::FadingIn(_)));
        assert_eq!(p.target_window_id, Some(2));
    }

    #[test]
    fn hide_from_visible() {
        let mut p = default_preview();
        p.show(1, "Firefox", dock_item_anchor());
        p.tick(200.0);
        p.hide();
        assert!(matches!(p.state, PreviewState::FadingOut(_)));
    }

    #[test]
    fn hide_from_hidden_is_noop() {
        let mut p = default_preview();
        p.hide();
        assert_eq!(p.state, PreviewState::Hidden);
    }

    #[test]
    fn hide_from_fading_out_is_noop() {
        let mut p = default_preview();
        p.show(1, "Firefox", dock_item_anchor());
        p.tick(200.0);
        p.hide();
        assert!(matches!(p.state, PreviewState::FadingOut(_)));
        p.hide(); // should not reset progress
        match p.state {
            PreviewState::FadingOut(prog) => assert_eq!(prog, 0.0),
            _ => panic!("Expected FadingOut"),
        }
    }

    // ── tick animation ─────────────────────────────────────────

    #[test]
    fn tick_fade_in_completes() {
        let mut p = default_preview();
        p.show(1, "Test", dock_item_anchor());
        let animating = p.tick(200.0); // default fade_in_ms = 150
        assert!(!animating || p.state == PreviewState::Visible);
        assert_eq!(p.state, PreviewState::Visible);
        assert_eq!(p.opacity(), 1.0);
    }

    #[test]
    fn tick_fade_in_partial() {
        let mut p = default_preview();
        p.show(1, "Test", dock_item_anchor());
        p.tick(75.0); // half of 150ms
        assert!(matches!(p.state, PreviewState::FadingIn(_)));
        let op = p.opacity();
        assert!(op > 0.0 && op < 1.0);
    }

    #[test]
    fn tick_fade_out_completes() {
        let mut p = default_preview();
        p.show(1, "Test", dock_item_anchor());
        p.tick(200.0); // visible
        p.hide();
        let animating = p.tick(200.0); // default fade_out_ms = 100
        assert!(!animating);
        assert_eq!(p.state, PreviewState::Hidden);
        assert_eq!(p.target_window_id, None);
        assert!(p.layout.is_none());
    }

    #[test]
    fn tick_hidden_returns_false() {
        let mut p = default_preview();
        assert!(!p.tick(16.0));
    }

    #[test]
    fn tick_visible_returns_false() {
        let mut p = default_preview();
        p.show(1, "Test", dock_item_anchor());
        p.tick(200.0);
        assert!(!p.tick(16.0));
    }

    // ── layout positioning ─────────────────────────────────────

    #[test]
    fn layout_above_anchor() {
        let mut p = default_preview();
        let anchor = OverviewRect::new(900.0, 900.0, 48.0, 48.0);
        p.show(1, "Test", anchor);
        let layout = p.layout.as_ref().unwrap();
        // Preview should be above the anchor.
        assert!(layout.bounds.y + layout.bounds.height <= anchor.y + 1.0);
    }

    #[test]
    fn layout_clamped_left_edge() {
        let mut p = default_preview();
        // Anchor at the very left edge.
        let anchor = OverviewRect::new(0.0, 500.0, 48.0, 48.0);
        p.show(1, "Test", anchor);
        let layout = p.layout.as_ref().unwrap();
        assert!(layout.bounds.x >= 0.0);
    }

    #[test]
    fn layout_clamped_right_edge() {
        let mut p = default_preview();
        // Anchor at the very right edge.
        let anchor = OverviewRect::new(1900.0, 500.0, 48.0, 48.0);
        p.show(1, "Test", anchor);
        let layout = p.layout.as_ref().unwrap();
        assert!(layout.bounds.x + layout.bounds.width <= 1920.0 + 1.0);
    }

    #[test]
    fn layout_contains_thumbnail_and_title() {
        let mut p = default_preview();
        p.show(1, "Test", OverviewRect::new(500.0, 500.0, 48.0, 48.0));
        let layout = p.layout.as_ref().unwrap();
        // Thumbnail inside bounds.
        assert!(layout.thumbnail_rect.x >= layout.bounds.x);
        assert!(layout.thumbnail_rect.y >= layout.bounds.y);
        // Title below thumbnail.
        assert!(
            layout.title_rect.y >= layout.thumbnail_rect.y + layout.thumbnail_rect.height - 1.0
        );
        // Close button inside bounds.
        assert!(
            layout.close_button_rect.x + layout.close_button_rect.width
                <= layout.bounds.x + layout.bounds.width + 1.0
        );
    }

    // ── hit testing ────────────────────────────────────────────

    #[test]
    fn hit_test_close_inside() {
        let mut p = default_preview();
        p.show(1, "Test", OverviewRect::new(500.0, 500.0, 48.0, 48.0));
        let close = &p.layout.as_ref().unwrap().close_button_rect;
        let cx = close.x + close.width / 2.0;
        let cy = close.y + close.height / 2.0;
        assert!(p.hit_test_close(cx, cy));
    }

    #[test]
    fn hit_test_close_outside() {
        let mut p = default_preview();
        p.show(1, "Test", OverviewRect::new(500.0, 500.0, 48.0, 48.0));
        assert!(!p.hit_test_close(0.0, 0.0));
    }

    #[test]
    fn hit_test_thumbnail_inside() {
        let mut p = default_preview();
        p.show(1, "Test", OverviewRect::new(500.0, 500.0, 48.0, 48.0));
        let thumb = &p.layout.as_ref().unwrap().thumbnail_rect;
        assert!(p.hit_test_thumbnail(thumb.x + 1.0, thumb.y + 1.0));
    }

    #[test]
    fn hit_test_no_layout() {
        let p = default_preview();
        assert!(!p.hit_test_close(500.0, 500.0));
        assert!(!p.hit_test_thumbnail(500.0, 500.0));
    }

    // ── screen resize ──────────────────────────────────────────

    #[test]
    fn set_screen_size_recomputes_layout() {
        let mut p = default_preview();
        p.show(1, "Test", OverviewRect::new(500.0, 500.0, 48.0, 48.0));
        let before = p.layout.as_ref().unwrap().bounds;
        p.set_screen_size(2560.0, 1440.0);
        let after = p.layout.as_ref().unwrap().bounds;
        // Layout should be the same since the anchor is far from edges.
        assert!((before.x - after.x).abs() < 1.0);
    }

    // ── config ─────────────────────────────────────────────────

    #[test]
    fn config_defaults() {
        let cfg = PreviewConfig::default();
        assert_eq!(cfg.thumbnail_width, 240.0);
        assert_eq!(cfg.thumbnail_height, 160.0);
        assert_eq!(cfg.fade_in_ms, 150.0);
        assert_eq!(cfg.fade_out_ms, 100.0);
    }
}
