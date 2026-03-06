//! Hardware-accelerated CRC-32C (Castagnoli) using SSE4.2 intrinsics.
//!
//! Falls back to a table-based implementation when SSE4.2 is unavailable.
//! When PCLMULQDQ is available, uses carry-less multiplication for 4-way
//! parallel folding at 64+ bytes per cycle. Otherwise the SSE4.2 path
//! processes 8 bytes per cycle via `_mm_crc32_u64`.

/// Castagnoli polynomial.
const POLYNOMIAL: u32 = 0x82F6_3B78;

/// Precomputed CRC-32C lookup table.
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
///
/// Dispatches to PCLMULQDQ folding (fastest, 64+ bytes/cycle), then SSE4.2
/// hardware CRC (8 bytes/cycle), otherwise uses a table-based implementation.
#[must_use]
pub fn crc32c(data: &[u8]) -> u32 {
    #[cfg(target_arch = "x86_64")]
    {
        if crate::detect::has_pclmulqdq() && crate::detect::has_sse42() {
            // SAFETY: PCLMULQDQ and SSE4.2 detected at runtime.
            unsafe { return crc32c_pclmul(data) }
        }
        if crate::detect::has_sse42() {
            // SAFETY: SSE4.2 detected at runtime.
            unsafe { return crc32c_sse42(data) }
        }
    }
    crc32c_table(data)
}

/// Table-based CRC-32C (scalar fallback).
#[must_use]
pub fn crc32c_table(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in data {
        crc = CRC32C_TABLE[((crc ^ byte as u32) & 0xFF) as usize] ^ (crc >> 8);
    }
    !crc
}

// ---------------------------------------------------------------------------
// PCLMULQDQ 4-way folding for CRC-32C
// ---------------------------------------------------------------------------
//
// Implements the Intel "Fast CRC Computation Using PCLMULQDQ" algorithm,
// following the zlib-ng structure with CRC-32C (Castagnoli) constants.
//
// Folding constants are `bit_reverse(x^n mod P(x), 33)` where P(x) is the
// CRC-32C polynomial 0x11EDC6F41 (normal form with explicit x^32 term).
//
// The fold helper uses CLMUL selectors 0x01 and 0x10 (cross-multiply),
// matching the zlib-ng convention where:
//   lo64(k) = constant for high64(accumulator)
//   hi64(k) = constant for low64(accumulator)

