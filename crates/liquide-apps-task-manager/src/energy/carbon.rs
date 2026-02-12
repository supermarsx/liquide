//! Carbon footprint tracking data (spec section 15.10).

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// CarbonIntensitySource
// ---------------------------------------------------------------------------

/// Source of carbon intensity data (spec section 15.10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CarbonIntensitySource {
    Grid,
    Average,
    Estimated,
    Manual,
}

impl CarbonIntensitySource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Grid => "Grid",
            Self::Average => "Average",
            Self::Estimated => "Estimated",
            Self::Manual => "Manual",
        }
    }
}

impl fmt::Display for CarbonIntensitySource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// CarbonFootprint
// ---------------------------------------------------------------------------

/// Carbon emissions tracking data (spec section 15.10).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CarbonFootprint {
    /// Total estimated CO2 emissions in grams.
    pub total_grams_co2: f64,
    /// CO2 emissions for the current session in grams.
    pub this_session_grams: f64,
    /// Carbon intensity of the electricity source in grams CO2 per kWh.
    pub intensity_g_per_kwh: f64,
    /// Source of the carbon intensity data.
    pub source: CarbonIntensitySource,
    /// Geographic region for grid carbon intensity data.
    pub region: Option<String>,
    /// When the carbon intensity data was last updated (ISO 8601 format).
    pub last_updated: Option<String>,
}

// ---------------------------------------------------------------------------
// CarbonBudget
// ---------------------------------------------------------------------------

/// Personal carbon budget tracking (spec section 15.10).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CarbonBudget {
    /// Daily carbon budget target in grams of CO2.
    pub daily_budget_grams: f64,
    /// Carbon used so far today in grams of CO2.
    pub used_grams: f64,
    /// Remaining carbon budget for today in grams of CO2.
    pub remaining_grams: f64,
    /// Whether current usage is on track to stay within budget.
    pub on_track: bool,
}
