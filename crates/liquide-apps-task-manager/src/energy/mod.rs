//! Energy & Power monitoring types (spec section 15).
//!
//! Provides data structures for system power consumption analysis,
//! per-component and per-process energy tracking, battery health
//! analytics, thermal management, and carbon footprint estimation.

pub mod battery;
pub mod carbon;
pub mod component;
pub mod history;
pub mod power_plan;
pub mod process_energy;
pub mod thermal;
pub mod wake_lock;

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// EnergyView
// ---------------------------------------------------------------------------

/// Sidebar navigation views for the Energy & Power tab (spec section 15.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnergyView {
    Overview,
    PerProcess,
    Components,
    Battery,
    Thermal,
    PowerPlan,
    History,
    Carbon,
    WakeLocks,
}

impl EnergyView {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::PerProcess => "Per Process",
            Self::Components => "Components",
            Self::Battery => "Battery",
            Self::Thermal => "Thermal",
            Self::PowerPlan => "Power Plan",
            Self::History => "History",
            Self::Carbon => "Carbon",
            Self::WakeLocks => "Wake Locks",
        }
    }
}

impl fmt::Display for EnergyView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// PowerSource
// ---------------------------------------------------------------------------

/// System power source type (spec section 15.3 – Power Source widget).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PowerSource {
    Ac,
    Battery,
    Usb,
}

impl PowerSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ac => "AC",
            Self::Battery => "Battery",
            Self::Usb => "USB",
        }
    }
}

impl fmt::Display for PowerSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// EnergyOverview
// ---------------------------------------------------------------------------

/// Aggregated energy dashboard data (spec section 15.3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnergyOverview {
    /// Current power source (AC / Battery / USB).
    pub power_source: PowerSource,
    /// Real-time total system power draw in watts.
    pub total_power_watts: f64,
    /// CPU package power draw in watts.
    pub cpu_power_watts: f64,
    /// GPU board power draw in watts.
    pub gpu_power_watts: f64,
    /// Display power draw in watts.
    pub display_power_watts: f64,
    /// Storage subsystem power draw in watts.
    pub storage_power_watts: f64,
    /// Network subsystem power draw in watts.
    pub network_power_watts: f64,
    /// Peripheral devices power draw in watts.
    pub peripheral_power_watts: f64,
    /// Battery charge percentage (if battery present).
    pub battery_percent: Option<f64>,
    /// Estimated battery time remaining in seconds (if discharging).
    pub battery_remaining_secs: Option<u64>,
    /// Energy efficiency score (0-100 or descriptive grade).
    pub energy_rating: String,
}

// ---------------------------------------------------------------------------
// PowerBreakdown
// ---------------------------------------------------------------------------

/// Single entry in the power breakdown donut chart (spec section 15.3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerBreakdown {
    /// Component name (CPU, GPU, Display, Storage, Network, Other).
    pub component: String,
    /// Power draw in watts for this component.
    pub power_watts: f64,
    /// This component's share of total system power as a percentage.
    pub percent_of_total: f64,
}
