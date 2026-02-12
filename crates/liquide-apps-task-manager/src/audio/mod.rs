//! Audio device management, stream monitoring, routing, and analysis types
//! (spec section 16).
//!
//! Provides data structures for output/input device management, per-process
//! audio stream tracking, audio routing, effects/DSP chains, spatial audio,
//! MIDI devices, real-time statistics, and audio diagnostics.

pub mod device;
pub mod diagnostics;
pub mod effects;
pub mod midi;
pub mod routing;
pub mod spatial;
pub mod stats;
pub mod stream;

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// AudioView
// ---------------------------------------------------------------------------

/// Sidebar navigation views for the Audio tab (spec section 16.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioView {
    OutputDevices,
    InputDevices,
    Streams,
    Routing,
    Effects,
    Spatial,
    Midi,
    Stats,
    Diagnostics,
    Overview,
}

impl AudioView {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OutputDevices => "Output Devices",
            Self::InputDevices => "Input Devices",
            Self::Streams => "Streams",
            Self::Routing => "Routing",
            Self::Effects => "Effects",
            Self::Spatial => "Spatial",
            Self::Midi => "MIDI",
            Self::Stats => "Stats",
            Self::Diagnostics => "Diagnostics",
            Self::Overview => "Overview",
        }
    }
}

impl fmt::Display for AudioView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
