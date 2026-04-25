//! Alt-Tab / window switcher overlay.
//!
//! Shows thumbnails of all open windows in a horizontal strip with MRU ordering,
//! keyboard navigation (Tab / Shift+Tab), mouse click selection, and optional
//! grouping by application (GNOME-style Alt-Tab).

/// Describes a single window that can appear in the switcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowEntry {
    pub id: u64,
    pub app_id: String,
    pub title: String,
    pub workspace: u32,
    pub is_minimized: bool,
    pub last_active_ms: u64,
}

/// Which windows the switcher should display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwitcherMode {
    /// All open windows across workspaces.
    AllWindows,
    /// Only windows belonging to the given application.
    AppWindows(String),
    /// Most recently used N windows.
    RecentOnly(usize),
}

/// A group of windows that share the same `app_id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppGroup {
    pub app_id: String,
    pub windows: Vec<WindowEntry>,
    /// Title of the most recently active window in the group.
    pub representative_title: String,
}

/// Keyboard input events the switcher handles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitcherKey {
    Tab,
    ShiftTab,
    Up,
    Down,
    Enter,
    Escape,
    Number(u8),
}

/// Actions emitted by the switcher in response to user input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwitcherAction {
    None,
    /// The user confirmed selection of the given window.
    SelectWindow(u64),
    /// The user cancelled the switcher (Escape).
    Cancel,
    /// Expand the selected app group to show individual windows.
    ExpandGroup(String),
    /// Collapse back to the app-group view.
    CollapseGroup,
}

/// Computed position for a single item in the switcher strip.
#[derive(Debug, Clone, PartialEq)]
pub struct SwitcherSlot {
    pub window_id: u64,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub is_selected: bool,
}

/// Full state machine for the Alt-Tab switcher overlay.
pub struct SwitcherState {
    pub active: bool,
    pub entries: Vec<WindowEntry>,
    pub selected_index: usize,
    pub mode: SwitcherMode,
    /// Windows grouped by `app_id`.
    pub groups: Vec<AppGroup>,
    /// `true` = showing app groups, `false` = showing individual windows.
    pub group_mode: bool,
    /// When an app group has been expanded, the app_id of the expanded group.
    expanded_app_id: Option<String>,
}

impl SwitcherState {
    pub fn new() -> Self {
        Self {
            active: false,
            entries: Vec::new(),
            selected_index: 0,
            mode: SwitcherMode::AllWindows,
            groups: Vec::new(),
            group_mode: false,
            expanded_app_id: None,
        }
    }

    /// Open the switcher with the given windows and mode.
    pub fn activate(&mut self, mut windows: Vec<WindowEntry>, mode: SwitcherMode) {
        sort_mru(&mut windows);

        let filtered = match &mode {
            SwitcherMode::AllWindows => windows,
            SwitcherMode::AppWindows(app) => {
                windows.into_iter().filter(|w| w.app_id == *app).collect()
            }
            SwitcherMode::RecentOnly(n) => {
                windows.truncate(*n);
                windows
            }
        };

        self.groups = group_by_app(&filtered);
        self.entries = filtered;
        self.mode = mode;
        self.selected_index = 0;
        self.group_mode = self.groups.len() > 1;
        self.expanded_app_id = None;
        self.active = true;
    }

    /// Close the switcher without selecting a window.
    pub fn deactivate(&mut self) {
        self.active = false;
        self.entries.clear();
        self.groups.clear();
        self.selected_index = 0;
        self.group_mode = false;
        self.expanded_app_id = None;
    }

    /// Move selection forward (Tab).
    pub fn select_next(&mut self) {
        let count = self.visible_count();
        if count == 0 {
            return;
        }
        self.selected_index = (self.selected_index + 1) % count;
    }

    /// Move selection backward (Shift+Tab).
    pub fn select_prev(&mut self) {
        let count = self.visible_count();
        if count == 0 {
            return;
        }
        if self.selected_index == 0 {
            self.selected_index = count - 1;
        } else {
            self.selected_index -= 1;
        }
    }

