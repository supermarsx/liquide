//! SIMD-accelerated alpha blending for BGRA8 scanlines.
//!
//! The primary entry point is [`blend_scanline_src_over`] which processes
//! 4 pixels at a time on SSE2, 8 on AVX2, with a scalar tail loop.

/// Blend `src` over `dst` using premultiplied-alpha Porter-Duff SrcOver.
///
/// Both slices must be BGRA8 (length divisible by 4) and equal length.
/// Formula per channel: `out = src + dst * (1 - src_alpha)`
pub fn blend_scanline_src_over(dst: &mut [u8], src: &[u8]) {
    assert_eq!(dst.len(), src.len());
    assert_eq!(dst.len() % 4, 0);

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

/// Blend `src` onto `dst` using premultiplied-alpha Multiply mode.
///
/// Per-channel formula (premultiplied):
/// ```text
/// out_c = (src_c * dst_c + src_c * (255 - dst_a) + dst_c * (255 - src_a) + 127) / 255
/// out_a = src_a + dst_a - (src_a * dst_a + 127) / 255
/// ```
pub fn blend_scanline_multiply(dst: &mut [u8], src: &[u8]) {
    assert_eq!(dst.len(), src.len());
    assert_eq!(dst.len() % 4, 0);

    #[cfg(target_arch = "x86_64")]
    {
        if crate::detect::has_avx2() {
            unsafe { return blend_scanline_multiply_avx2(dst, src) }
        }
        unsafe { return blend_scanline_multiply_sse2(dst, src) }
    }
    #[cfg(not(target_arch = "x86_64"))]
    blend_scanline_multiply_scalar(dst, src);
}

/// Scalar premultiplied-alpha Multiply blend.
pub fn blend_scanline_multiply_scalar(dst: &mut [u8], src: &[u8]) {
    #[inline(always)]
    fn div255(x: u32) -> u32 {
        (x + 128 + ((x + 128) >> 8)) >> 8
    }

    let pixels = dst.len() / 4;
    for i in 0..pixels {
        let off = i * 4;
        let sa = src[off + 3] as u32;
        if sa == 0 { continue; } // Fully transparent source — dst unchanged
        let da = dst[off + 3] as u32;
        let inv_sa = 255 - sa;
        let inv_da = 255 - da;

        // RGB channels: div255(s*d) + div255(s*inv_da) + div255(d*inv_sa)
        // Separate div255 per term matches the SIMD path and avoids u16 overflow.
        for c in 0..3 {
            let s = src[off + c] as u32;
            let d = dst[off + c] as u32;
            let val = div255(s * d) + div255(s * inv_da) + div255(d * inv_sa);
            dst[off + c] = val.min(255) as u8;
        }
        // Alpha: SrcOver formula
        dst[off + 3] = (sa + da - div255(sa * da)).min(255) as u8;
    }
}

/// SSE2 premultiplied-alpha Multiply: 4 BGRA pixels per iteration.
///
/// For RGB lanes: `out = (s*d + s*(255-da) + d*(255-sa) + 128) / 255`
/// For alpha lane: `out = s_a + d_a - (s_a * d_a + 128) / 255`
///
/// We compute a combined expression for all 4 channels, then fix up alpha.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn blend_scanline_multiply_sse2(dst: &mut [u8], src: &[u8]) {
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

        // Unpack u8 → u16 (low 2 pixels, high 2 pixels)
        let s_lo = _mm_unpacklo_epi8(s_packed, zero);
        let s_hi = _mm_unpackhi_epi8(s_packed, zero);
        let d_lo = _mm_unpacklo_epi8(d_packed, zero);
        let d_hi = _mm_unpackhi_epi8(d_packed, zero);

        // Broadcast src alpha across each pixel's 4 channels
        let sa_lo = _mm_shufflehi_epi16::<0xFF>(_mm_shufflelo_epi16::<0xFF>(s_lo));
        let sa_hi = _mm_shufflehi_epi16::<0xFF>(_mm_shufflelo_epi16::<0xFF>(s_hi));
        // Broadcast dst alpha
        let da_lo = _mm_shufflehi_epi16::<0xFF>(_mm_shufflelo_epi16::<0xFF>(d_lo));
        let da_hi = _mm_shufflehi_epi16::<0xFF>(_mm_shufflelo_epi16::<0xFF>(d_hi));

        // inv_sa = 255 - src_alpha, inv_da = 255 - dst_alpha
        let inv_sa_lo = _mm_sub_epi16(all_ff, sa_lo);
        let inv_sa_hi = _mm_sub_epi16(all_ff, sa_hi);
        let inv_da_lo = _mm_sub_epi16(all_ff, da_lo);
        let inv_da_hi = _mm_sub_epi16(all_ff, da_hi);

        // Compute each term and divide by 255 separately to avoid u16 overflow.
        // For non-premultiplied inputs, the sum of three products can exceed 65535.
        // div255(x) = (x + 128 + ((x + 128) >> 8)) >> 8

        // term1 = div255(s * d)
        let sd_lo = _mm_mullo_epi16(s_lo, d_lo);
        let sd_hi = _mm_mullo_epi16(s_hi, d_hi);
        let b1_lo = _mm_add_epi16(sd_lo, half);
        let b1_hi = _mm_add_epi16(sd_hi, half);
        let t1_lo = _mm_srli_epi16::<8>(_mm_add_epi16(b1_lo, _mm_srli_epi16::<8>(b1_lo)));
        let t1_hi = _mm_srli_epi16::<8>(_mm_add_epi16(b1_hi, _mm_srli_epi16::<8>(b1_hi)));

        // term2 = div255(s * inv_da)
        let s_invda_lo = _mm_mullo_epi16(s_lo, inv_da_lo);
        let s_invda_hi = _mm_mullo_epi16(s_hi, inv_da_hi);
        let b2_lo = _mm_add_epi16(s_invda_lo, half);
        let b2_hi = _mm_add_epi16(s_invda_hi, half);
        let t2_lo = _mm_srli_epi16::<8>(_mm_add_epi16(b2_lo, _mm_srli_epi16::<8>(b2_lo)));
        let t2_hi = _mm_srli_epi16::<8>(_mm_add_epi16(b2_hi, _mm_srli_epi16::<8>(b2_hi)));

        // term3 = div255(d * inv_sa)
        let d_invsa_lo = _mm_mullo_epi16(d_lo, inv_sa_lo);
        let d_invsa_hi = _mm_mullo_epi16(d_hi, inv_sa_hi);
        let b3_lo = _mm_add_epi16(d_invsa_lo, half);
        let b3_hi = _mm_add_epi16(d_invsa_hi, half);
        let t3_lo = _mm_srli_epi16::<8>(_mm_add_epi16(b3_lo, _mm_srli_epi16::<8>(b3_lo)));
        let t3_hi = _mm_srli_epi16::<8>(_mm_add_epi16(b3_hi, _mm_srli_epi16::<8>(b3_hi)));

        // rgb = term1 + term2 + term3 (each <= 255, sum <= 765, fits u16)
        let rgb_lo = _mm_add_epi16(_mm_add_epi16(t1_lo, t2_lo), t3_lo);
        let rgb_hi = _mm_add_epi16(_mm_add_epi16(t1_hi, t2_hi), t3_hi);

        // Alpha: SrcOver = sa + da - (sa * da + 128) / 255
        let sa_da_lo = _mm_mullo_epi16(sa_lo, da_lo);
        let sa_da_hi = _mm_mullo_epi16(sa_hi, da_hi);
        let a_biased_lo = _mm_add_epi16(sa_da_lo, half);
        let a_biased_hi = _mm_add_epi16(sa_da_hi, half);
        let a_div_lo = _mm_srli_epi16::<8>(_mm_add_epi16(a_biased_lo, _mm_srli_epi16::<8>(a_biased_lo)));
        let a_div_hi = _mm_srli_epi16::<8>(_mm_add_epi16(a_biased_hi, _mm_srli_epi16::<8>(a_biased_hi)));
        let alpha_lo = _mm_sub_epi16(_mm_add_epi16(sa_lo, da_lo), a_div_lo);
        let alpha_hi = _mm_sub_epi16(_mm_add_epi16(sa_hi, da_hi), a_div_hi);

        // Merge: use rgb result for B,G,R and alpha result for A channel
        // Alpha is lane 3 and 7 (for the two pixels in each half).
        // Mask: 0xFFFF in alpha lanes, 0x0000 in RGB lanes.
        // Build mask from shuffled alpha. We want: rgb where mask=0, alpha where mask=FF.
        // Use: (alpha & mask) | (rgb & ~mask)  — but SSE2 lacks _mm_blendv.
        // Alternative: just overwrite the alpha lanes.
        // Alpha mask: lane 3=0xFFFF, lane 7=0xFFFF, rest=0
        let alpha_mask = _mm_set_epi16(-1, 0, 0, 0, -1, 0, 0, 0);
        let out_lo = _mm_or_si128(
            _mm_and_si128(alpha_mask, alpha_lo),
            _mm_andnot_si128(alpha_mask, rgb_lo),
        );
        let out_hi = _mm_or_si128(
            _mm_and_si128(alpha_mask, alpha_hi),
            _mm_andnot_si128(alpha_mask, rgb_hi),
        );

        let result = _mm_packus_epi16(out_lo, out_hi);
        _mm_storeu_si128(d_ptr, result);

        offset += 16;
    }

    if offset < len {
        blend_scanline_multiply_scalar(&mut dst[offset..], &src[offset..]);
    }
}

