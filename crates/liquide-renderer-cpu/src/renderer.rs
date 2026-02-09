//! Main renderer trait and software renderer implementation.

use liquide_compositor::damage::{DamageSet, DamageTile};
use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::BlendMode;
use liquide_compositor::scene::{FlatNode, SceneNodeKind};

use crate::color::SrgbLut;
use crate::effects::{BackdropBlur, BoxShadow, InnerGlow};
use crate::glyph::GlyphAtlas;
use crate::rasterizer;

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
    _backdrop_blur: BackdropBlur,
    _box_shadow: BoxShadow,
    _inner_glow: InnerGlow,
}

impl SoftwareRenderer {
    /// Create a new software renderer with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            srgb_lut: SrgbLut::new(),
            glyph_atlas: GlyphAtlas::new(1024, 1024),
            _backdrop_blur: BackdropBlur,
            _box_shadow: BoxShadow,
            _inner_glow: InnerGlow,
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
        match &node.kind {
            SceneNodeKind::Background { color } => {
                let mut c = *color;
                if node.opacity < 1.0 {
                    c.a = (c.a as f32 * node.opacity + 0.5) as u8;
                }
                rasterizer::fill_rect(fb, node.absolute_bounds, c, BlendMode::Src);
            }

            SceneNodeKind::Surface { buffer, .. } | SceneNodeKind::ChildSurface { buffer, .. } => {
                if let Some(buf) = buffer {
                    if node.opacity >= 1.0 && buf.format == liquide_compositor::pixel::PixelFormat::Bgra8 {
                        rasterizer::blit_opaque(
                            fb,
                            &buf.pixels,
                            buf.width,
                            buf.height,
                            node.absolute_bounds.x.max(0.0) as u32,
                            node.absolute_bounds.y.max(0.0) as u32,
                        );
                    } else {
                        rasterizer::blit_alpha(
                            fb,
                            &buf.pixels,
                            buf.width,
                            buf.height,
                            node.absolute_bounds.x.max(0.0) as u32,
                            node.absolute_bounds.y.max(0.0) as u32,
                            node.opacity,
                        );
                    }
                }
            }

            SceneNodeKind::Glass(params) => {
                // Render the tint overlay (blur is handled separately)
                let mut tint = params.tint_color;
                tint.a = (tint.a as f32 * node.opacity + 0.5) as u8;
                rasterizer::fill_rect(fb, node.absolute_bounds, tint, BlendMode::SrcOver);
            }

            SceneNodeKind::Tint { color } => {
                let mut c = *color;
                c.a = (c.a as f32 * node.opacity + 0.5) as u8;
                rasterizer::fill_rect(fb, node.absolute_bounds, c, BlendMode::Multiply);
            }

            SceneNodeKind::Shadow { color, .. } => {
                // Simplified shadow: just fill with shadow color (real implementation would blur)
                let mut c = *color;
                c.a = (c.a as f32 * node.opacity * 0.5 + 0.5) as u8;
                rasterizer::fill_rect(fb, node.absolute_bounds, c, BlendMode::SrcOver);
            }

            _ => {
                tracing::trace!(node_id = node.id, kind = ?std::mem::discriminant(&node.kind), "unimplemented node kind");
            }
        }
    }
}
