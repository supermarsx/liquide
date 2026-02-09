//! Compression wrappers for Zstd and LZ4.
//!
//! Tile payloads are compressed before transmission. The encoder chooses
//! between Zstd (better ratio) and LZ4 (faster) based on the tile's
//! damage class and the current quality profile.

/// Compress data using Zstd at the given compression level (1–22).
pub fn compress_zstd(data: &[u8], level: i32) -> crate::Result<Vec<u8>> {
    zstd::bulk::compress(data, level)
        .map_err(|e| crate::EncoderError::CompressionFailed(e.to_string()))
}

/// Decompress Zstd-compressed data.
pub fn decompress_zstd(data: &[u8], max_size: usize) -> crate::Result<Vec<u8>> {
    zstd::bulk::decompress(data, max_size)
        .map_err(|e| crate::EncoderError::DecompressionFailed(e.to_string()))
}

/// Compress data using LZ4 (block mode, fast).
#[must_use]
pub fn compress_lz4(data: &[u8]) -> Vec<u8> {
    lz4_flex::compress_prepend_size(data)
}

/// Decompress LZ4-compressed data.
pub fn decompress_lz4(data: &[u8]) -> crate::Result<Vec<u8>> {
    lz4_flex::decompress_size_prepended(data)
        .map_err(|e| crate::EncoderError::DecompressionFailed(e.to_string()))
}
