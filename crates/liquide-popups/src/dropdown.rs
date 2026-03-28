//! Specialized dropdown handling with keyboard navigation and scroll.

/// A single item in a dropdown list.
#[derive(Debug, Clone)]
pub struct DropdownItem {
    /// Unique identifier for this item.
    pub id: u32,
    /// Display label.
    pub label: String,
    /// Optional icon name.
    pub icon: Option<String>,
    /// Whether the item is interactable.
    pub enabled: bool,
    /// Whether the item is currently selected (checked).
    pub selected: bool,
}

impl DropdownItem {
    /// Create a simple enabled, unselected item.
    #[must_use]
    pub fn new(id: u32, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            icon: None,
            enabled: true,
            selected: false,
        }
    }

    /// Builder: set icon.
    #[must_use]
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Builder: set disabled.
    #[must_use]
    pub fn with_disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Builder: set selected.
    #[must_use]
    pub fn with_selected(mut self) -> Self {
        self.selected = true;
        self
    }
}

/// Keyboard actions for dropdown navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropdownKey {
    /// Move highlight up.
    Up,
    /// Move highlight down.
    Down,
    /// Confirm selection.
    Enter,
    /// Cancel / close.
    Escape,
    /// Jump to first item.
    Home,
    /// Jump to last item.
    End,
    /// Page up (move by max_visible).
    PageUp,
    /// Page down (move by max_visible).
    PageDown,
}

/// Manages dropdown state: items, highlight, scroll offset, selection.
pub struct DropdownController {
    items: Vec<DropdownItem>,
    /// Currently highlighted item index (may differ from selected).
    highlight_index: Option<usize>,
    /// Index of the first visible item when scrolled.
    scroll_offset: usize,
    /// Maximum number of items visible at once.
    max_visible: usize,
    /// The item that was confirmed via Enter or click.
    confirmed_item: Option<u32>,
    /// Whether the dropdown is currently open.
    open: bool,
}

impl DropdownController {
    /// Create a new dropdown controller.
    #[must_use]
    pub fn new(max_visible: usize) -> Self {
        Self {
            items: Vec::new(),
            highlight_index: None,
            scroll_offset: 0,
            max_visible: max_visible.max(1),
            confirmed_item: None,
            open: false,
        }
    }

    /// Open the dropdown with the given items. Returns the initial highlight
    /// (the first selected item, or the first enabled item).
    pub fn open_dropdown(&mut self, items: Vec<DropdownItem>) {
        self.confirmed_item = None;
        self.open = true;
        self.scroll_offset = 0;

        // Find the first selected item to highlight.
        let initial_highlight = items
            .iter()
            .position(|it| it.selected && it.enabled)
            .or_else(|| items.iter().position(|it| it.enabled));

        self.highlight_index = initial_highlight;

        // Scroll to make the highlighted item visible.
        if let Some(idx) = initial_highlight {
            if idx >= self.max_visible {
                self.scroll_offset = idx - self.max_visible + 1;
            }
        }

        self.items = items;
    }

    /// Close the dropdown.
    pub fn close(&mut self) {
        self.open = false;
        self.highlight_index = None;
        self.confirmed_item = None;
    }

    /// Whether the dropdown is open.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// All items.
    #[must_use]
    pub fn items(&self) -> &[DropdownItem] {
        &self.items
    }

    /// The visible slice of items (accounting for scroll offset and max_visible).
    #[must_use]
    pub fn visible_items(&self) -> &[DropdownItem] {
        let end = (self.scroll_offset + self.max_visible).min(self.items.len());
        &self.items[self.scroll_offset..end]
    }

    /// Current highlight index (global, not relative to scroll).
    #[must_use]
    pub fn highlight_index(&self) -> Option<usize> {
        self.highlight_index
    }

    /// Scroll offset.
    #[must_use]
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Whether scrolling up is possible.
    #[must_use]
    pub fn can_scroll_up(&self) -> bool {
        self.scroll_offset > 0
    }

    /// Whether scrolling down is possible.
    #[must_use]
    pub fn can_scroll_down(&self) -> bool {
        self.scroll_offset + self.max_visible < self.items.len()
    }

    /// The confirmed (selected via Enter/click) item ID, if any.
    /// Consuming: returns the value and clears it.
    #[must_use]
    pub fn take_confirmed(&mut self) -> Option<u32> {
        self.confirmed_item.take()
    }

