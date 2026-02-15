//! Premultiplied alpha blending operations (Porter-Duff + CSS blend modes).
//!
//! Implements all 16 CSS Compositing and Blending Level 1 blend modes plus the
//! core Porter-Duff operators (`SrcOver`, `Src`, `SrcAtop`).

use liquide_compositor::pixel::{BlendMode, Color};

// ============================================================================
// Porter-Duff operators
// ============================================================================

/// Blend src over dst using premultiplied alpha Porter-Duff SrcOver.
///
/// Formula: `out = src + dst * (1 - src.a)`
#[inline]
#[must_use]
pub fn blend_src_over(dst: Color, src: Color) -> Color {
    if src.a == 255 {
        return src;
    }
    if src.a == 0 {
        return dst;
    }
    let inv_a = 255 - src.a as u16;
    Color {
        r: (src.r as u16 + (dst.r as u16 * inv_a + 127) / 255) as u8,
        g: (src.g as u16 + (dst.g as u16 * inv_a + 127) / 255) as u8,
        b: (src.b as u16 + (dst.b as u16 * inv_a + 127) / 255) as u8,
        a: (src.a as u16 + (dst.a as u16 * inv_a + 127) / 255) as u8,
    }
}

/// Blend using Src mode (replace destination entirely).
#[inline]
#[must_use]
pub fn blend_src(src: Color) -> Color {
    src
}

/// Blend using SrcAtop mode.
///
/// Formula: `out = src * dst.a + dst * (1 - src.a)`
#[inline]
#[must_use]
pub fn blend_src_atop(dst: Color, src: Color) -> Color {
    let src_a = src.a as u16;
    let dst_a = dst.a as u16;
    let inv_src_a = 255 - src_a;
    Color {
        r: ((src.r as u16 * dst_a + dst.r as u16 * inv_src_a + 127) / 255) as u8,
        g: ((src.g as u16 * dst_a + dst.g as u16 * inv_src_a + 127) / 255) as u8,
        b: ((src.b as u16 * dst_a + dst.b as u16 * inv_src_a + 127) / 255) as u8,
        a: dst.a,
    }
}

// ============================================================================
// CSS separable blend modes (per-channel)
// ============================================================================

/// Multiply: `out = src * dst`
#[inline]
#[must_use]
pub fn blend_multiply(dst: Color, src: Color) -> Color {
    Color {
        r: ((dst.r as u16 * src.r as u16 + 127) / 255) as u8,
        g: ((dst.g as u16 * src.g as u16 + 127) / 255) as u8,
        b: ((dst.b as u16 * src.b as u16 + 127) / 255) as u8,
        a: ((dst.a as u16 * src.a as u16 + 127) / 255) as u8,
    }
}

/// Screen: `out = src + dst - src * dst`
#[inline]
#[must_use]
pub fn blend_screen(dst: Color, src: Color) -> Color {
    Color {
        r: (src.r as u16 + dst.r as u16 - (dst.r as u16 * src.r as u16 + 127) / 255) as u8,
        g: (src.g as u16 + dst.g as u16 - (dst.g as u16 * src.g as u16 + 127) / 255) as u8,
        b: (src.b as u16 + dst.b as u16 - (dst.b as u16 * src.b as u16 + 127) / 255) as u8,
        a: (src.a as u16 + dst.a as u16 - (dst.a as u16 * src.a as u16 + 127) / 255) as u8,
    }
}

/// Overlay: Multiply if dst < 0.5, Screen if dst >= 0.5.
#[inline]
#[must_use]
pub fn blend_overlay(dst: Color, src: Color) -> Color {
    #[inline]
    fn overlay_ch(d: u8, s: u8) -> u8 {
        if d < 128 {
            ((2 * d as u16 * s as u16 + 127) / 255) as u8
        } else {
            (255 - ((2 * (255 - d as u16) * (255 - s as u16) + 127) / 255)) as u8
        }
    }
    Color {
        r: overlay_ch(dst.r, src.r),
        g: overlay_ch(dst.g, src.g),
        b: overlay_ch(dst.b, src.b),
        a: ((src.a as u16 + dst.a as u16 - (dst.a as u16 * src.a as u16 + 127) / 255)) as u8,
    }
}

