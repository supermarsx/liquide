//! Compositing effect implementations.
//!
//! Provides backdrop blur, box shadow, and inner glow effects using the
//! separable Gaussian blur engine from [`crate::blur`].

use liquide_compositor::effects::EffectParams;
use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::Color;

use crate::blend;
use crate::blur;

/// Trait for compositing effects.
///
/// Implementations should respect per-effect budgets from [`EffectParams`].
pub trait Effect {
    /// Render the effect into the frame buffer within the given region.
    fn render(&self, fb: &mut FrameBuffer, region: Rect, params: &EffectParams);

    /// Estimated cost in milliseconds for the given region size.
    fn estimated_cost_ms(&self, region: Rect) -> f64;
}

/// Backdrop blur effect (dual-pass separable Gaussian).
///
/// Extracts the region behind a glass surface, blurs it, and composites
/// a tint colour overlay. Respects `DegradationLevel` via `EffectParams`:
/// - blur_radius = 0 → skip blur entirely
/// - large blur_downsample → uses downsampled fast path
pub struct BackdropBlur;

impl BackdropBlur {
    /// Render a backdrop blur with a specific tint colour.
    pub fn render_with_tint(
        fb: &mut FrameBuffer,
        region: Rect,
        params: &EffectParams,
        tint: Color,
    ) {
        let radius = params.blur_radius;
        if radius == 0 {
            // Degradation disabled blur — just apply the tint overlay
            Self::apply_tint(fb, region, tint);
            return;
        }

        // Use the fast path for large radii
        if radius >= 8 {
            blur::blur_fast(fb, region, radius);
        } else {
            blur::blur_region(fb, region, radius);
        }

        // Apply tint overlay
        Self::apply_tint(fb, region, tint);
    }

    /// Apply a colour tint overlay with SrcOver blending.
    fn apply_tint(fb: &mut FrameBuffer, region: Rect, tint: Color) {
        if tint.is_transparent() {
            return;
        }

        let x0 = (region.x.max(0.0) as u32).min(fb.width);
        let y0 = (region.y.max(0.0) as u32).min(fb.height);
        let x1 = (region.right().ceil() as u32).min(fb.width);
        let y1 = (region.bottom().ceil() as u32).min(fb.height);

        let pm = tint.premultiply();
        for y in y0..y1 {
            for x in x0..x1 {
                let dst = fb.get_pixel(x, y);
                let result = blend::blend_src_over(dst, pm);
                fb.set_pixel(x, y, result);
            }
        }
    }
}

impl Effect for BackdropBlur {
    fn render(&self, fb: &mut FrameBuffer, region: Rect, params: &EffectParams) {
        // Default tint: white with low alpha
        let tint = Color::new(255, 255, 255, 40);
        BackdropBlur::render_with_tint(fb, region, params, tint);
    }

    fn estimated_cost_ms(&self, region: Rect) -> f64 {
        let area = (region.width * region.height) as f64;
        (area / (1920.0 * 1080.0)) * 4.0
    }
}

/// Box shadow effect.
///
/// Generates a rounded-rect alpha mask, blurs it, multiplies by the shadow
/// colour, and composites behind the surface with SrcOver.
pub struct BoxShadow;

/// Parameters for rendering a box shadow.
pub struct ShadowParams {
    /// The surface rectangle that casts the shadow.
    pub surface_rect: Rect,
    /// Corner radius of the shadow shape.
    pub corner_radius: f32,
    /// Spread distance in pixels.
    pub spread: f32,
    /// Blur radius in pixels.
    pub blur_radius: u32,
    /// Horizontal offset.
    pub offset_x: f32,
    /// Vertical offset.
    pub offset_y: f32,
    /// Shadow colour.
    pub shadow_color: Color,
}

