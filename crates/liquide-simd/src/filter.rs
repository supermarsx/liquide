//! SIMD-accelerated pixel color filters for BGRA8 scanlines.
//!
//! Provides vectorized versions of brightness, contrast, grayscale,
//! sepia, saturation, invert, and generic 5×4 color matrix transforms.

/// Apply a brightness factor to a BGRA8 scanline in-place.
///
/// Each color channel is multiplied by `factor`. Alpha is preserved.
/// `factor = 1.0` is identity, `0.0` is black, `2.0` is double brightness.
pub fn brightness(buf: &mut [u8], factor: f32) {
    assert_eq!(buf.len() % 4, 0);

    #[cfg(target_arch = "x86_64")]
    {
        unsafe { return brightness_sse2(buf, factor) }
    }
    #[cfg(not(target_arch = "x86_64"))]
    brightness_scalar(buf, factor);
}

fn brightness_scalar(buf: &mut [u8], factor: f32) {
    let pixels = buf.len() / 4;
    for i in 0..pixels {
        let off = i * 4;
        buf[off] = clamp_u8(buf[off] as f32 * factor);
        buf[off + 1] = clamp_u8(buf[off + 1] as f32 * factor);
        buf[off + 2] = clamp_u8(buf[off + 2] as f32 * factor);
    }
}

/// Apply contrast adjustment to a BGRA8 scanline in-place.
///
/// `factor = 1.0` is identity, `0.0` is all gray, `2.0` is double contrast.
pub fn contrast(buf: &mut [u8], factor: f32) {
    assert_eq!(buf.len() % 4, 0);

    #[cfg(target_arch = "x86_64")]
    {
        unsafe { return contrast_sse2(buf, factor) }
    }
    #[cfg(not(target_arch = "x86_64"))]
    contrast_scalar(buf, factor);
}

fn contrast_scalar(buf: &mut [u8], factor: f32) {
    let pixels = buf.len() / 4;
    for i in 0..pixels {
        let off = i * 4;
        buf[off] = clamp_u8((buf[off] as f32 - 128.0) * factor + 128.0);
        buf[off + 1] = clamp_u8((buf[off + 1] as f32 - 128.0) * factor + 128.0);
        buf[off + 2] = clamp_u8((buf[off + 2] as f32 - 128.0) * factor + 128.0);
    }
}

/// Convert a BGRA8 scanline to grayscale in-place (BT.709 luminance).
///
/// `Y = 0.2126R + 0.7152G + 0.0722B`. Alpha is preserved.
pub fn grayscale(buf: &mut [u8]) {
    assert_eq!(buf.len() % 4, 0);

    #[cfg(target_arch = "x86_64")]
    {
        unsafe { return grayscale_sse2(buf) }
    }
    #[cfg(not(target_arch = "x86_64"))]
    grayscale_scalar(buf);
}

fn grayscale_scalar(buf: &mut [u8]) {
    let pixels = buf.len() / 4;
    for i in 0..pixels {
        let off = i * 4;
        let b = buf[off] as f32;
        let g = buf[off + 1] as f32;
        let r = buf[off + 2] as f32;
        let lum = (0.0722 * b + 0.7152 * g + 0.2126 * r + 0.5) as u8;
        buf[off] = lum;
        buf[off + 1] = lum;
        buf[off + 2] = lum;
    }
}

/// Apply sepia tone to a BGRA8 scanline in-place.
pub fn sepia(buf: &mut [u8]) {
    #[rustfmt::skip]
    let matrix = [
        // R row              G row              B row
        0.272, 0.534, 0.131, 0.0, 0.0,  // out_B = 0.272R + 0.534G + 0.131B
        0.349, 0.686, 0.168, 0.0, 0.0,  // out_G = 0.349R + 0.686G + 0.168B
        0.393, 0.769, 0.189, 0.0, 0.0,  // out_R = 0.393R + 0.769G + 0.189B
        0.0,   0.0,   0.0,   1.0, 0.0,  // out_A = A
    ];
    color_matrix(buf, &matrix);
}