/// Darken: `out = min(src, dst)` per channel.
#[inline]
#[must_use]
pub fn blend_darken(dst: Color, src: Color) -> Color {
    Color {
        r: dst.r.min(src.r),
        g: dst.g.min(src.g),
        b: dst.b.min(src.b),
        a: dst.a.max(src.a),
    }
}

/// Lighten: `out = max(src, dst)` per channel.
#[inline]
#[must_use]
pub fn blend_lighten(dst: Color, src: Color) -> Color {
    Color {
        r: dst.r.max(src.r),
        g: dst.g.max(src.g),
        b: dst.b.max(src.b),
        a: dst.a.max(src.a),
    }
}

/// Color Dodge: brightens dst to reflect src.
#[inline]
#[must_use]
pub fn blend_color_dodge(dst: Color, src: Color) -> Color {
    #[inline]
    fn dodge_ch(d: u8, s: u8) -> u8 {
        if d == 0 {
            0
        } else if s == 255 {
            255
        } else {
            ((d as u32 * 255 / (255 - s as u32)).min(255)) as u8
        }
    }
    Color {
        r: dodge_ch(dst.r, src.r),
        g: dodge_ch(dst.g, src.g),
        b: dodge_ch(dst.b, src.b),
        a: dst.a.max(src.a),
    }
}

/// Color Burn: darkens dst to reflect src.
#[inline]
#[must_use]
pub fn blend_color_burn(dst: Color, src: Color) -> Color {
    #[inline]
    fn burn_ch(d: u8, s: u8) -> u8 {
        if d == 255 {
            255
        } else if s == 0 {
            0
        } else {
            255 - (((255 - d as u32) * 255 / s as u32).min(255)) as u8
        }
    }
    Color {
        r: burn_ch(dst.r, src.r),
        g: burn_ch(dst.g, src.g),
        b: burn_ch(dst.b, src.b),
        a: dst.a.max(src.a),
    }
}

/// Hard Light: Multiply if src < 0.5, Screen if src >= 0.5 (overlay with swapped roles).
#[inline]
#[must_use]
pub fn blend_hard_light(dst: Color, src: Color) -> Color {
    // hard-light(A,B) = overlay(B,A)
    blend_overlay(src, dst)
}

