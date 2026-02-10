//! Glyph atlas for text rendering.
//!
//! Provides an alpha-only bitmap cache with row-based packing.
//! Actual glyph rasterization (FreeType) is out of scope — this
//! crate provides the atlas infrastructure for pre-rasterized bitmaps.
//!
//! Supports both greyscale and subpixel (LCD) glyph rendering.
//! Subpixel glyphs are stored at 3x width (one byte per subpixel
//! channel: R, G, B) and blitted with per-channel alpha masking.

use std::collections::HashMap;

use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Point;
use liquide_compositor::pixel::Color;

use crate::blend;

/// Subpixel rendering mode for LCD text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SubpixelMode {
    /// No subpixel rendering (greyscale alpha).
    #[default]
    None,
    /// Horizontal RGB subpixel layout (most common LCD panels).
    Rgb,
    /// Horizontal BGR subpixel layout.
    Bgr,
    /// Vertical RGB subpixel layout.
    Vrgb,
    /// Vertical BGR subpixel layout.
    Vbgr,
}

/// Key for a glyph in the atlas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    pub font_id: u32,
    pub glyph_id: u32,
    pub size_px: u16,
    /// Whether this key refers to a subpixel-rendered glyph.
    pub subpixel: bool,
}

/// A cached glyph in the atlas.
#[derive(Debug, Clone)]
pub struct CachedGlyph {
    /// X position in the atlas texture.
    pub atlas_x: u32,
    /// Y position in the atlas texture.
    pub atlas_y: u32,
    /// Glyph bitmap width (logical pixels, not atlas storage width).
    /// For subpixel glyphs, this is the display width; atlas stores 3x this.
    pub width: u32,
    /// Glyph bitmap height.
    pub height: u32,
    /// Horizontal bearing offset.
    pub bearing_x: i32,
    /// Vertical bearing offset.
    pub bearing_y: i32,
    /// Horizontal advance.
    pub advance: f32,
    /// Whether this glyph uses subpixel rendering.
    pub subpixel: bool,
}

/// Metrics for a glyph being inserted into the atlas.
pub struct GlyphMetrics {
    /// Glyph bitmap width (logical display pixels).
    pub width: u32,
    /// Glyph bitmap height.
    pub height: u32,
    /// Horizontal bearing offset.
    pub bearing_x: i32,
    /// Vertical bearing offset.
    pub bearing_y: i32,
    /// Horizontal advance.
    pub advance: f32,
}

/// A glyph atlas: alpha-only bitmap cache for text rendering.
///
/// Initial size: 1024x1024 (see spec section 8.1).
pub struct GlyphAtlas {
    /// Alpha-only pixel data (1 byte per pixel).
    pixels: Vec<u8>,
    width: u32,
    height: u32,
    entries: HashMap<GlyphKey, CachedGlyph>,
    /// Current packing cursor (top-left of next free region).
    cursor_x: u32,
    cursor_y: u32,
    /// Height of the tallest glyph in the current row.
    row_height: u32,
}