/// Apply saturation adjustment to a BGRA8 scanline.
///
/// `factor = 0.0` is grayscale, `1.0` is identity, `> 1.0` is oversaturated.
pub fn saturate(buf: &mut [u8], factor: f32) {
    assert_eq!(buf.len() % 4, 0);

    let pixels = buf.len() / 4;
    for i in 0..pixels {
        let off = i * 4;
        let b = buf[off] as f32;
        let g = buf[off + 1] as f32;
        let r = buf[off + 2] as f32;
        let lum = 0.0722 * b + 0.7152 * g + 0.2126 * r;
        buf[off] = clamp_u8(lum + (b - lum) * factor);
        buf[off + 1] = clamp_u8(lum + (g - lum) * factor);
        buf[off + 2] = clamp_u8(lum + (r - lum) * factor);
    }
}

/// Apply a 5×4 color matrix to a BGRA8 scanline in-place.
///
/// Matrix layout (row-major, BGRA channel order for input):
/// ```text
/// out_B = m[0]*B + m[1]*G + m[2]*R + m[3]*A + m[4]*255
/// out_G = m[5]*B + m[6]*G + m[7]*R + m[8]*A + m[9]*255
/// out_R = m[10]*B + m[11]*G + m[12]*R + m[13]*A + m[14]*255
/// out_A = m[15]*B + m[16]*G + m[17]*R + m[18]*A + m[19]*255
/// ```
pub fn color_matrix(buf: &mut [u8], m: &[f32; 20]) {
    assert_eq!(buf.len() % 4, 0);

    #[cfg(target_arch = "x86_64")]
    {
        unsafe { return color_matrix_sse2(buf, m) }
    }
    #[cfg(not(target_arch = "x86_64"))]
    color_matrix_scalar(buf, m);
}

fn color_matrix_scalar(buf: &mut [u8], m: &[f32; 20]) {
    let pixels = buf.len() / 4;
    for i in 0..pixels {
        let off = i * 4;
        let b = buf[off] as f32;
        let g = buf[off + 1] as f32;
        let r = buf[off + 2] as f32;
        let a = buf[off + 3] as f32;

        buf[off] = clamp_u8(m[0] * b + m[1] * g + m[2] * r + m[3] * a + m[4] * 255.0);
        buf[off + 1] = clamp_u8(m[5] * b + m[6] * g + m[7] * r + m[8] * a + m[9] * 255.0);
        buf[off + 2] = clamp_u8(m[10] * b + m[11] * g + m[12] * r + m[13] * a + m[14] * 255.0);
        buf[off + 3] = clamp_u8(m[15] * b + m[16] * g + m[17] * r + m[18] * a + m[19] * 255.0);
    }
}

