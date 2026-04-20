//! Main renderer trait and software renderer implementation.

mod borders;
mod cursors;
mod decoration;
mod effects;
mod gradients;
mod helpers;
mod images;
mod text;

use std::collections::HashMap;

use liquide_compositor::damage::{DamageSet, DamageTile};
use liquide_compositor::effects::EffectParams;
use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::{BlendMode, Color};
use liquide_compositor::scene::{FlatNode, NodeId, SceneNodeKind};

use crate::blur_worker::BlurWorker;
use crate::color::SrgbLut;
use crate::dirty_rects::DirtyRectManager;
use crate::effects::ShadowMask;
use crate::font_worker::FontWorker;
use crate::glyph::{GlyphAtlas, GlyphKey};
use crate::layout_cache::LayoutCacheManager;
use crate::lod::{LodCriteria, LodLevel, LodManager, PerformanceMode};
use crate::object_pool::ObjectPool;
use crate::rasterizer;
use crate::texture_cache::TextureCache;

/// Cached shadow mask for a specific window position/size.
///
/// Avoids recomputing the expensive SDF + Gaussian blur every frame.
/// Invalidated when the source window bounds change.
pub(crate) struct CachedShadow {
    mask: ShadowMask,
    /// Source bounds as integer pixels for invalidation.
    bx: i32,
    by: i32,
    bw: u32,
    bh: u32,
}

/// Maximum number of entries in the shadow mask cache before eviction.
const MAX_SHADOW_CACHE: usize = 256;