/// PCLMULQDQ-accelerated CRC-32C using 4-way folding.
///
/// Processes 64 bytes per iteration using carry-less multiplication to fold
/// four 128-bit accumulators in parallel.  Falls back to SSE4.2 for inputs
/// smaller than 64 bytes and for any trailing bytes.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "pclmulqdq,sse4.1,sse4.2")]
unsafe fn crc32c_pclmul(data: &[u8]) -> u32 {
    use std::arch::x86_64::*;

    let len = data.len();
    if len < 64 {
        return crc32c_sse42(data);
    }

    let mut ptr = data.as_ptr();
    let mut remaining = len;

    // ---------------------------------------------------------------
    // Folding constants (for 0x01/0x10 selectors, zlib-ng convention)
    // ---------------------------------------------------------------

    // fold4: 4x128-bit stride (512-bit fold)
    //   lo64 = bit_rev33(x^480 mod P) = 0x09e4addf8
    //   hi64 = bit_rev33(x^544 mod P) = 0x0740eef02
    let fold4 = _mm_set_epi32(
        0x00000000u32 as i32,
        0x740eef02u32 as i32,
        0x00000000u32 as i32,
        0x9e4addf8u32 as i32,
    );

    // fold1: 128-bit stride (single block fold)
    //   lo64 = bit_rev33(x^96  mod P) = 0x14cd00bd6
    //   hi64 = bit_rev33(x^160 mod P) = 0x0f20c0dfe
    let fold1 = _mm_set_epi32(
        0x00000000u32 as i32,
        0xf20c0dfeu32 as i32,
        0x00000001u32 as i32,
        0x4cd00bd6u32 as i32,
    );

    // Barrett reduction constants:
    //   lo64 = bit_rev64(floor(x^95 / P)) = 0x4869ec38dea713f1
    //   hi64 = bit_rev33(P) & ~1          = 0x105ec76f0
    let barrett = _mm_set_epi32(
        0x00000001u32 as i32,
        0x05ec76f0u32 as i32,
        0x4869ec38u32 as i32,
        0xdea713f1u32 as i32,
    );

    // Load initial 4 x 128-bit blocks (64 bytes).
    let mut xmm_crc0 = _mm_loadu_si128(ptr as *const __m128i);
    let mut xmm_crc1 = _mm_loadu_si128(ptr.add(16) as *const __m128i);
    let mut xmm_crc2 = _mm_loadu_si128(ptr.add(32) as *const __m128i);
    let mut xmm_crc3 = _mm_loadu_si128(ptr.add(48) as *const __m128i);

    // XOR initial CRC (~0) into the first accumulator.
    xmm_crc0 = _mm_xor_si128(xmm_crc0, _mm_cvtsi32_si128(!0i32));

    ptr = ptr.add(64);
    remaining -= 64;

    // Process 64-byte chunks: fold each accumulator forward by 512 bits.
    while remaining >= 64 {
        let d0 = _mm_loadu_si128(ptr as *const __m128i);
        let d1 = _mm_loadu_si128(ptr.add(16) as *const __m128i);
        let d2 = _mm_loadu_si128(ptr.add(32) as *const __m128i);
        let d3 = _mm_loadu_si128(ptr.add(48) as *const __m128i);

        xmm_crc0 = pclmul_fold4(xmm_crc0, d0, fold4);
        xmm_crc1 = pclmul_fold4(xmm_crc1, d1, fold4);
        xmm_crc2 = pclmul_fold4(xmm_crc2, d2, fold4);
        xmm_crc3 = pclmul_fold4(xmm_crc3, d3, fold4);

        ptr = ptr.add(64);
        remaining -= 64;
    }

    // Combine 4 accumulators into xmm_crc3 using 128-bit fold constants.
    // fold(crc0) -> crc1 -> crc2 -> crc3
    let lo0 = _mm_clmulepi64_si128::<0x01>(xmm_crc0, fold1);
    let hi0 = _mm_clmulepi64_si128::<0x10>(xmm_crc0, fold1);
    xmm_crc1 = _mm_xor_si128(_mm_xor_si128(xmm_crc1, lo0), hi0);

    let lo1 = _mm_clmulepi64_si128::<0x01>(xmm_crc1, fold1);
    let hi1 = _mm_clmulepi64_si128::<0x10>(xmm_crc1, fold1);
    xmm_crc2 = _mm_xor_si128(_mm_xor_si128(xmm_crc2, lo1), hi1);

    let lo2 = _mm_clmulepi64_si128::<0x01>(xmm_crc2, fold1);
    let hi2 = _mm_clmulepi64_si128::<0x10>(xmm_crc2, fold1);
    xmm_crc3 = _mm_xor_si128(_mm_xor_si128(xmm_crc3, lo2), hi2);

    // Process remaining 16-byte blocks.
    while remaining >= 16 {
        let d = _mm_loadu_si128(ptr as *const __m128i);
        xmm_crc3 = pclmul_fold4(xmm_crc3, d, fold1);
        ptr = ptr.add(16);
        remaining -= 16;
    }

    // Barrett reduction: 128 bits -> 32 bits.
    // Following the zlib-ng two-stage Barrett reduction exactly.
    let x_tmp0 = _mm_clmulepi64_si128::<0x00>(xmm_crc3, barrett);
    let x_tmp1 = _mm_clmulepi64_si128::<0x10>(x_tmp0, barrett);

    // Keep only bits [64:95] of x_tmp1 (mask = 0xcf zeroes words 0-3 and 6-7).
    let x_tmp1 = _mm_blend_epi16::<0xcf>(x_tmp1, _mm_setzero_si128());
    let x_tmp0 = _mm_xor_si128(x_tmp1, xmm_crc3);

    let x_res_a = _mm_clmulepi64_si128::<0x01>(x_tmp0, barrett);
    let x_res_b = _mm_clmulepi64_si128::<0x10>(x_res_a, barrett);

    let crc = _mm_extract_epi32::<2>(x_res_b) as u32;

    // Process tail bytes (< 16) with SSE4.2.
    if remaining > 0 {
        let tail = std::slice::from_raw_parts(ptr, remaining);
        let mut c = crc;
        for &byte in tail {
            c = _mm_crc32_u8(c, byte);
        }
        return !c;
    }

    !crc
}

