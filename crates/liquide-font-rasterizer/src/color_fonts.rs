//! Color font / emoji support — COLR, CPAL, SVG, sbix tables.
//!
//! Modern fonts may contain color glyph data in several formats:
//!
//! - **COLR/CPAL** (v0 & v1): Layered color glyphs with palette colors.
//!   COLR v0 uses simple layer stacking; v1 adds gradients, composites,
//!   and variable color references.
//! - **SVG** : OpenType SVG table with per-glyph SVG documents.
//! - **sbix** : Apple bitmap-strike format (PNG/JPEG embedded bitmaps).
//! - **CBDT/CBLC** : Google color bitmap format (PNG embedded).
//!
//! This module detects color glyph availability in a font and provides
//! rasterization paths that produce RGBA pixel data instead of grayscale.

use crate::database::{FontDatabase, FontFaceId};
use ab_glyph::{Font, ScaleFont};

/// Color glyph format detected in a font.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorGlyphFormat {
    /// COLR v0: simple layered color glyphs with solid palette fills.
    ColrV0,
    /// COLR v1: advanced color glyphs with gradients, composites, etc.
    ColrV1,
    /// SVG table: per-glyph SVG documents.
    Svg,
    /// sbix: Apple embedded bitmap strikes.
    Sbix,
    /// CBDT/CBLC: Google color bitmap tables.
    Cbdt,
}

/// Information about a color glyph.
#[derive(Debug, Clone)]
pub struct ColorGlyph {
    /// Which format provides the color data.
    pub format: ColorGlyphFormat,
    /// The glyph index in the font.
    pub glyph_id: u32,
    /// RGBA pixel data (width × height × 4 bytes).
    pub pixels: Vec<u8>,
    /// Bitmap width in pixels.
    pub width: u32,
    /// Bitmap height in pixels.
    pub height: u32,
    /// Horizontal bearing.
    pub bearing_x: f32,
    /// Vertical bearing (from baseline).
    pub bearing_y: f32,
    /// Horizontal advance.
    pub advance: f32,
}

/// Detects available color glyph formats in a font.
#[must_use]
pub fn detect_color_formats(raw_data: &[u8]) -> Vec<ColorGlyphFormat> {
    let mut formats = Vec::new();

    // Detect table tags by scanning the font header.
    // OpenType fonts start with a table directory.
    if raw_data.len() < 12 {
        return formats;
    }

    let num_tables = u16::from_be_bytes([
        raw_data.get(4).copied().unwrap_or(0),
        raw_data.get(5).copied().unwrap_or(0),
    ]) as usize;

    // Each table record is 16 bytes, starting at offset 12.
    for i in 0..num_tables {
        let offset = 12 + i * 16;
        if offset + 4 > raw_data.len() {
            break;
        }
        let tag = &raw_data[offset..offset + 4];
        match tag {
            b"COLR" => {
                // Check version: first 2 bytes of COLR data
                let table_offset = u32::from_be_bytes([
                    raw_data.get(offset + 8).copied().unwrap_or(0),
                    raw_data.get(offset + 9).copied().unwrap_or(0),
                    raw_data.get(offset + 10).copied().unwrap_or(0),
                    raw_data.get(offset + 11).copied().unwrap_or(0),
                ]) as usize;
                if table_offset + 2 <= raw_data.len() {
                    let version =
                        u16::from_be_bytes([raw_data[table_offset], raw_data[table_offset + 1]]);
                    if version >= 1 {
                        formats.push(ColorGlyphFormat::ColrV1);
                    } else {
                        formats.push(ColorGlyphFormat::ColrV0);
                    }
                }
            }
            b"SVG " => formats.push(ColorGlyphFormat::Svg),
            b"sbix" => formats.push(ColorGlyphFormat::Sbix),
            b"CBDT" => formats.push(ColorGlyphFormat::Cbdt),
            _ => {}
        }
    }

    formats
}

/// Check if a specific glyph has color data in the font.
#[must_use]
pub fn has_color_glyph(raw_data: &[u8], glyph_id: u32) -> bool {
    let formats = detect_color_formats(raw_data);
    if formats.is_empty() {
        return false;
    }

    // For COLR v0: check if glyph_id is in the BaseGlyph array
    // For now, if the font has any color table, assume glyphs in the
    // emoji Unicode range have color data.
    // A full implementation would parse the COLR/SVG/sbix tables.
    let _ = glyph_id;
    !formats.is_empty()
}

