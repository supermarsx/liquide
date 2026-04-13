//! SIMD-accelerated XOR delta encoding for tile-level frame differencing.
//!
//! XOR delta compares current and previous tile data byte-by-byte.
//! Runs of zero (unchanged) bytes compress well with Zstd/LZ4.
//!
//! AVX-512 processes 64 bytes/cycle, AVX2 processes 32 bytes/cycle, SSE2 processes 16 bytes/cycle.

/// Compute XOR delta between `current` and `previous` into `dst`.
///
/// All three slices must have equal length. `dst[i] = current[i] ^ previous[i]`.
pub fn xor_delta(dst: &mut [u8], current: &[u8], previous: &[u8]) {
    assert_eq!(dst.len(), current.len());
    assert_eq!(dst.len(), previous.len());

    #[cfg(target_arch = "x86_64")]
    {
        if crate::detect::has_avx512() {
            unsafe { return xor_delta_avx512(dst, current, previous) }
        }
        if crate::detect::has_avx2() {
            unsafe { return xor_delta_avx2(dst, current, previous) }
        }
        unsafe { return xor_delta_sse2(dst, current, previous) }
    }
    #[cfg(not(target_arch = "x86_64"))]
    xor_delta_scalar(dst, current, previous);
}

/// Compute XOR delta, allocating the result.
#[must_use]
pub fn xor_delta_alloc(current: &[u8], previous: &[u8]) -> Vec<u8> {
    let mut dst = vec![0u8; current.len()];
    xor_delta(&mut dst, current, previous);
    dst
}

/// Apply XOR delta: `dst[i] = previous[i] ^ delta[i]`.
pub fn xor_apply(dst: &mut [u8], previous: &[u8], delta: &[u8]) {
    // Same operation as xor_delta — XOR is its own inverse
    xor_delta(dst, previous, delta);
}

/// Count non-zero bytes in a delta buffer.
///
/// Uses SSE2/AVX2/AVX-512 to compare 16/32/64 bytes at a time against zero,
/// then counts via popcount on the comparison mask.
#[must_use]
pub fn xor_popcount(delta: &[u8]) -> usize {
    #[cfg(target_arch = "x86_64")]
    {
        if crate::detect::has_avx512() {
            unsafe { return xor_popcount_avx512bw(delta) }
        }
        if crate::detect::has(crate::detect::features::POPCNT | crate::detect::features::AVX2) {
            unsafe { return xor_popcount_popcnt_avx2(delta) }
        }
        if crate::detect::has_avx2() {
            unsafe { return xor_popcount_avx2(delta) }
        }
        unsafe { return xor_popcount_sse2(delta) }
    }
    #[cfg(not(target_arch = "x86_64"))]
    delta.iter().filter(|&&b| b != 0).count()
}

/// Ratio of changed bytes (0.0 = identical, 1.0 = completely different).
#[must_use]
pub fn change_ratio(delta: &[u8]) -> f32 {
    if delta.is_empty() {
        return 0.0;
    }
    xor_popcount(delta) as f32 / delta.len() as f32
}

// ── Scalar fallback ───────────────────────────────────────────────────

fn xor_delta_scalar(dst: &mut [u8], current: &[u8], previous: &[u8]) {
    for i in 0..dst.len() {
        dst[i] = current[i] ^ previous[i];
    }
}

