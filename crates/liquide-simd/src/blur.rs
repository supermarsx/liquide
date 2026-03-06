//! SIMD-accelerated Gaussian blur passes for BGRA8 pixel buffers.
//!
//! Provides vectorized horizontal and vertical convolution passes.
//! The inner loop processes 4 channels (BGRA) simultaneously using SSE2,
//! or multiple pixels in parallel with AVX2.

/// Apply a horizontal Gaussian blur pass on a BGRA8 buffer.
///
/// `src` and `dst` must both be `width * height * 4` bytes.
/// `weights` is a symmetric kernel of length `2 * half_width + 1`.
pub fn blur_horizontal(
    src: &[u8],
    dst: &mut [u8],
    width: u32,
    height: u32,
    half_width: usize,
    weights: &[f32],
) {
    debug_assert_eq!(src.len(), dst.len());
    debug_assert_eq!(weights.len(), half_width * 2 + 1);

    #[cfg(target_arch = "x86_64")]
    {
        if crate::detect::has_fma() {
            unsafe { return blur_horizontal_fma(src, dst, width, height, half_width, weights) }
        }
        // SAFETY: SSE2 is always available on x86-64
        unsafe {
            return blur_horizontal_sse2(src, dst, width, height, half_width, weights);
        }
    }
    #[cfg(not(target_arch = "x86_64"))]
    blur_horizontal_scalar(src, dst, width, height, half_width, weights);
}

/// Apply a vertical Gaussian blur pass on a BGRA8 buffer.
pub fn blur_vertical(
    src: &[u8],
    dst: &mut [u8],
    width: u32,
    height: u32,
    half_width: usize,
    weights: &[f32],
) {
    debug_assert_eq!(src.len(), dst.len());
    debug_assert_eq!(weights.len(), half_width * 2 + 1);

    #[cfg(target_arch = "x86_64")]
    {
        if crate::detect::has_fma() {
            unsafe { return blur_vertical_fma(src, dst, width, height, half_width, weights) }
        }
        unsafe {
            return blur_vertical_sse2(src, dst, width, height, half_width, weights);
        }
    }
    #[cfg(not(target_arch = "x86_64"))]
    blur_vertical_scalar(src, dst, width, height, half_width, weights);
}

// ── Scalar fallbacks ──────────────────────────────────────────────────

pub fn blur_horizontal_scalar(
    src: &[u8],
    dst: &mut [u8],
    width: u32,
    height: u32,
    half_width: usize,
    weights: &[f32],
) {
    let w = width as usize;
    let half = half_width as i32;

    for y in 0..height as usize {
        let row_off = y * w * 4;
        for x in 0..w {
            let mut r = 0.0f32;
            let mut g = 0.0f32;
            let mut b = 0.0f32;
            let mut a = 0.0f32;

            for (ki, &weight) in weights.iter().enumerate() {
                let sx = (x as i32 + ki as i32 - half).clamp(0, w as i32 - 1) as usize;
                let off = row_off + sx * 4;
                b += src[off] as f32 * weight;
                g += src[off + 1] as f32 * weight;
                r += src[off + 2] as f32 * weight;
                a += src[off + 3] as f32 * weight;
            }

            let out = row_off + x * 4;
            dst[out] = b.round().clamp(0.0, 255.0) as u8;
            dst[out + 1] = g.round().clamp(0.0, 255.0) as u8;
            dst[out + 2] = r.round().clamp(0.0, 255.0) as u8;
            dst[out + 3] = a.round().clamp(0.0, 255.0) as u8;
        }
    }
}

pub fn blur_vertical_scalar(
    src: &[u8],
    dst: &mut [u8],
    width: u32,
    height: u32,
    half_width: usize,
    weights: &[f32],
) {
    let w = width as usize;
    let h = height as usize;
    let half = half_width as i32;

    for y in 0..h {
        for x in 0..w {
            let mut r = 0.0f32;
            let mut g = 0.0f32;
            let mut b = 0.0f32;
            let mut a = 0.0f32;

            for (ki, &weight) in weights.iter().enumerate() {
                let sy = (y as i32 + ki as i32 - half).clamp(0, h as i32 - 1) as usize;
                let off = sy * w * 4 + x * 4;
                b += src[off] as f32 * weight;
                g += src[off + 1] as f32 * weight;
                r += src[off + 2] as f32 * weight;
                a += src[off + 3] as f32 * weight;
            }

            let out = y * w * 4 + x * 4;
            dst[out] = b.round().clamp(0.0, 255.0) as u8;
            dst[out + 1] = g.round().clamp(0.0, 255.0) as u8;
            dst[out + 2] = r.round().clamp(0.0, 255.0) as u8;
            dst[out + 3] = a.round().clamp(0.0, 255.0) as u8;
        }
    }
}

