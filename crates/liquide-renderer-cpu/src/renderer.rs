//! Main renderer trait and software renderer implementation.

use std::collections::HashMap;

use liquide_compositor::damage::{DamageSet, DamageTile};
use liquide_compositor::effects::EffectParams;
use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::{BlendMode, Color};
use liquide_compositor::scene::{CursorShape, FlatNode, NodeId, SceneNodeKind};

use crate::blur_worker::BlurWorker;
use crate::color::SrgbLut;
use crate::dirty_rects::DirtyRectManager;
use crate::effects::{BoxShadow, ShadowMask, ShadowParams};
use crate::font_worker::FontWorker;
use crate::glyph::{GlyphAtlas, GlyphKey};
use crate::layout_cache::LayoutCacheManager;
use crate::lod::{LodCriteria, LodLevel, LodManager, PerformanceMode};
use crate::object_pool::ObjectPool;
use crate::rasterizer::{self, Fill};
use crate::texture_cache::TextureCache;

/// Cached shadow mask for a specific window position/size.
///
/// Avoids recomputing the expensive SDF + Gaussian blur every frame.
/// Invalidated when the source window bounds change.
struct CachedShadow {
    mask: ShadowMask,
    /// Source bounds as integer pixels for invalidation.
    bx: i32,
    by: i32,
    bw: u32,
    bh: u32,
}

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
    /// Per-node shadow mask cache — avoids recomputing expensive SDF + blur
    /// every frame. Invalidated when window bounds change.
    shadow_cache: HashMap<NodeId, CachedShadow>,
    /// Background thread for async glyph rasterization.
    font_worker: FontWorker,
    /// Layout cache manager for computed element layouts.
    layout_cache: LayoutCacheManager,
    /// Texture cache for decoded images and rendered assets.
    texture_cache: TextureCache,
    /// Dirty rectangle tracking for partial redraws.
    dirty_rects: DirtyRectManager,
    /// Level of detail manager for adaptive quality.
    lod_manager: LodManager,
    /// Object pool for temporary render buffers.
    buffer_pool: ObjectPool<Vec<u8>>,
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
            shadow_cache: HashMap::new(),
            font_worker: FontWorker::new(),
            layout_cache: LayoutCacheManager::new(),
            texture_cache: TextureCache::new(),
            dirty_rects: DirtyRectManager::new(1920, 1080),
            lod_manager: LodManager::new(1920.0, 1080.0),
            buffer_pool: ObjectPool::new(64),
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

    /// Retain only shadow cache entries for the given node IDs.
    pub fn retain_shadow_cache(&mut self, active_ids: &[NodeId]) {
        self.shadow_cache.retain(|id, _| active_ids.contains(id));
    }

    /// Clear the entire shadow cache.
    pub fn clear_shadow_cache(&mut self) {
        self.shadow_cache.clear();
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
        // Exponential moving average with α = 0.2 (responds within ~5 frames,
        // smoother than α=0.3 to prevent flip-flopping).
        const ALPHA: f64 = 0.2;
        if self.avg_render_ms <= 0.0 {
            self.avg_render_ms = render_ms;
        } else {
            self.avg_render_ms = ALPHA * render_ms + (1.0 - ALPHA) * self.avg_render_ms;
        }

        // Auto-disable blur when average render time exceeds budget.
        // Use wider hysteresis to prevent oscillation:
        //   disable when > budget (16ms)
        //   re-enable when < budget * 0.25 (4ms) — well below threshold
        if self.blur_enabled && self.avg_render_ms > self.blur_budget_ms {
            self.blur_enabled = false;
            self.blur_worker.clear_cache();
        }
        if !self.blur_enabled && self.avg_render_ms < self.blur_budget_ms * 0.25 {
            self.blur_enabled = true;
        }

        // Update LOD manager adaptive bias based on frame time
        self.lod_manager
            .update_adaptive_bias(render_ms, self.blur_budget_ms);
    }

    // --- Layout Cache Management ---

    /// Get cached layout for an element.
    #[must_use]
    pub fn get_cached_layout(&self, element_id: u32) -> Option<Rect> {
        self.layout_cache.get(element_id)
    }

    /// Cache a computed layout for an element.
    pub fn cache_layout(&mut self, element_id: u32, bounds: Rect) {
        self.layout_cache.insert(element_id, bounds);
    }

    /// Invalidate layout cache for a specific element.
    pub fn invalidate_layout(&mut self, element_id: u32) {
        self.layout_cache.invalidate(element_id);
    }

    /// Invalidate all cached layouts (e.g., on viewport resize).
    pub fn invalidate_all_layouts(&mut self) {
        self.layout_cache.invalidate_all();
    }

    /// Remove layout caches for elements no longer in the scene.
    pub fn retain_layout_cache(&mut self, active_ids: &[u32]) {
        self.layout_cache.retain(active_ids);
    }

    /// Get layout cache statistics.
    #[must_use]
    pub fn layout_cache_stats(&self) -> crate::layout_cache::LayoutCacheStats {
        self.layout_cache.stats()
    }

    // --- Texture Cache Management ---

    /// Get a cached texture by ID.
    pub fn get_cached_texture(
        &mut self,
        texture_id: &str,
    ) -> Option<crate::texture_cache::CachedTexture> {
        self.texture_cache.get(texture_id)
    }

    /// Cache a decoded texture.
    pub fn cache_texture(&mut self, texture_id: String, data: Vec<u8>, width: u32, height: u32) {
        self.texture_cache.insert(texture_id, data, width, height);
    }

    /// Remove a texture from the cache.
    pub fn remove_cached_texture(&mut self, texture_id: &str) -> bool {
        self.texture_cache.remove(texture_id)
    }

    /// Clear all cached textures.
    pub fn clear_texture_cache(&mut self) {
        self.texture_cache.clear();
    }

    /// Get texture cache statistics.
    #[must_use]
    pub fn texture_cache_stats(&self) -> crate::texture_cache::TextureCacheStats {
        self.texture_cache.stats()
    }

    // --- Dirty Rectangle Management ---

    /// Mark a screen region as dirty (needs rerendering).
    pub fn mark_dirty(&mut self, x: f32, y: f32, width: f32, height: f32) {
        self.dirty_rects.mark_dirty(x, y, width, height);
    }

    /// Mark the entire screen as dirty.
    pub fn mark_full_damage(&mut self) {
        self.dirty_rects.mark_full_damage();
    }

    /// Check if a rect intersects any dirty regions.
    #[must_use]
    pub fn intersects_dirty(&self, rect: &Rect) -> bool {
        self.dirty_rects.intersects_dirty(rect)
    }

    /// Clear dirty rectangles after rendering.
    pub fn clear_dirty_rects(&mut self) {
        self.dirty_rects.clear();
    }

    /// Update screen dimensions for dirty rect tracking.
    pub fn resize_dirty_tracking(&mut self, width: u32, height: u32) {
        self.dirty_rects.resize(width, height);
        self.lod_manager.resize(width as f32, height as f32);
        self.invalidate_all_layouts(); // Layouts need recalculation on resize
    }

    /// Get dirty rectangle statistics.
    #[must_use]
    pub fn dirty_rect_stats(&self) -> crate::dirty_rects::DirtyRectStats {
        self.dirty_rects.stats()
    }

    // --- Level of Detail Management ---

    /// Set LOD performance mode.
    pub fn set_lod_performance_mode(&mut self, mode: PerformanceMode) {
        self.lod_manager.set_performance_mode(mode);
    }

    /// Enable or disable adaptive LOD.
    pub fn set_adaptive_lod_enabled(&mut self, enabled: bool) {
        self.lod_manager.set_adaptive_enabled(enabled);
    }

    /// Select appropriate LOD level for a node.
    #[must_use]
    pub fn select_lod(&self, node: &FlatNode, viewport_center_distance: f32) -> LodLevel {
        let criteria = LodCriteria {
            screen_bounds: node.absolute_bounds,
            distance: viewport_center_distance,
            visible: node.opacity > 0.01,
            performance_mode: PerformanceMode::Balanced,
        };
        self.lod_manager.select_lod(&criteria)
    }

    /// Calculate distance from viewport center.
    #[must_use]
    pub fn calculate_distance_from_center(&self, bounds: &Rect) -> f32 {
        self.lod_manager.calculate_distance_from_center(bounds)
    }

    /// Get LOD manager statistics.
    #[must_use]
    pub fn lod_stats(&self) -> crate::lod::LodStats {
        self.lod_manager.stats()
    }

    // --- Object Pool Management ---

    /// Acquire a buffer from the pool or create a new one.
    pub fn acquire_buffer(&mut self, size: usize) -> Vec<u8> {
        self.buffer_pool
            .acquire_or_create(|| Vec::with_capacity(size))
    }

    /// Release a buffer back to the pool.
    pub fn release_buffer(&mut self, buffer: Vec<u8>) {
        self.buffer_pool.release(buffer);
    }

    /// Get buffer pool statistics.
    #[must_use]
    pub fn buffer_pool_stats(&self) -> crate::object_pool::ObjectPoolStats {
        self.buffer_pool.stats()
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

        // Drain completed glyph rasterizations into the atlas.
        let rasterized = self.font_worker.poll_results();
        for glyph in &rasterized {
            let _ = self
                .glyph_atlas
                .insert(glyph.key, &glyph.bitmap, &glyph.metrics);
        }

        let classified_tiles: Vec<DamageTile> = damage.tiles.clone();

        // Render each node exactly once in z-order.
        // Note: Dirty rect culling is disabled by default - it requires explicit
        // dirty tracking from the compositor. Enable by manually calling mark_dirty().
        for node in nodes {
            // Calculate LOD level for this node
            let distance = self.calculate_distance_from_center(&node.absolute_bounds);
            let lod_level = self.select_lod(node, distance);

            // Render node with appropriate LOD (even Minimal nodes are rendered)
            self.render_node_with_lod(node, fb, lod_level);
        }

        Ok(classified_tiles)
    }
}

