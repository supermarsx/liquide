//! Tile-based encoding pipeline for the LiquiDE remote desktop protocol.
//!
//! Provides CRC-32C hashing, XOR delta encoding, Zstd/LZ4 compression,
//! and a tile payload cache with eviction for efficient frame transmission.

pub mod cache;
pub mod compress;
pub mod delta;
pub mod encoder;
pub mod hash;
pub mod header;
pub mod strategy;
pub mod tile;

use thiserror::Error;

/// Errors produced by the encoding pipeline.
#[derive(Debug, Error)]
pub enum EncoderError {
    /// Tile dimensions do not match the expected size.
    #[error("tile dimensions mismatch: expected {expected}, got {got}")]
    TileMismatch { expected: u32, got: u32 },

    /// Compression failed.
    #[error("compression failed: {0}")]
    CompressionFailed(String),

    /// Decompression failed.
    #[error("decompression failed: {0}")]
    DecompressionFailed(String),

    /// Invalid tile header.
    #[error("invalid tile header")]
    InvalidHeader,

    /// Cache is full.
    #[error("cache is full ({size} entries)")]
    CacheFull { size: usize },

    /// Internal error.
    #[error("internal error: {0}")]
    Internal(String),
}

/// Result type for the encoding pipeline.
pub type Result<T> = std::result::Result<T, EncoderError>;

// Re-exports
pub use cache::TilePayloadCache;
pub use compress::{compress_lz4, compress_zstd, decompress_lz4, decompress_zstd};
pub use delta::{xor_apply, xor_delta};
pub use encoder::TileEncoder;
pub use hash::crc32c;
pub use header::CompressedTileHeader;
pub use strategy::{choose_strategy, EncodingStrategy, StrategyConfig};
pub use tile::{TileBatch, TileCodec, TileConfig, TileEncoding, TileGrid, TileUpdate};

#[cfg(test)]
mod tests;
