//! App history types for the App History tab (spec section 6).
//!
//! Tracks historical resource consumption per application over configurable
//! time periods, including CPU, GPU, network, disk, notifications, and more.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Time period for filtering app history data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimePeriod {
    Today,
    Yesterday,
    LastWeek,
    LastMonth,
    AllTime,
}

impl TimePeriod {
    /// Returns the string representation of this time period.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Today => "Today",
            Self::Yesterday => "Yesterday",
            Self::LastWeek => "Last Week",
            Self::LastMonth => "Last Month",
            Self::AllTime => "All Time",
        }
    }
}

impl fmt::Display for TimePeriod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Historical resource consumption record for a single application.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppHistoryEntry {
    /// Application name.
    pub name: String,
    /// Developer or publisher name.
    pub publisher: Option<String>,
    /// Total accumulated CPU time in milliseconds.
    pub cpu_time_total_ms: u64,
    /// CPU time spent while the application was in the foreground, in milliseconds.
    pub cpu_time_foreground_ms: u64,
    /// Total network bytes transferred (upload + download).
    pub network_bytes_total: u64,
    /// Network bytes transferred while in the foreground.
    pub network_bytes_foreground: u64,
    /// Network bytes transferred on metered connections.
    pub metered_network_bytes: u64,
    /// Number of live tile or widget updates.
    pub tile_updates: u32,
    /// Number of notifications sent by the application.
    pub notifications_sent: u32,
    /// Total GPU time consumed in milliseconds.
    pub gpu_time_ms: u64,
    /// Peak dedicated GPU memory usage in bytes.
    pub gpu_dedicated_bytes_peak: u64,
    /// Peak shared GPU memory usage in bytes.
    pub gpu_shared_bytes_peak: u64,
    /// Total bytes read from disk.
    pub disk_read_total_bytes: u64,
    /// Total bytes written to disk.
    pub disk_write_total_bytes: u64,
    /// Average power usage category (e.g. "Very Low", "Low", "Moderate", "High", "Very High").
    pub power_usage_avg: Option<String>,
    /// Number of times the application was launched.
    pub launch_count: u32,
    /// Timestamp of the last execution.
    pub last_used: Option<String>,
    /// Timestamp of when the application was first seen.
    pub first_seen: Option<String>,
}

impl Default for AppHistoryEntry {
    fn default() -> Self {
        Self {
            name: String::new(),
            publisher: None,
            cpu_time_total_ms: 0,
            cpu_time_foreground_ms: 0,
            network_bytes_total: 0,
            network_bytes_foreground: 0,
            metered_network_bytes: 0,
            tile_updates: 0,
            notifications_sent: 0,
            gpu_time_ms: 0,
            gpu_dedicated_bytes_peak: 0,
            gpu_shared_bytes_peak: 0,
            disk_read_total_bytes: 0,
            disk_write_total_bytes: 0,
            power_usage_avg: None,
            launch_count: 0,
            last_used: None,
            first_seen: None,
        }
    }
}
