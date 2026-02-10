//! Main renderer trait and software renderer implementation.

use std::collections::HashMap;

use liquide_compositor::damage::{DamageSet, DamageTile};
use liquide_compositor::effects::EffectParams;
use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::{BlendMode, Color};
use liquide_compositor::scene::{FlatNode, NodeId, SceneNodeKind};

use crate::blur;
use crate::color::SrgbLut;
use crate::effects::{BackdropBlur, BoxShadow, ShadowParams};
use crate::glyph::GlyphAtlas;
use crate::rasterizer::{self, Fill};

/// The renderer trait: processes a flattened scene into a frame buffer.
pub trait Renderer {
    /// Render the visible scene nodes into the frame buffer.
    ///
    /// Only tiles listed in `damage` need re-rendering. Returns
    /// per-tile damage classifications for the encoder.
    fn render(
        &mut self,
        nodes: &[FlatNode],
        fb: &mut FrameBuffer,
        damage: &DamageSet,
    ) -> crate::Result<Vec<DamageTile>>;
}

/// The software (CPU) renderer.
pub struct SoftwareRenderer {
    srgb_lut: SrgbLut,
    glyph_atlas: GlyphAtlas,
    /// Cached blurred bitmaps keyed by node ID.
    blur_cache: HashMap<NodeId, Vec<u8>>,
    /// Effect params derived from current degradation level.
    effect_params: EffectParams,
}

impl SoftwareRenderer {
    /// Create a new software renderer with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            srgb_lut: SrgbLut::new(),
            glyph_atlas: GlyphAtlas::new(1024, 1024),
            blur_cache: HashMap::new(),
            effect_params: EffectParams::for_profile(
                liquide_compositor::effects::QualityProfile::Balanced,
            ),
        }
    }

    /// Access the glyph atlas.
    #[must_use]
    pub fn glyph_atlas(&self) -> &GlyphAtlas {
        &self.glyph_atlas
    }

    /// Mutable access to the glyph atlas.
    pub fn glyph_atlas_mut(&mut self) -> &mut GlyphAtlas {
        &mut self.glyph_atlas
    }

    /// Access the sRGB LUT.
    #[must_use]
    pub fn srgb_lut(&self) -> &SrgbLut {
        &self.srgb_lut
    }

    /// Update the effect parameters (e.g. after degradation level changes).
    pub fn set_effect_params(&mut self, params: EffectParams) {
        self.effect_params = params;
    }

    /// Invalidate blur cache entries that are no longer in the scene.
    pub fn invalidate_blur_cache(&mut self, active_ids: &[NodeId]) {
        self.blur_cache.retain(|id, _| active_ids.contains(id));
    }

    /// Clear the entire blur cache.
    pub fn clear_blur_cache(&mut self) {
        self.blur_cache.clear();
    }
}

impl Default for SoftwareRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer for SoftwareRenderer {
    fn render(
        &mut self,
        nodes: &[FlatNode],
        fb: &mut FrameBuffer,
        damage: &DamageSet,
    ) -> crate::Result<Vec<DamageTile>> {
        let tile_size = damage.tile_size;
        let classified_tiles: Vec<DamageTile> = damage.tiles.clone();

        // For each damaged tile, determine which nodes overlap it and render them.
        for tile in &damage.tiles {
            let tile_rect = Rect::new(
                (tile.x * tile_size) as f32,
                (tile.y * tile_size) as f32,
                tile_size as f32,
                tile_size as f32,
            );

            // Render each node that intersects this tile, in z-order
            for node in nodes {
                if !node.absolute_bounds.intersects(&tile_rect) {
                    continue;
                }

                // Apply clip if present
                if let Some(clip) = &node.clip {
                    if !clip.intersects(&tile_rect) {
                        continue;
                    }
                }

                self.render_node(node, fb);
            }
        }

        Ok(classified_tiles)
    }
}