impl GlyphAtlas {
    /// Create a new glyph atlas.
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            pixels: vec![0u8; (width * height) as usize],
            width,
            height,
            entries: HashMap::new(),
            cursor_x: 0,
            cursor_y: 0,
            row_height: 0,
        }
    }

    /// Look up a glyph in the atlas.
    #[must_use]
    pub fn get(&self, key: &GlyphKey) -> Option<&CachedGlyph> {
        self.entries.get(key)
    }

    /// Insert a pre-rasterized glyph bitmap into the atlas.
    ///
    /// `bitmap` is alpha-only data (`width * height` bytes).
    pub fn insert(
        &mut self,
        key: GlyphKey,
        bitmap: &[u8],
        metrics: &GlyphMetrics,
    ) -> crate::Result<&CachedGlyph> {
        let GlyphMetrics { width, height, bearing_x, bearing_y, advance } = *metrics;

        // Check if it already exists
        if self.entries.contains_key(&key) {
            return Ok(&self.entries[&key]);
        }

        // Row-based packing: if the glyph doesn't fit on the current row, start a new row
        if self.cursor_x + width > self.width {
            self.cursor_x = 0;
            self.cursor_y += self.row_height + 1; // 1px padding
            self.row_height = 0;
        }

        if self.cursor_y + height > self.height {
            return Err(crate::RendererError::AtlasFull { size: self.width });
        }

        // Copy glyph bitmap into atlas
        for row in 0..height {
            let src_start = (row * width) as usize;
            let dst_start = ((self.cursor_y + row) * self.width + self.cursor_x) as usize;
            self.pixels[dst_start..dst_start + width as usize]
                .copy_from_slice(&bitmap[src_start..src_start + width as usize]);
        }

        let glyph = CachedGlyph {
            atlas_x: self.cursor_x,
            atlas_y: self.cursor_y,
            width,
            height,
            bearing_x,
            bearing_y,
            advance,
            subpixel: false,
        };

        self.cursor_x += width + 1; // 1px padding
        self.row_height = self.row_height.max(height);

        self.entries.insert(key, glyph);
        Ok(&self.entries[&key])
    }

    /// Blit a glyph from the atlas into a framebuffer at the given position.
    ///
    /// The glyph alpha is used as a mask with the given foreground color.
    pub fn blit_glyph(
        &self,
        fb: &mut FrameBuffer,
        glyph: &CachedGlyph,
        pos: Point,
        color: Color,
    ) {
        let dx = (pos.x + glyph.bearing_x as f32) as i32;
        let dy = (pos.y - glyph.bearing_y as f32) as i32;

        for row in 0..glyph.height {
            let fy = dy + row as i32;
            if fy < 0 || fy >= fb.height as i32 {
                continue;
            }
            for col in 0..glyph.width {
                let fx = dx + col as i32;
                if fx < 0 || fx >= fb.width as i32 {
                    continue;
                }
                let atlas_off =
                    ((glyph.atlas_y + row) * self.width + glyph.atlas_x + col) as usize;
                let alpha = self.pixels[atlas_off];
                if alpha == 0 {
                    continue;
                }

                // Use the glyph alpha to mask the foreground color
                let src = Color::new(
                    ((color.r as u16 * alpha as u16 + 127) / 255) as u8,
                    ((color.g as u16 * alpha as u16 + 127) / 255) as u8,
                    ((color.b as u16 * alpha as u16 + 127) / 255) as u8,
                    alpha,
                );

                let dst = fb.get_pixel(fx as u32, fy as u32);
                let result = blend::blend_src_over(dst, src);
                fb.set_pixel(fx as u32, fy as u32, result);
            }
        }
    }

    /// Insert a subpixel-rendered glyph bitmap into the atlas.
    ///
    /// `bitmap` contains 3 bytes per display pixel per row (R alpha, G alpha,
    /// B alpha), so the total size is `width * 3 * height` bytes.
    /// The atlas stores all 3 channels packed; `width` is the *display* width.
    pub fn insert_subpixel(
        &mut self,
        key: GlyphKey,
        bitmap: &[u8],
        metrics: &GlyphMetrics,
    ) -> crate::Result<&CachedGlyph> {
        let GlyphMetrics { width, height, bearing_x, bearing_y, advance } = *metrics;

        if self.entries.contains_key(&key) {
            return Ok(&self.entries[&key]);
        }

        // Subpixel glyphs are stored at 3x width in the atlas
        let atlas_width = width * 3;

        if self.cursor_x + atlas_width > self.width {
            self.cursor_x = 0;
            self.cursor_y += self.row_height + 1;
            self.row_height = 0;
        }

        if self.cursor_y + height > self.height {
            return Err(crate::RendererError::AtlasFull { size: self.width });
        }

        // Copy subpixel bitmap into atlas (3 bytes per display pixel)
        for row in 0..height {
            let src_start = (row * atlas_width) as usize;
            let dst_start = ((self.cursor_y + row) * self.width + self.cursor_x) as usize;
            self.pixels[dst_start..dst_start + atlas_width as usize]
                .copy_from_slice(&bitmap[src_start..src_start + atlas_width as usize]);
        }

        let glyph = CachedGlyph {
            atlas_x: self.cursor_x,
            atlas_y: self.cursor_y,
            width,
            height,
            bearing_x,
            bearing_y,
            advance,
            subpixel: true,
        };

        self.cursor_x += atlas_width + 1;
        self.row_height = self.row_height.max(height);

        self.entries.insert(key, glyph);
        Ok(&self.entries[&key])
    }

    /// Blit a subpixel glyph from the atlas into a framebuffer.
    ///
    /// Each display pixel has separate R/G/B alpha channels stored as 3
    /// consecutive bytes in the atlas. The subpixel mode controls how
    /// these channels map to physical subpixels.
    pub fn blit_glyph_subpixel(
        &self,
        fb: &mut FrameBuffer,
        glyph: &CachedGlyph,
        pos: Point,
        color: Color,
        mode: SubpixelMode,
    ) {
        let dx = (pos.x + glyph.bearing_x as f32) as i32;
        let dy = (pos.y - glyph.bearing_y as f32) as i32;

        for row in 0..glyph.height {
            let fy = dy + row as i32;
            if fy < 0 || fy >= fb.height as i32 {
                continue;
            }
            for col in 0..glyph.width {
                let fx = dx + col as i32;
                if fx < 0 || fx >= fb.width as i32 {
                    continue;
                }

                // Read 3 subpixel alpha values from atlas
                let atlas_base = ((glyph.atlas_y + row) * self.width
                    + glyph.atlas_x
                    + col * 3) as usize;
                let a0 = self.pixels[atlas_base];
                let a1 = self.pixels[atlas_base + 1];
                let a2 = self.pixels[atlas_base + 2];

                if a0 == 0 && a1 == 0 && a2 == 0 {
                    continue;
                }

                // Map subpixel channels to R/G/B alpha based on mode
                let (alpha_r, alpha_g, alpha_b) = match mode {
                    SubpixelMode::None => {
                        // Fallback: average the three channels
                        let avg = ((a0 as u16 + a1 as u16 + a2 as u16 + 1) / 3) as u8;
                        (avg, avg, avg)
                    }
                    SubpixelMode::Rgb => (a0, a1, a2),
                    SubpixelMode::Bgr => (a2, a1, a0),
                    SubpixelMode::Vrgb => {
                        // For vertical subpixels, we apply the same mapping
                        // per row offset; here we just use direct mapping
                        // since the rasterizer should have already arranged
                        // the data for vertical layout.
                        (a0, a1, a2)
                    }
                    SubpixelMode::Vbgr => (a2, a1, a0),
                };

                // Per-channel alpha blending: blend each channel independently
                let dst = fb.get_pixel(fx as u32, fy as u32);

                let r = blend_channel(color.r, dst.r, alpha_r);
                let g = blend_channel(color.g, dst.g, alpha_g);
                let b = blend_channel(color.b, dst.b, alpha_b);

                // Alpha: use max of the three subpixel alphas for compositing
                let alpha_max = alpha_r.max(alpha_g).max(alpha_b);
                let a = blend_channel(255, dst.a, alpha_max);

                fb.set_pixel(fx as u32, fy as u32, Color::new(r, g, b, a));
            }
        }
    }

    /// Get the atlas bitmap (for debugging / inspection).
    #[must_use]
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// Atlas dimensions.
    #[must_use]
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Number of cached glyphs.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the atlas is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all cached glyphs and reset the packing cursor.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.pixels.fill(0);
        self.cursor_x = 0;
        self.cursor_y = 0;
        self.row_height = 0;
    }
}

/// Blend a single colour channel with per-channel alpha.
///
/// `src * alpha + dst * (1 - alpha)`, all in 0–255 range.
#[inline]
fn blend_channel(src: u8, dst: u8, alpha: u8) -> u8 {
    let s = src as u16;
    let d = dst as u16;
    let a = alpha as u16;
    ((s * a + d * (255 - a) + 127) / 255) as u8
}
