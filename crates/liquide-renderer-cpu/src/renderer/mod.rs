//! Main renderer trait and software renderer implementation.

mod borders;
mod cursors;
pub use cursors::CursorTheme;
mod decoration;
mod effects;
mod gradients;
mod helpers;
mod images;
mod occlusion;
#[cfg(test)]
pub(crate) use occlusion::{reset_cull_probe, was_culled};
/// Surface-cache raster helpers (t2-surface-cache E2): raster an opaque subtree
/// to a cacheable [`liquide_compositor::scene::SurfaceBuffer`], and the glass
/// backdrop-CRC / re-blur-in-place path. Declared here (mod.rs is this
/// executor's lock) so it sits beside the renderer it extends.
mod surface;
mod text;
/// Live text shaping (rustybuzz OpenType + Unicode bidi + multi-font fallback)
/// wired into the text-draw path. Declared from `mod.rs` (which this executor
/// owns) so it sits beside `text.rs` rather than under a `text/` directory.
pub(crate) mod text_shaping;
#[cfg(test)]
pub(crate) use text::{compute_font_id, compute_shaped_font_id};

use std::collections::HashMap;

use liquide_compositor::damage::{DamageClass, DamageSet, DamageTile};
use liquide_compositor::effects::EffectParams;
use liquide_compositor::framebuffer::{FrameBuffer, FrameMemory};
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::{BlendMode, Color, PixelFormat};
use liquide_compositor::scene::{FlatNode, NodeId, SceneNodeKind};
use liquide_compositor::{
    FrameMemoryKind, RendererBackendInfo, RendererBackendKind, RendererCapabilities,
    RendererNegotiation, RendererRejectReason,
};
use liquide_font_rasterizer::database::{FontDatabase, FontFaceId};

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

/// Cached chrome box-shadow mask, keyed by a signature of EVERY input that
/// affects the generated mask (surface geometry, corner radius, spread, blur,
/// offset, colour, and the framebuffer dimensions used to clamp it).
///
/// Unlike the per-node window [`CachedShadow`] (keyed by `NodeId` + bounds), the
/// chrome `BoxShadows` nodes are rebuilt every frame with churning ids, so this
/// cache is keyed purely on the mask's INPUTS. A steady chrome shadow (statusbar,
/// dock) hits the cache and reuses the once-computed SDF + blur mask; any change
/// to a keyed input yields a different signature and forces a fresh compute, so a
/// stale mask can never paint. Only used on the FULL-frame path (no write-scissor);
/// a scissored frame computes a clip-confined mask fresh (never cached), exactly
/// like the window Shadow path.
pub(crate) struct CachedBoxShadow {
    /// Signature of all mask inputs (see [`box_shadow_mask_key`]).
    key: u64,
    /// Position-INDEPENDENT signature of the mask's SHAPE (every keyed input
    /// EXCEPT `surface_rect.x`/`.y`: width, height, corner radius, spread, blur,
    /// colour, offset, fb dims). Two cache entries with the same `shape_key`
    /// differ only in where the surface sits — so a moving window's identical
    /// shadow shape can be REUSED by translating the cached mask (t179) instead
    /// of regenerating the SDF + blur every drag frame.
    shape_key: u64,
    /// Whether the mask rect was fully inside `[0,fb)` when generated — i.e. no
    /// edge pixel was clamped by `generate_shadow_mask`'s `.max(0)`/`.min(fb)`.
    /// A translate is only byte-identical to a fresh compute when BOTH the cached
    /// generation AND the new position are unclamped (no edge truncation at
    /// either spot); otherwise the dimensions/edges differ and we must regenerate.
    unclamped: bool,
    /// The pre-computed mask.
    mask: ShadowMask,
}

/// Maximum number of distinct chrome box-shadow masks cached before eviction.
const MAX_BOX_SHADOW_CACHE: usize = 64;

/// An OPEN `clip-path` scope (t149).
///
/// The scene bridge brackets a clipped element's OWN draws between a paired
/// BEGIN and APPLY `ClipPath` marker (both carrying the same shape + bounds). On
/// BEGIN the renderer snapshots the pixels under the scope's bounds window
/// (already-painted siblings / background); on the matching APPLY it masks the
/// element's content to the shape and restores this snapshot for every pixel the
/// shape excludes — confining the clip to the element's own subtree instead of
/// destroying earlier siblings' pixels (the pre-t149 single-flat-mask bug).
struct ClipScope {
    /// Structural identity of the scope's shape + bounds, used to pair the BEGIN
    /// marker with its matching APPLY marker.
    identity: (u8, Vec<u32>, [u32; 4]),
    /// Pixel window the snapshot covers: (x0, y0, x1, y1) in framebuffer coords,
    /// already clamped to the framebuffer and the active write-scissor.
    window: (u32, u32, u32, u32),
    /// Snapshot of `window` as a row-major BGRA8 `Color` grid (width = x1-x0).
    snapshot: Vec<Color>,
}

/// An OPEN isolated render layer (group opacity / isolation).
///
/// A `RenderLayer { isolate: true }` scene node marks the start of a compositing
/// group: every node painted after it (until the layer closes) must be rendered
/// as ONE unit and then composited a single time at the group's opacity. The flat
/// node list carries no end-of-layer marker, so the layer is closed either when a
/// later layer at the same/shallower nesting opens (its bounds are not contained
/// in this one) or at the end of the node walk.
///
/// The isolation algorithm: on open, snapshot the window's backdrop pixels and
/// then CLEAR the window to transparent black so the group's children composite
/// onto an empty layer (not onto the backdrop). On close, the window holds the
/// fully-merged group content; composite it once over the saved backdrop weighted
/// by the group opacity. Because the children draw onto a clear layer at full
/// alpha and the group opacity is applied exactly once at the end, overlapping
/// children no longer double-darken the backdrop (each overlap is resolved WITHIN
/// the layer first, then the merged result is dimmed a single time).
struct LayerScope {
    /// Pixel window the layer covers: (x0, y0, x1, y1) in framebuffer coords,
    /// already clamped to the framebuffer and the active write-scissor.
    window: (u32, u32, u32, u32),
    /// Backdrop snapshot of `window` (row-major BGRA8, width = x1-x0), captured
    /// before the window was cleared so the layer can be composited over it.
    backdrop: Vec<Color>,
    /// Group opacity in `[0, 1]` applied ONCE when the layer is composited.
    opacity: f32,
}

/// A renderer-local structural identity for a `ClipPathKind` + bounds, used to
/// pair a BEGIN marker with its matching APPLY marker. `ClipPathKind` does not
/// derive `PartialEq` (and lives in `liquide-compositor`, out of this crate's
/// edit scope), so equality is computed here from the discriminant + fields +
/// the node bounds. Bit-equality of the `f32` fields is exactly what we want:
/// the bridge emits the begin/apply pair from the SAME values, so they compare
/// equal, while two distinct scopes essentially never collide.
fn clip_scope_identity(
    kind: &liquide_compositor::scene::ClipPathKind,
    bounds: &Rect,
) -> (u8, Vec<u32>, [u32; 4]) {
    use liquide_compositor::scene::ClipPathKind;
    let b = [
        bounds.x.to_bits(),
        bounds.y.to_bits(),
        bounds.width.to_bits(),
        bounds.height.to_bits(),
    ];
    match kind {
        ClipPathKind::Circle {
            center_x,
            center_y,
            radius,
        } => (
            0,
            vec![center_x.to_bits(), center_y.to_bits(), radius.to_bits()],
            b,
        ),
        ClipPathKind::RoundedRect { corner_radius } => (1, vec![corner_radius.to_bits()], b),
        ClipPathKind::Ellipse {
            center_x,
            center_y,
            rx,
            ry,
        } => (
            2,
            vec![
                center_x.to_bits(),
                center_y.to_bits(),
                rx.to_bits(),
                ry.to_bits(),
            ],
            b,
        ),
        ClipPathKind::Polygon { points } => {
            let mut v = Vec::with_capacity(points.len() * 2);
            for (px, py) in points {
                v.push(px.to_bits());
                v.push(py.to_bits());
            }
            (3, v, b)
        }
    }
}

