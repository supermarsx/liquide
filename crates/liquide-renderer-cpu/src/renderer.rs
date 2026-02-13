//! Main renderer trait and software renderer implementation.

use liquide_compositor::damage::{DamageSet, DamageTile};
use liquide_compositor::effects::EffectParams;
use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::{BlendMode, Color};
use liquide_compositor::scene::{FlatNode, NodeId, SceneNodeKind};

use crate::blur_worker::BlurWorker;
use crate::color::SrgbLut;
use crate::effects::{BoxShadow, ShadowParams};
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
    /// Effect params derived from current degradation level.
    effect_params: EffectParams,
    /// Whether real Gaussian blur is enabled for Glass nodes.
    /// When `false`, Glass falls back to a tinted fill (much faster).
    blur_enabled: bool,
    /// Exponential moving average of recent frame render times (ms).
    /// Used to adaptively disable blur when performance is poor.
    avg_render_ms: f64,
    /// Frame render time threshold (ms) above which blur is auto-disabled.
    blur_budget_ms: f64,
    /// Background thread for async Gaussian blur computation.
    blur_worker: BlurWorker,
}

impl SoftwareRenderer {
    /// Create a new software renderer with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            srgb_lut: SrgbLut::new(),
            glyph_atlas: GlyphAtlas::new(1024, 1024),
            effect_params: EffectParams::for_profile(
                liquide_compositor::effects::QualityProfile::Balanced,
            ),
            blur_enabled: true,
            avg_render_ms: 0.0,
            blur_budget_ms: 16.0, // Target ~60fps render budget
            blur_worker: BlurWorker::new(),
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
        self.blur_worker.retain_nodes(active_ids);
    }

    /// Clear the entire blur cache.
    pub fn clear_blur_cache(&mut self) {
        self.blur_worker.clear_cache();
    }

    /// Whether real Gaussian blur is currently active.
    #[must_use]
    pub fn blur_enabled(&self) -> bool {
        self.blur_enabled
    }

    /// Manually enable or disable Gaussian blur for Glass nodes.
    pub fn set_blur_enabled(&mut self, enabled: bool) {
        self.blur_enabled = enabled;
        if !enabled {
            self.blur_worker.clear_cache();
        }
    }

    /// Set the per-frame render budget (in ms).  When the exponential average
    /// frame render time exceeds this threshold, blur is auto-disabled.
    /// When it drops below half the threshold, blur is re-enabled.
    pub fn set_blur_budget_ms(&mut self, budget: f64) {
        self.blur_budget_ms = budget;
    }

    /// Report the most recent frame's render time so the renderer can
    /// adaptively toggle blur.  Call this after each `render()`.
    pub fn report_render_time(&mut self, render_ms: f64) {
        // Exponential moving average with α = 0.3 (responds within ~3 frames).
        const ALPHA: f64 = 0.3;
        if self.avg_render_ms <= 0.0 {
            self.avg_render_ms = render_ms;
        } else {
            self.avg_render_ms = ALPHA * render_ms + (1.0 - ALPHA) * self.avg_render_ms;
        }

        // Auto-disable blur when average render time exceeds budget.
        if self.blur_enabled && self.avg_render_ms > self.blur_budget_ms {
            self.blur_enabled = false;
            self.blur_worker.clear_cache();
        }
        // Re-enable when average drops to half the budget (hysteresis).
        if !self.blur_enabled && self.avg_render_ms < self.blur_budget_ms * 0.5 {
            self.blur_enabled = true;
        }
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
        // Drain any completed async blur results before rendering.
        self.blur_worker.poll_results();

        let classified_tiles: Vec<DamageTile> = damage.tiles.clone();

        // Render each node exactly once in z-order.
        // render_node writes directly to the full framebuffer, so there is
        // no benefit from per-tile iteration — it would only cause each
        // node to be rendered redundantly for every tile it overlaps.
        for node in nodes {
            self.render_node(node, fb);
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
                    if opacity >= 1.0 && buf.format == liquide_compositor::pixel::PixelFormat::Bgra8
                    {
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
                // Glass effect: blurred backdrop + tint overlay + optional glow.
                // Blur is computed asynchronously by the blur worker thread.
                // On the first frame (no cached result yet) we fall through
                // to tint-only.  Subsequent frames use the worker's result
                // (at most one frame old).
                if self.blur_enabled {
                    let radius = params.blur_radius.min(30);
                    if radius > 0 {
                        let x0 = (bounds.x.max(0.0) as u32).min(fb.width);
                        let y0 = (bounds.y.max(0.0) as u32).min(fb.height);
                        let x1 = (bounds.right().ceil() as u32).min(fb.width);
                        let y1 = (bounds.bottom().ceil() as u32).min(fb.height);
                        let w = x1.saturating_sub(x0);
                        let h = y1.saturating_sub(y0);

                        if w > 0 && h > 0 {
                            // Try the async cache first.
                            if let Some(cached) = self.blur_worker.get_cached(node.id, w, h) {
                                // Blit the pre-blurred pixels into the framebuffer.
                                for row in 0..h {
                                    let src_off = (row * w * 4) as usize;
                                    let dst_off = fb.pixel_offset(x0, y0 + row);
                                    let bytes = (w * 4) as usize;
                                    if src_off + bytes <= cached.pixels.len() {
                                        fb.pixels[dst_off..dst_off + bytes]
                                            .copy_from_slice(&cached.pixels[src_off..src_off + bytes]);
                                    }
                                }
                            }

                            // Always submit a fresh blur request so the cache
                            // stays current as the backdrop changes.
                            let mut snapshot = vec![0u8; (w * h * 4) as usize];
                            for row in 0..h {
                                let src_off = fb.pixel_offset(x0, y0 + row);
                                let dst_off = (row * w * 4) as usize;
                                let bytes = (w * 4) as usize;
                                snapshot[dst_off..dst_off + bytes]
                                    .copy_from_slice(&fb.pixels[src_off..src_off + bytes]);
                            }
                            self.blur_worker.request_blur(
                                node.id, snapshot, w, h, radius,
                            );
                        }
                    }
                }

                // Apply tint
                let mut tint = params.tint_color;
                tint.a = (tint.a as f32 * opacity + 0.5) as u8;
                rasterizer::fill_rect(fb, bounds, tint, BlendMode::SrcOver);

                // Inner glow
                if params.inner_glow {
                    crate::effects::InnerGlow::render_glow(
                        fb,
                        bounds,
                        8.0,
                        3.0,
                        Color::new(255, 255, 255, 30),
                    );
                }
            }

            SceneNodeKind::Tint { color } => {
                let mut c = *color;
                c.a = (c.a as f32 * opacity + 0.5) as u8;
                rasterizer::fill_rect(fb, bounds, c, BlendMode::Multiply);
            }

            SceneNodeKind::Shadow {
                spread,
                blur_radius,
                color,
            } => {
                BoxShadow::render_shadow(
                    fb,
                    &ShadowParams {
                        surface_rect: bounds,
                        corner_radius: 0.0,
                        spread: *spread,
                        blur_radius: *blur_radius as u32,
                        offset_x: 0.0,
                        offset_y: 0.0,
                        shadow_color: Color::new(
                            color.r,
                            color.g,
                            color.b,
                            (color.a as f32 * opacity + 0.5) as u8,
                        ),
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
                // Backdrop blur — offloaded to the async blur worker.
                if self.blur_enabled {
                    let radius = self.effect_params.blur_radius;
                    if radius > 0 {
                        let x0 = (bounds.x.max(0.0) as u32).min(fb.width);
                        let y0 = (bounds.y.max(0.0) as u32).min(fb.height);
                        let x1 = (bounds.right().ceil() as u32).min(fb.width);
                        let y1 = (bounds.bottom().ceil() as u32).min(fb.height);
                        let w = x1.saturating_sub(x0);
                        let h = y1.saturating_sub(y0);

                        if w > 0 && h > 0 {
                            if let Some(cached) = self.blur_worker.get_cached(node.id, w, h) {
                                for row in 0..h {
                                    let src_off = (row * w * 4) as usize;
                                    let dst_off = fb.pixel_offset(x0, y0 + row);
                                    let bytes = (w * 4) as usize;
                                    if src_off + bytes <= cached.pixels.len() {
                                        fb.pixels[dst_off..dst_off + bytes]
                                            .copy_from_slice(&cached.pixels[src_off..src_off + bytes]);
                                    }
                                }
                            }

                            let mut snapshot = vec![0u8; (w * h * 4) as usize];
                            for row in 0..h {
                                let src_off = fb.pixel_offset(x0, y0 + row);
                                let dst_off = (row * w * 4) as usize;
                                let bytes = (w * 4) as usize;
                                snapshot[dst_off..dst_off + bytes]
                                    .copy_from_slice(&fb.pixels[src_off..src_off + bytes]);
                            }
                            self.blur_worker.request_blur(node.id, snapshot, w, h, radius);
                        }
                    }
                }
            }

            SceneNodeKind::BlurCache => {
                // Cached blur region — offloaded to the async blur worker.
                if self.blur_enabled {
                    let radius = self.effect_params.blur_radius;
                    if radius > 0 {
                        let x0 = (bounds.x.max(0.0) as u32).min(fb.width);
                        let y0 = (bounds.y.max(0.0) as u32).min(fb.height);
                        let x1 = (bounds.right().ceil() as u32).min(fb.width);
                        let y1 = (bounds.bottom().ceil() as u32).min(fb.height);
                        let w = x1.saturating_sub(x0);
                        let h = y1.saturating_sub(y0);

                        if w > 0 && h > 0 {
                            if let Some(cached) = self.blur_worker.get_cached(node.id, w, h) {
                                for row in 0..h {
                                    let src_off = (row * w * 4) as usize;
                                    let dst_off = fb.pixel_offset(x0, y0 + row);
                                    let bytes = (w * 4) as usize;
                                    if src_off + bytes <= cached.pixels.len() {
                                        fb.pixels[dst_off..dst_off + bytes]
                                            .copy_from_slice(&cached.pixels[src_off..src_off + bytes]);
                                    }
                                }
                            }

                            let mut snapshot = vec![0u8; (w * h * 4) as usize];
                            for row in 0..h {
                                let src_off = fb.pixel_offset(x0, y0 + row);
                                let dst_off = (row * w * 4) as usize;
                                let bytes = (w * 4) as usize;
                                snapshot[dst_off..dst_off + bytes]
                                    .copy_from_slice(&fb.pixels[src_off..src_off + bytes]);
                            }
                            self.blur_worker.request_blur(node.id, snapshot, w, h, radius);
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
                // Software cursor: white arrow with black outline for
                // visibility on any background.  Scale factor derived
                // from the node bounds (default 24px → scale ~1.5).
                let cx = bounds.x;
                let cy = bounds.y;
                let s = (bounds.width / 16.0).max(1.0); // scale relative to 16px base

                let outline = Color::new(0, 0, 0, 255);
                let fill = Color::WHITE;

                // Arrow body rows (base 16px design): (y_offset, width)
                let arrow_rows: &[(f32, f32)] = &[
                    (0.0, 1.0),
                    (1.0, 2.0),
                    (2.0, 3.0),
                    (3.0, 4.0),
                    (4.0, 5.0),
                    (5.0, 6.0),
                    (6.0, 7.0),
                    (7.0, 8.0),
                    (8.0, 9.0),
                    (9.0, 10.0),
                    (10.0, 11.0),
                    (11.0, 12.0),
                    (12.0, 7.0),
                    (13.0, 5.0),
                ];

                // Outline: black border around the arrow
                for &(row_y, row_w) in arrow_rows {
                    rasterizer::fill_rect(
                        fb,
                        Rect::new(cx - s, cy + row_y * s - 0.5 * s, row_w * s + 2.0 * s, 2.0 * s),
                        outline,
                        BlendMode::SrcOver,
                    );
                }

                // Fill: white interior
                for &(row_y, row_w) in arrow_rows {
                    rasterizer::fill_rect(
                        fb,
                        Rect::new(cx, cy + row_y * s, row_w * s, s),
                        fill,
                        BlendMode::SrcOver,
                    );
                }
            }

            SceneNodeKind::LockScreen => {
                // Full-screen dark overlay with backdrop blur (async).
                if self.blur_enabled {
                    let radius = self.effect_params.blur_radius;
                    if radius > 0 {
                        let x0 = (bounds.x.max(0.0) as u32).min(fb.width);
                        let y0 = (bounds.y.max(0.0) as u32).min(fb.height);
                        let x1 = (bounds.right().ceil() as u32).min(fb.width);
                        let y1 = (bounds.bottom().ceil() as u32).min(fb.height);
                        let w = x1.saturating_sub(x0);
                        let h = y1.saturating_sub(y0);

                        if w > 0 && h > 0 {
                            if let Some(cached) = self.blur_worker.get_cached(node.id, w, h) {
                                for row in 0..h {
                                    let src_off = (row * w * 4) as usize;
                                    let dst_off = fb.pixel_offset(x0, y0 + row);
                                    let bytes = (w * 4) as usize;
                                    if src_off + bytes <= cached.pixels.len() {
                                        fb.pixels[dst_off..dst_off + bytes]
                                            .copy_from_slice(&cached.pixels[src_off..src_off + bytes]);
                                    }
                                }
                            }

                            let mut snapshot = vec![0u8; (w * h * 4) as usize];
                            for row in 0..h {
                                let src_off = fb.pixel_offset(x0, y0 + row);
                                let dst_off = (row * w * 4) as usize;
                                let bytes = (w * 4) as usize;
                                snapshot[dst_off..dst_off + bytes]
                                    .copy_from_slice(&fb.pixels[src_off..src_off + bytes]);
                            }
                            self.blur_worker.request_blur(node.id, snapshot, w, h, radius);
                        }
                    }
                }
                // Always apply the dark overlay tint.
                rasterizer::fill_rect(fb, bounds, Color::new(0, 0, 0, 180), BlendMode::SrcOver);
            }

            SceneNodeKind::CrashScreen => {
                // Full-screen red tint overlay
                let crash_color = Color::new(180, 0, 0, 200);
                rasterizer::fill_rect(fb, bounds, crash_color, BlendMode::SrcOver);
            }

            // Root and Workspace are structural, not visual
            SceneNodeKind::Root | SceneNodeKind::Workspace { .. } => {}

            SceneNodeKind::Text { text, color, scale } => {
                let mut c = *color;
                if opacity < 1.0 {
                    c.a = (c.a as f32 * opacity + 0.5) as u8;
                }
                crate::bitmap_font::draw_text(
                    fb,
                    text,
                    bounds.x as i32,
                    bounds.y as i32,
                    c,
                    *scale,
                );
            }

            SceneNodeKind::Icon { icon_id, color } => {
                let mut c = *color;
                if opacity < 1.0 {
                    c.a = (c.a as f32 * opacity + 0.5) as u8;
                }
                crate::icons::draw_icon(fb, *icon_id, bounds, c);
            }
        }
    }
}