/// Soft Light: W3C formula (gentler overlay).
#[inline]
#[must_use]
pub fn blend_soft_light(dst: Color, src: Color) -> Color {
    #[inline]
    fn soft_ch(d: u8, s: u8) -> u8 {
        let df = d as f32 / 255.0;
        let sf = s as f32 / 255.0;
        let result = if sf <= 0.5 {
            df - (1.0 - 2.0 * sf) * df * (1.0 - df)
        } else {
            let g = if df <= 0.25 {
                ((16.0 * df - 12.0) * df + 4.0) * df
            } else {
                df.sqrt()
            };
            df + (2.0 * sf - 1.0) * (g - df)
        };
        (result.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
    }
    Color {
        r: soft_ch(dst.r, src.r),
        g: soft_ch(dst.g, src.g),
        b: soft_ch(dst.b, src.b),
        a: dst.a.max(src.a),
    }
}

/// Difference: `out = |src - dst|` per channel.
#[inline]
#[must_use]
pub fn blend_difference(dst: Color, src: Color) -> Color {
    Color {
        r: (dst.r as i16 - src.r as i16).unsigned_abs() as u8,
        g: (dst.g as i16 - src.g as i16).unsigned_abs() as u8,
        b: (dst.b as i16 - src.b as i16).unsigned_abs() as u8,
        a: dst.a.max(src.a),
    }
}

/// Exclusion: lower-contrast version of difference.
///
/// Formula: `out = src + dst - 2 * src * dst`
#[inline]
#[must_use]
pub fn blend_exclusion(dst: Color, src: Color) -> Color {
    #[inline]
    fn excl_ch(d: u8, s: u8) -> u8 {
        (s as u16 + d as u16 - 2 * (s as u16 * d as u16 + 127) / 255) as u8
    }
    Color {
        r: excl_ch(dst.r, src.r),
        g: excl_ch(dst.g, src.g),
        b: excl_ch(dst.b, src.b),
        a: dst.a.max(src.a),
    }
}

// ============================================================================
// CSS non-separable blend modes (HSL-based)
// ============================================================================

/// sRGB luminance Y = 0.299R + 0.587G + 0.114B
#[inline]
fn luminance(r: u8, g: u8, b: u8) -> f32 {
    0.299 * r as f32 / 255.0 + 0.587 * g as f32 / 255.0 + 0.114 * b as f32 / 255.0
}

/// Saturation = max(R,G,B) - min(R,G,B)
#[inline]
fn saturation(r: u8, g: u8, b: u8) -> f32 {
    let max = r.max(g).max(b) as f32;
    let min = r.min(g).min(b) as f32;
    (max - min) / 255.0
}

/// Set luminance of an RGB triplet, clipping to [0,1].
#[inline]
fn set_lum(r: f32, g: f32, b: f32, lum: f32) -> (f32, f32, f32) {
    let cur = 0.299 * r + 0.587 * g + 0.114 * b;
    let d = lum - cur;
    let (mut r, mut g, mut b) = (r + d, g + d, b + d);
    let min = r.min(g).min(b);
    let max = r.max(g).max(b);
    if min < 0.0 {
        let l = lum;
        r = l + (r - l) * l / (l - min);
        g = l + (g - l) * l / (l - min);
        b = l + (b - l) * l / (l - min);
    }
    if max > 1.0 {
        let l = lum;
        r = l + (r - l) * (1.0 - l) / (max - l);
        g = l + (g - l) * (1.0 - l) / (max - l);
        b = l + (b - l) * (1.0 - l) / (max - l);
    }
    (r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0))
}

/// Set saturation of an RGB triplet, preserving channel order.
#[inline]
fn set_sat(r: f32, g: f32, b: f32, sat: f32) -> (f32, f32, f32) {
    // Sort channels: min, mid, max
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    if max == min {
        return (0.0, 0.0, 0.0);
    }
    let scale = sat / (max - min);
    ((r - min) * scale, (g - min) * scale, (b - min) * scale)
}

/// Hue blend: hue from src, saturation+luminosity from dst.
#[inline]
#[must_use]
pub fn blend_hue(dst: Color, src: Color) -> Color {
    let (sr, sg, sb) = (src.r as f32 / 255.0, src.g as f32 / 255.0, src.b as f32 / 255.0);
    let (_dr, _dg, _db) = (dst.r as f32 / 255.0, dst.g as f32 / 255.0, dst.b as f32 / 255.0);
    let s = saturation(dst.r, dst.g, dst.b);
    let l = luminance(dst.r, dst.g, dst.b);
    let (r, g, b) = set_sat(sr, sg, sb, s);
    let (r, g, b) = set_lum(r, g, b, l);
    Color {
        r: (r * 255.0 + 0.5) as u8,
        g: (g * 255.0 + 0.5) as u8,
        b: (b * 255.0 + 0.5) as u8,
        a: dst.a.max(src.a),
    }
}