/// Whether pixel window `inner` is fully contained within window `outer`.
/// Windows are `(x0, y0, x1, y1)` inclusive-exclusive in framebuffer coords.
/// Used to decide whether a newly-opened render layer is nested inside an
/// already-open one (kept open) or is a sibling/shallower layer (closes the
/// earlier one). A degenerate `outer` contains nothing but itself.
fn window_contains(outer: (u32, u32, u32, u32), inner: (u32, u32, u32, u32)) -> bool {
    outer.0 <= inner.0 && outer.1 <= inner.1 && outer.2 >= inner.2 && outer.3 >= inner.3
}

/// Upper bound (milliseconds) on how long the deterministic capture render
/// ([`SoftwareRenderer::render`]) will block waiting for already-in-flight glyph
/// rasterizations to complete before painting text. This makes glyph presence
/// deterministic for a given scene (identical atlas → byte-identical frame)
/// without risking an unbounded hang if the font-worker thread stalls; on
/// timeout the renderer falls back to the estimated-advance path exactly as it
/// did before this seam existed.
pub(crate) const GLYPH_DRAIN_BUDGET_MS: u64 = 2_000;

/// Per-frame glyph-drain budget (milliseconds) for the **live** render path
/// ([`SoftwareRenderer::render_live`] with [`RenderMode::LiveFull`]).
///
/// The live desktop loop runs a single render job in flight, so any time the
/// render thread spends block-draining glyphs is a direct present stall. Unlike
/// the capture path (which may block up to [`GLYPH_DRAIN_BUDGET_MS`] for
/// determinism), the live path commits whatever glyphs have arrived within this
/// tiny budget and signals `has_pending_glyphs` so the session schedules a
/// follow-up frame — text fills in within a frame or two instead of freezing
/// the desktop. Kept far below the 500 ms render watchdog so a single live
/// frame can never trip it (t68 cause #1 / C2).
///
/// At a 200 fps target the whole frame budget is ~5 ms, so even a few ms of
/// glyph-drain is a large tax. We keep a *tiny* 1 ms drain rather than going
/// fully non-blocking (returning no deadline): a 1 ms wait still lets glyphs
/// that finish almost immediately commit on the same frame, but guarantees text
/// commits within a bounded number of frames even if the session's
/// resubmit-on-`has_pending_glyphs` path ever regresses — whereas a pure
/// non-blocking poll would never commit text if that resubmit path broke (t77).
pub(crate) const LIVE_GLYPH_DRAIN_BUDGET_MS: u64 = 1;

// `RenderMode` is the single shared glyph-drain / liveness selector. It is
// defined in `liquide-compositor` (so the `Renderer` trait can name it without a
// backend dependency) and re-exported here so existing `RenderMode::*` call
// sites in this crate (and its tests) keep working unchanged.
pub use liquide_compositor::RenderMode;

/// Drain deadline for `mode`, or `None` for a pure non-blocking poll.
///
/// This is the ONLY behavioural difference between render modes: how long the
/// renderer waits for in-flight glyph rasterizations before painting text.
/// [`RenderMode::Capture`] block-drains up to [`GLYPH_DRAIN_BUDGET_MS`] for
/// determinism; [`RenderMode::LiveFull`] waits only [`LIVE_GLYPH_DRAIN_BUDGET_MS`];
/// [`RenderMode::LiveCursor`] never waits.
pub(crate) fn drain_deadline(mode: RenderMode) -> Option<std::time::Instant> {
    let budget_ms = match mode {
        RenderMode::Capture => GLYPH_DRAIN_BUDGET_MS,
        RenderMode::LiveFull => LIVE_GLYPH_DRAIN_BUDGET_MS,
        RenderMode::LiveCursor => return None,
    };
    Some(std::time::Instant::now() + std::time::Duration::from_millis(budget_ms))
}

// Re-export the Renderer trait from liquide-compositor so downstream crates
// can import it from either location.
pub use liquide_compositor::Renderer;

/// Controls whether the CPU renderer proactively queues common glyphs for a font.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GlyphPrewarmMode {
    /// Do not enqueue common glyphs ahead of visible text.
    Disabled,
    /// Queue common Latin glyphs when a font family/size is first encountered.
    #[default]
    CommonGlyphs,
}

/// Configuration knobs for [`SoftwareRenderer`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoftwareRendererOptions {
    /// Glyph prewarming behavior for newly seen font family/size combinations.
    pub glyph_prewarm: GlyphPrewarmMode,
}

impl Default for SoftwareRendererOptions {
    fn default() -> Self {
        Self {
            glyph_prewarm: GlyphPrewarmMode::CommonGlyphs,
        }
    }
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
    /// Chrome box-shadow mask cache — keyed on a signature of the mask inputs
    /// (geometry/radius/spread/blur/offset/colour/fb-size), NOT a node id, so a
    /// steady chrome drop-shadow (statusbar, dock) reuses its once-computed
    /// SDF + blur mask across frames instead of regenerating it every full frame.
    /// Bounded; invalidated implicitly because a changed input yields a new key.
    box_shadow_cache: Vec<CachedBoxShadow>,
    /// Background thread for async glyph rasterization.
    font_worker: FontWorker,
    /// Renderer behavior options.
    options: SoftwareRendererOptions,
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
    /// Stack of OPEN clip-path scopes (t149). A `clip-path` is now emitted by the
    /// scene bridge as a PAIRED begin/apply marker bracketing the clipped element's
    /// OWN draws. On the BEGIN marker the renderer snapshots the framebuffer region
    /// the scope covers; on the matching APPLY marker it masks the element's content
    /// to the shape AND restores the snapshot for pixels outside the shape — so the
    /// clip attenuates ONLY the element's own subtree, never the siblings painted
    /// underneath it (the pre-t149 flat mask zeroed those too). Each entry holds the
    /// scope identity (to pair begin↔apply) plus the snapshot window + pixels.
    clip_scopes: Vec<ClipScope>,
    /// Stack of OPEN isolated render layers (group opacity / isolation). A
    /// `RenderLayer { isolate: true }` with group opacity < 1 opens a layer here:
    /// the backdrop under the layer window is snapshotted and the window cleared
    /// so the group's children composite onto an empty layer at full alpha. The
    /// layer is closed (its content composited once at the group opacity over the
    /// saved backdrop) when a later same/shallower layer opens or at walk end.
    /// This makes overlapping children under a group opacity composite as a single
    /// unit instead of double-darkening the backdrop (the pre-fix stub discarded
    /// `isolate` and premultiplied each node's own alpha).
    layer_scopes: Vec<LayerScope>,
    /// Resolved cursor appearance (CSS seam). Defaults to the historic
    /// black-outline / white-fill, node-driven shape.
    cursor_theme: cursors::CursorTheme,
    /// Per-frame raster clip rectangle in pixel coordinates. When `Some`, every
    /// node's draw region is intersected with this rect so a near-idle frame only
    /// touches the damaged tiles instead of re-rastering the whole surface (t76).
    /// `None` = full-frame raster (resize / wallpaper change / full damage).
    raster_clip: Option<Rect>,
    /// When `true` (the deterministic capture path), backdrop blur is computed
    /// SYNCHRONOUSLY so a glass region's blur is always present and identical
    /// run-to-run, instead of depending on whether the async blur worker finished
    /// in time. Set per frame from the [`RenderMode`]; the live paths leave this
    /// `false` and keep the non-blocking async blur.
    deterministic_blur: bool,
    /// Test-only accounting of the number of backdrop-blur SOURCE pixels actually
    /// snapshotted + convolved on the most recent `render_backdrop_blur` call.
    /// Used by the cost regression to prove a tiny damage rect over glass shrinks
    /// the blur source area to ~O(damage + radius) rather than the full backdrop.
    /// Gated to test builds so it has zero production cost.
    #[cfg(test)]
    last_blur_source_px: std::cell::Cell<usize>,
}