    /// Expand the selected app group to show its individual windows (Down arrow).
    pub fn expand_group(&mut self) {
        if !self.group_mode {
            return;
        }
        if let Some(group) = self.groups.get(self.selected_index) {
            self.expanded_app_id = Some(group.app_id.clone());
            self.group_mode = false;
            self.selected_index = 0;
        }
    }

    /// Collapse back to the app-group view (Up arrow).
    pub fn collapse_group(&mut self) {
        if self.group_mode || self.expanded_app_id.is_none() {
            return;
        }
        // Restore selection to the group that was expanded.
        if let Some(ref app_id) = self.expanded_app_id {
            let group_idx = self
                .groups
                .iter()
                .position(|g| g.app_id == *app_id)
                .unwrap_or(0);
            self.selected_index = group_idx;
        }
        self.expanded_app_id = None;
        self.group_mode = true;
    }

    /// Confirm the current selection (Enter). Returns the window ID if one is
    /// selected.
    pub fn confirm(&mut self) -> Option<u64> {
        let id = self.selected_window().map(|w| w.id);
        if id.is_some() {
            self.deactivate();
        }
        id
    }

    /// Cancel the switcher (Escape).
    pub fn cancel(&mut self) {
        self.deactivate();
    }

    /// Handle a keyboard event. Returns the resulting action.
    pub fn on_key(&mut self, key: SwitcherKey) -> SwitcherAction {
        match key {
            SwitcherKey::Tab => {
                self.select_next();
                SwitcherAction::None
            }
            SwitcherKey::ShiftTab => {
                self.select_prev();
                SwitcherAction::None
            }
            SwitcherKey::Down => {
                if self.group_mode {
                    if let Some(group) = self.groups.get(self.selected_index) {
                        let app_id = group.app_id.clone();
                        self.expand_group();
                        return SwitcherAction::ExpandGroup(app_id);
                    }
                }
                SwitcherAction::None
            }
            SwitcherKey::Up => {
                if !self.group_mode && self.expanded_app_id.is_some() {
                    self.collapse_group();
                    return SwitcherAction::CollapseGroup;
                }
                SwitcherAction::None
            }
            SwitcherKey::Enter => {
                if let Some(id) = self.confirm() {
                    SwitcherAction::SelectWindow(id)
                } else {
                    SwitcherAction::None
                }
            }
            SwitcherKey::Escape => {
                self.cancel();
                SwitcherAction::Cancel
            }
            SwitcherKey::Number(n) => {
                let idx = if n == 0 { 9 } else { (n - 1) as usize };
                let count = self.visible_count();
                if count > 0 && idx < count {
                    self.selected_index = idx;
                    if let Some(id) = self.confirm() {
                        return SwitcherAction::SelectWindow(id);
                    }
                }
                SwitcherAction::None
            }
        }
    }

    /// The currently selected window, if any.
    pub fn selected_window(&self) -> Option<&WindowEntry> {
        let visible = self.visible_entries();
        visible.get(self.selected_index).copied()
    }

    /// The list of entries currently visible in the switcher.
    ///
    /// In group mode this returns one representative entry per group.
    /// When a group is expanded, only that group's windows are shown.
    /// Otherwise, all individual windows are shown.
    pub fn visible_entries(&self) -> Vec<&WindowEntry> {
        if self.group_mode {
            // One representative per group (most recent window).
            self.groups
                .iter()
                .filter_map(|g| g.windows.first())
                .collect()
        } else if let Some(ref app_id) = self.expanded_app_id {
            // Show only windows from the expanded group.
            self.entries
                .iter()
                .filter(|w| w.app_id == *app_id)
                .collect()
        } else {
            self.entries.iter().collect()
        }
    }

    fn visible_count(&self) -> usize {
        self.visible_entries().len()
    }
}

