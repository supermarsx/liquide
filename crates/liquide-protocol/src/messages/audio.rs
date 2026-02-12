//! Audio channel message types.
//!
//! These messages are used on the Audio Playback (0x20) and Audio Capture
//! (0x21) channels.  They cover format negotiation, encoded audio data
//! transfer, and mute/volume control.
//!
//! All structs are CBOR-serializable via `ciborium` and use the standard
//! Liquide derive set (`Serialize`, `Deserialize`, `Debug`, `Clone`,
//! `PartialEq`).

use serde::{Deserialize, Serialize};

/// Audio format negotiation.
///
/// Sent at channel open to agree on the audio encoding parameters.
/// The server proposes; the client may counter-propose.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioConfigMsg {
    /// Sample rate in Hz (e.g., 44100, 48000).
    pub sample_rate: u32,
    /// Number of audio channels (1 = mono, 2 = stereo).
    pub channels: u32,
    /// Codec identifier string (e.g., `"opus"`, `"aac"`, `"pcm"`).
    pub codec: String,
    /// Bits per sample (16, 24, or 32).
    pub bits_per_sample: u32,
    /// Target bitrate in kbit/s.  Omitted when not applicable (e.g., PCM).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bitrate_kbps: Option<u32>,
}

/// Encoded audio frame.
///
/// Carries a single encoded audio buffer from sender to receiver.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioDataMsg {
    /// Presentation timestamp in microseconds since the stream epoch.
    pub timestamp_us: u64,
    /// The encoded audio payload.
    pub data: Vec<u8>,
    /// Duration of this audio frame in microseconds.
    pub duration_us: u64,
    /// Monotonically increasing sequence number for loss detection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sequence: Option<u32>,
}

/// Mute/unmute notification.
///
/// Either side may send this to mute or unmute the audio stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioMuteMsg {
    /// `true` = muted, `false` = unmuted.
    pub muted: bool,
}

/// Volume level change.
///
/// Sets the playback/capture volume on the remote side.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioVolumeMsg {
    /// Volume level in the range `0.0` (silent) to `1.0` (full).
    pub volume: f32,
}
