//! Main renderer trait and software renderer implementation.

use std::collections::HashMap;

use liquide_compositor::damage::{DamageSet, DamageTile};
use liquide_compositor::effects::EffectParams;
use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::{BlendMode, Color};
use liquide_compositor::scene::{CursorShape, FlatNode, NodeId, ResizeDirection, SceneNodeKind};

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
    /// Window ID to render in skeleton mode (outline only during drag).
    skeleton_window: Option<u64>,
    /// Set to `true` during `render()` when any text node had glyphs
    /// not yet in the atlas.  The caller can check this to schedule an
    /// immediate follow-up render so the real TrueType glyphs appear
    /// without delay.
    has_pending_glyphs: bool,
    /// Tracks font_family+size combos that have already been pre-warmed
    /// to avoid redundant synchronous rasterization.
    prewarmed_fonts: std::collections::HashSet<(u32, u16)>,
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
            skeleton_window: None,
            has_pending_glyphs: false,
            prewarmed_fonts: std::collections::HashSet::new(),
        }
    }

    /// Create a renderer with a pre-loaded font database for real TrueType rendering.
    #[must_use]
    pub fn with_font_db(font_db: liquide_font_rasterizer::database::FontDatabase) -> Self {
        Self {
            srgb_lut: SrgbLut::new(),
            glyph_atlas: GlyphAtlas::new(2048, 2048),
            effect_params: EffectParams::for_profile(
                liquide_compositor::effects::QualityProfile::Balanced,
            ),
            blur_enabled: true,
            avg_render_ms: 0.0,
            blur_budget_ms: 16.0,
            blur_worker: BlurWorker::new(),
            shadow_cache: HashMap::new(),
            font_worker: FontWorker::with_font_db(font_db),
            layout_cache: LayoutCacheManager::new(),
            texture_cache: TextureCache::new(),
            dirty_rects: DirtyRectManager::new(1920, 1080),
            lod_manager: LodManager::new(1920.0, 1080.0),
            buffer_pool: ObjectPool::new(64),
            skeleton_window: None,
            has_pending_glyphs: false,
            prewarmed_fonts: std::collections::HashSet::new(),
        }
    }

    /// Returns `true` if the last `render()` call encountered text nodes
    /// whose glyphs were not yet in the atlas (i.e. still being rasterised
    /// by the font worker).  When this returns `true` the caller should
    /// schedule a follow-up render so the real glyphs appear promptly.
    #[must_use]
    pub fn has_pending_glyphs(&self) -> bool {
        self.has_pending_glyphs
    }

    /// Pre-warm the glyph atlas for a font by synchronously requesting
    /// common ASCII characters.  This runs once per unique (font_id, size)
    /// pair and avoids the 1-2 frame flash that used to occur when glyphs
    /// were not yet in the atlas.
    ///
    /// The actual rasterization still happens on the font-worker thread;
    /// this method merely ensures the requests are *queued* as early as
    /// possible so they complete before (or during) the very first frame
    /// that needs the glyphs.
    fn prewarm_glyphs(
        &mut self,
        font_id: u32,
        size_px: u16,
        target_height: u32,
        font_family: &str,
        font_weight: u16,
    ) {
        // Common characters that appear in virtually every UI text.
        const PREWARM_CHARS: &str =
            "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz\
             0123456789 .,;:!?-–—'\"()[]{}/<>@#$%^&*+=_~`|\\…•·";
        for ch in PREWARM_CHARS.chars() {
            let key = GlyphKey {
                font_id,
                glyph_id: ch as u32,
                size_px,
                subpixel: false,
            };
            if self.glyph_atlas.get(&key).is_none() {
                self.font_worker.request_glyph_with_font(
                    key,
                    ch,
                    target_height,
                    font_family.to_string(),
                    font_weight,
                );
            }
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

    /// Register an image from raw bytes (auto-detects format).
    /// Returns Ok(()) if successful, Err if decoding fails.
    pub fn register_image(&mut self, image_id: u64, data: &[u8]) -> Result<(), String> {
        let decoded = crate::image_decode::decode_image(data)
            .map_err(|e| format!("Image decode error: {}", e))?;

        let texture_id = format!("img_{}", image_id);
        self.texture_cache
            .insert(texture_id, decoded.pixels, decoded.width, decoded.height);
        Ok(())
    }

    /// Register a pre-decoded RGBA8 image.
    pub fn register_image_rgba(&mut self, image_id: u64, pixels: Vec<u8>, width: u32, height: u32) {
        let texture_id = format!("img_{}", image_id);
        self.texture_cache.insert(texture_id, pixels, width, height);
    }

    /// Check if an image is loaded.
    #[must_use]
    pub fn has_image(&mut self, image_id: u64) -> bool {
        let texture_id = format!("img_{}", image_id);
        self.texture_cache.get(&texture_id).is_some()
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

    /// Get the current LOD performance mode.
    #[must_use]
    pub fn get_lod_performance_mode(&self) -> PerformanceMode {
        self.lod_manager.get_performance_mode()
    }

    /// Enable or disable adaptive LOD.
    pub fn set_adaptive_lod_enabled(&mut self, enabled: bool) {
        self.lod_manager.set_adaptive_enabled(enabled);
    }

    // --- Skeleton Mode (for window drag visualization) ---

    /// Set skeleton window for simplified rendering during drag.
    pub fn set_skeleton_window(&mut self, window_id: Option<u64>) {
        self.skeleton_window = window_id;
    }

    /// Check if a node belongs to the skeleton window.
    fn is_skeleton_node(&self, node_id: u64) -> bool {
        if let Some(skeleton_wid) = self.skeleton_window {
            const NODE_WINDOW_BASE: u64 = 10_000;
            const NODE_WINDOW_STRIDE: u64 = 10;
            let win_base = NODE_WINDOW_BASE + skeleton_wid * NODE_WINDOW_STRIDE;
            let win_end = win_base + NODE_WINDOW_STRIDE;
            node_id >= win_base && node_id < win_end
        } else {
            false
        }
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
    #[allow(unused_assignments)]
    fn render(
        &mut self,
        nodes: &[FlatNode],
        fb: &mut FrameBuffer,
        damage: &DamageSet,
    ) -> crate::Result<Vec<DamageTile>> {
        // Reset pending-glyph tracker for this frame.
        self.has_pending_glyphs = false;

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
                button_colors,
                button_layout,
            } => {
                // Check if this is a skeleton node (window being dragged)
                let is_skeleton = self.is_skeleton_node(node.id);

                if is_skeleton {
                    // Skeleton mode: Only render a simple border outline
                    if *border_width > 0.0 {
                        let mut bc = *border_color;
                        if opacity < 1.0 {
                            bc.a = (bc.a as f32 * opacity + 0.5) as u8;
                        }
                        // Make border more visible during drag
                        bc.a = bc.a.saturating_add(40);
                        rasterizer::stroke_rounded_rect(
                            fb,
                            bounds,
                            *corner_radius,
                            *border_width * 1.5,
                            bc,
                            BlendMode::SrcOver,
                            &self.srgb_lut,
                        );
                    }
                } else {
                    // Normal mode: Full decoration with title bar, buttons, etc.
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
                    let title_bar_h = button_layout.title_bar_height;
                    let btn_w = button_layout.button_width;
                    let btn_h = button_layout.button_height;
                    let btn_y = bounds.y + (title_bar_h - btn_h) / 2.0;
                    let btn_right_margin = button_layout.button_right_margin;

                    // Close button (× icon) — rightmost
                    if button_state.close {
                        let close_x = bounds.x + bounds.width - btn_w - btn_right_margin;
                        let close_bg = if button_state.close_hovered {
                            button_colors.close_bg_hover
                        } else {
                            button_colors.close_bg
                        };
                        let close_bounds = Rect::new(close_x, btn_y, btn_w, btn_h);
                        rasterizer::fill_rounded_rect(
                            fb,
                            close_bounds,
                            button_layout.button_corner_radius,
                            &Fill::Solid(close_bg),
                            BlendMode::SrcOver,
                            &self.srgb_lut,
                        );
                        // × icon: two diagonal lines forming an X
                        let cx = close_x + btn_w / 2.0;
                        let cy_btn = btn_y + btn_h / 2.0;
                        let icon_color = button_colors.close_icon;
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
                            button_colors.maximize_bg_hover
                        } else {
                            button_colors.maximize_bg
                        };
                        let max_bounds = Rect::new(max_x, btn_y, btn_w, btn_h);
                        rasterizer::fill_rounded_rect(
                            fb,
                            max_bounds,
                            button_layout.button_corner_radius,
                            &Fill::Solid(btn_bg),
                            BlendMode::SrcOver,
                            &self.srgb_lut,
                        );
                        // □ icon: open rectangle outline
                        let cx = max_x + btn_w / 2.0;
                        let cy_btn = btn_y + btn_h / 2.0;
                        let icon_color = button_colors.maximize_icon;
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
                            button_colors.minimize_bg_hover
                        } else {
                            button_colors.minimize_bg
                        };
                        let min_bounds = Rect::new(min_x, btn_y, btn_w, btn_h);
                        rasterizer::fill_rounded_rect(
                            fb,
                            min_bounds,
                            button_layout.button_corner_radius,
                            &Fill::Solid(btn_bg),
                            BlendMode::SrcOver,
                            &self.srgb_lut,
                        );
                        // ─ icon: horizontal bar
                        let cx = min_x + btn_w / 2.0;
                        let cy_btn = btn_y + btn_h / 2.0;
                        let icon_color = button_colors.minimize_icon;
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
                                button_colors.pin_bg_active_hover
                            } else {
                                button_colors.pin_bg_active
                            }
                        } else if button_state.always_on_top_hovered {
                            button_colors.pin_bg_hover
                        } else {
                            button_colors.pin_bg
                        };
                        let aot_bounds = Rect::new(aot_x, btn_y, btn_w, btn_h);
                        rasterizer::fill_rounded_rect(
                            fb,
                            aot_bounds,
                            button_layout.button_corner_radius,
                            &Fill::Solid(btn_bg),
                            BlendMode::SrcOver,
                            &self.srgb_lut,
                        );
                        // Pin icon: vertical line with a small circle head
                        let cx = aot_x + btn_w / 2.0;
                        let cy_btn = btn_y + btn_h / 2.0;
                        let icon_color = if button_state.is_topmost {
                            button_colors.pin_icon_active
                        } else {
                            button_colors.pin_icon
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
                } // end else (normal decoration rendering)
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
                    CursorShape::Resize(dir) => {
                        use ResizeDirection::*;
                        match dir {
                            North | South => {
                                Self::draw_cursor_resize_ns(fb, cx, cy, s, outline, fill);
                            }
                            East | West => {
                                Self::draw_cursor_resize_ew(fb, cx, cy, s, outline, fill);
                            }
                            NorthWest | SouthEast => {
                                Self::draw_cursor_resize_nwse(fb, cx, cy, s, outline, fill);
                            }
                            NorthEast | SouthWest => {
                                Self::draw_cursor_resize_nesw(fb, cx, cy, s, outline, fill);
                            }
                        }
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
                    CursorShape::ColResize => {
                        Self::draw_cursor_resize_ew(fb, cx, cy, s, outline, fill);
                    }
                    CursorShape::RowResize => {
                        Self::draw_cursor_resize_ns(fb, cx, cy, s, outline, fill);
                    }
                    CursorShape::Custom { .. } | CursorShape::Hidden => {
                        // Custom cursors handled elsewhere, Hidden means don't draw
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

            SceneNodeKind::Text {
                text,
                color,
                scale,
                font_family,
                font_size,
                font_weight,
                font_style_italic: _,
                letter_spacing,
                word_spacing,
                line_height,
                text_align,
                text_transform,
                text_overflow,
                white_space: _,
                text_indent,
                text_decoration: _,
                text_shadows: _,
            } => {
                let mut c = *color;
                if opacity < 1.0 {
                    c.a = (c.a as f32 * opacity + 0.5) as u8;
                }

                // Apply text-transform before rendering
                let transformed: std::borrow::Cow<'_, str> = match text_transform {
                    2 => std::borrow::Cow::Owned(text.to_uppercase()),
                    3 => std::borrow::Cow::Owned(text.to_lowercase()),
                    1 => {
                        let mut result = String::with_capacity(text.len());
                        let mut cap_next = true;
                        for ch in text.chars() {
                            if ch.is_whitespace() {
                                cap_next = true;
                                result.push(ch);
                            } else if cap_next {
                                result.extend(ch.to_uppercase());
                                cap_next = false;
                            } else {
                                result.push(ch);
                            }
                        }
                        std::borrow::Cow::Owned(result)
                    }
                    _ => std::borrow::Cow::Borrowed(text.as_str()),
                };
                let render_text = &*transformed;

                // Determine effective glyph height:
                //  - If font_size > 0, use that directly as the pixel height.
                //  - Otherwise fall back to scale-based sizing (scale=1 → 16px).
                let glyph_height = if *font_size > 0.0 {
                    (*font_size).round() as u32
                } else {
                    16 * scale.max(&1)
                };

                // Encode font_weight and letter_spacing into the font_id
                // so the glyph atlas can differentiate bold vs regular.
                // Bit layout: [unused:8][weight:8][family_hash:16]
                let family_hash = if font_family.is_empty() {
                    0_u32
                } else {
                    // Simple hash of the family name for atlas key purposes.
                    let mut h: u32 = 5381;
                    for b in font_family.bytes() {
                        h = h.wrapping_mul(33).wrapping_add(b as u32);
                    }
                    h & 0xFFFF
                };
                let font_id = (((*font_weight as u32) & 0xFF) << 16) | family_hash;

                let size_px = glyph_height as u16;
                #[allow(unused_assignments)]
                let mut pen_x = bounds.x + text_indent;
                let mut pen_y = bounds.y;
                let line_h = if *line_height > 0.0 {
                    *line_height
                } else {
                    glyph_height as f32 * 1.2
                };

                // First pass: check which glyphs are in the atlas, request
                // missing ones from the font worker (with font family/weight
                // so the worker can use real TrueType rasterization).
                //
                // Pre-warm common glyphs synchronously the first time a new
                // font_id + size_px combo is encountered.  This avoids the
                // old bitmap-fallback flash: instead of rendering crude 8×16
                // bitmap text for 1-2 frames, we rasterise the most commonly
                // used characters up-front so they are already in the atlas
                // when we reach the rendering pass below.
                let prewarm_key = (font_id, size_px);
                if !font_family.is_empty() && !self.prewarmed_fonts.contains(&prewarm_key) {
                    self.prewarmed_fonts.insert(prewarm_key);
                    self.prewarm_glyphs(font_id, size_px, glyph_height, font_family, *font_weight);
                }

                for ch in render_text.chars() {
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
                        self.has_pending_glyphs = true;
                        self.font_worker.request_glyph_with_font(
                            key,
                            ch,
                            glyph_height,
                            font_family.clone(),
                            *font_weight,
                        );
                    }
                }

                // Always render using the atlas — draw glyphs that are
                // available and use an estimated advance for any that are
                // still being rasterised.  This completely eliminates the
                // old bitmap-fallback flash (crude 8×16 text for 1-2 frames).
                {
                    // Split text into lines and apply text-align per line
                    let lines: Vec<&str> = render_text.split('\n').collect();
                    let mut is_first_line = true;
                    // Estimated advance for a missing glyph (≈ 0.55 * font_size
                    // — a reasonable average for proportional Latin text).
                    let estimated_advance = glyph_height as f32 * 0.55;
                    for line_text in &lines {
                        // Measure line width for alignment
                        let mut line_width = 0.0f32;
                        if is_first_line {
                            line_width += text_indent;
                        }
                        for ch in line_text.chars() {
                            if ch == '\r' {
                                continue;
                            }
                            let key = GlyphKey {
                                font_id,
                                glyph_id: ch as u32,
                                size_px,
                                subpixel: false,
                            };
                            if let Some(cached) = self.glyph_atlas.get(&key) {
                                let extra = if ch == ' ' { *word_spacing } else { 0.0 };
                                line_width += cached.advance + *letter_spacing + extra;
                            } else {
                                let extra = if ch == ' ' { *word_spacing } else { 0.0 };
                                line_width += estimated_advance + *letter_spacing + extra;
                            }
                        }

                        // Text-align offset: 0=left, 1=center, 2=right, 3=justify
                        let align_x = match text_align {
                            1 => ((bounds.width - line_width) / 2.0).max(0.0),
                            2 => (bounds.width - line_width).max(0.0),
                            _ => 0.0,
                        };

                        pen_x = bounds.x + align_x;
                        if is_first_line {
                            pen_x += text_indent;
                        }

                        // Text overflow: ellipsis (1) — check if line overflows bounds
                        let max_x = bounds.x + bounds.width;
                        let use_ellipsis = *text_overflow == 1 && line_width > bounds.width;

                        for ch in line_text.chars() {
                            if ch == '\r' {
                                continue;
                            }

                            // Ellipsis check: if we're about to overflow, draw "…" instead
                            if use_ellipsis && pen_x + glyph_height as f32 * 0.6 > max_x {
                                let ellipsis_key = GlyphKey {
                                    font_id,
                                    glyph_id: '…' as u32,
                                    size_px,
                                    subpixel: false,
                                };
                                if let Some(cached) = self.glyph_atlas.get(&ellipsis_key).cloned() {
                                    let pos = liquide_compositor::geometry::Point::new(
                                        pen_x,
                                        pen_y + glyph_height as f32,
                                    );
                                    self.glyph_atlas.blit_glyph(fb, &cached, pos, c);
                                }
                                break;
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
                                let extra = if ch == ' ' { *word_spacing } else { 0.0 };
                                pen_x += cached.advance + *letter_spacing + extra;
                            } else {
                                // Glyph not yet in atlas — advance pen by estimated
                                // width so subsequent glyphs land in roughly the right
                                // position.  The missing glyph will appear on the next
                                // frame after the font worker completes rasterization.
                                let extra = if ch == ' ' { *word_spacing } else { 0.0 };
                                pen_x += estimated_advance + *letter_spacing + extra;
                            }
                        }
                        pen_y += line_h;
                        is_first_line = false;
                    }
                }
            }

            SceneNodeKind::Icon { icon_id, color } => {
                let mut c = *color;
                if opacity < 1.0 {
                    c.a = (c.a as f32 * opacity + 0.5) as u8;
                }
                crate::icons::draw_icon(fb, *icon_id, bounds, c, &self.srgb_lut);
            }

            // ── Backdrop Filter (blur + color effects on content behind) ──
            SceneNodeKind::BackdropFilter { filters } => {
                use liquide_compositor::scene::BackdropFilterSpec;
                for filter in filters {
                    match filter {
                        BackdropFilterSpec::Blur { radius } => {
                            let r = (*radius as u32).min(40);
                            if r > 0 && self.blur_enabled && lod_level != LodLevel::Low {
                                let lod_r = (r as f32 * quality_factor) as u32;
                                if lod_r > 0 {
                                    self.render_backdrop_blur(node.id, bounds, lod_r, fb);
                                }
                            }
                        }
                        BackdropFilterSpec::Brightness(b) => {
                            crate::filter::PixelFilter::Brightness(*b).apply(fb, bounds);
                        }
                        BackdropFilterSpec::Contrast(c) => {
                            crate::filter::PixelFilter::Contrast(*c).apply(fb, bounds);
                        }
                        BackdropFilterSpec::Saturate(s) => {
                            crate::filter::PixelFilter::Saturate(*s).apply(fb, bounds);
                        }
                        BackdropFilterSpec::HueRotate(deg) => {
                            crate::filter::PixelFilter::HueRotate(*deg).apply(fb, bounds);
                        }
                        BackdropFilterSpec::Grayscale(amount) => {
                            if *amount >= 0.99 {
                                crate::filter::PixelFilter::Grayscale.apply(fb, bounds);
                            } else {
                                crate::filter::PixelFilter::Saturate(1.0 - amount)
                                    .apply(fb, bounds);
                            }
                        }
                        BackdropFilterSpec::Sepia(amount) => {
                            if *amount >= 0.99 {
                                crate::filter::PixelFilter::Sepia.apply(fb, bounds);
                            }
                            // Partial sepia: lerp via saturate towards sepia
                        }
                        BackdropFilterSpec::Invert(amount) => {
                            if *amount >= 0.99 {
                                crate::filter::PixelFilter::Invert.apply(fb, bounds);
                            }
                            // Partial invert handled by brightness adjustment
                        }
                        BackdropFilterSpec::Opacity(o) => {
                            crate::filter::PixelFilter::Opacity(*o).apply(fb, bounds);
                        }
                    }
                }
            }

            // ── Post-processing Filter chain ────────────────────────
            SceneNodeKind::Filter { filters } => {
                use liquide_compositor::scene::FilterSpec;
                for filter in filters {
                    match filter {
                        FilterSpec::Blur { radius } => {
                            let r = (*radius as u32).min(40);
                            if r > 0 {
                                crate::blur::blur_region(fb, bounds, r);
                            }
                        }
                        FilterSpec::Brightness(b) => {
                            crate::filter::PixelFilter::Brightness(*b).apply(fb, bounds);
                        }
                        FilterSpec::Contrast(c) => {
                            crate::filter::PixelFilter::Contrast(*c).apply(fb, bounds);
                        }
                        FilterSpec::Saturate(s) => {
                            crate::filter::PixelFilter::Saturate(*s).apply(fb, bounds);
                        }
                        FilterSpec::HueRotate(deg) => {
                            crate::filter::PixelFilter::HueRotate(*deg).apply(fb, bounds);
                        }
                        FilterSpec::Grayscale(amount) => {
                            if *amount >= 0.99 {
                                crate::filter::PixelFilter::Grayscale.apply(fb, bounds);
                            } else {
                                crate::filter::PixelFilter::Saturate(1.0 - amount)
                                    .apply(fb, bounds);
                            }
                        }
                        FilterSpec::Sepia(amount) => {
                            if *amount >= 0.99 {
                                crate::filter::PixelFilter::Sepia.apply(fb, bounds);
                            }
                        }
                        FilterSpec::Invert(amount) => {
                            if *amount >= 0.99 {
                                crate::filter::PixelFilter::Invert.apply(fb, bounds);
                            }
                        }
                        FilterSpec::Opacity(o) => {
                            crate::filter::PixelFilter::Opacity(*o).apply(fb, bounds);
                        }
                        FilterSpec::DropShadow {
                            offset_x,
                            offset_y,
                            blur,
                            color,
                        } => {
                            let shadow_color = Color::new(
                                color.r,
                                color.g,
                                color.b,
                                (color.a as f32 * opacity + 0.5) as u8,
                            );
                            let params = ShadowParams {
                                surface_rect: bounds,
                                corner_radius: 0.0,
                                spread: 0.0,
                                blur_radius: (*blur as u32).min(40),
                                offset_x: *offset_x,
                                offset_y: *offset_y,
                                shadow_color,
                            };
                            if let Some(mask) =
                                BoxShadow::generate_shadow_mask(fb.width, fb.height, &params)
                            {
                                BoxShadow::composite_shadow_mask(fb, &mask);
                            }
                        }
                        FilterSpec::Url(_) => {} // SVG filter refs unsupported
                    }
                }
            }

            // ── Gradient Fill ────────────────────────────────────────
            SceneNodeKind::GradientFill { gradient } => {
                self.render_gradient(fb, bounds, gradient, opacity);
            }

            // ── Background Fill (color + optional gradient/image) ───
            SceneNodeKind::BackgroundFill { background } => {
                // Solid color first
                if let Some(bg_color) = background.color {
                    let mut c = bg_color;
                    if opacity < 1.0 {
                        c.a = (c.a as f32 * opacity + 0.5) as u8;
                    }
                    if c.a > 0 {
                        rasterizer::fill_rect(fb, bounds, c, BlendMode::SrcOver);
                    }
                }
                // Background image (gradient or texture)
                if let Some(ref img) = background.image {
                    use liquide_compositor::scene::BackgroundImage;
                    match img {
                        BackgroundImage::Gradient(gradient) => {
                            self.render_gradient(fb, bounds, gradient, opacity);
                        }
                        BackgroundImage::ImageId(image_id) => {
                            let texture_id = format!("img_{}", image_id);
                            if let Some(texture) = self.texture_cache.get(&texture_id) {
                                let src = Rect::new(
                                    0.0,
                                    0.0,
                                    texture.width as f32,
                                    texture.height as f32,
                                );
                                self.draw_scaled_texture(fb, &texture, src, bounds, opacity);
                            }
                        }
                        BackgroundImage::Url(_) => {} // External URLs unsupported
                    }
                }
            }

            // Scene node kinds not yet implemented in the CPU renderer
            SceneNodeKind::RenderLayer { .. }
            | SceneNodeKind::ClipPath { .. }
            | SceneNodeKind::Mask { .. }
            | SceneNodeKind::BorderImage { .. } => {}

            // ── CSS Border rendering ────────────────────────────────
            SceneNodeKind::Border { sides, radius } => {
                use liquide_compositor::scene::BorderSideStyle;

                let (r_tl, r_tr, r_br, r_bl) = *radius;
                let has_radius = r_tl > 0.5 || r_tr > 0.5 || r_br > 0.5 || r_bl > 0.5;

                if !has_radius {
                    // ── Fast path: straight edges (fill_rect per side) ──
                    let draw_border_side =
                        |fb: &mut FrameBuffer,
                         side_rect: Rect,
                         side: &liquide_compositor::scene::BorderSide,
                         op: f32,
                         horizontal: bool| {
                            if side.width <= 0.0
                                || side.style == BorderSideStyle::None
                                || side.style == BorderSideStyle::Hidden
                            {
                                return;
                            }
                            let mut c = side.color;
                            if op < 1.0 {
                                c.a = (c.a as f32 * op + 0.5) as u8;
                            }
                            if c.a == 0 {
                                return;
                            }

                            match side.style {
                                BorderSideStyle::Solid => {
                                    rasterizer::fill_rect(fb, side_rect, c, BlendMode::SrcOver);
                                }
                                BorderSideStyle::Dashed => {
                                    // Dashes: 3*width on, 3*width off
                                    let dash_len = (side.width * 3.0).max(3.0);
                                    let gap_len = dash_len;
                                    if horizontal {
                                        let mut dx = side_rect.x;
                                        let end = side_rect.x + side_rect.width;
                                        while dx < end {
                                            let seg_w = dash_len.min(end - dx);
                                            rasterizer::fill_rect(
                                                fb,
                                                Rect::new(dx, side_rect.y, seg_w, side_rect.height),
                                                c,
                                                BlendMode::SrcOver,
                                            );
                                            dx += dash_len + gap_len;
                                        }
                                    } else {
                                        let mut dy = side_rect.y;
                                        let end = side_rect.y + side_rect.height;
                                        while dy < end {
                                            let seg_h = dash_len.min(end - dy);
                                            rasterizer::fill_rect(
                                                fb,
                                                Rect::new(side_rect.x, dy, side_rect.width, seg_h),
                                                c,
                                                BlendMode::SrcOver,
                                            );
                                            dy += dash_len + gap_len;
                                        }
                                    }
                                }
                                BorderSideStyle::Dotted => {
                                    // Dots: circles spaced at 2*width intervals
                                    let dot_size = side.width;
                                    let spacing = dot_size * 2.0;
                                    if horizontal {
                                        let mut dx = side_rect.x + dot_size * 0.5;
                                        let end = side_rect.x + side_rect.width;
                                        let cy = side_rect.y + side_rect.height * 0.5;
                                        while dx < end {
                                            let r = (dot_size * 0.5).max(0.5);
                                            // Draw a filled circle approximated as a rect
                                            // (proper circle rendering would use SDF)
                                            rasterizer::fill_rect(
                                                fb,
                                                Rect::new(dx - r, cy - r, r * 2.0, r * 2.0),
                                                c,
                                                BlendMode::SrcOver,
                                            );
                                            dx += spacing;
                                        }
                                    } else {
                                        let mut dy = side_rect.y + dot_size * 0.5;
                                        let end = side_rect.y + side_rect.height;
                                        let cx = side_rect.x + side_rect.width * 0.5;
                                        while dy < end {
                                            let r = (dot_size * 0.5).max(0.5);
                                            rasterizer::fill_rect(
                                                fb,
                                                Rect::new(cx - r, dy - r, r * 2.0, r * 2.0),
                                                c,
                                                BlendMode::SrcOver,
                                            );
                                            dy += spacing;
                                        }
                                    }
                                }
                                BorderSideStyle::Double => {
                                    // Two lines with gap: each line is 1/3 of width
                                    let line_w = (side.width / 3.0).max(1.0);
                                    if horizontal {
                                        rasterizer::fill_rect(
                                            fb,
                                            Rect::new(
                                                side_rect.x,
                                                side_rect.y,
                                                side_rect.width,
                                                line_w,
                                            ),
                                            c,
                                            BlendMode::SrcOver,
                                        );
                                        rasterizer::fill_rect(
                                            fb,
                                            Rect::new(
                                                side_rect.x,
                                                side_rect.y + side_rect.height - line_w,
                                                side_rect.width,
                                                line_w,
                                            ),
                                            c,
                                            BlendMode::SrcOver,
                                        );
                                    } else {
                                        rasterizer::fill_rect(
                                            fb,
                                            Rect::new(
                                                side_rect.x,
                                                side_rect.y,
                                                line_w,
                                                side_rect.height,
                                            ),
                                            c,
                                            BlendMode::SrcOver,
                                        );
                                        rasterizer::fill_rect(
                                            fb,
                                            Rect::new(
                                                side_rect.x + side_rect.width - line_w,
                                                side_rect.y,
                                                line_w,
                                                side_rect.height,
                                            ),
                                            c,
                                            BlendMode::SrcOver,
                                        );
                                    }
                                }
                                BorderSideStyle::Groove | BorderSideStyle::Ridge => {
                                    // 3D effect: outer half is lighter/darker, inner is opposite
                                    let is_groove = side.style == BorderSideStyle::Groove;
                                    let light = Color::new(
                                        (c.r as u16 * 3 / 4 + 64).min(255) as u8,
                                        (c.g as u16 * 3 / 4 + 64).min(255) as u8,
                                        (c.b as u16 * 3 / 4 + 64).min(255) as u8,
                                        c.a,
                                    );
                                    let dark = Color::new(c.r / 2, c.g / 2, c.b / 2, c.a);
                                    let (outer_c, inner_c) = if is_groove {
                                        (dark, light)
                                    } else {
                                        (light, dark)
                                    };
                                    let half = (side.width / 2.0).max(1.0);
                                    if horizontal {
                                        rasterizer::fill_rect(
                                            fb,
                                            Rect::new(
                                                side_rect.x,
                                                side_rect.y,
                                                side_rect.width,
                                                half,
                                            ),
                                            outer_c,
                                            BlendMode::SrcOver,
                                        );
                                        rasterizer::fill_rect(
                                            fb,
                                            Rect::new(
                                                side_rect.x,
                                                side_rect.y + half,
                                                side_rect.width,
                                                (side_rect.height - half).max(0.0),
                                            ),
                                            inner_c,
                                            BlendMode::SrcOver,
                                        );
                                    } else {
                                        rasterizer::fill_rect(
                                            fb,
                                            Rect::new(
                                                side_rect.x,
                                                side_rect.y,
                                                half,
                                                side_rect.height,
                                            ),
                                            outer_c,
                                            BlendMode::SrcOver,
                                        );
                                        rasterizer::fill_rect(
                                            fb,
                                            Rect::new(
                                                side_rect.x + half,
                                                side_rect.y,
                                                (side_rect.width - half).max(0.0),
                                                side_rect.height,
                                            ),
                                            inner_c,
                                            BlendMode::SrcOver,
                                        );
                                    }
                                }
                                BorderSideStyle::Inset | BorderSideStyle::Outset => {
                                    // Inset: top+left darkened, bottom+right lightened
                                    // Outset: opposite
                                    let is_inset = side.style == BorderSideStyle::Inset;
                                    let light = Color::new(
                                        (c.r as u16 * 3 / 4 + 64).min(255) as u8,
                                        (c.g as u16 * 3 / 4 + 64).min(255) as u8,
                                        (c.b as u16 * 3 / 4 + 64).min(255) as u8,
                                        c.a,
                                    );
                                    let dark = Color::new(c.r / 2, c.g / 2, c.b / 2, c.a);
                                    // For horizontal borders: top uses outer, bottom uses outer
                                    // For vertical: left uses outer, right uses outer
                                    // "outer" meaning depends on inset vs outset
                                    let use_dark = is_inset;
                                    let final_c = if use_dark { dark } else { light };
                                    rasterizer::fill_rect(
                                        fb,
                                        side_rect,
                                        final_c,
                                        BlendMode::SrcOver,
                                    );
                                }
                                BorderSideStyle::None | BorderSideStyle::Hidden => {}
                            }
                        };

                    // Top border
                    draw_border_side(
                        fb,
                        Rect::new(bounds.x, bounds.y, bounds.width, sides.top.width),
                        &sides.top,
                        opacity,
                        true,
                    );
                    // Bottom border
                    draw_border_side(
                        fb,
                        Rect::new(
                            bounds.x,
                            bounds.bottom() - sides.bottom.width,
                            bounds.width,
                            sides.bottom.width,
                        ),
                        &sides.bottom,
                        opacity,
                        true,
                    );
                    // Left border (between top and bottom)
                    draw_border_side(
                        fb,
                        Rect::new(
                            bounds.x,
                            bounds.y + sides.top.width,
                            sides.left.width,
                            bounds.height - sides.top.width - sides.bottom.width,
                        ),
                        &sides.left,
                        opacity,
                        false,
                    );
                    // Right border (between top and bottom)
                    draw_border_side(
                        fb,
                        Rect::new(
                            bounds.right() - sides.right.width,
                            bounds.y + sides.top.width,
                            sides.right.width,
                            bounds.height - sides.top.width - sides.bottom.width,
                        ),
                        &sides.right,
                        opacity,
                        false,
                    );
                } else {
                    // ── Rounded border: SDF-based per-pixel rendering ──
                    //
                    // Uses outer − inner rounded-rect SDF coverage to
                    // determine the border region, then a diagonal quadrant
                    // test to pick the per-side color (CSS trapezoidal rule).

                    let outer = bounds;
                    let inner = Rect::new(
                        bounds.x + sides.left.width,
                        bounds.y + sides.top.width,
                        (bounds.width - sides.left.width - sides.right.width).max(0.0),
                        (bounds.height - sides.top.width - sides.bottom.width).max(0.0),
                    );

                    // Inner radii: shrink by the larger adjacent border width
                    let ir_tl = (r_tl - sides.left.width.max(sides.top.width)).max(0.0);
                    let ir_tr = (r_tr - sides.right.width.max(sides.top.width)).max(0.0);
                    let ir_br = (r_br - sides.right.width.max(sides.bottom.width)).max(0.0);
                    let ir_bl = (r_bl - sides.left.width.max(sides.bottom.width)).max(0.0);

                    let x0 = (outer.x.max(0.0) as u32).min(fb.width);
                    let y0 = (outer.y.max(0.0) as u32).min(fb.height);
                    let x1 = (outer.right().ceil() as u32).min(fb.width);
                    let y1 = (outer.bottom().ceil() as u32).min(fb.height);

                    if x0 >= x1 || y0 >= y1 {
                        return;
                    }

                    // Centre for CSS trapezoidal side selection
                    let hx = outer.width * 0.5;
                    let hy = outer.height * 0.5;
                    let cx = outer.x + hx;
                    let cy = outer.y + hy;

                    // Pre-resolve each side: (visible, premultiplied color)
                    let resolve_side = |side: &liquide_compositor::scene::BorderSide| {
                        if side.width <= 0.0
                            || side.style == BorderSideStyle::None
                            || side.style == BorderSideStyle::Hidden
                        {
                            return (false, Color::new(0, 0, 0, 0));
                        }
                        let mut c = side.color;
                        if opacity < 1.0 {
                            c.a = (c.a as f32 * opacity + 0.5) as u8;
                        }
                        if c.a == 0 {
                            return (false, Color::new(0, 0, 0, 0));
                        }
                        (true, c.premultiply())
                    };
                    let (top_vis, top_pm) = resolve_side(&sides.top);
                    let (right_vis, right_pm) = resolve_side(&sides.right);
                    let (bottom_vis, bottom_pm) = resolve_side(&sides.bottom);
                    let (left_vis, left_pm) = resolve_side(&sides.left);

                    // Skip entirely if no sides are visible
                    if !top_vis && !right_vis && !bottom_vis && !left_vis {
                        return;
                    }

                    // Aspect-ratio factor for diagonal side selection:
                    // normalise dx,dy to -1..1 range so diagonals are 45°.
                    let inv_hx = if hx > 0.0 { 1.0 / hx } else { 0.0 };
                    let inv_hy = if hy > 0.0 { 1.0 / hy } else { 0.0 };

                    for y in y0..y1 {
                        let fy = y as f32 + 0.5;
                        for x in x0..x1 {
                            let fx = x as f32 + 0.5;

                            // Outer SDF coverage (per-corner radii)
                            let outer_d = rasterizer::sdf_rounded_rect_per_corner(
                                fx, fy, &outer, r_tl, r_tr, r_br, r_bl,
                            );
                            let outer_cov = (-outer_d + 0.5).clamp(0.0, 1.0);
                            if outer_cov <= 0.0 {
                                continue;
                            }

                            // Inner SDF coverage (shrunk radii)
                            let inner_cov = if inner.width > 0.0 && inner.height > 0.0 {
                                let inner_d = rasterizer::sdf_rounded_rect_per_corner(
                                    fx, fy, &inner, ir_tl, ir_tr, ir_br, ir_bl,
                                );
                                (-inner_d + 0.5).clamp(0.0, 1.0)
                            } else {
                                0.0
                            };

                            let border_cov = (outer_cov - inner_cov).clamp(0.0, 1.0);
                            if border_cov <= 0.0 {
                                continue;
                            }

                            // CSS trapezoidal side selection via diagonals
                            let rx = (fx - cx) * inv_hx;
                            let ry = (fy - cy) * inv_hy;
                            let abs_rx = rx.abs();

                            let (vis, pm) = if ry < -abs_rx {
                                (top_vis, top_pm)
                            } else if ry > abs_rx {
                                (bottom_vis, bottom_pm)
                            } else if rx < 0.0 {
                                (left_vis, left_pm)
                            } else {
                                (right_vis, right_pm)
                            };

                            if !vis {
                                continue;
                            }

                            let mut src = pm;
                            if border_cov < 1.0 {
                                src.a = (src.a as f32 * border_cov + 0.5) as u8;
                                src.r = (src.r as f32 * border_cov + 0.5) as u8;
                                src.g = (src.g as f32 * border_cov + 0.5) as u8;
                                src.b = (src.b as f32 * border_cov + 0.5) as u8;
                            }

                            if src.a == 0 {
                                continue;
                            }

                            let dst = fb.get_pixel(x, y);
                            let blended = crate::blend::blend(dst, src, BlendMode::SrcOver);
                            fb.set_pixel(x, y, blended);
                        }
                    }
                }
            }

            // ── CSS BoxShadow rendering ─────────────────────────────
            SceneNodeKind::BoxShadows { shadows } => {
                if lod_level == LodLevel::Low {
                    return; // Skip shadows at low detail
                }
                for shadow in shadows {
                    if shadow.color.a == 0 {
                        continue;
                    }
                    let mut shadow_color = shadow.color;
                    if opacity < 1.0 {
                        shadow_color.a = (shadow_color.a as f32 * opacity + 0.5) as u8;
                    }

                    if shadow.inset {
                        // Inset shadow: darken inside edges of the element
                        // Top edge
                        let edge_h = shadow.blur_radius.max(shadow.spread_radius).max(1.0);
                        let mut c = shadow_color;
                        c.a = c.a / 2; // soften inset shadow
                        rasterizer::fill_rect(
                            fb,
                            Rect::new(bounds.x, bounds.y, bounds.width, edge_h),
                            c,
                            BlendMode::SrcOver,
                        );
                        // Bottom edge
                        rasterizer::fill_rect(
                            fb,
                            Rect::new(bounds.x, bounds.bottom() - edge_h, bounds.width, edge_h),
                            c,
                            BlendMode::SrcOver,
                        );
                    } else {
                        // Outer shadow: use the existing shadow effect system
                        let lod_blur = (shadow.blur_radius * quality_factor) as u32;
                        let shadow_bounds = Rect::new(
                            bounds.x + shadow.offset_x - shadow.spread_radius,
                            bounds.y + shadow.offset_y - shadow.spread_radius,
                            bounds.width + shadow.spread_radius * 2.0,
                            bounds.height + shadow.spread_radius * 2.0,
                        );
                        let params = ShadowParams {
                            surface_rect: shadow_bounds,
                            corner_radius: 0.0,
                            spread: shadow.spread_radius,
                            blur_radius: lod_blur,
                            offset_x: shadow.offset_x,
                            offset_y: shadow.offset_y,
                            shadow_color,
                        };
                        if let Some(mask) =
                            BoxShadow::generate_shadow_mask(fb.width, fb.height, &params)
                        {
                            BoxShadow::composite_shadow_mask(fb, &mask);
                        }
                    }
                }
            }

            // ── CSS Image rendering ─────────────────────────────────
            SceneNodeKind::Image {
                image_id,
                width,
                height,
                fit,
            } => {
                let texture_id = format!("img_{}", image_id);

                // Try to get the cached texture
                if let Some(texture) = self.texture_cache.get(&texture_id) {
                    // Calculate source and destination rectangles based on fit mode
                    let src_w = texture.width as f32;
                    let src_h = texture.height as f32;
                    let dst_w = bounds.width;
                    let dst_h = bounds.height;

                    let (src_rect, dst_rect) = match fit {
                        liquide_compositor::scene::ImageFit::Fill => {
                            // Stretch to fill entire bounds
                            (Rect::new(0.0, 0.0, src_w, src_h), bounds)
                        }
                        liquide_compositor::scene::ImageFit::Contain => {
                            // Scale to fit within bounds, preserving aspect ratio
                            let scale = (dst_w / src_w).min(dst_h / src_h);
                            let scaled_w = src_w * scale;
                            let scaled_h = src_h * scale;
                            let offset_x = (dst_w - scaled_w) / 2.0;
                            let offset_y = (dst_h - scaled_h) / 2.0;
                            (
                                Rect::new(0.0, 0.0, src_w, src_h),
                                Rect::new(
                                    bounds.x + offset_x,
                                    bounds.y + offset_y,
                                    scaled_w,
                                    scaled_h,
                                ),
                            )
                        }
                        liquide_compositor::scene::ImageFit::Cover => {
                            // Scale to fill bounds, preserving aspect ratio (may crop)
                            let scale = (dst_w / src_w).max(dst_h / src_h);
                            let scaled_w = src_w * scale;
                            let scaled_h = src_h * scale;
                            let crop_x = ((scaled_w - dst_w) / 2.0) / scale;
                            let crop_y = ((scaled_h - dst_h) / 2.0) / scale;
                            (
                                Rect::new(
                                    crop_x,
                                    crop_y,
                                    src_w - crop_x * 2.0,
                                    src_h - crop_y * 2.0,
                                ),
                                bounds,
                            )
                        }
                        liquide_compositor::scene::ImageFit::None => {
                            // Display at natural size, centered
                            let offset_x = (dst_w - src_w) / 2.0;
                            let offset_y = (dst_h - src_h) / 2.0;
                            (
                                Rect::new(0.0, 0.0, src_w, src_h),
                                Rect::new(
                                    bounds.x + offset_x,
                                    bounds.y + offset_y,
                                    src_w.min(dst_w),
                                    src_h.min(dst_h),
                                ),
                            )
                        }
                    };

                    // Draw the image with scaling
                    self.draw_scaled_texture(fb, &texture, src_rect, dst_rect, opacity);
                } else {
                    // Fallback: render placeholder when image not loaded
                    let placeholder_color = Color::new(
                        128,
                        128,
                        128,
                        if opacity < 1.0 {
                            (64.0 * opacity + 0.5) as u8
                        } else {
                            64
                        },
                    );
                    rasterizer::fill_rect(fb, bounds, placeholder_color, BlendMode::SrcOver);

                    // Small center indicator
                    let cx = bounds.x + bounds.width / 2.0;
                    let cy = bounds.y + bounds.height / 2.0;
                    let dot_size = 4.0_f32.min(bounds.width / 4.0).min(bounds.height / 4.0);
                    if dot_size > 0.5 {
                        let indicator = Color::new(
                            180,
                            180,
                            180,
                            if opacity < 1.0 {
                                (80.0 * opacity + 0.5) as u8
                            } else {
                                80
                            },
                        );
                        rasterizer::fill_rect(
                            fb,
                            Rect::new(cx - dot_size, cy - dot_size, dot_size * 2.0, dot_size * 2.0),
                            indicator,
                            BlendMode::SrcOver,
                        );
                    }
                }
                let _ = (width, height); // suppress unused warnings
            }

            // ── CSS Outline rendering ───────────────────────────────
            SceneNodeKind::Outline { outline } => {
                use liquide_compositor::scene::OutlineStyle;
                if outline.width <= 0.0 || outline.style == OutlineStyle::None {
                    return;
                }
                let mut c = outline.color;
                if opacity < 1.0 {
                    c.a = (c.a as f32 * opacity + 0.5) as u8;
                }
                if c.a == 0 {
                    return;
                }
                let offset = outline.offset;
                let outline_rect = Rect::new(
                    bounds.x - outline.width - offset,
                    bounds.y - outline.width - offset,
                    bounds.width + (outline.width + offset) * 2.0,
                    bounds.height + (outline.width + offset) * 2.0,
                );
                rasterizer::stroke_rect(fb, outline_rect, outline.width, c, BlendMode::SrcOver);
            }

            // ── Text Caret (blinking insertion cursor) ──────────────
            SceneNodeKind::TextCaret { color, width } => {
                let mut c = *color;
                if opacity < 1.0 {
                    c.a = (c.a as f32 * opacity + 0.5) as u8;
                }
                if c.a > 0 {
                    let caret_rect = Rect::new(bounds.x, bounds.y, *width, bounds.height);
                    rasterizer::fill_rect(fb, caret_rect, c, BlendMode::SrcOver);
                }
            }

            // ── Selection / inspection overlay ──────────────────────
            SceneNodeKind::SelectionOverlay {
                fill,
                border_color,
                border_width,
            } => {
                // Semi-transparent fill.
                let mut fc = *fill;
                if opacity < 1.0 {
                    fc.a = (fc.a as f32 * opacity + 0.5) as u8;
                }
                if fc.a > 0 {
                    rasterizer::fill_rect(fb, bounds, fc, BlendMode::SrcOver);
                }
                // Border.
                if *border_width > 0.0 {
                    let mut bc = *border_color;
                    if opacity < 1.0 {
                        bc.a = (bc.a as f32 * opacity + 0.5) as u8;
                    }
                    if bc.a > 0 {
                        rasterizer::stroke_rect(fb, bounds, *border_width, bc, BlendMode::SrcOver);
                    }
                }
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

    /// Render a gradient fill within `bounds`.
    ///
    /// Supports linear, radial, and conic gradients with antialiased color stops.
    /// Gradient rendering — linear interpolation between color stops:
    /// each pixel is evaluated against the gradient function and color stops
    /// are linearly interpolated.
    fn render_gradient(
        &mut self,
        fb: &mut FrameBuffer,
        bounds: Rect,
        gradient: &liquide_compositor::scene::GradientSpec,
        opacity: f32,
    ) {
        use liquide_compositor::scene::GradientSpec;

        let x0 = (bounds.x.max(0.0) as u32).min(fb.width);
        let y0 = (bounds.y.max(0.0) as u32).min(fb.height);
        let x1 = (bounds.right().ceil() as u32).min(fb.width);
        let y1 = (bounds.bottom().ceil() as u32).min(fb.height);

        if x0 >= x1 || y0 >= y1 {
            return;
        }

        match gradient {
            GradientSpec::Linear {
                start_x,
                start_y,
                end_x,
                end_y,
                stops,
            } => {
                if stops.is_empty() {
                    return;
                }
                // Compute direction vector in pixel space
                let sx = bounds.x + start_x * bounds.width;
                let sy = bounds.y + start_y * bounds.height;
                let ex = bounds.x + end_x * bounds.width;
                let ey = bounds.y + end_y * bounds.height;
                let dx = ex - sx;
                let dy = ey - sy;
                let len2 = dx * dx + dy * dy;
                if len2 < 0.001 {
                    return;
                }
                let inv_len2 = 1.0 / len2;

                for y in y0..y1 {
                    let fy = y as f32 + 0.5;
                    for x in x0..x1 {
                        let fx = x as f32 + 0.5;
                        // Project pixel onto gradient line
                        let t = ((fx - sx) * dx + (fy - sy) * dy) * inv_len2;
                        let t_clamped = t.clamp(0.0, 1.0);
                        let color = sample_gradient_stops(stops, t_clamped, opacity);
                        if color.a > 0 {
                            let dst = fb.get_pixel(x, y);
                            let blended =
                                crate::blend::blend(dst, color.premultiply(), BlendMode::SrcOver);
                            fb.set_pixel(x, y, blended);
                        }
                    }
                }
            }
            GradientSpec::Radial {
                center_x,
                center_y,
                radius,
                stops,
            } => {
                if stops.is_empty() || *radius <= 0.0 {
                    return;
                }
                let cx = bounds.x + center_x * bounds.width;
                let cy = bounds.y + center_y * bounds.height;
                let r = radius * bounds.width.min(bounds.height);
                let inv_r = 1.0 / r;

                for y in y0..y1 {
                    let fy = y as f32 + 0.5;
                    for x in x0..x1 {
                        let fx = x as f32 + 0.5;
                        let dx = fx - cx;
                        let dy = fy - cy;
                        let dist = (dx * dx + dy * dy).sqrt();
                        let t = (dist * inv_r).clamp(0.0, 1.0);
                        let color = sample_gradient_stops(stops, t, opacity);
                        if color.a > 0 {
                            let dst = fb.get_pixel(x, y);
                            let blended =
                                crate::blend::blend(dst, color.premultiply(), BlendMode::SrcOver);
                            fb.set_pixel(x, y, blended);
                        }
                    }
                }
            }
            GradientSpec::Conic {
                center_x,
                center_y,
                start_angle,
                stops,
            } => {
                if stops.is_empty() {
                    return;
                }
                let cx = bounds.x + center_x * bounds.width;
                let cy = bounds.y + center_y * bounds.height;
                let start_rad = start_angle.to_radians();

                for y in y0..y1 {
                    let fy = y as f32 + 0.5;
                    for x in x0..x1 {
                        let fx = x as f32 + 0.5;
                        let mut angle = (fy - cy).atan2(fx - cx) - start_rad;
                        if angle < 0.0 {
                            angle += std::f32::consts::TAU;
                        }
                        let t = angle / std::f32::consts::TAU;
                        let color = sample_gradient_stops(stops, t.clamp(0.0, 1.0), opacity);
                        if color.a > 0 {
                            let dst = fb.get_pixel(x, y);
                            let blended =
                                crate::blend::blend(dst, color.premultiply(), BlendMode::SrcOver);
                            fb.set_pixel(x, y, blended);
                        }
                    }
                }
            }
            GradientSpec::Mesh { .. } => {
                // Mesh gradients are complex; draw as a solid mid-gray fallback
                let c = Color::new(80, 80, 80, (128.0 * opacity + 0.5) as u8);
                rasterizer::fill_rect(fb, bounds, c, BlendMode::SrcOver);
            }
        }
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

    /// Draw a texture to the framebuffer with scaling.
    fn draw_scaled_texture(
        &mut self,
        fb: &mut FrameBuffer,
        texture: &crate::texture_cache::CachedTexture,
        src_rect: Rect,
        dst_rect: Rect,
        opacity: f32,
    ) {
        let src_x0 = src_rect.x.max(0.0) as u32;
        let src_y0 = src_rect.y.max(0.0) as u32;
        let src_x1 = (src_rect.right().min(texture.width as f32)) as u32;
        let src_y1 = (src_rect.bottom().min(texture.height as f32)) as u32;

        let dst_x0 = dst_rect.x.max(0.0);
        let dst_y0 = dst_rect.y.max(0.0);
        let dst_x1 = dst_rect.right().min(fb.width as f32);
        let dst_y1 = dst_rect.bottom().min(fb.height as f32);

        if dst_x0 >= dst_x1 || dst_y0 >= dst_y1 {
            return;
        }

        let src_w = (src_x1 - src_x0) as f32;
        let src_h = (src_y1 - src_y0) as f32;
        let dst_w = dst_x1 - dst_x0;
        let dst_h = dst_y1 - dst_y0;

        // Nearest-neighbor scaling for simplicity
        // (could be upgraded to bilinear for better quality)
        for dst_y in (dst_y0 as u32)..(dst_y1 as u32) {
            for dst_x in (dst_x0 as u32)..(dst_x1 as u32) {
                let rel_x = (dst_x as f32 - dst_x0) / dst_w;
                let rel_y = (dst_y as f32 - dst_y0) / dst_h;
                let src_x = (src_x0 as f32 + rel_x * src_w) as u32;
                let src_y = (src_y0 as f32 + rel_y * src_h) as u32;

                let src_idx = ((src_y * texture.width + src_x) * 4) as usize;
                if src_idx + 3 >= texture.data.len() {
                    continue;
                }

                let mut src_color = Color::new(
                    texture.data[src_idx],
                    texture.data[src_idx + 1],
                    texture.data[src_idx + 2],
                    texture.data[src_idx + 3],
                );

                // Apply opacity
                if opacity < 1.0 {
                    src_color.a = (src_color.a as f32 * opacity + 0.5) as u8;
                }

                // Premultiply and blend
                src_color = src_color.premultiply();
                let dst_color = fb.get_pixel(dst_x, dst_y);
                let blended = crate::blend::blend(dst_color, src_color, BlendMode::SrcOver);
                fb.set_pixel(dst_x, dst_y, blended);
            }
        }
    }
}

// ── Gradient stop sampling ──────────────────────────────────────────

/// Sample a color from sorted gradient stops at parameter `t` ∈ [0, 1].
///
/// Uses linear interpolation between adjacent stops, consistent with
/// linear gradient shader: if only one
/// stop exists, its color is returned. Opacity is pre-multiplied into
/// the alpha channel.
fn sample_gradient_stops(stops: &[(f32, Color)], t: f32, opacity: f32) -> Color {
    if stops.is_empty() {
        return Color::new(0, 0, 0, 0);
    }
    if stops.len() == 1 {
        let mut c = stops[0].1;
        if opacity < 1.0 {
            c.a = (c.a as f32 * opacity + 0.5) as u8;
        }
        return c;
    }

    // Clamp to first/last stop
    if t <= stops[0].0 {
        let mut c = stops[0].1;
        if opacity < 1.0 {
            c.a = (c.a as f32 * opacity + 0.5) as u8;
        }
        return c;
    }
    let last = stops.len() - 1;
    if t >= stops[last].0 {
        let mut c = stops[last].1;
        if opacity < 1.0 {
            c.a = (c.a as f32 * opacity + 0.5) as u8;
        }
        return c;
    }

    // Find the two stops bracketing `t`
    for i in 0..last {
        let (t0, c0) = &stops[i];
        let (t1, c1) = &stops[i + 1];
        if t >= *t0 && t <= *t1 {
            let range = t1 - t0;
            let frac = if range > 0.001 { (t - t0) / range } else { 0.0 };
            let inv = 1.0 - frac;
            let r = (c0.r as f32 * inv + c1.r as f32 * frac + 0.5) as u8;
            let g = (c0.g as f32 * inv + c1.g as f32 * frac + 0.5) as u8;
            let b = (c0.b as f32 * inv + c1.b as f32 * frac + 0.5) as u8;
            let a_raw = c0.a as f32 * inv + c1.a as f32 * frac;
            let a = if opacity < 1.0 {
                (a_raw * opacity + 0.5) as u8
            } else {
                (a_raw + 0.5) as u8
            };
            return Color::new(r, g, b, a);
        }
    }

    // Fallback
    let mut c = stops[last].1;
    if opacity < 1.0 {
        c.a = (c.a as f32 * opacity + 0.5) as u8;
    }
    c
}