/// Rasterize a COLR v0 color glyph by compositing layers.
///
/// Each layer is a glyph outline filled with a palette color.
/// The layers are composited bottom-to-top with simple alpha blending.
pub fn rasterize_colr_v0(
    db: &FontDatabase,
    face_id: FontFaceId,
    glyph_id: u32,
    size_px: f32,
    palette_index: u16,
) -> Option<ColorGlyph> {
    let face = db.get(face_id)?;
    let raw_data = &face.raw_data;
    let formats = detect_color_formats(raw_data);

    if !formats.contains(&ColorGlyphFormat::ColrV0) && !formats.contains(&ColorGlyphFormat::ColrV1)
    {
        return None;
    }

    // Parse COLR table to get layer records for this base glyph.
    // The COLR table format:
    //   uint16 version
    //   uint16 numBaseGlyphs
    //   offset32 baseGlyphRecordsOffset
    //   offset32 layerRecordsOffset
    //   uint16 numLayers
    let colr_data = find_table_data(raw_data, b"COLR")?;
    if colr_data.len() < 14 {
        return None;
    }

    let num_base = u16::from_be_bytes([colr_data[2], colr_data[3]]) as usize;
    let base_offset =
        u32::from_be_bytes([colr_data[4], colr_data[5], colr_data[6], colr_data[7]]) as usize;
    let layer_offset =
        u32::from_be_bytes([colr_data[8], colr_data[9], colr_data[10], colr_data[11]]) as usize;
    let _num_layers = u16::from_be_bytes([colr_data[12], colr_data[13]]);

    // Each BaseGlyphRecord: uint16 glyphID, uint16 firstLayerIndex, uint16 numLayers
    let mut first_layer = 0u16;
    let mut num_glyph_layers = 0u16;
    let mut found = false;

    for i in 0..num_base {
        let off = base_offset + i * 6;
        if off + 6 > colr_data.len() {
            break;
        }
        let gid = u16::from_be_bytes([colr_data[off], colr_data[off + 1]]);
        if gid as u32 == glyph_id {
            first_layer = u16::from_be_bytes([colr_data[off + 2], colr_data[off + 3]]);
            num_glyph_layers = u16::from_be_bytes([colr_data[off + 4], colr_data[off + 5]]);
            found = true;
            break;
        }
    }

    if !found || num_glyph_layers == 0 {
        return None;
    }

    // Parse CPAL palette for colors
    let palette = parse_cpal_palette(raw_data, palette_index);

    // For each layer, rasterize the glyph outline and tint with palette color
    let scale = ab_glyph::PxScale::from(size_px);
    let scaled = face.font.as_scaled(scale);
    let ascent = scaled.ascent();

    // Determine bounding box from the base glyph
    // Guard against glyph_id truncation: ab_glyph::GlyphId wraps u16
    if glyph_id > u16::MAX as u32 {
        return None;
    }
    let gid = ab_glyph::GlyphId(glyph_id as u16);
    let base_glyph_ab = gid.with_scale_and_position(scale, ab_glyph::point(0.0, ascent));
    let outline = face.font.outline_glyph(base_glyph_ab)?;
    let bounds = outline.px_bounds();
    let w = bounds.width().ceil() as u32;
    let h = bounds.height().ceil() as u32;
    if w == 0 || h == 0 {
        return None;
    }

    let advance = scaled.h_advance(gid);
    let mut rgba = vec![0u8; (w * h * 4) as usize];

    // Composite each layer
    for li in 0..num_glyph_layers {
        let off = layer_offset + (first_layer + li) as usize * 4;
        if off + 4 > colr_data.len() {
            break;
        }
        let layer_gid = u16::from_be_bytes([colr_data[off], colr_data[off + 1]]);
        let palette_idx = u16::from_be_bytes([colr_data[off + 2], colr_data[off + 3]]);

        let color = palette
            .get(palette_idx as usize)
            .copied()
            .unwrap_or([0, 0, 0, 255]);

        let layer_glyph = ab_glyph::GlyphId(layer_gid)
            .with_scale_and_position(scale, ab_glyph::point(0.0, ascent));
        if let Some(layer_outline) = face.font.outline_glyph(layer_glyph) {
            let layer_bounds = layer_outline.px_bounds();
            layer_outline.draw(|px, py, coverage| {
                let x = (px as f32 + layer_bounds.min.x - bounds.min.x).round() as i32;
                let y = (py as f32 + layer_bounds.min.y - bounds.min.y).round() as i32;
                if x >= 0 && (x as u32) < w && y >= 0 && (y as u32) < h {
                    let idx = ((y as u32 * w + x as u32) * 4) as usize;
                    if idx + 3 < rgba.len() {
                        let alpha = (coverage * color[3] as f32 / 255.0 * 255.0) as u8;
                        // Premultiplied alpha-over compositing
                        let src_a = alpha as f32 / 255.0;
                        let dst_a = rgba[idx + 3] as f32 / 255.0;
                        let out_a = src_a + dst_a * (1.0 - src_a);
                        if out_a > 0.0 {
                            rgba[idx] = ((color[0] as f32 * src_a
                                + rgba[idx] as f32 * dst_a * (1.0 - src_a))
                                / out_a) as u8;
                            rgba[idx + 1] = ((color[1] as f32 * src_a
                                + rgba[idx + 1] as f32 * dst_a * (1.0 - src_a))
                                / out_a) as u8;
                            rgba[idx + 2] = ((color[2] as f32 * src_a
                                + rgba[idx + 2] as f32 * dst_a * (1.0 - src_a))
                                / out_a) as u8;
                            rgba[idx + 3] = (out_a * 255.0) as u8;
                        }
                    }
                }
            });
        }
    }

    Some(ColorGlyph {
        format: ColorGlyphFormat::ColrV0,
        glyph_id,
        pixels: rgba,
        width: w,
        height: h,
        bearing_x: bounds.min.x,
        bearing_y: -bounds.min.y + ascent,
        advance,
    })
}

