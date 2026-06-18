//! 8-bit planar YUV → RGBA8 conversion.
//!
//! `register_image_rgba` expects tightly-packed **RGBA8**, so the decoder's
//! planar YUV output (rav1d hands back I420 / I422 / I444 8-bit planes) is
//! converted here. We use the BT.601 **limited-range** ("studio swing") matrix,
//! the default colour space for AV1 streams that do not signal otherwise (the
//! common case for short clips). Full-range and BT.709 are follow-ups; the
//! conversion is centralised here so a colour-space upgrade is one place.

/// The chroma subsampling of a decoded planar frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelLayout {
    /// Monochrome (no chroma planes) — chroma treated as neutral (128).
    I400,
    /// 4:2:0 — chroma planes are half width and half height.
    I420,
    /// 4:2:2 — chroma planes are half width, full height.
    I422,
    /// 4:4:4 — chroma planes are full resolution.
    I444,
}

impl PixelLayout {
    /// Horizontal subsampling shift (chroma_x = luma_x >> hshift).
    #[must_use]
    pub fn hshift(self) -> u32 {
        match self {
            PixelLayout::I400 => 0,
            PixelLayout::I420 | PixelLayout::I422 => 1,
            PixelLayout::I444 => 0,
        }
    }

    /// Vertical subsampling shift (chroma_y = luma_y >> vshift).
    #[must_use]
    pub fn vshift(self) -> u32 {
        match self {
            PixelLayout::I400 | PixelLayout::I422 | PixelLayout::I444 => 0,
            PixelLayout::I420 => 1,
        }
    }

    /// Whether this layout carries chroma planes at all.
    #[must_use]
    pub fn has_chroma(self) -> bool {
        !matches!(self, PixelLayout::I400)
    }
}

/// Convert one (Y, U, V) triple to an (R, G, B) triple using the BT.601
/// limited-range matrix. `y`, `u`, `v` are the raw 8-bit plane samples.
#[inline]
#[must_use]
pub fn yuv_to_rgb(y: u8, u: u8, v: u8) -> (u8, u8, u8) {
    // BT.601 limited range: Y in [16,235], C in [16,240], centred at 128.
    let yf = (y as f32 - 16.0) * 1.164_383_5;
    let uf = u as f32 - 128.0;
    let vf = v as f32 - 128.0;
    let r = yf + 1.596_027 * vf;
    let g = yf - 0.391_762 * uf - 0.812_968 * vf;
    let b = yf + 2.017_232 * uf;
    (
        clamp_u8(r),
        clamp_u8(g),
        clamp_u8(b),
    )
}

#[inline]
fn clamp_u8(v: f32) -> u8 {
    if v <= 0.0 {
        0
    } else if v >= 255.0 {
        255
    } else {
        (v + 0.5) as u8
    }
}

/// Planar 8-bit YUV planes (with their byte strides) for one frame.
pub struct YuvPlanes<'a> {
    /// Luma plane.
    pub y: &'a [u8],
    /// U (Cb) chroma plane (empty for [`PixelLayout::I400`]).
    pub u: &'a [u8],
    /// V (Cr) chroma plane (empty for [`PixelLayout::I400`]).
    pub v: &'a [u8],
    /// Luma plane stride in bytes.
    pub y_stride: usize,
    /// Chroma plane stride in bytes (both U and V share it).
    pub uv_stride: usize,
    /// Visible width in pixels.
    pub width: u32,
    /// Visible height in pixels.
    pub height: u32,
    /// Chroma subsampling.
    pub layout: PixelLayout,
}

/// Convert planar 8-bit YUV to a tightly-packed RGBA8 buffer
/// (`width * height * 4` bytes, alpha = 255, top row first).
///
/// Out-of-range plane indices are clamped (a malformed stride/plane can never
/// read out of bounds); a missing sample falls back to neutral so the output is
/// always a complete, correctly-sized RGBA buffer.
#[must_use]
pub fn yuv_to_rgba(planes: &YuvPlanes<'_>) -> Vec<u8> {
    let w = planes.width as usize;
    let h = planes.height as usize;
    let mut out = vec![0u8; w * h * 4];
    let hshift = planes.layout.hshift();
    let vshift = planes.layout.vshift();
    let has_chroma = planes.layout.has_chroma();

    for row in 0..h {
        let y_row = row * planes.y_stride;
        let c_row = (row >> vshift) * planes.uv_stride;
        for col in 0..w {
            let y = *planes.y.get(y_row + col).unwrap_or(&16);
            let (u, v) = if has_chroma {
                let ci = c_row + (col >> hshift);
                (
                    *planes.u.get(ci).unwrap_or(&128),
                    *planes.v.get(ci).unwrap_or(&128),
                )
            } else {
                (128, 128)
            };
            let (r, g, b) = yuv_to_rgb(y, u, v);
            let o = (row * w + col) * 4;
            out[o] = r;
            out[o + 1] = g;
            out[o + 2] = b;
            out[o + 3] = 255;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_gray_round_trips_to_gray() {
        // Y=126 (mid), neutral chroma → near-gray RGB; alpha 255.
        let (r, g, b) = yuv_to_rgb(126, 128, 128);
        // R, G, B should be close to each other (gray).
        assert!((r as i32 - g as i32).abs() <= 2, "r={r} g={g}");
        assert!((g as i32 - b as i32).abs() <= 2, "g={g} b={b}");
    }

    #[test]
    fn black_and_white_extremes() {
        let (r, g, b) = yuv_to_rgb(16, 128, 128);
        assert_eq!((r, g, b), (0, 0, 0), "limited-range black");
        let (r, g, b) = yuv_to_rgb(235, 128, 128);
        assert!(r >= 254 && g >= 254 && b >= 254, "limited-range white: {r},{g},{b}");
    }

    #[test]
    fn i420_buffer_is_full_size_and_opaque() {
        // 4x4 I420: luma 4x4, chroma 2x2.
        let y = vec![120u8; 16];
        let u = vec![84u8; 4];
        let v = vec![200u8; 4];
        let planes = YuvPlanes {
            y: &y,
            u: &u,
            v: &v,
            y_stride: 4,
            uv_stride: 2,
            width: 4,
            height: 4,
            layout: PixelLayout::I420,
        };
        let rgba = yuv_to_rgba(&planes);
        assert_eq!(rgba.len(), 4 * 4 * 4);
        // Every pixel opaque.
        for px in rgba.chunks_exact(4) {
            assert_eq!(px[3], 255);
        }
        // All pixels identical (solid color), matching yuv_to_rgb of the fill.
        let (er, eg, eb) = yuv_to_rgb(120, 84, 200);
        for px in rgba.chunks_exact(4) {
            assert_eq!((px[0], px[1], px[2]), (er, eg, eb));
        }
    }

    #[test]
    fn stride_padding_is_respected() {
        // 2x2 luma with stride 4 (2 bytes of row padding). The padding bytes
        // must NOT bleed into the output.
        let y = vec![235, 235, 99, 99, 235, 235, 99, 99]; // rows: [235,235|pad], [235,235|pad]
        let u = vec![128, 0];
        let v = vec![128, 0];
        let planes = YuvPlanes {
            y: &y,
            u: &u,
            v: &v,
            y_stride: 4,
            uv_stride: 2,
            width: 2,
            height: 2,
            layout: PixelLayout::I420,
        };
        let rgba = yuv_to_rgba(&planes);
        // All four visible pixels come from Y=235 (white), not the 99 padding.
        for px in rgba.chunks_exact(4) {
            assert!(px[0] >= 254 && px[1] >= 254 && px[2] >= 254, "got {px:?}");
        }
    }
}
