//! Per-process audio stream monitoring types (spec section 16.5).

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// StreamDirection
// ---------------------------------------------------------------------------

/// Whether a stream renders (output) or captures (input) audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamDirection {
    Output,
    Input,
}

impl StreamDirection {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Output => "Output",
            Self::Input => "Input",
        }
    }
}

impl fmt::Display for StreamDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// StreamFormat
// ---------------------------------------------------------------------------

/// Audio stream data format (spec section 16.5.1 – Format column).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamFormat {
    Pcm,
    Compressed,
    Raw,
    Passthrough,
}

impl StreamFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pcm => "PCM",
            Self::Compressed => "Compressed",
            Self::Raw => "Raw",
            Self::Passthrough => "Passthrough",
        }
    }
}

impl fmt::Display for StreamFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// StreamState
// ---------------------------------------------------------------------------

/// Playback / capture state of an audio stream (spec section 16.5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamState {
    Active,
    Inactive,
    Suspended,
    Error,
}

impl StreamState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "Active",
            Self::Inactive => "Inactive",
            Self::Suspended => "Suspended",
            Self::Error => "Error",
        }
    }
}

impl fmt::Display for StreamState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// StreamAction
// ---------------------------------------------------------------------------

/// Actions that can be performed on an audio stream (spec section 16.5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamAction {
    Mute,
    Unmute,
    SetVolume,
    Pause,
    Resume,
    Redirect,
    Duck,
    Unduck,
    Close,
}

impl StreamAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Mute => "Mute",
            Self::Unmute => "Unmute",
            Self::SetVolume => "Set Volume",
            Self::Pause => "Pause",
            Self::Resume => "Resume",
            Self::Redirect => "Redirect",
            Self::Duck => "Duck",
            Self::Unduck => "Unduck",
            Self::Close => "Close",
        }
    }
}

impl fmt::Display for StreamAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// AudioStream
// ---------------------------------------------------------------------------

/// A single per-process audio stream (spec section 16.5.1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioStream {
    /// Unique stream identifier.
    pub id: String,
    /// Process ID that owns this stream.
    pub pid: u32,
    /// Name of the owning process.
    pub process_name: String,
    /// Whether the stream renders output or captures input.
    pub direction: StreamDirection,
    /// Target device identifier.
    pub device_id: String,
    /// Friendly name of the target device.
    pub device_name: String,
    /// Current stream state.
    pub state: StreamState,
    /// Audio data format.
    pub format: StreamFormat,
    /// Stream sample rate in hertz.
    pub sample_rate_hz: u32,
    /// Stream bit depth.
    pub bit_depth: u16,
    /// Number of audio channels.
    pub channels: u16,
    /// Per-stream volume as a percentage (0–100).
    pub volume_percent: f64,
    /// Whether the stream is muted.
    pub muted: bool,
    /// Current peak level (dBFS).
    pub peak_level: f64,
    /// Duration the stream has been active in seconds.
    pub duration_secs: u64,
    /// Stream presentation latency in milliseconds.
    pub latency_ms: f64,
    /// Total buffer underrun count (audio glitches).
    pub underruns: u64,
    /// Total buffer overrun count.
    pub overruns: u64,
    /// Total bytes of audio data transferred.
    pub bytes_transferred: u64,
    /// Whether the stream uses exclusive mode.
    pub exclusive: bool,
    /// Whether this is a loopback capture stream.
    pub loopback: bool,
}

impl Default for AudioStream {
    fn default() -> Self {
        Self {
            id: String::new(),
            pid: 0,
            process_name: String::new(),
            direction: StreamDirection::Output,
            device_id: String::new(),
            device_name: String::new(),
            state: StreamState::Inactive,
            format: StreamFormat::Pcm,
            sample_rate_hz: 48000,
            bit_depth: 16,
            channels: 2,
            volume_percent: 100.0,
            muted: false,
            peak_level: -100.0,
            duration_secs: 0,
            latency_ms: 0.0,
            underruns: 0,
            overruns: 0,
            bytes_transferred: 0,
            exclusive: false,
            loopback: false,
        }
    }
}
