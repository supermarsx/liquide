//! Expose / window spread view — arranges all windows in a grid for quick
//! selection, similar to GNOME Shell's window picker or macOS Mission Control.

use crate::layout::OverviewRect;

/// State of the expose animation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExposeState {
    /// Expose is not active.
    Inactive,
    /// Windows are spreading to their grid positions. `progress` 0.0..1.0.
    Animating(f32),
    /// All windows are in position; interactive.
    Active,
    /// Windows are collapsing back. `progress` 0.0..1.0.
    Closing(f32),
}

/// A single window's computed slot in the expose grid.
#[derive(Debug, Clone, PartialEq)]
pub struct ExposeSlot {
    /// The window occupying this slot.
    pub window_id: u64,
    /// Where this window's thumbnail should be rendered.
    pub target_rect: OverviewRect,
    /// Thumbnail pixel dimensions for this slot.
    pub thumbnail_size: (u32, u32),
    /// Where the window title label is drawn.
    pub label_rect: OverviewRect,
}

/// Configuration for the expose layout.
#[derive(Debug, Clone)]
pub struct ExposeConfig {
    /// Padding from screen edges.
    pub padding: f32,
    /// Gap between windows in the grid.
    pub gap: f32,
    /// Height reserved for a title label below each thumbnail.
    pub label_height: f32,
    /// Maximum number of columns.
    pub max_columns: u32,
    /// Duration of the spread animation (ms).
    pub animate_in_ms: f32,
    /// Duration of the collapse animation (ms).
    pub animate_out_ms: f32,
}

impl Default for ExposeConfig {
    fn default() -> Self {
        Self {
            padding: 48.0,
            gap: 24.0,
            label_height: 22.0,
            max_columns: 8,
            animate_in_ms: 300.0,
            animate_out_ms: 200.0,
        }
    }
}

/// Input data for expose layout: window ID and its original dimensions.
#[derive(Debug, Clone)]
pub struct ExposeWindow {
    pub id: u64,
    pub title: String,
    pub width: f32,
    pub height: f32,
}

/// Keyboard input for expose navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExposeKey {
    Left,
    Right,
    Up,
    Down,
    Enter,
    Escape,
}

/// Compute the expose layout: a grid of slots that covers `screen_size`.
///
/// Each window is scaled to fit its grid cell while preserving aspect ratio.
/// The grid is centred on screen.
pub fn compute_expose_layout(
    windows: &[ExposeWindow],
    screen_width: f32,
    screen_height: f32,
    config: &ExposeConfig,
) -> Vec<ExposeSlot> {
    if windows.is_empty() {
        return Vec::new();
    }

    let count = windows.len();
    let (cols, rows) = optimal_grid(count, config.max_columns);

    let avail_w = (screen_width - config.padding * 2.0).max(1.0);
    let avail_h = (screen_height - config.padding * 2.0).max(1.0);

    let cell_w = (avail_w - config.gap * (cols as f32 - 1.0).max(0.0)) / cols as f32;
    let cell_h = (avail_h - config.gap * (rows as f32 - 1.0).max(0.0)) / rows as f32;

    let grid_w = cell_w * cols as f32 + config.gap * (cols as f32 - 1.0).max(0.0);
    let grid_h = cell_h * rows as f32 + config.gap * (rows as f32 - 1.0).max(0.0);

    let origin_x = config.padding + (avail_w - grid_w) / 2.0;
    let origin_y = config.padding + (avail_h - grid_h) / 2.0;

    let mut slots = Vec::with_capacity(count);

    for (i, win) in windows.iter().enumerate() {
        let col = i as u32 % cols;
        let row = i as u32 / cols;

        let cx = origin_x + col as f32 * (cell_w + config.gap);
        let cy = origin_y + row as f32 * (cell_h + config.gap);

        let usable_h = (cell_h - config.label_height).max(1.0);

        let orig_w = win.width.max(1.0);
        let orig_h = win.height.max(1.0);
        let sw = cell_w / orig_w;
        let sh = usable_h / orig_h;
        let s = sw.min(sh);

        let tw = orig_w * s;
        let th = orig_h * s;

        // Centre thumbnail in cell.
        let tx = cx + (cell_w - tw) / 2.0;
        let ty = cy + (usable_h - th) / 2.0;

        // Thumbnail pixel size (for requesting the right resolution).
        let thumb_w = tw.round().max(1.0) as u32;
        let thumb_h = th.round().max(1.0) as u32;

        let label_rect = OverviewRect::new(
            cx,
            cy + usable_h + 2.0,
            cell_w,
            config.label_height,
        );

        slots.push(ExposeSlot {
            window_id: win.id,
            target_rect: OverviewRect::new(tx, ty, tw, th),
            thumbnail_size: (thumb_w, thumb_h),
            label_rect,
        });
    }

    slots
}

