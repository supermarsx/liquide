//! Host-side screenshot fulfillment (t73-session item 3).
//!
//! The shell records a [`ScreenshotRequest`](liquide_shell::shell::ScreenshotRequest)
//! when a screenshot shortcut fires, but the shell never performs the capture
//! itself (it has no framebuffer and no OS capture capability on the headless
//! path — see t68-features §10). The session host owns the rendered framebuffer,
//! so it is the correct place to fulfil the request: it reads back the last
//! presented BGRA frame and writes a real PNG to disk.
//!
//! No new platform capability is needed — the capture is fulfilled entirely from
//! the session's own framebuffer, so this stays inside the session lock. A real
//! interactive Region selection or a true clipboard write would need extra
//! platform plumbing; those modes degrade to a full-frame PNG-to-disk here and
//! are documented as such (the request is still consumed and a file is written,
//! so nothing is silently dropped).
//!
//! ## PNG encoder
//!
//! The workspace has no real PNG encoder (`liquide-screenshot::output` advertises
//! PNG but actually writes a BMP). Rather than pull in an external crate (which
//! would touch the root manifest, outside this executor's lock), this module
//! ships a tiny dependency-free PNG writer that uses zlib **stored**
//! (uncompressed) DEFLATE blocks. The output is a valid, spec-conformant PNG
//! (correct signature, IHDR, IDAT with zlib header + Adler-32, IEND, per-chunk
//! CRC-32); it is simply larger than a compressed PNG. Correctness over size is
//! the right trade-off for a screenshot.

use std::path::{Path, PathBuf};

/// A captured frame ready to be encoded: tightly-packed or strided BGRA pixels.
pub(crate) struct ScreenshotFrame<'a> {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    /// BGRA8 pixel bytes, `stride * height` long.
    pub pixels: &'a [u8],
}

/// Resolve the directory screenshots are written to.
///
/// Honours `LIQUIDE_SCREENSHOT_DIR` (used by tests to redirect output to a temp
/// dir), then the per-OS Pictures/Screenshots folder, then the temp dir.
pub(crate) fn screenshot_directory() -> PathBuf {
    if let Some(dir) = std::env::var_os("LIQUIDE_SCREENSHOT_DIR") {
        let candidate = PathBuf::from(dir);
        if candidate.is_dir() || std::fs::create_dir_all(&candidate).is_ok() {
            return candidate;
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(profile) = std::env::var("USERPROFILE") {
            let pictures = PathBuf::from(profile).join("Pictures").join("Screenshots");
            if pictures.is_dir() || std::fs::create_dir_all(&pictures).is_ok() {
                return pictures;
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(home) = std::env::var("HOME") {
            let pictures = PathBuf::from(home).join("Pictures").join("Screenshots");
            if pictures.is_dir() || std::fs::create_dir_all(&pictures).is_ok() {
                return pictures;
            }
        }
    }

    std::env::temp_dir()
}

/// Build a default screenshot filename tagged with the capture mode and a
/// monotonic-ish timestamp so repeated captures don't collide.
pub(crate) fn default_filename(mode_tag: &str) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!(
        "screenshot_{}_{}_{}.png",
        mode_tag,
        now.as_secs(),
        now.subsec_nanos()
    )
}

/// Encode a BGRA frame as a PNG and write it to `path`.
pub(crate) fn write_png(frame: &ScreenshotFrame<'_>, path: &Path) -> std::io::Result<()> {
    let png = encode_png(frame);
    std::fs::write(path, png)
}

// ---------------------------------------------------------------------------
// Minimal dependency-free PNG encoder (RGBA8, zlib stored blocks).
// ---------------------------------------------------------------------------

fn encode_png(frame: &ScreenshotFrame<'_>) -> Vec<u8> {
    let width = frame.width.max(1);
    let height = frame.height.max(1);

    // 1. Build the raw image data: one filter byte (0 = None) per scanline,
    //    followed by RGBA pixels converted from the source BGRA framebuffer.
    let row_bytes = width as usize * 4;
    let mut raw = Vec::with_capacity((row_bytes + 1) * height as usize);
    for y in 0..height as usize {
        raw.push(0u8); // filter type: None
        let src_row = y * frame.stride as usize;
        for x in 0..width as usize {
            let off = src_row + x * 4;
            let (b, g, r, a) = if off + 3 < frame.pixels.len() {
                (
                    frame.pixels[off],
                    frame.pixels[off + 1],
                    frame.pixels[off + 2],
                    frame.pixels[off + 3],
                )
            } else {
                (0, 0, 0, 255)
            };
            raw.push(r);
            raw.push(g);
            raw.push(b);
            raw.push(a);
        }
    }

    // 2. Assemble the PNG byte stream.
    let mut out = Vec::new();
    out.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);

    // IHDR
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.push(8); // bit depth
    ihdr.push(6); // color type: RGBA
    ihdr.push(0); // compression: deflate
    ihdr.push(0); // filter method: adaptive
    ihdr.push(0); // interlace: none
    write_chunk(&mut out, b"IHDR", &ihdr);

    // IDAT (zlib-wrapped stored deflate of `raw`)
    let idat = zlib_stored(&raw);
    write_chunk(&mut out, b"IDAT", &idat);

    // IEND
    write_chunk(&mut out, b"IEND", &[]);

    out
}

fn write_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc_input = Vec::with_capacity(4 + data.len());
    crc_input.extend_from_slice(kind);
    crc_input.extend_from_slice(data);
    out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
}

