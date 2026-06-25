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
use liquide_compositor::geometry::{Point, Rect};
use liquide_compositor::pixel::Color;

/// Resolve an optional glyph clip rect to inclusive-exclusive integer pixel
/// bounds `(cx0, cy0, cx1, cy1)`. When no clip is set the window is unbounded
/// (`i32::MIN..i32::MAX`) so the per-pixel checks become no-ops. This confines a
/// glyph blit to the active damage region for the damage-only raster path (t76).
#[inline]
fn glyph_clip_window(clip: Option<Rect>) -> (i32, i32, i32, i32) {
    match clip {
        None => (i32::MIN, i32::MIN, i32::MAX, i32::MAX),
        Some(c) => (
            c.x.floor() as i32,
            c.y.floor() as i32,
            c.right().ceil() as i32,
            c.bottom().ceil() as i32,
        ),
    }
}

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
        let GlyphMetrics {
            width,
            height,
            bearing_x,
            bearing_y,
            advance,
        } = *metrics;

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
            // Atlas is full — evict everything and retry once.
            tracing::debug!(entries = self.entries.len(), "glyph atlas full, resetting");
            self.clear();
            // After clear, cursors are at (0,0). If the single glyph is
            // larger than the entire atlas, give up.
            if width > self.width || height > self.height {
                return Err(crate::RendererError::AtlasFull { size: self.width });
            }
        }
        let _total = width
            .checked_mul(height)
            .ok_or(crate::RendererError::AtlasFull { size: self.width })?;
        if bitmap.len() < (width * height) as usize {
            return Err(crate::RendererError::InvalidGlyph);
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
    /// The glyph alpha is used as coverage for the given foreground color, and
    /// the coverage is composited in **linear light** (gamma-correct AA) via the
    /// supplied sRGB LUT — naive sRGB-space coverage blending makes light-on-dark
    /// text too thin and dark-on-light too heavy (t83-crisp #4c).
    ///
    /// Horizontal **subpixel positioning** is honoured: the fractional part of
    /// the pen X is used to resample the glyph coverage across two adjacent
    /// destination columns (a 2-tap box reconstruction). This means a glyph drawn
    /// at pen X = 0.0 and pen X = 0.5 lands on genuinely different columns /
    /// coverage instead of both flooring to the same integer column — removing
    /// the per-glyph "wobble"/uneven tracking the floor-snapping caused
    /// (t83-crisp #1). The vertical origin is round-to-nearest (not floored) to
    /// drop the systematic half-pixel bias.
    pub fn blit_glyph(
        &self,
        fb: &mut FrameBuffer,
        glyph: &CachedGlyph,
        pos: Point,
        color: Color,
        clip: Option<Rect>,
        lut: &crate::color::SrgbLut,
    ) {
        // Fractional pen position. The integer base column is the floor; the
        // fractional remainder `fx_frac` is the subpixel phase used to split each
        // source coverage sample between column `fx` (weight 1-frac) and the next
        // column `fx + 1` (weight frac).
        let pen_x = pos.x + glyph.bearing_x as f32;
        let base_x = pen_x.floor();
        let fx_frac = pen_x - base_x;
        let dx = base_x as i32;
        // Vertical: round to nearest to remove the half-pixel-down floor bias.
        let dy = (pos.y - glyph.bearing_y as f32).round() as i32;
        let (cx0, cy0, cx1, cy1) = glyph_clip_window(clip);

        // Pre-linearize the foreground color once (coverage is applied in linear
        // light, then the result is converted back to sRGB).
        let fg_lin = [
            lut.linearize(color.r),
            lut.linearize(color.g),
            lut.linearize(color.b),
        ];
        let color_a = color.a as f32 / 255.0;

        // Coverage-reconstruction scratch: one f32 per destination column
        // (`0..=glyph.width`). The reconstruction (a 2-tap box resample of the
        // source coverage) is branch-free and identical for every row of a given
        // phase, so it is the natural SIMD target. The expensive, branchy LUT
        // gamma blend stays scalar; both the scalar and SIMD reconstruction paths
        // evaluate the SAME float expression in the SAME order, so the produced
        // coverage — and therefore every output pixel — is bit-identical
        // regardless of which path runs (the SIMD/scalar byte-identity contract).
        let ncols = glyph.width as usize + 1;
        let mut cov_row = vec![0.0f32; ncols];

        for row in 0..glyph.height {
            let fy = dy + row as i32;
            if fy < 0 || fy >= fb.height as i32 {
                continue;
            }
            if fy < cy0 || fy >= cy1 {
                continue;
            }
            let atlas_row = ((glyph.atlas_y + row) * self.width) as usize;
            let src = &self.pixels[atlas_row + glyph.atlas_x as usize..];

            // Fill `cov_row[col]` = (left*frac + right*(1-frac)) / 255 for each
            // destination column, runtime-dispatched to SIMD when available.
            reconstruct_coverage(src, glyph.width as usize, fx_frac, &mut cov_row);

            // Walk one extra column so the rightmost source sample can spill its
            // fractional weight into the trailing destination column.
            for col in 0..ncols {
                let cov = cov_row[col];
                if cov <= 0.0 {
                    continue;
                }

                let fx = dx + col as i32;
                if fx < 0 || fx >= fb.width as i32 {
                    continue;
                }
                if fx < cx0 || fx >= cx1 {
                    continue;
                }

                // Effective source coverage for this pixel.
                let a = cov * color_a;
                if a <= 0.0 {
                    continue;
                }

                // Composite src-over in linear light: out = src*a + dst*(1-a).
                let dst = fb.get_pixel(fx as u32, fy as u32);
                let dr = lut.linearize(dst.r);
                let dg = lut.linearize(dst.g);
                let db = lut.linearize(dst.b);
                let inv = 1.0 - a;
                let r = lut.delinearize(fg_lin[0] * a + dr * inv);
                let g = lut.delinearize(fg_lin[1] * a + dg * inv);
                let b = lut.delinearize(fg_lin[2] * a + db * inv);
                // Alpha is linear in coverage (opacity), composite normally.
                let out_a = (a + (dst.a as f32 / 255.0) * inv).clamp(0.0, 1.0);
                let result = Color::new(r, g, b, (out_a * 255.0 + 0.5) as u8);
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
        let GlyphMetrics {
            width,
            height,
            bearing_x,
            bearing_y,
            advance,
        } = *metrics;

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
            // Atlas is full — evict everything and retry once.
            tracing::debug!(
                entries = self.entries.len(),
                "glyph atlas full (subpixel), resetting"
            );
            self.clear();
            if atlas_width > self.width || height > self.height {
                return Err(crate::RendererError::AtlasFull { size: self.width });
            }
        }
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
        clip: Option<Rect>,
    ) {
        let dx = (pos.x + glyph.bearing_x as f32) as i32;
        let dy = (pos.y - glyph.bearing_y as f32) as i32;
        let (cx0, cy0, cx1, cy1) = glyph_clip_window(clip);

        for row in 0..glyph.height {
            let fy = dy + row as i32;
            if fy < 0 || fy >= fb.height as i32 {
                continue;
            }
            if fy < cy0 || fy >= cy1 {
                continue;
            }
            for col in 0..glyph.width {
                let fx = dx + col as i32;
                if fx < 0 || fx >= fb.width as i32 {
                    continue;
                }
                if fx < cx0 || fx >= cx1 {
                    continue;
                }

                // Read 3 subpixel alpha values from atlas
                let atlas_base =
                    ((glyph.atlas_y + row) * self.width + glyph.atlas_x + col * 3) as usize;
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

    /// Reset the atlas, evicting all cached glyphs.
    ///
    /// Alias for [`clear`] — provided for semantic clarity when the caller
    /// intends an eviction rather than a teardown.
    pub fn reset(&mut self) {
        self.clear();
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

/// Reconstruct per-column glyph coverage for one atlas row into `out`.
///
/// `src` is the atlas row beginning at the glyph's first source column; `width`
/// is the number of real source columns. For each destination column
/// `col ∈ 0..=width`, the coverage is the 2-tap box resample
/// `(left*frac + right*(1-frac)) / 255`, where `left = src[col-1]` (0 at the left
/// edge) and `right = src[col]` (0 at the trailing column). `out` must have length
/// `width + 1`.
///
/// This is the deterministic, branch-free inner arithmetic of `blit_glyph`, run
/// once per glyph row. It is runtime-dispatched to an AVX2 / SSE2 kernel when the
/// CPU supports it and otherwise to the scalar reference. **All three paths
/// evaluate the identical f32 expression in the identical order (no FMA
/// contraction), so they produce bit-identical coverage** — the SIMD-vs-scalar
/// byte-identity contract the glyph blit relies on.
#[inline]
pub(crate) fn reconstruct_coverage(src: &[u8], width: usize, frac: f32, out: &mut [f32]) {
    debug_assert_eq!(out.len(), width + 1);
    // Tiny glyphs (empty cells, single columns) are not worth dispatching and the
    // SIMD edge handling assumes at least one real source column on each edge —
    // fall back to scalar, which is correct for every width including 0 and 1.
    if width < 2 {
        reconstruct_coverage_scalar(src, width, frac, out);
        return;
    }
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if std::is_x86_feature_detected!("avx2") {
            // SAFETY: guarded by the runtime AVX2 feature check.
            unsafe {
                reconstruct_coverage_avx2(src, width, frac, out);
            }
            return;
        }
        if std::is_x86_feature_detected!("sse2") {
            // SAFETY: guarded by the runtime SSE2 feature check.
            unsafe {
                reconstruct_coverage_sse2(src, width, frac, out);
            }
            return;
        }
    }
    reconstruct_coverage_scalar(src, width, frac, out);
}

/// Scalar reference for [`reconstruct_coverage`] — the byte-identity baseline.
#[inline]
fn reconstruct_coverage_scalar(src: &[u8], width: usize, frac: f32, out: &mut [f32]) {
    let inv = 1.0 - frac;
    for col in 0..=width {
        let left = if col == 0 { 0.0 } else { src[col - 1] as f32 };
        let right = if col == width { 0.0 } else { src[col] as f32 };
        out[col] = (left * frac + right * inv) / 255.0;
    }
}

/// SSE2 kernel for [`reconstruct_coverage`]: 4 destination columns per step.
///
/// # Safety
/// Caller must ensure the CPU supports SSE2 (checked via `is_x86_feature_detected`).
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "sse2")]
unsafe fn reconstruct_coverage_sse2(src: &[u8], width: usize, frac: f32, out: &mut [f32]) {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    let inv = 1.0 - frac;
    let vfrac = _mm_set1_ps(frac);
    let vinv = _mm_set1_ps(inv);
    let v255 = _mm_set1_ps(255.0);

    // Vectorize the interior columns 1..width where both taps are real source
    // bytes (left = src[col-1], right = src[col]); handle col 0 and col==width
    // (the edge spills) with the scalar expression so every lane uses the SAME
    // float ops as the scalar path.
    out[0] = (src[0] as f32 * inv) / 255.0; // left=0
    let mut col = 1usize;
    while col + 4 <= width {
        // left  = src[col-1..col+3], right = src[col..col+4]
        let left = _mm_set_ps(
            src[col + 2] as f32,
            src[col + 1] as f32,
            src[col] as f32,
            src[col - 1] as f32,
        );
        let right = _mm_set_ps(
            src[col + 3] as f32,
            src[col + 2] as f32,
            src[col + 1] as f32,
            src[col] as f32,
        );
        let num = _mm_add_ps(_mm_mul_ps(left, vfrac), _mm_mul_ps(right, vinv));
        let cov = _mm_div_ps(num, v255);
        // SAFETY: `out` has length width+1 and `col + 4 <= width`, so the 4-lane
        // store at `col` stays in bounds.
        unsafe { _mm_storeu_ps(out.as_mut_ptr().add(col), cov) };
        col += 4;
    }
    while col < width {
        let left = src[col - 1] as f32;
        let right = src[col] as f32;
        out[col] = (left * frac + right * inv) / 255.0;
        col += 1;
    }
    // Trailing column (col == width): right = 0.
    out[width] = (src[width - 1] as f32 * frac) / 255.0;
}

/// AVX2 kernel for [`reconstruct_coverage`]: 8 destination columns per step.
///
/// # Safety
/// Caller must ensure the CPU supports AVX2 (checked via `is_x86_feature_detected`).
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
unsafe fn reconstruct_coverage_avx2(src: &[u8], width: usize, frac: f32, out: &mut [f32]) {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    let inv = 1.0 - frac;
    let vfrac = _mm256_set1_ps(frac);
    let vinv = _mm256_set1_ps(inv);
    let v255 = _mm256_set1_ps(255.0);

    out[0] = (src[0] as f32 * inv) / 255.0; // left=0
    let mut col = 1usize;
    while col + 8 <= width {
        let left = _mm256_set_ps(
            src[col + 6] as f32,
            src[col + 5] as f32,
            src[col + 4] as f32,
            src[col + 3] as f32,
            src[col + 2] as f32,
            src[col + 1] as f32,
            src[col] as f32,
            src[col - 1] as f32,
        );
        let right = _mm256_set_ps(
            src[col + 7] as f32,
            src[col + 6] as f32,
            src[col + 5] as f32,
            src[col + 4] as f32,
            src[col + 3] as f32,
            src[col + 2] as f32,
            src[col + 1] as f32,
            src[col] as f32,
        );
        let num = _mm256_add_ps(_mm256_mul_ps(left, vfrac), _mm256_mul_ps(right, vinv));
        let cov = _mm256_div_ps(num, v255);
        // SAFETY: `out` has length width+1 and `col + 8 <= width`, so the 8-lane
        // store at `col` stays in bounds.
        unsafe { _mm256_storeu_ps(out.as_mut_ptr().add(col), cov) };
        col += 8;
    }
    while col < width {
        let left = src[col - 1] as f32;
        let right = src[col] as f32;
        out[col] = (left * frac + right * inv) / 255.0;
        col += 1;
    }
    out[width] = (src[width - 1] as f32 * frac) / 255.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bitmap(w: u32, h: u32) -> Vec<u8> {
        vec![128u8; (w * h) as usize]
    }

    fn make_key(id: u32) -> GlyphKey {
        GlyphKey {
            font_id: 1,
            glyph_id: id,
            size_px: 16,
            subpixel: false,
        }
    }

    fn make_metrics(w: u32, h: u32) -> GlyphMetrics {
        GlyphMetrics {
            width: w,
            height: h,
            bearing_x: 0,
            bearing_y: h as i32,
            advance: w as f32,
        }
    }

    #[test]
    fn test_atlas_reset_on_full() {
        // Small 32x32 atlas — can hold a handful of 10x10 glyphs.
        let mut atlas = GlyphAtlas::new(32, 32);

        // Fill the atlas until it's near capacity.
        let mut inserted = 0u32;
        for i in 0..20 {
            let key = make_key(i);
            let metrics = make_metrics(10, 10);
            let bitmap = make_bitmap(10, 10);
            match atlas.insert(key, &bitmap, &metrics) {
                Ok(_) => inserted += 1,
                Err(_) => break,
            }
        }
        assert!(inserted > 0);

        // Now insert a glyph that would overflow — the atlas should
        // auto-reset and succeed.
        let key = make_key(100);
        let metrics = make_metrics(10, 10);
        let bitmap = make_bitmap(10, 10);
        let result = atlas.insert(key, &bitmap, &metrics);
        assert!(result.is_ok(), "insert should succeed after atlas reset");

        // Previous glyphs should have been evicted.
        assert!(atlas.get(&make_key(0)).is_none());
    }

    #[test]
    fn test_atlas_reset_method() {
        let mut atlas = GlyphAtlas::new(64, 64);
        let key = make_key(1);
        let metrics = make_metrics(8, 8);
        let bitmap = make_bitmap(8, 8);
        atlas.insert(key, &bitmap, &metrics).unwrap();
        assert_eq!(atlas.len(), 1);

        atlas.reset();
        assert_eq!(atlas.len(), 0);
        assert!(atlas.is_empty());
    }

    #[test]
    fn test_atlas_oversized_glyph_fails() {
        let mut atlas = GlyphAtlas::new(16, 16);
        let key = make_key(1);
        let metrics = make_metrics(32, 32);
        let bitmap = make_bitmap(32, 32);
        let result = atlas.insert(key, &bitmap, &metrics);
        assert!(result.is_err());
    }

    /// SIMD byte-identity teeth test: the SSE2 and AVX2 coverage-reconstruction
    /// kernels must produce results BIT-FOR-BIT identical to the scalar reference
    /// across a spread of widths and subpixel phases. The glyph blit consumes this
    /// coverage directly, so bit-identical coverage ⇒ byte-identical pixels. If a
    /// kernel ever drifts (FMA contraction, reordered ops, a lane bug), the
    /// `to_bits()` comparison fails. RED if SIMD math diverges; GREEN otherwise.
    #[test]
    fn simd_coverage_reconstruction_is_bit_identical_to_scalar() {
        // A pseudo-random-but-deterministic source row of alpha bytes.
        let mut src = vec![0u8; 256];
        let mut s: u32 = 0x1234_5678;
        for b in src.iter_mut() {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            *b = (s >> 24) as u8;
        }

        for &width in &[0usize, 1, 2, 3, 4, 5, 7, 8, 9, 15, 16, 17, 31, 64, 100, 255] {
            for &frac in &[0.0f32, 0.1, 0.25, 0.5, 0.5001, 0.75, 0.999] {
                let mut scalar = vec![0.0f32; width + 1];
                super::reconstruct_coverage_scalar(&src, width, frac, &mut scalar);

                // The SIMD kernels assume at least two real source columns (one on
                // each edge); widths < 2 are routed to scalar by the dispatcher, so
                // only call the kernels directly for width >= 2.
                #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
                if width >= 2 {
                    if std::is_x86_feature_detected!("sse2") {
                        let mut sse = vec![0.0f32; width + 1];
                        // SAFETY: guarded by the runtime SSE2 feature check.
                        unsafe {
                            super::reconstruct_coverage_sse2(&src, width, frac, &mut sse);
                        }
                        for (i, (a, b)) in scalar.iter().zip(sse.iter()).enumerate() {
                            assert_eq!(
                                a.to_bits(),
                                b.to_bits(),
                                "SSE2 != scalar at col {i} (width={width}, frac={frac})"
                            );
                        }
                    }
                    if std::is_x86_feature_detected!("avx2") {
                        let mut avx = vec![0.0f32; width + 1];
                        // SAFETY: guarded by the runtime AVX2 feature check.
                        unsafe {
                            super::reconstruct_coverage_avx2(&src, width, frac, &mut avx);
                        }
                        for (i, (a, b)) in scalar.iter().zip(avx.iter()).enumerate() {
                            assert_eq!(
                                a.to_bits(),
                                b.to_bits(),
                                "AVX2 != scalar at col {i} (width={width}, frac={frac})"
                            );
                        }
                    }
                }

                // The runtime dispatcher must also agree with the scalar baseline.
                let mut dispatched = vec![0.0f32; width + 1];
                super::reconstruct_coverage(&src, width, frac, &mut dispatched);
                for (i, (a, b)) in scalar.iter().zip(dispatched.iter()).enumerate() {
                    assert_eq!(
                        a.to_bits(),
                        b.to_bits(),
                        "dispatched != scalar at col {i} (width={width}, frac={frac})"
                    );
                }
            }
        }
    }

    /// Full-blit byte-identity: blit the SAME glyph through `blit_glyph` (which
    /// runs the dispatched SIMD coverage path) and verify it is byte-for-byte
    /// equal to a reference framebuffer produced by the pure-scalar coverage. This
    /// guards the whole blit, not just the kernel in isolation.
    #[test]
    fn blit_glyph_simd_matches_scalar_pixels() {
        use crate::color::SrgbLut;
        use liquide_compositor::framebuffer::FrameBuffer;
        use liquide_compositor::geometry::Point;
        use liquide_compositor::pixel::PixelFormat;

        // Build an atlas with one varied-coverage glyph.
        let (w, h) = (23u32, 11u32);
        let mut bitmap = vec![0u8; (w * h) as usize];
        let mut s: u32 = 0xDEAD_BEEF;
        for px in bitmap.iter_mut() {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            *px = (s >> 24) as u8;
        }
        let mut atlas = GlyphAtlas::new(64, 64);
        let key = make_key(7);
        let metrics = GlyphMetrics {
            width: w,
            height: h,
            bearing_x: 1,
            bearing_y: h as i32,
            advance: w as f32,
        };
        let cached = atlas.insert(key, &bitmap, &metrics).unwrap().clone();

        let lut = SrgbLut::new();
        let color = Color::new(200, 120, 60, 255);

        // Reference: render with the dispatched blit at a non-integer pen so the
        // 2-tap coverage path is exercised. Because the dispatcher itself is what
        // the live renderer uses, we compare it against a second buffer rendered
        // the same way but on a host where (if SIMD is present) the kernel runs —
        // and additionally assert the kernel/scalar identity above. Here we render
        // twice and require determinism (same pixels every time) plus exact match
        // to a hand-rolled scalar blit.
        let seed_backdrop = |fb: &mut FrameBuffer| {
            for y in 0..fb.height {
                for x in 0..fb.width {
                    fb.set_pixel(x, y, Color::new((x * 6) as u8, (y * 12) as u8, 30, 255));
                }
            }
        };
        let mut fb_dispatch = FrameBuffer::new(40, 20, PixelFormat::Bgra8);
        let mut fb_scalar = FrameBuffer::new(40, 20, PixelFormat::Bgra8);
        seed_backdrop(&mut fb_dispatch);
        seed_backdrop(&mut fb_scalar);

        let pos = Point::new(5.5, 4.0);
        atlas.blit_glyph(&mut fb_dispatch, &cached, pos, color, None, &lut);

        // Scalar reference blit: identical math but coverage filled with the scalar
        // kernel only.
        scalar_reference_blit(&mut fb_scalar, &atlas, &cached, pos, color, &lut);

        for y in 0..fb_dispatch.height {
            for x in 0..fb_dispatch.width {
                assert_eq!(
                    fb_dispatch.get_pixel(x, y),
                    fb_scalar.get_pixel(x, y),
                    "dispatched blit != scalar blit at ({x},{y})"
                );
            }
        }
    }

    /// A pure-scalar re-implementation of `blit_glyph`'s inner blend, used only as
    /// the byte-identity reference for `blit_glyph_simd_matches_scalar_pixels`.
    fn scalar_reference_blit(
        fb: &mut liquide_compositor::framebuffer::FrameBuffer,
        atlas: &GlyphAtlas,
        glyph: &CachedGlyph,
        pos: liquide_compositor::geometry::Point,
        color: Color,
        lut: &crate::color::SrgbLut,
    ) {
        let pen_x = pos.x + glyph.bearing_x as f32;
        let base_x = pen_x.floor();
        let fx_frac = pen_x - base_x;
        let dx = base_x as i32;
        let dy = (pos.y - glyph.bearing_y as f32).round() as i32;
        let fg_lin = [
            lut.linearize(color.r),
            lut.linearize(color.g),
            lut.linearize(color.b),
        ];
        let color_a = color.a as f32 / 255.0;
        let inv_frac = 1.0 - fx_frac;
        for row in 0..glyph.height {
            let fy = dy + row as i32;
            if fy < 0 || fy >= fb.height as i32 {
                continue;
            }
            let atlas_row = ((glyph.atlas_y + row) * atlas.width) as usize;
            for col in 0..=glyph.width {
                let left = if col == 0 {
                    0.0
                } else {
                    atlas.pixels[atlas_row + (glyph.atlas_x + col - 1) as usize] as f32
                };
                let right = if col == glyph.width {
                    0.0
                } else {
                    atlas.pixels[atlas_row + (glyph.atlas_x + col) as usize] as f32
                };
                let cov = (left * fx_frac + right * inv_frac) / 255.0;
                if cov <= 0.0 {
                    continue;
                }
                let fx = dx + col as i32;
                if fx < 0 || fx >= fb.width as i32 {
                    continue;
                }
                let a = cov * color_a;
                if a <= 0.0 {
                    continue;
                }
                let dst = fb.get_pixel(fx as u32, fy as u32);
                let dr = lut.linearize(dst.r);
                let dg = lut.linearize(dst.g);
                let db = lut.linearize(dst.b);
                let inv = 1.0 - a;
                let r = lut.delinearize(fg_lin[0] * a + dr * inv);
                let g = lut.delinearize(fg_lin[1] * a + dg * inv);
                let b = lut.delinearize(fg_lin[2] * a + db * inv);
                let out_a = (a + (dst.a as f32 / 255.0) * inv).clamp(0.0, 1.0);
                fb.set_pixel(
                    fx as u32,
                    fy as u32,
                    Color::new(r, g, b, (out_a * 255.0 + 0.5) as u8),
                );
            }
        }
    }
}
