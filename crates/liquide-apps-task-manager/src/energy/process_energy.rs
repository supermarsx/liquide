//! Per-process energy consumption data (spec section 15.4).

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// EfficiencyRating
// ---------------------------------------------------------------------------

/// Energy efficiency grade assigned to a process (spec section 15.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EfficiencyRating {
    VeryLow,
    Low,
    Moderate,
    High,
    VeryHigh,
    Critical,
}

impl EfficiencyRating {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::VeryLow => "Very Low",
            Self::Low => "Low",
            Self::Moderate => "Moderate",
            Self::High => "High",
            Self::VeryHigh => "Very High",
            Self::Critical => "Critical",
        }
    }
}

impl fmt::Display for EfficiencyRating {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// ProcessEnergyInfo
// ---------------------------------------------------------------------------

/// Per-process energy consumption and efficiency data (spec section 15.4).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessEnergyInfo {
    /// Process ID.
    pub pid: u32,
    /// Process name.
    pub name: String,
    /// CPU-attributable power draw in milliwatts.
    pub cpu_power_mw: f64,
    /// GPU-attributable power draw in milliwatts.
    pub gpu_power_mw: f64,
    /// Storage-attributable power draw in milliwatts.
    pub disk_power_mw: f64,
    /// Network-attributable power draw in milliwatts.
    pub network_power_mw: f64,
    /// Total estimated instantaneous power draw in milliwatts.
    pub total_power_mw: f64,
    /// Efficiency rating category.
    pub efficiency_rating: EfficiencyRating,
    /// Power trend indicator (increasing / stable / decreasing over 60s window).
    pub power_trend: String,
    /// Total energy consumed since process start in milliwatt-hours.
    pub energy_consumed_mwh: f64,
    /// Estimated carbon impact in grams of CO2 (if grid data available).
    pub carbon_impact_g: Option<f64>,
    /// Estimated battery drain percentage per hour (if on battery).
    pub battery_drain_percent_hr: Option<f64>,
    /// Current CPU usage as a percentage.
    pub cpu_percent: f64,
    /// Current GPU usage as a percentage.
    pub gpu_percent: f64,
    /// Current disk I/O rate in bytes per second.
    pub disk_bytes_sec: u64,
    /// Whether the process is a background process.
    pub background: bool,
}

impl Default for ProcessEnergyInfo {
    fn default() -> Self {
        Self {
            pid: 0,
            name: String::new(),
            cpu_power_mw: 0.0,
            gpu_power_mw: 0.0,
            disk_power_mw: 0.0,
            network_power_mw: 0.0,
            total_power_mw: 0.0,
            efficiency_rating: EfficiencyRating::VeryLow,
            power_trend: String::new(),
            energy_consumed_mwh: 0.0,
            carbon_impact_g: None,
            battery_drain_percent_hr: None,
            cpu_percent: 0.0,
            gpu_percent: 0.0,
            disk_bytes_sec: 0,
            background: false,
        }
    }
}
