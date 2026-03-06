//! SIMD-accelerated alpha blending for BGRA8 scanlines.
//!
//! The primary entry point is [`blend_scanline_src_over`] which processes
//! 4 pixels at a time on SSE2, 8 on AVX2, with a scalar tail loop.

/// Blend `src` over `dst` using premultiplied-alpha Porter-Duff SrcOver.
///
/// Both slices must be BGRA8 (length divisible by 4) and equal length.
/// Formula per channel: `out = src + dst * (1 - src_alpha)`
pub fn blend_scanline_src_over(dst: &mut [u8], src: &[u8]) {
    debug_assert_eq!(dst.len(), src.len());
    debug_assert_eq!(dst.len() % 4, 0);

    #[cfg(target_arch = "x86_64")]
    {
        if crate::detect::has_avx512() {
            unsafe { return blend_scanline_src_over_avx512(dst, src) }
        }
        if crate::detect::has_avx2() {
            // SAFETY: AVX2 detected at runtime
            unsafe { return blend_scanline_src_over_avx2(dst, src) }
        }
        // SSE2 is always available on x86-64
        unsafe { return blend_scanline_src_over_sse2(dst, src) }
    }
    #[cfg(not(target_arch = "x86_64"))]
    blend_scanline_src_over_scalar(dst, src);
}

/// Scalar fallback: one pixel at a time.
pub fn blend_scanline_src_over_scalar(dst: &mut [u8], src: &[u8]) {
    let pixels = dst.len() / 4;
    for i in 0..pixels {
        let off = i * 4;
        let sa = src[off + 3] as u16;
        if sa == 255 {
            dst[off..off + 4].copy_from_slice(&src[off..off + 4]);
        } else if sa > 0 {
            let inv_a = 255 - sa;
            for c in 0..4 {
                let s = src[off + c] as u16;
                let d = dst[off + c] as u16;
                dst[off + c] = (s + (d * inv_a + 127) / 255) as u8;
            }
        }
        // sa == 0: dst unchanged
    }
}

/// SSE2 implementation: 4 BGRA pixels per iteration.
///
/// Strategy: unpack u8 → u16, multiply, pack back.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn blend_scanline_src_over_sse2(dst: &mut [u8], src: &[u8]) {
    use std::arch::x86_64::*;

    let len = dst.len();
    let chunks = len / 16; // 4 pixels = 16 bytes
    let mut offset = 0;

    let zero = _mm_setzero_si128();
    let all_ff = _mm_set1_epi16(0x00FF);
    let half = _mm_set1_epi16(128);

    for _ in 0..chunks {
        let d_ptr = dst.as_mut_ptr().add(offset) as *mut __m128i;
        let s_ptr = src.as_ptr().add(offset) as *const __m128i;

        let s_packed = _mm_loadu_si128(s_ptr);
        let d_packed = _mm_loadu_si128(d_ptr);

        // Process low 8 bytes (2 pixels) and high 8 bytes (2 pixels) separately
        let s_lo = _mm_unpacklo_epi8(s_packed, zero); // u8 → u16
        let s_hi = _mm_unpackhi_epi8(s_packed, zero);
        let d_lo = _mm_unpacklo_epi8(d_packed, zero);
        let d_hi = _mm_unpackhi_epi8(d_packed, zero);

        // Extract alpha for each pixel and broadcast across 4 channels
        // BGRA layout: bytes [0]=B [1]=G [2]=R [3]=A within each pixel
        // In u16 lanes after unpack: [B0 G0 R0 A0 B1 G1 R1 A1]
        // Shuffle alpha to all lanes: A0→[A0 A0 A0 A0] A1→[A1 A1 A1 A1]
        let a_lo = _mm_shufflelo_epi16::<0xFF>(s_lo); // A0 to low 4 lanes
        let a_lo = _mm_shufflehi_epi16::<0xFF>(a_lo); // A1 to high 4 lanes
        let a_hi = _mm_shufflelo_epi16::<0xFF>(s_hi);
        let a_hi = _mm_shufflehi_epi16::<0xFF>(a_hi);

        // inv_a = 255 - alpha
        let inv_a_lo = _mm_sub_epi16(all_ff, a_lo);
        let inv_a_hi = _mm_sub_epi16(all_ff, a_hi);

        // dst * inv_a: (d * inv_a + 128) / 255
        // Approximate: (d * inv_a + 128) >> 8 is close enough and much faster.
        // More precise: ((d * inv_a + 128) + ((d * inv_a + 128) >> 8)) >> 8
        let prod_lo = _mm_mullo_epi16(d_lo, inv_a_lo);
        let prod_hi = _mm_mullo_epi16(d_hi, inv_a_hi);

        let biased_lo = _mm_add_epi16(prod_lo, half);
        let biased_hi = _mm_add_epi16(prod_hi, half);

        // Precise /255: add (biased >> 8) then >> 8
        let approx_lo = _mm_srli_epi16::<8>(_mm_add_epi16(biased_lo, _mm_srli_epi16::<8>(biased_lo)));
        let approx_hi = _mm_srli_epi16::<8>(_mm_add_epi16(biased_hi, _mm_srli_epi16::<8>(biased_hi)));

        // out = src + dst * inv_a / 255
        let out_lo = _mm_add_epi16(s_lo, approx_lo);
        let out_hi = _mm_add_epi16(s_hi, approx_hi);

        // Pack u16 → u8 with saturation
        let result = _mm_packus_epi16(out_lo, out_hi);
        _mm_storeu_si128(d_ptr, result);

        offset += 16;
    }

    // Scalar tail for remaining pixels
    if offset < len {
        blend_scanline_src_over_scalar(&mut dst[offset..], &src[offset..]);
    }
}

