use crate::animation::OverviewAnimator;
use crate::layout::{
    compute_overview_layout, LayoutConfig, OverviewRect, OverviewSlot, WindowInfo,
};
use crate::search::OverviewSearch;

/// Keyboard input events understood by the overview.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverviewKey {
    Up,
    Down,
    Left,
    Right,
    Enter,
    Escape,
    Char(char),
    Backspace,
}

/// Actions emitted by the overview in response to user input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverviewAction {
    None,
    SelectWindow(u64),
    SwitchWorkspace(u32),
    Close,
    CloseWindow(u64),
}

/// Full state machine for the overview mode.
pub struct OverviewState {
    pub active: bool,
    pub animator: OverviewAnimator,
    pub search: OverviewSearch,
    pub selected_index: Option<usize>,
    pub hovered_index: Option<usize>,
    pub workspace_hovered: Option<u32>,

    // Internal bookkeeping.
    windows: Vec<WindowInfo>,
    viewport: OverviewRect,
    config: LayoutConfig,
    slots: Vec<OverviewSlot>,
    /// Columns in the current grid (for arrow-key navigation).
    grid_cols: u32,
}

impl OverviewState {
    pub fn new() -> Self {
        Self {
            active: false,
            animator: OverviewAnimator::new(),
            search: OverviewSearch::new(),
            selected_index: None,
            hovered_index: None,
            workspace_hovered: None,
            windows: Vec::new(),
            viewport: OverviewRect::new(0.0, 0.0, 1.0, 1.0),
            config: LayoutConfig::default(),
            slots: Vec::new(),
            grid_cols: 1,
        }
    }

    /// Enter overview mode, arranging the supplied windows in a grid.
    pub fn activate(
        &mut self,
        windows: Vec<WindowInfo>,
        viewport: OverviewRect,
        config: LayoutConfig,
    ) {
        self.viewport = viewport;
        self.config = config;
        self.selected_index = None;
        self.hovered_index = None;
        self.workspace_hovered = None;
        self.search.clear();

        let originals: Vec<(u64, OverviewRect)> =
            windows.iter().map(|w| (w.id, w.original)).collect();

        self.slots = compute_overview_layout(&windows, self.viewport, &self.config);
        self.grid_cols = compute_grid_cols(windows.len());
        self.windows = windows;

        self.animator.begin_enter(self.slots.clone(), &originals);
        self.active = true;
    }

    /// Begin exiting overview mode (plays exit animation).
    pub fn deactivate(&mut self) {
        self.animator.begin_exit();
        self.active = false;
    }

    /// Advance the animation. Returns true while animation is in progress.
    pub fn tick(&mut self, dt_ms: f32) -> bool {
        self.animator.tick(dt_ms)
    }

    /// The current layout slots (after any search filtering).
    pub fn current_slots(&self) -> &[OverviewSlot] {
        &self.slots
    }

    /// Handle a keyboard event. Returns the resulting action.
    pub fn on_key(&mut self, key: OverviewKey) -> OverviewAction {
        match key {
            OverviewKey::Escape => {
                self.deactivate();
                OverviewAction::Close
            }
            OverviewKey::Enter => {
                if let Some(idx) = self.selected_index {
                    if let Some(slot) = self.slots.get(idx) {
                        let id = slot.window_id;
                        self.deactivate();
                        return OverviewAction::SelectWindow(id);
                    }
                }
                OverviewAction::None
            }
            OverviewKey::Char(c) => {
                self.search.push_char(c);
                self.relayout_filtered();
                OverviewAction::None
            }
            OverviewKey::Backspace => {
                self.search.pop_char();
                self.relayout_filtered();
                OverviewAction::None
            }
            OverviewKey::Left => {
                self.move_selection(-1, 0);
                OverviewAction::None
            }
            OverviewKey::Right => {
                self.move_selection(1, 0);
                OverviewAction::None
            }
            OverviewKey::Up => {
                self.move_selection(0, -1);
                OverviewAction::None
            }
            OverviewKey::Down => {
                self.move_selection(0, 1);
                OverviewAction::None
            }
        }
    }

