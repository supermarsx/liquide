//! Top-level settings coordinator.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::apply::{ChangeTracker, SettingChange};
use crate::category::{Category, CategoryInfo};
use crate::config::SettingsConfig;
use crate::entry::{SettingEntry, SettingValue};
use crate::notify::NotificationQueue;
use crate::page::{self, SettingsPage};
use crate::policy::PolicyEngine;
use crate::search::SettingsSearch;

/// Cross-platform directory for LiquiDE configuration files.
pub fn settings_dir() -> PathBuf {
    #[cfg(target_os = "linux")]
    {
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            return PathBuf::from(xdg).join("liquide");
        }
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(".config/liquide");
        }
        return "/tmp/liquide".into();
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata).join("liquide");
        }
        return "C:\\ProgramData\\liquide".into();
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join("Library/Application Support/liquide");
        }
        return "/tmp/liquide".into();
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        "/tmp/liquide".into()
    }
}

/// Path to the settings JSON file.
pub fn settings_file() -> PathBuf {
    settings_dir().join("settings.json")
}

/// A display-ready representation of a setting for UI rendering.
#[derive(Debug, Clone)]
pub struct SettingDisplay {
    /// Unique key.
    pub key: String,
    /// Human-readable label.
    pub label: String,
    /// Description / tooltip.
    pub description: String,
    /// Current value.
    pub value: SettingValue,
    /// Whether the setting is locked by policy.
    pub locked: bool,
    /// Category this setting belongs to.
    pub category: Category,
}

/// Convert a `serde_json::Value` into a `SettingValue`.
fn json_to_setting_value(v: &serde_json::Value) -> SettingValue {
    match v {
        serde_json::Value::Bool(b) => SettingValue::Bool(*b),
        serde_json::Value::Number(n) => {
            SettingValue::Number(n.as_f64().unwrap_or(0.0))
        }
        serde_json::Value::String(s) => SettingValue::Text(s.clone()),
        other => SettingValue::Text(other.to_string()),
    }
}

/// Convert a `SettingValue` into a `serde_json::Value`.
fn setting_value_to_json(v: &SettingValue) -> serde_json::Value {
    match v {
        SettingValue::Bool(b) => serde_json::Value::Bool(*b),
        SettingValue::Number(n) => {
            serde_json::Number::from_f64(*n)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null)
        }
        SettingValue::Text(s) => serde_json::Value::String(s.clone()),
    }
}

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

    // ---- Persistence ----

    /// Load settings from the JSON file on disk.
    ///
    /// Only values whose keys match a registered entry are applied; unknown
    /// keys are silently ignored. If the file does not exist yet, this is a
    /// no-op and returns `Ok(())`.
    pub fn load_from_disk(&mut self) -> crate::Result<()> {
        let path = settings_file();
        if !path.exists() {
            tracing::info!("no settings file found, using defaults");
            return Ok(());
        }
        let content = std::fs::read_to_string(&path)?;
        let map: HashMap<String, serde_json::Value> = serde_json::from_str(&content)?;

        for (key, json_val) in &map {
            if let Some(entry) = self.entries.get_mut(key.as_str()) {
                let value = json_to_setting_value(json_val);
                if entry.validate(&value).is_ok() {
                    entry.value = value;
                } else {
                    tracing::warn!(key = %key, "skipping invalid persisted value");
                }
            }
        }
        tracing::info!(path = %path.display(), entries = self.entries.len(), "settings loaded from disk");
        Ok(())
    }

    /// Save all current setting values to disk as pretty-printed JSON.
    pub fn save_to_disk(&self) -> crate::Result<()> {
        let path = settings_file();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut map = serde_json::Map::new();
        for (key, entry) in &self.entries {
            map.insert(key.clone(), setting_value_to_json(&entry.value));
        }
        let content = serde_json::to_string_pretty(&serde_json::Value::Object(map))?;
        std::fs::write(&path, content)?;
        tracing::info!(path = %path.display(), "settings saved to disk");
        Ok(())
    }

    /// Load settings from a specific path (useful for testing or import).
    pub fn load_from_path(&mut self, path: &std::path::Path) -> crate::Result<()> {
        if !path.exists() {
            return Ok(());
        }
        let content = std::fs::read_to_string(path)?;
        let map: HashMap<String, serde_json::Value> = serde_json::from_str(&content)?;
        for (key, json_val) in &map {
            if let Some(entry) = self.entries.get_mut(key.as_str()) {
                let value = json_to_setting_value(json_val);
                if entry.validate(&value).is_ok() {
                    entry.value = value;
                }
            }
        }
        Ok(())
    }

    /// Save settings to a specific path (useful for testing or export).
    pub fn save_to_path(&self, path: &std::path::Path) -> crate::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut map = serde_json::Map::new();
        for (key, entry) in &self.entries {
            map.insert(key.clone(), setting_value_to_json(&entry.value));
        }
        let content = serde_json::to_string_pretty(&serde_json::Value::Object(map))?;
        std::fs::write(path, content)?;
        Ok(())
    }

    // ---- Event handling ----

    /// Handle a setting change from the UI.
    ///
    /// Validates against policy and type constraints, records for undo,
    /// applies immediately, and queues a notification.
    pub fn handle_change(&mut self, key: &str, value: SettingValue) -> crate::Result<bool> {
        // Delegates to set_value which already checks policy, validates,
        // records undo, and sends notification.
        self.set_value(key, value)?;
        Ok(true)
    }

    /// Commit all pending changes and persist to disk.
    pub fn apply_changes(&mut self) -> crate::Result<()> {
        self.save_to_disk()
    }

    /// Revert all uncommitted changes by undoing everything in the undo stack.
    pub fn revert_changes(&mut self) {
        while self.changes.can_undo() {
            if let Ok(reversed) = self.changes.undo() {
                if let Some(entry) = self.entries.get_mut(&reversed.key) {
                    entry.value = reversed.new_value.clone();
                }
            } else {
                break;
            }
        }
    }

    // ---- Display helpers ----

    /// Get renderable settings for a specific category.
    #[must_use]
    pub fn category_settings(&self, category: Category) -> Vec<SettingDisplay> {
        self.entries.values()
            .filter(|e| e.category == category)
            .filter(|e| self.policy.is_visible(&e.key))
            .map(|entry| SettingDisplay {
                key: entry.key.clone(),
                label: entry.label.clone(),
                description: entry.description.clone(),
                value: entry.value.clone(),
                locked: !self.policy.is_editable(&entry.key),
                category,
            })
            .collect()
    }

    /// Get renderable settings for the active category.
    #[must_use]
    pub fn active_category_settings(&self) -> Vec<SettingDisplay> {
        self.category_settings(self.active_category)
    }

    /// Collect all entries as a JSON-serializable map (key -> value).
    #[must_use]
    pub fn all_entries_as_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        for (key, entry) in &self.entries {
            map.insert(key.clone(), setting_value_to_json(&entry.value));
        }
        serde_json::Value::Object(map)
    }
}
