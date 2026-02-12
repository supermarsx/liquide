//! Thermal management and fan control data (spec section 15.7).

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// ThermalStatus
// ---------------------------------------------------------------------------

/// Thermal zone status indicator (spec section 15.7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThermalStatus {
    Normal,
    Warm,
    Hot,
    Critical,
    Emergency,
}

impl ThermalStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Warm => "Warm",
            Self::Hot => "Hot",
            Self::Critical => "Critical",
            Self::Emergency => "Emergency",
        }
    }
}

impl fmt::Display for ThermalStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// ThermalTrend
// ---------------------------------------------------------------------------

/// Temperature change direction (spec section 15.7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThermalTrend {
    Rising,
    Stable,
    Falling,
}

impl ThermalTrend {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Rising => "Rising",
            Self::Stable => "Stable",
            Self::Falling => "Falling",
        }
    }
}

impl fmt::Display for ThermalTrend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// FanMode
// ---------------------------------------------------------------------------

/// Fan control operating mode (spec section 15.7.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FanMode {
    Auto,
    Manual,
    Silent,
    Performance,
}

impl FanMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Manual => "Manual",
            Self::Silent => "Silent",
            Self::Performance => "Performance",
        }
    }
}

impl fmt::Display for FanMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// ThermalSensor
// ---------------------------------------------------------------------------

/// Individual thermal sensor reading (spec section 15.7.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalSensor {
    /// Sensor descriptive name (e.g., "CPU Package", "GPU Core").
    pub name: String,
    /// Component or zone location identifier.
    pub location: String,
    /// Current temperature reading in degrees Celsius.
    pub temperature_celsius: f64,
    /// Warning threshold temperature in degrees Celsius.
    pub max_temperature_celsius: f64,
    /// Critical / shutdown threshold temperature in degrees Celsius.
    pub critical_temperature_celsius: f64,
    /// Current thermal status indicator.
    pub status: ThermalStatus,
    /// Temperature change direction.
    pub trend: ThermalTrend,
    /// Minimum temperature recorded during the session.
    pub min_recorded: f64,
    /// Maximum temperature recorded during the session.
    pub max_recorded: f64,
    /// Mean temperature recorded during the session.
    pub avg_temperature: f64,
    /// Number of temperature readings taken.
    pub reading_count: u64,
}

// ---------------------------------------------------------------------------
// FanInfo
// ---------------------------------------------------------------------------

/// Fan status and control data (spec section 15.7.3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FanInfo {
    /// Fan identifier (e.g., "CPU Fan", "System Fan 1").
    pub name: String,
    /// Current fan speed in revolutions per minute.
    pub speed_rpm: u32,
    /// Maximum rated fan speed in revolutions per minute.
    pub max_speed_rpm: u32,
    /// Current duty cycle as a percentage (0.0 to 100.0).
    pub speed_percent: f64,
    /// Current fan control mode.
    pub mode: FanMode,
    /// Whether the fan speed can be controlled by the user.
    pub controllable: bool,
}
