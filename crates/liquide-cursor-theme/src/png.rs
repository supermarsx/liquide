//! PNG decoding for cursor theme images.
//!
//! Cursor themes on Linux/XDG stores their frame images as PNG files under
//! `theme_root/cursors/<shape>` (either plain PNG or an XCursor file which
//! embeds PNG frames). Historically this crate's loader returned
//! all-zero pixel buffers — clearly incorrect; this module decodes the
//! bytes into an RGBA8 buffer sized exactly `width * height * 4`.

use crate::cursor::CursorImage;

/// Errors produced while decoding a PNG cursor image.
#[derive(Debug)]
pub enum PngDecodeError {
    /// Underlying PNG decoder error (malformed stream, unsupported feature).
    Decoder(String),
    /// I/O error reading the file.
    Io(String),
}

impl std::fmt::Display for PngDecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Decoder(s) => write!(f, "png decode error: {s}"),
            Self::Io(s) => write!(f, "png i/o error: {s}"),
        }
    }
}

impl std::error::Error for PngDecodeError {}

impl From<::png::DecodingError> for PngDecodeError {
    fn from(e: ::png::DecodingError) -> Self {
        Self::Decoder(e.to_string())
    }
}

impl From<std::io::Error> for PngDecodeError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

/// Decode a PNG byte buffer into RGBA8 pixels.
///
/// Handles the four most common sample formats that appear in cursor
/// themes — RGBA8, RGB8, GrayscaleAlpha8, Grayscale8 — by expanding them
/// to RGBA8. Other colour types are treated as an error.
pub fn decode_rgba8(bytes: &[u8]) -> Result<(Vec<u8>, u32, u32), PngDecodeError> {
    let decoder = ::png::Decoder::new(bytes);
    let mut reader = decoder.read_info()?;
    let info = reader.info().clone();
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let frame = reader.next_frame(&mut buf)?;
    let w = info.width;
    let h = info.height;
    let raw = &buf[..frame.buffer_size()];

    let rgba = match (info.color_type, info.bit_depth) {
        (::png::ColorType::Rgba, ::png::BitDepth::Eight) => raw.to_vec(),
        (::png::ColorType::Rgb, ::png::BitDepth::Eight) => expand_rgb_to_rgba(raw),
        (::png::ColorType::GrayscaleAlpha, ::png::BitDepth::Eight) => expand_ga_to_rgba(raw),
        (::png::ColorType::Grayscale, ::png::BitDepth::Eight) => expand_gray_to_rgba(raw),
        (ct, bd) => {
            return Err(PngDecodeError::Decoder(format!(
                "unsupported PNG format: color_type={ct:?} bit_depth={bd:?}"
            )));
        }
    };

    if rgba.len() != (w * h * 4) as usize {
        return Err(PngDecodeError::Decoder(format!(
            "pixel buffer size mismatch: got {} want {}",
            rgba.len(),
            w * h * 4
        )));
    }

    Ok((rgba, w, h))
}

/// Load a PNG file from disk as a `CursorImage` with an explicit hotspot.
pub fn load_png_cursor<P: AsRef<std::path::Path>>(
    path: P,
    hotspot_x: u32,
    hotspot_y: u32,
) -> Result<CursorImage, PngDecodeError> {
    let bytes = std::fs::read(path)?;
    let (pixels, w, h) = decode_rgba8(&bytes)?;
    Ok(CursorImage {
        width: w,
        height: h,
        hotspot_x: hotspot_x.min(w.saturating_sub(1)),
        hotspot_y: hotspot_y.min(h.saturating_sub(1)),
        pixels,
        nominal_size: w,
    })
}

fn expand_rgb_to_rgba(rgb: &[u8]) -> Vec<u8> {
    let pixels = rgb.len() / 3;
    let mut out = Vec::with_capacity(pixels * 4);
    for chunk in rgb.chunks_exact(3) {
        out.extend_from_slice(chunk);
        out.push(255);
    }
    out
}

fn expand_ga_to_rgba(ga: &[u8]) -> Vec<u8> {
    let pixels = ga.len() / 2;
    let mut out = Vec::with_capacity(pixels * 4);
    for chunk in ga.chunks_exact(2) {
        let g = chunk[0];
        let a = chunk[1];
        out.extend_from_slice(&[g, g, g, a]);
    }
    out
}

fn expand_gray_to_rgba(g: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(g.len() * 4);
    for &v in g {
        out.extend_from_slice(&[v, v, v, 255]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal 1×1 opaque-red RGBA PNG.
    const RED_1X1_RGBA_PNG: &[u8] = &[
        0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, // IHDR
        0x00, 0x00, 0x00, 0x0D, b'I', b'H', b'D', b'R', 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
        0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, // IDAT
        0x00, 0x00, 0x00, 0x0F, b'I', b'D', b'A', b'T', 0x78, 0x9C, 0x62, 0xF8, 0xCF, 0xC0, 0x00,
        0x00, 0x00, 0x04, 0x00, 0x01, 0x27, 0x18, 0xE3, 0x66, 0x3E, 0x85, 0xD6, 0xE5,
        // IEND
        0x00, 0x00, 0x00, 0x00, b'I', b'E', b'N', b'D', 0xAE, 0x42, 0x60, 0x82,
    ];

    #[test]
    fn decode_tiny_rgba_png() {
        let (px, w, h) = decode_rgba8(RED_1X1_RGBA_PNG).expect("decode");
        assert_eq!(w, 1);
        assert_eq!(h, 1);
        assert_eq!(px.len(), 4);
        // Red pixel: at least alpha should be 255 for opaque PNG.
        assert_eq!(px[3], 255);
    }

    #[test]
    fn decode_rejects_garbage() {
        assert!(decode_rgba8(&[0, 1, 2, 3]).is_err());
    }
}
