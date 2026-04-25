//! Policy constraints on modifiable settings.

use std::collections::HashMap;

/// Constraint applied to a setting by policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyConstraint {
    /// Setting is fully locked and cannot be changed.
    Locked,
    /// Setting is hidden from the UI.
    Hidden,
    /// Setting is read-only (visible but not editable).
    ReadOnly,
}

/// Policy engine that determines which settings the user may modify.
pub struct PolicyEngine {
    constraints: HashMap<String, PolicyConstraint>,
}

impl PolicyEngine {
    #[must_use]
    pub fn new() -> Self {
        Self {
            constraints: HashMap::new(),
        }
    }

    /// Add a constraint for a setting key.
    pub fn set_constraint(&mut self, key: impl Into<String>, constraint: PolicyConstraint) {
        self.constraints.insert(key.into(), constraint);
    }

    /// Remove a constraint for a setting key.
    pub fn remove_constraint(&mut self, key: &str) {
        self.constraints.remove(key);
    }

    /// Get the constraint for a setting, if any.
    #[must_use]
    pub fn constraint(&self, key: &str) -> Option<&PolicyConstraint> {
        self.constraints.get(key)
    }

    /// Whether the setting is editable (not locked, hidden, or read-only).
    #[must_use]
    pub fn is_editable(&self, key: &str) -> bool {
        self.constraints.get(key).is_none()
    }

    /// Whether the setting is visible (not hidden).
    #[must_use]
    pub fn is_visible(&self, key: &str) -> bool {
        self.constraints.get(key) != Some(&PolicyConstraint::Hidden)
    }

    /// Get all constrained setting keys.
    #[must_use]
    pub fn constrained_keys(&self) -> Vec<&str> {
        self.constraints.keys().map(String::as_str).collect()
    }

    /// Number of active constraints.
    #[must_use]
    pub fn constraint_count(&self) -> usize {
        self.constraints.len()
    }

    /// Clear all constraints.
    pub fn clear(&mut self) {
        self.constraints.clear();
    }
}

impl Default for PolicyEngine {
    fn default() -> Self {
        Self::new()
    }
}