    /// The currently selected item ID (the most recent confirmed selection,
    /// or the item marked `selected` in the items list).
    #[must_use]
    pub fn selected_item(&self) -> Option<u32> {
        if let Some(id) = self.confirmed_item {
            return Some(id);
        }
        self.items.iter().find(|it| it.selected).map(|it| it.id)
    }

    /// Handle a keyboard event. Returns `true` if the event was consumed.
    pub fn keyboard_select(&mut self, key: DropdownKey) -> bool {
        if !self.open || self.items.is_empty() {
            return false;
        }

        match key {
            DropdownKey::Up => {
                self.move_highlight(-1);
                true
            }
            DropdownKey::Down => {
                self.move_highlight(1);
                true
            }
            DropdownKey::Home => {
                self.highlight_first_enabled();
                true
            }
            DropdownKey::End => {
                self.highlight_last_enabled();
                true
            }
            DropdownKey::PageUp => {
                self.move_highlight(-(self.max_visible as i32));
                true
            }
            DropdownKey::PageDown => {
                self.move_highlight(self.max_visible as i32);
                true
            }
            DropdownKey::Enter => {
                self.confirm_highlighted();
                true
            }
            DropdownKey::Escape => {
                self.close();
                true
            }
        }
    }

    /// Select an item by clicking on it (given a visible-relative index).
    /// Returns the item ID if the click was on an enabled item.
    pub fn click_item(&mut self, visible_index: usize) -> Option<u32> {
        let global_index = self.scroll_offset + visible_index;
        if let Some(item) = self.items.get(global_index) {
            if item.enabled {
                self.highlight_index = Some(global_index);
                self.confirmed_item = Some(item.id);
                return Some(item.id);
            }
        }
        None
    }

    /// Set highlight by mouse hover on a visible-relative index.
    pub fn hover_item(&mut self, visible_index: usize) {
        let global_index = self.scroll_offset + visible_index;
        if global_index < self.items.len() && self.items[global_index].enabled {
            self.highlight_index = Some(global_index);
        }
    }

    // ----- internal helpers -----

    /// Move highlight by `delta` steps, skipping disabled items.
    fn move_highlight(&mut self, delta: i32) {
        let len = self.items.len();
        if len == 0 {
            return;
        }

        let start = self.highlight_index.unwrap_or(0) as i32;
        let mut candidate = start + delta;

        // Clamp to valid range.
        candidate = candidate.clamp(0, len as i32 - 1);

        // Walk in the direction of delta to find an enabled item.
        let step = if delta >= 0 { 1i32 } else { -1i32 };
        let mut idx = candidate;
        loop {
            if idx < 0 || idx >= len as i32 {
                // Couldn't find an enabled item — stay where we are.
                return;
            }
            if self.items[idx as usize].enabled {
                break;
            }
            idx += step;
        }

        self.highlight_index = Some(idx as usize);
        self.ensure_highlight_visible();
    }

    /// Highlight the first enabled item.
    fn highlight_first_enabled(&mut self) {
        if let Some(idx) = self.items.iter().position(|it| it.enabled) {
            self.highlight_index = Some(idx);
            self.ensure_highlight_visible();
        }
    }

    /// Highlight the last enabled item.
    fn highlight_last_enabled(&mut self) {
        if let Some(idx) = self.items.iter().rposition(|it| it.enabled) {
            self.highlight_index = Some(idx);
            self.ensure_highlight_visible();
        }
    }

    /// Confirm the currently highlighted item.
    fn confirm_highlighted(&mut self) {
        if let Some(idx) = self.highlight_index {
            if let Some(item) = self.items.get(idx) {
                if item.enabled {
                    self.confirmed_item = Some(item.id);
                }
            }
        }
    }

    /// Scroll so the highlighted item is within the visible window.
    fn ensure_highlight_visible(&mut self) {
        if let Some(idx) = self.highlight_index {
            if idx < self.scroll_offset {
                self.scroll_offset = idx;
            } else if idx >= self.scroll_offset + self.max_visible {
                self.scroll_offset = idx - self.max_visible + 1;
            }
        }
    }
}

impl Default for DropdownController {
    fn default() -> Self {
        Self::new(10)
    }
}
