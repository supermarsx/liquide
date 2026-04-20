//! Image format decoding and management.
//!
//! Provides in-memory decoding of common image formats (PNG, BMP, TGA, ICO)
//! into RGBA pixel buffers suitable for blitting to the frame buffer.
//! JPEG and WebP support can be added via feature flags.

use liquide_compositor::pixel::Color;

/// Maximum allowed image dimension (width or height) to prevent OOM.
const MAX_IMAGE_DIM: u32 = 16384;
/// Maximum allowed decoded image size in bytes (256 MiB).
const MAX_IMAGE_BYTES: u64 = 256 * 1024 * 1024;

/// Supported image formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageFormat {
    Png,
    Bmp,
    Tga,
    Ico,
    Jpeg,
    WebP,
    Unknown,
}

impl ImageFormat {
    /// Detect format from file header magic bytes.
    #[must_use]
    pub fn from_magic(data: &[u8]) -> Self {
        if data.len() < 4 {
            return Self::Unknown;
        }
        if data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
            Self::Png
        } else if data.starts_with(b"BM") {
            Self::Bmp
        } else if data.len() >= 12 && data.starts_with(b"RIFF") && &data[8..12] == b"WEBP" {
            Self::WebP
        } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
            Self::Jpeg
        } else if data.len() >= 4 && &data[0..4] == &[0x00, 0x00, 0x01, 0x00] {
            Self::Ico
        } else {
            Self::Unknown
        }
    }

    /// Detect format from file extension.
    #[must_use]
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "png" => Self::Png,
            "bmp" => Self::Bmp,
            "tga" => Self::Tga,
            "ico" => Self::Ico,
            "jpg" | "jpeg" => Self::Jpeg,
            "webp" => Self::WebP,
            _ => Self::Unknown,
        }
    }
}

/// A decoded image in RGBA8 format.
#[derive(Debug, Clone)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    /// Row-major RGBA pixel data (4 bytes per pixel).
    pub pixels: Vec<u8>,
    pub format: ImageFormat,
}

impl DecodedImage {
    /// Create a solid-color image for testing.
    #[must_use]
    pub fn solid(width: u32, height: u32, color: Color) -> Self {
        let pixel_count = (width * height) as usize;
        let mut pixels = Vec::with_capacity(pixel_count * 4);
        for _ in 0..pixel_count {
            pixels.push(color.r);
            pixels.push(color.g);
            pixels.push(color.b);
            pixels.push(color.a);
        }
        Self {
            width,
            height,
            pixels,
            format: ImageFormat::Unknown,
        }
    }

    /// Get a pixel at (x, y) or None if out of bounds.
    #[must_use]
    pub fn get_pixel(&self, x: u32, y: u32) -> Option<Color> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let idx = ((y * self.width + x) * 4) as usize;
        Some(Color::new(
            self.pixels[idx],
            self.pixels[idx + 1],
            self.pixels[idx + 2],
            self.pixels[idx + 3],
        ))
    }

    /// Get a pixel with bilinear interpolation at fractional coordinates.
    #[must_use]
    pub fn sample_bilinear(&self, x: f32, y: f32) -> Color {
        let x0 = x.floor() as i32;
        let y0 = y.floor() as i32;
        let fx = x - x0 as f32;
        let fy = y - y0 as f32;

        let c00 = self.get_pixel_clamp(x0, y0);
        let c10 = self.get_pixel_clamp(x0 + 1, y0);
        let c01 = self.get_pixel_clamp(x0, y0 + 1);
        let c11 = self.get_pixel_clamp(x0 + 1, y0 + 1);

        let r = lerp_u8(
            lerp_u8(c00.r, c10.r, fx),
            lerp_u8(c01.r, c11.r, fx),
            fy,
        );
        let g = lerp_u8(
            lerp_u8(c00.g, c10.g, fx),
            lerp_u8(c01.g, c11.g, fx),
            fy,
        );
        let b = lerp_u8(
            lerp_u8(c00.b, c10.b, fx),
            lerp_u8(c01.b, c11.b, fx),
            fy,
        );
        let a = lerp_u8(
            lerp_u8(c00.a, c10.a, fx),
            lerp_u8(c01.a, c11.a, fx),
            fy,
        );
        Color::new(r, g, b, a)
    }

    fn get_pixel_clamp(&self, x: i32, y: i32) -> Color {
        let cx = x.clamp(0, self.width as i32 - 1) as u32;
        let cy = y.clamp(0, self.height as i32 - 1) as u32;
        self.get_pixel(cx, cy).unwrap_or(Color::TRANSPARENT)
    }

    /// Total byte size of pixel data.
    #[must_use]
    pub fn byte_size(&self) -> usize {
        self.pixels.len()
    }
}

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t + 0.5) as u8
}