/// Saturation blend: saturation from src, hue+luminosity from dst.
#[inline]
#[must_use]
pub fn blend_saturation(dst: Color, src: Color) -> Color {
    let (dr, dg, db) = (dst.r as f32 / 255.0, dst.g as f32 / 255.0, dst.b as f32 / 255.0);
    let s = saturation(src.r, src.g, src.b);
    let l = luminance(dst.r, dst.g, dst.b);
    let (r, g, b) = set_sat(dr, dg, db, s);
    let (r, g, b) = set_lum(r, g, b, l);
    Color {
        r: (r * 255.0 + 0.5) as u8,
        g: (g * 255.0 + 0.5) as u8,
        b: (b * 255.0 + 0.5) as u8,
        a: dst.a.max(src.a),
    }
}

/// Color blend: hue+saturation from src, luminosity from dst.
#[inline]
#[must_use]
pub fn blend_color(dst: Color, src: Color) -> Color {
    let (sr, sg, sb) = (src.r as f32 / 255.0, src.g as f32 / 255.0, src.b as f32 / 255.0);
    let l = luminance(dst.r, dst.g, dst.b);
    let (r, g, b) = set_lum(sr, sg, sb, l);
    Color {
        r: (r * 255.0 + 0.5) as u8,
        g: (g * 255.0 + 0.5) as u8,
        b: (b * 255.0 + 0.5) as u8,
        a: dst.a.max(src.a),
    }
}

/// Luminosity blend: luminosity from src, hue+saturation from dst.
#[inline]
#[must_use]
pub fn blend_luminosity(dst: Color, src: Color) -> Color {
    let (dr, dg, db) = (dst.r as f32 / 255.0, dst.g as f32 / 255.0, dst.b as f32 / 255.0);
    let l = luminance(src.r, src.g, src.b);
    let (r, g, b) = set_lum(dr, dg, db, l);
    Color {
        r: (r * 255.0 + 0.5) as u8,
        g: (g * 255.0 + 0.5) as u8,
        b: (b * 255.0 + 0.5) as u8,
        a: dst.a.max(src.a),
    }
}

// ============================================================================
// Dispatch
// ============================================================================

/// Dispatch to the appropriate blend function based on mode.
#[inline]
#[must_use]
pub fn blend(dst: Color, src: Color, mode: BlendMode) -> Color {
    match mode {
        BlendMode::SrcOver => blend_src_over(dst, src),
        BlendMode::Src => blend_src(src),
        BlendMode::SrcAtop => blend_src_atop(dst, src),
        BlendMode::Multiply => blend_multiply(dst, src),
        BlendMode::Screen => blend_screen(dst, src),
        BlendMode::Overlay => blend_overlay(dst, src),
        BlendMode::Darken => blend_darken(dst, src),
        BlendMode::Lighten => blend_lighten(dst, src),
        BlendMode::ColorDodge => blend_color_dodge(dst, src),
        BlendMode::ColorBurn => blend_color_burn(dst, src),
        BlendMode::HardLight => blend_hard_light(dst, src),
        BlendMode::SoftLight => blend_soft_light(dst, src),
        BlendMode::Difference => blend_difference(dst, src),
        BlendMode::Exclusion => blend_exclusion(dst, src),
        BlendMode::Hue => blend_hue(dst, src),
        BlendMode::Saturation => blend_saturation(dst, src),
        BlendMode::ColorBlend => blend_color(dst, src),
        BlendMode::Luminosity => blend_luminosity(dst, src),
    }
}