/// Fold one 128-bit block: `clmul_01(a, k) ^ clmul_10(a, k) ^ d`.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "pclmulqdq")]
unsafe fn pclmul_fold4(
    a: std::arch::x86_64::__m128i,
    d: std::arch::x86_64::__m128i,
    k: std::arch::x86_64::__m128i,
) -> std::arch::x86_64::__m128i {
    use std::arch::x86_64::*;
    let lo = _mm_clmulepi64_si128::<0x01>(a, k);
    let hi = _mm_clmulepi64_si128::<0x10>(a, k);
    _mm_xor_si128(_mm_xor_si128(lo, hi), d)
}

/// SSE4.2 hardware CRC-32C.
///
/// Processes 8 bytes at a time via `_mm_crc32_u64`, with a byte-level tail.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse4.2")]
unsafe fn crc32c_sse42(data: &[u8]) -> u32 {
    use std::arch::x86_64::*;

    let mut crc = !0u32;
    let len = data.len();
    let mut offset = 0;

    // Process 8 bytes at a time
    let chunks = len / 8;
    for _ in 0..chunks {
        let ptr = data.as_ptr().add(offset) as *const u64;
        let val = ptr.read_unaligned();
        crc = _mm_crc32_u64(crc as u64, val) as u32;
        offset += 8;
    }

    // Process remaining bytes one at a time
    for &byte in &data[offset..] {
        crc = _mm_crc32_u8(crc, byte);
    }

    !crc
}

/// Compute CRC-32C for a tile region within a pixel buffer.
///
/// Hashes `tile_size x tile_size` pixels at tile coordinates `(tile_x, tile_y)`.
/// The buffer has `stride` bytes per row.
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
    let px_x = tile_x * tile_size;
    let px_y = tile_y * tile_size;

    if px_x >= fb_width || px_y >= fb_height {
        return 0;
    }

    let tw = tile_size.min(fb_width - px_x);
    let th = tile_size.min(fb_height - px_y);

    #[cfg(target_arch = "x86_64")]
    {
        if crate::detect::has_pclmulqdq() && crate::detect::has_sse42() {
            unsafe {
                return crc32c_tile_pclmul(pixels, stride, px_x, px_y, tw, th, bpp);
            }
        }
        if crate::detect::has_sse42() {
            unsafe {
                return crc32c_tile_sse42(pixels, stride, px_x, px_y, tw, th, bpp);
            }
        }
    }

    crc32c_tile_table(pixels, stride, px_x, px_y, tw, th, bpp)
}

/// PCLMULQDQ-accelerated tile CRC with unrolled SSE4.2 inner loop.
///
/// For tile hashing, rows are typically short (64-256 bytes), so the main
/// benefit is the 3x unrolled SSE4.2 loop that reduces branch overhead.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "pclmulqdq,sse4.1,sse4.2")]
unsafe fn crc32c_tile_pclmul(
    pixels: &[u8],
    stride: u32,
    px_x: u32,
    px_y: u32,
    tw: u32,
    th: u32,
    bpp: u32,
) -> u32 {
    use std::arch::x86_64::*;

    let mut crc = !0u32;
    let col_bytes = tw as usize * bpp as usize;

    for row in 0..th {
        let row_off = ((px_y + row) * stride) as usize + px_x as usize * bpp as usize;
        let end = row_off + col_bytes;
        if end > pixels.len() {
            continue;
        }

        let row_data = &pixels[row_off..end];
        let mut offset = 0;

        // 3x unrolled 8-byte loop for reduced branch overhead
        while offset + 24 <= row_data.len() {
            let p = row_data.as_ptr().add(offset);
            crc = _mm_crc32_u64(crc as u64, (p as *const u64).read_unaligned()) as u32;
            crc =
                _mm_crc32_u64(crc as u64, (p.add(8) as *const u64).read_unaligned()) as u32;
            crc =
                _mm_crc32_u64(crc as u64, (p.add(16) as *const u64).read_unaligned()) as u32;
            offset += 24;
        }

        // Remaining 8-byte chunks
        while offset + 8 <= row_data.len() {
            let val = (row_data.as_ptr().add(offset) as *const u64).read_unaligned();
            crc = _mm_crc32_u64(crc as u64, val) as u32;
            offset += 8;
        }

        // Remaining bytes
        for &byte in &row_data[offset..] {
            crc = _mm_crc32_u8(crc, byte);
        }
    }
    !crc
}

