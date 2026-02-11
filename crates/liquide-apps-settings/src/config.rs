//! Application-level configuration for the settings app.

use serde::{Deserialize, Serialize};

/// Top-level configuration for the settings application.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsConfig {
    /// Window width in pixels.
    pub window_width: u32,
    /// Window height in pixels.
    pub window_height: u32,
    /// Whether the sidebar is expanded.
    pub sidebar_expanded: bool,
    /// Default category to show on launch.
    pub default_category: String,
    /// Whether to show advanced settings.
    pub show_advanced: bool,
    /// Search history limit.
    pub search_history_limit: usize,
}

impl Default for SettingsConfig {
    fn default() -> Self {
        Self {
            window_width: 900,
            window_height: 640,
            sidebar_expanded: true,
            default_category: "display".into(),
            show_advanced: false,
            search_history_limit: 20,
        }
    }
}