/// AVX-512 implementation: 16 BGRA pixels per iteration.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f,avx512bw")]
unsafe fn blend_scanline_src_over_avx512(dst: &mut [u8], src: &[u8]) {
    use std::arch::x86_64::*;

    let len = dst.len();
    let chunks = len / 64; // 16 pixels = 64 bytes
    let mut offset = 0;

    let zero = _mm512_setzero_si512();
    let all_ff = _mm512_set1_epi16(0x00FF);
    let half = _mm512_set1_epi16(128);

    for _ in 0..chunks {
        let d_ptr = dst.as_mut_ptr().add(offset) as *mut __m512i;
        let s_ptr = src.as_ptr().add(offset) as *const __m512i;

        let s_packed = _mm512_loadu_si512(s_ptr);
        let d_packed = _mm512_loadu_si512(d_ptr);

        // Unpack low/high halves: u8 → u16
        let s_lo = _mm512_unpacklo_epi8(s_packed, zero);
        let s_hi = _mm512_unpackhi_epi8(s_packed, zero);
        let d_lo = _mm512_unpacklo_epi8(d_packed, zero);
        let d_hi = _mm512_unpackhi_epi8(d_packed, zero);

        // Broadcast alpha across each pixel's 4 channels
        let a_lo = _mm512_shufflelo_epi16::<0xFF>(s_lo);
        let a_lo = _mm512_shufflehi_epi16::<0xFF>(a_lo);
        let a_hi = _mm512_shufflelo_epi16::<0xFF>(s_hi);
        let a_hi = _mm512_shufflehi_epi16::<0xFF>(a_hi);

        let inv_a_lo = _mm512_sub_epi16(all_ff, a_lo);
        let inv_a_hi = _mm512_sub_epi16(all_ff, a_hi);

        let prod_lo = _mm512_mullo_epi16(d_lo, inv_a_lo);
        let prod_hi = _mm512_mullo_epi16(d_hi, inv_a_hi);

        let biased_lo = _mm512_add_epi16(prod_lo, half);
        let biased_hi = _mm512_add_epi16(prod_hi, half);

        let approx_lo = _mm512_srli_epi16::<8>(_mm512_add_epi16(biased_lo, _mm512_srli_epi16::<8>(biased_lo)));
        let approx_hi = _mm512_srli_epi16::<8>(_mm512_add_epi16(biased_hi, _mm512_srli_epi16::<8>(biased_hi)));

        let out_lo = _mm512_add_epi16(s_lo, approx_lo);
        let out_hi = _mm512_add_epi16(s_hi, approx_hi);

        let result = _mm512_packus_epi16(out_lo, out_hi);
        _mm512_storeu_si512(d_ptr, result);

        offset += 64;
    }

    // Fall through to AVX2 path for remaining pixels
    if offset < len {
        blend_scanline_src_over_avx2(&mut dst[offset..], &src[offset..]);
    }
}

