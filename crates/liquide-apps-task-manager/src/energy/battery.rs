//! Battery analytics and health data (spec section 15.6).

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// BatteryState
// ---------------------------------------------------------------------------

/// Current battery charge/discharge state (spec section 15.6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatteryState {
    Charging,
    Discharging,
    Full,
    NotCharging,
    Unknown,
}

impl BatteryState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Charging => "Charging",
            Self::Discharging => "Discharging",
            Self::Full => "Full",
            Self::NotCharging => "Not Charging",
            Self::Unknown => "Unknown",
        }
    }
}

impl fmt::Display for BatteryState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// BatteryChemistry
// ---------------------------------------------------------------------------

/// Battery cell chemistry type (spec section 15.6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatteryChemistry {
    LithiumIon,
    LithiumPolymer,
    NickelMetalHydride,
}

impl BatteryChemistry {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::LithiumIon => "Lithium Ion",
            Self::LithiumPolymer => "Lithium Polymer",
            Self::NickelMetalHydride => "Nickel Metal Hydride",
        }
    }
}

impl fmt::Display for BatteryChemistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// BatteryStatus
// ---------------------------------------------------------------------------

/// Comprehensive battery status panel data (spec section 15.6.1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatteryStatus {
    /// Whether a battery is physically present.
    pub present: bool,
    /// Current battery state (Charging / Discharging / Full / etc.).
    pub state: BatteryState,
    /// Battery cell chemistry.
    pub chemistry: BatteryChemistry,
    /// Current charge level as a percentage (0.0 to 100.0).
    pub charge_percent: f64,
    /// Current battery voltage in millivolts.
    pub voltage_mv: u32,
    /// Current charge/discharge current in milliamps (positive = charging).
    pub current_ma: i32,
    /// Battery temperature in degrees Celsius.
    pub temperature_celsius: Option<f64>,
    /// Original designed capacity in milliwatt-hours.
    pub design_capacity_mwh: u32,
    /// Current maximum capacity at full charge in milliwatt-hours.
    pub full_charge_capacity_mwh: u32,
    /// Remaining capacity in milliwatt-hours.
    pub remaining_capacity_mwh: u32,
    /// Current charging power in watts (when charging).
    pub charge_rate_watts: Option<f64>,
    /// Current discharge power in watts (when discharging).
    pub discharge_rate_watts: Option<f64>,
    /// Estimated seconds until fully charged.
    pub time_to_full_secs: Option<u64>,
    /// Estimated seconds until empty.
    pub time_to_empty_secs: Option<u64>,
    /// Total number of charge/discharge cycles.
    pub cycle_count: Option<u32>,
    /// Battery health as a percentage (full charge capacity / design capacity).
    pub health_percent: Option<f64>,
    /// Battery manufacturer name.
    pub manufacturer: Option<String>,
    /// Battery serial number.
    pub serial_number: Option<String>,
}

impl Default for BatteryStatus {
    fn default() -> Self {
        Self {
            present: false,
            state: BatteryState::Unknown,
            chemistry: BatteryChemistry::LithiumIon,
            charge_percent: 0.0,
            voltage_mv: 0,
            current_ma: 0,
            temperature_celsius: None,
            design_capacity_mwh: 0,
            full_charge_capacity_mwh: 0,
            remaining_capacity_mwh: 0,
            charge_rate_watts: None,
            discharge_rate_watts: None,
            time_to_full_secs: None,
            time_to_empty_secs: None,
            cycle_count: None,
            health_percent: None,
            manufacturer: None,
            serial_number: None,
        }
    }
}

// ---------------------------------------------------------------------------
// BatteryHealthReport
// ---------------------------------------------------------------------------

/// Battery health report data (spec section 15.6.3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatteryHealthReport {
    /// Original designed capacity in milliwatt-hours.
    pub design_capacity_mwh: u32,
    /// Current maximum capacity at full charge in milliwatt-hours.
    pub current_full_capacity_mwh: u32,
    /// Battery health as a percentage.
    pub health_percent: f64,
    /// Total number of charge/discharge cycles.
    pub cycle_count: u32,
    /// Date the battery was first used (ISO 8601 format).
    pub first_use_date: Option<String>,
    /// Estimated remaining useful battery life in months.
    pub estimated_life_remaining_months: Option<u32>,
}

// ---------------------------------------------------------------------------
// ChargeHabits
// ---------------------------------------------------------------------------

/// User charging habit analysis (spec section 15.6.3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChargeHabits {
    /// Average battery level when charging begins (percentage).
    pub avg_charge_start_percent: f64,
    /// Average battery level when charging ends (percentage).
    pub avg_charge_end_percent: f64,
    /// Average number of charge sessions per day.
    pub charges_per_day: f64,
    /// Average duration of each charge session in hours.
    pub avg_session_hours: f64,
    /// Whether smart/optimized charging is enabled.
    pub smart_charging_enabled: bool,
}