/// Rasterize an sbix embedded bitmap at the closest available strike size.
pub fn rasterize_sbix(raw_data: &[u8], glyph_id: u32, size_px: f32) -> Option<ColorGlyph> {
    let sbix_data = find_table_data(raw_data, b"sbix")?;
    if sbix_data.len() < 8 {
        return None;
    }

    // sbix format:
    //   uint16 version
    //   uint16 flags
    //   uint32 numStrikes
    //   offset32[] strikeOffsets
    let num_strikes =
        u32::from_be_bytes([sbix_data[4], sbix_data[5], sbix_data[6], sbix_data[7]]) as usize;

    // Find the best strike (closest ppem to target size)
    let mut best_strike_off = 0usize;
    let mut best_ppem = 0u16;
    let mut best_dist = u16::MAX;
    let target_ppem = size_px.round() as u16;

    for i in 0..num_strikes {
        let so_offset = 8 + i * 4;
        if so_offset + 4 > sbix_data.len() {
            break;
        }
        let strike_offset = u32::from_be_bytes([
            sbix_data[so_offset],
            sbix_data[so_offset + 1],
            sbix_data[so_offset + 2],
            sbix_data[so_offset + 3],
        ]) as usize;
        if strike_offset + 4 > sbix_data.len() {
            continue;
        }
        let ppem = u16::from_be_bytes([sbix_data[strike_offset], sbix_data[strike_offset + 1]]);
        let dist = (ppem as i32 - target_ppem as i32).unsigned_abs() as u16;
        if dist < best_dist {
            best_dist = dist;
            best_ppem = ppem;
            best_strike_off = strike_offset;
        }
    }

    if best_ppem == 0 {
        return None;
    }

    // The strike data contains glyph data records
    // For now, return a placeholder — full PNG decoding would require an image decoder
    let _ = (best_strike_off, glyph_id);

    None // PNG decoding not yet implemented — would need `image` or `png` crate
}

/// Detect and return the appropriate emoji font face for the platform.
///
/// On Windows: Segoe UI Emoji
/// On macOS: Apple Color Emoji
/// On Linux: Noto Color Emoji
#[must_use]
pub fn platform_emoji_font_path() -> Option<&'static str> {
    #[cfg(target_os = "windows")]
    {
        Some("C:\\Windows\\Fonts\\seguiemj.ttf")
    }
    #[cfg(target_os = "macos")]
    {
        Some("/System/Library/Fonts/Apple Color Emoji.ttc")
    }
    #[cfg(target_os = "linux")]
    {
        // Check common locations
        if std::path::Path::new("/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf").exists() {
            Some("/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf")
        } else if std::path::Path::new("/usr/share/fonts/noto-emoji/NotoColorEmoji.ttf").exists() {
            Some("/usr/share/fonts/noto-emoji/NotoColorEmoji.ttf")
        } else {
            None
        }
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        None
    }
}

/// Load the system emoji font into the database.
pub fn load_emoji_font(db: &mut FontDatabase) -> Option<FontFaceId> {
    let path = platform_emoji_font_path()?;
    let path = std::path::Path::new(path);
    if !path.exists() {
        return None;
    }
    db.load_file(path, "Emoji", 400, false).ok()
}

