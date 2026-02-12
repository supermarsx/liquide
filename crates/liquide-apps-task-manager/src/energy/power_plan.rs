//! Power plan and profile configuration data (spec section 15.8).

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// CoolingPolicy
// ---------------------------------------------------------------------------

/// System cooling policy preference (spec section 15.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoolingPolicy {
    Active,
    Passive,
}

impl CoolingPolicy {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "Active",
            Self::Passive => "Passive",
        }
    }
}

impl fmt::Display for CoolingPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// PowerPlan
// ---------------------------------------------------------------------------

/// A system power plan / profile (spec section 15.8).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerPlan {
    /// Human-readable plan name.
    pub name: String,
    /// Unique power plan identifier.
    pub id: String,
    /// Whether this plan is currently active.
    pub active: bool,
    /// Optional description of the plan's purpose.
    pub description: Option<String>,
    /// Detailed power plan settings.
    pub settings: PowerPlanSettings,
}

// ---------------------------------------------------------------------------
// PowerPlanSettings
// ---------------------------------------------------------------------------

/// Detailed settings within a power plan (spec section 15.8).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerPlanSettings {
    /// Minimum CPU performance state as a percentage (0-100).
    pub min_cpu_percent: u8,
    /// Maximum CPU performance state as a percentage (0-100).
    pub max_cpu_percent: u8,
    /// Cooling policy: active (fan first) or passive (throttle first).
    pub cooling_policy: CoolingPolicy,
    /// Default display brightness on AC power (percentage).
    pub display_brightness_ac: u8,
    /// Default display brightness on battery power (percentage).
    pub display_brightness_dc: u8,
    /// Display off timeout on AC power in seconds.
    pub display_timeout_ac_secs: u32,
    /// Display off timeout on battery power in seconds.
    pub display_timeout_dc_secs: u32,
    /// Sleep timeout on AC power in seconds.
    pub sleep_timeout_ac_secs: u32,
    /// Sleep timeout on battery power in seconds.
    pub sleep_timeout_dc_secs: u32,
    /// Hibernate timeout in seconds (0 = disabled).
    pub hibernate_timeout_secs: u32,
    /// Whether USB selective suspend is enabled.
    pub usb_selective_suspend: bool,
    /// PCIe link state power management level.
    pub pcie_link_state: String,
    /// Whether processor turbo boost is enabled.
    pub processor_boost: bool,
    /// Hard disk spin-down timeout in seconds.
    pub hard_disk_timeout_secs: u32,
}

impl Default for PowerPlanSettings {
    fn default() -> Self {
        Self {
            min_cpu_percent: 5,
            max_cpu_percent: 100,
            cooling_policy: CoolingPolicy::Active,
            display_brightness_ac: 80,
            display_brightness_dc: 40,
            display_timeout_ac_secs: 600,
            display_timeout_dc_secs: 300,
            sleep_timeout_ac_secs: 1800,
            sleep_timeout_dc_secs: 900,
            hibernate_timeout_secs: 0,
            usb_selective_suspend: true,
            pcie_link_state: String::from("moderate"),
            processor_boost: true,
            hard_disk_timeout_secs: 1200,
        }
    }
}
