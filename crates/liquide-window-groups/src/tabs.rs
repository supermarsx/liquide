use crate::group::WindowId;

/// Unique identifier for a tab group.
pub type TabGroupId = u64;

/// Windows merged into a tabbed interface.
#[derive(Debug, Clone)]
pub struct TabGroup {
    /// Unique identifier for this tab group.
    pub group_id: TabGroupId,
    /// Ordered list of window IDs representing tabs.
    pub tabs: Vec<WindowId>,
    /// Index of the currently active (visible) tab.
    pub active_tab: usize,
    /// Height of the tab bar in pixels.
    pub tab_bar_height: f32,
}

impl TabGroup {
    /// Create a new tab group with the given id and initial tabs.
    pub fn new(group_id: TabGroupId, tabs: Vec<WindowId>, tab_bar_height: f32) -> Self {
        Self {
            group_id,
            tabs,
            active_tab: 0,
            tab_bar_height,
        }
    }

    /// Returns the currently active window, if any.
    pub fn active_window(&self) -> Option<WindowId> {
        self.tabs.get(self.active_tab).copied()
    }

    /// Returns the number of tabs.
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Returns true if this tab group contains the given window.
    pub fn contains(&self, window_id: WindowId) -> bool {
        self.tabs.contains(&window_id)
    }

    /// Set the active tab index, clamping to valid range.
    /// Returns false if the group is empty.
    pub fn set_active(&mut self, index: usize) -> bool {
        if self.tabs.is_empty() {
            return false;
        }
        self.active_tab = index.min(self.tabs.len() - 1);
        true
    }

    /// Reorder a tab from `from_index` to `to_index`.
    /// Returns false if either index is out of bounds.
    pub fn reorder(&mut self, from_index: usize, to_index: usize) -> bool {
        if from_index >= self.tabs.len() || to_index >= self.tabs.len() {
            return false;
        }
        if from_index == to_index {
            return true;
        }
        let window_id = self.tabs.remove(from_index);
        self.tabs.insert(to_index, window_id);

        // Update active_tab to follow the moved tab if it was the active one,
        // or adjust for the shift.
        if self.active_tab == from_index {
            self.active_tab = to_index;
        } else if from_index < self.active_tab && to_index >= self.active_tab {
            self.active_tab -= 1;
        } else if from_index > self.active_tab && to_index <= self.active_tab {
            self.active_tab += 1;
        }
        true
    }

    /// Remove a tab by window ID. Returns false if not found.
    /// Adjusts active_tab if needed.
    pub fn remove_tab(&mut self, window_id: WindowId) -> bool {
        let Some(idx) = self.tabs.iter().position(|&w| w == window_id) else {
            return false;
        };
        self.tabs.remove(idx);
        if self.tabs.is_empty() {
            self.active_tab = 0;
        } else if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len() - 1;
        } else if idx < self.active_tab {
            self.active_tab -= 1;
        }
        true
    }

    /// Add a tab at the end. Returns false if already present.
    pub fn add_tab(&mut self, window_id: WindowId) -> bool {
        if self.tabs.contains(&window_id) {
            return false;
        }
        self.tabs.push(window_id);
        true
    }
}

/// Minimum tab width in pixels before scrolling kicks in.
pub const MIN_TAB_WIDTH: f32 = 80.0;

/// Width of the close button hit area on each tab.
pub const CLOSE_BUTTON_SIZE: f32 = 16.0;

/// Padding around the close button within a tab.
pub const CLOSE_BUTTON_PADDING: f32 = 4.0;

/// Computed layout for a single tab in the tab bar.
#[derive(Debug, Clone, Copy)]
pub struct TabRect {
    /// X position of this tab (relative to tab bar left edge).
    pub x: f32,
    /// Width of this tab.
    pub width: f32,
    /// Index into the tab group's tabs vec.
    pub index: usize,
}

impl TabRect {
    /// Returns the x-position of the close button center.
    pub fn close_button_x(&self) -> f32 {
        self.x + self.width - CLOSE_BUTTON_PADDING - CLOSE_BUTTON_SIZE / 2.0
    }

    /// Returns true if the given (x, y) position hits the close button.
    /// `y` is relative to the tab bar top; `bar_height` is the tab bar height.
    pub fn hit_test_close(&self, x: f32, y: f32, bar_height: f32) -> bool {
        let cx = self.close_button_x();
        let cy = bar_height / 2.0;
        let half = CLOSE_BUTTON_SIZE / 2.0 + CLOSE_BUTTON_PADDING;
        x >= cx - half && x <= cx + half && y >= cy - half && y <= cy + half
    }

    /// Returns true if the given x position is within this tab's bounds.
    pub fn hit_test(&self, x: f32) -> bool {
        x >= self.x && x < self.x + self.width
    }
}