/// AVX2 implementation: 8 BGRA pixels per iteration.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn blend_scanline_src_over_avx2(dst: &mut [u8], src: &[u8]) {
    use std::arch::x86_64::*;

    let len = dst.len();
    let chunks = len / 32; // 8 pixels = 32 bytes
    let mut offset = 0;

    let zero = _mm256_setzero_si256();
    let all_ff = _mm256_set1_epi16(0x00FF);
    let half = _mm256_set1_epi16(128);

    for _ in 0..chunks {
        let d_ptr = dst.as_mut_ptr().add(offset) as *mut __m256i;
        let s_ptr = src.as_ptr().add(offset) as *const __m256i;

        let s_packed = _mm256_loadu_si256(s_ptr);
        let d_packed = _mm256_loadu_si256(d_ptr);

        // Unpack low/high halves: u8 → u16
        let s_lo = _mm256_unpacklo_epi8(s_packed, zero);
        let s_hi = _mm256_unpackhi_epi8(s_packed, zero);
        let d_lo = _mm256_unpacklo_epi8(d_packed, zero);
        let d_hi = _mm256_unpackhi_epi8(d_packed, zero);

        // Broadcast alpha across each pixel's 4 channels
        let a_lo = _mm256_shufflelo_epi16::<0xFF>(s_lo);
        let a_lo = _mm256_shufflehi_epi16::<0xFF>(a_lo);
        let a_hi = _mm256_shufflelo_epi16::<0xFF>(s_hi);
        let a_hi = _mm256_shufflehi_epi16::<0xFF>(a_hi);

        let inv_a_lo = _mm256_sub_epi16(all_ff, a_lo);
        let inv_a_hi = _mm256_sub_epi16(all_ff, a_hi);

        let prod_lo = _mm256_mullo_epi16(d_lo, inv_a_lo);
        let prod_hi = _mm256_mullo_epi16(d_hi, inv_a_hi);

        let biased_lo = _mm256_add_epi16(prod_lo, half);
        let biased_hi = _mm256_add_epi16(prod_hi, half);

        let approx_lo = _mm256_srli_epi16::<8>(_mm256_add_epi16(biased_lo, _mm256_srli_epi16::<8>(biased_lo)));
        let approx_hi = _mm256_srli_epi16::<8>(_mm256_add_epi16(biased_hi, _mm256_srli_epi16::<8>(biased_hi)));

        let out_lo = _mm256_add_epi16(s_lo, approx_lo);
        let out_hi = _mm256_add_epi16(s_hi, approx_hi);

        let result = _mm256_packus_epi16(out_lo, out_hi);
        _mm256_storeu_si256(d_ptr, result);

        offset += 32;
    }

    // SSE2 for 4-pixel chunks remaining
    if offset + 16 <= len {
        blend_scanline_src_over_sse2(&mut dst[offset..], &src[offset..]);
    } else if offset < len {
        blend_scanline_src_over_scalar(&mut dst[offset..], &src[offset..]);
    }
}

/// Multiply blend on a BGRA8 scanline: `out = src * dst / 255`.
pub fn blend_scanline_multiply(dst: &mut [u8], src: &[u8]) {
    debug_assert_eq!(dst.len(), src.len());
    debug_assert_eq!(dst.len() % 4, 0);

    #[cfg(target_arch = "x86_64")]
    {
        // SSE2 is always available on x86-64
        unsafe { return blend_scanline_multiply_sse2(dst, src) }
    }
    #[cfg(not(target_arch = "x86_64"))]
    blend_scanline_multiply_scalar(dst, src);
}