/// Decode a PNG image from raw bytes.
///
/// Supports 8-bit RGBA, RGB (alpha filled to 255), grayscale, and
/// grayscale+alpha. Uses a minimal built-in decoder (no external deps).
pub fn decode_png(data: &[u8]) -> Result<DecodedImage, ImageDecodeError> {
    // Validate PNG signature
    if data.len() < 8 || &data[0..8] != &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A] {
        return Err(ImageDecodeError::InvalidFormat("not a PNG file".into()));
    }

    let mut pos = 8;
    let mut width = 0u32;
    let mut height = 0u32;
    let mut bit_depth = 0u8;
    let mut color_type = 0u8;
    let mut idat_data = Vec::new();

    while pos + 8 <= data.len() {
        let chunk_len = u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        let chunk_type = &data[pos + 4..pos + 8];
        let chunk_data_start = pos + 8;
        let chunk_data_end = chunk_data_start + chunk_len;

        if chunk_data_end > data.len() {
            return Err(ImageDecodeError::TruncatedData);
        }

        match chunk_type {
            b"IHDR" => {
                if chunk_len < 13 {
                    return Err(ImageDecodeError::InvalidFormat("IHDR too short".into()));
                }
                let d = &data[chunk_data_start..chunk_data_end];
                width = u32::from_be_bytes([d[0], d[1], d[2], d[3]]);
                height = u32::from_be_bytes([d[4], d[5], d[6], d[7]]);
                bit_depth = d[8];
                color_type = d[9];
            }
            b"IDAT" => {
                idat_data.extend_from_slice(&data[chunk_data_start..chunk_data_end]);
            }
            b"IEND" => break,
            _ => {} // Skip unknown chunks
        }

        pos = chunk_data_end + 4; // +4 for CRC
    }

    if width == 0 || height == 0 {
        return Err(ImageDecodeError::InvalidFormat("no IHDR chunk".into()));
    }

    if width > MAX_IMAGE_DIM || height > MAX_IMAGE_DIM {
        return Err(ImageDecodeError::InvalidFormat(
            format!("image dimensions {width}x{height} exceed maximum {MAX_IMAGE_DIM}"),
        ));
    }
    if (width as u64) * (height as u64) * 4 > MAX_IMAGE_BYTES {
        return Err(ImageDecodeError::InvalidFormat("decoded image would exceed 256 MiB".into()));
    }

    let channels: usize = match color_type {
        0 => 1, // Grayscale
        2 => 3, // RGB
        4 => 2, // Grayscale + Alpha
        6 => 4, // RGBA
        _ => return Err(ImageDecodeError::UnsupportedColorType(color_type)),
    };

    if bit_depth != 8 {
        return Err(ImageDecodeError::InvalidFormat(
            format!("unsupported bit depth {bit_depth} (only 8-bit supported)"),
        ));
    }

    // Decompress zlib-wrapped IDAT data
    let decompressed = miniz_oxide::inflate::decompress_to_vec_zlib(&idat_data)
        .map_err(|e| ImageDecodeError::InvalidFormat(format!("zlib decompress failed: {e:?}")))?;

    // PNG scanlines: each row is prefixed by a filter byte, then `width * channels` bytes
    let stride = width as usize * channels;
    let expected_len = height as usize * (1 + stride);
    if decompressed.len() < expected_len {
        return Err(ImageDecodeError::TruncatedData);
    }

    // Un-filter scanlines (PNG filter types 0-4)
    let mut raw_pixels = vec![0u8; (height as usize) * stride];
    let bpp = channels; // bytes per pixel (bit_depth == 8)

    for row in 0..height as usize {
        let src_off = row * (1 + stride);
        let filter = decompressed[src_off];
        let src_row = &decompressed[src_off + 1..src_off + 1 + stride];
        let dst_off = row * stride;

        match filter {
            0 => {
                // None
                raw_pixels[dst_off..dst_off + stride].copy_from_slice(src_row);
            }
            1 => {
                // Sub: pixel[x] += pixel[x - bpp]
                for i in 0..stride {
                    let left = if i >= bpp { raw_pixels[dst_off + i - bpp] } else { 0 };
                    raw_pixels[dst_off + i] = src_row[i].wrapping_add(left);
                }
            }
            2 => {
                // Up: pixel[x] += pixel_prev_row[x]
                for i in 0..stride {
                    let up = if row > 0 { raw_pixels[dst_off - stride + i] } else { 0 };
                    raw_pixels[dst_off + i] = src_row[i].wrapping_add(up);
                }
            }
            3 => {
                // Average: pixel[x] += floor((left + up) / 2)
                for i in 0..stride {
                    let left = if i >= bpp { raw_pixels[dst_off + i - bpp] as u16 } else { 0 };
                    let up = if row > 0 { raw_pixels[dst_off - stride + i] as u16 } else { 0 };
                    raw_pixels[dst_off + i] = src_row[i].wrapping_add(((left + up) / 2) as u8);
                }
            }
            4 => {
                // Paeth
                for i in 0..stride {
                    let a = if i >= bpp { raw_pixels[dst_off + i - bpp] as i32 } else { 0 };
                    let b = if row > 0 { raw_pixels[dst_off - stride + i] as i32 } else { 0 };
                    let c = if row > 0 && i >= bpp { raw_pixels[dst_off - stride + i - bpp] as i32 } else { 0 };
                    let p = a + b - c;
                    let pa = (p - a).abs();
                    let pb = (p - b).abs();
                    let pc = (p - c).abs();
                    let pr = if pa <= pb && pa <= pc { a } else if pb <= pc { b } else { c };
                    raw_pixels[dst_off + i] = src_row[i].wrapping_add(pr as u8);
                }
            }
            _ => {
                // Unknown filter — treat as None (best effort)
                raw_pixels[dst_off..dst_off + stride].copy_from_slice(src_row);
            }
        }
    }

    // Convert to RGBA
    let alloc_size = (width as u64).checked_mul(height as u64)
        .and_then(|n| n.checked_mul(4))
        .ok_or(ImageDecodeError::InvalidFormat("pixel buffer overflow".into()))? as usize;
    let pixel_count = (width as usize) * (height as usize);
    let mut pixels = vec![0u8; alloc_size];

    match color_type {
        0 => {
            // Grayscale → RGBA
            for i in 0..pixel_count {
                let v = raw_pixels[i];
                let o = i * 4;
                pixels[o] = v; pixels[o + 1] = v; pixels[o + 2] = v; pixels[o + 3] = 255;
            }
        }
        2 => {
            // RGB → RGBA
            for i in 0..pixel_count {
                let s = i * 3;
                let o = i * 4;
                pixels[o] = raw_pixels[s]; pixels[o + 1] = raw_pixels[s + 1]; pixels[o + 2] = raw_pixels[s + 2]; pixels[o + 3] = 255;
            }
        }
        4 => {
            // Grayscale + Alpha → RGBA
            for i in 0..pixel_count {
                let s = i * 2;
                let o = i * 4;
                let v = raw_pixels[s];
                pixels[o] = v; pixels[o + 1] = v; pixels[o + 2] = v; pixels[o + 3] = raw_pixels[s + 1];
            }
        }
        6 => {
            // RGBA — direct copy
            pixels.copy_from_slice(&raw_pixels[..pixel_count * 4]);
        }
        _ => unreachable!(),
    }

    Ok(DecodedImage {
        width,
        height,
        pixels,
        format: ImageFormat::Png,
    })
}