// ── SSE2 implementations ─────────────────────────────────────────────

/// SSE2 horizontal blur: processes one pixel's 4 channels (BGRA) in a single
/// __m128 register, accumulating the weighted sum across kernel taps.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn blur_horizontal_sse2(
    src: &[u8],
    dst: &mut [u8],
    width: u32,
    height: u32,
    half_width: usize,
    weights: &[f32],
) {
    use std::arch::x86_64::*;

    let w = width as usize;
    let half = half_width as i32;
    let zero = _mm_setzero_si128();
    let half_f = _mm_set1_ps(0.5);
    let max_f = _mm_set1_ps(255.0);
    let zero_f = _mm_setzero_ps();

    for y in 0..height as usize {
        let row_off = y * w * 4;
        for x in 0..w {
            let mut acc = _mm_setzero_ps(); // [B, G, R, A] as f32

            for (ki, &weight) in weights.iter().enumerate() {
                let sx = (x as i32 + ki as i32 - half).clamp(0, w as i32 - 1) as usize;
                let off = row_off + sx * 4;

                // Load 4 bytes, unpack to i32, convert to f32
                let pixel = _mm_cvtsi32_si128(i32::from_le_bytes([
                    src[off],
                    src[off + 1],
                    src[off + 2],
                    src[off + 3],
                ]));
                let pixel_16 = _mm_unpacklo_epi8(pixel, zero);
                let pixel_32 = _mm_unpacklo_epi16(pixel_16, zero);
                let pixel_f = _mm_cvtepi32_ps(pixel_32);

                let w_vec = _mm_set1_ps(weight);
                acc = _mm_add_ps(acc, _mm_mul_ps(pixel_f, w_vec));
            }

            // Round, clamp [0, 255], convert to u8
            acc = _mm_add_ps(acc, half_f);
            acc = _mm_max_ps(_mm_min_ps(acc, max_f), zero_f);
            let int = _mm_cvttps_epi32(acc);
            // Pack i32 → i16 → u8
            let packed_16 = _mm_packs_epi32(int, int);
            let packed_8 = _mm_packus_epi16(packed_16, packed_16);
            let val = _mm_cvtsi128_si32(packed_8) as u32;

            let out = row_off + x * 4;
            dst[out..out + 4].copy_from_slice(&val.to_le_bytes());
        }
    }
}

/// SSE2 vertical blur: same strategy but samples down the column.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn blur_vertical_sse2(
    src: &[u8],
    dst: &mut [u8],
    width: u32,
    height: u32,
    half_width: usize,
    weights: &[f32],
) {
    use std::arch::x86_64::*;

    let w = width as usize;
    let h = height as usize;
    let half = half_width as i32;
    let zero = _mm_setzero_si128();
    let half_f = _mm_set1_ps(0.5);
    let max_f = _mm_set1_ps(255.0);
    let zero_f = _mm_setzero_ps();

    for y in 0..h {
        for x in 0..w {
            let mut acc = _mm_setzero_ps();

            for (ki, &weight) in weights.iter().enumerate() {
                let sy = (y as i32 + ki as i32 - half).clamp(0, h as i32 - 1) as usize;
                let off = sy * w * 4 + x * 4;

                let pixel = _mm_cvtsi32_si128(i32::from_le_bytes([
                    src[off],
                    src[off + 1],
                    src[off + 2],
                    src[off + 3],
                ]));
                let pixel_16 = _mm_unpacklo_epi8(pixel, zero);
                let pixel_32 = _mm_unpacklo_epi16(pixel_16, zero);
                let pixel_f = _mm_cvtepi32_ps(pixel_32);

                let w_vec = _mm_set1_ps(weight);
                acc = _mm_add_ps(acc, _mm_mul_ps(pixel_f, w_vec));
            }

            acc = _mm_add_ps(acc, half_f);
            acc = _mm_max_ps(_mm_min_ps(acc, max_f), zero_f);
            let int = _mm_cvttps_epi32(acc);
            let packed_16 = _mm_packs_epi32(int, int);
            let packed_8 = _mm_packus_epi16(packed_16, packed_16);
            let val = _mm_cvtsi128_si32(packed_8) as u32;

            let out = y * w * 4 + x * 4;
            dst[out..out + 4].copy_from_slice(&val.to_le_bytes());
        }
    }
}

