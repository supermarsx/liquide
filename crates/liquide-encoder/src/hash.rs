//! CRC-32C (Castagnoli) hashing for tile content.
//!
//! Uses a table-based implementation. SIMD acceleration via SSE4.2
//! `crc32` instruction is deferred (see `// TODO: SSE4.2`).

/// Castagnoli polynomial used by CRC-32C.
const POLYNOMIAL: u32 = 0x82F6_3B78;

/// Precomputed CRC-32C lookup table (256 entries).
const CRC32C_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut i = 0u32;
    while i < 256 {
        let mut crc = i;
        let mut j = 0;
        while j < 8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ POLYNOMIAL;
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i as usize] = crc;
        i += 1;
    }
    table
};

/// Compute CRC-32C of a byte slice.
// TODO: SSE4.2 `_mm_crc32_u64` fast path
#[must_use]
pub fn crc32c(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        let index = ((crc ^ byte as u32) & 0xFF) as usize;
        crc = (crc >> 8) ^ CRC32C_TABLE[index];
    }
    crc ^ 0xFFFF_FFFF
}

/// Compute CRC-32C for a tile region within a larger pixel buffer.
///
/// Extracts `tile_size × tile_size × bpp` bytes from the buffer at
/// tile coordinates `(tx, ty)` and hashes them.
#[must_use]
pub fn crc32c_tile(
    pixels: &[u8],
    stride: u32,
    tile_x: u32,
    tile_y: u32,
    tile_size: u32,
    fb_width: u32,
    fb_height: u32,
    bpp: u32,
) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    let px_x = tile_x * tile_size;
    let px_y = tile_y * tile_size;

    let row_end = (px_y + tile_size).min(fb_height);
    let col_bytes = ((px_x + tile_size).min(fb_width) - px_x) as usize * bpp as usize;

    for row in px_y..row_end {
        let row_off = (row * stride) as usize + px_x as usize * bpp as usize;
        for &byte in &pixels[row_off..row_off + col_bytes] {
            let index = ((crc ^ byte as u32) & 0xFF) as usize;
            crc = (crc >> 8) ^ CRC32C_TABLE[index];
        }
    }
    crc ^ 0xFFFF_FFFF
}