/// Wrap `data` in a zlib stream using only **stored** (uncompressed) DEFLATE
/// blocks. Valid zlib: 2-byte header, stored blocks, 4-byte Adler-32 trailer.
fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + data.len() / 65535 * 5 + 16);
    // zlib header: CM=8 (deflate), CINFO=7 (32K window) → 0x78; FLG so that
    // (CMF*256 + FLG) % 31 == 0 with no preset dict and default level → 0x01.
    out.push(0x78);
    out.push(0x01);

    // Stored DEFLATE blocks, max 65535 bytes each.
    let mut offset = 0usize;
    let total = data.len();
    if total == 0 {
        // A single empty final stored block.
        out.push(0x01); // BFINAL=1, BTYPE=00
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&(!0u16).to_le_bytes());
    } else {
        while offset < total {
            let chunk = (total - offset).min(0xFFFF);
            let is_final = offset + chunk >= total;
            out.push(if is_final { 0x01 } else { 0x00 });
            let len = chunk as u16;
            out.extend_from_slice(&len.to_le_bytes());
            out.extend_from_slice(&(!len).to_le_bytes());
            out.extend_from_slice(&data[offset..offset + chunk]);
            offset += chunk;
        }
    }

    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

fn adler32(data: &[u8]) -> u32 {
    const MOD_ADLER: u32 = 65521;
    let mut a = 1u32;
    let mut b = 0u32;
    for &byte in data {
        a = (a + byte as u32) % MOD_ADLER;
        b = (b + a) % MOD_ADLER;
    }
    (b << 16) | a
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_has_valid_signature_and_chunks() {
        let pixels = vec![0u8; 4 * 2 * 2];
        let frame = ScreenshotFrame {
            width: 2,
            height: 2,
            stride: 8,
            pixels: &pixels,
        };
        let png = encode_png(&frame);
        // PNG signature.
        assert_eq!(&png[0..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
        // IHDR chunk type appears right after the 8-byte sig + 4-byte length.
        assert_eq!(&png[12..16], b"IHDR");
        // Must contain IDAT and IEND.
        assert!(png.windows(4).any(|w| w == b"IDAT"));
        assert!(png.windows(4).any(|w| w == b"IEND"));
    }

    #[test]
    fn ihdr_encodes_dimensions_and_rgba() {
        let pixels = vec![0u8; 4 * 3 * 5];
        let frame = ScreenshotFrame {
            width: 3,
            height: 5,
            stride: 12,
            pixels: &pixels,
        };
        let png = encode_png(&frame);
        // IHDR data starts at byte 16.
        let w = u32::from_be_bytes([png[16], png[17], png[18], png[19]]);
        let h = u32::from_be_bytes([png[20], png[21], png[22], png[23]]);
        assert_eq!(w, 3);
        assert_eq!(h, 5);
        assert_eq!(png[24], 8, "bit depth");
        assert_eq!(png[25], 6, "color type RGBA");
    }

    #[test]
    fn chunk_crc_is_correct() {
        // The IEND chunk has a well-known fixed CRC (0xAE426082).
        let mut out = Vec::new();
        write_chunk(&mut out, b"IEND", &[]);
        // length(4) + "IEND"(4) + crc(4)
        let crc = u32::from_be_bytes([out[8], out[9], out[10], out[11]]);
        assert_eq!(crc, 0xAE42_6082);
    }

    #[test]
    fn bgra_is_swapped_to_rgba_in_output() {
        // One red pixel in BGRA = [0, 0, 255, 255]; in PNG RGBA it must lead
        // with 255, 0, 0.
        let pixels = vec![0u8, 0u8, 255u8, 255u8];
        let frame = ScreenshotFrame {
            width: 1,
            height: 1,
            stride: 4,
            pixels: &pixels,
        };
        let raw_png = encode_png(&frame);
        // Decode the stored IDAT back: find IDAT, skip zlib header (2) + block
        // header (5), then the raw scanline begins (filter byte then RGBA).
        let idat_pos = raw_png
            .windows(4)
            .position(|w| w == b"IDAT")
            .expect("IDAT present");
        // 4 bytes type back to length, but we just index forward from data start:
        let data_start = idat_pos + 4; // skip "IDAT"
        // zlib header (2) + stored block header (1 + 2 + 2) = 7 bytes, then
        // filter byte (1), then R, G, B, A.
        let r = raw_png[data_start + 7 + 1];
        let g = raw_png[data_start + 7 + 2];
        let b = raw_png[data_start + 7 + 3];
        assert_eq!((r, g, b), (255, 0, 0));
    }

    #[test]
    fn write_png_creates_a_file() {
        let dir = std::env::temp_dir().join(format!(
            "liquide-t73-png-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("shot.png");
        let pixels = vec![0u8; 4 * 4 * 4];
        let frame = ScreenshotFrame {
            width: 4,
            height: 4,
            stride: 16,
            pixels: &pixels,
        };
        write_png(&frame, &path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[0..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