// ── FMA implementations ──────────────────────────────────────────────

/// FMA horizontal blur: uses `_mm_fmadd_ps` for fused multiply-add,
/// giving better precision (no intermediate rounding) and fewer instructions.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "fma")]
unsafe fn blur_horizontal_fma(
    src: &[u8],
    dst: &mut [u8],
    width: u32,
    height: u32,
    half_width: usize,
    weights: &[f32],
) {
    use std::arch::x86_64::*;

    let w = width as usize;
    let half = half_width as i32;
    let zero = _mm_setzero_si128();
    let half_f = _mm_set1_ps(0.5);
    let max_f = _mm_set1_ps(255.0);
    let zero_f = _mm_setzero_ps();

    for y in 0..height as usize {
        let row_off = y * w * 4;
        for x in 0..w {
            let mut acc = _mm_setzero_ps(); // [B, G, R, A] as f32

            for (ki, &weight) in weights.iter().enumerate() {
                let sx = (x as i32 + ki as i32 - half).clamp(0, w as i32 - 1) as usize;
                let off = row_off + sx * 4;

                // Load 4 bytes, unpack to i32, convert to f32
                let pixel = _mm_cvtsi32_si128(i32::from_le_bytes([
                    src[off],
                    src[off + 1],
                    src[off + 2],
                    src[off + 3],
                ]));
                let pixel_16 = _mm_unpacklo_epi8(pixel, zero);
                let pixel_32 = _mm_unpacklo_epi16(pixel_16, zero);
                let pixel_f = _mm_cvtepi32_ps(pixel_32);

                let w_vec = _mm_set1_ps(weight);
                acc = _mm_fmadd_ps(pixel_f, w_vec, acc);
            }

            // Round, clamp [0, 255], convert to u8
            acc = _mm_add_ps(acc, half_f);
            acc = _mm_max_ps(_mm_min_ps(acc, max_f), zero_f);
            let int = _mm_cvttps_epi32(acc);
            // Pack i32 → i16 → u8
            let packed_16 = _mm_packs_epi32(int, int);
            let packed_8 = _mm_packus_epi16(packed_16, packed_16);
            let val = _mm_cvtsi128_si32(packed_8) as u32;

            let out = row_off + x * 4;
            dst[out..out + 4].copy_from_slice(&val.to_le_bytes());
        }
    }
}

/// FMA vertical blur: same strategy but samples down the column.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "fma")]
unsafe fn blur_vertical_fma(
    src: &[u8],
    dst: &mut [u8],
    width: u32,
    height: u32,
    half_width: usize,
    weights: &[f32],
) {
    use std::arch::x86_64::*;

    let w = width as usize;
    let h = height as usize;
    let half = half_width as i32;
    let zero = _mm_setzero_si128();
    let half_f = _mm_set1_ps(0.5);
    let max_f = _mm_set1_ps(255.0);
    let zero_f = _mm_setzero_ps();

    for y in 0..h {
        for x in 0..w {
            let mut acc = _mm_setzero_ps();

            for (ki, &weight) in weights.iter().enumerate() {
                let sy = (y as i32 + ki as i32 - half).clamp(0, h as i32 - 1) as usize;
                let off = sy * w * 4 + x * 4;

                let pixel = _mm_cvtsi32_si128(i32::from_le_bytes([
                    src[off],
                    src[off + 1],
                    src[off + 2],
                    src[off + 3],
                ]));
                let pixel_16 = _mm_unpacklo_epi8(pixel, zero);
                let pixel_32 = _mm_unpacklo_epi16(pixel_16, zero);
                let pixel_f = _mm_cvtepi32_ps(pixel_32);

                let w_vec = _mm_set1_ps(weight);
                acc = _mm_fmadd_ps(pixel_f, w_vec, acc);
            }

            acc = _mm_add_ps(acc, half_f);
            acc = _mm_max_ps(_mm_min_ps(acc, max_f), zero_f);
            let int = _mm_cvttps_epi32(acc);
            let packed_16 = _mm_packs_epi32(int, int);
            let packed_8 = _mm_packus_epi16(packed_16, packed_16);
            let val = _mm_cvtsi128_si32(packed_8) as u32;

            let out = y * w * 4 + x * 4;
            dst[out..out + 4].copy_from_slice(&val.to_le_bytes());
        }
    }
}

