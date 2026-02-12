//! Compression and decompression for protocol payloads.
//!
//! Supports LZ4 (fast, moderate ratio) and Zstd (good ratio, configurable level).

use std::io::Cursor;

use crate::ProtocolError;

/// Compression algorithm identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum CompressionAlgorithm {
    None = 0x00,
    Lz4 = 0x01,
    Zstd = 0x02,
}

impl CompressionAlgorithm {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::None),
            1 => Some(Self::Lz4),
            2 => Some(Self::Zstd),
            _ => None,
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "none" => Some(Self::None),
            "lz4" => Some(Self::Lz4),
            "zstd" => Some(Self::Zstd),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Lz4 => "lz4",
            Self::Zstd => "zstd",
        }
    }
}

/// Compress data using the specified algorithm.
pub fn compress(
    data: &[u8],
    algorithm: CompressionAlgorithm,
    level: Option<i32>,
) -> crate::Result<Vec<u8>> {
    match algorithm {
        CompressionAlgorithm::None => Ok(data.to_vec()),
        CompressionAlgorithm::Lz4 => Ok(lz4_flex::compress_prepend_size(data)),
        CompressionAlgorithm::Zstd => {
            let level = level.unwrap_or(3);
            zstd::encode_all(Cursor::new(data), level)
                .map_err(|e| ProtocolError::Compression(e.to_string()))
        }
    }
}

/// Decompress data using the specified algorithm.
pub fn decompress(data: &[u8], algorithm: CompressionAlgorithm) -> crate::Result<Vec<u8>> {
    match algorithm {
        CompressionAlgorithm::None => Ok(data.to_vec()),
        CompressionAlgorithm::Lz4 => lz4_flex::decompress_size_prepended(data)
            .map_err(|e| ProtocolError::Compression(e.to_string())),
        CompressionAlgorithm::Zstd => zstd::decode_all(Cursor::new(data))
            .map_err(|e| ProtocolError::Compression(e.to_string())),
    }
}

/// Minimum payload size before compression is applied (bytes).
pub const MIN_COMPRESS_SIZE: usize = 64;

/// Determine the recommended compression algorithm for a channel.
pub fn channel_compression(channel: crate::channel::ChannelId) -> CompressionAlgorithm {
    match channel.as_u16() {
        0x00 => CompressionAlgorithm::Lz4,  // Control
        0x01 => CompressionAlgorithm::Lz4,  // Emergency
        0x10 => CompressionAlgorithm::None,  // Video (already compressed)
        0x11 => CompressionAlgorithm::None,  // Cursor (small)
        0x12 => CompressionAlgorithm::Zstd,  // Tile
        0x20 | 0x21 => CompressionAlgorithm::None, // Audio
        0x30 => CompressionAlgorithm::Lz4,  // Clipboard
        0x31 => CompressionAlgorithm::Zstd,  // File transfer
        0x50 => CompressionAlgorithm::None,  // Input (tiny, latency-critical)
        _ => CompressionAlgorithm::None,
    }
}