impl SoftwareRenderer {
    /// Render a single flattened node into the frame buffer.
    fn render_node(&mut self, node: &FlatNode, fb: &mut FrameBuffer) {
        let bounds = node.absolute_bounds;
        let opacity = node.opacity;

        match &node.kind {
            SceneNodeKind::Background { color } => {
                let mut c = *color;
                if opacity < 1.0 {
                    c.a = (c.a as f32 * opacity + 0.5) as u8;
                }
                rasterizer::fill_rect(fb, bounds, c, BlendMode::Src);
            }

            SceneNodeKind::Surface { buffer, .. } | SceneNodeKind::ChildSurface { buffer, .. } => {
                if let Some(buf) = buffer {
                    if opacity >= 1.0 && buf.format == liquide_compositor::pixel::PixelFormat::Bgra8 {
                        rasterizer::blit_opaque(
                            fb,
                            &buf.pixels,
                            buf.width,
                            buf.height,
                            bounds.x.max(0.0) as u32,
                            bounds.y.max(0.0) as u32,
                        );
                    } else {
                        rasterizer::blit_alpha(
                            fb,
                            &buf.pixels,
                            buf.width,
                            buf.height,
                            bounds.x.max(0.0) as u32,
                            bounds.y.max(0.0) as u32,
                            opacity,
                        );
                    }
                }
            }

            SceneNodeKind::Glass(params) => {
                let mut tint = params.tint_color;
                tint.a = (tint.a as f32 * opacity + 0.5) as u8;
                rasterizer::fill_rect(fb, bounds, tint, BlendMode::SrcOver);
            }

            SceneNodeKind::Tint { color } => {
                let mut c = *color;
                c.a = (c.a as f32 * opacity + 0.5) as u8;
                rasterizer::fill_rect(fb, bounds, c, BlendMode::Multiply);
            }

            SceneNodeKind::Shadow { spread, blur_radius, color } => {
                BoxShadow::render_shadow(
                    fb,
                    &ShadowParams {
                        surface_rect: bounds,
                        corner_radius: 0.0,
                        spread: *spread,
                        blur_radius: *blur_radius as u32,
                        offset_x: 0.0,
                        offset_y: 0.0,
                        shadow_color: Color::new(color.r, color.g, color.b, (color.a as f32 * opacity + 0.5) as u8),
                    },
                );
            }

            SceneNodeKind::Decoration {
                background,
                border_color,
                border_width,
                corner_radius,
                ..
            } => {
                // Title bar background as a rounded rect (top corners only)
                let mut bg = *background;
                if opacity < 1.0 {
                    bg.a = (bg.a as f32 * opacity + 0.5) as u8;
                }
                rasterizer::fill_rounded_rect(
                    fb,
                    bounds,
                    *corner_radius,
                    &Fill::Solid(bg),
                    BlendMode::SrcOver,
                    &self.srgb_lut,
                );

                // Border stroke around the window bounds
                if *border_width > 0.0 {
                    let mut bc = *border_color;
                    if opacity < 1.0 {
                        bc.a = (bc.a as f32 * opacity + 0.5) as u8;
                    }
                    rasterizer::stroke_rounded_rect(
                        fb,
                        bounds,
                        *corner_radius,
                        *border_width,
                        bc,
                        BlendMode::SrcOver,
                        &self.srgb_lut,
                    );
                }
            }

            SceneNodeKind::BlurBackdrop => {
                // Apply backdrop blur to the region behind a glass surface
                let params = self.effect_params.clone();
                BackdropBlur::render_with_tint(
                    fb,
                    bounds,
                    &params,
                    Color::TRANSPARENT,
                );
            }

            SceneNodeKind::BlurCache => {
                // Extract region, blur it, cache by node ID
                let radius = self.effect_params.blur_radius;
                if radius > 0 {
                    if let std::collections::hash_map::Entry::Vacant(e) = self.blur_cache.entry(node.id) {
                        // Extract and blur
                        let x0 = (bounds.x.max(0.0) as u32).min(fb.width);
                        let y0 = (bounds.y.max(0.0) as u32).min(fb.height);
                        let x1 = (bounds.right().ceil() as u32).min(fb.width);
                        let y1 = (bounds.bottom().ceil() as u32).min(fb.height);
                        let w = x1.saturating_sub(x0);
                        let h = y1.saturating_sub(y0);
                        if w > 0 && h > 0 {
                            let mut buf = vec![0u8; (w * h * 4) as usize];
                            for row in 0..h {
                                let src_off = fb.pixel_offset(x0, y0 + row);
                                let dst_off = (row * w * 4) as usize;
                                let bytes = (w * 4) as usize;
                                buf[dst_off..dst_off + bytes]
                                    .copy_from_slice(&fb.pixels[src_off..src_off + bytes]);
                            }
                            blur::blur_buffer(&mut buf, w, h, radius);
                            e.insert(buf);
                        }
                    }
                    // Blit cached blur back
                    if let Some(cached) = self.blur_cache.get(&node.id) {
                        let x0 = (bounds.x.max(0.0) as u32).min(fb.width);
                        let y0 = (bounds.y.max(0.0) as u32).min(fb.height);
                        let x1 = (bounds.right().ceil() as u32).min(fb.width);
                        let y1 = (bounds.bottom().ceil() as u32).min(fb.height);
                        let w = x1.saturating_sub(x0);
                        let h = y1.saturating_sub(y0);
                        for row in 0..h {
                            let src_off = (row * w * 4) as usize;
                            let dst_off = fb.pixel_offset(x0, y0 + row);
                            let bytes = (w * 4) as usize;
                            if src_off + bytes <= cached.len() {
                                fb.pixels[dst_off..dst_off + bytes]
                                    .copy_from_slice(&cached[src_off..src_off + bytes]);
                            }
                        }
                    }
                }
            }

            SceneNodeKind::Content | SceneNodeKind::Overlay | SceneNodeKind::ShellLayer => {
                // These are container-like nodes: their content is rendered
                // via child Surface/ChildSurface nodes already flattened.
                // The node itself draws a transparent overlay if opacity < 1.
                if opacity < 1.0 {
                    let tint = Color::new(0, 0, 0, 0);
                    rasterizer::fill_rect(fb, bounds, tint, BlendMode::SrcOver);
                }
            }

            SceneNodeKind::Cursor => {
                // Cursor is typically rendered on a hardware plane or separate
                // overlay. Here we draw a simple white arrow indicator.
                let cursor_color = Color::WHITE;
                rasterizer::fill_rect(
                    fb,
                    Rect::new(bounds.x, bounds.y, 2.0, 16.0),
                    cursor_color,
                    BlendMode::SrcOver,
                );
                rasterizer::fill_rect(
                    fb,
                    Rect::new(bounds.x, bounds.y, 12.0, 2.0),
                    cursor_color,
                    BlendMode::SrcOver,
                );
            }

            SceneNodeKind::LockScreen => {
                // Full-screen dark overlay with backdrop blur
                let params = self.effect_params.clone();
                BackdropBlur::render_with_tint(
                    fb,
                    bounds,
                    &params,
                    Color::new(0, 0, 0, 180),
                );
            }

            SceneNodeKind::CrashScreen => {
                // Full-screen red tint overlay
                let crash_color = Color::new(180, 0, 0, 200);
                rasterizer::fill_rect(fb, bounds, crash_color, BlendMode::SrcOver);
            }

            // Root and Workspace are structural, not visual
            SceneNodeKind::Root | SceneNodeKind::Workspace { .. } => {}
        }
    }
}
