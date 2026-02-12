//! Centralized shell configuration.

use serde::{Deserialize, Serialize};

use crate::dock::{DockConfig, DockMonitorMode, DockPosition};
use crate::launcher::{AppCategory, LauncherConfig, LauncherView};
use crate::notification::{NotificationConfig, NotificationPosition};
use crate::seamless::{SeamlessConfig, SeamlessMode};
use crate::status_bar::StatusBarConfig;
use crate::tiling::{TilingConfig, TilingLayoutKind, TilingMode};

/// Aggregate shell configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

impl Default for ShellConfig {
    fn default() -> Self {
        Self {
            dock: DockConfig::default(),
            status_bar: StatusBarConfig::default(),
            launcher: LauncherConfig::default(),
            tiling: TilingConfig::default(),
            notifications: NotificationConfig::default(),
            seamless: SeamlessConfig::default(),
        }
    }
}

impl std::fmt::Display for ShellConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ShellConfig(dock={}, status_bar={}, launcher={}, tiling={})",
            if self.dock.auto_hide { "auto-hide" } else { "visible" },
            if self.status_bar.enabled { "on" } else { "off" },
            if self.launcher.calculator_enabled { "calc" } else { "no-calc" },
            if self.tiling.enabled { "on" } else { "off" },
        )
    }
}