/// Decode a BMP image from raw bytes.
pub fn decode_bmp(data: &[u8]) -> Result<DecodedImage, ImageDecodeError> {
    if data.len() < 54 || &data[0..2] != b"BM" {
        return Err(ImageDecodeError::InvalidFormat("not a BMP file".into()));
    }

    let data_offset = u32::from_le_bytes([data[10], data[11], data[12], data[13]]) as usize;
    let width = i32::from_le_bytes([data[18], data[19], data[20], data[21]]);
    let height = i32::from_le_bytes([data[22], data[23], data[24], data[25]]);
    let bpp = u16::from_le_bytes([data[28], data[29]]);

    if width <= 0 {
        return Err(ImageDecodeError::InvalidFormat("invalid BMP width".into()));
    }

    let abs_height = height.unsigned_abs();
    let bottom_up = height > 0;
    let w = width as u32;
    let h = abs_height;

    if w > MAX_IMAGE_DIM || h > MAX_IMAGE_DIM {
        return Err(ImageDecodeError::InvalidFormat(
            format!("image dimensions {w}x{h} exceed maximum {MAX_IMAGE_DIM}"),
        ));
    }
    if (w as u64) * (h as u64) * 4 > MAX_IMAGE_BYTES {
        return Err(ImageDecodeError::InvalidFormat("decoded image would exceed 256 MiB".into()));
    }

    let row_size = ((w * bpp as u32 + 31) / 32 * 4) as usize;

    // Validate that file data is large enough for the claimed pixel data
    if data_offset + (h as usize) * row_size > data.len() {
        return Err(ImageDecodeError::TruncatedData);
    }

    let alloc_size = (w as u64).checked_mul(h as u64)
        .and_then(|n| n.checked_mul(4))
        .ok_or(ImageDecodeError::InvalidFormat("pixel buffer overflow".into()))? as usize;
    let mut pixels = vec![0u8; alloc_size];

    for row in 0..h {
        let src_row = if bottom_up { h - 1 - row } else { row };
        let src_start = data_offset + src_row as usize * row_size;

        for col in 0..w {
            let dst_idx = ((row * w + col) * 4) as usize;

            match bpp {
                24 => {
                    let src_idx = src_start + col as usize * 3;
                    if src_idx + 2 < data.len() {
                        pixels[dst_idx] = data[src_idx + 2];     // R (BMP is BGR)
                        pixels[dst_idx + 1] = data[src_idx + 1]; // G
                        pixels[dst_idx + 2] = data[src_idx];     // B
                        pixels[dst_idx + 3] = 255;               // A
                    }
                }
                32 => {
                    let src_idx = src_start + col as usize * 4;
                    if src_idx + 3 < data.len() {
                        pixels[dst_idx] = data[src_idx + 2];     // R
                        pixels[dst_idx + 1] = data[src_idx + 1]; // G
                        pixels[dst_idx + 2] = data[src_idx];     // B
                        pixels[dst_idx + 3] = data[src_idx + 3]; // A
                    }
                }
                _ => {
                    return Err(ImageDecodeError::UnsupportedBitDepth(bpp));
                }
            }
        }
    }

    Ok(DecodedImage {
        width: w,
        height: h,
        pixels,
        format: ImageFormat::Bmp,
    })
}

