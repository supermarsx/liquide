//! Audio capture, playback, codec, and metering for the LiquiDE remote desktop protocol.
//!
//! Provides audio format definitions, ring buffers, codec abstractions,
//! stream management, device enumeration, session handling, and level metering.

pub mod buffer;
pub mod codec;
pub mod device;
pub mod format;
pub mod meter;
pub mod session;
pub mod stream;

use thiserror::Error;

/// Errors produced by the audio subsystem.
#[derive(Debug, Error)]
pub enum AudioError {
    /// The requested audio device was not found.
    #[error("device not found: {name}")]
    DeviceNotFound { name: String },

    /// The requested audio format is not supported.
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),

    /// A buffer overflow occurred during a write operation.
    #[error("buffer overflow: wrote {written}, capacity {capacity}")]
    BufferOverflow { written: usize, capacity: usize },

    /// A buffer underrun occurred during a read operation.
    #[error("buffer underrun")]
    BufferUnderrun,

    /// A codec encode or decode error.
    #[error("codec error: {0}")]
    CodecError(String),

    /// The stream is not in an active state.
    #[error("stream not active")]
    StreamNotActive,

    /// A device-level error.
    #[error("device error: {0}")]
    DeviceError(String),

    /// An internal error.
    #[error("internal error: {0}")]
    Internal(String),
}

/// Result type for the audio subsystem.
pub type Result<T> = std::result::Result<T, AudioError>;

// Re-exports
pub use buffer::{AudioBuffer, AudioRingBuffer};
pub use codec::{AudioCodec, AudioCodecId, OpusPlaceholder, PcmCodec};
pub use device::{DeviceInfo, DeviceManager, NullDeviceManager};
pub use format::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
pub use meter::AudioMeter;
pub use session::{AudioSession, AudioSessionStats};
pub use stream::{AudioStream, MemoryStream, StreamConfig, StreamDirection, StreamState};

#[cfg(test)]
mod tests;
