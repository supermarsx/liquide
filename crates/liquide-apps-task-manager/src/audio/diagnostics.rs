//! Audio diagnostics, test types, and event logging
//! (spec section 16.12–16.13).

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// AudioEventType
// ---------------------------------------------------------------------------

/// Types of events recorded in the audio event log (spec section 16.13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioEventType {
    DeviceAdded,
    DeviceRemoved,
    DeviceStateChanged,
    DefaultChanged,
    FormatChanged,
    VolumeChanged,
    StreamCreated,
    StreamDestroyed,
    ExclusiveModeChanged,
    GlitchDetected,
    DriverError,
}

impl AudioEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DeviceAdded => "Device Added",
            Self::DeviceRemoved => "Device Removed",
            Self::DeviceStateChanged => "Device State Changed",
            Self::DefaultChanged => "Default Changed",
            Self::FormatChanged => "Format Changed",
            Self::VolumeChanged => "Volume Changed",
            Self::StreamCreated => "Stream Created",
            Self::StreamDestroyed => "Stream Destroyed",
            Self::ExclusiveModeChanged => "Exclusive Mode Changed",
            Self::GlitchDetected => "Glitch Detected",
            Self::DriverError => "Driver Error",
        }
    }
}

impl fmt::Display for AudioEventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// AudioTest
// ---------------------------------------------------------------------------

/// Diagnostic tests that can be run on audio devices (spec section 16.12).
///
/// Not `Copy` because several variants carry `String` data.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioTest {
    /// Play test tones (sine, pink noise, sweep) through a named output.
    ToneGenerator(String),
    /// Play sequential tones from each speaker to verify channel mapping.
    ChannelCheck,
    /// Measure round-trip audio latency.
    LatencyMeasurement,
    /// Loopback test (requires loopback cable or software loopback).
    LoopbackTest,
    /// Verify speaker phase alignment.
    SpeakerPhase,
    /// Record and playback from selected input with analysis.
    MicrophoneTest,
    /// Measure input device noise floor with silence.
    NoiseFloor,
    /// Sweep test to measure device frequency response.
    FrequencyResponse,
    /// Impulse response measurement.
    ImpulseResponse,
    /// Test HDMI ARC / eARC connectivity.
    HdmiArcTest,
    /// Identify active Bluetooth audio codec (SBC / AAC / aptX / LDAC / LC3).
    BluetoothCodecTest,
    /// Identify USB Audio Class version (UAC1 / UAC2 / UAC3).
    UsbDacTest,
    /// Verify spatial audio object positioning with test sounds.
    SpatialAudioTest,
    /// Run driver diagnostics (DPC / ISR latency analysis).
    DriverDiagnostics,
}

impl AudioTest {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ToneGenerator(_) => "Tone Generator",
            Self::ChannelCheck => "Channel Check",
            Self::LatencyMeasurement => "Latency Measurement",
            Self::LoopbackTest => "Loopback Test",
            Self::SpeakerPhase => "Speaker Phase",
            Self::MicrophoneTest => "Microphone Test",
            Self::NoiseFloor => "Noise Floor",
            Self::FrequencyResponse => "Frequency Response",
            Self::ImpulseResponse => "Impulse Response",
            Self::HdmiArcTest => "HDMI ARC Test",
            Self::BluetoothCodecTest => "Bluetooth Codec Test",
            Self::UsbDacTest => "USB DAC Test",
            Self::SpatialAudioTest => "Spatial Audio Test",
            Self::DriverDiagnostics => "Driver Diagnostics",
        }
    }
}

impl fmt::Display for AudioTest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// AudioEventLog
// ---------------------------------------------------------------------------

/// A single entry in the audio event log (spec section 16.13).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioEventLog {
    /// ISO-8601 timestamp of the event.
    pub timestamp: String,
    /// Type of audio event.
    pub event_type: AudioEventType,
    /// Device identifier related to the event (if applicable).
    pub device_id: Option<String>,
    /// Human-readable event description.
    pub description: String,
    /// Additional event-specific details.
    pub details: Option<String>,
}
