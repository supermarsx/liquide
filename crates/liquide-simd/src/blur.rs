//! SIMD-accelerated Gaussian blur passes for BGRA8 pixel buffers.
//!
//! Provides vectorized horizontal and vertical convolution passes.
//! Dispatch is runtime-detected (`is_x86_feature_detected!`, cached via
//! [`crate::detect`]) so a binary built for the SSE2 baseline still uses the
//! widest path the *running* CPU supports — no `target-feature` build flag and
//! no SIGILL on older CPUs:
//!
//! - **SSE2 / FMA** (baseline): one pixel's 4 BGRA channels packed in one
//!   128-bit lane, accumulated across kernel taps.
//! - **AVX2 (+FMA)**: multiple pixels per iteration. The horizontal pass packs
//!   pixel `x` / `x+1` into the two 128-bit halves of a 256-bit register (2
//!   px/iter); the vertical pass exploits unit column stride to load 4 contiguous
//!   pixels with one 128-bit load and accumulate them in two 256-bit registers
//!   (4 px/iter). Every output lane runs the *same* per-pixel FMA accumulation in
//!   the *same* tap order as the 128-bit FMA path, so the output is
//!   **bit-for-bit identical** to the FMA path (the live capture / golden path) —
//!   only the throughput changes, never the math.
//!
//! Byte-identical determinism is load-bearing: the backdrop-blur capture path
//! feeds goldens + `e2e_temporal`. The AVX2 path is therefore gated on
//! `avx2 && fma` so it reproduces the FMA reference exactly (fused multiply-add,
//! same rounding `+0.5`/truncate, same `[0,255]` clamp, same edge clamp).

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
    assert_eq!(src.len(), dst.len());
    assert_eq!(weights.len(), half_width * 2 + 1);

    #[cfg(target_arch = "x86_64")]
    {
        // AVX2 needs FMA so its per-pixel math is bit-identical to the FMA path.
        if crate::detect::has_avx2() && crate::detect::has_fma() {
            // SAFETY: avx2 + fma detected at runtime.
            unsafe { return blur_horizontal_avx2(src, dst, width, height, half_width, weights) }
        }
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
    assert_eq!(src.len(), dst.len());
    assert_eq!(weights.len(), half_width * 2 + 1);

    #[cfg(target_arch = "x86_64")]
    {
        // AVX2 needs FMA so its per-pixel math is bit-identical to the FMA path.
        if crate::detect::has_avx2() && crate::detect::has_fma() {
            // SAFETY: avx2 + fma detected at runtime.
            unsafe { return blur_vertical_avx2(src, dst, width, height, half_width, weights) }
        }
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

// ── AVX2 implementations (multi-pixel / iteration) ───────────────────
//
// Each output pixel runs the EXACT same per-pixel FMA accumulation (same tap
// order, same `_mm*_fmadd_ps`, same round/clamp) as the 128-bit FMA path — a
// 256-bit register just carries two such independent accumulations side by
// side (low half = pixel i, high half = pixel i+1). IEEE-754 FMA is lanewise,
// so the result is bit-for-bit identical to `blur_*_fma`, only wider. The
// horizontal pass does 2 px/iter (gathered, since adjacent outputs sample
// differently-clamped source columns); the vertical pass does 4 px/iter via a
// single contiguous 16-byte load (unit column stride). Leftover pixels fall
// through to a 128-bit FMA tail, itself bit-identical to `blur_*_fma`.

/// Pack one BGRA pixel's 4 bytes (at `src[off..off+4]`) into the low 4 f32
/// lanes of a 128-bit register: `[B, G, R, A]`.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
#[inline]
unsafe fn load_pixel_f128(src: &[u8], off: usize) -> std::arch::x86_64::__m128 {
    use std::arch::x86_64::*;
    let zero = _mm_setzero_si128();
    let pixel = _mm_cvtsi32_si128(i32::from_le_bytes([
        src[off],
        src[off + 1],
        src[off + 2],
        src[off + 3],
    ]));
    let pixel_16 = _mm_unpacklo_epi8(pixel, zero);
    let pixel_32 = _mm_unpacklo_epi16(pixel_16, zero);
    _mm_cvtepi32_ps(pixel_32)
}

/// Round (`+0.5`, truncate), clamp to `[0,255]`, pack the low 4 f32 lanes of
/// `acc128` to 4 BGRA bytes and write them to `dst[out..out+4]`.
/// Identical math to the FMA path's finalize step.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
#[inline]
unsafe fn store_pixel_f128(acc128: std::arch::x86_64::__m128, dst: &mut [u8], out: usize) {
    use std::arch::x86_64::*;
    let half_f = _mm_set1_ps(0.5);
    let max_f = _mm_set1_ps(255.0);
    let zero_f = _mm_setzero_ps();
    let mut acc = _mm_add_ps(acc128, half_f);
    acc = _mm_max_ps(_mm_min_ps(acc, max_f), zero_f);
    let int = _mm_cvttps_epi32(acc);
    let packed_16 = _mm_packs_epi32(int, int);
    let packed_8 = _mm_packus_epi16(packed_16, packed_16);
    let val = _mm_cvtsi128_si32(packed_8) as u32;
    dst[out..out + 4].copy_from_slice(&val.to_le_bytes());
}

/// AVX2 horizontal blur: two output pixels per iteration. Pixel `x` occupies
/// the low 128-bit half of the 256-bit accumulator, pixel `x+1` the high half;
/// each tap broadcasts the weight to all 8 lanes and FMAs the two (separately
/// edge-clamped) source pixels. Bit-identical to `blur_horizontal_fma`.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2,fma")]
unsafe fn blur_horizontal_avx2(
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
    let max_x = w as i32 - 1;

    for y in 0..height as usize {
        let row_off = y * w * 4;
        let mut x = 0usize;

        // Process pairs of output pixels.
        while x + 1 < w {
            let mut acc = _mm256_setzero_ps(); // [B0,G0,R0,A0, B1,G1,R1,A1]

            for (ki, &weight) in weights.iter().enumerate() {
                let k = ki as i32 - half;
                let sx0 = (x as i32 + k).clamp(0, max_x) as usize;
                let sx1 = (x as i32 + 1 + k).clamp(0, max_x) as usize;

                let p0 = load_pixel_f128(src, row_off + sx0 * 4); // low half
                let p1 = load_pixel_f128(src, row_off + sx1 * 4); // high half
                let pixels = _mm256_insertf128_ps(_mm256_castps128_ps256(p0), p1, 1);

                let w_vec = _mm256_set1_ps(weight);
                acc = _mm256_fmadd_ps(pixels, w_vec, acc);
            }

            store_pixel_f128(_mm256_castps256_ps128(acc), dst, row_off + x * 4);
            store_pixel_f128(_mm256_extractf128_ps(acc, 1), dst, row_off + (x + 1) * 4);
            x += 2;
        }

        // Tail: odd trailing pixel — 128-bit FMA, identical to blur_horizontal_fma.
        while x < w {
            let mut acc = _mm_setzero_ps();
            for (ki, &weight) in weights.iter().enumerate() {
                let sx = (x as i32 + ki as i32 - half).clamp(0, max_x) as usize;
                let p = load_pixel_f128(src, row_off + sx * 4);
                acc = _mm_fmadd_ps(p, _mm_set1_ps(weight), acc);
            }
            store_pixel_f128(acc, dst, row_off + x * 4);
            x += 1;
        }
    }
}

/// AVX2 vertical blur: FOUR contiguous output pixels per iteration. The
/// vertical pass has unit column stride, so the four pixels `(x..x+4)` at any
/// tap row `sy` are contiguous (16 bytes) and share the same `sy`. A single
/// 128-bit load brings in all four; they widen into two 256-bit accumulators
/// (pixels 0,1 / pixels 2,3), each running the SAME per-pixel FMA in the SAME
/// tap order as `blur_vertical_fma`, so every output lane is bit-identical —
/// the wider load is the only difference. Remainders (2-px then 1-px) fall
/// through to the same FMA tail.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2,fma")]
unsafe fn blur_vertical_avx2(
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
    let max_y = h as i32 - 1;
    let zero = _mm_setzero_si128();

    for y in 0..h {
        let out_row = y * w * 4;
        let mut x = 0usize;

        // Body: 4 pixels (16 contiguous bytes) per iteration.
        while x + 4 <= w {
            let mut acc_lo = _mm256_setzero_ps(); // pixels x, x+1
            let mut acc_hi = _mm256_setzero_ps(); // pixels x+2, x+3

            for (ki, &weight) in weights.iter().enumerate() {
                let sy = (y as i32 + ki as i32 - half).clamp(0, max_y) as usize;
                let off = sy * w * 4 + x * 4;

                // Load 16 bytes = 4 BGRA pixels.
                let bytes = _mm_loadu_si128(src.as_ptr().add(off) as *const __m128i);
                // Widen u8 -> u16 (low 8 bytes = px0,px1; high 8 bytes = px2,px3).
                let lo16 = _mm_unpacklo_epi8(bytes, zero); // px0,px1 channels
                let hi16 = _mm_unpackhi_epi8(bytes, zero); // px2,px3 channels
                // Widen u16 -> i32 -> f32, 4 lanes each, then join two 128s into 256.
                let p01 = _mm256_cvtepi32_ps(_mm256_set_m128i(
                    _mm_unpackhi_epi16(lo16, zero), // px1
                    _mm_unpacklo_epi16(lo16, zero), // px0
                ));
                let p23 = _mm256_cvtepi32_ps(_mm256_set_m128i(
                    _mm_unpackhi_epi16(hi16, zero), // px3
                    _mm_unpacklo_epi16(hi16, zero), // px2
                ));

                let w_vec = _mm256_set1_ps(weight);
                acc_lo = _mm256_fmadd_ps(p01, w_vec, acc_lo);
                acc_hi = _mm256_fmadd_ps(p23, w_vec, acc_hi);
            }

            store_pixel_f128(_mm256_castps256_ps128(acc_lo), dst, out_row + x * 4);
            store_pixel_f128(_mm256_extractf128_ps(acc_lo, 1), dst, out_row + (x + 1) * 4);
            store_pixel_f128(_mm256_castps256_ps128(acc_hi), dst, out_row + (x + 2) * 4);
            store_pixel_f128(_mm256_extractf128_ps(acc_hi, 1), dst, out_row + (x + 3) * 4);
            x += 4;
        }

        // Remainder: 2 pixels via one 256-bit accumulator.
        while x + 2 <= w {
            let mut acc = _mm256_setzero_ps();
            for (ki, &weight) in weights.iter().enumerate() {
                let sy = (y as i32 + ki as i32 - half).clamp(0, max_y) as usize;
                let off = sy * w * 4 + x * 4;
                let p0 = load_pixel_f128(src, off);
                let p1 = load_pixel_f128(src, off + 4);
                let pixels = _mm256_insertf128_ps(_mm256_castps128_ps256(p0), p1, 1);
                acc = _mm256_fmadd_ps(pixels, _mm256_set1_ps(weight), acc);
            }
            store_pixel_f128(_mm256_castps256_ps128(acc), dst, out_row + x * 4);
            store_pixel_f128(_mm256_extractf128_ps(acc, 1), dst, out_row + (x + 1) * 4);
            x += 2;
        }

        // Tail: final odd pixel — 128-bit FMA, identical to blur_vertical_fma.
        while x < w {
            let mut acc = _mm_setzero_ps();
            for (ki, &weight) in weights.iter().enumerate() {
                let sy = (y as i32 + ki as i32 - half).clamp(0, max_y) as usize;
                let off = sy * w * 4 + x * 4;
                let p = load_pixel_f128(src, off);
                acc = _mm_fmadd_ps(p, _mm_set1_ps(weight), acc);
            }
            store_pixel_f128(acc, dst, out_row + x * 4);
            x += 1;
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
                    src[off],
                    src[off + 1],
                    src[off + 2],
                    src[off + 3],
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
            assert!(
                diff <= 1,
                "byte {i}: scalar={} simd={}",
                dst_scalar[i],
                dst_simd[i]
            );
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
            assert!(
                diff <= 1,
                "byte {i}: scalar={} simd={}",
                dst_scalar[i],
                dst_simd[i]
            );
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
            assert!(
                diff <= 1,
                "byte {i}: scalar={} fma={}",
                dst_scalar[i],
                dst_fma[i]
            );
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
            assert!(
                diff <= 1,
                "byte {i}: scalar={} fma={}",
                dst_scalar[i],
                dst_fma[i]
            );
        }
    }

    // ── AVX2 byte-identical proof ────────────────────────────────────
    //
    // The live capture/golden path dispatches to the FMA kernel on any modern
    // CPU. The AVX2 path MUST reproduce it bit-for-bit (not just ±1) so blur
    // output is unchanged. These tests run BOTH kernels in-process and assert
    // exact equality across the required radii and random inputs, gated on
    // runtime AVX2 detection (skipped on CPUs without it — no SIGILL).

    /// Build a Gaussian kernel identical to renderer-cpu's `GaussianKernel`
    /// (truncated at radius taps each side, σ = radius/3, normalised).
    fn gaussian_kernel(radius: u32) -> (usize, Vec<f32>) {
        if radius == 0 {
            return (0, vec![1.0]);
        }
        let sigma = radius as f64 / 3.0;
        let half = radius as usize;
        let size = half * 2 + 1;
        let mut weights = Vec::with_capacity(size);
        let mut sum = 0.0f64;
        for i in 0..size {
            let x = i as f64 - half as f64;
            let wv = (-x * x / (2.0 * sigma * sigma)).exp();
            weights.push(wv as f32);
            sum += wv;
        }
        let inv = 1.0 / sum as f32;
        for wv in &mut weights {
            *wv *= inv;
        }
        (half, weights)
    }

    /// Tiny deterministic LCG so the "random" inputs are reproducible.
    fn lcg_fill(n: usize, mut seed: u64) -> Vec<u8> {
        let mut v = Vec::with_capacity(n);
        for _ in 0..n {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            v.push((seed >> 33) as u8);
        }
        v
    }

    const TEST_RADII: [u32; 7] = [1, 3, 7, 8, 9, 16, 32];

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn avx2_horizontal_is_bit_identical_to_fma() {
        if !(is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma")) {
            eprintln!("skipping: AVX2+FMA not available on this CPU");
            return;
        }
        // Mix of even and odd widths to exercise the 2-px body + scalar tail.
        let dims = [(16u32, 4u32), (17, 5), (1, 3), (2, 2), (33, 7), (64, 1)];
        for (seed, (w, h)) in dims.iter().enumerate() {
            let (w, h) = (*w, *h);
            let src = lcg_fill((w * h * 4) as usize, 0x1234_5678 ^ seed as u64);
            for &r in &TEST_RADII {
                let (half, weights) = gaussian_kernel(r);
                if half * 2 + 1 > w as usize * 4 {
                    // kernel wider than line is still fine (clamped), no skip
                }
                let mut dst_fma = vec![0u8; src.len()];
                let mut dst_avx2 = vec![0u8; src.len()];
                unsafe {
                    blur_horizontal_fma(&src, &mut dst_fma, w, h, half, &weights);
                    blur_horizontal_avx2(&src, &mut dst_avx2, w, h, half, &weights);
                }
                assert_eq!(
                    dst_fma, dst_avx2,
                    "H pass diverged: w={w} h={h} r={r} (must be bit-identical)"
                );
            }
        }
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn avx2_vertical_is_bit_identical_to_fma() {
        if !(is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma")) {
            eprintln!("skipping: AVX2+FMA not available on this CPU");
            return;
        }
        let dims = [(16u32, 8u32), (17, 9), (3, 1), (2, 2), (33, 33), (1, 64)];
        for (seed, (w, h)) in dims.iter().enumerate() {
            let (w, h) = (*w, *h);
            let src = lcg_fill((w * h * 4) as usize, 0xC0FF_EE00 ^ seed as u64);
            for &r in &TEST_RADII {
                let (half, weights) = gaussian_kernel(r);
                let mut dst_fma = vec![0u8; src.len()];
                let mut dst_avx2 = vec![0u8; src.len()];
                unsafe {
                    blur_vertical_fma(&src, &mut dst_fma, w, h, half, &weights);
                    blur_vertical_avx2(&src, &mut dst_avx2, w, h, half, &weights);
                }
                assert_eq!(
                    dst_fma, dst_avx2,
                    "V pass diverged: w={w} h={h} r={r} (must be bit-identical)"
                );
            }
        }
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn avx2_full_separable_blur_is_bit_identical_to_fma() {
        // Proves the two-pass (H then V) pipeline — exactly what the worker's
        // small-radius `blur_buffer` and the downsample inner blur run — is
        // byte-identical end to end, including the downsample (r>=8) regime's
        // half-res blur which goes through these same kernels.
        if !(is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma")) {
            eprintln!("skipping: AVX2+FMA not available on this CPU");
            return;
        }
        let (w, h) = (40u32, 30u32);
        let src = lcg_fill((w * h * 4) as usize, 0xDEAD_BEEF);
        for &r in &TEST_RADII {
            let (half, weights) = gaussian_kernel(r);

            let mut tmp_fma = vec![0u8; src.len()];
            let mut out_fma = vec![0u8; src.len()];
            let mut tmp_avx2 = vec![0u8; src.len()];
            let mut out_avx2 = vec![0u8; src.len()];
            unsafe {
                blur_horizontal_fma(&src, &mut tmp_fma, w, h, half, &weights);
                blur_vertical_fma(&tmp_fma, &mut out_fma, w, h, half, &weights);
                blur_horizontal_avx2(&src, &mut tmp_avx2, w, h, half, &weights);
                blur_vertical_avx2(&tmp_avx2, &mut out_avx2, w, h, half, &weights);
            }
            assert_eq!(out_fma, out_avx2, "separable blur diverged at r={r}");
        }
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn avx2_matches_public_dispatch_entry_point() {
        // The public `blur_horizontal`/`blur_vertical` pick AVX2 at runtime on a
        // capable CPU; assert that result equals the FMA reference too, so the
        // dispatch wiring (not just the kernel) is proven byte-identical.
        if !(is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma")) {
            return;
        }
        let (w, h) = (23u32, 11u32);
        let src = lcg_fill((w * h * 4) as usize, 0xABCD_1234);
        let (half, weights) = gaussian_kernel(9);

        let mut via_dispatch_h = vec![0u8; src.len()];
        let mut via_fma_h = vec![0u8; src.len()];
        blur_horizontal(&src, &mut via_dispatch_h, w, h, half, &weights);
        unsafe { blur_horizontal_fma(&src, &mut via_fma_h, w, h, half, &weights) };
        assert_eq!(via_dispatch_h, via_fma_h);

        let mut via_dispatch_v = vec![0u8; src.len()];
        let mut via_fma_v = vec![0u8; src.len()];
        blur_vertical(&src, &mut via_dispatch_v, w, h, half, &weights);
        unsafe { blur_vertical_fma(&src, &mut via_fma_v, w, h, half, &weights) };
        assert_eq!(via_dispatch_v, via_fma_v);
    }

    /// Hand benchmark: FMA (old live path) vs AVX2 (new) for a full separable
    /// blur over a realistic glass region. Ignored by default; run with
    /// `cargo test -p liquide-simd --offline --release -- --ignored --nocapture blur_bench`.
    #[test]
    #[ignore]
    #[cfg(target_arch = "x86_64")]
    fn blur_bench_fma_vs_avx2() {
        use std::time::Instant;
        if !(is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma")) {
            eprintln!("AVX2+FMA not available; cannot benchmark");
            return;
        }
        let (w, h) = (480u32, 320u32); // representative glass region
        let src = lcg_fill((w * h * 4) as usize, 0x9E37_79B9);
        let mut tmp = vec![0u8; src.len()];
        let mut out = vec![0u8; src.len()];

        for &r in &[8u32, 16, 32] {
            let (half, weights) = gaussian_kernel(r);
            let iters = 50;

            let t = Instant::now();
            for _ in 0..iters {
                unsafe {
                    blur_horizontal_fma(&src, &mut tmp, w, h, half, &weights);
                    blur_vertical_fma(&tmp, &mut out, w, h, half, &weights);
                }
            }
            let fma_ms = t.elapsed().as_secs_f64() * 1000.0 / iters as f64;

            let t = Instant::now();
            for _ in 0..iters {
                unsafe {
                    blur_horizontal_avx2(&src, &mut tmp, w, h, half, &weights);
                    blur_vertical_avx2(&tmp, &mut out, w, h, half, &weights);
                }
            }
            let avx2_ms = t.elapsed().as_secs_f64() * 1000.0 / iters as f64;

            eprintln!(
                "blur {w}x{h} r={r:>2}: FMA={fma_ms:7.3}ms  AVX2={avx2_ms:7.3}ms  speedup={:.2}x",
                fma_ms / avx2_ms
            );
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