/// Pre-rendered shadow mask ready for compositing.
///
/// Generated once per unique window bounds and cached by the renderer
/// to avoid recomputing the expensive SDF + blur every frame.
pub struct ShadowMask {
    /// BGRA premultiplied shadow pixels.
    pub pixels: Vec<u8>,
    /// Top-left X in framebuffer coordinates.
    pub x0: u32,
    /// Top-left Y in framebuffer coordinates.
    pub y0: u32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl BoxShadow {
    /// Generate a shadow mask without compositing into the framebuffer.
    ///
    /// Returns `None` if the shadow has zero area (degenerate bounds).
    /// The returned mask contains pre-blurred BGRA pixels ready for
    /// [`Self::composite_shadow_mask`].
    pub fn generate_shadow_mask(
        fb_width: u32,
        fb_height: u32,
        params: &ShadowParams,
    ) -> Option<ShadowMask> {
        let ShadowParams {
            surface_rect,
            corner_radius,
            spread,
            blur_radius,
            offset_x,
            offset_y,
            shadow_color,
        } = *params;

        if blur_radius == 0 && spread <= 0.0 {
            return None;
        }

        let expand = spread + blur_radius as f32;
        let shadow_rect = Rect::new(
            surface_rect.x - expand + offset_x,
            surface_rect.y - expand + offset_y,
            surface_rect.width + expand * 2.0,
            surface_rect.height + expand * 2.0,
        );

        let mut x0 = (shadow_rect.x.max(0.0) as u32).min(fb_width);
        let mut y0 = (shadow_rect.y.max(0.0) as u32).min(fb_height);
        let mut x1 = (shadow_rect.right().ceil() as u32).min(fb_width);
        let mut y1 = (shadow_rect.bottom().ceil() as u32).min(fb_height);

        // Damage-confine the shadow mask (t82). The mask is only EVER composited
        // through the per-thread write-scissor (`composite_shadow_mask` consults
        // `scissor_allows` per pixel), so on a partial-damage frame we need not
        // compute the SDF + blur over the FULL shadow rect — only over the part
        // that can actually be written. The composited result is BYTE-IDENTICAL:
        //   * the SDF coverage is a pure per-pixel function (no neighbour reads),
        //     so any pixel computed at all is exact;
        //   * the subsequent `blur_buffer` over the mask samples ±blur_radius, so
        //     we keep a margin of `blur_radius` around the scissor — every pixel
        //     inside the scissor then sees the same neighbourhood (clamp-to-edge)
        //     as the full mask would, since the SDF outside the surface is 0 and
        //     the margin reaches past any non-zero coverage feeding a scissor pixel.
        // When no scissor is set this is a no-op (full shadow rect, as before).
        if let Some(s) = crate::rasterizer::write_scissor() {
            let m = blur_radius;
            let sx0 = (s.x.max(0.0) as u32).saturating_sub(m);
            let sy0 = (s.y.max(0.0) as u32).saturating_sub(m);
            let sx1 = (s.right().ceil().max(0.0) as u32).saturating_add(m);
            let sy1 = (s.bottom().ceil().max(0.0) as u32).saturating_add(m);
            x0 = x0.max(sx0);
            y0 = y0.max(sy0);
            x1 = x1.min(sx1);
            y1 = y1.min(sy1);
        }

        let w = x1.saturating_sub(x0);
        let h = y1.saturating_sub(y0);
        if w == 0 || h == 0 {
            return None;
        }

        let expanded_surface = Rect::new(
            surface_rect.x - spread + offset_x,
            surface_rect.y - spread + offset_y,
            surface_rect.width + spread * 2.0,
            surface_rect.height + spread * 2.0,
        );
        let r = (corner_radius + spread)
            .min(expanded_surface.width / 2.0)
            .min(expanded_surface.height / 2.0)
            .max(0.0);

        let mut mask = vec![0u8; (w * h * 4) as usize];

        for my in 0..h {
            let fy = (y0 + my) as f32 + 0.5;
            for mx in 0..w {
                let fx = (x0 + mx) as f32 + 0.5;
                let coverage = sdf_rounded_rect_coverage(fx, fy, &expanded_surface, r);
                if coverage > 0.0 {
                    let alpha = (shadow_color.a as f32 * coverage).round().clamp(0.0, 255.0) as u8;
                    let off = ((my * w + mx) * 4) as usize;
                    mask[off] = ((shadow_color.b as u16 * alpha as u16 + 127) / 255) as u8;
                    mask[off + 1] = ((shadow_color.g as u16 * alpha as u16 + 127) / 255) as u8;
                    mask[off + 2] = ((shadow_color.r as u16 * alpha as u16 + 127) / 255) as u8;
                    mask[off + 3] = alpha;
                }
            }
        }

        if blur_radius > 0 {
            blur::blur_buffer(&mut mask, w, h, blur_radius);
        }

        Some(ShadowMask {
            pixels: mask,
            x0,
            y0,
            width: w,
            height: h,
        })
    }

    /// Composite a pre-rendered shadow mask into the framebuffer.
    pub fn composite_shadow_mask(fb: &mut FrameBuffer, mask: &ShadowMask) {
        for my in 0..mask.height {
            for mx in 0..mask.width {
                let off = ((my * mask.width + mx) * 4) as usize;
                let src = Color::from_bgra_bytes([
                    mask.pixels[off],
                    mask.pixels[off + 1],
                    mask.pixels[off + 2],
                    mask.pixels[off + 3],
                ]);
                if src.a == 0 {
                    continue;
                }
                let dx = mask.x0 + mx;
                let dy = mask.y0 + my;
                // Confine to the per-thread write-scissor (t80) so a shadow does
                // not paint outside the damage rect on a partial frame.
                if !crate::rasterizer::scissor_allows(dx, dy) {
                    continue;
                }
                let dst = fb.get_pixel(dx, dy);
                let result = blend::blend_src_over(dst, src);
                fb.set_pixel(dx, dy, result);
            }
        }
    }

    /// Render a box shadow with specific parameters.
    ///
    /// Generates the shadow mask and composites it in one step.
    /// For cached rendering, use [`Self::generate_shadow_mask`] and
    /// [`Self::composite_shadow_mask`] separately.
    pub fn render_shadow(fb: &mut FrameBuffer, params: &ShadowParams) {
        if let Some(mask) = Self::generate_shadow_mask(fb.width, fb.height, params) {
            Self::composite_shadow_mask(fb, &mask);
        }
    }
}

impl Effect for BoxShadow {
    fn render(&self, fb: &mut FrameBuffer, region: Rect, params: &EffectParams) {
        let shadow_color = Color::new(0, 0, 0, 80);
        BoxShadow::render_shadow(
            fb,
            &ShadowParams {
                surface_rect: region,
                corner_radius: 8.0,
                spread: params.shadow_spread as f32,
                blur_radius: params.shadow_blur_radius,
                offset_x: 0.0,
                offset_y: 4.0,
                shadow_color,
            },
        );
    }