impl SoftwareRenderer {
    /// Create a new software renderer with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::with_options(SoftwareRendererOptions::default())
    }

    /// Create a new software renderer with explicit options.
    #[must_use]
    pub fn with_options(options: SoftwareRendererOptions) -> Self {
        Self::from_font_worker(FontWorker::new(), 1024, options)
    }

    /// Create a renderer with a pre-loaded font database for real TrueType rendering.
    #[must_use]
    pub fn with_font_db(font_db: FontDatabase) -> Self {
        Self::with_font_db_and_options(font_db, SoftwareRendererOptions::default())
    }

    /// Create a renderer with a pre-loaded font database and explicit options.
    #[must_use]
    pub fn with_font_db_and_options(
        font_db: FontDatabase,
        options: SoftwareRendererOptions,
    ) -> Self {
        Self::from_font_worker(FontWorker::with_font_db(font_db), 2048, options)
    }

    fn from_font_worker(
        font_worker: FontWorker,
        glyph_atlas_size: u32,
        options: SoftwareRendererOptions,
    ) -> Self {
        Self {
            srgb_lut: SrgbLut::new(),
            glyph_atlas: GlyphAtlas::new(glyph_atlas_size, glyph_atlas_size),
            effect_params: EffectParams::for_profile(
                liquide_compositor::effects::QualityProfile::Balanced,
            ),
            blur_enabled: true,
            avg_render_ms: 0.0,
            blur_budget_ms: 16.0, // Target ~60fps render budget
            blur_worker: BlurWorker::new(),
            shadow_cache: HashMap::new(),
            box_shadow_cache: Vec::new(),
            font_worker,
            options,
            layout_cache: LayoutCacheManager::new(),
            texture_cache: TextureCache::new(),
            dirty_rects: DirtyRectManager::new(1920, 1080),
            lod_manager: LodManager::new(1920.0, 1080.0),
            buffer_pool: ObjectPool::new(64),
            skeleton_window: None,
            has_pending_glyphs: false,
            prewarmed_fonts: std::collections::HashSet::new(),
            active_blend_mode: BlendMode::SrcOver,
            clip_scopes: Vec::new(),
            layer_scopes: Vec::new(),
            cursor_theme: cursors::CursorTheme::default(),
            raster_clip: None,
            deterministic_blur: false,
            #[cfg(test)]
            last_blur_source_px: std::cell::Cell::new(0),
        }
    }

    /// Return the active renderer options.
    #[must_use]
    pub fn options(&self) -> SoftwareRendererOptions {
        self.options
    }

    fn common_glyph_prewarm_enabled(&self) -> bool {
        matches!(self.options.glyph_prewarm, GlyphPrewarmMode::CommonGlyphs)
    }

    /// CPU renderer capabilities exposed through the compositor renderer contract.
    #[must_use]
    pub fn cpu_capabilities() -> RendererCapabilities {
        RendererCapabilities {
            frame_memory_kinds: vec![FrameMemoryKind::Cpu],
            pixel_formats: vec![PixelFormat::Bgra8, PixelFormat::Rgba8, PixelFormat::Rgb8],
            supports_partial_damage: true,
            supports_blur: true,
            supports_skeleton_window: true,
            supports_async_glyphs: true,
            max_framebuffer_width: None,
            max_framebuffer_height: None,
        }
    }

    /// Check file-backed font faces for staleness, **reload** changed faces
    /// from disk, and invalidate CPU-side glyph state.
    ///
    /// Stale detection and the actual byte reload happen inside the font worker
    /// (which shares the [`FontDatabase`]): a changed face is re-read and
    /// re-parsed so subsequent rasterization uses the fresh outlines (closes
    /// t49-e3-F15 — the prior code re-rasterized the same stale bytes forever).
    ///
    /// The CPU glyph atlas is keyed by renderer-local font IDs rather than
    /// rasterizer face IDs, so once any face reloads this conservatively clears
    /// the whole atlas and prewarm state, returning the exact stale face IDs to
    /// the caller.
    pub fn invalidate_stale_fonts(&mut self) -> Vec<FontFaceId> {
        let stale_faces = self.font_worker.invalidate_stale_faces();
        if !stale_faces.is_empty() {
            self.glyph_atlas.clear();
            self.prewarmed_fonts.clear();
            self.has_pending_glyphs = false;
        }
        stale_faces
    }

    /// Alias for callers that prefer a poll-style stale font hook name.
    pub fn check_stale_fonts_and_invalidate(&mut self) -> Vec<FontFaceId> {
        self.invalidate_stale_fonts()
    }

    #[cfg(test)]
    pub(crate) fn pending_glyph_request_count(&self) -> usize {
        self.font_worker.pending_count()
    }

    #[cfg(test)]
    pub(crate) fn prewarmed_font_count(&self) -> usize {
        self.prewarmed_fonts.len()
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
    /// Prewarm common glyphs for the SHAPED text path.
    ///
    /// The shaped path keys the atlas by the concrete font face id + the REAL font
    /// glyph id (not the codepoint), so the legacy codepoint-keyed
    /// [`prewarm_glyphs`](Self::prewarm_glyphs) would warm entries the shaped path
    /// never reads. This resolves the run's primary face, maps each common
    /// character to its real glyph id in that face, and requests those shaped keys
    /// — exactly the entries the next frames' shaping will look up. Characters the
    /// face does not cover (glyph id 0) are skipped (their fallback face is warmed
    /// lazily when first shaped). Runs after the per-line shape so the shared
    /// font-database lock is uncontended during shaping.
    fn prewarm_shaped_glyphs(
        &mut self,
        size_px: u16,
        target_height: u32,
        font_family: &str,
        font_weight: u16,
        italic: bool,
    ) {
        const PREWARM_CHARS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz\
             0123456789 .,;:!?-'\"()[]/";

        // Resolve the primary face and snapshot (char, glyph_id) pairs while
        // holding the lock briefly, then drop the lock before enqueueing requests.
        let face_and_glyphs: Option<(liquide_font_rasterizer::database::FontFaceId, Vec<(char, u32)>)> = {
            use ab_glyph::Font;
            let db = self
                .font_worker
                .font_db()
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            db.resolve(font_family, font_weight, italic).and_then(|face_id| {
                db.get(face_id).map(|face| {
                    let pairs = PREWARM_CHARS
                        .chars()
                        .filter_map(|ch| {
                            let gid = face.font.glyph_id(ch).0 as u32;
                            (gid != 0).then_some((ch, gid))
                        })
                        .collect::<Vec<_>>();
                    (face_id, pairs)
                })
            })
        };

        let Some((face_id, pairs)) = face_and_glyphs else {
            return;
        };
        let font_id = text::compute_shaped_font_id(face_id.0, italic);
        for (ch, gid) in pairs {
            let key = GlyphKey {
                font_id,
                glyph_id: gid,
                size_px,
                subpixel: false,
            };
            if self.glyph_atlas.get(&key).is_none() {
                self.font_worker
                    .request_shaped_glyph(key, face_id, ch, target_height);
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
        self.box_shadow_cache.clear();
    }

    /// Look up a cached chrome box-shadow mask by its input signature.
    pub(crate) fn box_shadow_cache_get(&self, key: u64) -> Option<&ShadowMask> {
        self.box_shadow_cache
            .iter()
            .find(|c| c.key == key)
            .map(|c| &c.mask)
    }

    /// Insert a freshly-computed chrome box-shadow mask, evicting the oldest
    /// entry when at capacity (simple FIFO — chrome shadows are few + long-lived).
    ///
    /// `shape_key` is the position-independent shape signature and `unclamped`
    /// records whether the mask rect was fully inside the framebuffer when
    /// generated; both feed the t179 translate fast path.
    pub(crate) fn box_shadow_cache_insert(
        &mut self,
        key: u64,
        shape_key: u64,
        unclamped: bool,
        mask: ShadowMask,
    ) {
        if let Some(slot) = self.box_shadow_cache.iter_mut().find(|c| c.key == key) {
            slot.shape_key = shape_key;
            slot.unclamped = unclamped;
            slot.mask = mask;
            return;
        }
        if self.box_shadow_cache.len() >= MAX_BOX_SHADOW_CACHE {
            self.box_shadow_cache.remove(0);
        }
        self.box_shadow_cache.push(CachedBoxShadow {
            key,
            shape_key,
            unclamped,
            mask,
        });
    }

    /// t179 position-translate fast path for a MOVING window's drop-shadow.
    ///
    /// On an exact-key MISS, look for an UNCLAMPED cache entry with the same
    /// SHAPE (`shape_key`) — i.e. the identical shadow shape rendered at some
    /// other position. If the move delta from the cached mask's origin to the
    /// requested `(dst_x0, dst_y0)` is integral (always true for pixel positions)
    /// and the requested position is itself unclamped (the mask rect at the new
    /// spot is fully inside `[0,fb)`), TRANSLATE the cached mask's pixels to the
    /// new origin and store it under the exact `key` — making subsequent identical
    /// frames plain exact-key hits. Returns `true` when a translate was performed.
    ///
    /// Byte-identical basis (t178): `generate_shadow_mask` samples the SDF at
    /// `(x0+mx)+0.5` against an `expanded_surface` anchored at `surface_rect - …`;
    /// the coverage depends only on `(sample - centre)`. An integer move shifts
    /// BOTH the sample grid and the surface centre by the same amount, so every
    /// mask pixel is unchanged — only the origin moves. This holds ONLY when no
    /// edge clamps at EITHER position (same `width`/`height`, no truncated edge),
    /// hence the `unclamped` guard on both the source entry and the destination.
    pub(crate) fn box_shadow_cache_translate(
        &mut self,
        key: u64,
        shape_key: u64,
        dst_unclamped: bool,
        dst_x0: u32,
        dst_y0: u32,
    ) -> bool {
        // The destination position must itself be unclamped: a clamped new spot
        // would have a DIFFERENT (truncated) mask rect than the cached shape, so
        // translating would paint a wrong/stale shape. Regenerate instead.
        if !dst_unclamped {
            return false;
        }
        // Find a same-shape, unclamped source entry (any position). The shape key
        // already encodes width/height/radius/spread/blur/colour/offset/fb-dims,
        // so a match guarantees the cached mask's pixels ARE this shape — only the
        // origin differs. (The delta is integral by construction: both origins are
        // `u32` framebuffer pixel coordinates.)
        let src = self
            .box_shadow_cache
            .iter()
            .find(|c| c.unclamped && c.shape_key == shape_key)
            .map(|c| (c.mask.width, c.mask.height, c.mask.pixels.clone()));
        let Some((width, height, pixels)) = src else {
            return false;
        };
        let translated = ShadowMask {
            pixels,
            x0: dst_x0,
            y0: dst_y0,
            width,
            height,
        };
        self.box_shadow_cache_insert(key, shape_key, dst_unclamped, translated);
        true
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
        if self.box_shadow_cache.len() > MAX_BOX_SHADOW_CACHE / 2 {
            let drop = self.box_shadow_cache.len() / 2;
            self.box_shadow_cache.drain(0..drop);
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

    /// Number of cached blur results (test-only diagnostic).
    #[cfg(test)]
    #[must_use]
    pub fn blur_cache_len(&self) -> usize {
        self.blur_worker.cache_len()
    }

    /// Drain completed async blur results into the cache (test-only).
    ///
    /// Production code drains at the top of [`render`]; tests use this to
    /// observe the cache without driving a full frame.
    #[cfg(test)]
    pub fn poll_blur_results(&mut self) {
        self.blur_worker.poll_results();
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
    fn backend_info(&self) -> RendererBackendInfo {
        let mut info =
            RendererBackendInfo::new(RendererBackendKind::Software, "liquide-renderer-cpu");
        info.version = Some(env!("CARGO_PKG_VERSION").to_string());
        info.adapter = Some("host CPU".to_string());
        info
    }

    fn capabilities(&self) -> RendererCapabilities {
        Self::cpu_capabilities()
    }

    fn negotiate_render(
        &self,
        _nodes: &[FlatNode],
        fb: &FrameBuffer,
        damage: &DamageSet,
    ) -> RendererNegotiation {
        let negotiation = Self::cpu_capabilities().negotiate(fb, damage);
        if !negotiation.is_accepted() {
            return negotiation;
        }

        if fb.width == 0 || fb.height == 0 {
            return RendererNegotiation::rejected(RendererRejectReason::Other(format!(
                "CPU renderer requires non-zero framebuffer dimensions, got {}x{}",
                fb.width, fb.height
            )));
        }

        let bytes_per_pixel = fb.format.bytes_per_pixel();
        let Some(minimum_stride) = fb.width.checked_mul(bytes_per_pixel) else {
            return RendererNegotiation::rejected(RendererRejectReason::FramebufferTooLarge {
                width: fb.width,
                height: fb.height,
                max_width: None,
                max_height: None,
            });
        };
        if fb.stride < minimum_stride {
            return RendererNegotiation::rejected(RendererRejectReason::Other(format!(
                "CPU framebuffer stride {} is smaller than width {} * {} bytes per pixel",
                fb.stride, fb.width, bytes_per_pixel
            )));
        }

        let Some(required_len) = (fb.stride as usize).checked_mul(fb.height as usize) else {
            return RendererNegotiation::rejected(RendererRejectReason::FramebufferTooLarge {
                width: fb.width,
                height: fb.height,
                max_width: None,
                max_height: None,
            });
        };
        match &fb.memory {
            FrameMemory::Cpu(pixels) if pixels.len() < required_len => {
                RendererNegotiation::rejected(RendererRejectReason::Other(format!(
                    "CPU framebuffer has {} bytes but rendering requires at least {} bytes",
                    pixels.len(),
                    required_len
                )))
            }
            _ => RendererNegotiation::accepted(),
        }
    }

    #[allow(unused_assignments)]
    fn render(
        &mut self,
        nodes: &[FlatNode],
        fb: &mut FrameBuffer,
        damage: &DamageSet,
    ) -> liquide_compositor::RenderResult<Vec<DamageTile>> {
        // The `render` trait entry is the DETERMINISTIC CAPTURE path. It MUST
        // keep block-draining glyphs so the visual-test capture seam and the
        // goldens stay byte-stable (e2e_temporal). The live desktop loop calls
        // `render_live` instead (non-blocking). Do not change this to anything
        // other than `RenderMode::Capture`.
        self.render_with_mode(nodes, fb, damage, RenderMode::Capture)
    }

    /// Live, non-blocking render entry. Overrides the trait default (which would
    /// delegate to the blocking `render`) so the interactive desktop loop honours
    /// the live glyph-drain budget and never stalls present cadence. Forwards to
    /// the inherent [`SoftwareRenderer::render_live`].
    fn render_live(
        &mut self,
        nodes: &[FlatNode],
        fb: &mut FrameBuffer,
        damage: &DamageSet,
        mode: RenderMode,
    ) -> liquide_compositor::RenderResult<Vec<DamageTile>> {
        SoftwareRenderer::render_live(self, nodes, fb, damage, mode)
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
            crate::lod::PerformanceMode::Performance => {
                liquide_compositor::RenderQuality::Performance
            }
        }
    }

    fn set_quality_mode(&mut self, mode: liquide_compositor::RenderQuality) {
        let lod_mode = match mode {
            liquide_compositor::RenderQuality::Quality => crate::lod::PerformanceMode::Quality,
            liquide_compositor::RenderQuality::Balanced => crate::lod::PerformanceMode::Balanced,
            liquide_compositor::RenderQuality::Performance => {
                crate::lod::PerformanceMode::Performance
            }
        };
        self.lod_manager.set_performance_mode(lod_mode);
    }

    /// Expose the concrete renderer so the host can drive backend-specific
    /// operations (image upload via [`register_image_rgba`](Self::register_image_rgba)
    /// and the CSS cursor seam via [`set_cursor_theme`](Self::set_cursor_theme))
    /// through a `Box<dyn Renderer>`.
    fn as_any_mut(&mut self) -> Option<&mut dyn core::any::Any> {
        Some(self)
    }
}

impl SoftwareRenderer {
    /// **Live, non-blocking render entry** for the desktop runtime.
    ///
    /// This is the responsive counterpart to the [`Renderer::render`] capture
    /// path. It NEVER block-drains glyphs for a perceptible duration:
    ///
    /// - [`RenderMode::LiveFull`] polls completed glyphs and waits at most
    ///   [`LIVE_GLYPH_DRAIN_BUDGET_MS`] (≪ the 500 ms render watchdog) for more,
    ///   then paints with whatever is ready (estimated advances / last-good for
    ///   the rest) and leaves `has_pending_glyphs()` set so the session can
    ///   schedule a cheap follow-up frame — text fills in within a frame or two
    ///   instead of stalling the desktop for up to 2 s.
    /// - [`RenderMode::LiveCursor`] does a pure non-blocking poll and never
    ///   waits at all — a pointer move must not stall on text glyphs requested
    ///   by an earlier full frame.
    ///
    /// The capture path ([`Renderer::render`]) is intentionally left untouched
    /// and still block-drains for determinism. Passing [`RenderMode::Capture`]
    /// here is equivalent to calling `render`.
    ///
    /// Returns the per-tile damage classification exactly like `render`. Check
    /// [`has_pending_glyphs`](Self::has_pending_glyphs) after the call: when it
    /// is `true`, request another frame.
    pub fn render_live(
        &mut self,
        nodes: &[FlatNode],
        fb: &mut FrameBuffer,
        damage: &DamageSet,
        mode: RenderMode,
    ) -> liquide_compositor::RenderResult<Vec<DamageTile>> {
        self.render_with_mode(nodes, fb, damage, mode)
    }

    /// Shared render body. The only behavioural difference between modes is how
    /// not-yet-rasterized glyphs are drained (see [`RenderMode`]); the painting
    /// code below is identical, so a fully-quiesced atlas produces byte-identical
    /// output regardless of mode.
    #[allow(unused_assignments)]
    fn render_with_mode(
        &mut self,
        nodes: &[FlatNode],
        fb: &mut FrameBuffer,
        damage: &DamageSet,
        mode: RenderMode,
    ) -> liquide_compositor::RenderResult<Vec<DamageTile>> {
        // Reset pending-glyph tracker for this frame.
        self.has_pending_glyphs = false;

        // Reset the active blend mode to default for this frame.
        self.active_blend_mode = BlendMode::SrcOver;

        // Data-parallel rasterization across cores (t76 #2) is available but
        // DEFAULT-OFF, because the CPU fill/blit kernels are memory-BANDWIDTH
        // bound, not compute bound: on this host (and typical desktops with a
        // single memory controller) splitting a full-screen fill/blit across
        // cores adds rayon scheduling + cache-line contention with no extra
        // bandwidth, measuring ~35% SLOWER for the full frame and ~2x slower for
        // damage-only (render_bench A/B). It is therefore opt-in via
        // `LIQUIDE_PARALLEL_RASTER=1` for hosts with more memory channels where
        // it may pay off. The capture/golden path is always serial (determinism:
        // parallel fills race the async blur worker). When enabled, the row-band
        // split is over disjoint scanlines, so output stays byte-identical.
        let parallel_enabled = match mode {
            RenderMode::Capture => false,
            RenderMode::LiveFull | RenderMode::LiveCursor => {
                std::env::var("LIQUIDE_PARALLEL_RASTER").is_ok()
            }
        };
        let prev_parallel = rasterizer::set_parallel_raster(parallel_enabled);

        // The capture path computes backdrop blur synchronously for determinism;
        // the live paths keep the non-blocking async blur worker.
        self.deterministic_blur = matches!(mode, RenderMode::Capture);

        // Drain any completed async blur results before rendering.
        self.blur_worker.poll_results();

        // Drain completed glyph rasterizations into the atlas.
        //
        // DETERMINISM (Capture mode): glyphs are rasterised asynchronously on
        // the font-worker thread, so a non-blocking poll commits only whatever
        // happens to have arrived by now — a per-run race. Because text layout
        // (word-wrap, pen advance) reads glyph advances out of the atlas, a
        // partially populated atlas yields a *different layout and
        // dropped/garbled glyphs every run*. The capture path therefore
        // block-drains the outstanding pending set first so the atlas is
        // identical for an identical scene. The deadline bounds the wait so a
        // wedged worker can never hang the render loop.
        //
        // LIVENESS (LiveFull / LiveCursor modes): the live desktop loop must not
        // stall on glyphs, so it either waits a tiny budget (LiveFull) or not at
        // all (LiveCursor), paints with whatever is ready, and relies on
        // `has_pending_glyphs` (set while painting text) to request a follow-up
        // frame. Any still-pending glyphs simply resolve on the next frame.
        let rasterized = match drain_deadline(mode) {
            Some(deadline) => self.font_worker.drain_pending_blocking(deadline),
            None => self.font_worker.poll_results(),
        };
        for glyph in &rasterized {
            let _ = self
                .glyph_atlas
                .insert(glyph.key, &glyph.bitmap, &glyph.metrics);
        }

        // Compute damage bounding box in pixel coordinates for early culling.
        // Nodes fully outside the damaged region are skipped since only damaged
        // tiles will be blitted to the final output.
        let damage_bbox = if damage.is_empty() {
            None
        } else if let Some((grid_width, grid_height, _)) = damage.full_grid_dimensions() {
            let ts = damage.tile_size as f32;
            let padding = 32.0_f32;
            Some((
                -padding,
                -padding,
                grid_width as f32 * ts + padding,
                grid_height as f32 * ts + padding,
            ))
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

        // Set the per-frame raster clip so every node's fill/blit/text raster is
        // confined to the changed region (t76 #1). On a near-idle frame only a
        // few tiles change, so this turns a ~280 ms full-frame raster into a
        // ~tens-of-ms damage-only raster (~8x) — the dominant lever. On full
        // damage (resize / wallpaper / theme reload) the clip is `None` and the
        // whole frame is rastered exactly as before. The clip restricts only
        // WHICH pixels are written, never their values, so output is identical to
        // the unclipped path within the damaged region.
        self.raster_clip = match damage_bbox {
            Some((dx0, dy0, dx1, dy1)) if !damage.is_full() => {
                Some(Rect::new(dx0, dy0, dx1 - dx0, dy1 - dy0))
            }
            _ => None,
        };

        // Install the SAME rect as a hard per-thread framebuffer write-scissor
        // (t80). Unlike `self.raster_clip` — which several node kinds (Image,
        // BackgroundFill, backdrop-blur write, Decoration, Icon, Gradient,
        // SvgPath, Shadow) silently ignored, causing a full-screen wallpaper to
        // overpaint a partial-damage frame and leave a permanent hole — the
        // write-scissor is enforced by the write helpers themselves, so NO node
        // kind can write outside the damage rect. On full damage the clip is
        // `None` and the scissor is `None`, so the capture/full path is
        // byte-identical to before.
        let prev_scissor = rasterizer::set_write_scissor(self.raster_clip);

        self.render_nodes_in_order(nodes, fb, damage_bbox);

        // Clear the clip + scissor so neither leaks into a subsequent
        // capture/full frame.
        rasterizer::set_write_scissor(prev_scissor);
        self.raster_clip = None;
        // Restore the parallel-raster flag for this thread.
        rasterizer::set_parallel_raster(prev_parallel);

        Ok(self.classify_damage_tiles(nodes, damage, fb))
    }

    /// Walk the flattened nodes in z-order and paint each one, culling nodes
    /// fully outside `damage_bbox`. Factored out of [`Self::render_with_mode`] so
    /// the serial damage-clipped path and the parallel full-frame path share one
    /// node-iteration body.
    ///
    /// **Front-to-back occlusion culling (t137):** in addition to the
    /// damage-bbox cull above, a node is skipped if every pixel it would paint is
    /// guaranteed to be over-painted by one or more *fully-opaque* nodes drawn
    /// later in the list (higher z). Skipping a fully-occluded node by a
    /// fully-opaque cover cannot change a single output pixel — the cover writes
    /// the final value at every shared pixel regardless of what was beneath — so
    /// this is byte-identical to painting it. See [`occlusion`] for the strict
    /// (conservative) opaque-occluder rule and the coverage proof.
    fn render_nodes_in_order(
        &mut self,
        nodes: &[FlatNode],
        fb: &mut FrameBuffer,
        damage_bbox: Option<(f32, f32, f32, f32)>,
    ) {
        // Precompute the set of later opaque-occluder rects so each node can be
        // tested against everything painted ON TOP of it. The occluder rect for
        // a node is its guaranteed fully-painted opaque region (bounds ∩ clip),
        // or `None` if the node is not a safe opaque occluder.
        let occluders = occlusion::occluder_rects(nodes);

        // Start each walk with no open clip-path scopes (t149). The begin/apply
        // markers are balanced within a frame; clearing here also guards against
        // an unbalanced list leaking a snapshot into the next frame.
        self.clip_scopes.clear();
        // Likewise start with no open render layers; any opened during the walk
        // are drained (composited) by `finish_layers` after the loop.
        self.layer_scopes.clear();

        for (i, node) in nodes.iter().enumerate() {
            // Skip nodes completely outside the damage bounding box.
            if let Some((dx0, dy0, dx1, dy1)) = damage_bbox {
                let b = &node.absolute_bounds;
                if b.x >= dx1 || b.y >= dy1 || b.x + b.width <= dx0 || b.y + b.height <= dy0 {
                    continue;
                }
            }

            // Front-to-back occlusion cull: if this node's painted rect (its
            // bounds confined to the active raster clip — outside the clip
            // nothing is written anyway) is ENTIRELY covered by the union of
            // later fully-opaque occluder rects, it is invisible. Skip it.
            if let Some(test_rect) =
                occlusion::cullable_paint_rect(node, self.raster_clip)
            {
                if occlusion::is_fully_covered_by_later(test_rect, i, &occluders) {
                    #[cfg(test)]
                    occlusion::record_culled(node.id);
                    continue;
                }
            }

            let distance = self.calculate_distance_from_center(&node.absolute_bounds);
            let lod_level = self.select_lod(node, distance);

            self.render_node_with_lod(node, fb, lod_level);
        }

        // Composite any still-open isolated render layers (group opacity) once,
        // innermost first, over their saved backdrops. The flat node list has no
        // end-of-layer marker, so layers that were not closed by a later
        // same/shallower layer are closed here at walk end.
        self.finish_layers(fb);
    }

    fn classify_damage_tiles(
        &self,
        nodes: &[FlatNode],
        damage: &DamageSet,
        fb: &FrameBuffer,
    ) -> Vec<DamageTile> {
        if damage.is_empty() {
            return Vec::new();
        }

        let expanded_damage_tiles = if damage.is_full() {
            damage.materialize_tiles()
        } else {
            damage.tiles.clone()
        };

        let mut damage_tiles: HashMap<(u32, u32), DamageClass> =
            HashMap::with_capacity(expanded_damage_tiles.len());
        for tile in &expanded_damage_tiles {
            damage_tiles
                .entry((tile.x, tile.y))
                .and_modify(|existing| {
                    if tile.class.priority() < existing.priority() {
                        *existing = tile.class;
                    }
                })
                .or_insert(tile.class);
        }

        let mut classified: HashMap<(u32, u32), DamageClass> =
            HashMap::with_capacity(damage_tiles.len());

        let fb_bounds = Rect::new(0.0, 0.0, fb.width as f32, fb.height as f32);
        let tile_size = damage.tile_size as f32;
        let max_tx = fb.width.div_ceil(damage.tile_size);
        let max_ty = fb.height.div_ceil(damage.tile_size);

        for node in nodes {
            let Some(node_class) = Self::classify_node_kind(node.kind_ref()) else {
                continue;
            };

            let clipped_bounds = node
                .clip
                .as_ref()
                .map_or(Some(node.absolute_bounds), |clip| {
                    node.absolute_bounds.intersection(clip)
                })
                .and_then(|bounds| bounds.intersection(&fb_bounds));

            let Some(bounds) = clipped_bounds else {
                continue;
            };

            let tx_start = (bounds.x.max(0.0) / tile_size).floor() as u32;
            let ty_start = (bounds.y.max(0.0) / tile_size).floor() as u32;
            let tx_end = (bounds.right().max(0.0) / tile_size).ceil() as u32;
            let ty_end = (bounds.bottom().max(0.0) / tile_size).ceil() as u32;

            for ty in ty_start..ty_end.min(max_ty) {
                for tx in tx_start..tx_end.min(max_tx) {
                    if damage_tiles.contains_key(&(tx, ty)) {
                        classified
                            .entry((tx, ty))
                            .and_modify(|existing| {
                                if node_class.priority() < existing.priority() {
                                    *existing = node_class;
                                }
                            })
                            .or_insert(node_class);
                    }
                }
            }
        }

        for (&coords, &fallback_class) in &damage_tiles {
            classified.entry(coords).or_insert(fallback_class);
        }

        let mut tiles: Vec<DamageTile> = classified
            .into_iter()
            .map(|((x, y), class)| DamageTile { x, y, class })
            .collect();
        tiles.sort_by_key(|tile| (tile.class.priority(), tile.y, tile.x));
        tiles
    }

    fn classify_node_kind(kind: &SceneNodeKind) -> Option<DamageClass> {
        match kind {
            SceneNodeKind::Cursor { .. } => Some(DamageClass::CursorOnly),
            SceneNodeKind::Text { .. } | SceneNodeKind::TextCaret { .. } => {
                Some(DamageClass::TextGlyph)
            }
            SceneNodeKind::Surface { .. }
            | SceneNodeKind::ChildSurface { .. }
            | SceneNodeKind::Image { .. }
            | SceneNodeKind::BlurCache => Some(DamageClass::BitmapRegion),
            SceneNodeKind::Root
            | SceneNodeKind::Workspace { .. }
            | SceneNodeKind::Overlay
            | SceneNodeKind::Content
            | SceneNodeKind::ShellLayer
            | SceneNodeKind::RenderLayer { .. }
            | SceneNodeKind::ClipPath { .. }
            | SceneNodeKind::Filter { .. }
            | SceneNodeKind::BackdropFilter { .. } => None,
            _ => Some(DamageClass::UiPrimitive),
        }
    }

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

        match node.kind_ref() {
            SceneNodeKind::Background { color } => {
                let mut c = *color;
                if opacity < 1.0 {
                    c.a = (c.a as f32 * opacity + 0.5) as u8;
                }
                let blend = self.active_blend_mode;
                let (r_tl, r_tr, r_br, r_bl) = node.corner_radius;
                let has_radius = r_tl > 0.5 || r_tr > 0.5 || r_br > 0.5 || r_bl > 0.5;
                // Confine the solid fill to the active damage region (t76). The
                // intersection only restricts which pixels are written; the fill
                // colour is unchanged, so damage-clipping is byte-identical to a
                // full fill within the clip window. Rounded fills keep their full
                // geometry (the SDF samples `bounds`) but still clamp via the
                // clip-aware corner fill.
                let Some(fill_bounds) = rasterizer::clip_rect(bounds, self.raster_clip) else {
                    return;
                };
                if has_radius {
                    self.fill_rounded_rect_per_corner_clipped(
                        fb,
                        bounds,
                        c,
                        r_tl,
                        r_tr,
                        r_br,
                        r_bl,
                        blend,
                        self.raster_clip,
                    );
                } else if c.a == 255 && blend == BlendMode::SrcOver {
                    rasterizer::fill_rect(fb, fill_bounds, c, BlendMode::Src);
                } else {
                    rasterizer::fill_rect(fb, fill_bounds, c, blend);
                }
            }

            SceneNodeKind::Surface { buffer, .. } | SceneNodeKind::ChildSurface { buffer, .. } => {
                if let Some(buf) = buffer {
                    if opacity >= 1.0 && buf.format == liquide_compositor::pixel::PixelFormat::Bgra8
                    {
                        rasterizer::blit_opaque_stride_clipped(
                            fb,
                            &buf.pixels,
                            buf.width,
                            buf.height,
                            buf.stride as usize,
                            bounds.x.max(0.0) as u32,
                            bounds.y.max(0.0) as u32,
                            self.raster_clip,
                        );
                    } else {
                        rasterizer::blit_alpha_stride_clipped(
                            fb,
                            &buf.pixels,
                            buf.width,
                            buf.height,
                            buf.stride as usize,
                            bounds.x.max(0.0) as u32,
                            bounds.y.max(0.0) as u32,
                            opacity,
                            self.raster_clip,
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
                if let Some(tint_rect) = rasterizer::clip_rect(bounds, self.raster_clip) {
                    rasterizer::fill_rect(fb, tint_rect, c, BlendMode::Multiply);
                }
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
                        self.render_backdrop_blur(node.id, bounds, radius, node.corner_radius, fb);
                    }
                }
            }

            SceneNodeKind::BlurCache => {
                if self.blur_enabled && self.intersects_dirty(&bounds) {
                    let radius = self.effect_params.blur_radius;
                    if radius > 0 {
                        self.render_backdrop_blur(node.id, bounds, radius, node.corner_radius, fb);
                    }
                }
            }

            SceneNodeKind::Content | SceneNodeKind::Overlay | SceneNodeKind::ShellLayer => {
                if opacity < 1.0 {
                    // Multiply alpha of existing pixels in the region.
                    let x0 = (bounds.x.max(0.0) as u32).min(fb.width);
                    let y0 = (bounds.y.max(0.0) as u32).min(fb.height);
                    let x1 = (bounds.right().ceil() as u32).min(fb.width);
                    let y1 = (bounds.bottom().ceil() as u32).min(fb.height);
                    // Confine to the damage write-scissor (t84): a full-bleed
                    // dimming Overlay/fade on a partial-damage frame must not
                    // touch preserved pixels outside the damage rect. `set_pixel`
                    // also drops out-of-scissor writes as a backstop, but
                    // clamping the loop keeps the hot path tight.
                    let (x0, y0, x1, y1) = rasterizer::scissor_clamp_window(x0, y0, x1, y1);
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
                        self.render_backdrop_blur(node.id, bounds, radius, node.corner_radius, fb);
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

            SceneNodeKind::SvgPath {
                d,
                fill,
                stroke,
                stroke_width,
            } => {
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
                            ox + seg.x1,
                            oy + seg.y1,
                            ox + seg.x2,
                            oy + seg.y2,
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
                self.render_clip_path_node(clip_kind, &bounds, fb);
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
                // Confine the mask multiply to the damage write-scissor (t84) so a
                // full-bleed mask cannot dim preserved pixels outside the damage
                // rect on a partial frame.
                let (x0, y0, x1, y1) = rasterizer::scissor_clamp_window(x0, y0, x1, y1);
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

            SceneNodeKind::RenderLayer {
                blend_mode,
                isolate,
            } => {
                // Unconditionally set the blend mode so that a normal
                // (SrcOver) layer resets the mode after a previous
                // non-default layer.
                self.active_blend_mode = *blend_mode;

                // Isolated group with group opacity < 1: open an offscreen layer
                // so the group's children composite as a single unit and are
                // dimmed exactly once (no double-darkening of overlaps). A fully
                // opaque isolated group is a visual no-op for compositing (the
                // merged content equals painting the children directly), so we
                // only pay the snapshot/clear cost when opacity actually matters.
                if *isolate && opacity < 0.999 {
                    self.open_layer(&bounds, opacity, fb);
                }
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

    /// Compute the framebuffer pixel window a layer/effect covers, clamped to the
    /// framebuffer and the active write-scissor. Returns `None` for an empty window.
    fn clamped_window(&self, bounds: &Rect, fb: &FrameBuffer) -> Option<(u32, u32, u32, u32)> {
        let x0 = (bounds.x.max(0.0) as u32).min(fb.width);
        let y0 = (bounds.y.max(0.0) as u32).min(fb.height);
        let x1 = (bounds.right().ceil() as u32).min(fb.width);
        let y1 = (bounds.bottom().ceil() as u32).min(fb.height);
        let (x0, y0, x1, y1) = rasterizer::scissor_clamp_window(x0, y0, x1, y1);
        if x0 >= x1 || y0 >= y1 {
            None
        } else {
            Some((x0, y0, x1, y1))
        }
    }

    /// Open an isolated render layer for group opacity (see [`LayerScope`]).
    ///
    /// Before opening, any currently-open layers whose window does NOT contain the
    /// new layer's window are closed first — they are siblings/ancestors that have
    /// ended (the flat list has no end marker, so a new same/shallower layer is the
    /// signal that earlier ones are finished). Then the backdrop under the new
    /// layer's window is snapshotted and the window cleared to transparent black so
    /// the group's children composite onto an empty layer.
    fn open_layer(&mut self, bounds: &Rect, opacity: f32, fb: &mut FrameBuffer) {
        let Some(window) = self.clamped_window(bounds, fb) else {
            return;
        };

        // Close any open layers that this new layer is NOT nested inside (their
        // content has ended). Containment is by pixel window.
        while let Some(top) = self.layer_scopes.last() {
            if window_contains(top.window, window) {
                break;
            }
            self.close_top_layer(fb);
        }

        let (x0, y0, x1, y1) = window;
        let w = (x1 - x0) as usize;
        let h = (y1 - y0) as usize;
        let mut backdrop = Vec::with_capacity(w * h);
        for y in y0..y1 {
            for x in x0..x1 {
                backdrop.push(fb.get_pixel(x, y));
            }
        }
        // Clear the window so the group's children composite onto an empty layer.
        let clear = Color {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        };
        for y in y0..y1 {
            for x in x0..x1 {
                fb.set_pixel(x, y, clear);
            }
        }
        self.layer_scopes.push(LayerScope {
            window,
            backdrop,
            opacity: opacity.clamp(0.0, 1.0),
        });
    }

    /// Close (composite) the top open layer: blend the layer's merged content over
    /// its saved backdrop a single time, weighted by the group opacity. The window
    /// after this holds `backdrop  SrcOver  (layer * group_opacity)`.
    fn close_top_layer(&mut self, fb: &mut FrameBuffer) {
        let Some(scope) = self.layer_scopes.pop() else {
            return;
        };
        let (x0, y0, x1, _y1) = scope.window;
        let w = (x1 - x0) as usize;
        for y in y0..scope.window.3 {
            for x in x0..x1 {
                let layer = fb.get_pixel(x, y);
                let idx = (y - y0) as usize * w + (x - x0) as usize;
                let back = scope.backdrop.get(idx).copied().unwrap_or(Color {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 0,
                });
                // SrcOver of the isolated layer (scaled by group opacity) over the
                // saved backdrop. The layer is straight-alpha BGRA; scaling its
                // alpha by the group opacity dims the whole group once. Channels
                // are straight (non-premultiplied), so blend channels with the
                // effective source alpha.
                let sa = (layer.a as f32 / 255.0) * scope.opacity;
                if sa <= 0.0 {
                    // Layer contributed nothing here: restore the backdrop verbatim.
                    fb.set_pixel(x, y, back);
                    continue;
                }
                let inv = 1.0 - sa;
                let ba = back.a as f32 / 255.0;
                let out_a = sa + ba * inv;
                let mix = |s: u8, d: u8| -> u8 {
                    // Straight-alpha SrcOver: out = (s*sa + d*da*inv) / out_a.
                    let num = s as f32 * sa + d as f32 * ba * inv;
                    if out_a <= 0.0 {
                        0
                    } else {
                        (num / out_a + 0.5).clamp(0.0, 255.0) as u8
                    }
                };
                let out = Color {
                    r: mix(layer.r, back.r),
                    g: mix(layer.g, back.g),
                    b: mix(layer.b, back.b),
                    a: (out_a * 255.0 + 0.5).clamp(0.0, 255.0) as u8,
                };
                fb.set_pixel(x, y, out);
            }
        }
    }

    /// Close every open render layer (innermost first). Called at the end of the
    /// node walk so no layer leaks past the frame.
    fn finish_layers(&mut self, fb: &mut FrameBuffer) {
        while !self.layer_scopes.is_empty() {
            self.close_top_layer(fb);
        }
    }

    /// Render a `ClipPath` marker node (t149 — sibling-scoped clip-path).
    ///
    /// The scene bridge emits a clip-path as a PAIRED begin/apply marker that
    /// brackets the clipped element's OWN draws. The two markers carry identical
    /// shape + bounds. We pair them with a renderer-local stack:
    ///
    /// * **BEGIN** (identity does NOT match the open scope on top of the stack):
    ///   snapshot the pixels under the scope window (already-painted siblings /
    ///   background) and push the scope. Nothing is masked yet.
    /// * **APPLY** (identity matches the top of the stack): the element's content
    ///   is now in the framebuffer over the snapshot. For each pixel in the window
    ///   compute the shape coverage `c` and composite `c·element + (1−c)·snapshot`
    ///   — so inside the shape the element survives (c≈1, byte-identical to the old
    ///   flat mask), outside the shape the snapshot is restored (c≈0; the OLD code
    ///   destroyed those pixels to transparent — the sibling-leak bug), and edges
    ///   anti-alias against the actual content behind the element. Pop the scope.
    ///
    /// Both the snapshot and the composite write through the framebuffer (which
    /// enforces the damage write-scissor), so the clip stays confined to the
    /// damage rect exactly as before (t84).
    fn render_clip_path_node(
        &mut self,
        clip_kind: &liquide_compositor::scene::ClipPathKind,
        bounds: &Rect,
        fb: &mut FrameBuffer,
    ) {
        use liquide_compositor::scene::ClipPathKind;

        // A degenerate polygon clips nothing — skip it entirely (matches the
        // pre-t149 behaviour and avoids opening an unbalanced scope).
        if let ClipPathKind::Polygon { points } = clip_kind {
            if points.len() < 3 {
                return;
            }
        }

        // Compute the window the shape can touch, clamped to the framebuffer and
        // the active write-scissor (t84) — identical to the pre-t149 per-arm
        // bounds so the snapshot covers exactly the pixels the mask may alter.
        let bx0 = (bounds.x.max(0.0) as u32).min(fb.width);
        let by0 = (bounds.y.max(0.0) as u32).min(fb.height);
        let bx1 = (bounds.right().ceil() as u32).min(fb.width);
        let by1 = (bounds.bottom().ceil() as u32).min(fb.height);
        let (bx0, by0, bx1, by1) = rasterizer::scissor_clamp_window(bx0, by0, bx1, by1);
        if bx0 >= bx1 || by0 >= by1 {
            // Empty window. Still pair begin/apply so the stack stays balanced:
            // a begin with an empty window pushes an empty snapshot; the matching
            // apply pops it. (Both markers carry the same bounds, so both reach
            // this branch together.)
            let identity = clip_scope_identity(clip_kind, bounds);
            match self.clip_scopes.last() {
                Some(top) if top.identity == identity => {
                    self.clip_scopes.pop();
                }
                _ => self.clip_scopes.push(ClipScope {
                    identity,
                    window: (bx0, by0, bx0, by0),
                    snapshot: Vec::new(),
                }),
            }
            return;
        }

        let identity = clip_scope_identity(clip_kind, bounds);
        let is_apply = matches!(self.clip_scopes.last(), Some(top) if top.identity == identity);

        if !is_apply {
            // BEGIN: snapshot the window before the element paints over it.
            let w = (bx1 - bx0) as usize;
            let h = (by1 - by0) as usize;
            let mut snapshot = Vec::with_capacity(w * h);
            for y in by0..by1 {
                for x in bx0..bx1 {
                    snapshot.push(fb.get_pixel(x, y));
                }
            }
            self.clip_scopes.push(ClipScope {
                identity,
                window: (bx0, by0, bx1, by1),
                snapshot,
            });
            return;
        }

        // APPLY: pop the snapshot taken at BEGIN and composite the element's
        // content against it per the shape coverage.
        let scope = self
            .clip_scopes
            .pop()
            .expect("is_apply implies a scope on the stack");
        let (sx0, sy0, sx1, _sy1) = scope.window;
        let snap_w = (sx1 - sx0) as usize;

        // A coverage function for the active shape: 1.0 fully inside, 0.0 fully
        // outside, anti-aliased across the 1px edge. Geometry matches the
        // pre-t149 per-arm SDF/winding tests exactly (byte-identical interior).
        let coverage_at = |fx: f32, fy: f32| -> f32 {
            match clip_kind {
                ClipPathKind::RoundedRect { corner_radius } => {
                    let r = *corner_radius;
                    let d = rasterizer::sdf_rounded_rect_per_corner(fx, fy, bounds, r, r, r, r);
                    (-d + 0.5).clamp(0.0, 1.0)
                }
                ClipPathKind::Circle {
                    center_x,
                    center_y,
                    radius,
                } => {
                    let cx = bounds.x + center_x * bounds.width;
                    let cy = bounds.y + center_y * bounds.height;
                    let r = radius * bounds.width.min(bounds.height);
                    let d = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt() - r;
                    (-d + 0.5).clamp(0.0, 1.0)
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
                    let nx = (fx - cx) / erx;
                    let ny = (fy - cy) / ery;
                    let d = (nx * nx + ny * ny).sqrt() - 1.0;
                    (-d * erx.min(ery) + 0.5).clamp(0.0, 1.0)
                }
                ClipPathKind::Polygon { points } => {
                    let mut winding = 0i32;
                    let mut min_dist_sq = f32::MAX;
                    let n = points.len();
                    for i in 0..n {
                        let j = (i + 1) % n;
                        let x0 = bounds.x + points[i].0 * bounds.width;
                        let y0 = bounds.y + points[i].1 * bounds.height;
                        let x1 = bounds.x + points[j].0 * bounds.width;
                        let y1 = bounds.y + points[j].1 * bounds.height;
                        if y0 <= fy {
                            if y1 > fy && ((x1 - x0) * (fy - y0) - (fx - x0) * (y1 - y0)) > 0.0 {
                                winding += 1;
                            }
                        } else if y1 <= fy
                            && ((x1 - x0) * (fy - y0) - (fx - x0) * (y1 - y0)) < 0.0
                        {
                            winding -= 1;
                        }
                        let ex = x1 - x0;
                        let ey = y1 - y0;
                        let len_sq = ex * ex + ey * ey;
                        let t = if len_sq > 0.0 {
                            ((fx - x0) * ex + (fy - y0) * ey) / len_sq
                        } else {
                            0.0
                        }
                        .clamp(0.0, 1.0);
                        let px = x0 + t * ex - fx;
                        let py = y0 + t * ey - fy;
                        min_dist_sq = min_dist_sq.min(px * px + py * py);
                    }
                    let dist = min_dist_sq.sqrt();
                    let signed_dist = if winding != 0 { dist } else { -dist };
                    (signed_dist + 0.5).clamp(0.0, 1.0)
                }
            }
        };

        for y in by0..by1 {
            let fy = y as f32 + 0.5;
            for x in bx0..bx1 {
                let fx = x as f32 + 0.5;
                let coverage = coverage_at(fx, fy);
                // Fully inside the shape: the element's content is kept verbatim
                // (byte-identical to the old flat mask for a single element).
                if coverage >= 1.0 {
                    continue;
                }
                // The snapshot pixel captured under this position at BEGIN. The
                // window the snapshot covers is identical to the apply window
                // (same bounds → same clamp), so the index maps 1:1.
                let snap = {
                    let sx = (x - sx0) as usize;
                    let sy = (y - sy0) as usize;
                    scope
                        .snapshot
                        .get(sy * snap_w + sx)
                        .copied()
                        .unwrap_or(Color {
                            r: 0,
                            g: 0,
                            b: 0,
                            a: 0,
                        })
                };
                let elem = fb.get_pixel(x, y);
                // Composite element OVER the pre-element snapshot weighted by the
                // shape coverage (premultiplied — channels and alpha scale
                // together to avoid dark fringes). coverage 0 ⇒ snapshot restored
                // verbatim (siblings/background survive — the t149 fix); coverage
                // 1 was handled above.
                let inv = 1.0 - coverage;
                let mix = |e: u8, s: u8| -> u8 {
                    (e as f32 * coverage + s as f32 * inv + 0.5).clamp(0.0, 255.0) as u8
                };
                let out = Color {
                    r: mix(elem.r, snap.r),
                    g: mix(elem.g, snap.g),
                    b: mix(elem.b, snap.b),
                    a: mix(elem.a, snap.a),
                };
                fb.set_pixel(x, y, out);
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
        let end = self
            .remaining
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