/// AVX2 premultiplied-alpha Multiply: 8 BGRA pixels per iteration.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn blend_scanline_multiply_avx2(dst: &mut [u8], src: &[u8]) {
    use std::arch::x86_64::*;

    let len = dst.len();
    let chunks = len / 32; // 8 pixels = 32 bytes
    let mut offset = 0;

    let zero = _mm256_setzero_si256();
    let all_ff = _mm256_set1_epi16(0x00FF);
    let half = _mm256_set1_epi16(128);
    let alpha_mask = _mm256_set_epi16(-1, 0, 0, 0, -1, 0, 0, 0, -1, 0, 0, 0, -1, 0, 0, 0);

    for _ in 0..chunks {
        let d_ptr = dst.as_mut_ptr().add(offset) as *mut __m256i;
        let s_ptr = src.as_ptr().add(offset) as *const __m256i;

        let s_packed = _mm256_loadu_si256(s_ptr);
        let d_packed = _mm256_loadu_si256(d_ptr);

        let s_lo = _mm256_unpacklo_epi8(s_packed, zero);
        let s_hi = _mm256_unpackhi_epi8(s_packed, zero);
        let d_lo = _mm256_unpacklo_epi8(d_packed, zero);
        let d_hi = _mm256_unpackhi_epi8(d_packed, zero);

        // Broadcast alpha channels
        let sa_lo = _mm256_shufflehi_epi16::<0xFF>(_mm256_shufflelo_epi16::<0xFF>(s_lo));
        let sa_hi = _mm256_shufflehi_epi16::<0xFF>(_mm256_shufflelo_epi16::<0xFF>(s_hi));
        let da_lo = _mm256_shufflehi_epi16::<0xFF>(_mm256_shufflelo_epi16::<0xFF>(d_lo));
        let da_hi = _mm256_shufflehi_epi16::<0xFF>(_mm256_shufflelo_epi16::<0xFF>(d_hi));

        let inv_sa_lo = _mm256_sub_epi16(all_ff, sa_lo);
        let inv_sa_hi = _mm256_sub_epi16(all_ff, sa_hi);
        let inv_da_lo = _mm256_sub_epi16(all_ff, da_lo);
        let inv_da_hi = _mm256_sub_epi16(all_ff, da_hi);

        // Compute each term and divide by 255 separately to avoid u16 overflow.
        // div255(x) = (x + 128 + ((x + 128) >> 8)) >> 8

        let sd_lo = _mm256_mullo_epi16(s_lo, d_lo);
        let sd_hi = _mm256_mullo_epi16(s_hi, d_hi);
        let b1_lo = _mm256_add_epi16(sd_lo, half);
        let b1_hi = _mm256_add_epi16(sd_hi, half);
        let t1_lo = _mm256_srli_epi16::<8>(_mm256_add_epi16(b1_lo, _mm256_srli_epi16::<8>(b1_lo)));
        let t1_hi = _mm256_srli_epi16::<8>(_mm256_add_epi16(b1_hi, _mm256_srli_epi16::<8>(b1_hi)));

        let s_invda_lo = _mm256_mullo_epi16(s_lo, inv_da_lo);
        let s_invda_hi = _mm256_mullo_epi16(s_hi, inv_da_hi);
        let b2_lo = _mm256_add_epi16(s_invda_lo, half);
        let b2_hi = _mm256_add_epi16(s_invda_hi, half);
        let t2_lo = _mm256_srli_epi16::<8>(_mm256_add_epi16(b2_lo, _mm256_srli_epi16::<8>(b2_lo)));
        let t2_hi = _mm256_srli_epi16::<8>(_mm256_add_epi16(b2_hi, _mm256_srli_epi16::<8>(b2_hi)));

        let d_invsa_lo = _mm256_mullo_epi16(d_lo, inv_sa_lo);
        let d_invsa_hi = _mm256_mullo_epi16(d_hi, inv_sa_hi);
        let b3_lo = _mm256_add_epi16(d_invsa_lo, half);
        let b3_hi = _mm256_add_epi16(d_invsa_hi, half);
        let t3_lo = _mm256_srli_epi16::<8>(_mm256_add_epi16(b3_lo, _mm256_srli_epi16::<8>(b3_lo)));
        let t3_hi = _mm256_srli_epi16::<8>(_mm256_add_epi16(b3_hi, _mm256_srli_epi16::<8>(b3_hi)));

        let rgb_lo = _mm256_add_epi16(_mm256_add_epi16(t1_lo, t2_lo), t3_lo);
        let rgb_hi = _mm256_add_epi16(_mm256_add_epi16(t1_hi, t2_hi), t3_hi);

        // Alpha: sa + da - (sa*da + 128)/255
        let sa_da_lo = _mm256_mullo_epi16(sa_lo, da_lo);
        let sa_da_hi = _mm256_mullo_epi16(sa_hi, da_hi);
        let a_biased_lo = _mm256_add_epi16(sa_da_lo, half);
        let a_biased_hi = _mm256_add_epi16(sa_da_hi, half);
        let a_div_lo = _mm256_srli_epi16::<8>(_mm256_add_epi16(a_biased_lo, _mm256_srli_epi16::<8>(a_biased_lo)));
        let a_div_hi = _mm256_srli_epi16::<8>(_mm256_add_epi16(a_biased_hi, _mm256_srli_epi16::<8>(a_biased_hi)));
        let alpha_lo = _mm256_sub_epi16(_mm256_add_epi16(sa_lo, da_lo), a_div_lo);
        let alpha_hi = _mm256_sub_epi16(_mm256_add_epi16(sa_hi, da_hi), a_div_hi);

        // Merge RGB and alpha
        let out_lo = _mm256_or_si256(
            _mm256_and_si256(alpha_mask, alpha_lo),
            _mm256_andnot_si256(alpha_mask, rgb_lo),
        );
        let out_hi = _mm256_or_si256(
            _mm256_and_si256(alpha_mask, alpha_hi),
            _mm256_andnot_si256(alpha_mask, rgb_hi),
        );

        let result = _mm256_packus_epi16(out_lo, out_hi);
        _mm256_storeu_si256(d_ptr, result);

        offset += 32;
    }

    // SSE2 for remaining 4-pixel chunks
    if offset + 16 <= len {
        blend_scanline_multiply_sse2(&mut dst[offset..], &src[offset..]);
    } else if offset < len {
        blend_scanline_multiply_scalar(&mut dst[offset..], &src[offset..]);
    }
}