/// 2x box-filter downsample: each 2x2 block → one pixel (average).
///
/// Returns `(buffer, new_width, new_height)`.
#[must_use]
pub fn downsample_2x(src: &[u8], width: u32, height: u32) -> (Vec<u8>, u32, u32) {
    let dw = width / 2;
    let dh = height / 2;
    if dw == 0 || dh == 0 {
        return (Vec::new(), 0, 0);
    }

    #[cfg(target_arch = "x86_64")]
    {
        unsafe { return downsample_2x_sse2(src, width, height, dw, dh) }
    }
    #[cfg(not(target_arch = "x86_64"))]
    downsample_2x_scalar(src, width, dw, dh)
}

fn downsample_2x_scalar(src: &[u8], width: u32, dw: u32, dh: u32) -> (Vec<u8>, u32, u32) {
    let sw = width as usize;
    let mut dst = vec![0u8; (dw * dh * 4) as usize];

    for y in 0..dh as usize {
        for x in 0..dw as usize {
            let sx = x * 2;
            let sy = y * 2;
            let mut sum = [0u32; 4];

            for dy in 0..2usize {
                for dx in 0..2usize {
                    let off = ((sy + dy) * sw + (sx + dx)) * 4;
                    for c in 0..4 {
                        sum[c] += src[off + c] as u32;
                    }
                }
            }

            let out = (y * dw as usize + x) * 4;
            for c in 0..4 {
                dst[out + c] = (sum[c] / 4) as u8;
            }
        }
    }

    (dst, dw, dh)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn downsample_2x_sse2(
    src: &[u8],
    width: u32,
    _height: u32,
    dw: u32,
    dh: u32,
) -> (Vec<u8>, u32, u32) {
    use std::arch::x86_64::*;

    let sw = width as usize;
    let mut dst = vec![0u8; (dw * dh * 4) as usize];
    let zero = _mm_setzero_si128();

    for y in 0..dh as usize {
        for x in 0..dw as usize {
            let sx = x * 2;
            let sy = y * 2;

            // Load 4 pixels from a 2x2 block
            let off00 = (sy * sw + sx) * 4;
            let off10 = (sy * sw + sx + 1) * 4;
            let off01 = ((sy + 1) * sw + sx) * 4;
            let off11 = ((sy + 1) * sw + sx + 1) * 4;

            let load4 = |off: usize| -> __m128i {
                let pixel = _mm_cvtsi32_si128(i32::from_le_bytes([
                    src[off], src[off + 1], src[off + 2], src[off + 3],
                ]));
                _mm_unpacklo_epi8(pixel, zero)
            };

            let p00 = _mm_unpacklo_epi16(load4(off00), zero);
            let p10 = _mm_unpacklo_epi16(load4(off10), zero);
            let p01 = _mm_unpacklo_epi16(load4(off01), zero);
            let p11 = _mm_unpacklo_epi16(load4(off11), zero);

            let sum = _mm_add_epi32(_mm_add_epi32(p00, p10), _mm_add_epi32(p01, p11));
            let avg = _mm_srli_epi32::<2>(sum); // / 4

            // Pack back to u8
            let packed_16 = _mm_packs_epi32(avg, avg);
            let packed_8 = _mm_packus_epi16(packed_16, packed_16);
            let val = _mm_cvtsi128_si32(packed_8) as u32;

            let out = (y * dw as usize + x) * 4;
            dst[out..out + 4].copy_from_slice(&val.to_le_bytes());
        }
    }

    (dst, dw, dh)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity_kernel() -> (usize, Vec<f32>) {
        (0, vec![1.0])
    }

    fn box_kernel_3() -> (usize, Vec<f32>) {
        (1, vec![1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0])
    }

    #[test]
    fn identity_kernel_preserves_data() {
        let (half, weights) = identity_kernel();
        let src = vec![100u8, 150, 200, 255, 50, 75, 25, 128];
        let mut dst = vec![0u8; 8];
        blur_horizontal(&src, &mut dst, 2, 1, half, &weights);
        assert_eq!(dst, src);
    }

    #[test]
    fn blur_uniform_unchanged() {
        let (half, weights) = box_kernel_3();
        // Uniform color should be unchanged after blur
        let src = vec![128u8; 4 * 4 * 4]; // 4x4 image
        let mut dst = vec![0u8; src.len()];
        blur_horizontal(&src, &mut dst, 4, 4, half, &weights);

        // Interior pixels should be ~128
        for &b in &dst[4..12] {
            assert!((b as i16 - 128).abs() <= 1);
        }
    }

    #[test]
    fn horizontal_matches_scalar() {
        let (half, weights) = box_kernel_3();
        let w = 16u32;
        let h = 4u32;
        let src: Vec<u8> = (0..(w * h * 4) as usize).map(|i| (i % 256) as u8).collect();
        let mut dst_scalar = vec![0u8; src.len()];
        let mut dst_simd = vec![0u8; src.len()];

        blur_horizontal_scalar(&src, &mut dst_scalar, w, h, half, &weights);
        blur_horizontal(&src, &mut dst_simd, w, h, half, &weights);

        for i in 0..src.len() {
            let diff = (dst_scalar[i] as i16 - dst_simd[i] as i16).abs();
            assert!(diff <= 1, "byte {i}: scalar={} simd={}", dst_scalar[i], dst_simd[i]);
        }
    }

    #[test]
    fn vertical_matches_scalar() {
        let (half, weights) = box_kernel_3();
        let w = 8u32;
        let h = 8u32;
        let src: Vec<u8> = (0..(w * h * 4) as usize).map(|i| (i % 256) as u8).collect();
        let mut dst_scalar = vec![0u8; src.len()];
        let mut dst_simd = vec![0u8; src.len()];

        blur_vertical_scalar(&src, &mut dst_scalar, w, h, half, &weights);
        blur_vertical(&src, &mut dst_simd, w, h, half, &weights);

        for i in 0..src.len() {
            let diff = (dst_scalar[i] as i16 - dst_simd[i] as i16).abs();
            assert!(diff <= 1, "byte {i}: scalar={} simd={}", dst_scalar[i], dst_simd[i]);
        }
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn fma_horizontal_matches_scalar() {
        if !crate::detect::has_fma() {
            return;
        }
        let (half, weights) = box_kernel_3();
        let w = 16u32;
        let h = 4u32;
        let src: Vec<u8> = (0..(w * h * 4) as usize).map(|i| (i % 256) as u8).collect();
        let mut dst_scalar = vec![0u8; src.len()];
        let mut dst_fma = vec![0u8; src.len()];

        blur_horizontal_scalar(&src, &mut dst_scalar, w, h, half, &weights);
        unsafe {
            blur_horizontal_fma(&src, &mut dst_fma, w, h, half, &weights);
        }

        for i in 0..src.len() {
            let diff = (dst_scalar[i] as i16 - dst_fma[i] as i16).abs();
            assert!(diff <= 1, "byte {i}: scalar={} fma={}", dst_scalar[i], dst_fma[i]);
        }
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn fma_vertical_matches_scalar() {
        if !crate::detect::has_fma() {
            return;
        }
        let (half, weights) = box_kernel_3();
        let w = 8u32;
        let h = 8u32;
        let src: Vec<u8> = (0..(w * h * 4) as usize).map(|i| (i % 256) as u8).collect();
        let mut dst_scalar = vec![0u8; src.len()];
        let mut dst_fma = vec![0u8; src.len()];

        blur_vertical_scalar(&src, &mut dst_scalar, w, h, half, &weights);
        unsafe {
            blur_vertical_fma(&src, &mut dst_fma, w, h, half, &weights);
        }

        for i in 0..src.len() {
            let diff = (dst_scalar[i] as i16 - dst_fma[i] as i16).abs();
            assert!(diff <= 1, "byte {i}: scalar={} fma={}", dst_scalar[i], dst_fma[i]);
        }
    }

    #[test]
    fn downsample_2x_averages() {
        // 2x2 image: top-left=white, rest=black → average = (255+0+0+0)/4 ≈ 63
        #[rustfmt::skip]
        let src = vec![
            255, 255, 255, 255,   0, 0, 0, 0,
              0,   0,   0,   0,   0, 0, 0, 0,
        ];
        let (buf, dw, dh) = downsample_2x(&src, 2, 2);
        assert_eq!(dw, 1);
        assert_eq!(dh, 1);
        assert_eq!(buf.len(), 4);
        assert_eq!(buf[0], 63); // B
        assert_eq!(buf[1], 63); // G
        assert_eq!(buf[2], 63); // R
        assert_eq!(buf[3], 63); // A
    }
}
