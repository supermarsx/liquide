//! Premultiplied alpha blending operations (Porter-Duff compositing).

use liquide_compositor::pixel::{BlendMode, Color};

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

/// Blend using Multiply mode.
///
/// Formula: `out = src * dst` (per channel, normalised).
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

/// Blend using Screen mode.
///
/// Formula: `out = src + dst - src * dst` (per channel, normalised).
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

/// Dispatch to the appropriate blend function based on mode.
#[inline]
#[must_use]
pub fn blend(dst: Color, src: Color, mode: BlendMode) -> Color {
    match mode {
        BlendMode::SrcOver => blend_src_over(dst, src),
        BlendMode::Src => blend_src(src),
        BlendMode::Multiply => blend_multiply(dst, src),
        BlendMode::Screen => blend_screen(dst, src),
        BlendMode::SrcAtop => blend_src_atop(dst, src),
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