/// Choose the grid dimensions (cols, rows) for `count` items.
fn optimal_grid(count: usize, max_columns: u32) -> (u32, u32) {
    if count == 0 {
        return (1, 1);
    }
    let n = count as u32;
    let max_c = max_columns.max(1).min(n);

    let mut best_cols = 1u32;
    let mut best_rows = n;
    let mut best_diff = n as i32;

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

/// Hit-test: find which slot (if any) contains the point `(x, y)`.
pub fn select_at_point(slots: &[ExposeSlot], x: f32, y: f32) -> Option<u64> {
    for slot in slots {
        if slot.target_rect.contains(x, y) {
            return Some(slot.window_id);
        }
    }
    None
}

/// Full state machine for the expose view.
pub struct ExposeManager {
    pub state: ExposeState,
    pub slots: Vec<ExposeSlot>,
    pub selected_index: Option<usize>,
    pub hovered_index: Option<usize>,
    config: ExposeConfig,
    grid_cols: u32,
}

impl ExposeManager {
    pub fn new(config: ExposeConfig) -> Self {
        Self {
            state: ExposeState::Inactive,
            slots: Vec::new(),
            selected_index: None,
            hovered_index: None,
            config,
            grid_cols: 1,
        }
    }

    /// Activate the expose view with the given windows.
    pub fn activate(
        &mut self,
        windows: &[ExposeWindow],
        screen_width: f32,
        screen_height: f32,
    ) {
        self.slots = compute_expose_layout(windows, screen_width, screen_height, &self.config);
        self.grid_cols = optimal_grid(windows.len(), self.config.max_columns).0;
        self.state = ExposeState::Animating(0.0);
        self.selected_index = None;
        self.hovered_index = None;
    }

    /// Begin closing the expose view (plays collapse animation).
    pub fn close(&mut self) {
        match self.state {
            ExposeState::Inactive | ExposeState::Closing(_) => {}
            _ => {
                self.state = ExposeState::Closing(0.0);
            }
        }
    }

    /// Advance the animation by `dt_ms` milliseconds.
    ///
    /// Returns `true` while the animation is in progress.
    pub fn tick(&mut self, dt_ms: f32) -> bool {
        match self.state {
            ExposeState::Animating(progress) => {
                let new = progress + dt_ms / self.config.animate_in_ms;
                if new >= 1.0 {
                    self.state = ExposeState::Active;
                } else {
                    self.state = ExposeState::Animating(new);
                }
                true
            }
            ExposeState::Closing(progress) => {
                let new = progress + dt_ms / self.config.animate_out_ms;
                if new >= 1.0 {
                    self.state = ExposeState::Inactive;
                    self.slots.clear();
                    return false;
                }
                self.state = ExposeState::Closing(new);
                true
            }
            _ => false,
        }
    }

    /// Current animation progress (0.0..1.0), useful for interpolating
    /// window positions from original to grid.
    pub fn progress(&self) -> f32 {
        match self.state {
            ExposeState::Animating(p) => p,
            ExposeState::Active => 1.0,
            ExposeState::Closing(p) => 1.0 - p,
            ExposeState::Inactive => 0.0,
        }
    }

    /// Hit-test at screen coordinates. Returns the window ID if a slot is hit.
    pub fn select_at_point(&self, x: f32, y: f32) -> Option<u64> {
        select_at_point(&self.slots, x, y)
    }

    /// Update hover state from mouse position.
    pub fn on_hover(&mut self, x: f32, y: f32) {
        self.hovered_index = None;
        for (i, slot) in self.slots.iter().enumerate() {
            if slot.target_rect.contains(x, y) {
                self.hovered_index = Some(i);
                return;
            }
        }
    }

    /// Handle keyboard navigation. Returns `Some(window_id)` if a window was
    /// selected, or `None` for other actions.
    pub fn on_key(&mut self, key: ExposeKey) -> Option<u64> {
        match key {
            ExposeKey::Escape => {
                self.close();
                None
            }
            ExposeKey::Enter => {
                if let Some(idx) = self.selected_index {
                    if let Some(slot) = self.slots.get(idx) {
                        let id = slot.window_id;
                        self.close();
                        return Some(id);
                    }
                }
                None
            }
            ExposeKey::Left => {
                self.move_selection(-1, 0);
                None
            }
            ExposeKey::Right => {
                self.move_selection(1, 0);
                None
            }
            ExposeKey::Up => {
                self.move_selection(0, -1);
                None
            }
            ExposeKey::Down => {
                self.move_selection(0, 1);
                None
            }
        }
    }

    fn move_selection(&mut self, dx: i32, dy: i32) {
        if self.slots.is_empty() {
            return;
        }
        let cols = self.grid_cols.max(1) as i32;
        let count = self.slots.len() as i32;

        let current = self.selected_index.unwrap_or(0) as i32;
        let col = current % cols;
        let row = current / cols;

        let new_col = (col + dx).clamp(0, cols - 1);
        let new_row = (row + dy).max(0);
        let mut new_idx = new_row * cols + new_col;
        if new_idx >= count {
            new_idx = count - 1;
        }
        if new_idx < 0 {
            new_idx = 0;
        }
        self.selected_index = Some(new_idx as usize);
    }

    /// Whether the expose is visible (active or animating).
    pub fn is_active(&self) -> bool {
        self.state != ExposeState::Inactive
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen() -> (f32, f32) {
        (1920.0, 1080.0)
    }

    fn make_windows(n: usize) -> Vec<ExposeWindow> {
        (0..n)
            .map(|i| ExposeWindow {
                id: i as u64 + 1,
                title: format!("Window {}", i + 1),
                width: 800.0,
                height: 600.0,
            })
            .collect()
    }

    fn default_config() -> ExposeConfig {
        ExposeConfig::default()
    }

    // ── compute_expose_layout ──────────────────────────────────

    #[test]
    fn empty_windows() {
        let slots = compute_expose_layout(&[], 1920.0, 1080.0, &default_config());
        assert!(slots.is_empty());
    }

    #[test]
    fn single_window_centered() {
        let wins = make_windows(1);
        let (sw, sh) = screen();
        let slots = compute_expose_layout(&wins, sw, sh, &default_config());
        assert_eq!(slots.len(), 1);
        let s = &slots[0];
        let mid_x = s.target_rect.x + s.target_rect.width / 2.0;
        assert!((mid_x - sw / 2.0).abs() < 2.0);
    }

    #[test]
    fn four_windows_grid() {
        let wins = make_windows(4);
        let (sw, sh) = screen();
        let slots = compute_expose_layout(&wins, sw, sh, &default_config());
        assert_eq!(slots.len(), 4);
        // Should form a 2x2 grid.
        assert!((slots[0].target_rect.y - slots[1].target_rect.y).abs() < 1.0);
        assert!(slots[2].target_rect.y > slots[0].target_rect.y);
    }

    #[test]
    fn slots_within_screen() {
        let wins = make_windows(12);
        let (sw, sh) = screen();
        let slots = compute_expose_layout(&wins, sw, sh, &default_config());
        for s in &slots {
            assert!(s.target_rect.x >= 0.0);
            assert!(s.target_rect.y >= 0.0);
            assert!(s.target_rect.x + s.target_rect.width <= sw + 1.0);
            assert!(s.target_rect.y + s.target_rect.height <= sh + 1.0);
        }
    }

    #[test]
    fn aspect_ratio_preserved() {
        let wins = vec![ExposeWindow {
            id: 1,
            title: "Wide".into(),
            width: 1600.0,
            height: 400.0,
        }];
        let slots = compute_expose_layout(&wins, 1920.0, 1080.0, &default_config());
        let s = &slots[0];
        let src_ratio = 1600.0 / 400.0;
        let dst_ratio = s.target_rect.width / s.target_rect.height;
        assert!((src_ratio - dst_ratio).abs() < 0.1);
    }

    #[test]
    fn label_below_thumbnail() {
        let wins = make_windows(2);
        let slots = compute_expose_layout(&wins, 1920.0, 1080.0, &default_config());
        for s in &slots {
            assert!(s.label_rect.y >= s.target_rect.y + s.target_rect.height - 1.0);
        }
    }

    #[test]
    fn thumbnail_size_positive() {
        let wins = make_windows(6);
        let slots = compute_expose_layout(&wins, 1920.0, 1080.0, &default_config());
        for s in &slots {
            assert!(s.thumbnail_size.0 >= 1);
            assert!(s.thumbnail_size.1 >= 1);
        }
    }

    #[test]
    fn window_ids_preserved() {
        let wins = make_windows(5);
        let slots = compute_expose_layout(&wins, 1920.0, 1080.0, &default_config());
        for (w, s) in wins.iter().zip(slots.iter()) {
            assert_eq!(w.id, s.window_id);
        }
    }

    // ── select_at_point ────────────────────────────────────────

    #[test]
    fn hit_test_inside() {
        let wins = make_windows(4);
        let slots = compute_expose_layout(&wins, 1920.0, 1080.0, &default_config());
        let s = &slots[0];
        let x = s.target_rect.x + s.target_rect.width / 2.0;
        let y = s.target_rect.y + s.target_rect.height / 2.0;
        assert_eq!(select_at_point(&slots, x, y), Some(1));
    }

    #[test]
    fn hit_test_outside() {
        let wins = make_windows(4);
        let slots = compute_expose_layout(&wins, 1920.0, 1080.0, &default_config());
        assert_eq!(select_at_point(&slots, 0.0, 0.0), None);
    }

    #[test]
    fn hit_test_empty() {
        assert_eq!(select_at_point(&[], 100.0, 100.0), None);
    }

    // ── ExposeManager lifecycle ────────────────────────────────

    #[test]
    fn starts_inactive() {
        let mgr = ExposeManager::new(default_config());
        assert_eq!(mgr.state, ExposeState::Inactive);
        assert!(!mgr.is_active());
    }

    #[test]
    fn activate_starts_animation() {
        let mut mgr = ExposeManager::new(default_config());
        let wins = make_windows(4);
        let (sw, sh) = screen();
        mgr.activate(&wins, sw, sh);
        assert!(matches!(mgr.state, ExposeState::Animating(_)));
        assert!(mgr.is_active());
        assert_eq!(mgr.slots.len(), 4);
    }

    #[test]
    fn tick_animation_completes() {
        let mut mgr = ExposeManager::new(default_config());
        mgr.activate(&make_windows(2), 1920.0, 1080.0);
        mgr.tick(400.0); // > 300ms default
        assert_eq!(mgr.state, ExposeState::Active);
        assert_eq!(mgr.progress(), 1.0);
    }

    #[test]
    fn tick_animation_partial() {
        let mut mgr = ExposeManager::new(default_config());
        mgr.activate(&make_windows(2), 1920.0, 1080.0);
        mgr.tick(150.0); // half of 300ms
        assert!(matches!(mgr.state, ExposeState::Animating(_)));
        let p = mgr.progress();
        assert!(p > 0.3 && p < 0.7);
    }

    #[test]
    fn close_starts_collapse() {
        let mut mgr = ExposeManager::new(default_config());
        mgr.activate(&make_windows(2), 1920.0, 1080.0);
        mgr.tick(400.0);
        mgr.close();
        assert!(matches!(mgr.state, ExposeState::Closing(_)));
    }

    #[test]
    fn close_animation_completes() {
        let mut mgr = ExposeManager::new(default_config());
        mgr.activate(&make_windows(2), 1920.0, 1080.0);
        mgr.tick(400.0);
        mgr.close();
        mgr.tick(300.0);
        assert_eq!(mgr.state, ExposeState::Inactive);
        assert!(mgr.slots.is_empty());
    }

    #[test]
    fn close_from_inactive_is_noop() {
        let mut mgr = ExposeManager::new(default_config());
        mgr.close();
        assert_eq!(mgr.state, ExposeState::Inactive);
    }

    #[test]
    fn tick_inactive_returns_false() {
        let mut mgr = ExposeManager::new(default_config());
        assert!(!mgr.tick(16.0));
    }

    // ── keyboard navigation ────────────────────────────────────

    #[test]
    fn arrow_right_selects() {
        let mut mgr = ExposeManager::new(default_config());
        mgr.activate(&make_windows(4), 1920.0, 1080.0);
        mgr.tick(400.0);
        mgr.on_key(ExposeKey::Right);
        assert_eq!(mgr.selected_index, Some(1));
    }

    #[test]
    fn arrow_down_moves_row() {
        let mut mgr = ExposeManager::new(default_config());
        mgr.activate(&make_windows(4), 1920.0, 1080.0);
        mgr.tick(400.0);
        mgr.on_key(ExposeKey::Down);
        // 2x2 grid, so down from row 0 goes to row 1.
        assert!(mgr.selected_index.unwrap() >= 1);
    }

    #[test]
    fn arrow_left_clamps() {
        let mut mgr = ExposeManager::new(default_config());
        mgr.activate(&make_windows(4), 1920.0, 1080.0);
        mgr.tick(400.0);
        mgr.selected_index = Some(0);
        mgr.on_key(ExposeKey::Left);
        assert_eq!(mgr.selected_index, Some(0));
    }

    #[test]
    fn enter_selects_window() {
        let mut mgr = ExposeManager::new(default_config());
        mgr.activate(&make_windows(4), 1920.0, 1080.0);
        mgr.tick(400.0);
        mgr.selected_index = Some(2);
        let result = mgr.on_key(ExposeKey::Enter);
        assert_eq!(result, Some(3)); // window id=3
        assert!(matches!(mgr.state, ExposeState::Closing(_)));
    }

    #[test]
    fn enter_no_selection() {
        let mut mgr = ExposeManager::new(default_config());
        mgr.activate(&make_windows(4), 1920.0, 1080.0);
        mgr.tick(400.0);
        let result = mgr.on_key(ExposeKey::Enter);
        assert_eq!(result, None);
    }

    #[test]
    fn escape_closes() {
        let mut mgr = ExposeManager::new(default_config());
        mgr.activate(&make_windows(2), 1920.0, 1080.0);
        mgr.tick(400.0);
        let result = mgr.on_key(ExposeKey::Escape);
        assert_eq!(result, None);
        assert!(matches!(mgr.state, ExposeState::Closing(_)));
    }

    // ── hover ──────────────────────────────────────────────────

    #[test]
    fn hover_sets_index() {
        let mut mgr = ExposeManager::new(default_config());
        mgr.activate(&make_windows(4), 1920.0, 1080.0);
        mgr.tick(400.0);
        let s = &mgr.slots[0];
        mgr.on_hover(s.target_rect.x + 1.0, s.target_rect.y + 1.0);
        assert_eq!(mgr.hovered_index, Some(0));
    }

    #[test]
    fn hover_outside_clears() {
        let mut mgr = ExposeManager::new(default_config());
        mgr.activate(&make_windows(4), 1920.0, 1080.0);
        mgr.tick(400.0);
        mgr.hovered_index = Some(0);
        mgr.on_hover(0.0, 0.0);
        assert_eq!(mgr.hovered_index, None);
    }

    // ── optimal_grid ───────────────────────────────────────────

    #[test]
    fn grid_one() {
        assert_eq!(optimal_grid(1, 8), (1, 1));
    }

    #[test]
    fn grid_four() {
        let (c, r) = optimal_grid(4, 8);
        assert_eq!(c, 2);
        assert_eq!(r, 2);
    }

    #[test]
    fn grid_nine() {
        let (c, r) = optimal_grid(9, 8);
        assert!(c * r >= 9);
    }

    #[test]
    fn grid_zero() {
        assert_eq!(optimal_grid(0, 8), (1, 1));
    }

    // ── progress tracking ──────────────────────────────────────

    #[test]
    fn progress_inactive_is_zero() {
        let mgr = ExposeManager::new(default_config());
        assert_eq!(mgr.progress(), 0.0);
    }

    #[test]
    fn progress_active_is_one() {
        let mut mgr = ExposeManager::new(default_config());
        mgr.activate(&make_windows(1), 1920.0, 1080.0);
        mgr.tick(400.0);
        assert_eq!(mgr.progress(), 1.0);
    }

    #[test]
    fn progress_during_close_decreases() {
        let mut mgr = ExposeManager::new(default_config());
        mgr.activate(&make_windows(1), 1920.0, 1080.0);
        mgr.tick(400.0);
        mgr.close();
        mgr.tick(100.0); // half of 200ms
        let p = mgr.progress();
        assert!(p > 0.0 && p < 1.0);
    }

    #[test]
    fn manager_select_at_point() {
        let mut mgr = ExposeManager::new(default_config());
        mgr.activate(&make_windows(4), 1920.0, 1080.0);
        mgr.tick(400.0);
        let s = &mgr.slots[1];
        let cx = s.target_rect.x + s.target_rect.width / 2.0;
        let cy = s.target_rect.y + s.target_rect.height / 2.0;
        assert_eq!(mgr.select_at_point(cx, cy), Some(2));
    }
}
