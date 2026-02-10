//! Audio codec abstractions — encode/decode traits and built-in implementations.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{AudioError, Result};

/// Identifies a codec implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AudioCodecId {
    /// Uncompressed PCM passthrough.
    Pcm,
    /// Opus compressed audio.
    Opus,
}

impl fmt::Display for AudioCodecId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pcm => write!(f, "PCM"),
            Self::Opus => write!(f, "Opus"),
        }
    }
}

/// Trait for audio codecs that can encode and decode byte buffers.
pub trait AudioCodec: Send {
    /// The codec identifier.
    fn id(&self) -> AudioCodecId;

    /// Encode raw PCM bytes into the codec's output format.
    fn encode(&mut self, input: &[u8]) -> Result<Vec<u8>>;

    /// Decode codec-format bytes back into raw PCM.
    fn decode(&mut self, input: &[u8]) -> Result<Vec<u8>>;

    /// Flush any buffered data and return remaining encoded bytes.
    fn flush(&mut self) -> Result<Vec<u8>>;
}

/// Passthrough PCM codec — returns input unchanged.
pub struct PcmCodec;

impl PcmCodec {
    /// Create a new PCM passthrough codec.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for PcmCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioCodec for PcmCodec {
    fn id(&self) -> AudioCodecId {
        AudioCodecId::Pcm
    }

    fn encode(&mut self, input: &[u8]) -> Result<Vec<u8>> {
        Ok(input.to_vec())
    }

    fn decode(&mut self, input: &[u8]) -> Result<Vec<u8>> {
        Ok(input.to_vec())
    }

    fn flush(&mut self) -> Result<Vec<u8>> {
        Ok(Vec::new())
    }
}

impl fmt::Display for PcmCodec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PcmCodec")
    }
}

/// Placeholder Opus codec — always returns a codec error.
pub struct OpusPlaceholder;

impl OpusPlaceholder {
    /// Create a new Opus placeholder.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for OpusPlaceholder {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioCodec for OpusPlaceholder {
    fn id(&self) -> AudioCodecId {
        AudioCodecId::Opus
    }

    fn encode(&mut self, _input: &[u8]) -> Result<Vec<u8>> {
        Err(AudioError::CodecError("opus not available".to_string()))
    }

    fn decode(&mut self, _input: &[u8]) -> Result<Vec<u8>> {
        Err(AudioError::CodecError("opus not available".to_string()))
    }

    fn flush(&mut self) -> Result<Vec<u8>> {
        Err(AudioError::CodecError("opus not available".to_string()))
    }
}

impl fmt::Display for OpusPlaceholder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OpusPlaceholder")
    }
}
