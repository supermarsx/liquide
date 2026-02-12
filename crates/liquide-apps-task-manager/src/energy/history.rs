//! Energy history and reporting data (spec section 15.9).

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// EnergyHistoryEntry
// ---------------------------------------------------------------------------

/// Single point-in-time energy measurement (spec section 15.9).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnergyHistoryEntry {
    /// Timestamp for this measurement (ISO 8601 format).
    pub timestamp: String,
    /// Total system power draw at this moment in watts.
    pub power_watts: f64,
    /// Power source at this moment (AC / Battery / USB).
    pub source: String,
    /// Battery charge percentage at this moment (if on battery).
    pub battery_percent: Option<f64>,
    /// CPU utilization as a percentage.
    pub cpu_percent: f64,
    /// GPU utilization as a percentage.
    pub gpu_percent: f64,
    /// Display brightness level (0-100).
    pub screen_brightness: u8,
}

// ---------------------------------------------------------------------------
// EnergyReport
// ---------------------------------------------------------------------------

/// Aggregated energy report for a time period (spec section 15.9).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnergyReport {
    /// Reporting period description (e.g., "Last 24 hours", "2026-02-12").
    pub period: String,
    /// Total energy consumed during the period in watt-hours.
    pub total_energy_wh: f64,
    /// Average power draw during the period in watts.
    pub avg_power_watts: f64,
    /// Peak power draw during the period in watts.
    pub peak_power_watts: f64,
    /// Number of battery drain events during the period.
    pub battery_drain_events: u32,
    /// Total hours the screen was on.
    pub screen_on_hours: f64,
    /// Total hours the system was in sleep state.
    pub sleep_hours: f64,
    /// Top energy-consuming applications during the period.
    pub top_consumers: Vec<String>,
}