/// Blend an entire scanline of BGRA pixels.
///
/// `dst` and `src` are slices of `len * 4` bytes in BGRA order.
// TODO: AVX2 8-pixel vectorised blend path
pub fn blend_scanline(dst: &mut [u8], src: &[u8], mode: BlendMode) {
    debug_assert_eq!(dst.len(), src.len());
    debug_assert_eq!(dst.len() % 4, 0);

    let pixel_count = dst.len() / 4;
    for i in 0..pixel_count {
        let off = i * 4;
        let d = Color::from_bgra_bytes([dst[off], dst[off + 1], dst[off + 2], dst[off + 3]]);
        let s = Color::from_bgra_bytes([src[off], src[off + 1], src[off + 2], src[off + 3]]);
        let result = blend(d, s, mode);
        let bgra = result.to_bgra_bytes();
        dst[off..off + 4].copy_from_slice(&bgra);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn src_over_opaque_replaces() {
        let dst = Color::new(100, 100, 100, 255);
        let src = Color::new(200, 50, 50, 255);
        assert_eq!(blend_src_over(dst, src), src);
    }

    #[test]
    fn src_over_transparent_noop() {
        let dst = Color::new(100, 100, 100, 255);
        let src = Color::new(0, 0, 0, 0);
        assert_eq!(blend_src_over(dst, src), dst);
    }

    #[test]
    fn multiply_black_yields_black() {
        let any = Color::new(200, 150, 100, 255);
        let black = Color::new(0, 0, 0, 255);
        let result = blend_multiply(any, black);
        assert_eq!(result.r, 0);
        assert_eq!(result.g, 0);
        assert_eq!(result.b, 0);
    }

    #[test]
    fn screen_white_yields_white() {
        let any = Color::new(100, 100, 100, 255);
        let white = Color::new(255, 255, 255, 255);
        let result = blend_screen(any, white);
        assert_eq!(result.r, 255);
        assert_eq!(result.g, 255);
        assert_eq!(result.b, 255);
    }

    #[test]
    fn overlay_symmetry() {
        // overlay(A,B) at mid-gray should produce distinct results for each half
        let light = Color::new(200, 200, 200, 255);
        let dark = Color::new(50, 50, 50, 255);
        let result = blend_overlay(light, dark);
        // Dark over light → Screen branch (light dst >= 128)
        assert!(result.r > 50);
    }

    #[test]
    fn darken_picks_min() {
        let a = Color::new(100, 200, 50, 255);
        let b = Color::new(150, 100, 150, 255);
        let result = blend_darken(a, b);
        assert_eq!(result.r, 100);
        assert_eq!(result.g, 100);
        assert_eq!(result.b, 50);
    }

    #[test]
    fn lighten_picks_max() {
        let a = Color::new(100, 200, 50, 255);
        let b = Color::new(150, 100, 150, 255);
        let result = blend_lighten(a, b);
        assert_eq!(result.r, 150);
        assert_eq!(result.g, 200);
        assert_eq!(result.b, 150);
    }

    #[test]
    fn difference_abs() {
        let a = Color::new(200, 50, 100, 255);
        let b = Color::new(100, 150, 100, 255);
        let result = blend_difference(a, b);
        assert_eq!(result.r, 100);
        assert_eq!(result.g, 100);
        assert_eq!(result.b, 0);
    }

    #[test]
    fn exclusion_symmetric() {
        let a = Color::new(0, 0, 0, 255);
        let b = Color::new(255, 255, 255, 255);
        let r1 = blend_exclusion(a, b);
        let r2 = blend_exclusion(b, a);
        assert_eq!(r1.r, r2.r);
    }

    #[test]
    fn all_modes_dispatch() {
        let dst = Color::new(128, 128, 128, 255);
        let src = Color::new(64, 192, 64, 200);
        // Ensure every mode produces a result without panic
        for mode in [
            BlendMode::SrcOver,
            BlendMode::Src,
            BlendMode::SrcAtop,
            BlendMode::Multiply,
            BlendMode::Screen,
            BlendMode::Overlay,
            BlendMode::Darken,
            BlendMode::Lighten,
            BlendMode::ColorDodge,
            BlendMode::ColorBurn,
            BlendMode::HardLight,
            BlendMode::SoftLight,
            BlendMode::Difference,
            BlendMode::Exclusion,
            BlendMode::Hue,
            BlendMode::Saturation,
            BlendMode::ColorBlend,
            BlendMode::Luminosity,
        ] {
            let _ = blend(dst, src, mode);
        }
    }
}