/// Darken blend on a BGRA8 scanline: `out = min(src, dst)` per channel.
pub fn blend_scanline_darken(dst: &mut [u8], src: &[u8]) {
    assert_eq!(dst.len(), src.len());
    assert_eq!(dst.len() % 4, 0);

    #[cfg(target_arch = "x86_64")]
    {
        if crate::detect::has_avx512() {
            unsafe { return blend_scanline_darken_avx512(dst, src) }
        }
        unsafe { return blend_scanline_darken_sse2(dst, src) }
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
            d[0] = d[0].min(s[0]); // B
            d[1] = d[1].min(s[1]); // G
            d[2] = d[2].min(s[2]); // R
            d[3] = d[3].max(s[3]); // A — use max, not min
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

    // Alpha mask: 0xFF at every 4th byte (alpha channel in BGRA), 0x00 elsewhere
    let alpha_mask = _mm512_set1_epi32(i32::from_ne_bytes([0x00, 0x00, 0x00, 0xFF_u8 as u8]));

    for _ in 0..chunks {
        let d_ptr = dst.as_mut_ptr().add(offset) as *mut __m512i;
        let s_ptr = src.as_ptr().add(offset) as *const __m512i;
        let d = _mm512_loadu_si512(d_ptr);
        let s = _mm512_loadu_si512(s_ptr);
        let rgb_blend = _mm512_min_epu8(d, s);   // min for BGR
        let alpha_blend = _mm512_max_epu8(d, s);  // max for alpha
        // Select: alpha bytes from alpha_blend, RGB bytes from rgb_blend
        let result = _mm512_or_si512(
            _mm512_and_si512(alpha_mask, alpha_blend),
            _mm512_andnot_si512(alpha_mask, rgb_blend),
        );
        _mm512_storeu_si512(d_ptr, result);
        offset += 64;
    }

    // Scalar tail: process remaining pixels in 4-byte BGRA chunks
    for chunk in (offset..len).step_by(4) {
        if chunk + 3 < len {
            dst[chunk]     = dst[chunk].min(src[chunk]);       // B
            dst[chunk + 1] = dst[chunk + 1].min(src[chunk + 1]); // G
            dst[chunk + 2] = dst[chunk + 2].min(src[chunk + 2]); // R
            dst[chunk + 3] = dst[chunk + 3].max(src[chunk + 3]); // A
        }
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn blend_scanline_darken_sse2(dst: &mut [u8], src: &[u8]) {
    use std::arch::x86_64::*;

    let len = dst.len();
    let chunks = len / 16;
    let mut offset = 0;

    // Alpha mask: 0xFF at every 4th byte (alpha channel in BGRA), 0x00 elsewhere
    let alpha_mask = _mm_set1_epi32(i32::from_ne_bytes([0x00, 0x00, 0x00, 0xFF_u8 as u8]));

    for _ in 0..chunks {
        let d_ptr = dst.as_mut_ptr().add(offset) as *mut __m128i;
        let s_ptr = src.as_ptr().add(offset) as *const __m128i;
        let d = _mm_loadu_si128(d_ptr);
        let s = _mm_loadu_si128(s_ptr);
        let rgb_blend = _mm_min_epu8(d, s);   // min for BGR
        let alpha_blend = _mm_max_epu8(d, s);  // max for alpha
        let result = _mm_or_si128(
            _mm_and_si128(alpha_mask, alpha_blend),
            _mm_andnot_si128(alpha_mask, rgb_blend),
        );
        _mm_storeu_si128(d_ptr, result);
        offset += 16;
    }

    // Scalar tail: process remaining pixels in 4-byte BGRA chunks
    for chunk in (offset..len).step_by(4) {
        if chunk + 3 < len {
            dst[chunk]     = dst[chunk].min(src[chunk]);       // B
            dst[chunk + 1] = dst[chunk + 1].min(src[chunk + 1]); // G
            dst[chunk + 2] = dst[chunk + 2].min(src[chunk + 2]); // R
            dst[chunk + 3] = dst[chunk + 3].max(src[chunk + 3]); // A
        }
    }
}

/// Lighten blend on a BGRA8 scanline: `out = max(src, dst)` per channel.
pub fn blend_scanline_lighten(dst: &mut [u8], src: &[u8]) {
    assert_eq!(dst.len(), src.len());
    assert_eq!(dst.len() % 4, 0);

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
    assert_eq!(dst.len(), src.len());
    assert_eq!(dst.len() % 4, 0);

    #[cfg(target_arch = "x86_64")]
    {
        unsafe { return blend_scanline_difference_sse2(dst, src) }
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
            d[0] = (d[0] as i16 - s[0] as i16).unsigned_abs() as u8; // B
            d[1] = (d[1] as i16 - s[1] as i16).unsigned_abs() as u8; // G
            d[2] = (d[2] as i16 - s[2] as i16).unsigned_abs() as u8; // R
            d[3] = d[3].max(s[3]); // A — use max, not abs_diff
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
        let max_val = _mm_max_epu8(d, s);
        let min_val = _mm_min_epu8(d, s);
        let diff = _mm_sub_epi8(max_val, min_val);
        // Alpha channel should be max(d, s), not abs_diff
        let alpha_mask = _mm_set1_epi32(i32::from_ne_bytes([0x00, 0x00, 0x00, 0xFF_u8 as u8]));
        let result = _mm_or_si128(
            _mm_and_si128(alpha_mask, max_val),
            _mm_andnot_si128(alpha_mask, diff),
        );
        _mm_storeu_si128(d_ptr, result);
        offset += 16;
    }

    // Scalar tail: process remaining pixels in 4-byte BGRA chunks
    for chunk in (offset..len).step_by(4) {
        if chunk + 3 < len {
            dst[chunk]     = (dst[chunk] as i16 - src[chunk] as i16).unsigned_abs() as u8;     // B
            dst[chunk + 1] = (dst[chunk + 1] as i16 - src[chunk + 1] as i16).unsigned_abs() as u8; // G
            dst[chunk + 2] = (dst[chunk + 2] as i16 - src[chunk + 2] as i16).unsigned_abs() as u8; // R
            dst[chunk + 3] = dst[chunk + 3].max(src[chunk + 3]); // A
        }
    }
}

/// Screen blend on a BGRA8 scanline: `out = src + dst - src * dst / 255`.
pub fn blend_scanline_screen(dst: &mut [u8], src: &[u8]) {
    assert_eq!(dst.len(), src.len());
    assert_eq!(dst.len() % 4, 0);

    #[cfg(target_arch = "x86_64")]
    {
        if crate::detect::has_avx2() {
            unsafe { return blend_scanline_screen_avx2(dst, src) }
        }
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

/// AVX2 Screen blend: 8 BGRA pixels per iteration.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn blend_scanline_screen_avx2(dst: &mut [u8], src: &[u8]) {
    use std::arch::x86_64::*;

    let len = dst.len();
    let chunks = len / 32;
    let mut offset = 0;

    let zero = _mm256_setzero_si256();
    let half = _mm256_set1_epi16(128);

    for _ in 0..chunks {
        let d_ptr = dst.as_mut_ptr().add(offset) as *mut __m256i;
        let s_ptr = src.as_ptr().add(offset) as *const __m256i;

        let s = _mm256_loadu_si256(s_ptr);
        let d = _mm256_loadu_si256(d_ptr);

        let s_lo = _mm256_unpacklo_epi8(s, zero);
        let s_hi = _mm256_unpackhi_epi8(s, zero);
        let d_lo = _mm256_unpacklo_epi8(d, zero);
        let d_hi = _mm256_unpackhi_epi8(d, zero);

        // s * d / 255
        let prod_lo = _mm256_mullo_epi16(s_lo, d_lo);
        let prod_hi = _mm256_mullo_epi16(s_hi, d_hi);
        let biased_lo = _mm256_add_epi16(prod_lo, half);
        let biased_hi = _mm256_add_epi16(prod_hi, half);
        let div_lo = _mm256_srli_epi16::<8>(_mm256_add_epi16(biased_lo, _mm256_srli_epi16::<8>(biased_lo)));
        let div_hi = _mm256_srli_epi16::<8>(_mm256_add_epi16(biased_hi, _mm256_srli_epi16::<8>(biased_hi)));

        // s + d - (s * d / 255)
        let sum_lo = _mm256_add_epi16(s_lo, d_lo);
        let sum_hi = _mm256_add_epi16(s_hi, d_hi);
        let out_lo = _mm256_sub_epi16(sum_lo, div_lo);
        let out_hi = _mm256_sub_epi16(sum_hi, div_hi);

        let result = _mm256_packus_epi16(out_lo, out_hi);
        _mm256_storeu_si256(d_ptr, result);

        offset += 32;
    }

    if offset + 16 <= len {
        blend_scanline_screen_sse2(&mut dst[offset..], &src[offset..]);
    } else if offset < len {
        blend_scanline_screen_scalar(&mut dst[offset..], &src[offset..]);
    }
}

/// Invert a BGRA8 scanline in-place: `R = 255-R, G = 255-G, B = 255-B`, alpha preserved.
pub fn invert_scanline(buf: &mut [u8]) {
    assert_eq!(buf.len() % 4, 0);

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

// ============================================================================
// Overlay blend: Multiply when dst < 128, Screen when dst >= 128
// ============================================================================

/// Overlay blend on a BGRA8 scanline.
///
/// Per channel: `if d < 128 { 2*s*d/255 } else { 255 - 2*(255-s)*(255-d)/255 }`
/// Alpha channel: `max(sa, da)` (matches renderer convention).
pub fn blend_scanline_overlay(dst: &mut [u8], src: &[u8]) {
    assert_eq!(dst.len(), src.len());
    assert_eq!(dst.len() % 4, 0);

    #[cfg(target_arch = "x86_64")]
    {
        unsafe { return blend_scanline_overlay_sse2(dst, src) }
    }
    #[cfg(not(target_arch = "x86_64"))]
    blend_scanline_overlay_scalar(dst, src);
}

fn blend_scanline_overlay_scalar(dst: &mut [u8], src: &[u8]) {
    let pixels = dst.len() / 4;
    for i in 0..pixels {
        let off = i * 4;
        for c in 0..3 {
            let d = dst[off + c] as u16;
            let s = src[off + c] as u16;
            dst[off + c] = if d < 128 {
                ((2 * s * d + 127) / 255) as u8
            } else {
                (255 - (2 * (255 - s) * (255 - d) + 127) / 255) as u8
            };
        }
        // Alpha: max
        dst[off + 3] = dst[off + 3].max(src[off + 3]);
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn blend_scanline_overlay_sse2(dst: &mut [u8], src: &[u8]) {
    use std::arch::x86_64::*;

    let len = dst.len();
    let chunks = len / 16;
    let mut offset = 0;

    let zero = _mm_setzero_si128();
    let half = _mm_set1_epi16(128);
    let all_ff = _mm_set1_epi16(255);
    let threshold = _mm_set1_epi16(128); // d < 128 threshold

    for _ in 0..chunks {
        let d_ptr = dst.as_mut_ptr().add(offset) as *mut __m128i;
        let s_ptr = src.as_ptr().add(offset) as *const __m128i;

        let s = _mm_loadu_si128(s_ptr);
        let d = _mm_loadu_si128(d_ptr);

        let s_lo = _mm_unpacklo_epi8(s, zero);
        let s_hi = _mm_unpackhi_epi8(s, zero);
        let d_lo = _mm_unpacklo_epi8(d, zero);
        let d_hi = _mm_unpackhi_epi8(d, zero);

        // Multiply path: 2*s*d/255
        // Compute s*d/255 first to avoid u16 overflow, then double
        let prod_lo = _mm_mullo_epi16(s_lo, d_lo);
        let biased_lo = _mm_add_epi16(prod_lo, half);
        let div_lo = _mm_srli_epi16::<8>(_mm_add_epi16(biased_lo, _mm_srli_epi16::<8>(biased_lo)));
        let mul_result_lo = _mm_add_epi16(div_lo, div_lo); // 2 * (s*d/255)

        let prod_hi = _mm_mullo_epi16(s_hi, d_hi);
        let biased_hi = _mm_add_epi16(prod_hi, half);
        let div_hi = _mm_srli_epi16::<8>(_mm_add_epi16(biased_hi, _mm_srli_epi16::<8>(biased_hi)));
        let mul_result_hi = _mm_add_epi16(div_hi, div_hi);

        // Screen path: 255 - 2*(255-s)*(255-d)/255
        let inv_s_lo = _mm_sub_epi16(all_ff, s_lo);
        let inv_s_hi = _mm_sub_epi16(all_ff, s_hi);
        let inv_d_lo = _mm_sub_epi16(all_ff, d_lo);
        let inv_d_hi = _mm_sub_epi16(all_ff, d_hi);

        let scr_prod_lo = _mm_mullo_epi16(inv_s_lo, inv_d_lo);
        let scr_biased_lo = _mm_add_epi16(scr_prod_lo, half);
        let scr_div_lo = _mm_srli_epi16::<8>(_mm_add_epi16(scr_biased_lo, _mm_srli_epi16::<8>(scr_biased_lo)));
        let scr_result_lo = _mm_sub_epi16(all_ff, _mm_add_epi16(scr_div_lo, scr_div_lo));

        let scr_prod_hi = _mm_mullo_epi16(inv_s_hi, inv_d_hi);
        let scr_biased_hi = _mm_add_epi16(scr_prod_hi, half);
        let scr_div_hi = _mm_srli_epi16::<8>(_mm_add_epi16(scr_biased_hi, _mm_srli_epi16::<8>(scr_biased_hi)));
        let scr_result_hi = _mm_sub_epi16(all_ff, _mm_add_epi16(scr_div_hi, scr_div_hi));

        // mask: 0xFFFF where d < 128 (multiply path), 0 where d >= 128 (screen path)
        let mask_lo = _mm_cmplt_epi16(d_lo, threshold);
        let mask_hi = _mm_cmplt_epi16(d_hi, threshold);

        // Select: (mul & mask) | (scr & ~mask)
        let out_lo = _mm_or_si128(
            _mm_and_si128(mask_lo, mul_result_lo),
            _mm_andnot_si128(mask_lo, scr_result_lo),
        );
        let out_hi = _mm_or_si128(
            _mm_and_si128(mask_hi, mul_result_hi),
            _mm_andnot_si128(mask_hi, scr_result_hi),
        );

        let result = _mm_packus_epi16(out_lo, out_hi);

        // Preserve alpha as max(da, sa)
        let alpha_mask = _mm_set_epi8(-1, 0, 0, 0, -1, 0, 0, 0, -1, 0, 0, 0, -1, 0, 0, 0);
        let max_alpha = _mm_max_epu8(d, s);
        let result = _mm_or_si128(
            _mm_andnot_si128(alpha_mask, result), // color channels from blend
            _mm_and_si128(alpha_mask, max_alpha),  // alpha from max
        );

        _mm_storeu_si128(d_ptr, result);
        offset += 16;
    }

    if offset < len {
        blend_scanline_overlay_scalar(&mut dst[offset..], &src[offset..]);
    }
}

// ============================================================================
// Hard Light blend: overlay with swapped src/dst roles
// ============================================================================

/// Hard Light blend on a BGRA8 scanline.
///
/// Same as Overlay but the condition tests src instead of dst:
/// `if s < 128 { 2*s*d/255 } else { 255 - 2*(255-s)*(255-d)/255 }`
pub fn blend_scanline_hard_light(dst: &mut [u8], src: &[u8]) {
    assert_eq!(dst.len(), src.len());
    assert_eq!(dst.len() % 4, 0);

    #[cfg(target_arch = "x86_64")]
    {
        unsafe { return blend_scanline_hard_light_sse2(dst, src) }
    }
    #[cfg(not(target_arch = "x86_64"))]
    blend_scanline_hard_light_scalar(dst, src);
}

fn blend_scanline_hard_light_scalar(dst: &mut [u8], src: &[u8]) {
    let pixels = dst.len() / 4;
    for i in 0..pixels {
        let off = i * 4;
        for c in 0..3 {
            let d = dst[off + c] as u16;
            let s = src[off + c] as u16;
            // Hard light: condition on src, not dst
            dst[off + c] = if s < 128 {
                ((2 * s * d + 127) / 255) as u8
            } else {
                (255 - (2 * (255 - s) * (255 - d) + 127) / 255) as u8
            };
        }
        dst[off + 3] = dst[off + 3].max(src[off + 3]);
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn blend_scanline_hard_light_sse2(dst: &mut [u8], src: &[u8]) {
    use std::arch::x86_64::*;

    let len = dst.len();
    let chunks = len / 16;
    let mut offset = 0;

    let zero = _mm_setzero_si128();
    let half = _mm_set1_epi16(128);
    let all_ff = _mm_set1_epi16(255);
    let threshold = _mm_set1_epi16(128);

    for _ in 0..chunks {
        let d_ptr = dst.as_mut_ptr().add(offset) as *mut __m128i;
        let s_ptr = src.as_ptr().add(offset) as *const __m128i;

        let s = _mm_loadu_si128(s_ptr);
        let d = _mm_loadu_si128(d_ptr);

        let s_lo = _mm_unpacklo_epi8(s, zero);
        let s_hi = _mm_unpackhi_epi8(s, zero);
        let d_lo = _mm_unpacklo_epi8(d, zero);
        let d_hi = _mm_unpackhi_epi8(d, zero);

        // Multiply path: 2*s*d/255 — compute s*d/255 first, then double
        let prod_lo = _mm_mullo_epi16(s_lo, d_lo);
        let biased_lo = _mm_add_epi16(prod_lo, half);
        let div_lo = _mm_srli_epi16::<8>(_mm_add_epi16(biased_lo, _mm_srli_epi16::<8>(biased_lo)));
        let mul_result_lo = _mm_add_epi16(div_lo, div_lo);

        let prod_hi = _mm_mullo_epi16(s_hi, d_hi);
        let biased_hi = _mm_add_epi16(prod_hi, half);
        let div_hi = _mm_srli_epi16::<8>(_mm_add_epi16(biased_hi, _mm_srli_epi16::<8>(biased_hi)));
        let mul_result_hi = _mm_add_epi16(div_hi, div_hi);

        // Screen path: 255 - 2*(255-s)*(255-d)/255
        let inv_s_lo = _mm_sub_epi16(all_ff, s_lo);
        let inv_s_hi = _mm_sub_epi16(all_ff, s_hi);
        let inv_d_lo = _mm_sub_epi16(all_ff, d_lo);
        let inv_d_hi = _mm_sub_epi16(all_ff, d_hi);

        let scr_prod_lo = _mm_mullo_epi16(inv_s_lo, inv_d_lo);
        let scr_biased_lo = _mm_add_epi16(scr_prod_lo, half);
        let scr_div_lo = _mm_srli_epi16::<8>(_mm_add_epi16(scr_biased_lo, _mm_srli_epi16::<8>(scr_biased_lo)));
        let scr_result_lo = _mm_sub_epi16(all_ff, _mm_add_epi16(scr_div_lo, scr_div_lo));

        let scr_prod_hi = _mm_mullo_epi16(inv_s_hi, inv_d_hi);
        let scr_biased_hi = _mm_add_epi16(scr_prod_hi, half);
        let scr_div_hi = _mm_srli_epi16::<8>(_mm_add_epi16(scr_biased_hi, _mm_srli_epi16::<8>(scr_biased_hi)));
        let scr_result_hi = _mm_sub_epi16(all_ff, _mm_add_epi16(scr_div_hi, scr_div_hi));

        // mask: 0xFFFF where s < 128 (multiply path)
        let mask_lo = _mm_cmplt_epi16(s_lo, threshold);
        let mask_hi = _mm_cmplt_epi16(s_hi, threshold);

        let out_lo = _mm_or_si128(
            _mm_and_si128(mask_lo, mul_result_lo),
            _mm_andnot_si128(mask_lo, scr_result_lo),
        );
        let out_hi = _mm_or_si128(
            _mm_and_si128(mask_hi, mul_result_hi),
            _mm_andnot_si128(mask_hi, scr_result_hi),
        );

        let result = _mm_packus_epi16(out_lo, out_hi);

        // Preserve alpha as max(da, sa)
        let alpha_mask = _mm_set_epi8(-1, 0, 0, 0, -1, 0, 0, 0, -1, 0, 0, 0, -1, 0, 0, 0);
        let max_alpha = _mm_max_epu8(d, s);
        let result = _mm_or_si128(
            _mm_andnot_si128(alpha_mask, result),
            _mm_and_si128(alpha_mask, max_alpha),
        );

        _mm_storeu_si128(d_ptr, result);
        offset += 16;
    }

    if offset < len {
        blend_scanline_hard_light_scalar(&mut dst[offset..], &src[offset..]);
    }
}

// ============================================================================
// Exclusion blend: s + d - 2*s*d/255
// ============================================================================

/// Exclusion blend on a BGRA8 scanline: `out = s + d - 2*s*d/255`.
pub fn blend_scanline_exclusion(dst: &mut [u8], src: &[u8]) {
    assert_eq!(dst.len(), src.len());
    assert_eq!(dst.len() % 4, 0);

    #[cfg(target_arch = "x86_64")]
    {
        unsafe { return blend_scanline_exclusion_sse2(dst, src) }
    }
    #[cfg(not(target_arch = "x86_64"))]
    blend_scanline_exclusion_scalar(dst, src);
}

fn blend_scanline_exclusion_scalar(dst: &mut [u8], src: &[u8]) {
    let pixels = dst.len() / 4;
    for i in 0..pixels {
        let off = i * 4;
        for c in 0..3 {
            let s = src[off + c] as u32;
            let d = dst[off + c] as u32;
            dst[off + c] = (s + d - 2 * (s * d + 127) / 255) as u8;
        }
        dst[off + 3] = dst[off + 3].max(src[off + 3]);
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn blend_scanline_exclusion_sse2(dst: &mut [u8], src: &[u8]) {
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

        // Compute s*d/255 first (fits in u16 after division), then 2*(s*d/255)
        let prod_lo = _mm_mullo_epi16(s_lo, d_lo);
        let prod_hi = _mm_mullo_epi16(s_hi, d_hi);
        let biased_lo = _mm_add_epi16(prod_lo, half);
        let biased_hi = _mm_add_epi16(prod_hi, half);
        let div_lo = _mm_srli_epi16::<8>(_mm_add_epi16(biased_lo, _mm_srli_epi16::<8>(biased_lo)));
        let div_hi = _mm_srli_epi16::<8>(_mm_add_epi16(biased_hi, _mm_srli_epi16::<8>(biased_hi)));

        // 2 * (s*d/255) — now safe since div result is at most 255
        let twice_div_lo = _mm_add_epi16(div_lo, div_lo);
        let twice_div_hi = _mm_add_epi16(div_hi, div_hi);

        // s + d - 2*s*d/255
        let sum_lo = _mm_add_epi16(s_lo, d_lo);
        let sum_hi = _mm_add_epi16(s_hi, d_hi);
        let out_lo = _mm_sub_epi16(sum_lo, twice_div_lo);
        let out_hi = _mm_sub_epi16(sum_hi, twice_div_hi);

        let result = _mm_packus_epi16(out_lo, out_hi);

        // Preserve alpha as max(da, sa)
        let alpha_mask = _mm_set_epi8(-1, 0, 0, 0, -1, 0, 0, 0, -1, 0, 0, 0, -1, 0, 0, 0);
        let max_alpha = _mm_max_epu8(d, s);
        let result = _mm_or_si128(
            _mm_andnot_si128(alpha_mask, result),
            _mm_and_si128(alpha_mask, max_alpha),
        );

        _mm_storeu_si128(d_ptr, result);
        offset += 16;
    }

    if offset < len {
        blend_scanline_exclusion_scalar(&mut dst[offset..], &src[offset..]);
    }
}

// ============================================================================
// Color Dodge blend: min(255, d*255 / (255-s))  (scalar only — division)
// ============================================================================

/// Color Dodge blend on a BGRA8 scanline.
///
/// Per RGB channel: `if d == 0 { 0 } else if s == 255 { 255 } else { min(255, d*255/(255-s)) }`
/// Scalar-only: per-pixel division makes SIMD impractical without lookup tables.
pub fn blend_scanline_color_dodge(dst: &mut [u8], src: &[u8]) {
    assert_eq!(dst.len(), src.len());
    assert_eq!(dst.len() % 4, 0);

    let pixels = dst.len() / 4;
    for i in 0..pixels {
        let off = i * 4;
        for c in 0..3 {
            let d = dst[off + c];
            let s = src[off + c];
            dst[off + c] = if d == 0 {
                0
            } else if s == 255 {
                255
            } else {
                ((d as u32 * 255 / (255 - s as u32)).min(255)) as u8
            };
        }
        dst[off + 3] = dst[off + 3].max(src[off + 3]);
    }
}

// ============================================================================
// Color Burn blend: 255 - min(255, (255-d)*255 / s)  (scalar only — division)
// ============================================================================

/// Color Burn blend on a BGRA8 scanline.
///
/// Per RGB channel: `if d == 255 { 255 } else if s == 0 { 0 } else { 255 - min(255, (255-d)*255/s) }`
/// Scalar-only: per-pixel division makes SIMD impractical without lookup tables.
pub fn blend_scanline_color_burn(dst: &mut [u8], src: &[u8]) {
    assert_eq!(dst.len(), src.len());
    assert_eq!(dst.len() % 4, 0);

    let pixels = dst.len() / 4;
    for i in 0..pixels {
        let off = i * 4;
        for c in 0..3 {
            let d = dst[off + c];
            let s = src[off + c];
            dst[off + c] = if d == 255 {
                255
            } else if s == 0 {
                0
            } else {
                (255 - ((255 - d as u32) * 255 / s as u32).min(255)) as u8
            };
        }
        dst[off + 3] = dst[off + 3].max(src[off + 3]);
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
        // White opaque src: multiply RGB unchanged, alpha stays 255
        let mut dst = make_pixel(100, 150, 200, 255).to_vec();
        let src = make_pixel(255, 255, 255, 255);
        blend_scanline_multiply(&mut dst, &src);
        assert_eq!(dst[0], 100);
        assert_eq!(dst[1], 150);
        assert_eq!(dst[2], 200);
        assert_eq!(dst[3], 255);
    }

    #[test]
    fn multiply_transparent_src_noop() {
        // Fully transparent source should leave dst unchanged
        let original = make_pixel(200, 150, 100, 255);
        let mut dst = original.to_vec();
        let src = make_pixel(0, 0, 0, 0);
        blend_scanline_multiply(&mut dst, &src);
        assert_eq!(&dst, &original);
    }

    #[test]
    fn multiply_opaque_both() {
        // Both opaque: out_c = (s*d + s*0 + d*0 + 127)/255 = (s*d+127)/255
        // out_a = 255 + 255 - (255*255+127)/255 = 510 - 255 = 255
        let mut dst = make_pixel(200, 100, 50, 255).to_vec();
        let src = make_pixel(128, 255, 0, 255);
        blend_scanline_multiply(&mut dst, &src);
        // B: (128*200+127)/255 = 25727/255 = 100
        assert!((dst[0] as i16 - 101).abs() <= 1, "B={}", dst[0]);
        // G: (255*100+127)/255 = 25627/255 = 100
        assert!((dst[1] as i16 - 100).abs() <= 1, "G={}", dst[1]);
        // R: (0*50+127)/255 = 0
        assert_eq!(dst[2], 0);
        assert_eq!(dst[3], 255);
    }

    #[test]
    fn multiply_half_alpha() {
        // src_a=128, dst_a=255: inv_sa=127, inv_da=0
        // out_c = (s*d + s*0 + d*127 + 127)/255
        let mut dst = make_pixel(200, 200, 200, 255).to_vec();
        let src = make_pixel(64, 64, 64, 128);
        blend_scanline_multiply(&mut dst, &src);
        // B: (64*200 + 64*0 + 200*127 + 127)/255 = (12800 + 0 + 25400 + 127)/255 = 38327/255 = 150
        assert!((dst[0] as i16 - 150).abs() <= 1, "B={}", dst[0]);
    }

    #[test]
    fn multiply_matches_scalar() {
        // Verify SIMD path matches scalar for a larger buffer
        let pixel_count = 137;
        let len = pixel_count * 4;
        let mut dst_scalar = vec![0u8; len];
        let mut dst_auto = vec![0u8; len];
        let src: Vec<u8> = (0..len).map(|i| (i % 256) as u8).collect();

        for i in 0..len {
            dst_scalar[i] = (i as u8).wrapping_mul(7).wrapping_add(31);
            dst_auto[i] = dst_scalar[i];
        }

        blend_scanline_multiply_scalar(&mut dst_scalar, &src);
        blend_scanline_multiply(&mut dst_auto, &src);

        for i in 0..len {
            let diff = (dst_scalar[i] as i16 - dst_auto[i] as i16).abs();
            assert!(
                diff <= 1,
                "multiply mismatch at byte {i}: scalar={} auto={}",
                dst_scalar[i],
                dst_auto[i]
            );
        }
    }

    #[test]
    fn multiply_avx2_large_buffer() {
        // 256 pixels to exercise AVX2 path (8 pixels per iteration)
        let pixel_count = 256;
        let len = pixel_count * 4;
        let mut dst_scalar = vec![0u8; len];
        let mut dst_auto = vec![0u8; len];
        let src: Vec<u8> = (0..len).map(|i| ((i * 3 + 17) % 256) as u8).collect();

        for i in 0..len {
            dst_scalar[i] = ((i * 5 + 43) % 256) as u8;
            dst_auto[i] = dst_scalar[i];
        }

        blend_scanline_multiply_scalar(&mut dst_scalar, &src);
        blend_scanline_multiply(&mut dst_auto, &src);

        for i in 0..len {
            let diff = (dst_scalar[i] as i16 - dst_auto[i] as i16).abs();
            assert!(
                diff <= 1,
                "multiply AVX2 mismatch at byte {i}: scalar={} auto={}",
                dst_scalar[i],
                dst_auto[i]
            );
        }
    }

    #[test]
    fn darken_picks_min() {
        let mut dst = make_pixel(200, 50, 150, 255).to_vec();
        let src = make_pixel(100, 100, 100, 200);
        blend_scanline_darken(&mut dst, &src);
        assert_eq!(dst, [100, 50, 100, 255]);
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
        assert_eq!(dst, [100, 100, 0, 255]);
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
    fn screen_matches_scalar() {
        let pixel_count = 200;
        let len = pixel_count * 4;
        let mut dst_scalar = vec![0u8; len];
        let mut dst_auto = vec![0u8; len];
        let src: Vec<u8> = (0..len).map(|i| ((i * 7 + 13) % 256) as u8).collect();

        for i in 0..len {
            dst_scalar[i] = ((i * 11 + 59) % 256) as u8;
            dst_auto[i] = dst_scalar[i];
        }

        blend_scanline_screen_scalar(&mut dst_scalar, &src);
        blend_scanline_screen(&mut dst_auto, &src);

        for i in 0..len {
            let diff = (dst_scalar[i] as i16 - dst_auto[i] as i16).abs();
            assert!(
                diff <= 1,
                "screen mismatch at byte {i}: scalar={} auto={}",
                dst_scalar[i],
                dst_auto[i]
            );
        }
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

    // ---- Overlay tests ----

    #[test]
    fn overlay_dark_dst_uses_multiply() {
        // dst < 128 → multiply path: 2*s*d/255
        let mut dst = make_pixel(50, 50, 50, 255).to_vec();
        let src = make_pixel(100, 100, 100, 255);
        blend_scanline_overlay(&mut dst, &src);
        // 2*50*100/255 ≈ 39
        assert!((dst[0] as i16 - 39).abs() <= 1);
    }

    #[test]
    fn overlay_light_dst_uses_screen() {
        // dst >= 128 → screen path: 255 - 2*(255-s)*(255-d)/255
        let mut dst = make_pixel(200, 200, 200, 255).to_vec();
        let src = make_pixel(150, 150, 150, 255);
        blend_scanline_overlay(&mut dst, &src);
        // 255 - 2*105*55/255 ≈ 255 - 45 = 210
        let expected = 255u16.saturating_sub(2 * 105 * 55 / 255);
        assert!((dst[0] as i16 - expected as i16).abs() <= 2);
    }

    #[test]
    fn overlay_matches_scalar() {
        let pixel_count = 137;
        let len = pixel_count * 4;
        let mut dst_scalar = vec![0u8; len];
        let mut dst_simd = vec![0u8; len];
        let src: Vec<u8> = (0..len).map(|i| ((i * 7 + 13) % 256) as u8).collect();

        for i in 0..len {
            dst_scalar[i] = (i as u8).wrapping_mul(3).wrapping_add(50);
            dst_simd[i] = dst_scalar[i];
        }

        blend_scanline_overlay_scalar(&mut dst_scalar, &src);
        blend_scanline_overlay(&mut dst_simd, &src);

        for i in 0..len {
            let diff = (dst_scalar[i] as i16 - dst_simd[i] as i16).abs();
            assert!(diff <= 1, "overlay mismatch at {i}: scalar={} simd={}", dst_scalar[i], dst_simd[i]);
        }
    }

    // ---- Hard Light tests ----

    #[test]
    fn hard_light_dark_src_uses_multiply() {
        let mut dst = make_pixel(200, 200, 200, 255).to_vec();
        let src = make_pixel(50, 50, 50, 255);
        blend_scanline_hard_light(&mut dst, &src);
        // s < 128 → 2*s*d/255 = 2*50*200/255 ≈ 78
        assert!((dst[0] as i16 - 78).abs() <= 2);
    }

    #[test]
    fn hard_light_matches_scalar() {
        let pixel_count = 137;
        let len = pixel_count * 4;
        let mut dst_scalar = vec![0u8; len];
        let mut dst_simd = vec![0u8; len];
        let src: Vec<u8> = (0..len).map(|i| ((i * 11 + 7) % 256) as u8).collect();

        for i in 0..len {
            dst_scalar[i] = (i as u8).wrapping_mul(5).wrapping_add(20);
            dst_simd[i] = dst_scalar[i];
        }

        blend_scanline_hard_light_scalar(&mut dst_scalar, &src);
        blend_scanline_hard_light(&mut dst_simd, &src);

        for i in 0..len {
            let diff = (dst_scalar[i] as i16 - dst_simd[i] as i16).abs();
            assert!(diff <= 1, "hard_light mismatch at {i}: scalar={} simd={}", dst_scalar[i], dst_simd[i]);
        }
    }

    // ---- Exclusion tests ----

    #[test]
    fn exclusion_black_identity() {
        // exclusion with black src: s+d-0 = d
        let mut dst = make_pixel(100, 150, 200, 255).to_vec();
        let src = make_pixel(0, 0, 0, 255);
        blend_scanline_exclusion(&mut dst, &src);
        assert_eq!(dst[0], 100);
        assert_eq!(dst[1], 150);
        assert_eq!(dst[2], 200);
    }

    #[test]
    fn exclusion_symmetric() {
        let mut dst1 = make_pixel(80, 120, 200, 255).to_vec();
        let src1 = make_pixel(200, 80, 120, 255).to_vec();
        let mut dst2 = src1.clone();
        let src2 = make_pixel(80, 120, 200, 255).to_vec();

        blend_scanline_exclusion(&mut dst1, &src1);
        blend_scanline_exclusion(&mut dst2, &src2);

        // Exclusion is symmetric: B(s,d) == B(d,s)
        for c in 0..3 {
            assert!((dst1[c] as i16 - dst2[c] as i16).abs() <= 1);
        }
    }

    #[test]
    fn exclusion_matches_scalar() {
        let pixel_count = 137;
        let len = pixel_count * 4;
        let mut dst_scalar = vec![0u8; len];
        let mut dst_simd = vec![0u8; len];
        let src: Vec<u8> = (0..len).map(|i| ((i * 13 + 3) % 256) as u8).collect();

        for i in 0..len {
            dst_scalar[i] = (i as u8).wrapping_mul(7).wrapping_add(11);
            dst_simd[i] = dst_scalar[i];
        }

        blend_scanline_exclusion_scalar(&mut dst_scalar, &src);
        blend_scanline_exclusion(&mut dst_simd, &src);

        for i in 0..len {
            let diff = (dst_scalar[i] as i16 - dst_simd[i] as i16).abs();
            assert!(diff <= 1, "exclusion mismatch at {i}: scalar={} simd={}", dst_scalar[i], dst_simd[i]);
        }
    }

    // ---- Color Dodge tests ----

    #[test]
    fn color_dodge_zero_dst_yields_zero() {
        let mut dst = make_pixel(0, 0, 0, 255).to_vec();
        let src = make_pixel(128, 128, 128, 255);
        blend_scanline_color_dodge(&mut dst, &src);
        assert_eq!(dst[0], 0);
        assert_eq!(dst[1], 0);
        assert_eq!(dst[2], 0);
    }

    #[test]
    fn color_dodge_white_src_saturates() {
        let mut dst = make_pixel(100, 100, 100, 255).to_vec();
        let src = make_pixel(255, 255, 255, 255);
        blend_scanline_color_dodge(&mut dst, &src);
        assert_eq!(dst[0], 255);
        assert_eq!(dst[1], 255);
        assert_eq!(dst[2], 255);
    }

    #[test]
    fn color_dodge_basic() {
        let mut dst = make_pixel(100, 100, 100, 255).to_vec();
        let src = make_pixel(128, 128, 128, 255);
        blend_scanline_color_dodge(&mut dst, &src);
        // 100*255/(255-128) = 100*255/127 ≈ 200
        let expected = (100u32 * 255 / 127).min(255) as u8;
        assert_eq!(dst[0], expected);
    }

    // ---- Color Burn tests ----

    #[test]
    fn color_burn_white_dst_stays_white() {
        let mut dst = make_pixel(255, 255, 255, 255).to_vec();
        let src = make_pixel(128, 128, 128, 255);
        blend_scanline_color_burn(&mut dst, &src);
        assert_eq!(dst[0], 255);
        assert_eq!(dst[1], 255);
        assert_eq!(dst[2], 255);
    }

    #[test]
    fn color_burn_black_src_yields_black() {
        let mut dst = make_pixel(100, 100, 100, 255).to_vec();
        let src = make_pixel(0, 0, 0, 255);
        blend_scanline_color_burn(&mut dst, &src);
        assert_eq!(dst[0], 0);
        assert_eq!(dst[1], 0);
        assert_eq!(dst[2], 0);
    }

    #[test]
    fn color_burn_basic() {
        let mut dst = make_pixel(200, 200, 200, 255).to_vec();
        let src = make_pixel(128, 128, 128, 255);
        blend_scanline_color_burn(&mut dst, &src);
        // 255 - min(255, (255-200)*255/128) = 255 - min(255, 55*255/128) ≈ 255 - 109 = 146
        let expected = (255 - ((255 - 200u32) * 255 / 128).min(255)) as u8;
        assert_eq!(dst[0], expected);
    }

    // ---- Alpha preservation tests ----

    #[test]
    fn all_new_modes_preserve_alpha_as_max() {
        let da = 180u8;
        let sa = 220u8;
        let expected_alpha = da.max(sa);

        let test_modes: Vec<(&str, fn(&mut [u8], &[u8]))> = vec![
            ("overlay", blend_scanline_overlay),
            ("hard_light", blend_scanline_hard_light),
            ("exclusion", blend_scanline_exclusion),
            ("color_dodge", blend_scanline_color_dodge),
            ("color_burn", blend_scanline_color_burn),
        ];

        for (name, func) in test_modes {
            let mut dst = make_pixel(100, 150, 200, da).to_vec();
            let src = make_pixel(50, 100, 150, sa).to_vec();
            func(&mut dst, &src);
            assert_eq!(dst[3], expected_alpha, "{name} alpha mismatch");
        }
    }
}
