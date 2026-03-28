//! Status bar configuration.

use serde::{Deserialize, Serialize};

/// Configuration for the status bar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusBarConfig {
    /// Whether the status bar is visible.
    pub enabled: bool,
    /// Height in logical pixels.
    pub height: f32,
    /// Show the application name and menu bar on the left.
    pub show_app_menu: bool,
    /// Application name to display.
    pub app_name: String,
    /// Show the clock.
    pub show_clock: bool,
    /// Clock format string (strftime-style).
    pub clock_format: String,
    /// Show system tray icons.
    pub show_tray: bool,
    /// Show notification indicator.
    pub show_notifications: bool,
    /// Show the dark/light mode toggle.
    pub show_theme_toggle: bool,
    /// Auto-hide on maximized windows.
    pub auto_hide: bool,
    /// Padding inside the bar.
    pub padding: f32,
    /// Item spacing.
    pub item_spacing: f32,
    /// Glass blur radius.
    pub blur_radius: f32,
}

impl Default for StatusBarConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            height: 34.0,
            show_app_menu: true,
            app_name: "Liquide".into(),
            show_clock: true,
            clock_format: "%H:%M".into(),
            show_tray: true,
            show_notifications: true,
            show_theme_toggle: true,
            auto_hide: false,
            padding: 8.0,
            item_spacing: 12.0,
            blur_radius: 24.0,
        }
    }
}
