//! CRC-32C (Castagnoli) hashing for tile content.
//!
//! Delegates to SIMD-accelerated implementations in `liquide-simd`.

/// Compute CRC-32C of a byte slice.
#[must_use]
pub fn crc32c(data: &[u8]) -> u32 {
    liquide_simd::crc::crc32c(data)
}

/// Parameters describing a tile region within a pixel buffer.
pub struct TileRegion {
    pub tile_x: u32,
    pub tile_y: u32,
    pub tile_size: u32,
    pub fb_width: u32,
    pub fb_height: u32,
    pub bpp: u32,
}

/// Compute CRC-32C for a tile region within a larger pixel buffer.
///
/// Extracts `tile_size × tile_size × bpp` bytes from the buffer at
/// tile coordinates `(tx, ty)` and hashes them.
#[must_use]
pub fn crc32c_tile(pixels: &[u8], stride: u32, region: &TileRegion) -> u32 {
    liquide_simd::crc::crc32c_tile(
        pixels,
        stride,
        region.tile_x,
        region.tile_y,
        region.tile_size,
        region.fb_width,
        region.fb_height,
        region.bpp,
    )
}