// ── SSE2 ──────────────────────────────────────────────────────────────

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn xor_delta_sse2(dst: &mut [u8], current: &[u8], previous: &[u8]) {
    use std::arch::x86_64::*;

    let len = dst.len();
    let chunks = len / 16;
    let mut offset = 0;

    for _ in 0..chunks {
        let c = _mm_loadu_si128(current.as_ptr().add(offset) as *const __m128i);
        let p = _mm_loadu_si128(previous.as_ptr().add(offset) as *const __m128i);
        let result = _mm_xor_si128(c, p);
        _mm_storeu_si128(dst.as_mut_ptr().add(offset) as *mut __m128i, result);
        offset += 16;
    }

    for i in offset..len {
        dst[i] = current[i] ^ previous[i];
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn xor_popcount_sse2(delta: &[u8]) -> usize {
    use std::arch::x86_64::*;

    let len = delta.len();
    let chunks = len / 16;
    let mut offset = 0;
    let mut count = 0usize;
    let zero = _mm_setzero_si128();

    for _ in 0..chunks {
        let v = _mm_loadu_si128(delta.as_ptr().add(offset) as *const __m128i);
        // Compare each byte == 0, result is 0xFF for equal, 0x00 for not equal
        let eq = _mm_cmpeq_epi8(v, zero);
        // movemask extracts MSB of each byte → 16-bit mask
        let mask = _mm_movemask_epi8(eq) as u32;
        // Count zero bytes (bits set in mask), subtract from 16
        count += 16 - mask.count_ones() as usize;
        offset += 16;
    }

    for i in offset..len {
        if delta[i] != 0 {
            count += 1;
        }
    }

    count
}

// ── AVX-512 ──────────────────────────────────────────────────────────

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f")]
unsafe fn xor_delta_avx512(dst: &mut [u8], current: &[u8], previous: &[u8]) {
    use std::arch::x86_64::*;

    let len = dst.len();
    let chunks = len / 64;
    let mut offset = 0;

    for _ in 0..chunks {
        let c = _mm512_loadu_si512(current.as_ptr().add(offset) as *const __m512i);
        let p = _mm512_loadu_si512(previous.as_ptr().add(offset) as *const __m512i);
        let result = _mm512_xor_si512(c, p);
        _mm512_storeu_si512(dst.as_mut_ptr().add(offset) as *mut __m512i, result);
        offset += 64;
    }

    // Tail falls through to AVX2 for remaining bytes
    if offset < len {
        xor_delta_avx2(&mut dst[offset..], &current[offset..], &previous[offset..]);
    }
}

// ── AVX2 ──────────────────────────────────────────────────────────────

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn xor_delta_avx2(dst: &mut [u8], current: &[u8], previous: &[u8]) {
    use std::arch::x86_64::*;

    let len = dst.len();
    let chunks = len / 32;
    let mut offset = 0;

    for _ in 0..chunks {
        let c = _mm256_loadu_si256(current.as_ptr().add(offset) as *const __m256i);
        let p = _mm256_loadu_si256(previous.as_ptr().add(offset) as *const __m256i);
        let result = _mm256_xor_si256(c, p);
        _mm256_storeu_si256(dst.as_mut_ptr().add(offset) as *mut __m256i, result);
        offset += 32;
    }

    // SSE2 for 16-byte chunks
    if offset + 16 <= len {
        xor_delta_sse2(&mut dst[offset..], &current[offset..], &previous[offset..]);
    } else {
        for i in offset..len {
            dst[i] = current[i] ^ previous[i];
        }
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn xor_popcount_avx2(delta: &[u8]) -> usize {
    use std::arch::x86_64::*;

    let len = delta.len();
    let chunks = len / 32;
    let mut offset = 0;
    let mut count = 0usize;
    let zero = _mm256_setzero_si256();

    for _ in 0..chunks {
        let v = _mm256_loadu_si256(delta.as_ptr().add(offset) as *const __m256i);
        let eq = _mm256_cmpeq_epi8(v, zero);
        let mask = _mm256_movemask_epi8(eq) as u32;
        count += 32 - mask.count_ones() as usize;
        offset += 32;
    }

    // Handle remaining bytes via SSE2 path
    if offset + 16 <= len {
        count += xor_popcount_sse2(&delta[offset..]);
    } else {
        for i in offset..len {
            if delta[i] != 0 {
                count += 1;
            }
        }
    }

    count
}

// ── Hardware POPCNT + AVX2 ──────────────────────────────────────────

/// AVX2 popcount with guaranteed hardware POPCNT instruction.
///
/// Same structure as `xor_popcount_avx2` but enabling the `popcnt` target
/// feature ensures `_popcnt32` compiles to the hardware POPCNT instruction
/// rather than a software fallback.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "popcnt,avx2")]
unsafe fn xor_popcount_popcnt_avx2(delta: &[u8]) -> usize {
    use std::arch::x86_64::*;

    let len = delta.len();
    let chunks = len / 32;
    let mut offset = 0;
    let mut count = 0usize;
    let zero = _mm256_setzero_si256();

    for _ in 0..chunks {
        let v = _mm256_loadu_si256(delta.as_ptr().add(offset) as *const __m256i);
        let eq = _mm256_cmpeq_epi8(v, zero);
        let mask = _mm256_movemask_epi8(eq) as u32;
        count += 32 - _popcnt32(mask as i32) as usize;
        offset += 32;
    }

    // Handle remaining bytes via SSE2 path
    if offset + 16 <= len {
        count += xor_popcount_sse2(&delta[offset..]);
    } else {
        for i in offset..len {
            if delta[i] != 0 {
                count += 1;
            }
        }
    }

    count
}

// ── AVX-512BW popcount ──────────────────────────────────────────────

/// AVX-512BW popcount: processes 64 bytes per iteration using
/// `_mm512_cmpeq_epi8_mask` which returns a 64-bit mask directly.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f,avx512bw")]
unsafe fn xor_popcount_avx512bw(delta: &[u8]) -> usize {
    use std::arch::x86_64::*;

    let len = delta.len();
    let chunks = len / 64;
    let mut offset = 0;
    let mut count = 0usize;
    let zero = _mm512_setzero_si512();

    for _ in 0..chunks {
        let v = _mm512_loadu_si512(delta.as_ptr().add(offset) as *const __m512i);
        let mask = _mm512_cmpeq_epi8_mask(v, zero);
        // mask has a 1 for each byte that IS zero; count NON-zero bytes
        count += 64 - _popcnt64(mask as i64) as usize;
        offset += 64;
    }

    // Tail falls through to AVX2 + POPCNT path
    if offset < len {
        count += xor_popcount_popcnt_avx2(&delta[offset..]);
    }

    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xor_identical_is_zero() {
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let result = xor_delta_alloc(&data, &data);
        assert!(result.iter().all(|&b| b == 0));
    }

    #[test]
    fn xor_inverse_roundtrip() {
        let current = vec![10, 20, 30, 40, 50, 60, 70, 80];
        let previous = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let delta = xor_delta_alloc(&current, &previous);
        let mut recovered = vec![0u8; previous.len()];
        xor_apply(&mut recovered, &previous, &delta);
        assert_eq!(recovered, current);
    }

    #[test]
    fn popcount_all_zero() {
        let delta = vec![0u8; 64];
        assert_eq!(xor_popcount(&delta), 0);
    }

    #[test]
    fn popcount_all_nonzero() {
        let delta = vec![1u8; 64];
        assert_eq!(xor_popcount(&delta), 64);
    }

    #[test]
    fn popcount_mixed() {
        let mut delta = vec![0u8; 100];
        delta[5] = 1;
        delta[50] = 255;
        delta[99] = 42;
        assert_eq!(xor_popcount(&delta), 3);
    }

    #[test]
    fn change_ratio_empty() {
        assert_eq!(change_ratio(&[]), 0.0);
    }

    #[test]
    fn change_ratio_half() {
        let delta = vec![0, 1, 0, 1];
        assert_eq!(change_ratio(&delta), 0.5);
    }

    #[test]
    fn xor_avx512_large_buffer() {
        // 128 * 64 = 8192 bytes — exercises multiple AVX-512 iterations + AVX2 tail
        let size = 128 * 64;
        let current: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
        let previous: Vec<u8> = (0..size).map(|i| ((i * 7 + 3) % 256) as u8).collect();

        let delta = xor_delta_alloc(&current, &previous);

        let expected: Vec<u8> = current.iter().zip(previous.iter()).map(|(&c, &p)| c ^ p).collect();
        assert_eq!(delta, expected);
    }

    #[test]
    fn xor_avx512_tail_handling() {
        // 64 + 32 + 16 + 7 = 119 bytes — exercises AVX-512 chunk, AVX2 tail, SSE2 tail, scalar tail
        let size = 119;
        let current: Vec<u8> = (0..size).map(|i| (i as u8).wrapping_mul(13)).collect();
        let previous: Vec<u8> = (0..size).map(|i| (i as u8).wrapping_add(42)).collect();

        let delta = xor_delta_alloc(&current, &previous);

        let expected: Vec<u8> = current.iter().zip(previous.iter()).map(|(&c, &p)| c ^ p).collect();
        assert_eq!(delta, expected);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn popcount_popcnt_avx2_matches_scalar() {
        if !crate::detect::has(crate::detect::features::POPCNT | crate::detect::features::AVX2) {
            return;
        }
        let size = 256;
        let mut delta = vec![0u8; size];
        // Set some bytes to non-zero
        for i in (0..size).step_by(3) {
            delta[i] = (i % 255 + 1) as u8;
        }
        let expected = delta.iter().filter(|&&b| b != 0).count();
        let result = unsafe { xor_popcount_popcnt_avx2(&delta) };
        assert_eq!(result, expected);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn popcount_avx512bw_matches_scalar() {
        if !crate::detect::has_avx512() {
            return;
        }
        let size = 512;
        let mut delta = vec![0u8; size];
        for i in (0..size).step_by(5) {
            delta[i] = (i % 255 + 1) as u8;
        }
        let expected = delta.iter().filter(|&&b| b != 0).count();
        let result = unsafe { xor_popcount_avx512bw(&delta) };
        assert_eq!(result, expected);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn popcount_avx512bw_tail_handling() {
        if !crate::detect::has_avx512() {
            return;
        }
        // 64 + 32 + 16 + 7 = 119 bytes — exercises AVX-512 chunk + AVX2 tail + SSE2 tail + scalar tail
        let size = 119;
        let delta: Vec<u8> = (0..size).map(|i| if i % 2 == 0 { 1 } else { 0 }).collect();
        let expected = delta.iter().filter(|&&b| b != 0).count();
        let result = unsafe { xor_popcount_avx512bw(&delta) };
        assert_eq!(result, expected);
    }

    #[test]
    fn large_buffer_correctness() {
        // 16 KB tile (64x64 BGRA)
        let size = 64 * 64 * 4;
        let current: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
        let previous: Vec<u8> = (0..size).map(|i| ((i + 1) % 256) as u8).collect();

        let delta = xor_delta_alloc(&current, &previous);

        // Verify against scalar
        let expected: Vec<u8> = current.iter().zip(previous.iter()).map(|(&c, &p)| c ^ p).collect();
        assert_eq!(delta, expected);

        // Verify popcount
        let expected_count = expected.iter().filter(|&&b| b != 0).count();
        assert_eq!(xor_popcount(&delta), expected_count);
    }
}
