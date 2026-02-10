//! Recording container format — header, state, and chapter markers.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// Magic bytes for the recording format.
pub const RECORDING_MAGIC: [u8; 4] = *b"LQR\x01";

/// Current format version.
pub const RECORDING_VERSION: u32 = 1;

/// Recording file header with session metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingHeader {
    /// Magic bytes identifying the format.
    pub magic: [u8; 4],
    /// Format version.
    pub version: u32,
    /// Creation timestamp in microseconds.
    pub created_us: u64,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Tile size in pixels.
    pub tile_size: u32,
    /// Pixel format identifier.
    pub pixel_format: String,
    /// Audio format identifier, if audio is enabled.
    pub audio_format: Option<String>,
    /// Arbitrary key-value metadata.
    pub metadata: HashMap<String, String>,
}

impl RecordingHeader {
    /// Create a new header with defaults.
    #[must_use]
    pub fn new(width: u32, height: u32, tile_size: u32, pixel_format: &str) -> Self {
        Self {
            magic: RECORDING_MAGIC,
            version: RECORDING_VERSION,
            created_us: 0,
            width,
            height,
            tile_size,
            pixel_format: pixel_format.to_string(),
            audio_format: None,
            metadata: HashMap::new(),
        }
    }

    /// Check if the magic bytes are valid.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.magic == RECORDING_MAGIC
    }

    /// Serialized size estimate for the header (approximate).
    #[must_use]
    pub fn estimated_size(&self) -> usize {
        4 + 4 + 8 + 4 + 4 + 4
            + self.pixel_format.len()
            + self.audio_format.as_ref().map_or(0, |s| s.len())
    }
}

impl std::fmt::Display for RecordingHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "RecordingHeader({}x{}, tile={}, fmt={})",
            self.width, self.height, self.tile_size, self.pixel_format
        )
    }
}

/// Recording lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecordingState {
    /// Not yet started.
    Idle,
    /// Actively recording.
    Recording,
    /// Temporarily paused.
    Paused,
    /// Recording finished.
    Stopped,
}

impl std::fmt::Display for RecordingState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Recording => write!(f, "Recording"),
            Self::Paused => write!(f, "Paused"),
            Self::Stopped => write!(f, "Stopped"),
        }
    }
}

/// A chapter marker within a recording.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChapterMark {
    /// Timestamp in microseconds from recording start.
    pub timestamp_us: u64,
    /// Human-readable label.
    pub label: String,
}

impl ChapterMark {
    /// Create a new chapter mark.
    #[must_use]
    pub fn new(timestamp_us: u64, label: &str) -> Self {
        Self {
            timestamp_us,
            label: label.to_string(),
        }
    }
}

impl std::fmt::Display for ChapterMark {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Chapter({}: {})", self.timestamp_us, self.label)
    }
}