    fn estimated_cost_ms(&self, region: Rect) -> f64 {
        let area = (region.width * region.height) as f64;
        (area / (1920.0 * 1080.0)) * 1.0
    }
}

/// Inner glow effect.
///
/// Draws a soft inset border using a signed distance field from the rounded
/// rect edges. Uses Screen blend mode for a light highlight effect.
pub struct InnerGlow;

impl InnerGlow {
    /// Render an inner glow with specific parameters.
    pub fn render_glow(
        fb: &mut FrameBuffer,
        region: Rect,
        corner_radius: f32,
        glow_width: f32,
        glow_color: Color,
    ) {
        if glow_width <= 0.0 || glow_color.is_transparent() {
            return;
        }

        let x0 = (region.x.max(0.0) as u32).min(fb.width);
        let y0 = (region.y.max(0.0) as u32).min(fb.height);
        let x1 = (region.right().ceil() as u32).min(fb.width);
        let y1 = (region.bottom().ceil() as u32).min(fb.height);

        let r = corner_radius
            .min(region.width / 2.0)
            .min(region.height / 2.0)
            .max(0.0);

        for y in y0..y1 {
            let fy = y as f32 + 0.5;
            for x in x0..x1 {
                let fx = x as f32 + 0.5;

                // Compute signed distance to the rounded rect boundary
                // Negative = inside, positive = outside
                let outer_coverage = sdf_rounded_rect_coverage(fx, fy, &region, r);
                if outer_coverage <= 0.0 {
                    continue; // Outside the shape
                }

                // Compute distance from the edge inward
                // The SDF gives us approximate distance via the coverage gradient
                let dist_from_edge = sdf_rounded_rect_distance(fx, fy, &region, r);

                // Only glow near the edge (within glow_width pixels inside)
                if dist_from_edge > glow_width || dist_from_edge < 0.0 {
                    continue;
                }

                // Fade from full glow at edge to zero at glow_width
                let t = 1.0 - dist_from_edge / glow_width;
                let alpha = (glow_color.a as f32 * t * outer_coverage)
                    .round()
                    .clamp(0.0, 255.0) as u8;

                if alpha == 0 {
                    continue;
                }

                let src = Color::new(
                    ((glow_color.r as u16 * alpha as u16 + 127) / 255) as u8,
                    ((glow_color.g as u16 * alpha as u16 + 127) / 255) as u8,
                    ((glow_color.b as u16 * alpha as u16 + 127) / 255) as u8,
                    alpha,
                );

                let dst = fb.get_pixel(x, y);
                let result = blend::blend_screen(dst, src);
                fb.set_pixel(x, y, result);
            }
        }
    }
}

impl Effect for InnerGlow {
    fn render(&self, fb: &mut FrameBuffer, region: Rect, params: &EffectParams) {
        let glow_color = Color::new(255, 255, 255, 60);
        InnerGlow::render_glow(fb, region, 8.0, params.inner_glow_width, glow_color);
    }

