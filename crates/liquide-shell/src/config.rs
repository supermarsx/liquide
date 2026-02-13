//! Centralized shell configuration.

use serde::{Deserialize, Serialize};

use crate::dock::DockConfig;
use crate::launcher::LauncherConfig;
use crate::notification::NotificationConfig;
use crate::seamless::SeamlessConfig;
use crate::status_bar::StatusBarConfig;
use crate::tiling::TilingConfig;

/// Aggregate shell configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ShellConfig {
    /// Dock settings.
    pub dock: DockConfig,
    /// Status bar settings.
    pub status_bar: StatusBarConfig,
    /// App launcher settings.
    pub launcher: LauncherConfig,
    /// Tiling engine settings.
    pub tiling: TilingConfig,
    /// Notification settings.
    pub notifications: NotificationConfig,
    /// Seamless window mode settings.
    pub seamless: SeamlessConfig,
    /// Window management settings.
    pub window_management: WindowManagementConfig,
}

/// Window management behavior configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowManagementConfig {
    /// Automatically repatriate windows that go off-screen.
    pub auto_repatriate: bool,
    /// Minimum visible pixels before forcing repatriation.
    pub repatriation_threshold_px: f32,
    /// Anti-flicker frame limiter (minimum ms between frames).
    pub anti_flicker_min_frame_interval_ms: u64,
    /// Enable double-buffered scene submission to prevent tearing.
    pub enable_anti_flicker_insurance: bool,
}

impl Default for WindowManagementConfig {
    fn default() -> Self {
        Self {
            auto_repatriate: true,
            repatriation_threshold_px: 50.0,
            anti_flicker_min_frame_interval_ms: 8, // ~120Hz max
            enable_anti_flicker_insurance: true,
        }
    }
}

impl std::fmt::Display for ShellConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ShellConfig(dock={}, status_bar={}, launcher={}, tiling={})",
            if self.dock.auto_hide {
                "auto-hide"
            } else {
                "visible"
            },
            if self.status_bar.enabled { "on" } else { "off" },
            if self.launcher.calculator_enabled {
                "calc"
            } else {
                "no-calc"
            },
            if self.tiling.enabled { "on" } else { "off" },
        )
    }
}
