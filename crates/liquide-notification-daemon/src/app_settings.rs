//! Per-application notification settings.
//!
//! [`AppNotificationSettings`] stores per-app overrides for notification
//! behavior: whether notifications are enabled, sound enabled, priority
//! override, and DND bypass.

use crate::layout::Priority;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Per-application notification settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    /// Whether notifications from this app are enabled. If false, all
    /// notifications from this app are silently dropped.
    pub enabled: bool,
    /// Whether sounds are enabled for this app's notifications.
    pub sound_enabled: bool,
    /// If set, overrides the visual priority for all notifications from this app.
    pub priority_override: Option<Priority>,
    /// If true, this app's notifications bypass DND mode (e.g. alarm clocks).
    pub bypass_dnd: bool,
    /// If set, overrides the default notification timeout for this app (ms).
    pub timeout_override: Option<i32>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            sound_enabled: true,
            priority_override: None,
            bypass_dnd: false,
            timeout_override: None,
        }
    }
}

/// Registry of per-application notification settings.
///
/// Apps without explicit settings use the defaults (enabled, sound on,
/// no priority override, no DND bypass).
pub struct AppNotificationSettings {
    /// Per-app settings keyed by app name.
    settings: HashMap<String, AppSettings>,
}

impl AppNotificationSettings {
    /// Creates an empty settings registry.
    pub fn new() -> Self {
        Self {
            settings: HashMap::new(),
        }
    }

    /// Returns the settings for an app. If no explicit settings exist,
    /// returns the defaults.
    pub fn get(&self, app_id: &str) -> AppSettings {
        self.settings.get(app_id).cloned().unwrap_or_default()
    }

    /// Returns a reference to the settings for an app, if explicitly configured.
    pub fn get_explicit(&self, app_id: &str) -> Option<&AppSettings> {
        self.settings.get(app_id)
    }

    /// Sets (or replaces) the settings for an app.
    pub fn set(&mut self, app_id: impl Into<String>, settings: AppSettings) {
        self.settings.insert(app_id.into(), settings);
    }

    /// Removes explicit settings for an app (reverts to defaults).
    pub fn remove(&mut self, app_id: &str) -> bool {
        self.settings.remove(app_id).is_some()
    }

    /// Enables or disables notifications for an app.
    pub fn set_enabled(&mut self, app_id: &str, enabled: bool) {
        self.settings.entry(app_id.to_string()).or_default().enabled = enabled;
    }

    /// Enables or disables sound for an app.
    pub fn set_sound_enabled(&mut self, app_id: &str, enabled: bool) {
        self.settings
            .entry(app_id.to_string())
            .or_default()
            .sound_enabled = enabled;
    }

    /// Sets a priority override for an app. Pass `None` to clear.
    pub fn set_priority_override(&mut self, app_id: &str, priority: Option<Priority>) {
        self.settings
            .entry(app_id.to_string())
            .or_default()
            .priority_override = priority;
    }

    /// Sets whether an app can bypass DND mode.
    pub fn set_bypass_dnd(&mut self, app_id: &str, bypass: bool) {
        self.settings
            .entry(app_id.to_string())
            .or_default()
            .bypass_dnd = bypass;
    }

    /// Sets a timeout override for an app. Pass `None` to clear.
    pub fn set_timeout_override(&mut self, app_id: &str, timeout: Option<i32>) {
        self.settings
            .entry(app_id.to_string())
            .or_default()
            .timeout_override = timeout;
    }

    /// Returns the number of apps with explicit settings.
    pub fn app_count(&self) -> usize {
        self.settings.len()
    }

    /// Returns all app IDs with explicit settings.
    pub fn configured_apps(&self) -> Vec<&str> {
        self.settings.keys().map(|s| s.as_str()).collect()
    }

    /// Checks whether notifications from a given app should be delivered,
    /// taking DND state into account.
    ///
    /// Returns `true` if the notification should be delivered:
    /// - App must be enabled.
    /// - If DND is active, app must have `bypass_dnd = true`.
    pub fn should_deliver(&self, app_id: &str, dnd_active: bool) -> bool {
        let settings = self.get(app_id);
        if !settings.enabled {
            return false;
        }
        if dnd_active && !settings.bypass_dnd {
            return false;
        }
        true
    }
}

impl Default for AppNotificationSettings {
    fn default() -> Self {
        Self::new()
    }
}