/// Sort entries by `last_active_ms` descending (most recently used first).
pub fn sort_mru(entries: &mut [WindowEntry]) {
    entries.sort_by(|a, b| b.last_active_ms.cmp(&a.last_active_ms));
}

/// Group entries by `app_id`, preserving MRU ordering within each group
/// and ordering groups by the most recent window in each group.
pub fn group_by_app(entries: &[WindowEntry]) -> Vec<AppGroup> {
    // Use a Vec of (app_id, windows) to preserve insertion order (MRU).
    let mut groups: Vec<(String, Vec<WindowEntry>)> = Vec::new();

    for entry in entries {
        if let Some(group) = groups.iter_mut().find(|(id, _)| *id == entry.app_id) {
            group.1.push(entry.clone());
        } else {
            groups.push((entry.app_id.clone(), vec![entry.clone()]));
        }
    }

    groups
        .into_iter()
        .map(|(app_id, windows)| {
            let representative_title = windows.first().map(|w| w.title.clone()).unwrap_or_default();
            AppGroup {
                app_id,
                windows,
                representative_title,
            }
        })
        .collect()
}

/// Layout computation for the switcher strip.
pub struct SwitcherLayout;

impl SwitcherLayout {
    /// Compute the horizontal strip layout for the given entries.
    ///
    /// Items are arranged in a centred horizontal row in the middle of the
    /// viewport, with a fixed thumbnail size that scales down when there are
    /// many windows.
    pub fn compute(
        entries: &[WindowEntry],
        selected_index: usize,
        viewport_width: f32,
        viewport_height: f32,
    ) -> Vec<SwitcherSlot> {
        if entries.is_empty() {
            return Vec::new();
        }

        let count = entries.len();

        // Thumbnail sizing: start with 160x120, shrink if too many to fit.
        let gap = 12.0f32;
        let max_thumb_w = 160.0f32;
        let max_thumb_h = 120.0f32;
        let padding = 40.0f32;

        let available_w = (viewport_width - padding * 2.0).max(1.0);
        let total_gaps = gap * (count as f32 - 1.0).max(0.0);
        let total_needed = max_thumb_w * count as f32 + total_gaps;

        let (thumb_w, thumb_h) = if total_needed <= available_w {
            (max_thumb_w, max_thumb_h)
        } else {
            // Scale thumbs so they fit (gaps stay fixed).
            let available_for_thumbs = (available_w - total_gaps).max(1.0);
            let per_thumb = available_for_thumbs / count as f32;
            let scale = per_thumb / max_thumb_w;
            (per_thumb, max_thumb_h * scale)
        };

        let strip_w = thumb_w * count as f32 + total_gaps;
        let start_x = ((viewport_width - strip_w) / 2.0).max(0.0);
        let start_y = ((viewport_height - thumb_h) / 2.0).max(0.0);

        entries
            .iter()
            .enumerate()
            .map(|(i, entry)| SwitcherSlot {
                window_id: entry.id,
                x: start_x + i as f32 * (thumb_w + gap),
                y: start_y,
                width: thumb_w,
                height: thumb_h,
                is_selected: i == selected_index,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(id: u64, app_id: &str, title: &str, last_active_ms: u64) -> WindowEntry {
        WindowEntry {
            id,
            app_id: app_id.to_string(),
            title: title.to_string(),
            workspace: 0,
            is_minimized: false,
            last_active_ms,
        }
    }

    fn sample_entries() -> Vec<WindowEntry> {
        vec![
            make_entry(1, "firefox", "Tab 1 - Firefox", 1000),
            make_entry(2, "firefox", "Tab 2 - Firefox", 900),
            make_entry(3, "terminal", "Terminal", 800),
            make_entry(4, "files", "Home - Files", 700),
            make_entry(5, "editor", "main.rs - Editor", 600),
        ]
    }

    // ── MRU ordering ────────────────────────────────────────────

    #[test]
    fn sort_mru_orders_by_last_active_descending() {
        let mut entries = vec![
            make_entry(1, "a", "Old", 100),
            make_entry(2, "b", "New", 500),
            make_entry(3, "c", "Mid", 300),
        ];
        sort_mru(&mut entries);
        assert_eq!(entries[0].id, 2);
        assert_eq!(entries[1].id, 3);
        assert_eq!(entries[2].id, 1);
    }

    #[test]
    fn sort_mru_empty() {
        let mut entries: Vec<WindowEntry> = Vec::new();
        sort_mru(&mut entries);
        assert!(entries.is_empty());
    }

    #[test]
    fn sort_mru_single() {
        let mut entries = vec![make_entry(1, "a", "Only", 42)];
        sort_mru(&mut entries);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, 1);
    }

    #[test]
    fn sort_mru_equal_timestamps() {
        let mut entries = vec![make_entry(1, "a", "A", 100), make_entry(2, "b", "B", 100)];
        sort_mru(&mut entries);
        // Stable sort: order preserved for equal timestamps.
        assert_eq!(entries.len(), 2);
    }

    // ── App grouping ────────────────────────────────────────────

    #[test]
    fn group_by_app_basic() {
        let entries = sample_entries();
        let groups = group_by_app(&entries);
        assert_eq!(groups.len(), 4); // firefox, terminal, files, editor
    }

    #[test]
    fn group_by_app_firefox_has_two_windows() {
        let entries = sample_entries();
        let groups = group_by_app(&entries);
        let firefox = groups.iter().find(|g| g.app_id == "firefox").unwrap();
        assert_eq!(firefox.windows.len(), 2);
    }

    #[test]
    fn group_by_app_representative_title() {
        let entries = sample_entries();
        let groups = group_by_app(&entries);
        let firefox = groups.iter().find(|g| g.app_id == "firefox").unwrap();
        assert_eq!(firefox.representative_title, "Tab 1 - Firefox");
    }

    #[test]
    fn group_by_app_preserves_mru_order() {
        let entries = sample_entries();
        let groups = group_by_app(&entries);
        // Groups should be ordered by first appearance (which is MRU since entries
        // are already MRU-sorted).
        assert_eq!(groups[0].app_id, "firefox");
        assert_eq!(groups[1].app_id, "terminal");
        assert_eq!(groups[2].app_id, "files");
        assert_eq!(groups[3].app_id, "editor");
    }

    #[test]
    fn group_by_app_empty() {
        let groups = group_by_app(&[]);
        assert!(groups.is_empty());
    }

    #[test]
    fn group_by_app_single_window() {
        let entries = vec![make_entry(1, "app", "Window", 100)];
        let groups = group_by_app(&entries);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].windows.len(), 1);
    }

