//! Typed recording segments — video, audio, input events, and metadata.

use serde::{Deserialize, Serialize};

/// The kind of a recording segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SegmentKind {
    /// Video tile data.
    Video,
    /// Audio sample data.
    Audio,
    /// Input event data.
    InputEvent,
    /// Key-value metadata.
    Metadata,
    /// Chapter marker.
    Chapter,
}

impl std::fmt::Display for SegmentKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Video => write!(f, "Video"),
            Self::Audio => write!(f, "Audio"),
            Self::InputEvent => write!(f, "InputEvent"),
            Self::Metadata => write!(f, "Metadata"),
            Self::Chapter => write!(f, "Chapter"),
        }
    }
}

/// Header common to all segment types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentHeader {
    /// Segment kind.
    pub kind: SegmentKind,
    /// Timestamp in microseconds from recording start.
    pub timestamp_us: u64,
    /// Payload length in bytes.
    pub length: u32,
    /// Flags (reserved).
    pub flags: u8,
}

impl SegmentHeader {
    /// Create a new segment header.
    #[must_use]
    pub fn new(kind: SegmentKind, timestamp_us: u64, length: u32) -> Self {
        Self {
            kind,
            timestamp_us,
            length,
            flags: 0,
        }
    }

    /// Size of the header in bytes (fixed).
    #[must_use]
    pub fn header_size() -> usize {
        // kind(1) + timestamp(8) + length(4) + flags(1) = 14
        14
    }
}

impl std::fmt::Display for SegmentHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SegmentHeader({}, t={}us, len={})",
            self.kind, self.timestamp_us, self.length
        )
    }
}

/// A video segment containing encoded tile data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoSegment {
    /// Common header.
    pub header: SegmentHeader,
    /// Compressed tile data.
    pub tile_data: Vec<u8>,
    /// Number of tiles in this segment.
    pub tiles_encoded: u32,
}

impl VideoSegment {
    /// Create a new video segment.
    #[must_use]
    pub fn new(timestamp_us: u64, tile_data: Vec<u8>, tiles_encoded: u32) -> Self {
        let length = tile_data.len() as u32;
        Self {
            header: SegmentHeader::new(SegmentKind::Video, timestamp_us, length),
            tile_data,
            tiles_encoded,
        }
    }

    /// Total byte size of this segment.
    #[must_use]
    pub fn byte_size(&self) -> usize {
        SegmentHeader::header_size() + self.tile_data.len()
    }
}

/// An audio segment containing sample data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioSegment {
    /// Common header.
    pub header: SegmentHeader,
    /// Audio sample data.
    pub audio_data: Vec<u8>,
    /// Number of audio samples.
    pub sample_count: u32,
}

impl AudioSegment {
    /// Create a new audio segment.
    #[must_use]
    pub fn new(timestamp_us: u64, audio_data: Vec<u8>, sample_count: u32) -> Self {
        let length = audio_data.len() as u32;
        Self {
            header: SegmentHeader::new(SegmentKind::Audio, timestamp_us, length),
            audio_data,
            sample_count,
        }
    }

    /// Total byte size of this segment.
    #[must_use]
    pub fn byte_size(&self) -> usize {
        SegmentHeader::header_size() + self.audio_data.len()
    }
}

/// An input event segment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventSegment {
    /// Common header.
    pub header: SegmentHeader,
    /// Serialized event data.
    pub event_data: Vec<u8>,
}

impl EventSegment {
    /// Create a new event segment.
    #[must_use]
    pub fn new(timestamp_us: u64, event_data: Vec<u8>) -> Self {
        let length = event_data.len() as u32;
        Self {
            header: SegmentHeader::new(SegmentKind::InputEvent, timestamp_us, length),
            event_data,
        }
    }

    /// Total byte size of this segment.
    #[must_use]
    pub fn byte_size(&self) -> usize {
        SegmentHeader::header_size() + self.event_data.len()
    }
}

/// A metadata segment containing a key-value pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataSegment {
    /// Common header.
    pub header: SegmentHeader,
    /// Metadata key.
    pub key: String,
    /// Metadata value.
    pub value: String,
}

impl MetadataSegment {
    /// Create a new metadata segment.
    #[must_use]
    pub fn new(timestamp_us: u64, key: &str, value: &str) -> Self {
        let length = (key.len() + value.len()) as u32;
        Self {
            header: SegmentHeader::new(SegmentKind::Metadata, timestamp_us, length),
            key: key.to_string(),
            value: value.to_string(),
        }
    }

    /// Total byte size of this segment.
    #[must_use]
    pub fn byte_size(&self) -> usize {
        SegmentHeader::header_size() + self.key.len() + self.value.len()
    }
}
