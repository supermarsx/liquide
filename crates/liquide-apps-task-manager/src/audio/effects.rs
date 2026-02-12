//! Audio effects and DSP processing chain types (spec section 16.8).

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// AudioEffect
// ---------------------------------------------------------------------------

/// Audio processing effect types available in the system DSP chain
/// (spec section 16.8.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioEffect {
    Equalizer,
    Compressor,
    Limiter,
    NoiseGate,
    Reverb,
    Echo,
    BassBoost,
    VirtualSurround,
    LoudnessEqualization,
    RoomCorrection,
}

impl AudioEffect {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Equalizer => "Equalizer",
            Self::Compressor => "Compressor",
            Self::Limiter => "Limiter",
            Self::NoiseGate => "Noise Gate",
            Self::Reverb => "Reverb",
            Self::Echo => "Echo",
            Self::BassBoost => "Bass Boost",
            Self::VirtualSurround => "Virtual Surround",
            Self::LoudnessEqualization => "Loudness Equalization",
            Self::RoomCorrection => "Room Correction",
        }
    }
}

impl fmt::Display for AudioEffect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// EffectNode
// ---------------------------------------------------------------------------

/// A single node in the audio effects processing chain (spec section 16.8.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectNode {
    /// The type of audio effect.
    pub effect: AudioEffect,
    /// Whether this effect node is enabled in the chain.
    pub enabled: bool,
    /// Device identifier this effect is applied to.
    pub device_id: String,
    /// Position in the processing chain (lower = earlier).
    pub order: u8,
    /// Serialized effect parameters (JSON or DSP-specific format).
    pub parameters: String,
}

// ---------------------------------------------------------------------------
// DspLoad
// ---------------------------------------------------------------------------

/// DSP processing load for a single device (spec section 16.8.3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DspLoad {
    /// Device identifier.
    pub device_id: String,
    /// Friendly device name.
    pub device_name: String,
    /// CPU usage consumed by audio processing as a percentage.
    pub cpu_percent: f64,
    /// Total latency added by the effects chain in milliseconds.
    pub latency_contribution_ms: f64,
    /// Number of active effects in the processing chain.
    pub effect_count: u32,
}