/// Computes tab positions given a tab group and available width.
#[derive(Debug, Clone)]
pub struct TabBarLayout {
    /// Computed tab rectangles.
    pub tabs: Vec<TabRect>,
    /// Total content width (may exceed available width if scrolling).
    pub total_width: f32,
    /// Available width of the tab bar.
    pub available_width: f32,
    /// Current scroll offset (0 if no scrolling needed).
    pub scroll_offset: f32,
    /// Whether the tabs require scrolling.
    pub needs_scroll: bool,
}

impl TabBarLayout {
    /// Compute the tab bar layout for the given tab group and available width.
    pub fn compute(tab_group: &TabGroup, available_width: f32) -> Self {
        let count = tab_group.tabs.len();
        if count == 0 {
            return Self {
                tabs: Vec::new(),
                total_width: 0.0,
                available_width,
                scroll_offset: 0.0,
                needs_scroll: false,
            };
        }

        let equal_width = available_width / count as f32;
        let (tab_width, needs_scroll) = if equal_width >= MIN_TAB_WIDTH {
            (equal_width, false)
        } else {
            (MIN_TAB_WIDTH, true)
        };

        let total_width = tab_width * count as f32;

        let tabs = (0..count)
            .map(|i| TabRect {
                x: i as f32 * tab_width,
                width: tab_width,
                index: i,
            })
            .collect();

        Self {
            tabs,
            total_width,
            available_width,
            scroll_offset: 0.0,
            needs_scroll,
        }
    }

    /// Find which tab (if any) is at the given x coordinate,
    /// accounting for scroll offset.
    pub fn tab_at_x(&self, x: f32) -> Option<usize> {
        let scrolled_x = x + self.scroll_offset;
        for tab in &self.tabs {
            if tab.hit_test(scrolled_x) {
                return Some(tab.index);
            }
        }
        None
    }

    /// Scroll the tab bar by the given delta. Clamps to valid range.
    pub fn scroll_by(&mut self, delta: f32) {
        if !self.needs_scroll {
            return;
        }
        let max_scroll = (self.total_width - self.available_width).max(0.0);
        self.scroll_offset = (self.scroll_offset + delta).clamp(0.0, max_scroll);
    }

    /// Ensure the tab at the given index is visible by scrolling if needed.
    pub fn ensure_visible(&mut self, index: usize) {
        if !self.needs_scroll || index >= self.tabs.len() {
            return;
        }
        let tab = &self.tabs[index];
        if tab.x < self.scroll_offset {
            self.scroll_offset = tab.x;
        } else if tab.x + tab.width > self.scroll_offset + self.available_width {
            self.scroll_offset = tab.x + tab.width - self.available_width;
        }
    }
}

/// Threshold in pixels: dragging a tab beyond this distance from the
/// tab bar triggers a detach.
pub const DETACH_THRESHOLD: f32 = 40.0;

/// State for an in-progress tab drag operation.
#[derive(Debug, Clone)]
pub struct TabDragState {
    /// The tab group being dragged within.
    pub tab_group_id: TabGroupId,
    /// The window ID of the tab being dragged.
    pub window_id: WindowId,
    /// The original index of the dragged tab.
    pub original_index: usize,
    /// Current drag x position relative to the tab bar.
    pub drag_x: f32,
    /// Current drag y position relative to the tab bar.
    pub drag_y: f32,
    /// The x offset from the tab's left edge where the drag started.
    pub grab_offset_x: f32,
    /// Whether the drag has moved beyond the detach threshold.
    pub should_detach: bool,
}

impl TabDragState {
    /// Create a new drag state.
    pub fn new(
        tab_group_id: TabGroupId,
        window_id: WindowId,
        original_index: usize,
        start_x: f32,
        grab_offset_x: f32,
    ) -> Self {
        Self {
            tab_group_id,
            window_id,
            original_index,
            drag_x: start_x,
            drag_y: 0.0,
            grab_offset_x,
            should_detach: false,
        }
    }

    /// Update the drag position. Returns the target index for reorder
    /// based on current drag position, and updates `should_detach`.
    pub fn update(&mut self, x: f32, y: f32, layout: &TabBarLayout, bar_height: f32) -> usize {
        self.drag_x = x;
        self.drag_y = y;

        // Check detach: if dragged far enough vertically from the tab bar
        self.should_detach = y < -DETACH_THRESHOLD || y > bar_height + DETACH_THRESHOLD;

        // Compute the target index based on the center of where the tab
        // would be if placed at the current drag x.
        let center_x = x - self.grab_offset_x + (layout.tabs.first().map_or(MIN_TAB_WIDTH, |t| t.width) / 2.0);
        let scrolled_center = center_x + layout.scroll_offset;

        layout
            .tab_at_x(scrolled_center)
            .unwrap_or_else(|| {
                if scrolled_center <= 0.0 {
                    0
                } else {
                    layout.tabs.len().saturating_sub(1)
                }
            })
    }
}
