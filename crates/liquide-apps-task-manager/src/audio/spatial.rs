//! Spatial audio configuration types (spec section 16.9).

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// SpatialEngine
// ---------------------------------------------------------------------------

/// Spatial audio rendering engine (spec section 16.9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpatialEngine {
    None,
    WindowsSonic,
    DolbyAtmos,
    DtsX,
    Custom,
}

impl SpatialEngine {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::WindowsSonic => "Windows Sonic",
            Self::DolbyAtmos => "Dolby Atmos",
            Self::DtsX => "DTS:X",
            Self::Custom => "Custom",
        }
    }
}

impl fmt::Display for SpatialEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// SpatialConfig
// ---------------------------------------------------------------------------

/// Spatial audio configuration and state (spec section 16.9).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpatialConfig {
    /// Active spatial audio rendering engine.
    pub engine: SpatialEngine,
    /// Whether spatial audio processing is enabled.
    pub enabled: bool,
    /// Whether head tracking is active (requires compatible hardware).
    pub head_tracking: bool,
    /// Whether speaker virtualization is enabled.
    pub virtualization: bool,
    /// Virtual room size description (e.g., "Small", "Medium", "Large").
    pub room_size: String,
    /// Virtual speaker layout description (e.g., "7.1.4 Atmos").
    pub speaker_layout: String,
    /// Cross-feed level for headphone virtualization (0.0–1.0).
    pub crossfeed: Option<f64>,
    /// Active HRTF profile name (e.g., "Generic", "Personalized").
    pub hrtf_profile: Option<String>,
    /// Whether bass management / redirection to subwoofer is enabled.
    pub bass_management: bool,
    /// Whether dialog / centre-channel enhancement is enabled.
    pub dialog_enhancement: bool,
}
