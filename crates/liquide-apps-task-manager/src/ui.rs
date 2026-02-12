//! Global UI shell types for the task manager.
//!
//! Defines tab identifiers, view modes, theme modes, status bar state,
//! column visibility, and sort state corresponding to spec section 3.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Identifies a top-level tab in the task manager.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TabId {
    /// Running processes view.
    Processes,
    /// System performance graphs and stats.
    Performance,
    /// Historical per-application resource usage.
    AppHistory,
    /// Boot and login startup entries.
    Startup,
    /// Logged-in user sessions.
    Users,
    /// System services management.
    Services,
    /// Hardware device inventory.
    Devices,
    /// Currently open file handles.
    FilesInUse,
    /// Locked resource finder and releaser.
    ResourceUnlocking,
    /// Full process hierarchy view.
    ProcessTree,
    /// Network traffic monitoring and analysis.
    NetworkTraffic,
    /// Power consumption and thermal management.
    EnergyPower,
    /// Audio device and stream management.
    Audio,
    /// System event log viewer.
    SystemEventViewer,
}

impl TabId {
    /// Return a human-readable name for this tab.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Processes => "Processes",
            Self::Performance => "Performance",
            Self::AppHistory => "App History",
            Self::Startup => "Startup",
            Self::Users => "Users",
            Self::Services => "Services",
            Self::Devices => "Devices",
            Self::FilesInUse => "Files In Use",
            Self::ResourceUnlocking => "Resource Unlocking",
            Self::ProcessTree => "Process Tree",
            Self::NetworkTraffic => "Network Traffic",
            Self::EnergyPower => "Energy & Power",
            Self::Audio => "Audio",
            Self::SystemEventViewer => "Event Viewer",
        }
    }
}

impl fmt::Display for TabId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Window view mode controlling detail level and layout.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewMode {
    /// Processes tab only, minimal columns, resizable mini-window.
    Compact,
    /// All tabs, default column sets, medium-resolution graphs.
    Standard,
    /// All tabs, all columns visible, high-resolution graphs, debug info.
    Advanced,
    /// Always-on-top mini overlay showing CPU/RAM/GPU gauges.
    FloatingWidget,
}

impl ViewMode {
    /// Return a human-readable name for this view mode.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Compact => "Compact",
            Self::Standard => "Standard",
            Self::Advanced => "Advanced",
            Self::FloatingWidget => "Floating Widget",
        }
    }
}

impl fmt::Display for ViewMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Theme mode controlling visual appearance.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    /// Light desktop theme.
    Light,
    /// Dark desktop theme.
    Dark,
    /// Custom user-defined theme.
    Custom,
    /// High-contrast mode with WCAG AAA compliance.
    HighContrast,
}

impl ThemeMode {
    /// Return a human-readable name for this theme mode.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Light => "Light",
            Self::Dark => "Dark",
            Self::Custom => "Custom",
            Self::HighContrast => "High Contrast",
        }
    }
}

impl fmt::Display for ThemeMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Bottom status bar showing at-a-glance system totals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusBar {
    /// Overall CPU utilization percentage (0.0 – 100.0).
    pub cpu_percent: f64,
    /// Current CPU clock speed in GHz.
    pub cpu_speed_ghz: f64,
    /// Physical RAM currently in use (bytes).
    pub ram_used_bytes: u64,
    /// Total installed physical RAM (bytes).
    pub ram_total_bytes: u64,
    /// Current disk read throughput (bytes per second).
    pub disk_read_bytes_sec: u64,
    /// Current disk write throughput (bytes per second).
    pub disk_write_bytes_sec: u64,
    /// Current network send throughput (bytes per second).
    pub net_send_bytes_sec: u64,
    /// Current network receive throughput (bytes per second).
    pub net_recv_bytes_sec: u64,
    /// Overall GPU utilization percentage (0.0 – 100.0).
    pub gpu_percent: f64,
    /// GPU VRAM currently in use (bytes).
    pub gpu_vram_used_bytes: u64,
    /// Total GPU VRAM (bytes).
    pub gpu_vram_total_bytes: u64,
    /// Number of running processes.
    pub process_count: u32,
    /// Total thread count across all processes.
    pub thread_count: u32,
    /// System uptime in seconds.
    pub uptime_seconds: u64,
    /// Battery charge percentage, if a battery is present.
    pub battery_percent: Option<u8>,
    /// Estimated battery time remaining in seconds.
    pub battery_remaining_secs: Option<u64>,
    /// Name of the currently active audio output device.
    pub audio_output_name: Option<String>,
    /// System-wide power draw in watts.
    pub power_draw_watts: Option<f64>,
}

impl Default for StatusBar {
    fn default() -> Self {
        Self {
            cpu_percent: 0.0,
            cpu_speed_ghz: 0.0,
            ram_used_bytes: 0,
            ram_total_bytes: 0,
            disk_read_bytes_sec: 0,
            disk_write_bytes_sec: 0,
            net_send_bytes_sec: 0,
            net_recv_bytes_sec: 0,
            gpu_percent: 0.0,
            gpu_vram_used_bytes: 0,
            gpu_vram_total_bytes: 0,
            process_count: 0,
            thread_count: 0,
            uptime_seconds: 0,
            battery_percent: None,
            battery_remaining_secs: None,
            audio_output_name: None,
            power_draw_watts: None,
        }
    }
}

/// Controls visibility and ordering of a single table column.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnVisibility {
    /// Column identifier key (e.g. `"cpu_percent"`).
    pub column_key: String,
    /// Whether the column is currently visible.
    pub visible: bool,
    /// Display order (lower values appear first).
    pub order: u16,
    /// Fixed width in pixels, or `None` for auto-sizing.
    pub width_px: Option<u16>,
}

/// Sort direction for a table column.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortOrder {
    /// Smallest values first.
    Ascending,
    /// Largest values first.
    Descending,
}

impl SortOrder {
    /// Return a human-readable name for this sort order.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ascending => "Ascending",
            Self::Descending => "Descending",
        }
    }
}

impl fmt::Display for SortOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Current sort state for a table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SortState {
    /// The column key currently being sorted on.
    pub column_key: String,
    /// The direction of the sort.
    pub order: SortOrder,
}
