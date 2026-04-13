#![doc = "Wire format, CBOR schemas, message types, channel IDs, and protocol"]
#![doc = "version constants for the Liquide protocol."]
#![doc = ""]
#![doc = "This crate is the single source of truth for everything that goes on the wire"]
#![doc = "between a Liquide server and its clients."]

pub mod channel;
pub mod codec;
pub mod compress;
pub mod fragment;
pub mod frame;
pub mod message;
pub mod messages;
pub mod state;
pub mod version;

// Re-exports for convenience.
pub use channel::ChannelId;
pub use frame::{FrameFlags, FrameHeader};
pub use message::MessageType;
pub use state::{ChannelEvent, ChannelState, SessionEvent, SessionState};
pub use version::{MAGIC, PROTOCOL_VERSION};

/// Protocol magic bytes identifying a Liquide stream (`"LD"` as little-endian u16).
pub const PROTOCOL_MAGIC: u16 = MAGIC;

/// Maximum size of a single frame payload in bytes (16 MiB).
pub const MAX_FRAME_PAYLOAD: u32 = 16 * 1024 * 1024;

/// Error types specific to protocol encoding / decoding.
#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    /// The magic bytes do not match.
    #[error("invalid magic: expected 0x{expected:04X}, got 0x{actual:04X}")]
    BadMagic { expected: u16, actual: u16 },

    /// The peer advertised an unsupported protocol version.
    #[error("unsupported protocol version: {0}")]
    UnsupportedVersion(String),

    /// A frame exceeded the maximum allowed payload size.
    #[error("frame payload too large: {size} bytes (max {max})")]
    PayloadTooLarge { size: u32, max: u32 },

    /// CBOR encoding or decoding failed.
    #[error("CBOR codec error: {0}")]
    Cbor(String),

    /// CRC-32C checksum mismatch.
    #[error("CRC mismatch: expected 0x{expected:08X}, got 0x{actual:08X}")]
    CrcMismatch { expected: u32, actual: u32 },

    /// Compression or decompression failed.
    #[error("compression error: {0}")]
    Compression(String),

    /// Not enough data to parse a complete structure.
    #[error("incomplete data: need {needed} bytes, have {available}")]
    Incomplete { needed: usize, available: usize },

    /// Generic I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Convenience result type for protocol operations.
pub type Result<T> = std::result::Result<T, ProtocolError>;

impl From<ProtocolError> for liquide_common::LiquideError {
    fn from(e: ProtocolError) -> Self {
        Self::Protocol(e.to_string())
    }
}
