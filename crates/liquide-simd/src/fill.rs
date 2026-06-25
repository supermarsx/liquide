//! SIMD-accelerated buffer fill operations.
//!
//! Fast pattern fills for BGRA8 pixel buffers.

/// Fill a buffer with a repeating 4-byte BGRA pattern.
///
/// Uses SSE2/AVX2 to broadcast the pattern across 16/32 bytes per store.
pub fn fill_pattern(buf: &mut [u8], pattern: [u8; 4]) {
    assert_eq!(buf.len() % 4, 0);

    #[cfg(target_arch = "x86_64")]
    {
        if crate::detect::has_avx512() {
            unsafe { return fill_pattern_avx512(buf, pattern) }
        }
        if crate::detect::has_avx2() {
            unsafe { return fill_pattern_avx2(buf, pattern) }
        }
        unsafe { return fill_pattern_sse2(buf, pattern) }
    }
    #[cfg(not(target_arch = "x86_64"))]
    fill_pattern_scalar(buf, pattern);
}

fn fill_pattern_scalar(buf: &mut [u8], pattern: [u8; 4]) {
    let pixels = buf.len() / 4;
    for i in 0..pixels {
        let off = i * 4;
        buf[off..off + 4].copy_from_slice(&pattern);
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn fill_pattern_sse2(buf: &mut [u8], pattern: [u8; 4]) {
    use std::arch::x86_64::*;

    let val = u32::from_le_bytes(pattern) as i32;
    let v = _mm_set1_epi32(val);

    let len = buf.len();
    let chunks = len / 16;
    let mut offset = 0;

    for _ in 0..chunks {
        _mm_storeu_si128(buf.as_mut_ptr().add(offset) as *mut __m128i, v);
        offset += 16;
    }

    // Scalar tail
    let remaining = (len - offset) / 4;
    for i in 0..remaining {
        let off = offset + i * 4;
        buf[off..off + 4].copy_from_slice(&pattern);
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f")]
unsafe fn fill_pattern_avx512(buf: &mut [u8], pattern: [u8; 4]) {
    use std::arch::x86_64::*;

    let val = u32::from_le_bytes(pattern) as i32;
    let v = _mm512_set1_epi32(val);

    let len = buf.len();
    let chunks = len / 64;
    let mut offset = 0;

    for _ in 0..chunks {
        _mm512_storeu_si512(buf.as_mut_ptr().add(offset) as *mut __m512i, v);
        offset += 64;
    }

    if offset < len {
        fill_pattern_avx2(&mut buf[offset..], pattern);
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn fill_pattern_avx2(buf: &mut [u8], pattern: [u8; 4]) {
    use std::arch::x86_64::*;

    let val = u32::from_le_bytes(pattern) as i32;
    let v = _mm256_set1_epi32(val);

    let len = buf.len();
    let chunks = len / 32;
    let mut offset = 0;

    for _ in 0..chunks {
        _mm256_storeu_si256(buf.as_mut_ptr().add(offset) as *mut __m256i, v);
        offset += 32;
    }

    if offset < len {
        fill_pattern_sse2(&mut buf[offset..], pattern);
    }
}

/// Fill a buffer with zeros (transparent black).
pub fn fill_zero(buf: &mut [u8]) {
    #[cfg(target_arch = "x86_64")]
    {
        if crate::detect::has_avx512() {
            unsafe { return fill_zero_avx512(buf) }
        }
        if crate::detect::has_avx2() {
            unsafe { return fill_zero_avx2(buf) }
        }
        unsafe { return fill_zero_sse2(buf) }
    }
    #[cfg(not(target_arch = "x86_64"))]
    buf.fill(0);
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn fill_zero_sse2(buf: &mut [u8]) {
    use std::arch::x86_64::*;

    let zero = _mm_setzero_si128();
    let len = buf.len();
    let chunks = len / 16;
    let mut offset = 0;

    for _ in 0..chunks {
        _mm_storeu_si128(buf.as_mut_ptr().add(offset) as *mut __m128i, zero);
        offset += 16;
    }

    for i in offset..len {
        buf[i] = 0;
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f")]
unsafe fn fill_zero_avx512(buf: &mut [u8]) {
    use std::arch::x86_64::*;

    let zero = _mm512_setzero_si512();
    let len = buf.len();
    let chunks = len / 64;
    let mut offset = 0;

    for _ in 0..chunks {
        _mm512_storeu_si512(buf.as_mut_ptr().add(offset) as *mut __m512i, zero);
        offset += 64;
    }

    if offset < len {
        fill_zero_avx2(&mut buf[offset..]);
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn fill_zero_avx2(buf: &mut [u8]) {
    use std::arch::x86_64::*;

    let zero = _mm256_setzero_si256();
    let len = buf.len();
    let chunks = len / 32;
    let mut offset = 0;

    for _ in 0..chunks {
        _mm256_storeu_si256(buf.as_mut_ptr().add(offset) as *mut __m256i, zero);
        offset += 32;
    }

    if offset < len {
        fill_zero_sse2(&mut buf[offset..]);
    }
}

/// Copy BGRA rows with different src/dst strides (2D strided memcpy).
///
/// Copies `height` rows of `row_bytes` from `src` (at `src_stride`) to
/// `dst` (at `dst_stride`).
pub fn copy_rows(
    dst: &mut [u8],
    dst_stride: usize,
    src: &[u8],
    src_stride: usize,
    row_bytes: usize,
    height: usize,
) {
    for row in 0..height {
        let src_off = row * src_stride;
        let dst_off = row * dst_stride;
        let src_end = src_off + row_bytes;
        let dst_end = dst_off + row_bytes;
        if src_end <= src.len() && dst_end <= dst.len() {
            dst[dst_off..dst_end].copy_from_slice(&src[src_off..src_end]);
        }
    }
}

/// Premultiply alpha for a BGRA8 scanline in-place.
///
/// `channel = channel * alpha / 255` for B, G, R. Alpha unchanged.
pub fn premultiply_alpha(buf: &mut [u8]) {
    assert_eq!(buf.len() % 4, 0);

    #[cfg(target_arch = "x86_64")]
    {
        if crate::detect::has_avx512() {
            unsafe { return premultiply_alpha_avx512(buf) }
        }
        if crate::detect::has_avx2() {
            unsafe { return premultiply_alpha_avx2(buf) }
        }
        unsafe { return premultiply_alpha_sse2(buf) }
    }
    #[cfg(not(target_arch = "x86_64"))]
    premultiply_alpha_scalar(buf);
}

pub fn premultiply_alpha_scalar(buf: &mut [u8]) {
    let pixels = buf.len() / 4;
    for i in 0..pixels {
        let off = i * 4;
        let a = buf[off + 3] as u16;
        if a == 255 {
            continue;
        }
        if a == 0 {
            buf[off] = 0;
            buf[off + 1] = 0;
            buf[off + 2] = 0;
            continue;
        }
        buf[off] = ((buf[off] as u16 * a + 127) / 255) as u8;
        buf[off + 1] = ((buf[off + 1] as u16 * a + 127) / 255) as u8;
        buf[off + 2] = ((buf[off + 2] as u16 * a + 127) / 255) as u8;
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn premultiply_alpha_sse2(buf: &mut [u8]) {
    use std::arch::x86_64::*;

    let zero = _mm_setzero_si128();
    let half = _mm_set1_epi16(128);

    let pixels = buf.len() / 4;
    // Process 4 pixels at a time (16 bytes)
    let chunks = pixels / 4;
    let mut offset = 0;

    for _ in 0..chunks {
        let ptr = buf.as_mut_ptr().add(offset) as *mut __m128i;
        let packed = _mm_loadu_si128(ptr);

        let lo = _mm_unpacklo_epi8(packed, zero);
        let hi = _mm_unpackhi_epi8(packed, zero);

        // Broadcast alpha for each pixel
        let a_lo = _mm_shufflelo_epi16::<0xFF>(lo);
        let a_lo = _mm_shufflehi_epi16::<0xFF>(a_lo);
        let a_hi = _mm_shufflelo_epi16::<0xFF>(hi);
        let a_hi = _mm_shufflehi_epi16::<0xFF>(a_hi);

        // channel * alpha / 255
        let prod_lo = _mm_mullo_epi16(lo, a_lo);
        let prod_hi = _mm_mullo_epi16(hi, a_hi);
        let biased_lo = _mm_add_epi16(prod_lo, half);
        let biased_hi = _mm_add_epi16(prod_hi, half);
        let mut result_lo =
            _mm_srli_epi16::<8>(_mm_add_epi16(biased_lo, _mm_srli_epi16::<8>(biased_lo)));
        let mut result_hi =
            _mm_srli_epi16::<8>(_mm_add_epi16(biased_hi, _mm_srli_epi16::<8>(biased_hi)));

        // Restore original alpha (alpha * alpha / 255 ≠ alpha)
        // Alpha is in lanes 3 and 7 of the u16 vectors
        // Use blend: take alpha lanes from original
        // SSE2 doesn't have blend, so use AND/OR mask
        let alpha_mask = _mm_set_epi16(-1, 0, 0, 0, -1, 0, 0, 0);
        let inv_mask = _mm_set_epi16(0, -1, -1, -1, 0, -1, -1, -1);
        result_lo = _mm_or_si128(
            _mm_and_si128(result_lo, inv_mask),
            _mm_and_si128(lo, alpha_mask),
        );
        result_hi = _mm_or_si128(
            _mm_and_si128(result_hi, inv_mask),
            _mm_and_si128(hi, alpha_mask),
        );

        let result = _mm_packus_epi16(result_lo, result_hi);
        _mm_storeu_si128(ptr, result);
        offset += 16;
    }

    // Scalar tail
    if offset / 4 < pixels {
        premultiply_alpha_scalar(&mut buf[offset..]);
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn premultiply_alpha_avx2(buf: &mut [u8]) {
    use std::arch::x86_64::*;

    let zero = _mm256_setzero_si256();
    let half = _mm256_set1_epi16(128);
    // Alpha lanes within each 128-bit half: lane 3 and lane 7 (matches SSE2 pattern,
    // _mm256 unpack/shuffle operate per 128-bit lane).
    let alpha_mask = _mm256_set_epi16(-1, 0, 0, 0, -1, 0, 0, 0, -1, 0, 0, 0, -1, 0, 0, 0);

    let pixels = buf.len() / 4;
    let chunks = pixels / 8; // 8 pixels = 32 bytes
    let mut offset = 0;

    for _ in 0..chunks {
        let ptr = buf.as_mut_ptr().add(offset) as *mut __m256i;
        let packed = _mm256_loadu_si256(ptr);

        let lo = _mm256_unpacklo_epi8(packed, zero);
        let hi = _mm256_unpackhi_epi8(packed, zero);

        // Broadcast alpha for each pixel across its 4 channels
        let a_lo = _mm256_shufflehi_epi16::<0xFF>(_mm256_shufflelo_epi16::<0xFF>(lo));
        let a_hi = _mm256_shufflehi_epi16::<0xFF>(_mm256_shufflelo_epi16::<0xFF>(hi));

        // channel * alpha / 255 (matches SSE2: (x+128 + ((x+128)>>8)) >> 8)
        let prod_lo = _mm256_mullo_epi16(lo, a_lo);
        let prod_hi = _mm256_mullo_epi16(hi, a_hi);
        let biased_lo = _mm256_add_epi16(prod_lo, half);
        let biased_hi = _mm256_add_epi16(prod_hi, half);
        let result_lo = _mm256_srli_epi16::<8>(_mm256_add_epi16(
            biased_lo,
            _mm256_srli_epi16::<8>(biased_lo),
        ));
        let result_hi = _mm256_srli_epi16::<8>(_mm256_add_epi16(
            biased_hi,
            _mm256_srli_epi16::<8>(biased_hi),
        ));

        // Restore original alpha lanes (alpha*alpha/255 != alpha)
        let result_lo = _mm256_or_si256(
            _mm256_andnot_si256(alpha_mask, result_lo),
            _mm256_and_si256(alpha_mask, lo),
        );
        let result_hi = _mm256_or_si256(
            _mm256_andnot_si256(alpha_mask, result_hi),
            _mm256_and_si256(alpha_mask, hi),
        );

        let result = _mm256_packus_epi16(result_lo, result_hi);
        _mm256_storeu_si256(ptr, result);
        offset += 32;
    }

    // Fall through to SSE2 for remaining pixels (4-pixel chunks + scalar tail)
    if offset / 4 < pixels {
        premultiply_alpha_sse2(&mut buf[offset..]);
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f,avx512bw")]
unsafe fn premultiply_alpha_avx512(buf: &mut [u8]) {
    use std::arch::x86_64::*;

    let zero = _mm512_setzero_si512();
    let half = _mm512_set1_epi16(128);
    // Alpha lanes repeat per 128-bit lane: lane 3 and 7 within each.
    #[rustfmt::skip]
    let alpha_mask = _mm512_set_epi16(
        -1, 0, 0, 0, -1, 0, 0, 0, -1, 0, 0, 0, -1, 0, 0, 0,
        -1, 0, 0, 0, -1, 0, 0, 0, -1, 0, 0, 0, -1, 0, 0, 0,
    );

    let pixels = buf.len() / 4;
    let chunks = pixels / 16; // 16 pixels = 64 bytes
    let mut offset = 0;

    for _ in 0..chunks {
        let ptr = buf.as_mut_ptr().add(offset) as *mut __m512i;
        let packed = _mm512_loadu_si512(ptr as *const __m512i);

        let lo = _mm512_unpacklo_epi8(packed, zero);
        let hi = _mm512_unpackhi_epi8(packed, zero);

        let a_lo = _mm512_shufflehi_epi16::<0xFF>(_mm512_shufflelo_epi16::<0xFF>(lo));
        let a_hi = _mm512_shufflehi_epi16::<0xFF>(_mm512_shufflelo_epi16::<0xFF>(hi));

        let prod_lo = _mm512_mullo_epi16(lo, a_lo);
        let prod_hi = _mm512_mullo_epi16(hi, a_hi);
        let biased_lo = _mm512_add_epi16(prod_lo, half);
        let biased_hi = _mm512_add_epi16(prod_hi, half);
        let result_lo = _mm512_srli_epi16::<8>(_mm512_add_epi16(
            biased_lo,
            _mm512_srli_epi16::<8>(biased_lo),
        ));
        let result_hi = _mm512_srli_epi16::<8>(_mm512_add_epi16(
            biased_hi,
            _mm512_srli_epi16::<8>(biased_hi),
        ));

        let result_lo = _mm512_or_si512(
            _mm512_andnot_si512(alpha_mask, result_lo),
            _mm512_and_si512(alpha_mask, lo),
        );
        let result_hi = _mm512_or_si512(
            _mm512_andnot_si512(alpha_mask, result_hi),
            _mm512_and_si512(alpha_mask, hi),
        );

        let result = _mm512_packus_epi16(result_lo, result_hi);
        _mm512_storeu_si512(ptr, result);
        offset += 64;
    }

    // Fall through to AVX2 for remaining pixels
    if offset / 4 < pixels {
        premultiply_alpha_avx2(&mut buf[offset..]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_pattern_works() {
        let mut buf = vec![0u8; 64];
        fill_pattern(&mut buf, [10, 20, 30, 40]);
        for i in 0..16 {
            assert_eq!(buf[i * 4], 10);
            assert_eq!(buf[i * 4 + 1], 20);
            assert_eq!(buf[i * 4 + 2], 30);
            assert_eq!(buf[i * 4 + 3], 40);
        }
    }

    #[test]
    fn fill_zero_works() {
        let mut buf = vec![0xFFu8; 100];
        fill_zero(&mut buf);
        assert!(buf.iter().all(|&b| b == 0));
    }

    #[test]
    fn fill_pattern_odd_count() {
        // Not a multiple of 8 pixels
        let mut buf = vec![0u8; 20]; // 5 pixels
        fill_pattern(&mut buf, [1, 2, 3, 4]);
        for i in 0..5 {
            assert_eq!(&buf[i * 4..i * 4 + 4], &[1, 2, 3, 4]);
        }
    }

    #[test]
    fn premultiply_opaque_unchanged() {
        let mut buf = vec![100, 150, 200, 255];
        premultiply_alpha(&mut buf);
        assert_eq!(buf, [100, 150, 200, 255]);
    }

    #[test]
    fn premultiply_transparent_zeroes() {
        let mut buf = vec![100, 150, 200, 0];
        premultiply_alpha(&mut buf);
        assert_eq!(buf, [0, 0, 0, 0]);
    }

    #[test]
    fn premultiply_half_alpha() {
        let mut buf = vec![200, 200, 200, 128];
        premultiply_alpha(&mut buf);
        // 200 * 128 / 255 ≈ 100
        assert!((buf[0] as i16 - 100).abs() <= 1);
        assert!((buf[1] as i16 - 100).abs() <= 1);
        assert!((buf[2] as i16 - 100).abs() <= 1);
        assert_eq!(buf[3], 128); // alpha preserved
    }

    #[test]
    fn premultiply_matches_scalar() {
        let pixel_count = 33; // odd count
        let mut buf: Vec<u8> = (0..pixel_count * 4).map(|i| (i % 256) as u8).collect();
        let mut buf_scalar = buf.clone();

        premultiply_alpha(&mut buf);
        premultiply_alpha_scalar(&mut buf_scalar);

        for i in 0..buf.len() {
            let diff = (buf[i] as i16 - buf_scalar[i] as i16).abs();
            assert!(
                diff <= 1,
                "byte {i}: simd={} scalar={}",
                buf[i],
                buf_scalar[i]
            );
        }
    }

    // ── Byte-identical wide-path equality (no-fake-green) ────────────────
    // Each wide path MUST produce bit-for-bit identical output to the SSE2
    // narrow path for the same input. Exercises tail / non-multiple widths.
    #[cfg(target_arch = "x86_64")]
    fn premultiply_inputs() -> Vec<Vec<u8>> {
        // Pixel counts chosen to exercise every tail: not multiples of 4/8/16,
        // plus exact boundaries and large buffers.
        let counts = [0usize, 1, 3, 4, 5, 7, 8, 9, 15, 16, 17, 31, 33, 64, 137, 1000];
        counts
            .iter()
            .map(|&n| {
                (0..n * 4)
                    .map(|i| ((i * 37 + 11) % 256) as u8)
                    .collect::<Vec<u8>>()
            })
            .collect()
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn premultiply_avx2_byte_identical_to_sse2() {
        if !crate::detect::has_avx2() {
            eprintln!("skipping: AVX2 not available");
            return;
        }
        for input in premultiply_inputs() {
            let mut sse2 = input.clone();
            let mut avx2 = input.clone();
            unsafe {
                premultiply_alpha_sse2(&mut sse2);
                premultiply_alpha_avx2(&mut avx2);
            }
            assert_eq!(avx2, sse2, "AVX2 != SSE2 for {} pixels", input.len() / 4);
        }
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn premultiply_avx512_byte_identical_to_sse2() {
        if !crate::detect::has_avx512() {
            eprintln!("skipping: AVX-512 not available");
            return;
        }
        for input in premultiply_inputs() {
            let mut sse2 = input.clone();
            let mut avx512 = input.clone();
            unsafe {
                premultiply_alpha_sse2(&mut sse2);
                premultiply_alpha_avx512(&mut avx512);
            }
            assert_eq!(
                avx512,
                sse2,
                "AVX-512 != SSE2 for {} pixels",
                input.len() / 4
            );
        }
    }

    #[test]
    fn copy_rows_different_strides() {
        let src = vec![1, 2, 3, 4, 0, 0, 5, 6, 7, 8, 0, 0]; // stride=6, 4 bytes data
        let mut dst = vec![0u8; 8]; // stride=4, packed
        copy_rows(&mut dst, 4, &src, 6, 4, 2);
        assert_eq!(dst, [1, 2, 3, 4, 5, 6, 7, 8]);
    }
}