fn crc32c_tile_table(
    pixels: &[u8],
    stride: u32,
    px_x: u32,
    px_y: u32,
    tw: u32,
    th: u32,
    bpp: u32,
) -> u32 {
    let mut crc = !0u32;
    let col_bytes = tw as usize * bpp as usize;

    for row in 0..th {
        let row_off = ((px_y + row) * stride) as usize + px_x as usize * bpp as usize;
        let end = row_off + col_bytes;
        if end > pixels.len() {
            continue;
        }
        for &byte in &pixels[row_off..end] {
            crc = CRC32C_TABLE[((crc ^ byte as u32) & 0xFF) as usize] ^ (crc >> 8);
        }
    }
    !crc
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse4.2")]
unsafe fn crc32c_tile_sse42(
    pixels: &[u8],
    stride: u32,
    px_x: u32,
    px_y: u32,
    tw: u32,
    th: u32,
    bpp: u32,
) -> u32 {
    use std::arch::x86_64::*;

    let mut crc = !0u32;
    let col_bytes = tw as usize * bpp as usize;

    for row in 0..th {
        let row_off = ((px_y + row) * stride) as usize + px_x as usize * bpp as usize;
        let end = row_off + col_bytes;
        if end > pixels.len() {
            continue;
        }

        let row_data = &pixels[row_off..end];
        let mut offset = 0;

        // 8-byte chunks
        let chunks = row_data.len() / 8;
        for _ in 0..chunks {
            let ptr = row_data.as_ptr().add(offset) as *const u64;
            crc = _mm_crc32_u64(crc as u64, ptr.read_unaligned()) as u32;
            offset += 8;
        }

        // Remaining bytes
        for &byte in &row_data[offset..] {
            crc = _mm_crc32_u8(crc, byte);
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_data() {
        assert_eq!(crc32c(&[]), crc32c_table(&[]));
    }

    #[test]
    fn known_values() {
        // CRC-32C of single zero byte
        let result = crc32c(&[0]);
        let expected = crc32c_table(&[0]);
        assert_eq!(result, expected);
    }

    #[test]
    fn consistency_across_sizes() {
        for size in [1, 2, 3, 4, 7, 8, 15, 16, 31, 32, 63, 64, 100, 1024] {
            let data: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
            let hw = crc32c(&data);
            let sw = crc32c_table(&data);
            assert_eq!(hw, sw, "mismatch at size {size}");
        }
    }

    #[test]
    fn tile_crc_out_of_bounds() {
        let pixels = vec![0u8; 64 * 64 * 4];
        let result = crc32c_tile(&pixels, 64 * 4, 100, 100, 16, 64, 64, 4);
        assert_eq!(result, 0);
    }

    #[test]
    fn tile_crc_matches_flat() {
        // 4x4 image, single tile = entire image
        let pixels: Vec<u8> = (0..64).map(|i| i as u8).collect();
        let tile = crc32c_tile(&pixels, 16, 0, 0, 4, 4, 4, 4);
        let flat = crc32c(&pixels);
        assert_eq!(tile, flat);
    }

    #[test]
    fn tile_crc_sub_region() {
        // 8x8 image, tile at (1,1) of size 4 should hash a sub-region
        let stride = 8 * 4u32;
        let pixels: Vec<u8> = (0..(8 * 8 * 4)).map(|i| (i % 256) as u8).collect();
        let result = crc32c_tile(&pixels, stride, 1, 1, 4, 8, 8, 4);
        assert_ne!(result, 0);

        // Manually extract the same region and hash it
        let mut region_data = Vec::new();
        for row in 0..4u32 {
            let off = ((4 + row) * stride + 4 * 4) as usize;
            region_data.extend_from_slice(&pixels[off..off + 16]);
        }
        // tile_crc hashes row by row, so the result should match
        // if we hash the concatenated rows
        let manual = crc32c(&region_data);
        assert_eq!(result, manual);
    }
}