// ── SSE2 implementations ─────────────────────────────────────────────

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn brightness_sse2(buf: &mut [u8], factor: f32) {
    use std::arch::x86_64::*;

    let zero = _mm_setzero_si128();
    let factor_v = _mm_set1_ps(factor);
    let max_f = _mm_set1_ps(255.0);
    let zero_f = _mm_setzero_ps();
    let half_f = _mm_set1_ps(0.5);

    let pixels = buf.len() / 4;
    for i in 0..pixels {
        let off = i * 4;
        let pixel = _mm_cvtsi32_si128(i32::from_le_bytes([
            buf[off], buf[off + 1], buf[off + 2], buf[off + 3],
        ]));
        let pixel_16 = _mm_unpacklo_epi8(pixel, zero);
        let pixel_32 = _mm_unpacklo_epi16(pixel_16, zero);
        let mut pixel_f = _mm_cvtepi32_ps(pixel_32);

        // Multiply B, G, R by factor. Preserve A.
        pixel_f = _mm_mul_ps(pixel_f, factor_v);
        // Restore alpha: replace lane 3 with original
        let alpha = _mm_cvtsi32_si128(buf[off + 3] as i32);
        let alpha_f = _mm_cvtepi32_ps(alpha);
        // Shuffle: take [B*f, G*f, R*f] from pixel_f and [A] from alpha
        // Use a mask to blend: pixel_f for lanes 0-2, alpha for lane 3
        let alpha_broadcast = _mm_shuffle_ps::<0x00>(alpha_f, alpha_f);
        // Unpack: pixel_f = [B*f, G*f, R*f, A*f]. We want lane 3 = A original.
        // Simplest: clamp then fix alpha after pack
        pixel_f = _mm_add_ps(pixel_f, half_f);
        pixel_f = _mm_max_ps(_mm_min_ps(pixel_f, max_f), zero_f);
        let int = _mm_cvttps_epi32(pixel_f);
        let packed_16 = _mm_packs_epi32(int, int);
        let packed_8 = _mm_packus_epi16(packed_16, packed_16);
        let val = _mm_cvtsi128_si32(packed_8) as u32;
        let bytes = val.to_le_bytes();
        buf[off] = bytes[0];
        buf[off + 1] = bytes[1];
        buf[off + 2] = bytes[2];
        // Preserve original alpha
        let _ = alpha_broadcast;
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn contrast_sse2(buf: &mut [u8], factor: f32) {
    use std::arch::x86_64::*;

    let zero = _mm_setzero_si128();
    let factor_v = _mm_set1_ps(factor);
    let mid = _mm_set1_ps(128.0);
    let max_f = _mm_set1_ps(255.0);
    let zero_f = _mm_setzero_ps();
    let half_f = _mm_set1_ps(0.5);

    let pixels = buf.len() / 4;
    for i in 0..pixels {
        let off = i * 4;
        let alpha = buf[off + 3];
        let pixel = _mm_cvtsi32_si128(i32::from_le_bytes([
            buf[off], buf[off + 1], buf[off + 2], buf[off + 3],
        ]));
        let pixel_16 = _mm_unpacklo_epi8(pixel, zero);
        let pixel_32 = _mm_unpacklo_epi16(pixel_16, zero);
        let pixel_f = _mm_cvtepi32_ps(pixel_32);

        // (pixel - 128) * factor + 128
        let centered = _mm_sub_ps(pixel_f, mid);
        let scaled = _mm_add_ps(_mm_mul_ps(centered, factor_v), mid);
        let clamped = _mm_add_ps(scaled, half_f);
        let clamped = _mm_max_ps(_mm_min_ps(clamped, max_f), zero_f);

        let int = _mm_cvttps_epi32(clamped);
        let packed_16 = _mm_packs_epi32(int, int);
        let packed_8 = _mm_packus_epi16(packed_16, packed_16);
        let val = (_mm_cvtsi128_si32(packed_8) as u32).to_le_bytes();
        buf[off] = val[0];
        buf[off + 1] = val[1];
        buf[off + 2] = val[2];
        buf[off + 3] = alpha; // preserve
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn grayscale_sse2(buf: &mut [u8]) {
    use std::arch::x86_64::*;

    let zero = _mm_setzero_si128();
    // BT.709 weights: B=0.0722, G=0.7152, R=0.2126
    let w_b = _mm_set1_ps(0.0722);
    let w_g = _mm_set1_ps(0.7152);
    let w_r = _mm_set1_ps(0.2126);
    let half_f = _mm_set1_ps(0.5);

    let pixels = buf.len() / 4;
    for i in 0..pixels {
        let off = i * 4;
        let alpha = buf[off + 3];
        let pixel = _mm_cvtsi32_si128(i32::from_le_bytes([
            buf[off], buf[off + 1], buf[off + 2], buf[off + 3],
        ]));
        let pixel_16 = _mm_unpacklo_epi8(pixel, zero);
        let pixel_32 = _mm_unpacklo_epi16(pixel_16, zero);
        let pixel_f = _mm_cvtepi32_ps(pixel_32);

        // Extract B, G, R as scalars (shuffle to lane 0)
        let b_f = pixel_f; // lane 0 = B
        let g_f = _mm_shuffle_ps::<0x55>(pixel_f, pixel_f); // lane 0 = G
        let r_f = _mm_shuffle_ps::<0xAA>(pixel_f, pixel_f); // lane 0 = R

        // lum = w_b*B + w_g*G + w_r*R
        let lum = _mm_add_ps(
            _mm_add_ps(_mm_mul_ps(w_b, b_f), _mm_mul_ps(w_g, g_f)),
            _mm_mul_ps(w_r, r_f),
        );
        let lum = _mm_add_ss(lum, half_f);
        let lum_i = _mm_cvttss_si32(lum) as u8;

        buf[off] = lum_i;
        buf[off + 1] = lum_i;
        buf[off + 2] = lum_i;
        buf[off + 3] = alpha;
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn color_matrix_sse2(buf: &mut [u8], m: &[f32; 20]) {
    use std::arch::x86_64::*;

    let zero = _mm_setzero_si128();
    let max_f = _mm_set1_ps(255.0);
    let zero_f = _mm_setzero_ps();
    let half_f = _mm_set1_ps(0.5);

    // Load matrix rows
    let row0 = _mm_loadu_ps(m.as_ptr()); // [m0, m1, m2, m3]
    let off0 = _mm_set1_ps(m[4] * 255.0);
    let row1 = _mm_loadu_ps(m.as_ptr().add(5));
    let off1 = _mm_set1_ps(m[9] * 255.0);
    let row2 = _mm_loadu_ps(m.as_ptr().add(10));
    let off2 = _mm_set1_ps(m[14] * 255.0);
    let row3 = _mm_loadu_ps(m.as_ptr().add(15));
    let off3 = _mm_set1_ps(m[19] * 255.0);

    let pixels = buf.len() / 4;
    for i in 0..pixels {
        let off = i * 4;
        let pixel = _mm_cvtsi32_si128(i32::from_le_bytes([
            buf[off], buf[off + 1], buf[off + 2], buf[off + 3],
        ]));
        let pixel_16 = _mm_unpacklo_epi8(pixel, zero);
        let pixel_32 = _mm_unpacklo_epi16(pixel_16, zero);
        let pixel_f = _mm_cvtepi32_ps(pixel_32); // [B, G, R, A]

        // Dot product: row * pixel + offset
        // Using horizontal adds would be ideal but SSE3+. Use shuffles instead.
        let dot = |row: __m128, offset: __m128| -> f32 {
            let mul = _mm_mul_ps(row, pixel_f);
            // Horizontal sum: mul[0] + mul[1] + mul[2] + mul[3]
            let shuf1 = _mm_shuffle_ps::<0x4E>(mul, mul); // [2,3,0,1]
            let sum1 = _mm_add_ps(mul, shuf1); // [0+2, 1+3, ...]
            let shuf2 = _mm_shuffle_ps::<0xB1>(sum1, sum1); // [1+3, 0+2, ...]
            let sum2 = _mm_add_ss(sum1, shuf2); // [0+1+2+3, ...]
            let result = _mm_add_ss(sum2, offset);
            _mm_cvtss_f32(result)
        };

        let out_b = dot(row0, off0);
        let out_g = dot(row1, off1);
        let out_r = dot(row2, off2);
        let out_a = dot(row3, off3);

        let result = _mm_set_ps(out_a, out_r, out_g, out_b);
        let result = _mm_add_ps(result, half_f);
        let result = _mm_max_ps(_mm_min_ps(result, max_f), zero_f);
        let int = _mm_cvttps_epi32(result);
        let packed_16 = _mm_packs_epi32(int, int);
        let packed_8 = _mm_packus_epi16(packed_16, packed_16);
        let val = (_mm_cvtsi128_si32(packed_8) as u32).to_le_bytes();
        buf[off..off + 4].copy_from_slice(&val);
    }
}

#[inline]
fn clamp_u8(v: f32) -> u8 {
    (v + 0.5).clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brightness_identity() {
        let original = vec![100, 150, 200, 255];
        let mut buf = original.clone();
        brightness(&mut buf, 1.0);
        // Allow ±1 rounding
        for (i, (&o, &b)) in original.iter().zip(buf.iter()).enumerate() {
            if i < 3 {
                assert!((o as i16 - b as i16).abs() <= 1);
            }
        }
    }

    #[test]
    fn brightness_double() {
        let mut buf = vec![50, 50, 50, 255];
        brightness(&mut buf, 2.0);
        assert_eq!(buf[0], 100);
        assert_eq!(buf[1], 100);
        assert_eq!(buf[2], 100);
        assert_eq!(buf[3], 255); // alpha preserved
    }

    #[test]
    fn brightness_clamps() {
        let mut buf = vec![200, 200, 200, 255];
        brightness(&mut buf, 2.0);
        assert_eq!(buf[0], 255);
        assert_eq!(buf[1], 255);
        assert_eq!(buf[2], 255);
    }

    #[test]
    fn contrast_zero_produces_gray() {
        let mut buf = vec![200, 100, 50, 255];
        contrast(&mut buf, 0.0);
        assert_eq!(buf[0], 128);
        assert_eq!(buf[1], 128);
        assert_eq!(buf[2], 128);
        assert_eq!(buf[3], 255);
    }

    #[test]
    fn grayscale_red() {
        let mut buf = vec![0, 0, 255, 255]; // BGRA: pure red
        grayscale(&mut buf);
        // BT.709: 0.2126 * 255 ≈ 54
        let lum = buf[0];
        assert!((lum as i16 - 54).abs() <= 1, "got {lum}");
        assert_eq!(buf[0], buf[1]);
        assert_eq!(buf[1], buf[2]);
        assert_eq!(buf[3], 255);
    }

    #[test]
    fn sepia_applies() {
        let mut buf = vec![128, 128, 128, 255]; // mid-gray
        sepia(&mut buf);
        // Sepia of gray: R > G > B
        assert!(buf[2] > buf[1], "R({}) should > G({})", buf[2], buf[1]);
        assert!(buf[1] > buf[0], "G({}) should > B({})", buf[1], buf[0]);
    }

    #[test]
    fn saturate_zero_is_grayscale() {
        let mut buf_sat = vec![0, 0, 255, 255]; // red
        let mut buf_gray = buf_sat.clone();
        saturate(&mut buf_sat, 0.0);
        grayscale(&mut buf_gray);
        for i in 0..3 {
            assert!((buf_sat[i] as i16 - buf_gray[i] as i16).abs() <= 1);
        }
    }

    #[test]
    fn color_matrix_identity() {
        #[rustfmt::skip]
        let identity = [
            1.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 1.0, 0.0,
        ];
        let original = vec![100, 150, 200, 128];
        let mut buf = original.clone();
        color_matrix(&mut buf, &identity);
        for i in 0..4 {
            assert!((original[i] as i16 - buf[i] as i16).abs() <= 1);
        }
    }

    #[test]
    fn multi_pixel_consistency() {
        // Test with multiple pixels to exercise any vectorized paths
        let pixel_count = 37;
        let mut buf: Vec<u8> = (0..pixel_count * 4).map(|i| (i % 256) as u8).collect();
        let mut buf_scalar = buf.clone();

        brightness(&mut buf, 1.5);
        brightness_scalar(&mut buf_scalar, 1.5);

        for i in 0..buf.len() {
            let diff = (buf[i] as i16 - buf_scalar[i] as i16).abs();
            assert!(diff <= 1, "byte {i}: simd={} scalar={}", buf[i], buf_scalar[i]);
        }
    }
}