fn blend_scanline_multiply_scalar(dst: &mut [u8], src: &[u8]) {
    let pixels = dst.len() / 4;
    for i in 0..pixels {
        let off = i * 4;
        for c in 0..4 {
            dst[off + c] = ((dst[off + c] as u16 * src[off + c] as u16 + 127) / 255) as u8;
        }
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn blend_scanline_multiply_sse2(dst: &mut [u8], src: &[u8]) {
    use std::arch::x86_64::*;

    let len = dst.len();
    let chunks = len / 16;
    let mut offset = 0;

    let zero = _mm_setzero_si128();
    let half = _mm_set1_epi16(128);

    for _ in 0..chunks {
        let d_ptr = dst.as_mut_ptr().add(offset) as *mut __m128i;
        let s_ptr = src.as_ptr().add(offset) as *const __m128i;

        let s = _mm_loadu_si128(s_ptr);
        let d = _mm_loadu_si128(d_ptr);

        let s_lo = _mm_unpacklo_epi8(s, zero);
        let s_hi = _mm_unpackhi_epi8(s, zero);
        let d_lo = _mm_unpacklo_epi8(d, zero);
        let d_hi = _mm_unpackhi_epi8(d, zero);

        // (d * s + 128) — approximate /255 using (x + 128 + (x+128)>>8) >> 8
        let prod_lo = _mm_mullo_epi16(d_lo, s_lo);
        let prod_hi = _mm_mullo_epi16(d_hi, s_hi);

        let biased_lo = _mm_add_epi16(prod_lo, half);
        let biased_hi = _mm_add_epi16(prod_hi, half);

        let result_lo = _mm_srli_epi16::<8>(_mm_add_epi16(biased_lo, _mm_srli_epi16::<8>(biased_lo)));
        let result_hi = _mm_srli_epi16::<8>(_mm_add_epi16(biased_hi, _mm_srli_epi16::<8>(biased_hi)));

        let result = _mm_packus_epi16(result_lo, result_hi);
        _mm_storeu_si128(d_ptr, result);

        offset += 16;
    }

    if offset < len {
        blend_scanline_multiply_scalar(&mut dst[offset..], &src[offset..]);
    }
}

/// Darken blend on a BGRA8 scanline: `out = min(src, dst)` per channel.
pub fn blend_scanline_darken(dst: &mut [u8], src: &[u8]) {
    debug_assert_eq!(dst.len(), src.len());

    #[cfg(target_arch = "x86_64")]
    {
        if crate::detect::has_avx512() {
            unsafe { return blend_scanline_darken_avx512(dst, src) }
        }
        unsafe { return blend_scanline_darken_sse2(dst, src) }
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        for (d, s) in dst.iter_mut().zip(src.iter()) {
            *d = (*d).min(*s);
        }
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f,avx512bw")]
unsafe fn blend_scanline_darken_avx512(dst: &mut [u8], src: &[u8]) {
    use std::arch::x86_64::*;

    let len = dst.len();
    let chunks = len / 64;
    let mut offset = 0;

    for _ in 0..chunks {
        let d_ptr = dst.as_mut_ptr().add(offset) as *mut __m512i;
        let s_ptr = src.as_ptr().add(offset) as *const __m512i;
        let result = _mm512_min_epu8(_mm512_loadu_si512(d_ptr), _mm512_loadu_si512(s_ptr));
        _mm512_storeu_si512(d_ptr, result);
        offset += 64;
    }

    for i in offset..len {
        dst[i] = dst[i].min(src[i]);
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn blend_scanline_darken_sse2(dst: &mut [u8], src: &[u8]) {
    use std::arch::x86_64::*;

    let len = dst.len();
    let chunks = len / 16;
    let mut offset = 0;

    for _ in 0..chunks {
        let d_ptr = dst.as_mut_ptr().add(offset) as *mut __m128i;
        let s_ptr = src.as_ptr().add(offset) as *const __m128i;
        let result = _mm_min_epu8(_mm_loadu_si128(d_ptr), _mm_loadu_si128(s_ptr));
        _mm_storeu_si128(d_ptr, result);
        offset += 16;
    }

    for i in offset..len {
        dst[i] = dst[i].min(src[i]);
    }
}

/// Lighten blend on a BGRA8 scanline: `out = max(src, dst)` per channel.
pub fn blend_scanline_lighten(dst: &mut [u8], src: &[u8]) {
    debug_assert_eq!(dst.len(), src.len());

    #[cfg(target_arch = "x86_64")]
    {
        if crate::detect::has_avx512() {
            unsafe { return blend_scanline_lighten_avx512(dst, src) }
        }
        unsafe { return blend_scanline_lighten_sse2(dst, src) }
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        for (d, s) in dst.iter_mut().zip(src.iter()) {
            *d = (*d).max(*s);
        }
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f,avx512bw")]
unsafe fn blend_scanline_lighten_avx512(dst: &mut [u8], src: &[u8]) {
    use std::arch::x86_64::*;

    let len = dst.len();
    let chunks = len / 64;
    let mut offset = 0;

    for _ in 0..chunks {
        let d_ptr = dst.as_mut_ptr().add(offset) as *mut __m512i;
        let s_ptr = src.as_ptr().add(offset) as *const __m512i;
        let result = _mm512_max_epu8(_mm512_loadu_si512(d_ptr), _mm512_loadu_si512(s_ptr));
        _mm512_storeu_si512(d_ptr, result);
        offset += 64;
    }

    for i in offset..len {
        dst[i] = dst[i].max(src[i]);
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn blend_scanline_lighten_sse2(dst: &mut [u8], src: &[u8]) {
    use std::arch::x86_64::*;

    let len = dst.len();
    let chunks = len / 16;
    let mut offset = 0;

    for _ in 0..chunks {
        let d_ptr = dst.as_mut_ptr().add(offset) as *mut __m128i;
        let s_ptr = src.as_ptr().add(offset) as *const __m128i;
        let result = _mm_max_epu8(_mm_loadu_si128(d_ptr), _mm_loadu_si128(s_ptr));
        _mm_storeu_si128(d_ptr, result);
        offset += 16;
    }

    for i in offset..len {
        dst[i] = dst[i].max(src[i]);
    }
}

/// Difference blend on a BGRA8 scanline: `out = |src - dst|` per byte.
pub fn blend_scanline_difference(dst: &mut [u8], src: &[u8]) {
    debug_assert_eq!(dst.len(), src.len());

    #[cfg(target_arch = "x86_64")]
    {
        unsafe { return blend_scanline_difference_sse2(dst, src) }
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        for (d, s) in dst.iter_mut().zip(src.iter()) {
            *d = (*d as i16 - *s as i16).unsigned_abs() as u8;
        }
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn blend_scanline_difference_sse2(dst: &mut [u8], src: &[u8]) {
    use std::arch::x86_64::*;

    let len = dst.len();
    let chunks = len / 16;
    let mut offset = 0;

    for _ in 0..chunks {
        let d_ptr = dst.as_mut_ptr().add(offset) as *mut __m128i;
        let s_ptr = src.as_ptr().add(offset) as *const __m128i;
        let d = _mm_loadu_si128(d_ptr);
        let s = _mm_loadu_si128(s_ptr);
        // |a - b| = max(a,b) - min(a,b) for unsigned bytes
        let max = _mm_max_epu8(d, s);
        let min = _mm_min_epu8(d, s);
        let result = _mm_sub_epi8(max, min);
        _mm_storeu_si128(d_ptr, result);
        offset += 16;
    }

    for i in offset..len {
        dst[i] = (dst[i] as i16 - src[i] as i16).unsigned_abs() as u8;
    }
}

/// Screen blend on a BGRA8 scanline: `out = src + dst - src * dst / 255`.
pub fn blend_scanline_screen(dst: &mut [u8], src: &[u8]) {
    debug_assert_eq!(dst.len(), src.len());
    debug_assert_eq!(dst.len() % 4, 0);

    #[cfg(target_arch = "x86_64")]
    {
        unsafe { return blend_scanline_screen_sse2(dst, src) }
    }
    #[cfg(not(target_arch = "x86_64"))]
    blend_scanline_screen_scalar(dst, src);
}

fn blend_scanline_screen_scalar(dst: &mut [u8], src: &[u8]) {
    let pixels = dst.len() / 4;
    for i in 0..pixels {
        let off = i * 4;
        for c in 0..4 {
            let s = src[off + c] as u16;
            let d = dst[off + c] as u16;
            dst[off + c] = (s + d - (s * d + 127) / 255) as u8;
        }
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn blend_scanline_screen_sse2(dst: &mut [u8], src: &[u8]) {
    use std::arch::x86_64::*;

    let len = dst.len();
    let chunks = len / 16;
    let mut offset = 0;

    let zero = _mm_setzero_si128();
    let half = _mm_set1_epi16(128);

    for _ in 0..chunks {
        let d_ptr = dst.as_mut_ptr().add(offset) as *mut __m128i;
        let s_ptr = src.as_ptr().add(offset) as *const __m128i;

        let s = _mm_loadu_si128(s_ptr);
        let d = _mm_loadu_si128(d_ptr);

        let s_lo = _mm_unpacklo_epi8(s, zero);
        let s_hi = _mm_unpackhi_epi8(s, zero);
        let d_lo = _mm_unpacklo_epi8(d, zero);
        let d_hi = _mm_unpackhi_epi8(d, zero);

        // s * d / 255 (approximate)
        let prod_lo = _mm_mullo_epi16(s_lo, d_lo);
        let prod_hi = _mm_mullo_epi16(s_hi, d_hi);
        let biased_lo = _mm_add_epi16(prod_lo, half);
        let biased_hi = _mm_add_epi16(prod_hi, half);
        let div_lo = _mm_srli_epi16::<8>(_mm_add_epi16(biased_lo, _mm_srli_epi16::<8>(biased_lo)));
        let div_hi = _mm_srli_epi16::<8>(_mm_add_epi16(biased_hi, _mm_srli_epi16::<8>(biased_hi)));

        // s + d - (s * d / 255)
        let sum_lo = _mm_add_epi16(s_lo, d_lo);
        let sum_hi = _mm_add_epi16(s_hi, d_hi);
        let out_lo = _mm_sub_epi16(sum_lo, div_lo);
        let out_hi = _mm_sub_epi16(sum_hi, div_hi);

        let result = _mm_packus_epi16(out_lo, out_hi);
        _mm_storeu_si128(d_ptr, result);

        offset += 16;
    }

    if offset < len {
        blend_scanline_screen_scalar(&mut dst[offset..], &src[offset..]);
    }
}

/// Invert a BGRA8 scanline in-place: `R = 255-R, G = 255-G, B = 255-B`, alpha preserved.
pub fn invert_scanline(buf: &mut [u8]) {
    debug_assert_eq!(buf.len() % 4, 0);

    #[cfg(target_arch = "x86_64")]
    {
        if crate::detect::has_avx512() {
            unsafe { return invert_scanline_avx512(buf) }
        }
        unsafe { return invert_scanline_sse2(buf) }
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        let pixels = buf.len() / 4;
        for i in 0..pixels {
            let off = i * 4;
            buf[off] = 255 - buf[off]; // B
            buf[off + 1] = 255 - buf[off + 1]; // G
            buf[off + 2] = 255 - buf[off + 2]; // R
            // alpha unchanged
        }
    }
}

/// AVX-512 implementation: 16 BGRA pixels (64 bytes) per iteration.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f")]
unsafe fn invert_scanline_avx512(buf: &mut [u8]) {
    use std::arch::x86_64::*;

    let len = buf.len();
    let chunks = len / 64; // 16 pixels = 64 bytes
    let mut offset = 0;

    // Mask: 0xFF for BGR channels, 0x00 for alpha.
    // In BGRA little-endian each 32-bit pixel is [B, G, R, A] → 0x00FFFFFF.
    let mask = _mm512_set1_epi32(0x00FFFFFF_u32 as i32);

    for _ in 0..chunks {
        let ptr = buf.as_mut_ptr().add(offset) as *mut __m512i;
        let v = _mm512_loadu_si512(ptr as *const __m512i);
        let result = _mm512_xor_si512(v, mask);
        _mm512_storeu_si512(ptr, result);
        offset += 64;
    }

    // Fall through to SSE2 for remaining pixels
    if offset < len {
        invert_scanline_sse2(&mut buf[offset..]);
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn invert_scanline_sse2(buf: &mut [u8]) {
    use std::arch::x86_64::*;

    let len = buf.len();
    let chunks = len / 16;
    let mut offset = 0;

    // Mask: FF for BGR channels, 00 for alpha → XOR inverts only color
    #[rustfmt::skip]
    let mask = _mm_set_epi8(
        0x00, -1, -1, -1,   // pixel 3: A=keep, R=invert, G=invert, B=invert
        0x00, -1, -1, -1,   // pixel 2
        0x00, -1, -1, -1,   // pixel 1
        0x00, -1, -1, -1,   // pixel 0
    );

    for _ in 0..chunks {
        let ptr = buf.as_mut_ptr().add(offset) as *mut __m128i;
        let v = _mm_loadu_si128(ptr);
        let result = _mm_xor_si128(v, mask);
        _mm_storeu_si128(ptr, result);
        offset += 16;
    }

    // Scalar tail
    let remaining = (len - offset) / 4;
    for i in 0..remaining {
        let off = offset + i * 4;
        buf[off] = 255 - buf[off];
        buf[off + 1] = 255 - buf[off + 1];
        buf[off + 2] = 255 - buf[off + 2];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pixel(b: u8, g: u8, r: u8, a: u8) -> [u8; 4] {
        [b, g, r, a]
    }

    #[test]
    fn src_over_opaque_replaces() {
        let mut dst = make_pixel(100, 100, 100, 255).to_vec();
        let src = make_pixel(50, 200, 50, 255);
        blend_scanline_src_over(&mut dst, &src);
        assert_eq!(dst, src);
    }

    #[test]
    fn src_over_transparent_noop() {
        let original = make_pixel(100, 100, 100, 255);
        let mut dst = original.to_vec();
        let src = make_pixel(0, 0, 0, 0);
        blend_scanline_src_over(&mut dst, &src);
        assert_eq!(&dst, &original);
    }

    #[test]
    fn src_over_half_alpha() {
        let mut dst = make_pixel(0, 0, 0, 255).to_vec();
        let src = make_pixel(128, 128, 128, 128).to_vec();
        blend_scanline_src_over(&mut dst, &src);
        // src + dst * (1 - 128/255)
        // 128 + 0 * 127/255 = 128
        assert_eq!(dst[0], 128);
    }

    #[test]
    fn src_over_matches_scalar() {
        let mut dst_scalar = vec![0u8; 64];
        let mut dst_auto = vec![0u8; 64];
        let src: Vec<u8> = (0..64).collect();

        // Fill dst with known pattern
        for i in 0..64 {
            dst_scalar[i] = (i as u8).wrapping_mul(3);
            dst_auto[i] = dst_scalar[i];
        }

        blend_scanline_src_over_scalar(&mut dst_scalar, &src);
        blend_scanline_src_over(&mut dst_auto, &src);

        // Allow ±1 difference from /255 approximation
        for i in 0..64 {
            let diff = (dst_scalar[i] as i16 - dst_auto[i] as i16).abs();
            assert!(diff <= 1, "mismatch at {i}: scalar={} auto={}", dst_scalar[i], dst_auto[i]);
        }
    }

    #[test]
    fn multiply_white_identity() {
        let mut dst = make_pixel(100, 150, 200, 255).to_vec();
        let src = make_pixel(255, 255, 255, 255);
        blend_scanline_multiply(&mut dst, &src);
        assert_eq!(dst[0], 100);
        assert_eq!(dst[1], 150);
        assert_eq!(dst[2], 200);
    }

    #[test]
    fn multiply_black_yields_black() {
        let mut dst = make_pixel(200, 150, 100, 255).to_vec();
        let src = make_pixel(0, 0, 0, 0);
        blend_scanline_multiply(&mut dst, &src);
        assert_eq!(&dst, &[0, 0, 0, 0]);
    }

    #[test]
    fn darken_picks_min() {
        let mut dst = make_pixel(200, 50, 150, 255).to_vec();
        let src = make_pixel(100, 100, 100, 200);
        blend_scanline_darken(&mut dst, &src);
        assert_eq!(dst, [100, 50, 100, 200]);
    }

    #[test]
    fn lighten_picks_max() {
        let mut dst = make_pixel(200, 50, 150, 255).to_vec();
        let src = make_pixel(100, 100, 100, 200);
        blend_scanline_lighten(&mut dst, &src);
        assert_eq!(dst, [200, 100, 150, 255]);
    }

    #[test]
    fn difference_abs_value() {
        let mut dst = make_pixel(200, 50, 100, 255).to_vec();
        let src = make_pixel(100, 150, 100, 255);
        blend_scanline_difference(&mut dst, &src);
        assert_eq!(dst, [100, 100, 0, 0]);
    }

    #[test]
    fn screen_white_saturates() {
        let mut dst = make_pixel(100, 100, 100, 255).to_vec();
        let src = make_pixel(255, 255, 255, 255);
        blend_scanline_screen(&mut dst, &src);
        assert_eq!(dst[0], 255);
        assert_eq!(dst[1], 255);
        assert_eq!(dst[2], 255);
    }

    #[test]
    fn invert_rgb_preserves_alpha() {
        let mut buf = make_pixel(200, 100, 50, 128).to_vec();
        invert_scanline(&mut buf);
        assert_eq!(buf, [55, 155, 205, 128]);
    }

    #[test]
    fn invert_avx512_large_buffer() {
        // 256 pixels = 1024 bytes — exercises multiple AVX-512 iterations
        let pixel_count = 256;
        let len = pixel_count * 4;
        let mut buf: Vec<u8> = (0..len).map(|i| (i % 256) as u8).collect();
        let original = buf.clone();

        invert_scanline(&mut buf);

        for i in 0..pixel_count {
            let off = i * 4;
            assert_eq!(buf[off], 255 - original[off], "B mismatch at pixel {i}");
            assert_eq!(buf[off + 1], 255 - original[off + 1], "G mismatch at pixel {i}");
            assert_eq!(buf[off + 2], 255 - original[off + 2], "R mismatch at pixel {i}");
            assert_eq!(buf[off + 3], original[off + 3], "alpha should be preserved at pixel {i}");
        }
    }

    #[test]
    fn invert_avx512_tail_handling() {
        // 19 pixels (76 bytes) — 1 AVX-512 chunk of 16 pixels + 3 pixel tail via SSE2
        let pixel_count = 19;
        let len = pixel_count * 4;
        let mut buf: Vec<u8> = (0..len).map(|i| ((i * 3 + 7) % 256) as u8).collect();
        let original = buf.clone();

        invert_scanline(&mut buf);

        for i in 0..pixel_count {
            let off = i * 4;
            assert_eq!(buf[off], 255 - original[off]);
            assert_eq!(buf[off + 1], 255 - original[off + 1]);
            assert_eq!(buf[off + 2], 255 - original[off + 2]);
            assert_eq!(buf[off + 3], original[off + 3]);
        }
    }

    #[test]
    fn invert_double_is_identity() {
        // Inverting twice should restore the original (alpha always preserved)
        let pixel_count = 100;
        let len = pixel_count * 4;
        let mut buf: Vec<u8> = (0..len).map(|i| (i % 256) as u8).collect();
        let original = buf.clone();

        invert_scanline(&mut buf);
        invert_scanline(&mut buf);

        assert_eq!(buf, original);
    }

    #[test]
    fn large_buffer_consistency() {
        // Test with many pixels to exercise SIMD + tail loop
        let pixel_count = 137; // not divisible by 4 or 8
        let len = pixel_count * 4;
        let mut dst = vec![128u8; len];
        let src: Vec<u8> = (0..len).map(|i| (i % 256) as u8).collect();

        let mut dst_scalar = dst.clone();
        blend_scanline_src_over_scalar(&mut dst_scalar, &src);
        blend_scanline_src_over(&mut dst, &src);

        for i in 0..len {
            let diff = (dst[i] as i16 - dst_scalar[i] as i16).abs();
            assert!(diff <= 1, "pixel byte {i}: simd={} scalar={}", dst[i], dst_scalar[i]);
        }
    }

    #[test]
    fn src_over_avx512_matches_scalar() {
        // Large buffer (256 pixels) to exercise the AVX-512 path (16 pixels / 64 bytes per iteration)
        let pixel_count = 256;
        let len = pixel_count * 4;
        let mut dst_scalar = vec![0u8; len];
        let mut dst_auto = vec![0u8; len];
        let src: Vec<u8> = (0..len).map(|i| (i % 256) as u8).collect();

        // Fill dst with a known pattern
        for i in 0..len {
            dst_scalar[i] = (i as u8).wrapping_mul(7).wrapping_add(31);
            dst_auto[i] = dst_scalar[i];
        }

        blend_scanline_src_over_scalar(&mut dst_scalar, &src);
        blend_scanline_src_over(&mut dst_auto, &src);

        // Allow ±1 difference from /255 approximation
        for i in 0..len {
            let diff = (dst_scalar[i] as i16 - dst_auto[i] as i16).abs();
            assert!(
                diff <= 1,
                "mismatch at byte {i}: scalar={} auto={}",
                dst_scalar[i],
                dst_auto[i]
            );
        }
    }
}