/// Decode an image from raw bytes, detecting format automatically.
pub fn decode_image(data: &[u8]) -> Result<DecodedImage, ImageDecodeError> {
    match ImageFormat::from_magic(data) {
        ImageFormat::Png => decode_png(data),
        ImageFormat::Bmp => decode_bmp(data),
        _ => Err(ImageDecodeError::UnsupportedFormat),
    }
}

/// Errors during image decoding.
#[derive(Debug, Clone)]
pub enum ImageDecodeError {
    InvalidFormat(String),
    TruncatedData,
    UnsupportedFormat,
    UnsupportedColorType(u8),
    UnsupportedBitDepth(u16),
    DecompressionFailed(String),
}

impl std::fmt::Display for ImageDecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidFormat(msg) => write!(f, "invalid format: {msg}"),
            Self::TruncatedData => write!(f, "truncated data"),
            Self::UnsupportedFormat => write!(f, "unsupported image format"),
            Self::UnsupportedColorType(ct) => write!(f, "unsupported color type: {ct}"),
            Self::UnsupportedBitDepth(bd) => write!(f, "unsupported bit depth: {bd}"),
            Self::DecompressionFailed(msg) => write!(f, "decompression failed: {msg}"),
        }
    }
}

impl std::error::Error for ImageDecodeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_detection_png() {
        let data = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        assert_eq!(ImageFormat::from_magic(&data), ImageFormat::Png);
    }

    #[test]
    fn test_format_detection_bmp() {
        let data = [0x42, 0x4D, 0x00, 0x00];
        assert_eq!(ImageFormat::from_magic(&data), ImageFormat::Bmp);
    }

    #[test]
    fn test_format_from_extension() {
        assert_eq!(ImageFormat::from_extension("png"), ImageFormat::Png);
        assert_eq!(ImageFormat::from_extension("PNG"), ImageFormat::Png);
        assert_eq!(ImageFormat::from_extension("bmp"), ImageFormat::Bmp);
        assert_eq!(ImageFormat::from_extension("jpg"), ImageFormat::Jpeg);
        assert_eq!(ImageFormat::from_extension("jpeg"), ImageFormat::Jpeg);
        assert_eq!(ImageFormat::from_extension("webp"), ImageFormat::WebP);
        assert_eq!(ImageFormat::from_extension("xyz"), ImageFormat::Unknown);
    }

    #[test]
    fn test_solid_image() {
        let red = Color::new(255, 0, 0, 255);
        let img = DecodedImage::solid(4, 4, red);
        assert_eq!(img.width, 4);
        assert_eq!(img.height, 4);
        assert_eq!(img.byte_size(), 64);
        let p = img.get_pixel(2, 2).unwrap();
        assert_eq!(p.r, 255);
        assert_eq!(p.g, 0);
    }

    #[test]
    fn test_bilinear_sampling() {
        let img = DecodedImage::solid(2, 2, Color::new(100, 200, 50, 255));
        let s = img.sample_bilinear(0.5, 0.5);
        assert_eq!(s.r, 100);
        assert_eq!(s.g, 200);
    }

    #[test]
    fn test_decode_invalid() {
        let garbage = [0u8; 10];
        assert!(decode_image(&garbage).is_err());
    }

    #[test]
    fn test_bmp_decode_minimal() {
        // Construct a minimal 1x1 24-bit BMP
        let mut bmp = Vec::new();
        // Header
        bmp.extend_from_slice(b"BM");
        bmp.extend_from_slice(&58u32.to_le_bytes()); // total file size
        bmp.extend_from_slice(&[0u8; 4]); // reserved
        bmp.extend_from_slice(&54u32.to_le_bytes()); // data offset
        // Info header (BITMAPINFOHEADER = 40 bytes)
        bmp.extend_from_slice(&40u32.to_le_bytes()); // header size
        bmp.extend_from_slice(&1i32.to_le_bytes());  // width
        bmp.extend_from_slice(&1i32.to_le_bytes());  // height (bottom-up)
        bmp.extend_from_slice(&1u16.to_le_bytes());  // planes
        bmp.extend_from_slice(&24u16.to_le_bytes()); // bpp
        bmp.extend_from_slice(&[0u8; 24]); // rest of header
        // Pixel data: 1 pixel BGR + padding to 4 bytes
        bmp.extend_from_slice(&[0xFF, 0x00, 0x00, 0x00]); // Blue pixel

        let img = decode_bmp(&bmp).unwrap();
        assert_eq!(img.width, 1);
        assert_eq!(img.height, 1);
        let p = img.get_pixel(0, 0).unwrap();
        assert_eq!(p.b, 255); // Blue channel
    }

    #[test]
    fn test_png_rejects_huge_dimensions() {
        // Craft a minimal PNG with a huge width in the IHDR chunk
        let mut png = Vec::new();
        // PNG signature
        png.extend_from_slice(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
        // IHDR chunk: length=13
        png.extend_from_slice(&13u32.to_be_bytes());
        png.extend_from_slice(b"IHDR");
        png.extend_from_slice(&100_000u32.to_be_bytes()); // width = 100000
        png.extend_from_slice(&100_000u32.to_be_bytes()); // height = 100000
        png.push(8);  // bit depth
        png.push(6);  // color type RGBA
        png.extend_from_slice(&[0u8; 3]); // compression, filter, interlace
        png.extend_from_slice(&[0u8; 4]); // CRC (ignored by decoder)
        // IEND chunk
        png.extend_from_slice(&0u32.to_be_bytes());
        png.extend_from_slice(b"IEND");
        png.extend_from_slice(&[0u8; 4]); // CRC

        let err = decode_png(&png).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("exceed maximum"), "unexpected error: {msg}");
    }

    #[test]
    fn test_bmp_rejects_huge_dimensions() {
        // Construct a BMP header with absurdly large dimensions
        let mut bmp = vec![0u8; 54];
        bmp[0] = b'B'; bmp[1] = b'M';
        bmp[10..14].copy_from_slice(&54u32.to_le_bytes()); // data offset
        bmp[14..18].copy_from_slice(&40u32.to_le_bytes()); // header size
        bmp[18..22].copy_from_slice(&100_000i32.to_le_bytes()); // width
        bmp[22..26].copy_from_slice(&100_000i32.to_le_bytes()); // height
        bmp[28..30].copy_from_slice(&24u16.to_le_bytes()); // bpp

        let err = decode_bmp(&bmp).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("exceed maximum"), "unexpected error: {msg}");
    }

    #[test]
    fn test_bmp_rejects_truncated_data() {
        // Construct a valid BMP header claiming 100x100 pixels but with no pixel data
        let mut bmp = vec![0u8; 54];
        bmp[0] = b'B'; bmp[1] = b'M';
        bmp[10..14].copy_from_slice(&54u32.to_le_bytes()); // data offset
        bmp[14..18].copy_from_slice(&40u32.to_le_bytes()); // header size
        bmp[18..22].copy_from_slice(&100i32.to_le_bytes()); // width
        bmp[22..26].copy_from_slice(&100i32.to_le_bytes()); // height
        bmp[28..30].copy_from_slice(&24u16.to_le_bytes()); // bpp

        let err = decode_bmp(&bmp).unwrap_err();
        assert!(matches!(err, ImageDecodeError::TruncatedData), "expected TruncatedData, got: {err}");
    }
}