// Re-export the Renderer trait from liquide-compositor so downstream crates
// can import it from either location.
pub use liquide_compositor::Renderer;

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
    /// Active blend mode set by the most recent `RenderLayer` node.
    /// Subsequent content nodes use this instead of the default `SrcOver`.
    active_blend_mode: BlendMode,
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
            active_blend_mode: BlendMode::SrcOver,
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
            active_blend_mode: BlendMode::SrcOver,
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
    /// common ASCII characters.
    fn prewarm_glyphs(
        &mut self,
        font_id: u32,
        size_px: u16,
        target_height: u32,
        font_family: &str,
        font_weight: u16,
    ) {
        const PREWARM_CHARS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz\
             0123456789 .,;:!?-\u{2013}\u{2014}'\"()[]{}/<>@#$%^&*+=_~`|\\\u{2026}\u{2022}\u{00b7}";
        // Latin Extended-A accented characters, common symbols, and list markers.
        const EXTENDED_PREWARM: &str = "\
            \u{00e0}\u{00e1}\u{00e2}\u{00e3}\u{00e4}\u{00e5}\u{00e6}\u{00e7}\
            \u{00e8}\u{00e9}\u{00ea}\u{00eb}\u{00ec}\u{00ed}\u{00ee}\u{00ef}\
            \u{00f0}\u{00f1}\u{00f2}\u{00f3}\u{00f4}\u{00f5}\u{00f6}\u{00f9}\
            \u{00fa}\u{00fb}\u{00fc}\u{00fd}\u{00fe}\u{00ff}\
            \u{00c0}\u{00c1}\u{00c2}\u{00c3}\u{00c4}\u{00c5}\u{00c6}\u{00c7}\
            \u{00c8}\u{00c9}\u{00ca}\u{00cb}\u{00cc}\u{00cd}\u{00ce}\u{00cf}\
            \u{00d0}\u{00d1}\u{00d2}\u{00d3}\u{00d4}\u{00d5}\u{00d6}\u{00d9}\
            \u{00da}\u{00db}\u{00dc}\u{00dd}\u{00de}\
            \u{20ac}\u{00a3}\u{00a5}\u{00a9}\u{00ae}\u{2122}\u{00b0}\u{00b1}\
            \u{00d7}\u{00f7}\u{2026}\u{2014}\u{2013}\u{2018}\u{2019}\u{201c}\
            \u{201d}\u{00ab}\u{00bb}\u{00bf}\u{00a1}\
            \u{2022}\u{25e6}\u{25aa}\u{25b8}\u{25b9}";
        for ch in PREWARM_CHARS.chars().chain(EXTENDED_PREWARM.chars()) {
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

    /// Insert a shadow into the cache, evicting the oldest half when at capacity.
    pub(crate) fn shadow_cache_insert(&mut self, node_id: NodeId, shadow: CachedShadow) {
        if self.shadow_cache.len() >= MAX_SHADOW_CACHE {
            let to_remove: Vec<NodeId> = self
                .shadow_cache
                .keys()
                .take(MAX_SHADOW_CACHE / 2)
                .copied()
                .collect();
            for id in to_remove {
                self.shadow_cache.remove(&id);
            }
        }
        self.shadow_cache.insert(node_id, shadow);
    }

    /// Trim all internal caches to reduce memory usage.
    pub fn trim_caches(&mut self) {
        if self.shadow_cache.len() > MAX_SHADOW_CACHE / 2 {
            let to_remove: Vec<NodeId> = self
                .shadow_cache
                .keys()
                .take(self.shadow_cache.len() / 2)
                .copied()
                .collect();
            for id in to_remove {
                self.shadow_cache.remove(&id);
            }
        }
        self.blur_worker.trim_cache();
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

    /// Set the per-frame render budget (in ms).
    pub fn set_blur_budget_ms(&mut self, budget: f64) {
        self.blur_budget_ms = budget;
    }

    /// Report the most recent frame's render time so the renderer can
    /// adaptively toggle blur.
    pub fn report_render_time(&mut self, render_ms: f64) {
        const ALPHA: f64 = 0.2;
        if self.avg_render_ms <= 0.0 {
            self.avg_render_ms = render_ms;
        } else {
            self.avg_render_ms = ALPHA * render_ms + (1.0 - ALPHA) * self.avg_render_ms;
        }

        if self.blur_enabled && self.avg_render_ms > self.blur_budget_ms {
            self.blur_enabled = false;
            self.blur_worker.clear_cache();
        }
        if !self.blur_enabled && self.avg_render_ms < self.blur_budget_ms * 0.25 {
            self.blur_enabled = true;
        }

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
    pub fn register_image(&mut self, image_id: u64, data: &[u8]) -> Result<(), String> {
        let decoded = crate::image_decode::decode_image(data)
            .map_err(|e| format!("Image decode error: {}", e))?;

        let key = crate::texture_cache::image_texture_key(image_id);
        self.texture_cache
            .insert_by_key(key, decoded.pixels, decoded.width, decoded.height);
        Ok(())
    }

    /// Register a pre-decoded RGBA8 image.
    pub fn register_image_rgba(&mut self, image_id: u64, pixels: Vec<u8>, width: u32, height: u32) {
        let key = crate::texture_cache::image_texture_key(image_id);
        self.texture_cache.insert_by_key(key, pixels, width, height);
    }

    /// Check if an image is loaded.
    #[must_use]
    pub fn has_image(&mut self, image_id: u64) -> bool {
        let key = crate::texture_cache::image_texture_key(image_id);
        self.texture_cache.get_by_key(key).is_some()
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
        self.invalidate_all_layouts();
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
    ) -> liquide_compositor::RenderResult<Vec<DamageTile>> {
        // Reset pending-glyph tracker for this frame.
        self.has_pending_glyphs = false;

        // Reset the active blend mode to default for this frame.
        self.active_blend_mode = BlendMode::SrcOver;

        // Drain any completed async blur results before rendering.
        self.blur_worker.poll_results();

        // Drain completed glyph rasterizations into the atlas.
        let rasterized = self.font_worker.poll_results();
        for glyph in &rasterized {
            let _ = self
                .glyph_atlas
                .insert(glyph.key, &glyph.bitmap, &glyph.metrics);
        }

        // Compute damage bounding box in pixel coordinates for early culling.
        // Nodes fully outside the damaged region are skipped since only damaged
        // tiles will be blitted to the final output.
        let damage_bbox = if damage.tiles.is_empty() {
            None
        } else {
            let ts = damage.tile_size as f32;
            // Padding accounts for effects (blur, shadow) that extend beyond
            // the node's nominal bounds.
            let padding = 32.0_f32;
            let min_x = damage.tiles.iter().map(|t| t.x).min().unwrap_or(0) as f32 * ts - padding;
            let min_y = damage.tiles.iter().map(|t| t.y).min().unwrap_or(0) as f32 * ts - padding;
            let max_x =
                (damage.tiles.iter().map(|t| t.x).max().unwrap_or(0) as f32 + 1.0) * ts + padding;
            let max_y =
                (damage.tiles.iter().map(|t| t.y).max().unwrap_or(0) as f32 + 1.0) * ts + padding;
            Some((min_x, min_y, max_x, max_y))
        };

        // Render each node exactly once in z-order.
        for node in nodes {
            // Skip nodes completely outside the damage bounding box.
            if let Some((dx0, dy0, dx1, dy1)) = damage_bbox {
                let b = &node.absolute_bounds;
                if b.x >= dx1 || b.y >= dy1 || b.x + b.width <= dx0 || b.y + b.height <= dy0 {
                    continue;
                }
            }

            let distance = self.calculate_distance_from_center(&node.absolute_bounds);
            let lod_level = self.select_lod(node, distance);

            self.render_node_with_lod(node, fb, lod_level);
        }

        // Return value is unused by all call sites (`let _ = renderer.render(...)`)
        // so we avoid cloning the entire damage tiles Vec.
        Ok(Vec::new())
    }

    fn blur_enabled(&self) -> bool {
        self.blur_enabled
    }

    fn set_blur_enabled(&mut self, enabled: bool) {
        self.blur_enabled = enabled;
    }

    fn has_pending_glyphs(&self) -> bool {
        self.has_pending_glyphs
    }

    fn report_render_time(&mut self, ms: f64) {
        let alpha = 0.1;
        self.avg_render_ms = self.avg_render_ms * (1.0 - alpha) + ms * alpha;
        if self.blur_enabled && self.avg_render_ms > self.blur_budget_ms {
            self.blur_enabled = false;
        } else if !self.blur_enabled && self.avg_render_ms < self.blur_budget_ms * 0.5 {
            self.blur_enabled = true;
        }
    }

    fn set_skeleton_window(&mut self, window_id: Option<u64>) {
        self.skeleton_window = window_id;
    }

    fn get_quality_mode(&self) -> liquide_compositor::RenderQuality {
        match self.lod_manager.get_performance_mode() {
            crate::lod::PerformanceMode::Quality => liquide_compositor::RenderQuality::Quality,
            crate::lod::PerformanceMode::Balanced => liquide_compositor::RenderQuality::Balanced,
            crate::lod::PerformanceMode::Performance => liquide_compositor::RenderQuality::Performance,
        }
    }

    fn set_quality_mode(&mut self, mode: liquide_compositor::RenderQuality) {
        let lod_mode = match mode {
            liquide_compositor::RenderQuality::Quality => crate::lod::PerformanceMode::Quality,
            liquide_compositor::RenderQuality::Balanced => crate::lod::PerformanceMode::Balanced,
            liquide_compositor::RenderQuality::Performance => crate::lod::PerformanceMode::Performance,
        };
        self.lod_manager.set_performance_mode(lod_mode);
    }
}

impl SoftwareRenderer {
    /// Render a single flattened node into the frame buffer with LOD support.
    fn render_node_with_lod(&mut self, node: &FlatNode, fb: &mut FrameBuffer, lod_level: LodLevel) {
        // Compute the visible (clipped) region if a clip rect is set.
        let bounds = node.absolute_bounds;
        if let Some(ref clip) = node.clip {
            let right = bounds.right().min(clip.right());
            let bottom = bounds.bottom().min(clip.bottom());
            let vis_x = bounds.x.max(clip.x);
            let vis_y = bounds.y.max(clip.y);
            if right <= vis_x || bottom <= vis_y {
                return; // Fully clipped
            }
        }
        let opacity = node.opacity;

        // Apply LOD quality factor to certain effects
        let quality_factor = lod_level.quality_factor();

        match &node.kind {
            SceneNodeKind::Background { color } => {
                let mut c = *color;
                if opacity < 1.0 {
                    c.a = (c.a as f32 * opacity + 0.5) as u8;
                }
                let blend = self.active_blend_mode;
                let (r_tl, r_tr, r_br, r_bl) = node.corner_radius;
                let has_radius = r_tl > 0.5 || r_tr > 0.5 || r_br > 0.5 || r_bl > 0.5;
                if has_radius {
                    self.fill_rounded_rect_per_corner(fb, bounds, c, r_tl, r_tr, r_br, r_bl, blend);
                } else if c.a == 255 && blend == BlendMode::SrcOver {
                    rasterizer::fill_rect(fb, bounds, c, BlendMode::Src);
                } else {
                    rasterizer::fill_rect(fb, bounds, c, blend);
                }
            }

            SceneNodeKind::Surface { buffer, .. } | SceneNodeKind::ChildSurface { buffer, .. } => {
                if let Some(buf) = buffer {
                    if opacity >= 1.0 && buf.format == liquide_compositor::pixel::PixelFormat::Bgra8
                    {
                        rasterizer::blit_opaque_stride(
                            fb,
                            &buf.pixels,
                            buf.width,
                            buf.height,
                            buf.stride as usize,
                            bounds.x.max(0.0) as u32,
                            bounds.y.max(0.0) as u32,
                        );
                    } else {
                        rasterizer::blit_alpha_stride(
                            fb,
                            &buf.pixels,
                            buf.width,
                            buf.height,
                            buf.stride as usize,
                            bounds.x.max(0.0) as u32,
                            bounds.y.max(0.0) as u32,
                            opacity,
                        );
                    }
                }
            }

            SceneNodeKind::Glass(_) => {
                self.render_glass_node(node, fb, lod_level, quality_factor);
            }

            SceneNodeKind::Tint { color } => {
                let mut c = *color;
                c.a = (c.a as f32 * opacity + 0.5) as u8;
                rasterizer::fill_rect(fb, bounds, c, BlendMode::Multiply);
            }

            SceneNodeKind::Shadow { .. } => {
                self.render_shadow_node(node, fb, lod_level, quality_factor);
            }

            SceneNodeKind::Decoration { .. } => {
                self.render_decoration_node(node, fb);
            }

            SceneNodeKind::BlurBackdrop => {
                if self.blur_enabled && self.intersects_dirty(&bounds) {
                    let radius = self.effect_params.blur_radius;
                    if radius > 0 {
                        self.render_backdrop_blur(node.id, bounds, radius, fb);
                    }
                }
            }

            SceneNodeKind::BlurCache => {
                if self.blur_enabled && self.intersects_dirty(&bounds) {
                    let radius = self.effect_params.blur_radius;
                    if radius > 0 {
                        self.render_backdrop_blur(node.id, bounds, radius, fb);
                    }
                }
            }

            SceneNodeKind::Content | SceneNodeKind::Overlay | SceneNodeKind::ShellLayer => {
                if opacity < 1.0 {
                    // Multiply alpha of existing pixels in the region
                    let x0 = (bounds.x.max(0.0) as u32).min(fb.width);
                    let y0 = (bounds.y.max(0.0) as u32).min(fb.height);
                    let x1 = (bounds.right().ceil() as u32).min(fb.width);
                    let y1 = (bounds.bottom().ceil() as u32).min(fb.height);
                    for y in y0..y1 {
                        for x in x0..x1 {
                            let mut px = fb.get_pixel(x, y);
                            px.r = (px.r as f32 * opacity + 0.5) as u8;
                            px.g = (px.g as f32 * opacity + 0.5) as u8;
                            px.b = (px.b as f32 * opacity + 0.5) as u8;
                            px.a = (px.a as f32 * opacity + 0.5) as u8;
                            fb.set_pixel(x, y, px);
                        }
                    }
                }
            }

            SceneNodeKind::Cursor { .. } => {
                self.render_cursor_node(node, fb);
            }

            SceneNodeKind::LockScreen => {
                if self.blur_enabled {
                    let radius = self.effect_params.blur_radius;
                    if radius > 0 {
                        self.render_backdrop_blur(node.id, bounds, radius, fb);
                    }
                }
                rasterizer::fill_rect(fb, bounds, Color::new(0, 0, 0, 180), BlendMode::SrcOver);
            }

            SceneNodeKind::CrashScreen => {
                let crash_color = Color::new(180, 0, 0, 200);
                rasterizer::fill_rect(fb, bounds, crash_color, BlendMode::SrcOver);
            }

            SceneNodeKind::Root | SceneNodeKind::Workspace { .. } => {}

            SceneNodeKind::Text { .. } => {
                self.render_text_node(node, fb);
            }

            SceneNodeKind::Icon { icon_id, color } => {
                let mut c = *color;
                if opacity < 1.0 {
                    c.a = (c.a as f32 * opacity + 0.5) as u8;
                }
                crate::icons::draw_icon(fb, *icon_id, bounds, c, &self.srgb_lut);
            }

            SceneNodeKind::BackdropFilter { .. } => {
                self.render_backdrop_filter_node(node, fb, lod_level, quality_factor);
            }

            SceneNodeKind::Filter { .. } => {
                self.render_filter_node(node, fb);
            }

            SceneNodeKind::GradientFill { gradient } => {
                self.render_gradient(fb, bounds, gradient, opacity, node.corner_radius);
            }

            SceneNodeKind::SvgPath { d, fill, stroke, stroke_width } => {
                use liquide_paint::svg_path::flatten_path_cached;
                let segments = flatten_path_cached(d);
                if let Some(fill_color) = fill {
                    let mut fc = *fill_color;
                    if opacity < 1.0 {
                        fc.a = (fc.a as f32 * opacity + 0.5) as u8;
                    }
                    if !segments.is_empty() {
                        let ox = bounds.x;
                        let oy = bounds.y;
                        for seg in &segments {
                            let r = Rect::new(
                                ox + seg.x1.min(seg.x2),
                                oy + seg.y1.min(seg.y2),
                                (seg.x2 - seg.x1).abs().max(1.0),
                                (seg.y2 - seg.y1).abs().max(1.0),
                            );
                            rasterizer::fill_rect(fb, r, fc, BlendMode::SrcOver);
                        }
                    }
                }
                if *stroke_width > 0.0 {
                    let mut sc = *stroke;
                    if opacity < 1.0 {
                        sc.a = (sc.a as f32 * opacity + 0.5) as u8;
                    }
                    let ox = bounds.x;
                    let oy = bounds.y;
                    for seg in &segments {
                        rasterizer::draw_line(
                            fb,
                            ox + seg.x1, oy + seg.y1,
                            ox + seg.x2, oy + seg.y2,
                            sc,
                            *stroke_width,
                        );
                    }
                }
            }

            SceneNodeKind::BackgroundFill { .. } => {
                self.render_background_fill_node(node, fb);
            }

            SceneNodeKind::ClipPath { clip_kind } => {
                use liquide_compositor::scene::ClipPathKind;
                match clip_kind {
                    ClipPathKind::RoundedRect { corner_radius } => {
                        let r = *corner_radius;
                        let bx0 = (bounds.x.max(0.0) as u32).min(fb.width);
                        let by0 = (bounds.y.max(0.0) as u32).min(fb.height);
                        let bx1 = (bounds.right().ceil() as u32).min(fb.width);
                        let by1 = (bounds.bottom().ceil() as u32).min(fb.height);
                        for y in by0..by1 {
                            let fy = y as f32 + 0.5;
                            for x in bx0..bx1 {
                                let fx = x as f32 + 0.5;
                                let d = rasterizer::sdf_rounded_rect_per_corner(
                                    fx, fy, &bounds, r, r, r, r,
                                );
                                let coverage = (-d + 0.5).clamp(0.0, 1.0);
                                if coverage >= 1.0 {
                                    continue;
                                }
                                let mut px = fb.get_pixel(x, y);
                                if coverage <= 0.0 {
                                    px = Color { r: 0, g: 0, b: 0, a: 0 };
                                } else {
                                    // Premultiplied alpha: scale all channels by coverage
                                    // to avoid dark halos at anti-aliased edges.
                                    px.r = (px.r as f32 * coverage + 0.5) as u8;
                                    px.g = (px.g as f32 * coverage + 0.5) as u8;
                                    px.b = (px.b as f32 * coverage + 0.5) as u8;
                                    px.a = (px.a as f32 * coverage + 0.5) as u8;
                                }
                                fb.set_pixel(x, y, px);
                            }
                        }
                    }
                    ClipPathKind::Circle {
                        center_x,
                        center_y,
                        radius,
                    } => {
                        let cx = bounds.x + center_x * bounds.width;
                        let cy = bounds.y + center_y * bounds.height;
                        let r = radius * bounds.width.min(bounds.height);
                        let bx0 = (bounds.x.max(0.0) as u32).min(fb.width);
                        let by0 = (bounds.y.max(0.0) as u32).min(fb.height);
                        let bx1 = (bounds.right().ceil() as u32).min(fb.width);
                        let by1 = (bounds.bottom().ceil() as u32).min(fb.height);
                        for y in by0..by1 {
                            let fy = y as f32 + 0.5;
                            for x in bx0..bx1 {
                                let fx = x as f32 + 0.5;
                                let d = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt() - r;
                                let coverage = (-d + 0.5).clamp(0.0, 1.0);
                                if coverage >= 1.0 { continue; }
                                let mut px = fb.get_pixel(x, y);
                                if coverage <= 0.0 {
                                    px = Color { r: 0, g: 0, b: 0, a: 0 };
                                } else {
                                    px.r = (px.r as f32 * coverage + 0.5) as u8;
                                    px.g = (px.g as f32 * coverage + 0.5) as u8;
                                    px.b = (px.b as f32 * coverage + 0.5) as u8;
                                    px.a = (px.a as f32 * coverage + 0.5) as u8;
                                }
                                fb.set_pixel(x, y, px);
                            }
                        }
                    }
                    ClipPathKind::Ellipse {
                        center_x,
                        center_y,
                        rx,
                        ry,
                    } => {
                        let cx = bounds.x + center_x * bounds.width;
                        let cy = bounds.y + center_y * bounds.height;
                        let erx = rx * bounds.width;
                        let ery = ry * bounds.height;
                        let bx0 = (bounds.x.max(0.0) as u32).min(fb.width);
                        let by0 = (bounds.y.max(0.0) as u32).min(fb.height);
                        let bx1 = (bounds.right().ceil() as u32).min(fb.width);
                        let by1 = (bounds.bottom().ceil() as u32).min(fb.height);
                        for y in by0..by1 {
                            let fy = y as f32 + 0.5;
                            for x in bx0..bx1 {
                                let fx = x as f32 + 0.5;
                                let nx = (fx - cx) / erx;
                                let ny = (fy - cy) / ery;
                                let d = (nx * nx + ny * ny).sqrt() - 1.0;
                                let coverage = (-d * erx.min(ery) + 0.5).clamp(0.0, 1.0);
                                if coverage >= 1.0 { continue; }
                                let mut px = fb.get_pixel(x, y);
                                if coverage <= 0.0 {
                                    px = Color { r: 0, g: 0, b: 0, a: 0 };
                                } else {
                                    px.r = (px.r as f32 * coverage + 0.5) as u8;
                                    px.g = (px.g as f32 * coverage + 0.5) as u8;
                                    px.b = (px.b as f32 * coverage + 0.5) as u8;
                                    px.a = (px.a as f32 * coverage + 0.5) as u8;
                                }
                                fb.set_pixel(x, y, px);
                            }
                        }
                    }
                    ClipPathKind::Polygon { points } => {
                        if points.len() < 3 { /* skip degenerate polygon */ }
                        else {
                            let bx0 = (bounds.x.max(0.0) as u32).min(fb.width);
                            let by0 = (bounds.y.max(0.0) as u32).min(fb.height);
                            let bx1 = (bounds.right().ceil() as u32).min(fb.width);
                            let by1 = (bounds.bottom().ceil() as u32).min(fb.height);
                            let pts: Vec<(f32, f32)> = points
                                .iter()
                                .map(|p| (bounds.x + p.0 * bounds.width, bounds.y + p.1 * bounds.height))
                                .collect();
                            for y in by0..by1 {
                                let fy = y as f32 + 0.5;
                                for x in bx0..bx1 {
                                    let fx = x as f32 + 0.5;
                                    // Winding number test
                                    let mut winding = 0i32;
                                    // Minimum signed distance to nearest edge (for AA)
                                    let mut min_dist_sq = f32::MAX;
                                    for i in 0..pts.len() {
                                        let j = (i + 1) % pts.len();
                                        let (x0, y0) = pts[i];
                                        let (x1, y1) = pts[j];
                                        if y0 <= fy {
                                            if y1 > fy && ((x1 - x0) * (fy - y0) - (fx - x0) * (y1 - y0)) > 0.0 {
                                                winding += 1;
                                            }
                                        } else if y1 <= fy && ((x1 - x0) * (fy - y0) - (fx - x0) * (y1 - y0)) < 0.0 {
                                            winding -= 1;
                                        }
                                        // Point-to-segment distance squared
                                        let ex = x1 - x0;
                                        let ey = y1 - y0;
                                        let len_sq = ex * ex + ey * ey;
                                        let t = if len_sq > 0.0 {
                                            ((fx - x0) * ex + (fy - y0) * ey) / len_sq
                                        } else {
                                            0.0
                                        }.clamp(0.0, 1.0);
                                        let px = x0 + t * ex - fx;
                                        let py = y0 + t * ey - fy;
                                        min_dist_sq = min_dist_sq.min(px * px + py * py);
                                    }
                                    let dist = min_dist_sq.sqrt();
                                    let inside = winding != 0;
                                    let signed_dist = if inside { dist } else { -dist };
                                    let coverage = (signed_dist + 0.5).clamp(0.0, 1.0);
                                    if coverage >= 1.0 { continue; }
                                    let mut px = fb.get_pixel(x, y);
                                    if coverage <= 0.0 {
                                        px = Color { r: 0, g: 0, b: 0, a: 0 };
                                    } else {
                                        px.r = (px.r as f32 * coverage + 0.5) as u8;
                                        px.g = (px.g as f32 * coverage + 0.5) as u8;
                                        px.b = (px.b as f32 * coverage + 0.5) as u8;
                                        px.a = (px.a as f32 * coverage + 0.5) as u8;
                                    }
                                    fb.set_pixel(x, y, px);
                                }
                            }
                        }
                    }
                }
            }

            SceneNodeKind::BorderImage { .. } => {
                self.render_border_image_node(node, fb);
            }

            SceneNodeKind::Mask { mask } => {
                use liquide_compositor::scene::{MaskMode, MaskSpec};
                let x0 = (bounds.x.max(0.0) as u32).min(fb.width);
                let y0 = (bounds.y.max(0.0) as u32).min(fb.height);
                let x1 = (bounds.right().ceil() as u32).min(fb.width);
                let y1 = (bounds.bottom().ceil() as u32).min(fb.height);
                if x0 >= x1 || y0 >= y1 {
                    return;
                }
                match mask {
                    MaskSpec::Gradient { gradient, mode } => {
                        // Evaluate the gradient at each pixel and use its
                        // luminance or alpha channel as a mask multiplier.
                        for y in y0..y1 {
                            let fy = y as f32 + 0.5;
                            for x in x0..x1 {
                                let fx = x as f32 + 0.5;
                                let t = gradient_t(gradient, fx, fy, &bounds);
                                let stops = gradient_stops(gradient);
                                let mc = gradients::sample_gradient_stops(stops, t, 1.0);
                                let mask_alpha = match mode {
                                    MaskMode::Alpha | MaskMode::MatchSource => mc.a,
                                    MaskMode::Luminance => {
                                        // ITU-R BT.709 luminance
                                        let lum = 0.2126 * mc.r as f32
                                            + 0.7152 * mc.g as f32
                                            + 0.0722 * mc.b as f32;
                                        (lum / 255.0 * mc.a as f32 + 0.5) as u8
                                    }
                                };
                                let alpha_f = mask_alpha as f32 / 255.0 * opacity;
                                if alpha_f >= 1.0 {
                                    continue;
                                }
                                let mut px = fb.get_pixel(x, y);
                                px.r = (px.r as f32 * alpha_f + 0.5) as u8;
                                px.g = (px.g as f32 * alpha_f + 0.5) as u8;
                                px.b = (px.b as f32 * alpha_f + 0.5) as u8;
                                px.a = (px.a as f32 * alpha_f + 0.5) as u8;
                                fb.set_pixel(x, y, px);
                            }
                        }
                    }
                    MaskSpec::Image { mode, .. } => {
                        // Image mask requires texture lookup.  Without it,
                        // fall back to opacity-based uniform alpha.
                        let alpha_f = opacity;
                        let _ = mode;
                        if alpha_f < 1.0 {
                            for y in y0..y1 {
                                for x in x0..x1 {
                                    let mut px = fb.get_pixel(x, y);
                                    px.r = (px.r as f32 * alpha_f + 0.5) as u8;
                                    px.g = (px.g as f32 * alpha_f + 0.5) as u8;
                                    px.b = (px.b as f32 * alpha_f + 0.5) as u8;
                                    px.a = (px.a as f32 * alpha_f + 0.5) as u8;
                                    fb.set_pixel(x, y, px);
                                }
                            }
                        }
                    }
                }
            }

            SceneNodeKind::RenderLayer { blend_mode, isolate } => {
                // Unconditionally set the blend mode so that a normal
                // (SrcOver) layer resets the mode after a previous
                // non-default layer.  True isolation would require
                // rendering children into a temp buffer, but the flat
                // node list has no end-of-layer marker.
                self.active_blend_mode = *blend_mode;
                let _ = isolate;
            }

            SceneNodeKind::Border { .. } => {
                self.render_border_node(node, fb);
            }

            SceneNodeKind::BoxShadows { .. } => {
                self.render_box_shadows_node(node, fb, lod_level, quality_factor);
            }

            SceneNodeKind::Image { .. } => {
                self.render_image_node(node, fb);
            }

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

            SceneNodeKind::SelectionOverlay {
                fill,
                border_color,
                border_width,
            } => {
                let mut fc = *fill;
                if opacity < 1.0 {
                    fc.a = (fc.a as f32 * opacity + 0.5) as u8;
                }
                if fc.a > 0 {
                    rasterizer::fill_rect(fb, bounds, fc, BlendMode::SrcOver);
                }
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
}

// ── Mask gradient helpers ───────────────────────────────────────────

/// Compute the gradient parameter `t` ∈ [0, 1] for a pixel at `(fx, fy)`
/// within `bounds`, given a `GradientSpec`.
fn gradient_t(
    gradient: &liquide_compositor::scene::GradientSpec,
    fx: f32,
    fy: f32,
    bounds: &Rect,
) -> f32 {
    use liquide_compositor::scene::GradientSpec;
    match gradient {
        GradientSpec::Linear {
            start_x,
            start_y,
            end_x,
            end_y,
            ..
        } => {
            let sx = bounds.x + start_x * bounds.width;
            let sy = bounds.y + start_y * bounds.height;
            let ex = bounds.x + end_x * bounds.width;
            let ey = bounds.y + end_y * bounds.height;
            let dx = ex - sx;
            let dy = ey - sy;
            let len2 = dx * dx + dy * dy;
            if len2 < 0.001 {
                return 0.0;
            }
            (((fx - sx) * dx + (fy - sy) * dy) / len2).clamp(0.0, 1.0)
        }
        GradientSpec::Radial {
            center_x,
            center_y,
            radius,
            radius_y,
            ..
        } => {
            let cx = bounds.x + center_x * bounds.width;
            let cy = bounds.y + center_y * bounds.height;
            let min_dim = bounds.width.min(bounds.height);
            let rx = radius * min_dim;
            let ry = radius_y * min_dim;
            if rx <= 0.0 || ry <= 0.0 {
                return 0.0;
            }
            let dx = fx - cx;
            let dy = fy - cy;
            ((dx * dx / (rx * rx) + dy * dy / (ry * ry)).sqrt()).clamp(0.0, 1.0)
        }
        GradientSpec::Conic {
            center_x,
            center_y,
            start_angle,
            ..
        } => {
            let cx = bounds.x + center_x * bounds.width;
            let cy = bounds.y + center_y * bounds.height;
            let mut angle = (fy - cy).atan2(fx - cx) - start_angle.to_radians();
            if angle < 0.0 {
                angle += std::f32::consts::TAU;
            }
            (angle / std::f32::consts::TAU).clamp(0.0, 1.0)
        }
        GradientSpec::Mesh { .. } => 0.5,
    }
}

/// Extract the color stops slice from a `GradientSpec`.
fn gradient_stops(gradient: &liquide_compositor::scene::GradientSpec) -> &[(f32, Color)] {
    use liquide_compositor::scene::GradientSpec;
    match gradient {
        GradientSpec::Linear { stops, .. }
        | GradientSpec::Radial { stops, .. }
        | GradientSpec::Conic { stops, .. } => stops,
        GradientSpec::Mesh { .. } => &[],
    }
}

// ── Word splitting for text wrapping ────────────────────────────────

/// Splits text into chunks suitable for word-wrapping.
///
/// Each yielded chunk is either a run of non-space characters (a "word")
/// or a run of spaces. The caller can decide where to break by checking
/// whether appending the next word would exceed the line width.
///
/// Example: `"Hello  World"` yields `["Hello", "  ", "World"]`.
pub(crate) struct WordSplitter<'a> {
    remaining: &'a str,
}

impl<'a> WordSplitter<'a> {
    pub(crate) fn new(text: &'a str) -> Self {
        Self { remaining: text }
    }
}

impl<'a> Iterator for WordSplitter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        if self.remaining.is_empty() {
            return None;
        }
        let bytes = self.remaining.as_bytes();
        let is_space = bytes[0] == b' ';
        let end = self.remaining
            .char_indices()
            .skip(1)
            .find(|(_, ch)| (*ch == ' ') != is_space)
            .map(|(i, _)| i)
            .unwrap_or(self.remaining.len());
        let chunk = &self.remaining[..end];
        self.remaining = &self.remaining[end..];
        Some(chunk)
    }
}