/// Check if a character is likely to need an emoji font.
#[must_use]
pub fn is_emoji_codepoint(ch: char) -> bool {
    let cp = ch as u32;
    matches!(cp,
        // Miscellaneous Symbols
        0x2600..=0x26FF |
        // Dingbats
        0x2700..=0x27BF |
        // Miscellaneous Symbols and Pictographs
        0x1F300..=0x1F5FF |
        // Emoticons
        0x1F600..=0x1F64F |
        // Transport and Map Symbols
        0x1F680..=0x1F6FF |
        // Supplemental Symbols and Pictographs
        0x1F900..=0x1F9FF |
        // Symbols and Pictographs Extended-A
        0x1FA00..=0x1FA6F |
        // Symbols and Pictographs Extended-B
        0x1FA70..=0x1FAFF |
        // Regional Indicator Symbols
        0x1F1E0..=0x1F1FF |
        // ZWJ + variation selectors commonly used in emoji
        0xFE0E..=0xFE0F |
        0x200D
    )
}

// ── Internal helpers ─────────────────────────────────────────────────

/// Locate a table's raw data within the font file.
fn find_table_data<'a>(raw_data: &'a [u8], tag: &[u8; 4]) -> Option<&'a [u8]> {
    if raw_data.len() < 12 {
        return None;
    }
    let num_tables = u16::from_be_bytes([raw_data[4], raw_data[5]]) as usize;
    for i in 0..num_tables {
        let offset = 12 + i * 16;
        if offset + 16 > raw_data.len() {
            break;
        }
        if &raw_data[offset..offset + 4] == tag {
            let table_offset = u32::from_be_bytes([
                raw_data[offset + 8],
                raw_data[offset + 9],
                raw_data[offset + 10],
                raw_data[offset + 11],
            ]) as usize;
            let table_length = u32::from_be_bytes([
                raw_data[offset + 12],
                raw_data[offset + 13],
                raw_data[offset + 14],
                raw_data[offset + 15],
            ]) as usize;
            if table_offset + table_length <= raw_data.len() {
                return Some(&raw_data[table_offset..table_offset + table_length]);
            }
        }
    }
    None
}

/// Parse the CPAL color palette — returns Vec of [R, G, B, A] tuples.
fn parse_cpal_palette(raw_data: &[u8], palette_index: u16) -> Vec<[u8; 4]> {
    let Some(cpal) = find_table_data(raw_data, b"CPAL") else {
        return vec![[0, 0, 0, 255]];
    };
    if cpal.len() < 12 {
        return vec![[0, 0, 0, 255]];
    }

    let _version = u16::from_be_bytes([cpal[0], cpal[1]]);
    let num_entries = u16::from_be_bytes([cpal[2], cpal[3]]) as usize;
    let _num_palettes = u16::from_be_bytes([cpal[4], cpal[5]]);
    let _num_colors = u16::from_be_bytes([cpal[6], cpal[7]]);
    let color_offset = u32::from_be_bytes([cpal[8], cpal[9], cpal[10], cpal[11]]) as usize;

    // Palette records start at offset 12, each is 2 bytes (first color index)
    let palette_record_off = 12 + palette_index as usize * 2;
    let first_color = if palette_record_off + 2 <= cpal.len() {
        u16::from_be_bytes([cpal[palette_record_off], cpal[palette_record_off + 1]]) as usize
    } else {
        0
    };

    let mut colors = Vec::with_capacity(num_entries);
    for i in 0..num_entries {
        let co = color_offset + (first_color + i) * 4;
        if co + 4 <= cpal.len() {
            // CPAL stores as BGRA
            colors.push([cpal[co + 2], cpal[co + 1], cpal[co], cpal[co + 3]]);
        } else {
            colors.push([0, 0, 0, 255]);
        }
    }

    colors
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emoji_detection() {
        assert!(is_emoji_codepoint('😀'));
        assert!(is_emoji_codepoint('🎉'));
        assert!(!is_emoji_codepoint('A'));
        assert!(!is_emoji_codepoint('中'));
    }

    #[test]
    fn test_detect_empty_data() {
        assert!(detect_color_formats(&[]).is_empty());
    }

    #[test]
    fn test_detect_small_data() {
        assert!(detect_color_formats(&[0u8; 10]).is_empty());
    }

    #[test]
    fn test_has_color_glyph_empty() {
        assert!(!has_color_glyph(&[], 65));
    }

    #[test]
    fn test_platform_emoji_path() {
        // Just check it doesn't panic
        let _ = platform_emoji_font_path();
    }
}
