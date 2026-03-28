/// Unique identifier for a window group.
pub type GroupId = u64;

/// Unique identifier for a window.
pub type WindowId = u64;

/// A group of related windows (e.g., all windows from the same application).
#[derive(Debug, Clone)]
pub struct WindowGroup {
    /// Unique identifier for this group.
    pub group_id: GroupId,
    /// Human-readable label for the group.
    pub label: String,
    /// Ordered list of window IDs belonging to this group.
    pub windows: Vec<WindowId>,
    /// Optional color tag for visual distinction (RGBA hex, e.g. "#FF5733FF").
    pub color_tag: Option<String>,
    /// Optional icon name or path for the group.
    pub icon: Option<String>,
    /// Application identifier for auto-grouping.
    pub app_id: Option<String>,
}

impl WindowGroup {
    /// Create a new window group with the given id and label.
    pub fn new(group_id: GroupId, label: String) -> Self {
        Self {
            group_id,
            label,
            windows: Vec::new(),
            color_tag: None,
            icon: None,
            app_id: None,
        }
    }

    /// Returns true if this group contains the given window.
    pub fn contains(&self, window_id: WindowId) -> bool {
        self.windows.contains(&window_id)
    }

    /// Add a window to this group. Returns false if already present.
    pub fn add_window(&mut self, window_id: WindowId) -> bool {
        if self.windows.contains(&window_id) {
            return false;
        }
        self.windows.push(window_id);
        true
    }

    /// Remove a window from this group. Returns false if not found.
    pub fn remove_window(&mut self, window_id: WindowId) -> bool {
        let len_before = self.windows.len();
        self.windows.retain(|&w| w != window_id);
        self.windows.len() < len_before
    }

    /// Returns the number of windows in this group.
    pub fn len(&self) -> usize {
        self.windows.len()
    }

    /// Returns true if this group has no windows.
    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }
}