impl SoftwareRenderer {
    /// Render a single flattened node into the frame buffer with LOD support.
    fn render_node_with_lod(&mut self, node: &FlatNode, fb: &mut FrameBuffer, lod_level: LodLevel) {
        let bounds = node.absolute_bounds;
        let opacity = node.opacity;

        // Apply LOD quality factor to certain effects
        let quality_factor = lod_level.quality_factor();

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
                // Apply LOD quality factor to blur radius for performance.
                if self.blur_enabled && lod_level != LodLevel::Low {
                    let radius = params.blur_radius.min(30);
                    // Reduce blur radius for lower LOD levels
                    let lod_radius = (radius as f32 * quality_factor) as u32;
                    if lod_radius > 0 {
                        self.render_backdrop_blur(node.id, bounds, lod_radius, fb);
                    }
                }

                // Apply tint
                let mut tint = params.tint_color;
                tint.a = (tint.a as f32 * opacity + 0.5) as u8;
                rasterizer::fill_rect(fb, bounds, tint, BlendMode::SrcOver);

                // Inner glow (skip for low LOD)
                if params.inner_glow && lod_level != LodLevel::Low {
                    crate::effects::InnerGlow::render_glow(
                        fb,
                        bounds,
                        8.0 * quality_factor,
                        3.0 * quality_factor,
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
                // Skip shadows for low detail levels (expensive)
                if lod_level == LodLevel::Low {
                    return;
                }

                // Cached shadow rendering: the expensive SDF + Gaussian blur
                // is only computed when the window bounds actually change.
                let bx = bounds.x as i32;
                let by = bounds.y as i32;
                let bw = bounds.width as u32;
                let bh = bounds.height as u32;

                let cache_hit = self
                    .shadow_cache
                    .get(&node.id)
                    .is_some_and(|c| c.bx == bx && c.by == by && c.bw == bw && c.bh == bh);

                if cache_hit {
                    // Fast path: composite cached shadow mask (~0.5ms vs ~20ms).
                    if let Some(cached) = self.shadow_cache.get(&node.id) {
                        BoxShadow::composite_shadow_mask(fb, &cached.mask);
                    }
                } else {
                    // Cache miss: generate shadow mask and store for reuse.
                    // Apply LOD quality factor to blur radius.
                    let shadow_color = Color::new(
                        color.r,
                        color.g,
                        color.b,
                        (color.a as f32 * opacity + 0.5) as u8,
                    );
                    let lod_blur_radius = (*blur_radius as f32 * quality_factor) as u32;
                    let params = ShadowParams {
                        surface_rect: bounds,
                        corner_radius: 0.0,
                        spread: *spread,
                        blur_radius: lod_blur_radius,
                        offset_x: 0.0,
                        offset_y: 0.0,
                        shadow_color,
                    };
                    if let Some(mask) =
                        BoxShadow::generate_shadow_mask(fb.width, fb.height, &params)
                    {
                        BoxShadow::composite_shadow_mask(fb, &mask);
                        self.shadow_cache.insert(
                            node.id,
                            CachedShadow {
                                mask,
                                bx,
                                by,
                                bw,
                                bh,
                            },
                        );
                    }
                }
            }

            SceneNodeKind::Decoration {
                title,
                title_color,
                background,
                border_color,
                border_width,
                corner_radius,
                button_state,
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

                // --- Window control buttons ---
                // Modern style: subtle rounded-rect backgrounds with
                // crisp icon glyphs (×, □, ─, 📌).
                // Layout: right-aligned in the title bar.
                let title_bar_h = 30.0_f32;
                let btn_w = 32.0_f32; // wider for better click targets
                let btn_h = 22.0_f32;
                let btn_y = bounds.y + (title_bar_h - btn_h) / 2.0;
                let btn_right_margin = 4.0_f32;

                // Close button (× icon) — rightmost
                if button_state.close {
                    let close_x = bounds.x + bounds.width - btn_w - btn_right_margin;
                    let close_bg = if button_state.close_hovered {
                        Color::new(241, 60, 70, 255) // Brighter red on hover
                    } else {
                        Color::new(232, 17, 35, 220) // Windows-red
                    };
                    let close_bounds = Rect::new(close_x, btn_y, btn_w, btn_h);
                    rasterizer::fill_rounded_rect(
                        fb,
                        close_bounds,
                        3.0,
                        &Fill::Solid(close_bg),
                        BlendMode::SrcOver,
                        &self.srgb_lut,
                    );
                    // × icon: two diagonal lines forming an X
                    let cx = close_x + btn_w / 2.0;
                    let cy_btn = btn_y + btn_h / 2.0;
                    let icon_color = Color::new(255, 255, 255, 240);
                    let arm = 4.0_f32;
                    let thickness = 1.5_f32;
                    // Top-left to bottom-right diagonal
                    for i in 0..((arm * 2.0) as i32) {
                        let t = i as f32 - arm;
                        rasterizer::fill_rect(
                            fb,
                            Rect::new(
                                cx + t - thickness / 2.0,
                                cy_btn + t - thickness / 2.0,
                                thickness,
                                thickness,
                            ),
                            icon_color,
                            BlendMode::SrcOver,
                        );
                    }
                    // Top-right to bottom-left diagonal
                    for i in 0..((arm * 2.0) as i32) {
                        let t = i as f32 - arm;
                        rasterizer::fill_rect(
                            fb,
                            Rect::new(
                                cx - t - thickness / 2.0,
                                cy_btn + t - thickness / 2.0,
                                thickness,
                                thickness,
                            ),
                            icon_color,
                            BlendMode::SrcOver,
                        );
                    }
                }

                // Maximize button (□ outline icon) — second from right
                if button_state.maximize {
                    let max_x = bounds.x + bounds.width - btn_w * 2.0 - btn_right_margin;
                    let btn_bg = if button_state.maximize_hovered {
                        Color::new(255, 255, 255, 60) // Brighter on hover
                    } else {
                        Color::new(255, 255, 255, 20)
                    };
                    let max_bounds = Rect::new(max_x, btn_y, btn_w, btn_h);
                    rasterizer::fill_rounded_rect(
                        fb,
                        max_bounds,
                        3.0,
                        &Fill::Solid(btn_bg),
                        BlendMode::SrcOver,
                        &self.srgb_lut,
                    );
                    // □ icon: open rectangle outline
                    let cx = max_x + btn_w / 2.0;
                    let cy_btn = btn_y + btn_h / 2.0;
                    let icon_color = Color::new(220, 220, 220, 240);
                    let half = 4.0_f32;
                    let stroke = 1.5_f32;
                    // Top edge
                    rasterizer::fill_rect(
                        fb,
                        Rect::new(cx - half, cy_btn - half, half * 2.0, stroke),
                        icon_color,
                        BlendMode::SrcOver,
                    );
                    // Bottom edge
                    rasterizer::fill_rect(
                        fb,
                        Rect::new(cx - half, cy_btn + half - stroke, half * 2.0, stroke),
                        icon_color,
                        BlendMode::SrcOver,
                    );
                    // Left edge
                    rasterizer::fill_rect(
                        fb,
                        Rect::new(cx - half, cy_btn - half, stroke, half * 2.0),
                        icon_color,
                        BlendMode::SrcOver,
                    );
                    // Right edge
                    rasterizer::fill_rect(
                        fb,
                        Rect::new(cx + half - stroke, cy_btn - half, stroke, half * 2.0),
                        icon_color,
                        BlendMode::SrcOver,
                    );
                }

                // Minimize button (─ horizontal line icon) — third from right
                if button_state.minimize {
                    let min_x = bounds.x + bounds.width - btn_w * 3.0 - btn_right_margin;
                    let btn_bg = if button_state.minimize_hovered {
                        Color::new(255, 255, 255, 60) // Brighter on hover
                    } else {
                        Color::new(255, 255, 255, 20)
                    };
                    let min_bounds = Rect::new(min_x, btn_y, btn_w, btn_h);
                    rasterizer::fill_rounded_rect(
                        fb,
                        min_bounds,
                        3.0,
                        &Fill::Solid(btn_bg),
                        BlendMode::SrcOver,
                        &self.srgb_lut,
                    );
                    // ─ icon: horizontal bar
                    let cx = min_x + btn_w / 2.0;
                    let cy_btn = btn_y + btn_h / 2.0;
                    let icon_color = Color::new(220, 220, 220, 240);
                    rasterizer::fill_rect(
                        fb,
                        Rect::new(cx - 5.0, cy_btn + 2.0, 10.0, 1.5),
                        icon_color,
                        BlendMode::SrcOver,
                    );
                }

                // Always-on-top button (📌 pin icon) — fourth from right
                if button_state.always_on_top {
                    let aot_x = bounds.x + bounds.width - btn_w * 4.0 - btn_right_margin;
                    let btn_bg = if button_state.is_topmost {
                        if button_state.always_on_top_hovered {
                            Color::new(80, 150, 240, 220) // Brighter blue on hover
                        } else {
                            Color::new(60, 130, 220, 180) // Blue when active
                        }
                    } else if button_state.always_on_top_hovered {
                        Color::new(255, 255, 255, 60) // Brighter on hover
                    } else {
                        Color::new(255, 255, 255, 20)
                    };
                    let aot_bounds = Rect::new(aot_x, btn_y, btn_w, btn_h);
                    rasterizer::fill_rounded_rect(
                        fb,
                        aot_bounds,
                        3.0,
                        &Fill::Solid(btn_bg),
                        BlendMode::SrcOver,
                        &self.srgb_lut,
                    );
                    // Pin icon: vertical line with a small circle head
                    let cx = aot_x + btn_w / 2.0;
                    let cy_btn = btn_y + btn_h / 2.0;
                    let icon_color = if button_state.is_topmost {
                        Color::new(255, 255, 255, 255)
                    } else {
                        Color::new(220, 220, 220, 240)
                    };
                    // Pin head (small filled circle-like square)
                    rasterizer::fill_rect(
                        fb,
                        Rect::new(cx - 3.0, cy_btn - 5.0, 6.0, 4.0),
                        icon_color,
                        BlendMode::SrcOver,
                    );
                    // Pin shaft (vertical line)
                    rasterizer::fill_rect(
                        fb,
                        Rect::new(cx - 0.75, cy_btn - 1.0, 1.5, 6.0),
                        icon_color,
                        BlendMode::SrcOver,
                    );
                    // Pin point (small triangle approximation)
                    rasterizer::fill_rect(
                        fb,
                        Rect::new(cx - 0.5, cy_btn + 5.0, 1.0, 2.0),
                        icon_color,
                        BlendMode::SrcOver,
                    );
                }

                // --- Title text (centered in title bar) ---
                if let Some(title_text) = title {
                    if !title_text.is_empty() {
                        let mut tc = *title_color;
                        if opacity < 1.0 {
                            tc.a = (tc.a as f32 * opacity + 0.5) as u8;
                        }
                        // Approximate centering: 8×16 bitmap font chars
                        let char_w = 8_i32;
                        let text_w = title_text.len() as i32 * char_w;
                        let text_x = bounds.x as i32 + (bounds.width as i32 - text_w) / 2;
                        let text_y = bounds.y as i32 + (title_bar_h as i32 - 16) / 2;
                        crate::bitmap_font::draw_text(fb, title_text, text_x, text_y, tc, 1);
                    }
                }
            }

            SceneNodeKind::BlurBackdrop => {
                // Backdrop blur — offloaded to the async blur worker.
                if self.blur_enabled {
                    let radius = self.effect_params.blur_radius;
                    if radius > 0 {
                        self.render_backdrop_blur(node.id, bounds, radius, fb);
                    }
                }
            }

            SceneNodeKind::BlurCache => {
                // Cached blur region — offloaded to the async blur worker.
                if self.blur_enabled {
                    let radius = self.effect_params.blur_radius;
                    if radius > 0 {
                        self.render_backdrop_blur(node.id, bounds, radius, fb);
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

            SceneNodeKind::Cursor { shape } => {
                // Software cursor rendered in different shapes based on context.
                let cx = bounds.x;
                let cy = bounds.y;
                let s = (bounds.width / 16.0).max(1.0);

                let outline = Color::new(0, 0, 0, 255);
                let fill = Color::WHITE;

                match shape {
                    CursorShape::Arrow => {
                        Self::draw_cursor_arrow(fb, cx, cy, s, outline, fill);
                    }
                    CursorShape::Move => {
                        Self::draw_cursor_move(fb, cx, cy, s, outline, fill);
                    }
                    CursorShape::ResizeNS => {
                        Self::draw_cursor_resize_ns(fb, cx, cy, s, outline, fill);
                    }
                    CursorShape::ResizeEW => {
                        Self::draw_cursor_resize_ew(fb, cx, cy, s, outline, fill);
                    }
                    CursorShape::ResizeNWSE => {
                        Self::draw_cursor_resize_nwse(fb, cx, cy, s, outline, fill);
                    }
                    CursorShape::ResizeNESW => {
                        Self::draw_cursor_resize_nesw(fb, cx, cy, s, outline, fill);
                    }
                    CursorShape::Pointer => {
                        Self::draw_cursor_pointer(fb, cx, cy, s, outline, fill);
                    }
                    CursorShape::Text => {
                        Self::draw_cursor_text(fb, cx, cy, s, outline, fill);
                    }
                    CursorShape::NotAllowed => {
                        Self::draw_cursor_not_allowed(fb, cx, cy, s, outline, fill);
                    }
                    CursorShape::Wait => {
                        Self::draw_cursor_wait(fb, cx, cy, s, outline, fill);
                    }
                    CursorShape::Progress => {
                        // Arrow + small hourglass
                        Self::draw_cursor_arrow(fb, cx, cy, s, outline, fill);
                        Self::draw_cursor_wait(
                            fb,
                            cx + 8.0 * s,
                            cy + 8.0 * s,
                            s * 0.6,
                            outline,
                            fill,
                        );
                    }
                    CursorShape::Help => {
                        // Arrow + question mark
                        Self::draw_cursor_arrow(fb, cx, cy, s, outline, fill);
                        Self::draw_question_mark(
                            fb,
                            cx + 10.0 * s,
                            cy + 10.0 * s,
                            s * 0.7,
                            outline,
                        );
                    }
                    CursorShape::Crosshair => {
                        Self::draw_cursor_crosshair(fb, cx, cy, s, outline);
                    }
                    CursorShape::Grab => {
                        Self::draw_cursor_hand(fb, cx, cy, s, outline, fill, false);
                    }
                    CursorShape::Grabbing => {
                        Self::draw_cursor_hand(fb, cx, cy, s, outline, fill, true);
                    }
                    CursorShape::ZoomIn => {
                        Self::draw_cursor_magnifier(fb, cx, cy, s, outline, fill, true);
                    }
                    CursorShape::ZoomOut => {
                        Self::draw_cursor_magnifier(fb, cx, cy, s, outline, fill, false);
                    }
                    CursorShape::ContextMenu => {
                        Self::draw_cursor_pointer(fb, cx, cy, s, outline, fill);
                    }
                    CursorShape::Alias => {
                        Self::draw_cursor_arrow(fb, cx, cy, s, outline, fill);
                    }
                    CursorShape::Copy => {
                        Self::draw_cursor_arrow(fb, cx, cy, s, outline, fill);
                    }
                    CursorShape::NoDrop => {
                        Self::draw_cursor_not_allowed(fb, cx, cy, s, outline, fill);
                    }
                    CursorShape::Cell => {
                        Self::draw_cursor_crosshair(fb, cx, cy, s, outline);
                    }
                    CursorShape::VerticalText => {
                        Self::draw_cursor_text_vertical(fb, cx, cy, s, outline, fill);
                    }
                    CursorShape::AllScroll => {
                        Self::draw_cursor_all_scroll(fb, cx, cy, s, outline, fill);
                    }
                    CursorShape::ExpandH => {
                        Self::draw_cursor_resize_ew(fb, cx, cy, s, outline, fill);
                    }
                    CursorShape::ExpandV => {
                        Self::draw_cursor_resize_ns(fb, cx, cy, s, outline, fill);
                    }
                }
            }

            SceneNodeKind::LockScreen => {
                // Full-screen dark overlay with backdrop blur (async).
                if self.blur_enabled {
                    let radius = self.effect_params.blur_radius;
                    if radius > 0 {
                        self.render_backdrop_blur(node.id, bounds, radius, fb);
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

                // Calculate target glyph height from scale.
                // scale=1 → 16px (base), scale=2 → 32px, etc.
                let glyph_height = 16 * scale.max(&1);

                // Try atlas-based antialiased rendering first.
                let font_id = 0_u32; // built-in bitmap font
                let size_px = glyph_height as u16;
                let mut pen_x = bounds.x;
                let pen_y = bounds.y;
                let mut all_in_atlas = true;

                // First pass: check which glyphs are in the atlas, request
                // missing ones from the font worker.
                for ch in text.chars() {
                    if ch == '\n' || ch == '\r' {
                        continue;
                    }
                    let glyph_id = ch as u32;
                    let key = GlyphKey {
                        font_id,
                        glyph_id,
                        size_px,
                        subpixel: false,
                    };
                    if self.glyph_atlas.get(&key).is_none() {
                        all_in_atlas = false;
                        self.font_worker.request_glyph(key, ch, glyph_height);
                    }
                }

                if all_in_atlas {
                    // Render using antialiased atlas glyphs.
                    for ch in text.chars() {
                        if ch == '\n' || ch == '\r' {
                            continue;
                        }
                        let key = GlyphKey {
                            font_id,
                            glyph_id: ch as u32,
                            size_px,
                            subpixel: false,
                        };
                        if let Some(cached) = self.glyph_atlas.get(&key).cloned() {
                            let pos = liquide_compositor::geometry::Point::new(
                                pen_x,
                                pen_y + glyph_height as f32,
                            );
                            self.glyph_atlas.blit_glyph(fb, &cached, pos, c);
                            pen_x += cached.advance;
                        }
                    }
                } else {
                    // Fallback: use 1-bit bitmap font while atlas is being populated.
                    crate::bitmap_font::draw_text(
                        fb,
                        text,
                        bounds.x as i32,
                        bounds.y as i32,
                        c,
                        *scale,
                    );
                }
            }

            SceneNodeKind::Icon { icon_id, color } => {
                let mut c = *color;
                if opacity < 1.0 {
                    c.a = (c.a as f32 * opacity + 0.5) as u8;
                }
                crate::icons::draw_icon(fb, *icon_id, bounds, c, &self.srgb_lut);
            }
        }
    }

    // =======================================================================
    // Cursor shape drawing helpers
    // =======================================================================

    /// Arrow cursor: classic top-left pointer.
    fn draw_cursor_arrow(
        fb: &mut FrameBuffer,
        cx: f32,
        cy: f32,
        s: f32,
        outline: Color,
        fill: Color,
    ) {
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
        for &(row_y, row_w) in arrow_rows {
            rasterizer::fill_rect(
                fb,
                Rect::new(
                    cx - s,
                    cy + row_y * s - 0.5 * s,
                    row_w * s + 2.0 * s,
                    2.0 * s,
                ),
                outline,
                BlendMode::SrcOver,
            );
        }
        for &(row_y, row_w) in arrow_rows {
            rasterizer::fill_rect(
                fb,
                Rect::new(cx, cy + row_y * s, row_w * s, s),
                fill,
                BlendMode::SrcOver,
            );
        }
    }

    /// Move cursor: four-way cross arrow (for window dragging).
    fn draw_cursor_move(
        fb: &mut FrameBuffer,
        cx: f32,
        cy: f32,
        s: f32,
        outline: Color,
        fill: Color,
    ) {
        let center_x = cx + 7.0 * s;
        let center_y = cy + 7.0 * s;
        let arm = 5.0 * s;
        let thickness = 2.0 * s;
        let half_t = thickness * 0.5;
        let arrow_w = 4.0 * s;
        let arrow_h = 3.0 * s;

        // Outline (1px bigger each side)
        let o = s;
        // Vertical arm (outline)
        rasterizer::fill_rect(
            fb,
            Rect::new(
                center_x - half_t - o,
                center_y - arm - arrow_h - o,
                thickness + 2.0 * o,
                arm * 2.0 + thickness + 2.0 * arrow_h + 2.0 * o,
            ),
            outline,
            BlendMode::SrcOver,
        );
        // Horizontal arm (outline)
        rasterizer::fill_rect(
            fb,
            Rect::new(
                center_x - arm - arrow_h - o,
                center_y - half_t - o,
                arm * 2.0 + thickness + 2.0 * arrow_h + 2.0 * o,
                thickness + 2.0 * o,
            ),
            outline,
            BlendMode::SrcOver,
        );

        // Fill: vertical arm
        rasterizer::fill_rect(
            fb,
            Rect::new(center_x - half_t, center_y - arm, thickness, arm * 2.0),
            fill,
            BlendMode::SrcOver,
        );
        // Fill: horizontal arm
        rasterizer::fill_rect(
            fb,
            Rect::new(center_x - arm, center_y - half_t, arm * 2.0, thickness),
            fill,
            BlendMode::SrcOver,
        );

        // Arrowheads (triangles made of rects)
        // Up arrow
        for i in 0..3 {
            let fi = i as f32;
            let w = (arrow_w - fi * 2.0 * s).max(s);
            rasterizer::fill_rect(
                fb,
                Rect::new(center_x - w * 0.5, center_y - arm - fi * s, w, s),
                fill,
                BlendMode::SrcOver,
            );
        }
        // Down arrow
        for i in 0..3 {
            let fi = i as f32;
            let w = (arrow_w - fi * 2.0 * s).max(s);
            rasterizer::fill_rect(
                fb,
                Rect::new(center_x - w * 0.5, center_y + arm + fi * s, w, s),
                fill,
                BlendMode::SrcOver,
            );
        }
        // Left arrow
        for i in 0..3 {
            let fi = i as f32;
            let h = (arrow_w - fi * 2.0 * s).max(s);
            rasterizer::fill_rect(
                fb,
                Rect::new(center_x - arm - fi * s, center_y - h * 0.5, s, h),
                fill,
                BlendMode::SrcOver,
            );
        }
        // Right arrow
        for i in 0..3 {
            let fi = i as f32;
            let h = (arrow_w - fi * 2.0 * s).max(s);
            rasterizer::fill_rect(
                fb,
                Rect::new(center_x + arm + fi * s, center_y - h * 0.5, s, h),
                fill,
                BlendMode::SrcOver,
            );
        }
    }

    /// Vertical resize cursor (↕): double-headed vertical arrow.
    fn draw_cursor_resize_ns(
        fb: &mut FrameBuffer,
        cx: f32,
        cy: f32,
        s: f32,
        outline: Color,
        fill: Color,
    ) {
        let center_x = cx + 6.0 * s;
        let center_y = cy + 7.0 * s;
        let arm = 5.0 * s;
        let thickness = 2.0 * s;
        let half_t = thickness * 0.5;
        let o = s;

        // Outline
        rasterizer::fill_rect(
            fb,
            Rect::new(
                center_x - half_t - o,
                center_y - arm - 3.0 * s - o,
                thickness + 2.0 * o,
                arm * 2.0 + 6.0 * s + 2.0 * o,
            ),
            outline,
            BlendMode::SrcOver,
        );
        // Fill: vertical bar
        rasterizer::fill_rect(
            fb,
            Rect::new(center_x - half_t, center_y - arm, thickness, arm * 2.0),
            fill,
            BlendMode::SrcOver,
        );
        // Up arrowhead
        for i in 0..3 {
            let fi = i as f32;
            let w = (6.0 * s - fi * 2.0 * s).max(s);
            rasterizer::fill_rect(
                fb,
                Rect::new(center_x - w * 0.5, center_y - arm - fi * s, w, s),
                fill,
                BlendMode::SrcOver,
            );
        }
        // Down arrowhead
        for i in 0..3 {
            let fi = i as f32;
            let w = (6.0 * s - fi * 2.0 * s).max(s);
            rasterizer::fill_rect(
                fb,
                Rect::new(center_x - w * 0.5, center_y + arm + fi * s, w, s),
                fill,
                BlendMode::SrcOver,
            );
        }
    }

    /// Horizontal resize cursor (↔): double-headed horizontal arrow.
    fn draw_cursor_resize_ew(
        fb: &mut FrameBuffer,
        cx: f32,
        cy: f32,
        s: f32,
        outline: Color,
        fill: Color,
    ) {
        let center_x = cx + 7.0 * s;
        let center_y = cy + 6.0 * s;
        let arm = 5.0 * s;
        let thickness = 2.0 * s;
        let half_t = thickness * 0.5;
        let o = s;

        // Outline
        rasterizer::fill_rect(
            fb,
            Rect::new(
                center_x - arm - 3.0 * s - o,
                center_y - half_t - o,
                arm * 2.0 + 6.0 * s + 2.0 * o,
                thickness + 2.0 * o,
            ),
            outline,
            BlendMode::SrcOver,
        );
        // Fill: horizontal bar
        rasterizer::fill_rect(
            fb,
            Rect::new(center_x - arm, center_y - half_t, arm * 2.0, thickness),
            fill,
            BlendMode::SrcOver,
        );
        // Left arrowhead
        for i in 0..3 {
            let fi = i as f32;
            let h = (6.0 * s - fi * 2.0 * s).max(s);
            rasterizer::fill_rect(
                fb,
                Rect::new(center_x - arm - fi * s, center_y - h * 0.5, s, h),
                fill,
                BlendMode::SrcOver,
            );
        }
        // Right arrowhead
        for i in 0..3 {
            let fi = i as f32;
            let h = (6.0 * s - fi * 2.0 * s).max(s);
            rasterizer::fill_rect(
                fb,
                Rect::new(center_x + arm + fi * s, center_y - h * 0.5, s, h),
                fill,
                BlendMode::SrcOver,
            );
        }
    }

    /// Diagonal resize cursor (↘↖): NW-SE direction.
    fn draw_cursor_resize_nwse(
        fb: &mut FrameBuffer,
        cx: f32,
        cy: f32,
        s: f32,
        outline: Color,
        fill: Color,
    ) {
        let o = s;
        // Diagonal line from top-left to bottom-right
        let len = 12;
        // Outline
        for i in 0..len {
            let fi = i as f32;
            rasterizer::fill_rect(
                fb,
                Rect::new(
                    cx + fi * s - o,
                    cy + fi * s - o,
                    2.0 * s + 2.0 * o,
                    2.0 * s + 2.0 * o,
                ),
                outline,
                BlendMode::SrcOver,
            );
        }
        // Fill
        for i in 0..len {
            let fi = i as f32;
            rasterizer::fill_rect(
                fb,
                Rect::new(cx + fi * s, cy + fi * s, 2.0 * s, 2.0 * s),
                fill,
                BlendMode::SrcOver,
            );
        }
        // NW arrowhead
        for i in 0..4 {
            let fi = i as f32;
            rasterizer::fill_rect(
                fb,
                Rect::new(cx, cy + fi * s, (4.0 - fi) * s, s),
                fill,
                BlendMode::SrcOver,
            );
        }
        // SE arrowhead
        let end = (len - 1) as f32;
        for i in 0..4 {
            let fi = i as f32;
            rasterizer::fill_rect(
                fb,
                Rect::new(
                    cx + (end - 3.0 + fi) * s + 2.0 * s,
                    cy + (end - fi) * s,
                    (4.0 - fi) * s,
                    s,
                ),
                fill,
                BlendMode::SrcOver,
            );
        }
    }

    /// Diagonal resize cursor (↗↙): NE-SW direction.
    fn draw_cursor_resize_nesw(
        fb: &mut FrameBuffer,
        cx: f32,
        cy: f32,
        s: f32,
        outline: Color,
        fill: Color,
    ) {
        let o = s;
        let len = 12;
        let max_i = (len - 1) as f32;
        // Outline
        for i in 0..len {
            let fi = i as f32;
            rasterizer::fill_rect(
                fb,
                Rect::new(
                    cx + (max_i - fi) * s - o,
                    cy + fi * s - o,
                    2.0 * s + 2.0 * o,
                    2.0 * s + 2.0 * o,
                ),
                outline,
                BlendMode::SrcOver,
            );
        }
        // Fill
        for i in 0..len {
            let fi = i as f32;
            rasterizer::fill_rect(
                fb,
                Rect::new(cx + (max_i - fi) * s, cy + fi * s, 2.0 * s, 2.0 * s),
                fill,
                BlendMode::SrcOver,
            );
        }
        // NE arrowhead
        for i in 0..4 {
            let fi = i as f32;
            rasterizer::fill_rect(
                fb,
                Rect::new(cx + (max_i - 3.0 + fi) * s, cy + fi * s, (4.0 - fi) * s, s),
                fill,
                BlendMode::SrcOver,
            );
        }
        // SW arrowhead
        for i in 0..4 {
            let fi = i as f32;
            rasterizer::fill_rect(
                fb,
                Rect::new(cx, cy + (max_i - fi) * s, (4.0 - fi) * s, s),
                fill,
                BlendMode::SrcOver,
            );
        }
    }

    /// Pointer / hand cursor: pointing hand for clickable items.
    fn draw_cursor_pointer(
        fb: &mut FrameBuffer,
        cx: f32,
        cy: f32,
        s: f32,
        outline: Color,
        fill: Color,
    ) {
        // Simplified pointing hand: index finger + palm
        let finger_rows: &[(f32, f32, f32)] = &[
            // (y_offset, x_offset, width)
            (0.0, 4.0, 2.0), // fingertip
            (1.0, 4.0, 2.0),
            (2.0, 4.0, 2.0),
            (3.0, 4.0, 2.0),
            (4.0, 4.0, 2.0),
            (5.0, 4.0, 2.0),
            (6.0, 1.0, 9.0), // palm starts
            (7.0, 0.0, 10.0),
            (8.0, 0.0, 10.0),
            (9.0, 0.0, 10.0),
            (10.0, 0.0, 10.0),
            (11.0, 1.0, 9.0),
            (12.0, 1.0, 8.0),
            (13.0, 2.0, 6.0),
        ];
        // Outline
        for &(row_y, row_x, row_w) in finger_rows {
            rasterizer::fill_rect(
                fb,
                Rect::new(
                    cx + row_x * s - s,
                    cy + row_y * s - 0.5 * s,
                    row_w * s + 2.0 * s,
                    2.0 * s,
                ),
                outline,
                BlendMode::SrcOver,
            );
        }
        // Fill
        for &(row_y, row_x, row_w) in finger_rows {
            rasterizer::fill_rect(
                fb,
                Rect::new(cx + row_x * s, cy + row_y * s, row_w * s, s),
                fill,
                BlendMode::SrcOver,
            );
        }
    }

    /// Text / I-beam cursor for text selection.
    fn draw_cursor_text(
        fb: &mut FrameBuffer,
        cx: f32,
        cy: f32,
        s: f32,
        outline: Color,
        fill: Color,
    ) {
        let center_x = cx + 6.0 * s;
        let top = cy + 1.0 * s;
        let bottom = cy + 13.0 * s;
        let bar_h = bottom - top;
        let serif_w = 4.0 * s;
        let o = s;

        // Outline
        rasterizer::fill_rect(
            fb,
            Rect::new(
                center_x - s - o,
                top - o,
                2.0 * s + 2.0 * o,
                bar_h + 2.0 * o,
            ),
            outline,
            BlendMode::SrcOver,
        );
        rasterizer::fill_rect(
            fb,
            Rect::new(
                center_x - serif_w * 0.5 - o,
                top - o,
                serif_w + 2.0 * o,
                2.0 * s + 2.0 * o,
            ),
            outline,
            BlendMode::SrcOver,
        );
        rasterizer::fill_rect(
            fb,
            Rect::new(
                center_x - serif_w * 0.5 - o,
                bottom - s - o,
                serif_w + 2.0 * o,
                2.0 * s + 2.0 * o,
            ),
            outline,
            BlendMode::SrcOver,
        );

        // Fill: vertical bar
        rasterizer::fill_rect(
            fb,
            Rect::new(center_x - s, top, 2.0 * s, bar_h),
            fill,
            BlendMode::SrcOver,
        );
        // Top serif
        rasterizer::fill_rect(
            fb,
            Rect::new(center_x - serif_w * 0.5, top, serif_w, s),
            fill,
            BlendMode::SrcOver,
        );
        // Bottom serif
        rasterizer::fill_rect(
            fb,
            Rect::new(center_x - serif_w * 0.5, bottom - s, serif_w, s),
            fill,
            BlendMode::SrcOver,
        );
    }

    /// Not-allowed / forbidden cursor: circle with diagonal line.
    fn draw_cursor_not_allowed(
        fb: &mut FrameBuffer,
        cx: f32,
        cy: f32,
        s: f32,
        outline: Color,
        fill: Color,
    ) {
        let center_x = cx + 7.0 * s;
        let center_y = cy + 7.0 * s;
        let _radius = 6.0 * s;
        let _thickness = 2.0 * s;

        // Approximate circle outline with rect segments
        let segments: &[(f32, f32, f32, f32)] = &[
            // (x_off, y_off, w, h) relative to center
            (-2.0, -6.0, 4.0, 1.0), // top
            (-4.0, -5.0, 8.0, 1.0),
            (-5.0, -4.0, 2.0, 1.0),
            (3.0, -4.0, 2.0, 1.0),
            (-6.0, -2.0, 1.0, 4.0), // left
            (5.0, -2.0, 1.0, 4.0),  // right
            (-5.0, 3.0, 2.0, 1.0),
            (3.0, 3.0, 2.0, 1.0),
            (-4.0, 4.0, 8.0, 1.0),
            (-2.0, 5.0, 4.0, 1.0), // bottom
        ];

        // Outline
        for &(xo, yo, w, h) in segments {
            rasterizer::fill_rect(
                fb,
                Rect::new(
                    center_x + xo * s - s,
                    center_y + yo * s - s,
                    w * s + 2.0 * s,
                    h * s + 2.0 * s,
                ),
                outline,
                BlendMode::SrcOver,
            );
        }
        // Fill ring
        for &(xo, yo, w, h) in segments {
            rasterizer::fill_rect(
                fb,
                Rect::new(center_x + xo * s, center_y + yo * s, w * s, h * s),
                fill,
                BlendMode::SrcOver,
            );
        }
        // Diagonal line through the circle (outline + fill)
        for i in 0..10 {
            let fi = i as f32;
            rasterizer::fill_rect(
                fb,
                Rect::new(
                    center_x + (-4.0 + fi) * s - s,
                    center_y + (-4.0 + fi) * s - s,
                    2.0 * s + 2.0 * s,
                    2.0 * s + 2.0 * s,
                ),
                outline,
                BlendMode::SrcOver,
            );
        }
        for i in 0..10 {
            let fi = i as f32;
            rasterizer::fill_rect(
                fb,
                Rect::new(
                    center_x + (-4.0 + fi) * s,
                    center_y + (-4.0 + fi) * s,
                    2.0 * s,
                    2.0 * s,
                ),
                fill,
                BlendMode::SrcOver,
            );
        }
    }

    fn draw_cursor_wait(
        fb: &mut FrameBuffer,
        cx: f32,
        cy: f32,
        s: f32,
        outline: Color,
        fill: Color,
    ) {
        // Hourglass shape
        let center_x = cx + 7.0 * s;
        let center_y = cy + 7.0 * s;

        // Top half
        rasterizer::fill_rect(
            fb,
            Rect::new(center_x - 4.0 * s, center_y - 6.0 * s, 8.0 * s, 2.0 * s),
            outline,
            BlendMode::SrcOver,
        );
        rasterizer::fill_rect(
            fb,
            Rect::new(center_x - 3.0 * s, center_y - 5.0 * s, 6.0 * s, 1.5 * s),
            fill,
            BlendMode::SrcOver,
        );

        // Neck
        rasterizer::fill_rect(
            fb,
            Rect::new(center_x - 1.0 * s, center_y - 1.0 * s, 2.0 * s, 2.0 * s),
            outline,
            BlendMode::SrcOver,
        );

        // Bottom half
        rasterizer::fill_rect(
            fb,
            Rect::new(center_x - 4.0 * s, center_y + 4.0 * s, 8.0 * s, 2.0 * s),
            outline,
            BlendMode::SrcOver,
        );
        rasterizer::fill_rect(
            fb,
            Rect::new(center_x - 3.0 * s, center_y + 3.5 * s, 6.0 * s, 1.5 * s),
            fill,
            BlendMode::SrcOver,
        );
    }

    fn draw_question_mark(fb: &mut FrameBuffer, cx: f32, cy: f32, s: f32, color: Color) {
        // Simple question mark shape
        rasterizer::fill_rect(
            fb,
            Rect::new(cx, cy, 3.0 * s, 1.0 * s),
            color,
            BlendMode::SrcOver,
        );
        rasterizer::fill_rect(
            fb,
            Rect::new(cx + 2.0 * s, cy + 1.0 * s, 1.0 * s, 2.0 * s),
            color,
            BlendMode::SrcOver,
        );
        rasterizer::fill_rect(
            fb,
            Rect::new(cx + 1.0 * s, cy + 3.0 * s, 1.0 * s, 1.0 * s),
            color,
            BlendMode::SrcOver,
        );
        rasterizer::fill_rect(
            fb,
            Rect::new(cx + 1.0 * s, cy + 5.0 * s, 1.0 * s, 1.0 * s),
            color,
            BlendMode::SrcOver,
        );
    }

    fn draw_cursor_crosshair(fb: &mut FrameBuffer, cx: f32, cy: f32, s: f32, color: Color) {
        let center_x = cx + 8.0 * s;
        let center_y = cy + 8.0 * s;

        // Vertical line
        rasterizer::fill_rect(
            fb,
            Rect::new(center_x - 0.5 * s, center_y - 6.0 * s, 1.0 * s, 12.0 * s),
            color,
            BlendMode::SrcOver,
        );
        // Horizontal line
        rasterizer::fill_rect(
            fb,
            Rect::new(center_x - 6.0 * s, center_y - 0.5 * s, 12.0 * s, 1.0 * s),
            color,
            BlendMode::SrcOver,
        );
    }

    fn draw_cursor_hand(
        fb: &mut FrameBuffer,
        cx: f32,
        cy: f32,
        s: f32,
        outline: Color,
        fill: Color,
        closed: bool,
    ) {
        let offset_x = if closed { 2.0 * s } else { 0.0 };

        // Palm
        rasterizer::fill_rect(
            fb,
            Rect::new(cx + 4.0 * s + offset_x, cy + 8.0 * s, 5.0 * s, 6.0 * s),
            outline,
            BlendMode::SrcOver,
        );
        rasterizer::fill_rect(
            fb,
            Rect::new(cx + 4.5 * s + offset_x, cy + 8.5 * s, 4.0 * s, 5.0 * s),
            fill,
            BlendMode::SrcOver,
        );

        // Fingers (simplified)
        for i in 0..4 {
            let fi = i as f32;
            rasterizer::fill_rect(
                fb,
                Rect::new(
                    cx + (5.0 + fi * 1.2) * s + offset_x,
                    cy + 4.0 * s,
                    1.0 * s,
                    5.0 * s,
                ),
                outline,
                BlendMode::SrcOver,
            );
        }
    }

    fn draw_cursor_magnifier(
        fb: &mut FrameBuffer,
        cx: f32,
        cy: f32,
        s: f32,
        outline: Color,
        fill: Color,
        plus: bool,
    ) {
        let center_x = cx + 6.0 * s;
        let center_y = cy + 6.0 * s;

        // Circle (lens)
        let segments: &[(f32, f32, f32, f32)] = &[
            (-2.0, -4.0, 4.0, 1.0),
            (-3.0, -3.0, 6.0, 1.0),
            (-4.0, -2.0, 8.0, 4.0),
            (-3.0, 2.0, 6.0, 1.0),
            (-2.0, 3.0, 4.0, 1.0),
        ];

        for &(xo, yo, w, h) in segments {
            rasterizer::fill_rect(
                fb,
                Rect::new(center_x + xo * s, center_y + yo * s, w * s, h * s),
                outline,
                BlendMode::SrcOver,
            );
        }

        // Handle
        rasterizer::fill_rect(
            fb,
            Rect::new(center_x + 3.0 * s, center_y + 3.0 * s, 4.0 * s, 1.0 * s),
            outline,
            BlendMode::SrcOver,
        );
        rasterizer::fill_rect(
            fb,
            Rect::new(center_x + 4.0 * s, center_y + 4.0 * s, 3.0 * s, 1.0 * s),
            outline,
            BlendMode::SrcOver,
        );

        // Plus or minus symbol
        if plus {
            rasterizer::fill_rect(
                fb,
                Rect::new(center_x - 1.0 * s, center_y - 0.5 * s, 2.0 * s, 1.0 * s),
                fill,
                BlendMode::SrcOver,
            );
            rasterizer::fill_rect(
                fb,
                Rect::new(center_x - 0.5 * s, center_y - 1.0 * s, 1.0 * s, 2.0 * s),
                fill,
                BlendMode::SrcOver,
            );
        } else {
            rasterizer::fill_rect(
                fb,
                Rect::new(center_x - 1.0 * s, center_y - 0.5 * s, 2.0 * s, 1.0 * s),
                fill,
                BlendMode::SrcOver,
            );
        }
    }

    fn draw_cursor_text_vertical(
        fb: &mut FrameBuffer,
        cx: f32,
        cy: f32,
        s: f32,
        outline: Color,
        fill: Color,
    ) {
        let center_x = cx + 8.0 * s;
        let center_y = cy + 8.0 * s;

        // Horizontal I-beam
        rasterizer::fill_rect(
            fb,
            Rect::new(
                center_x - 0.5 * s - s,
                center_y - 6.0 * s - s,
                1.0 * s + 2.0 * s,
                12.0 * s + 2.0 * s,
            ),
            outline,
            BlendMode::SrcOver,
        );
        rasterizer::fill_rect(
            fb,
            Rect::new(center_x - 0.5 * s, center_y - 6.0 * s, 1.0 * s, 12.0 * s),
            fill,
            BlendMode::SrcOver,
        );

        // Top and bottom bars (horizontal)
        rasterizer::fill_rect(
            fb,
            Rect::new(
                center_x - 3.0 * s - s,
                center_y - 6.0 * s - s,
                6.0 * s + 2.0 * s,
                1.0 * s + 2.0 * s,
            ),
            outline,
            BlendMode::SrcOver,
        );
        rasterizer::fill_rect(
            fb,
            Rect::new(center_x - 3.0 * s, center_y - 6.0 * s, 6.0 * s, 1.0 * s),
            fill,
            BlendMode::SrcOver,
        );

        rasterizer::fill_rect(
            fb,
            Rect::new(
                center_x - 3.0 * s - s,
                center_y + 5.0 * s - s,
                6.0 * s + 2.0 * s,
                1.0 * s + 2.0 * s,
            ),
            outline,
            BlendMode::SrcOver,
        );
        rasterizer::fill_rect(
            fb,
            Rect::new(center_x - 3.0 * s, center_y + 5.0 * s, 6.0 * s, 1.0 * s),
            fill,
            BlendMode::SrcOver,
        );
    }

    fn draw_cursor_all_scroll(
        fb: &mut FrameBuffer,
        cx: f32,
        cy: f32,
        s: f32,
        outline: Color,
        fill: Color,
    ) {
        let center_x = cx + 8.0 * s;
        let center_y = cy + 8.0 * s;

        // Four arrows pointing outward
        // Up arrow
        Self::draw_small_arrow(fb, center_x, center_y - 4.0 * s, s, 0.0, outline, fill);
        // Down arrow
        Self::draw_small_arrow(fb, center_x, center_y + 4.0 * s, s, 180.0, outline, fill);
        // Left arrow
        Self::draw_small_arrow(fb, center_x - 4.0 * s, center_y, s, 270.0, outline, fill);
        // Right arrow
        Self::draw_small_arrow(fb, center_x + 4.0 * s, center_y, s, 90.0, outline, fill);

        // Center dot
        rasterizer::fill_rect(
            fb,
            Rect::new(center_x - 1.0 * s, center_y - 1.0 * s, 2.0 * s, 2.0 * s),
            outline,
            BlendMode::SrcOver,
        );
    }

    fn draw_small_arrow(
        fb: &mut FrameBuffer,
        cx: f32,
        cy: f32,
        s: f32,
        _rotation: f32,
        outline: Color,
        _fill: Color,
    ) {
        // Simplified arrow (pointing up by default)
        rasterizer::fill_rect(
            fb,
            Rect::new(cx - 2.0 * s, cy, 4.0 * s, 1.0 * s),
            outline,
            BlendMode::SrcOver,
        );
        rasterizer::fill_rect(
            fb,
            Rect::new(cx - 1.0 * s, cy - 1.0 * s, 2.0 * s, 1.0 * s),
            outline,
            BlendMode::SrcOver,
        );
    }

    /// Submit an async backdrop blur for a region.
    ///
    /// Blits any cached result and submits a new blur request if needed.
    /// Used by Glass, BlurBackdrop, BlurCache, and LockScreen nodes.
    fn render_backdrop_blur(
        &mut self,
        node_id: NodeId,
        bounds: Rect,
        radius: u32,
        fb: &mut FrameBuffer,
    ) {
        let x0 = (bounds.x.max(0.0) as u32).min(fb.width);
        let y0 = (bounds.y.max(0.0) as u32).min(fb.height);
        let x1 = (bounds.right().ceil() as u32).min(fb.width);
        let y1 = (bounds.bottom().ceil() as u32).min(fb.height);
        let w = x1.saturating_sub(x0);
        let h = y1.saturating_sub(y0);

        if w == 0 || h == 0 {
            return;
        }

        let has_cache = self.blur_worker.get_cached(node_id, w, h).is_some();

        // Blit cached blur result if available.
        if let Some(cached) = self.blur_worker.get_cached(node_id, w, h) {
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

        // Submit new blur request if worker doesn't have one pending.
        if !has_cache || !self.blur_worker.has_pending(node_id) {
            let mut snapshot = vec![0u8; (w * h * 4) as usize];
            for row in 0..h {
                let src_off = fb.pixel_offset(x0, y0 + row);
                let dst_off = (row * w * 4) as usize;
                let bytes = (w * 4) as usize;
                snapshot[dst_off..dst_off + bytes]
                    .copy_from_slice(&fb.pixels[src_off..src_off + bytes]);
            }
            self.blur_worker
                .request_blur(node_id, snapshot, w, h, radius);
        }
    }
}