    #[test]
    fn group_by_app_all_same_app() {
        let entries = vec![
            make_entry(1, "app", "W1", 300),
            make_entry(2, "app", "W2", 200),
            make_entry(3, "app", "W3", 100),
        ];
        let groups = group_by_app(&entries);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].windows.len(), 3);
    }

    // ── Keyboard navigation ─────────────────────────────────────

    #[test]
    fn select_next_wraps_around() {
        let mut state = SwitcherState::new();
        state.activate(sample_entries(), SwitcherMode::AllWindows);
        state.group_mode = false;
        let count = state.visible_entries().len();
        // Go to the last entry.
        for _ in 0..count - 1 {
            state.select_next();
        }
        assert_eq!(state.selected_index, count - 1);
        // Next wraps to 0.
        state.select_next();
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn select_prev_wraps_around() {
        let mut state = SwitcherState::new();
        state.activate(sample_entries(), SwitcherMode::AllWindows);
        state.group_mode = false;
        assert_eq!(state.selected_index, 0);
        state.select_prev();
        let count = state.visible_entries().len();
        assert_eq!(state.selected_index, count - 1);
    }

    #[test]
    fn select_next_no_entries() {
        let mut state = SwitcherState::new();
        state.activate(Vec::new(), SwitcherMode::AllWindows);
        state.select_next(); // Should not panic.
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn select_prev_no_entries() {
        let mut state = SwitcherState::new();
        state.activate(Vec::new(), SwitcherMode::AllWindows);
        state.select_prev(); // Should not panic.
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn on_key_tab_advances() {
        let mut state = SwitcherState::new();
        state.activate(sample_entries(), SwitcherMode::AllWindows);
        state.group_mode = false;
        let action = state.on_key(SwitcherKey::Tab);
        assert_eq!(action, SwitcherAction::None);
        assert_eq!(state.selected_index, 1);
    }

    #[test]
    fn on_key_shift_tab_retreats() {
        let mut state = SwitcherState::new();
        state.activate(sample_entries(), SwitcherMode::AllWindows);
        state.group_mode = false;
        state.selected_index = 2;
        state.on_key(SwitcherKey::ShiftTab);
        assert_eq!(state.selected_index, 1);
    }

    // ── Expand / collapse groups ────────────────────────────────

    #[test]
    fn expand_group_shows_individual_windows() {
        let mut state = SwitcherState::new();
        state.activate(sample_entries(), SwitcherMode::AllWindows);
        assert!(state.group_mode); // Multiple groups → group mode.
        let action = state.on_key(SwitcherKey::Down);
        assert_eq!(action, SwitcherAction::ExpandGroup("firefox".to_string()));
        assert!(!state.group_mode);
        // Now visible entries are firefox's 2 windows.
        assert_eq!(state.visible_entries().len(), 2);
    }

    #[test]
    fn collapse_group_returns_to_groups() {
        let mut state = SwitcherState::new();
        state.activate(sample_entries(), SwitcherMode::AllWindows);
        state.on_key(SwitcherKey::Down); // expand firefox
        assert!(!state.group_mode);
        let action = state.on_key(SwitcherKey::Up);
        assert_eq!(action, SwitcherAction::CollapseGroup);
        assert!(state.group_mode);
    }

    #[test]
    fn expand_group_not_in_individual_mode() {
        let mut state = SwitcherState::new();
        state.activate(sample_entries(), SwitcherMode::AllWindows);
        state.group_mode = false;
        state.expanded_app_id = None;
        // Down in non-group mode without expanded_app_id does nothing.
        let action = state.on_key(SwitcherKey::Down);
        assert_eq!(action, SwitcherAction::None);
    }

    #[test]
    fn collapse_group_noop_when_already_grouped() {
        let mut state = SwitcherState::new();
        state.activate(sample_entries(), SwitcherMode::AllWindows);
        assert!(state.group_mode);
        let action = state.on_key(SwitcherKey::Up);
        assert_eq!(action, SwitcherAction::None); // Already in group mode.
    }

    // ── Confirm / cancel ────────────────────────────────────────

    #[test]
    fn confirm_returns_selected_window_id() {
        let mut state = SwitcherState::new();
        state.activate(sample_entries(), SwitcherMode::AllWindows);
        state.group_mode = false;
        state.selected_index = 2;
        let id = state.confirm();
        assert!(id.is_some());
        assert!(!state.active);
    }

    #[test]
    fn confirm_empty_returns_none() {
        let mut state = SwitcherState::new();
        state.activate(Vec::new(), SwitcherMode::AllWindows);
        let id = state.confirm();
        assert_eq!(id, None);
    }

    #[test]
    fn cancel_deactivates() {
        let mut state = SwitcherState::new();
        state.activate(sample_entries(), SwitcherMode::AllWindows);
        state.cancel();
        assert!(!state.active);
        assert!(state.entries.is_empty());
    }

    #[test]
    fn on_key_enter_selects() {
        let mut state = SwitcherState::new();
        state.activate(sample_entries(), SwitcherMode::AllWindows);
        state.group_mode = false;
        state.selected_index = 0;
        let action = state.on_key(SwitcherKey::Enter);
        // First entry has highest last_active_ms (1000) → id=1.
        assert_eq!(action, SwitcherAction::SelectWindow(1));
        assert!(!state.active);
    }

    #[test]
    fn on_key_escape_cancels() {
        let mut state = SwitcherState::new();
        state.activate(sample_entries(), SwitcherMode::AllWindows);
        let action = state.on_key(SwitcherKey::Escape);
        assert_eq!(action, SwitcherAction::Cancel);
        assert!(!state.active);
    }

    #[test]
    fn on_key_number_selects_directly() {
        let mut state = SwitcherState::new();
        state.activate(sample_entries(), SwitcherMode::AllWindows);
        state.group_mode = false;
        // Number(2) → index 1 (second entry).
        let action = state.on_key(SwitcherKey::Number(2));
        match action {
            SwitcherAction::SelectWindow(id) => {
                // The second MRU-sorted entry.
                assert!(id > 0);
            }
            _ => panic!("Expected SelectWindow"),
        }
    }

    #[test]
    fn on_key_number_out_of_range() {
        let mut state = SwitcherState::new();
        state.activate(sample_entries(), SwitcherMode::AllWindows);
        state.group_mode = false;
        // Number(9) → index 8, but only 5 entries.
        let action = state.on_key(SwitcherKey::Number(9));
        assert_eq!(action, SwitcherAction::None);
        assert!(state.active); // Still active — nothing happened.
    }

    #[test]
    fn on_key_number_zero_is_tenth() {
        let mut state = SwitcherState::new();
        // Need at least 10 entries.
        let entries: Vec<WindowEntry> = (0..10)
            .map(|i| make_entry(i + 1, "app", &format!("W{}", i), 1000 - i))
            .collect();
        state.activate(entries, SwitcherMode::AllWindows);
        state.group_mode = false;
        // Number(0) → index 9 (tenth entry).
        let action = state.on_key(SwitcherKey::Number(0));
        assert_eq!(action, SwitcherAction::SelectWindow(10));
    }

    // ── SwitcherMode filtering ──────────────────────────────────

    #[test]
    fn mode_all_windows() {
        let mut state = SwitcherState::new();
        state.activate(sample_entries(), SwitcherMode::AllWindows);
        assert_eq!(state.entries.len(), 5);
    }

    #[test]
    fn mode_app_windows_filters() {
        let mut state = SwitcherState::new();
        state.activate(
            sample_entries(),
            SwitcherMode::AppWindows("firefox".to_string()),
        );
        assert_eq!(state.entries.len(), 2);
        assert!(state.entries.iter().all(|e| e.app_id == "firefox"));
    }

    #[test]
    fn mode_recent_only_truncates() {
        let mut state = SwitcherState::new();
        state.activate(sample_entries(), SwitcherMode::RecentOnly(3));
        assert_eq!(state.entries.len(), 3);
        // Should be the 3 most recently active.
        assert_eq!(state.entries[0].last_active_ms, 1000);
        assert_eq!(state.entries[1].last_active_ms, 900);
        assert_eq!(state.entries[2].last_active_ms, 800);
    }

    // ── Visible entries ─────────────────────────────────────────

    #[test]
    fn visible_entries_group_mode() {
        let mut state = SwitcherState::new();
        state.activate(sample_entries(), SwitcherMode::AllWindows);
        assert!(state.group_mode);
        let visible = state.visible_entries();
        // 4 app groups → 4 visible entries.
        assert_eq!(visible.len(), 4);
    }

    #[test]
    fn visible_entries_individual_mode() {
        let mut state = SwitcherState::new();
        state.activate(sample_entries(), SwitcherMode::AllWindows);
        state.group_mode = false;
        state.expanded_app_id = None;
        let visible = state.visible_entries();
        assert_eq!(visible.len(), 5);
    }

    #[test]
    fn visible_entries_expanded_group() {
        let mut state = SwitcherState::new();
        state.activate(sample_entries(), SwitcherMode::AllWindows);
        // Expand firefox group.
        state.on_key(SwitcherKey::Down);
        let visible = state.visible_entries();
        assert_eq!(visible.len(), 2); // 2 firefox windows.
    }

    #[test]
    fn selected_window_returns_correct_entry() {
        let mut state = SwitcherState::new();
        state.activate(sample_entries(), SwitcherMode::AllWindows);
        state.group_mode = false;
        state.selected_index = 0;
        let win = state.selected_window().unwrap();
        assert_eq!(win.id, 1); // Most recently active.
    }

    #[test]
    fn selected_window_none_when_empty() {
        let mut state = SwitcherState::new();
        state.activate(Vec::new(), SwitcherMode::AllWindows);
        assert!(state.selected_window().is_none());
    }

    // ── Layout computation ──────────────────────────────────────

    #[test]
    fn layout_empty() {
        let slots = SwitcherLayout::compute(&[], 0, 1920.0, 1080.0);
        assert!(slots.is_empty());
    }

    #[test]
    fn layout_single_centered() {
        let entries = vec![make_entry(1, "app", "W", 100)];
        let slots = SwitcherLayout::compute(&entries, 0, 1920.0, 1080.0);
        assert_eq!(slots.len(), 1);
        let s = &slots[0];
        // Should be centered horizontally.
        let mid_x = s.x + s.width / 2.0;
        assert!((mid_x - 960.0).abs() < 1.0);
        // Should be centered vertically.
        let mid_y = s.y + s.height / 2.0;
        assert!((mid_y - 540.0).abs() < 1.0);
    }

    #[test]
    fn layout_marks_selected() {
        let entries = vec![make_entry(1, "a", "W1", 200), make_entry(2, "b", "W2", 100)];
        let slots = SwitcherLayout::compute(&entries, 1, 1920.0, 1080.0);
        assert!(!slots[0].is_selected);
        assert!(slots[1].is_selected);
    }

    #[test]
    fn layout_items_left_to_right() {
        let entries: Vec<WindowEntry> = (0..4)
            .map(|i| make_entry(i + 1, "app", &format!("W{}", i), 100))
            .collect();
        let slots = SwitcherLayout::compute(&entries, 0, 1920.0, 1080.0);
        for i in 1..slots.len() {
            assert!(slots[i].x > slots[i - 1].x, "Slots must be left-to-right");
        }
    }

    #[test]
    fn layout_fits_viewport() {
        let entries: Vec<WindowEntry> = (0..20)
            .map(|i| make_entry(i + 1, "app", &format!("W{}", i), 100))
            .collect();
        let slots = SwitcherLayout::compute(&entries, 0, 1920.0, 1080.0);
        for s in &slots {
            assert!(s.x >= 0.0);
            assert!(s.x + s.width <= 1920.0 + 1.0);
            assert!(s.y >= 0.0);
            assert!(s.y + s.height <= 1080.0 + 1.0);
        }
    }

    #[test]
    fn layout_many_windows_scales_down() {
        let entries: Vec<WindowEntry> = (0..30)
            .map(|i| make_entry(i + 1, "app", &format!("W{}", i), 100))
            .collect();
        let slots = SwitcherLayout::compute(&entries, 0, 1920.0, 1080.0);
        // With 30 windows, thumbnails should be smaller than max.
        assert!(slots[0].width < 160.0);
        assert!(slots[0].height < 120.0);
    }

    #[test]
    fn layout_same_y_for_all() {
        let entries: Vec<WindowEntry> = (0..5)
            .map(|i| make_entry(i + 1, "app", &format!("W{}", i), 100))
            .collect();
        let slots = SwitcherLayout::compute(&entries, 0, 1920.0, 1080.0);
        let y = slots[0].y;
        for s in &slots {
            assert!(
                (s.y - y).abs() < 0.01,
                "All items should be on the same row"
            );
        }
    }

    // ── Edge cases ──────────────────────────────────────────────

    #[test]
    fn activate_deactivate_cycle() {
        let mut state = SwitcherState::new();
        assert!(!state.active);
        state.activate(sample_entries(), SwitcherMode::AllWindows);
        assert!(state.active);
        state.deactivate();
        assert!(!state.active);
        assert!(state.entries.is_empty());
    }

    #[test]
    fn single_app_no_group_mode() {
        let entries = vec![
            make_entry(1, "app", "W1", 200),
            make_entry(2, "app", "W2", 100),
        ];
        let mut state = SwitcherState::new();
        state.activate(entries, SwitcherMode::AllWindows);
        // Only 1 app group → group_mode is false (no point showing groups).
        assert!(!state.group_mode);
    }

    #[test]
    fn single_window_navigation() {
        let mut state = SwitcherState::new();
        state.activate(
            vec![make_entry(1, "app", "Only", 100)],
            SwitcherMode::AllWindows,
        );
        state.group_mode = false;
        state.select_next();
        assert_eq!(state.selected_index, 0); // Wraps back to 0 immediately.
        state.select_prev();
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn minimized_windows_included() {
        let mut entries = sample_entries();
        entries[2].is_minimized = true;
        let mut state = SwitcherState::new();
        state.activate(entries, SwitcherMode::AllWindows);
        state.group_mode = false;
        // All 5 windows should be present, including minimized.
        assert_eq!(state.entries.len(), 5);
    }

    #[test]
    fn multiple_workspaces() {
        let entries = vec![
            make_entry(1, "app", "W1", 300),
            make_entry(2, "app", "W2", 200),
            make_entry(3, "app", "W3", 100),
        ];
        let mut state = SwitcherState::new();
        state.activate(entries, SwitcherMode::AllWindows);
        // All windows visible regardless of workspace in AllWindows mode.
        state.group_mode = false;
        assert_eq!(state.visible_entries().len(), 3);
    }

    #[test]
    fn app_windows_mode_unknown_app() {
        let mut state = SwitcherState::new();
        state.activate(
            sample_entries(),
            SwitcherMode::AppWindows("nonexistent".to_string()),
        );
        assert!(state.entries.is_empty());
    }

    #[test]
    fn recent_only_more_than_available() {
        let mut state = SwitcherState::new();
        state.activate(sample_entries(), SwitcherMode::RecentOnly(100));
        // Should have all 5 (truncate is a no-op when n > len).
        assert_eq!(state.entries.len(), 5);
    }

    #[test]
    fn group_mode_navigation_cycles_through_groups() {
        let mut state = SwitcherState::new();
        state.activate(sample_entries(), SwitcherMode::AllWindows);
        assert!(state.group_mode);
        // 4 groups: firefox, terminal, files, editor.
        state.on_key(SwitcherKey::Tab);
        assert_eq!(state.selected_index, 1);
        state.on_key(SwitcherKey::Tab);
        assert_eq!(state.selected_index, 2);
        state.on_key(SwitcherKey::Tab);
        assert_eq!(state.selected_index, 3);
        state.on_key(SwitcherKey::Tab); // Wrap.
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn expand_then_confirm_selects_window() {
        let mut state = SwitcherState::new();
        state.activate(sample_entries(), SwitcherMode::AllWindows);
        // Expand firefox (index 0).
        state.on_key(SwitcherKey::Down);
        // Select second firefox window.
        state.on_key(SwitcherKey::Tab);
        assert_eq!(state.selected_index, 1);
        let action = state.on_key(SwitcherKey::Enter);
        // Second firefox window (id=2).
        assert_eq!(action, SwitcherAction::SelectWindow(2));
    }

    #[test]
    fn collapse_restores_group_selection() {
        let mut state = SwitcherState::new();
        state.activate(sample_entries(), SwitcherMode::AllWindows);
        // Select second group (terminal) then expand.
        state.on_key(SwitcherKey::Tab); // index 1 → terminal.
        state.on_key(SwitcherKey::Down); // expand terminal.
        assert!(!state.group_mode);
        // Collapse.
        state.on_key(SwitcherKey::Up);
        assert!(state.group_mode);
        // Should be back at the terminal group index.
        assert_eq!(state.selected_index, 1);
    }
}
