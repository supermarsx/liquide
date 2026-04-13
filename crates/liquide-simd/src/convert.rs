//! Channel conversion, unpremultiply, interpolation, and constant-color blending.
//!
//! - [`swap_rb`] — BGRA↔RGBA channel swizzle (in-place)
//! - [`unpremultiply_alpha`] — Inverse of [`crate::fill::premultiply_alpha`]
//! - [`upsample_2x_bilinear`] — Bilinear 2× upscale
//! - [`blend_constant_src_over`] — SrcOver with a single premultiplied color

// ── 1. BGRA↔RGBA channel swizzle ────────────────────────────────────

/// Swap B and R channels in a BGRA8 buffer (converts BGRA↔RGBA in-place).
pub fn swap_rb(buf: &mut [u8]) {
    assert_eq!(buf.len() % 4, 0);

    #[cfg(target_arch = "x86_64")]
    {
        if crate::detect::has_avx512() {
            unsafe { return swap_rb_avx512(buf) }
        }
        if crate::detect::has_avx2() {
            unsafe { return swap_rb_avx2(buf) }
        }
        if crate::detect::has(crate::detect::features::SSSE3) {
            unsafe { return swap_rb_ssse3(buf) }
        }
    }
    swap_rb_scalar(buf);
}

fn swap_rb_scalar(buf: &mut [u8]) {
    for pixel in buf.chunks_exact_mut(4) {
        pixel.swap(0, 2); // swap B and R
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn swap_rb_ssse3(buf: &mut [u8]) {
    use std::arch::x86_64::*;

    // Shuffle mask: swap bytes 0↔2 within each 4-byte pixel
    // BGRA → RGBA: [B,G,R,A] at indices [0,1,2,3] → [R,G,B,A] = indices [2,1,0,3]
    let mask = _mm_setr_epi8(2, 1, 0, 3, 6, 5, 4, 7, 10, 9, 8, 11, 14, 13, 12, 15);

    let len = buf.len();
    let chunks = len / 16; // 4 pixels = 16 bytes
    let mut offset = 0;

    for _ in 0..chunks {
        let ptr = buf.as_mut_ptr().add(offset) as *mut __m128i;
        let v = _mm_loadu_si128(ptr);
        let result = _mm_shuffle_epi8(v, mask);
        _mm_storeu_si128(ptr, result);
        offset += 16;
    }

    // Scalar tail
    if offset < len {
        swap_rb_scalar(&mut buf[offset..]);
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn swap_rb_avx2(buf: &mut [u8]) {
    use std::arch::x86_64::*;

    // Same mask pattern broadcast to both 128-bit lanes
    let mask = _mm256_setr_epi8(
        2, 1, 0, 3, 6, 5, 4, 7, 10, 9, 8, 11, 14, 13, 12, 15,
        2, 1, 0, 3, 6, 5, 4, 7, 10, 9, 8, 11, 14, 13, 12, 15,
    );

    let len = buf.len();
    let chunks = len / 32; // 8 pixels = 32 bytes
    let mut offset = 0;

    for _ in 0..chunks {
        let ptr = buf.as_mut_ptr().add(offset) as *mut __m256i;
        let v = _mm256_loadu_si256(ptr);
        let result = _mm256_shuffle_epi8(v, mask);
        _mm256_storeu_si256(ptr, result);
        offset += 32;
    }

    // Fall through to SSSE3 for remaining
    if offset < len {
        swap_rb_ssse3(&mut buf[offset..]);
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f,avx512bw")]
unsafe fn swap_rb_avx512(buf: &mut [u8]) {
    use std::arch::x86_64::*;

    // _mm512_shuffle_epi8 operates per 128-bit lane, same mask repeated 4 times
    let mask = _mm512_set4_epi32(
        // Each 128-bit lane: [2,1,0,3, 6,5,4,7, 10,9,8,11, 14,13,12,15]
        // as i32 (little-endian): bytes [12..15] = 0x0F_0C_0D_0E, etc.
        i32::from_le_bytes([14, 13, 12, 15]),
        i32::from_le_bytes([10, 9, 8, 11]),
        i32::from_le_bytes([6, 5, 4, 7]),
        i32::from_le_bytes([2, 1, 0, 3]),
    );

    let len = buf.len();
    let chunks = len / 64; // 16 pixels = 64 bytes
    let mut offset = 0;

    for _ in 0..chunks {
        let ptr = buf.as_mut_ptr().add(offset) as *mut __m512i;
        let v = _mm512_loadu_si512(ptr as *const __m512i);
        let result = _mm512_shuffle_epi8(v, mask);
        _mm512_storeu_si512(ptr, result);
        offset += 64;
    }

    if offset < len {
        swap_rb_avx2(&mut buf[offset..]);
    }
}

// ── 2. Alpha unpremultiply ───────────────────────────────────────────

/// Unpremultiply alpha for a BGRA8 scanline in-place.
/// `channel = channel * 255 / alpha` for B, G, R. Alpha unchanged.
pub fn unpremultiply_alpha(buf: &mut [u8]) {
    assert_eq!(buf.len() % 4, 0);

    #[cfg(target_arch = "x86_64")]
    {
        // SSE2 is always available on x86-64
        unsafe { return unpremultiply_alpha_sse2(buf) }
    }
    #[cfg(not(target_arch = "x86_64"))]
    unpremultiply_alpha_scalar(buf);
}

fn unpremultiply_alpha_scalar(buf: &mut [u8]) {
    for pixel in buf.chunks_exact_mut(4) {
        let a = pixel[3] as u16;
        if a == 0 || a == 255 {
            continue;
        }
        pixel[0] = ((pixel[0] as u16 * 255 + a / 2) / a).min(255) as u8;
        pixel[1] = ((pixel[1] as u16 * 255 + a / 2) / a).min(255) as u8;
        pixel[2] = ((pixel[2] as u16 * 255 + a / 2) / a).min(255) as u8;
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn unpremultiply_alpha_sse2(buf: &mut [u8]) {
    use std::arch::x86_64::*;

    let zero = _mm_setzero_si128();
    let ff = _mm_set1_ps(255.0);
    let max_f = _mm_set1_ps(255.0);
    let zero_f = _mm_setzero_ps();
    let half_f = _mm_set1_ps(0.5);

    let pixels = buf.len() / 4;
    for i in 0..pixels {
        let off = i * 4;
        let alpha = buf[off + 3];
        if alpha == 0 || alpha == 255 {
            continue;
        } // skip transparent and opaque

        let pixel = _mm_cvtsi32_si128(i32::from_le_bytes([
            buf[off],
            buf[off + 1],
            buf[off + 2],
            buf[off + 3],
        ]));
        let pixel_16 = _mm_unpacklo_epi8(pixel, zero);
        let pixel_32 = _mm_unpacklo_epi16(pixel_16, zero);
        let pixel_f = _mm_cvtepi32_ps(pixel_32); // [B, G, R, A] as f32

        let alpha_f = _mm_set1_ps(alpha as f32);
        let scale = _mm_div_ps(ff, alpha_f); // 255.0 / alpha

        let result = _mm_mul_ps(pixel_f, scale);
        let result = _mm_add_ps(result, half_f); // rounding
        let result = _mm_max_ps(_mm_min_ps(result, max_f), zero_f);

        let int = _mm_cvttps_epi32(result);
        let packed_16 = _mm_packs_epi32(int, int);
        let packed_8 = _mm_packus_epi16(packed_16, packed_16);
        let val = (_mm_cvtsi128_si32(packed_8) as u32).to_le_bytes();
        buf[off] = val[0];
        buf[off + 1] = val[1];
        buf[off + 2] = val[2];
        // alpha unchanged
    }
}

// ── 3. Bilinear 2× upsample ─────────────────────────────────────────

/// Bilinear 2× upsample: each pixel in src maps to a 2×2 block in the output.
/// Returns `(buffer, dst_width, dst_height)`.
pub fn upsample_2x_bilinear(src: &[u8], width: u32, height: u32) -> (Vec<u8>, u32, u32) {
    assert_eq!(src.len(), (width * height * 4) as usize);

    let dst_w = width * 2;
    let dst_h = height * 2;
    let mut dst = vec![0u8; (dst_w * dst_h * 4) as usize];

    #[cfg(target_arch = "x86_64")]
    {
        if crate::detect::has_fma() {
            unsafe {
                upsample_2x_bilinear_fma(src, width, height, &mut dst, dst_w);
            }
            return (dst, dst_w, dst_h);
        }
        // SSE2 is always available on x86-64
        unsafe {
            upsample_2x_bilinear_sse2(src, width, height, &mut dst, dst_w);
        }
        return (dst, dst_w, dst_h);
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        upsample_2x_bilinear_scalar(src, width, height, &mut dst, dst_w);
        (dst, dst_w, dst_h)
    }
}

fn upsample_2x_bilinear_scalar(
    src: &[u8],
    width: u32,
    height: u32,
    dst: &mut [u8],
    dst_w: u32,
) {
    let w = width as usize;
    let h = height as usize;
    let dw = dst_w as usize;

    for dy in 0..(h * 2) {
        for dx in 0..(w * 2) {
            // Map output (dx, dy) back to source coordinates
            // Source coordinate: (dx - 0.5) / 2, but we clamp to [0, w-1]
            let sx_f = (dx as f32) * 0.5;
            let sy_f = (dy as f32) * 0.5;

            let sx0 = (sx_f.floor() as usize).min(w - 1);
            let sy0 = (sy_f.floor() as usize).min(h - 1);
            let sx1 = (sx0 + 1).min(w - 1);
            let sy1 = (sy0 + 1).min(h - 1);

            let fx = sx_f - sx0 as f32;
            let fy = sy_f - sy0 as f32;

            let dst_off = (dy * dw + dx) * 4;
            let s00 = (sy0 * w + sx0) * 4;
            let s10 = (sy0 * w + sx1) * 4;
            let s01 = (sy1 * w + sx0) * 4;
            let s11 = (sy1 * w + sx1) * 4;

            for c in 0..4 {
                let p00 = src[s00 + c] as f32;
                let p10 = src[s10 + c] as f32;
                let p01 = src[s01 + c] as f32;
                let p11 = src[s11 + c] as f32;

                let top = p00 + (p10 - p00) * fx;
                let bot = p01 + (p11 - p01) * fx;
                let val = top + (bot - top) * fy;
                dst[dst_off + c] = val.round().clamp(0.0, 255.0) as u8;
            }
        }
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn upsample_2x_bilinear_sse2(
    src: &[u8],
    width: u32,
    height: u32,
    dst: &mut [u8],
    dst_w: u32,
) {
    use std::arch::x86_64::*;

    let w = width as usize;
    let h = height as usize;
    let dw = dst_w as usize;
    let zero = _mm_setzero_si128();
    let max_f = _mm_set1_ps(255.0);
    let zero_f = _mm_setzero_ps();
    let half_f = _mm_set1_ps(0.5);

    for dy in 0..(h * 2) {
        for dx in 0..(w * 2) {
            let sx_f = (dx as f32) * 0.5;
            let sy_f = (dy as f32) * 0.5;

            let sx0 = (sx_f.floor() as usize).min(w - 1);
            let sy0 = (sy_f.floor() as usize).min(h - 1);
            let sx1 = (sx0 + 1).min(w - 1);
            let sy1 = (sy0 + 1).min(h - 1);

            let fx = _mm_set1_ps(sx_f - sx0 as f32);
            let fy = _mm_set1_ps(sy_f - sy0 as f32);

            let dst_off = (dy * dw + dx) * 4;
            let s00 = (sy0 * w + sx0) * 4;
            let s10 = (sy0 * w + sx1) * 4;
            let s01 = (sy1 * w + sx0) * 4;
            let s11 = (sy1 * w + sx1) * 4;

            // Load 4 source pixels as f32 vectors [B, G, R, A]
            let load_pixel = |off: usize| -> __m128 {
                let px = _mm_cvtsi32_si128(i32::from_le_bytes([
                    src[off],
                    src[off + 1],
                    src[off + 2],
                    src[off + 3],
                ]));
                let px16 = _mm_unpacklo_epi8(px, zero);
                let px32 = _mm_unpacklo_epi16(px16, zero);
                _mm_cvtepi32_ps(px32)
            };

            let p00 = load_pixel(s00);
            let p10 = load_pixel(s10);
            let p01 = load_pixel(s01);
            let p11 = load_pixel(s11);

            // Bilinear: lerp(a, b, t) = a + (b - a) * t
            let top = _mm_add_ps(p00, _mm_mul_ps(_mm_sub_ps(p10, p00), fx));
            let bot = _mm_add_ps(p01, _mm_mul_ps(_mm_sub_ps(p11, p01), fx));
            let val = _mm_add_ps(top, _mm_mul_ps(_mm_sub_ps(bot, top), fy));

            // Round and clamp
            let val = _mm_add_ps(val, half_f);
            let val = _mm_max_ps(_mm_min_ps(val, max_f), zero_f);
            let int = _mm_cvttps_epi32(val);
            let packed_16 = _mm_packs_epi32(int, int);
            let packed_8 = _mm_packus_epi16(packed_16, packed_16);
            let bytes = (_mm_cvtsi128_si32(packed_8) as u32).to_le_bytes();

            dst[dst_off] = bytes[0];
            dst[dst_off + 1] = bytes[1];
            dst[dst_off + 2] = bytes[2];
            dst[dst_off + 3] = bytes[3];
        }
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "fma")]
unsafe fn upsample_2x_bilinear_fma(
    src: &[u8],
    width: u32,
    height: u32,
    dst: &mut [u8],
    dst_w: u32,
) {
    use std::arch::x86_64::*;

    let w = width as usize;
    let h = height as usize;
    let dw = dst_w as usize;
    let zero = _mm_setzero_si128();
    let max_f = _mm_set1_ps(255.0);
    let zero_f = _mm_setzero_ps();
    let half_f = _mm_set1_ps(0.5);

    for dy in 0..(h * 2) {
        for dx in 0..(w * 2) {
            let sx_f = (dx as f32) * 0.5;
            let sy_f = (dy as f32) * 0.5;

            let sx0 = (sx_f.floor() as usize).min(w - 1);
            let sy0 = (sy_f.floor() as usize).min(h - 1);
            let sx1 = (sx0 + 1).min(w - 1);
            let sy1 = (sy0 + 1).min(h - 1);

            let fx = _mm_set1_ps(sx_f - sx0 as f32);
            let fy = _mm_set1_ps(sy_f - sy0 as f32);

            let dst_off = (dy * dw + dx) * 4;
            let s00 = (sy0 * w + sx0) * 4;
            let s10 = (sy0 * w + sx1) * 4;
            let s01 = (sy1 * w + sx0) * 4;
            let s11 = (sy1 * w + sx1) * 4;

            let load_pixel = |off: usize| -> __m128 {
                let px = _mm_cvtsi32_si128(i32::from_le_bytes([
                    src[off],
                    src[off + 1],
                    src[off + 2],
                    src[off + 3],
                ]));
                let px16 = _mm_unpacklo_epi8(px, zero);
                let px32 = _mm_unpacklo_epi16(px16, zero);
                _mm_cvtepi32_ps(px32)
            };

            let p00 = load_pixel(s00);
            let p10 = load_pixel(s10);
            let p01 = load_pixel(s01);
            let p11 = load_pixel(s11);

            // FMA lerp: lerp(a, b, t) = fma(b - a, t, a)
            let top = _mm_fmadd_ps(_mm_sub_ps(p10, p00), fx, p00);
            let bot = _mm_fmadd_ps(_mm_sub_ps(p11, p01), fx, p01);
            let val = _mm_fmadd_ps(_mm_sub_ps(bot, top), fy, top);

            // Round and clamp
            let val = _mm_add_ps(val, half_f);
            let val = _mm_max_ps(_mm_min_ps(val, max_f), zero_f);
            let int = _mm_cvttps_epi32(val);
            let packed_16 = _mm_packs_epi32(int, int);
            let packed_8 = _mm_packus_epi16(packed_16, packed_16);
            let bytes = (_mm_cvtsi128_si32(packed_8) as u32).to_le_bytes();

            dst[dst_off] = bytes[0];
            dst[dst_off + 1] = bytes[1];
            dst[dst_off + 2] = bytes[2];
            dst[dst_off + 3] = bytes[3];
        }
    }
}

// ── 4. Constant-color SrcOver blend ─────────────────────────────────

/// Blend a single premultiplied BGRA color over an entire scanline.
///
/// This is faster than [`crate::blend::blend_scanline_src_over`] when the
/// source is a constant color, because the source and inv_alpha values can
/// be computed once and reused for every pixel.
pub fn blend_constant_src_over(dst: &mut [u8], color: [u8; 4]) {
    assert_eq!(dst.len() % 4, 0);

    #[cfg(target_arch = "x86_64")]
    {
        if crate::detect::has_avx512() {
            unsafe { return blend_constant_src_over_avx512(dst, color) }
        }
        if crate::detect::has_avx2() {
            unsafe { return blend_constant_src_over_avx2(dst, color) }
        }
        // SSE2 is always available on x86-64
        unsafe { return blend_constant_src_over_sse2(dst, color) }
    }
    #[cfg(not(target_arch = "x86_64"))]
    blend_constant_src_over_scalar(dst, color);
}

fn blend_constant_src_over_scalar(dst: &mut [u8], color: [u8; 4]) {
    let sa = color[3] as u16;
    if sa == 0 {
        return;
    }
    if sa == 255 {
        for pixel in dst.chunks_exact_mut(4) {
            pixel.copy_from_slice(&color);
        }
        return;
    }
    let inv_a = 255 - sa;
    for pixel in dst.chunks_exact_mut(4) {
        for c in 0..4 {
            let d = pixel[c] as u16;
            pixel[c] = (color[c] as u16 + (d * inv_a + 127) / 255).min(255) as u8;
        }
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn blend_constant_src_over_sse2(dst: &mut [u8], color: [u8; 4]) {
    use std::arch::x86_64::*;

    let sa = color[3] as u16;
    if sa == 0 {
        return;
    }
    if sa == 255 {
        // Opaque: just fill
        crate::fill::fill_pattern(dst, color);
        return;
    }

    let inv_a = 255 - sa;
    // Pre-compute source as u16 lanes, broadcast to all pixel positions
    let zero = _mm_setzero_si128();
    let src_pixel = _mm_set1_epi32(i32::from_le_bytes(color));
    let src_lo = _mm_unpacklo_epi8(src_pixel, zero); // all 4 pixels are identical
    let inv_a_v = _mm_set1_epi16(inv_a as i16);
    let half = _mm_set1_epi16(128);

    let len = dst.len();
    let chunks = len / 16;
    let mut offset = 0;

    for _ in 0..chunks {
        let d_ptr = dst.as_mut_ptr().add(offset) as *mut __m128i;
        let d_packed = _mm_loadu_si128(d_ptr);

        let d_lo = _mm_unpacklo_epi8(d_packed, zero);
        let d_hi = _mm_unpackhi_epi8(d_packed, zero);

        // dst * inv_a / 255
        let prod_lo = _mm_mullo_epi16(d_lo, inv_a_v);
        let prod_hi = _mm_mullo_epi16(d_hi, inv_a_v);
        let biased_lo = _mm_add_epi16(prod_lo, half);
        let biased_hi = _mm_add_epi16(prod_hi, half);
        let approx_lo =
            _mm_srli_epi16::<8>(_mm_add_epi16(biased_lo, _mm_srli_epi16::<8>(biased_lo)));
        let approx_hi =
            _mm_srli_epi16::<8>(_mm_add_epi16(biased_hi, _mm_srli_epi16::<8>(biased_hi)));

        // out = src + dst * inv_a / 255
        let out_lo = _mm_add_epi16(src_lo, approx_lo);
        let out_hi = _mm_add_epi16(src_lo, approx_hi); // src_lo reused — same for all pixels

        let result = _mm_packus_epi16(out_lo, out_hi);
        _mm_storeu_si128(d_ptr, result);
        offset += 16;
    }

    // Scalar tail
    if offset < len {
        blend_constant_src_over_scalar(&mut dst[offset..], color);
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn blend_constant_src_over_avx2(dst: &mut [u8], color: [u8; 4]) {
    use std::arch::x86_64::*;

    let sa = color[3] as u16;
    if sa == 0 {
        return;
    }
    if sa == 255 {
        crate::fill::fill_pattern(dst, color);
        return;
    }

    let inv_a = 255 - sa;
    let zero = _mm256_setzero_si256();
    let src_pixel = _mm256_set1_epi32(i32::from_le_bytes(color));
    let src_lo = _mm256_unpacklo_epi8(src_pixel, zero);
    let inv_a_v = _mm256_set1_epi16(inv_a as i16);
    let half = _mm256_set1_epi16(128);

    let len = dst.len();
    let chunks = len / 32; // 8 pixels = 32 bytes
    let mut offset = 0;

    for _ in 0..chunks {
        let d_ptr = dst.as_mut_ptr().add(offset) as *mut __m256i;
        let d_packed = _mm256_loadu_si256(d_ptr);

        let d_lo = _mm256_unpacklo_epi8(d_packed, zero);
        let d_hi = _mm256_unpackhi_epi8(d_packed, zero);

        let prod_lo = _mm256_mullo_epi16(d_lo, inv_a_v);
        let prod_hi = _mm256_mullo_epi16(d_hi, inv_a_v);
        let biased_lo = _mm256_add_epi16(prod_lo, half);
        let biased_hi = _mm256_add_epi16(prod_hi, half);
        let approx_lo =
            _mm256_srli_epi16::<8>(_mm256_add_epi16(biased_lo, _mm256_srli_epi16::<8>(biased_lo)));
        let approx_hi =
            _mm256_srli_epi16::<8>(_mm256_add_epi16(biased_hi, _mm256_srli_epi16::<8>(biased_hi)));

        let out_lo = _mm256_add_epi16(src_lo, approx_lo);
        let out_hi = _mm256_add_epi16(src_lo, approx_hi);

        let result = _mm256_packus_epi16(out_lo, out_hi);
        _mm256_storeu_si256(d_ptr, result);
        offset += 32;
    }

    // Fall through to SSE2 for remaining
    if offset < len {
        blend_constant_src_over_sse2(&mut dst[offset..], color);
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f,avx512bw")]
unsafe fn blend_constant_src_over_avx512(dst: &mut [u8], color: [u8; 4]) {
    use std::arch::x86_64::*;

    let sa = color[3] as u16;
    if sa == 0 {
        return;
    }
    if sa == 255 {
        crate::fill::fill_pattern(dst, color);
        return;
    }

    let inv_a = 255 - sa;
    let zero = _mm512_setzero_si512();
    let src_pixel = _mm512_set1_epi32(i32::from_le_bytes(color));
    let src_lo = _mm512_unpacklo_epi8(src_pixel, zero);
    let inv_a_v = _mm512_set1_epi16(inv_a as i16);
    let half = _mm512_set1_epi16(128);

    let len = dst.len();
    let chunks = len / 64; // 16 pixels = 64 bytes
    let mut offset = 0;

    for _ in 0..chunks {
        let d_ptr = dst.as_mut_ptr().add(offset) as *mut __m512i;
        let d_packed = _mm512_loadu_si512(d_ptr as *const __m512i);

        let d_lo = _mm512_unpacklo_epi8(d_packed, zero);
        let d_hi = _mm512_unpackhi_epi8(d_packed, zero);

        let prod_lo = _mm512_mullo_epi16(d_lo, inv_a_v);
        let prod_hi = _mm512_mullo_epi16(d_hi, inv_a_v);
        let biased_lo = _mm512_add_epi16(prod_lo, half);
        let biased_hi = _mm512_add_epi16(prod_hi, half);
        let approx_lo =
            _mm512_srli_epi16::<8>(_mm512_add_epi16(biased_lo, _mm512_srli_epi16::<8>(biased_lo)));
        let approx_hi =
            _mm512_srli_epi16::<8>(_mm512_add_epi16(biased_hi, _mm512_srli_epi16::<8>(biased_hi)));

        let out_lo = _mm512_add_epi16(src_lo, approx_lo);
        let out_hi = _mm512_add_epi16(src_lo, approx_hi);

        let result = _mm512_packus_epi16(out_lo, out_hi);
        _mm512_storeu_si512(d_ptr, result);
        offset += 64;
    }

    if offset < len {
        blend_constant_src_over_avx2(&mut dst[offset..], color);
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── swap_rb tests ────────────────────────────────────────────────

    #[test]
    fn swap_rb_single_pixel() {
        let mut buf = [10u8, 20, 30, 255]; // B=10, G=20, R=30, A=255
        swap_rb(&mut buf);
        assert_eq!(buf, [30, 20, 10, 255]); // R=30, G=20, B=10, A=255
    }

    #[test]
    fn swap_rb_double_is_identity() {
        let original: Vec<u8> = (0..64).collect();
        let mut buf = original.clone();
        swap_rb(&mut buf);
        swap_rb(&mut buf);
        assert_eq!(buf, original);
    }

    #[test]
    fn swap_rb_matches_scalar() {
        let pixel_count = 137; // odd count to exercise tail
        let len = pixel_count * 4;
        let mut buf_scalar: Vec<u8> = (0..len).map(|i| (i % 256) as u8).collect();
        let mut buf_auto = buf_scalar.clone();

        swap_rb_scalar(&mut buf_scalar);
        swap_rb(&mut buf_auto);

        assert_eq!(buf_scalar, buf_auto);
    }

    #[test]
    fn swap_rb_large_buffer() {
        // 256 pixels — exercises AVX-512 + AVX2 + SSSE3 tails
        let pixel_count = 256;
        let len = pixel_count * 4;
        let mut buf: Vec<u8> = (0..len).map(|i| (i % 256) as u8).collect();
        let original = buf.clone();

        swap_rb(&mut buf);

        for i in 0..pixel_count {
            let off = i * 4;
            assert_eq!(buf[off], original[off + 2], "B→R at pixel {i}");
            assert_eq!(buf[off + 1], original[off + 1], "G unchanged at pixel {i}");
            assert_eq!(buf[off + 2], original[off], "R→B at pixel {i}");
            assert_eq!(buf[off + 3], original[off + 3], "A unchanged at pixel {i}");
        }
    }

    #[test]
    fn swap_rb_empty() {
        let mut buf: [u8; 0] = [];
        swap_rb(&mut buf); // should not panic
    }

    // ── unpremultiply_alpha tests ────────────────────────────────────

    #[test]
    fn unpremultiply_opaque_unchanged() {
        let mut buf = vec![100, 150, 200, 255];
        unpremultiply_alpha(&mut buf);
        assert_eq!(buf, [100, 150, 200, 255]);
    }

    #[test]
    fn unpremultiply_transparent_unchanged() {
        let mut buf = vec![0, 0, 0, 0];
        unpremultiply_alpha(&mut buf);
        assert_eq!(buf, [0, 0, 0, 0]);
    }

    #[test]
    fn unpremultiply_half_alpha() {
        // Premultiplied: channel = original * alpha / 255
        // If original was 200 and alpha is 128: premul ≈ 100
        // Unpremultiply: 100 * 255 / 128 ≈ 199
        let mut buf = vec![100, 100, 100, 128];
        unpremultiply_alpha(&mut buf);
        // 100 * 255 / 128 ≈ 199.2 → 199 or 200
        assert!((buf[0] as i16 - 199).abs() <= 1, "got {}", buf[0]);
        assert!((buf[1] as i16 - 199).abs() <= 1);
        assert!((buf[2] as i16 - 199).abs() <= 1);
        assert_eq!(buf[3], 128); // alpha preserved
    }

    #[test]
    fn unpremultiply_matches_scalar() {
        let pixel_count = 33;
        let mut buf: Vec<u8> = (0..pixel_count * 4).map(|i| (i % 256) as u8).collect();
        let mut buf_scalar = buf.clone();

        unpremultiply_alpha(&mut buf);
        unpremultiply_alpha_scalar(&mut buf_scalar);

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

    #[test]
    fn unpremultiply_clamps_to_255() {
        // Pathological: channel > alpha (invalid premultiplied data, but should not overflow)
        let mut buf = vec![200, 200, 200, 50];
        unpremultiply_alpha(&mut buf);
        // Values should be valid u8 (the SIMD path clamps, not wraps)
        assert_eq!(buf[0], 255); // 200 * 255 / 50 = 1020, clamped to 255
        assert_eq!(buf[1], 255);
        assert_eq!(buf[2], 255);
        assert_eq!(buf[3], 50);
    }

    #[test]
    fn premultiply_unpremultiply_roundtrip() {
        let original = vec![200u8, 150, 100, 128];
        let mut buf = original.clone();
        crate::fill::premultiply_alpha(&mut buf);
        unpremultiply_alpha(&mut buf);
        // Should be close to original (within rounding)
        for c in 0..3 {
            let diff = (buf[c] as i16 - original[c] as i16).abs();
            assert!(
                diff <= 2,
                "channel {c}: got {} expected {}",
                buf[c],
                original[c]
            );
        }
        assert_eq!(buf[3], 128);
    }

    // ── upsample_2x_bilinear tests ──────────────────────────────────

    #[test]
    fn upsample_1x1_solid() {
        let src = [100u8, 150, 200, 255]; // single pixel
        let (dst, w, h) = upsample_2x_bilinear(&src, 1, 1);
        assert_eq!(w, 2);
        assert_eq!(h, 2);
        assert_eq!(dst.len(), 16); // 2x2 = 4 pixels * 4 bytes
        // All output pixels should be the same as input (no neighbors to interpolate with)
        for i in 0..4 {
            let off = i * 4;
            assert_eq!(dst[off], 100, "pixel {i} B");
            assert_eq!(dst[off + 1], 150, "pixel {i} G");
            assert_eq!(dst[off + 2], 200, "pixel {i} R");
            assert_eq!(dst[off + 3], 255, "pixel {i} A");
        }
    }

    #[test]
    fn upsample_output_dimensions() {
        let src = vec![0u8; 3 * 5 * 4]; // 3x5 image
        let (dst, w, h) = upsample_2x_bilinear(&src, 3, 5);
        assert_eq!(w, 6);
        assert_eq!(h, 10);
        assert_eq!(dst.len(), (6 * 10 * 4) as usize);
    }

    #[test]
    fn upsample_matches_scalar() {
        // 4x3 test image with gradient
        let width = 4u32;
        let height = 3u32;
        let len = (width * height * 4) as usize;
        let src: Vec<u8> = (0..len).map(|i| ((i * 17 + 31) % 256) as u8).collect();

        let (dst_auto, w1, h1) = upsample_2x_bilinear(&src, width, height);

        let dst_w = width * 2;
        let dst_h = height * 2;
        let mut dst_scalar = vec![0u8; (dst_w * dst_h * 4) as usize];
        upsample_2x_bilinear_scalar(&src, width, height, &mut dst_scalar, dst_w);

        assert_eq!(w1, dst_w);
        assert_eq!(h1, dst_h);

        for i in 0..dst_auto.len() {
            let diff = (dst_auto[i] as i16 - dst_scalar[i] as i16).abs();
            assert!(
                diff <= 1,
                "byte {i}: auto={} scalar={}",
                dst_auto[i],
                dst_scalar[i]
            );
        }
    }

    // ── blend_constant_src_over tests ────────────────────────────────

    #[test]
    fn blend_constant_transparent_noop() {
        let original = vec![100u8, 150, 200, 255, 50, 50, 50, 128];
        let mut dst = original.clone();
        blend_constant_src_over(&mut dst, [0, 0, 0, 0]);
        assert_eq!(dst, original);
    }

    #[test]
    fn blend_constant_opaque_fills() {
        let mut dst = vec![100u8, 150, 200, 255, 50, 50, 50, 128];
        blend_constant_src_over(&mut dst, [10, 20, 30, 255]);
        for pixel in dst.chunks_exact(4) {
            assert_eq!(pixel, &[10, 20, 30, 255]);
        }
    }

    #[test]
    fn blend_constant_half_alpha() {
        let mut dst = vec![0u8, 0, 0, 255]; // black opaque
        let color = [128u8, 128, 128, 128]; // premultiplied 50% gray
        blend_constant_src_over(&mut dst, color);
        // out = src + dst * (255 - 128) / 255
        // = 128 + 0 * 127 / 255 = 128
        assert_eq!(dst[0], 128);
        assert_eq!(dst[1], 128);
        assert_eq!(dst[2], 128);
    }

    #[test]
    fn blend_constant_matches_scalar() {
        let pixel_count = 137;
        let len = pixel_count * 4;
        let color = [64u8, 128, 192, 180];

        let mut dst_scalar: Vec<u8> = (0..len).map(|i| ((i * 7 + 13) % 256) as u8).collect();
        let mut dst_auto = dst_scalar.clone();

        blend_constant_src_over_scalar(&mut dst_scalar, color);
        blend_constant_src_over(&mut dst_auto, color);

        for i in 0..len {
            let diff = (dst_scalar[i] as i16 - dst_auto[i] as i16).abs();
            assert!(
                diff <= 1,
                "byte {i}: scalar={} auto={}",
                dst_scalar[i],
                dst_auto[i]
            );
        }
    }

    #[test]
    fn blend_constant_large_buffer() {
        // 256 pixels — exercises AVX-512 + AVX2 + SSE2 tails
        let pixel_count = 256;
        let len = pixel_count * 4;
        let color = [50u8, 100, 150, 200];

        let mut dst_scalar: Vec<u8> = (0..len).map(|i| (i % 256) as u8).collect();
        let mut dst_auto = dst_scalar.clone();

        blend_constant_src_over_scalar(&mut dst_scalar, color);
        blend_constant_src_over(&mut dst_auto, color);

        for i in 0..len {
            let diff = (dst_scalar[i] as i16 - dst_auto[i] as i16).abs();
            assert!(
                diff <= 1,
                "byte {i}: scalar={} auto={}",
                dst_scalar[i],
                dst_auto[i]
            );
        }
    }

    #[test]
    fn blend_constant_empty() {
        let mut dst: [u8; 0] = [];
        blend_constant_src_over(&mut dst, [128, 128, 128, 128]); // should not panic
    }
}
