//! Change tracking, validation, and persistence.

use crate::entry::SettingValue;

/// A single change to a setting.
#[derive(Debug, Clone)]
pub struct SettingChange {
    /// The setting key.
    pub key: String,
    /// The old value before the change.
    pub old_value: SettingValue,
    /// The new value after the change.
    pub new_value: SettingValue,
}

/// Tracks pending changes with undo/redo support.
pub struct ChangeTracker {
    /// Pending changes not yet applied.
    pending: Vec<SettingChange>,
    /// Undo stack.
    undo_stack: Vec<SettingChange>,
    /// Redo stack.
    redo_stack: Vec<SettingChange>,
}

impl ChangeTracker {
    #[must_use]
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// Record a change. Clears the redo stack.
    pub fn record(&mut self, change: SettingChange) {
        // If there's already a pending change for this key, update it.
        if let Some(existing) = self.pending.iter_mut().find(|c| c.key == change.key) {
            existing.new_value = change.new_value;
        } else {
            self.pending.push(change);
        }
        self.redo_stack.clear();
    }

    /// Apply all pending changes, moving them to the undo stack.
    /// Returns the list of applied changes.
    pub fn apply(&mut self) -> Vec<SettingChange> {
        let applied = std::mem::take(&mut self.pending);
        self.undo_stack.extend(applied.clone());
        applied
    }

    /// Undo the last change.
    pub fn undo(&mut self) -> crate::Result<SettingChange> {
        let change = self.undo_stack.pop()
            .ok_or(crate::SettingsError::NothingToUndo)?;
        let reversed = SettingChange {
            key: change.key.clone(),
            old_value: change.new_value.clone(),
            new_value: change.old_value.clone(),
        };
        self.redo_stack.push(change);
        Ok(reversed)
    }

    /// Redo the last undone change.
    pub fn redo(&mut self) -> crate::Result<SettingChange> {
        let change = self.redo_stack.pop()
            .ok_or(crate::SettingsError::NothingToRedo)?;
        let reapplied = SettingChange {
            key: change.key.clone(),
            old_value: change.old_value.clone(),
            new_value: change.new_value.clone(),
        };
        self.undo_stack.push(change);
        Ok(reapplied)
    }

    /// Whether there are pending changes.
    #[must_use]
    pub fn has_pending(&self) -> bool { !self.pending.is_empty() }

    /// Number of pending changes.
    #[must_use]
    pub fn pending_count(&self) -> usize { self.pending.len() }

    /// Get pending changes.
    #[must_use]
    pub fn pending(&self) -> &[SettingChange] { &self.pending }

    /// Whether undo is available.
    #[must_use]
    pub fn can_undo(&self) -> bool { !self.undo_stack.is_empty() }

    /// Whether redo is available.
    #[must_use]
    pub fn can_redo(&self) -> bool { !self.redo_stack.is_empty() }

    /// Discard all pending changes.
    pub fn discard(&mut self) {
        self.pending.clear();
    }

    /// Clear everything.
    pub fn clear(&mut self) {
        self.pending.clear();
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}

impl Default for ChangeTracker {
    fn default() -> Self { Self::new() }
}