    /// Handle a mouse click at `(x, y)`. Hit-tests against the current slots.
    pub fn on_click(&mut self, x: f32, y: f32) -> OverviewAction {
        for (i, slot) in self.slots.iter().enumerate() {
            if slot.target.contains(x, y) {
                self.selected_index = Some(i);
                let id = slot.window_id;
                self.deactivate();
                return OverviewAction::SelectWindow(id);
            }
        }
        OverviewAction::None
    }

    /// Update hover state based on mouse position.
    pub fn on_hover(&mut self, x: f32, y: f32) {
        self.hovered_index = None;
        for (i, slot) in self.slots.iter().enumerate() {
            if slot.target.contains(x, y) {
                self.hovered_index = Some(i);
                return;
            }
        }
    }

    // ── internal ────────────────────────────────────────────────

    fn relayout_filtered(&mut self) {
        let filtered = self.search.filter_windows(&self.windows);
        let infos: Vec<WindowInfo> = filtered.into_iter().cloned().collect();
        self.slots = compute_overview_layout(&infos, self.viewport, &self.config);
        self.grid_cols = compute_grid_cols(self.slots.len());

        // Reset selection if it's now out of range.
        if let Some(idx) = self.selected_index {
            if idx >= self.slots.len() {
                self.selected_index = if self.slots.is_empty() {
                    None
                } else {
                    Some(self.slots.len() - 1)
                };
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
}

fn compute_grid_cols(count: usize) -> u32 {
    if count <= 1 {
        return 1;
    }
    let sq = (count as f32).sqrt().ceil() as u32;
    sq.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn viewport() -> OverviewRect {
        OverviewRect::new(0.0, 0.0, 1920.0, 1080.0)
    }

    fn make_windows(n: usize) -> Vec<WindowInfo> {
        (0..n)
            .map(|i| WindowInfo {
                id: i as u64 + 1,
                title: format!("Window {}", i + 1),
                original: OverviewRect::new(
                    (i as f32) * 50.0,
                    (i as f32) * 30.0,
                    800.0,
                    600.0,
                ),
                workspace: 0,
                monitor: 0,
            })
            .collect()
    }

    #[test]
    fn activate_deactivate_cycle() {
        let mut state = OverviewState::new();
        assert!(!state.active);
        state.activate(make_windows(4), viewport(), LayoutConfig::default());
        assert!(state.active);
        assert_eq!(state.current_slots().len(), 4);
        state.deactivate();
        assert!(!state.active);
    }

    #[test]
    fn escape_closes_overview() {
        let mut state = OverviewState::new();
        state.activate(make_windows(2), viewport(), LayoutConfig::default());
        let action = state.on_key(OverviewKey::Escape);
        assert_eq!(action, OverviewAction::Close);
        assert!(!state.active);
    }

    #[test]
    fn keyboard_navigation_right() {
        let mut state = OverviewState::new();
        state.activate(make_windows(4), viewport(), LayoutConfig::default());
        state.on_key(OverviewKey::Right);
        assert_eq!(state.selected_index, Some(1));
    }

    #[test]
    fn keyboard_navigation_down() {
        let mut state = OverviewState::new();
        state.activate(make_windows(4), viewport(), LayoutConfig::default());
        state.on_key(OverviewKey::Down);
        assert!(state.selected_index.is_some());
        let idx = state.selected_index.unwrap();
        assert!(idx >= 1); // Moved down from row 0.
    }

    #[test]
    fn keyboard_navigation_clamps() {
        let mut state = OverviewState::new();
        state.activate(make_windows(2), viewport(), LayoutConfig::default());
        // Try going left from index 0 — should stay at 0.
        state.selected_index = Some(0);
        state.on_key(OverviewKey::Left);
        assert_eq!(state.selected_index, Some(0));
    }

    #[test]
    fn enter_selects_window() {
        let mut state = OverviewState::new();
        state.activate(make_windows(3), viewport(), LayoutConfig::default());
        state.selected_index = Some(1);
        let action = state.on_key(OverviewKey::Enter);
        assert_eq!(action, OverviewAction::SelectWindow(2));
    }

    #[test]
    fn enter_without_selection_is_none() {
        let mut state = OverviewState::new();
        state.activate(make_windows(3), viewport(), LayoutConfig::default());
        let action = state.on_key(OverviewKey::Enter);
        assert_eq!(action, OverviewAction::None);
    }

    #[test]
    fn click_selection() {
        let mut state = OverviewState::new();
        state.activate(make_windows(1), viewport(), LayoutConfig::default());
        let slot = &state.current_slots()[0];
        let cx = slot.target.x + slot.target.width / 2.0;
        let cy = slot.target.y + slot.target.height / 2.0;
        let action = state.on_click(cx, cy);
        assert_eq!(action, OverviewAction::SelectWindow(1));
    }

    #[test]
    fn click_outside_returns_none() {
        let mut state = OverviewState::new();
        state.activate(make_windows(1), viewport(), LayoutConfig::default());
        let action = state.on_click(0.0, 0.0);
        assert_eq!(action, OverviewAction::None);
    }

    #[test]
    fn hover_updates_index() {
        let mut state = OverviewState::new();
        state.activate(make_windows(1), viewport(), LayoutConfig::default());
        let slot = &state.current_slots()[0];
        let cx = slot.target.x + 1.0;
        let cy = slot.target.y + 1.0;
        state.on_hover(cx, cy);
        assert_eq!(state.hovered_index, Some(0));
    }

    #[test]
    fn hover_outside_clears() {
        let mut state = OverviewState::new();
        state.activate(make_windows(1), viewport(), LayoutConfig::default());
        state.hovered_index = Some(0);
        state.on_hover(0.0, 0.0);
        assert_eq!(state.hovered_index, None);
    }

    #[test]
    fn search_filtering() {
        let mut state = OverviewState::new();
        let mut wins = make_windows(3);
        wins[0].title = "Firefox".into();
        wins[1].title = "Terminal".into();
        wins[2].title = "Files".into();
        state.activate(wins, viewport(), LayoutConfig::default());
        assert_eq!(state.current_slots().len(), 3);

        state.on_key(OverviewKey::Char('f'));
        // "Firefox" and "Files" match.
        assert_eq!(state.current_slots().len(), 2);
    }

    #[test]
    fn search_backspace_restores() {
        let mut state = OverviewState::new();
        let mut wins = make_windows(3);
        wins[0].title = "Firefox".into();
        wins[1].title = "Terminal".into();
        wins[2].title = "Files".into();
        state.activate(wins, viewport(), LayoutConfig::default());

        state.on_key(OverviewKey::Char('t'));
        assert_eq!(state.current_slots().len(), 1); // "Terminal"
        state.on_key(OverviewKey::Backspace);
        assert_eq!(state.current_slots().len(), 3); // all back
    }

    #[test]
    fn tick_advances_animation() {
        let mut state = OverviewState::new();
        state.activate(make_windows(2), viewport(), LayoutConfig::default());
        let animating = state.tick(16.0);
        assert!(animating);
    }

    #[test]
    fn selected_index_clamped_on_search() {
        let mut state = OverviewState::new();
        let mut wins = make_windows(5);
        wins[0].title = "A".into();
        wins[1].title = "B".into();
        wins[2].title = "C".into();
        wins[3].title = "D".into();
        wins[4].title = "E".into();
        state.activate(wins, viewport(), LayoutConfig::default());
        state.selected_index = Some(4);
        // Filter to 1 result.
        state.on_key(OverviewKey::Char('a'));
        assert!(state.selected_index.unwrap_or(0) < state.current_slots().len());
    }
}
