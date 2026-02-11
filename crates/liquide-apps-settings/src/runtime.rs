//! Top-level settings coordinator.

use std::collections::HashMap;

use crate::apply::{ChangeTracker, SettingChange};
use crate::category::{Category, CategoryInfo};
use crate::config::SettingsConfig;
use crate::entry::{SettingEntry, SettingValue};
use crate::notify::NotificationQueue;
use crate::page::{self, SettingsPage};
use crate::policy::PolicyEngine;
use crate::search::SettingsSearch;

/// The settings runtime that coordinates all subsystems.
pub struct SettingsRuntime {
    config: SettingsConfig,
    pages: Vec<SettingsPage>,
    entries: HashMap<String, SettingEntry>,
    active_category: Category,
    search: SettingsSearch,
    changes: ChangeTracker,
    policy: PolicyEngine,
    notifications: NotificationQueue,
}

impl SettingsRuntime {
    /// Create a new settings runtime with default entries.
    #[must_use]
    pub fn new(config: SettingsConfig) -> Self {
        let default_cat = Category::from_id(&config.default_category)
            .unwrap_or(Category::Display);
        let history_limit = config.search_history_limit;

        let (pages, entry_list) = page::default_pages();
        let mut entries = HashMap::new();
        for entry in entry_list {
            entries.insert(entry.key.clone(), entry);
        }

        Self {
            config,
            pages,
            entries,
            active_category: default_cat,
            search: SettingsSearch::new(history_limit),
            changes: ChangeTracker::new(),
            policy: PolicyEngine::new(),
            notifications: NotificationQueue::new(),
        }
    }

    // ---- Navigation ----

    /// Get the active category.
    #[must_use]
    pub fn active_category(&self) -> Category { self.active_category }

    /// Switch to a different category.
    pub fn set_category(&mut self, cat: Category) {
        self.active_category = cat;
    }

    /// Get the page for a specific category.
    #[must_use]
    pub fn page(&self, cat: Category) -> Option<&SettingsPage> {
        self.pages.iter().find(|p| p.category == cat)
    }

    /// Get info for all categories.
    #[must_use]
    pub fn category_infos(&self) -> Vec<CategoryInfo> {
        Category::ALL.iter().map(|&cat| {
            let entry_count = self.entries.values()
                .filter(|e| e.category == cat)
                .count();
            let has_pending = self.changes.pending().iter()
                .any(|c| self.entries.get(&c.key).is_some_and(|e| e.category == cat));
            CategoryInfo { category: cat, entry_count, has_pending_changes: has_pending }
        }).collect()
    }

    // ---- Entries ----

    /// Get a setting entry by key.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&SettingEntry> {
        self.entries.get(key)
    }

    /// Get the current value of a setting.
    #[must_use]
    pub fn value(&self, key: &str) -> Option<&SettingValue> {
        self.entries.get(key).map(|e| &e.value)
    }

    /// Get all entries for a category.
    #[must_use]
    pub fn entries_for(&self, cat: Category) -> Vec<&SettingEntry> {
        self.entries.values()
            .filter(|e| e.category == cat)
            .collect()
    }

    /// Get all entries for the active category, respecting policy visibility.
    #[must_use]
    pub fn visible_entries(&self) -> Vec<&SettingEntry> {
        self.entries.values()
            .filter(|e| e.category == self.active_category)
            .filter(|e| self.policy.is_visible(&e.key))
            .filter(|e| self.config.show_advanced || !e.advanced)
            .collect()
    }

    /// Total number of settings.
    #[must_use]
    pub fn total_entries(&self) -> usize { self.entries.len() }

    // ---- Value changes ----

    /// Set a setting value, recording the change for undo.
    pub fn set_value(&mut self, key: &str, value: SettingValue) -> crate::Result<()> {
        if !self.policy.is_editable(key) {
            return Err(crate::SettingsError::LockedByPolicy { key: key.into() });
        }

        let entry = self.entries.get(key)
            .ok_or_else(|| crate::SettingsError::UnknownSetting { key: key.into() })?;

        entry.validate(&value)?;

        let old_value = entry.value.clone();
        let change = SettingChange {
            key: key.into(),
            old_value,
            new_value: value.clone(),
        };
        self.changes.record(change);
        self.changes.apply();

        // Apply value immediately to the entry.
        if let Some(entry) = self.entries.get_mut(key) {
            entry.value = value.clone();
        }

        self.notifications.push(key, value, 0);

        Ok(())
    }

    /// Reset a setting to its default value.
    pub fn reset_to_default(&mut self, key: &str) -> crate::Result<()> {
        let default = self.entries.get(key)
            .ok_or_else(|| crate::SettingsError::UnknownSetting { key: key.into() })?
            .default.clone();
        self.set_value(key, default)
    }

    // ---- Undo/Redo ----

    /// Undo the last change.
    pub fn undo(&mut self) -> crate::Result<()> {
        let reversed = self.changes.undo()?;
        if let Some(entry) = self.entries.get_mut(&reversed.key) {
            entry.value = reversed.new_value.clone();
        }
        self.notifications.push(&reversed.key, reversed.new_value, 0);
        Ok(())
    }

    /// Redo the last undone change.
    pub fn redo(&mut self) -> crate::Result<()> {
        let reapplied = self.changes.redo()?;
        if let Some(entry) = self.entries.get_mut(&reapplied.key) {
            entry.value = reapplied.new_value.clone();
        }
        self.notifications.push(&reapplied.key, reapplied.new_value, 0);
        Ok(())
    }

    /// Whether undo is available.
    #[must_use]
    pub fn can_undo(&self) -> bool { self.changes.can_undo() }
    /// Whether redo is available.
    #[must_use]
    pub fn can_redo(&self) -> bool { self.changes.can_redo() }

    // ---- Search ----

    /// Search settings.
    pub fn search(&mut self, query: &str) {
        let entries: Vec<_> = self.entries.values().cloned().collect();
        self.search.search(query, &entries);
    }

    /// Get search results.
    #[must_use]
    pub fn search_results(&self) -> &[crate::search::SearchResult] {
        self.search.results()
    }

    /// Clear the search.
    pub fn clear_search(&mut self) {
        self.search.clear();
    }

    // ---- Policy ----

    /// Get the policy engine.
    #[must_use]
    pub fn policy(&self) -> &PolicyEngine { &self.policy }

    /// Get mutable access to the policy engine.
    pub fn policy_mut(&mut self) -> &mut PolicyEngine { &mut self.policy }

    // ---- Notifications ----

    /// Drain queued notifications.
    pub fn drain_notifications(&mut self) -> Vec<crate::notify::SettingNotification> {
        self.notifications.drain()
    }

    // ---- Config ----

    /// Get the current config.
    #[must_use]
    pub fn config(&self) -> &SettingsConfig { &self.config }
}
