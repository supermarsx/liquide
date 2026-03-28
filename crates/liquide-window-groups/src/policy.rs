/// Policy for automatic window grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoGroupPolicy {
    /// Windows with the same `app_id` are automatically grouped.
    ByApplication,
    /// Windows on the same workspace are automatically grouped.
    ByWorkspace,
    /// No automatic grouping; groups must be created manually.
    Manual,
}

impl Default for AutoGroupPolicy {
    fn default() -> Self {
        Self::Manual
    }
}

/// Policy for what happens when one window in a group is minimized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupMinimizePolicy {
    /// Only the targeted window is minimized.
    Individual,
    /// All windows in the group are minimized together.
    All,
}

impl Default for GroupMinimizePolicy {
    fn default() -> Self {
        Self::Individual
    }
}