    fn estimated_cost_ms(&self, region: Rect) -> f64 {
        let perimeter = 2.0 * (region.width + region.height) as f64;
        perimeter * 0.001
    }
}

/// Compute pixel coverage for a rounded rectangle using SDF. Returns 0.0–1.0.
fn sdf_rounded_rect_coverage(fx: f32, fy: f32, rect: &Rect, radius: f32) -> f32 {
    let d = sdf_rounded_rect(fx, fy, rect, radius);
    // d < 0 means inside, d > 0 means outside
    // Anti-alias over 1 pixel
    (-d + 0.5).clamp(0.0, 1.0)
}

/// Compute the minimum distance from a point inside the rounded rect to its edge.
/// Returns 0.0 at the edge, positive towards the interior.
fn sdf_rounded_rect_distance(fx: f32, fy: f32, rect: &Rect, radius: f32) -> f32 {
    let d = sdf_rounded_rect(fx, fy, rect, radius);
    (-d).max(0.0) // d is negative inside, so -d is positive distance inward from edge
}

/// Signed distance from a point to the boundary of a rounded rectangle.
/// Negative = inside, positive = outside.
fn sdf_rounded_rect(fx: f32, fy: f32, rect: &Rect, radius: f32) -> f32 {
    // Transform to rect-local coordinates centered at the rect center
    let cx = rect.x + rect.width * 0.5;
    let cy = rect.y + rect.height * 0.5;
    let hx = rect.width * 0.5;
    let hy = rect.height * 0.5;

    // Absolute position relative to rect center (exploit symmetry)
    let px = (fx - cx).abs();
    let py = (fy - cy).abs();

    // Distance to inner rounded rect
    let qx = px - (hx - radius);
    let qy = py - (hy - radius);

    let outside = (qx.max(0.0) * qx.max(0.0) + qy.max(0.0) * qy.max(0.0)).sqrt();
    let inside = qx.max(qy).min(0.0);

    outside + inside - radius
}
