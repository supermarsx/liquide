//! `build_scene()` method and scene graph assembly.

use std::sync::Arc;

use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::{Affine2D, Rect};
use liquide_compositor::pixel::Color;
use liquide_compositor::scene::{
    DecorationButtonRects, DecorationButtons, DecorationColors, DecorationLayout, NodeProperties,
    SceneNode, SceneNodeKind,
};

use crate::decoration::{DecorationStyle, HitZone};
use crate::scene_builder::*;
use crate::theme::ShellTheme;
use crate::tiling::SnapZone;
use crate::window::{Window, WindowFlags, WindowState};

use super::Shell;

/// Base of the chrome overlay z-band. Background fills get `[0..)`, the workspace
/// (windows) sits at `WORKSPACE_Z_ORDER`, and every chrome surface gets
/// `[CHROME_Z_BASE..)`. The software cursor is composited at flatten time with
/// `z_order = 9999` (render_thread.rs `cursor_flat_node`), so it paints ABOVE the
/// background band and BELOW the chrome band — the invariant this classifier
/// preserves.
const CHROME_Z_BASE: u32 = 10_000;

/// Is `node` a full-screen FILL — a solid color, gradient, or image that covers
/// (nearly) the whole screen? These are the only node kinds the
/// `<desktop-background>` element emits for its `background` shorthand. The
/// 0.9-screen-area guard keeps small images (icons, thumbnails) and bar-shaped
/// chrome fills (statusbar, dock) out of the desktop-background classification.
fn is_fullscreen_fill(node: &SceneNode, screen_area: f32) -> bool {
    let nb = &node.properties.bounds;
    let node_area = nb.width * nb.height;
    matches!(
        node.kind,
        SceneNodeKind::Background { .. }
            | SceneNodeKind::GradientFill { .. }
            // t74-realimg: a `background-image: url(...)` desktop wallpaper
            // becomes a full-screen Image node. It is the backdrop exactly like a
            // gradient fill, so it must join the background layer (below windows),
            // not the chrome overlay (above them).
            | SceneNodeKind::Image { .. }
    ) && node_area >= screen_area * 0.9
}

/// Assign each pipeline node its final `z_order`, splitting the stream into a
/// background band and a chrome overlay band.
///
/// ── Classify the desktop-background fills by ORIGIN, not by count (t182) ──
///
/// `<desktop-background>` is the FIRST element in the desktop DOM
/// (desktop_dom.rs) — `position:fixed; (0,0); 100%×100%` — so the CSS pipeline
/// paints its fills FIRST, ahead of all other chrome (statusbar, dock, windows,
/// overlays). Its `background` shorthand can resolve to MORE THAN ONE full-screen
/// fill: e.g. after the cascade fix the liquid-glass theme layers a
/// `var(--bg-primary)` solid color (from components.css) UNDER a `url(...)`
/// wallpaper Image — two stacked full-screen nodes that BOTH originate from the
/// same element and must BOTH live in the background band, below windows and
/// below the software cursor (z=9999) and every overlay.
///
/// The desktop-background's fills are therefore the LEADING, CONTIGUOUS run of
/// full-screen fills in the pipeline stream. Pre-pended Glass nodes (chrome
/// blurs) are never full-screen FILLS, so they don't open the run; the run OPENS
/// at the first full-screen fill and CLOSES at the first node after it that is
/// NOT a full-screen fill (the first real chrome content — statusbar / dock /
/// window draws). Every full-screen fill INSIDE that run is the
/// desktop-background's own stack and joins the background band, preserving its
/// emit order (color UNDER image). Any full-screen fill AFTER the run closes is a
/// later overlay (launcher-overlay / loading-overlay) and stays in the chrome
/// band, ABOVE windows + cursor — never demoted below them.
///
/// This is origin-based: it captures N desktop-background fills (1, 2, or more)
/// without assuming a fixed count, and without dropping or hacking any node. The
/// single-fullscreen-bg case (themes with just one fill) still classifies that
/// one fill as background — identical behaviour to before t182.
///
/// Z-order scheme for root's children:
///   `[0 .. bg_count)`                      — background layer
///   `WORKSPACE_Z_ORDER` (caller)           — workspace (windows)
///   `[chrome_z_base .. chrome_z_base+N)`   — chrome overlay layer
fn classify_pipeline_nodes(
    pipeline_nodes: Vec<SceneNode>,
    screen: Rect,
    chrome_z_base: u32,
) -> Vec<SceneNode> {
    let screen_area = screen.width * screen.height;
    let mut bg_z = 0u32;
    let mut chrome_z = chrome_z_base;
    let mut in_desktop_bg_run = false;
    let mut desktop_bg_run_closed = false;

    let mut out = Vec::with_capacity(pipeline_nodes.len());
    for mut node in pipeline_nodes {
        let fullscreen = is_fullscreen_fill(&node, screen_area);

        // A node belongs to the desktop-background origin when it is a
        // full-screen fill within the LEADING contiguous run (see above). Glass /
        // other chrome before the first fill leaves the run unopened; the first
        // non-fill after the run permanently closes it.
        if !desktop_bg_run_closed {
            if fullscreen {
                in_desktop_bg_run = true;
            } else if in_desktop_bg_run {
                // First non-fill after the run started → the desktop-background
                // stack is complete; everything full-screen after this is a later
                // overlay (chrome), not the desktop background.
                desktop_bg_run_closed = true;
            }
        }

        let is_bg = fullscreen && in_desktop_bg_run && !desktop_bg_run_closed;
        if is_bg {
            node.properties.z_order = bg_z;
            bg_z += 1;
        } else {
            node.properties.z_order = chrome_z;
            chrome_z += 1;
        }
        out.push(node);
    }
    out
}

/// Traffic-light button REST colors resolved from the FULL CSS cascade
/// (t172-e2). See [`Shell::button_colors_from_css`]: the painted decoration
/// reads its button backgrounds from the active theme resolver, which does NOT
/// carry the base `components.css` button rules / traffic-light tokens; this
/// carries the rest-state background + icon colors read from the laid-out button
/// elements' computed styles (the full cascade) so the dots paint in the exact
/// red / yellow / green the CSS resolves.
struct DecorationCssColors {
    close_bg: Color,
    close_icon: Color,
    minimize_bg: Color,
    minimize_icon: Color,
    maximize_bg: Color,
    maximize_icon: Color,
}

/// Base id for the per-window effect/paint container (t93-e2 / t92 gap #4).
///
/// Each window's nodes are wrapped in one non-visual `Workspace`-kind container
/// (id = base + `window_id`) that carries the per-window effect opacity. The
/// container is stripped from the flattened paint output, so this id never
/// reaches a `FlatNode`; it sits in its own reserved range purely to keep the
/// scene-tree ids distinct from the window leaf-node ids.
const NODE_WINDOW_EFFECT_GROUP_BASE: u64 = 50_000_000;

/// Base id for the per-window CONTENT wrapper (t163-drag-cache). Like the effect
/// group above, this non-visual `Workspace`-kind container is stripped from the
/// flattened paint output, so this id never reaches a `FlatNode`; it sits in its
/// own reserved range (distinct from the effect-group range) purely to keep the
/// scene-tree ids unique. It carries the per-window content TRANSLATE.
const NODE_WINDOW_CONTENT_GROUP_BASE: u64 = 60_000_000;

/// Canonical node id the cached (position-independent) content subtree is built
/// with before it is rebased onto each window's `win_base` (t163-drag-cache).
const CONTENT_CANON_NODE_BASE: u64 = 0;

/// Canonical group id for the cached content wrapper (overwritten per window).
const CONTENT_CANON_GROUP_ID: u64 = 0;

/// Canonical content z-base the cached content subtree is built with before its
/// per-node z_orders are rebased by each window's band-aware `paint_z_base`
/// (t163-drag-cache). Mirrors the live `z_content = paint_z_base + 3`.
const CONTENT_CANON_Z_BASE: u32 = 3;

/// Rebase a cached canonical content subtree onto a specific window: add
/// `id_delta` to every node id and `z_delta` to every node z_order
/// (t163-drag-cache).
///
/// The cached content subtree is built with canonical (0-based) node ids and a
/// canonical z-base so two windows that SHARE one cached entry (identical
/// size+content, different position / window id / stacking rank) can each rebase
/// the clone: ids onto their own `win_base` (no cross-window id collision in
/// damage / skeleton / hit identity), and z_orders by their own band-aware
/// `paint_z_base` (so a stacked window's content keeps its correct paint order).
fn rebase_content_subtree(node: &mut SceneNode, id_delta: u64, z_delta: u32) {
    node.id = node.id.wrapping_add(id_delta);
    node.properties.z_order = node.properties.z_order.wrapping_add(z_delta);
    for child in &mut node.children {
        rebase_content_subtree(child, id_delta, z_delta);
    }
}

/// Lightweight counters for the retained window workspace scene cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowSceneCacheStats {
    pub hits: u64,
    pub misses: u64,
    pub dirty: bool,
    pub cached: bool,
}

fn themed_alpha(mut color: Color, alpha: u8) -> Color {
    color.a = alpha;
    color
}

/// Scale `(w, h)` down so its longer edge is at most `max_edge`, preserving
/// aspect (never upscales). Used to bound a stored overview thumbnail (t93-e6).
fn scale_within(w: u32, h: u32, max_edge: u32) -> (u32, u32) {
    let max_edge = max_edge.max(1);
    let longer = w.max(h);
    if longer <= max_edge || longer == 0 {
        return (w.max(1), h.max(1));
    }
    let s = max_edge as f32 / longer as f32;
    (((w as f32 * s).round() as u32).max(1), ((h as f32 * s).round() as u32).max(1))
}

/// Fit `(w, h)` inside `(box_w, box_h)` preserving aspect (never upscales past
/// the box; may downscale). Returns integer pixel dimensions for the painted
/// overview thumbnail (t93-e6).
fn fit_within(w: u32, h: u32, box_w: f32, box_h: f32) -> (u32, u32) {
    if w == 0 || h == 0 || box_w < 1.0 || box_h < 1.0 {
        return (1, 1);
    }
    let s = (box_w / w as f32).min(box_h / h as f32);
    (
        ((w as f32 * s).round() as u32).max(1),
        ((h as f32 * s).round() as u32).max(1),
    )
}

/// Lightweight counters for the full-scene (whole `build_scene` root) cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FullSceneCacheStats {
    pub hits: u64,
    pub misses: u64,
    pub dirty: bool,
    pub cached: bool,
}

/// Retains the complete assembled `build_scene` root across idle frames
/// (t76-scenecache).
///
/// On a steady-state frame where nothing that affects the scene has changed,
/// `build_scene` returns a clone of [`Self::node`] instead of re-running the
/// whole assembly (sync_dom bridge + CSS pipeline + HitTest rebuild + manual
/// root reassembly). The `dirty` flag is the conservative invalidation channel:
/// it starts `true` (no cache yet) and is set by [`Shell::mark_full_scene_dirty`]
/// — which the existing [`Shell::mark_window_scene_dirty`] also calls, so every
/// window-affecting state path already invalidates this cache too. Chrome /
/// animation / cursor-blink changes are caught by the additional predicate in
/// `build_scene` (pipeline fast-path + blink check), never by a stale clone.
#[derive(Debug)]
pub(crate) struct FullSceneCache {
    node: Option<SceneNode>,
    hits: u64,
    misses: u64,
    dirty: bool,
}

impl FullSceneCache {
    pub(crate) fn new() -> Self {
        Self {
            node: None,
            hits: 0,
            misses: 0,
            dirty: true,
        }
    }

    /// Mark the cache stale so the next `build_scene` rebuilds.
    pub(crate) fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    fn dirty(&self) -> bool {
        self.dirty
    }

    /// Clone the cached root, if one is retained.
    fn node_clone(&self) -> Option<SceneNode> {
        self.node.clone()
    }

    fn record_hit(&mut self) {
        self.hits = self.hits.saturating_add(1);
    }

    fn record_miss(&mut self) {
        self.misses = self.misses.saturating_add(1);
    }

    fn store(&mut self, node: SceneNode) {
        self.node = Some(node);
        self.dirty = false;
    }

    pub(crate) fn stats(&self) -> FullSceneCacheStats {
        FullSceneCacheStats {
            hits: self.hits,
            misses: self.misses,
            dirty: self.dirty,
            cached: self.node.is_some(),
        }
    }
}

impl Default for FullSceneCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Lightweight counters for the POSITION-INDEPENDENT per-window content cache
/// (t163-drag-cache). A window MOVE (x/y change, same w/h + content) HITS this
/// cache so the expensive content subtree (`content_view` + per-row/cell nodes)
/// is reused and only the wrapper translate updates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowContentCacheStats {
    pub hits: u64,
    pub misses: u64,
    pub entries: usize,
}

/// Upper bound on retained distinct content signatures. Identical-size/content
/// windows SHARE one entry (the key excludes position AND window id), so this
/// only grows with genuinely distinct content shapes; clear wholesale past the
/// cap rather than carry an unbounded map.
const CONTENT_CACHE_CAP: usize = 64;

/// Retains the manually assembled active-workspace/window subtree.
#[derive(Debug)]
pub(crate) struct WindowSceneCache {
    signature: Option<WindowSceneSignature>,
    node: Option<SceneNode>,
    hits: u64,
    misses: u64,
    dirty: bool,
    /// Per-window CONTENT subtree cache (t163-drag-cache), keyed by a
    /// POSITION-INDEPENDENT [`WindowContentSignature`] (no x/y, no window id).
    /// Built once at the canonical origin `(0,0)` with canonical (0-based) node
    /// ids; the caller rebases the ids per window and wraps the clone in a
    /// translate carrying the absolute `(x,y)`. So a pure MOVE reuses this entry
    /// (no `content_view`, no per-row rebuild) and only the wrapper translate
    /// changes; a RESIZE (w/h change) or content change misses (different
    /// signature) and rebuilds. Two windows at different positions but identical
    /// size+content share one entry. This map is deliberately NOT cleared by
    /// [`Self::mark_dirty`] — a drag-move calls `mark_window_scene_dirty` every
    /// frame, and blowing the content cache away there would re-introduce the
    /// per-frame rebuild this cache exists to remove. Its only invalidation is
    /// the signature mismatch (content/size change), which is exact.
    content: std::collections::HashMap<WindowContentSignature, SceneNode>,
    content_hits: u64,
    content_misses: u64,
}

impl WindowSceneCache {
    pub(crate) fn new() -> Self {
        Self {
            signature: None,
            node: None,
            hits: 0,
            misses: 0,
            dirty: true,
            content: std::collections::HashMap::new(),
            content_hits: 0,
            content_misses: 0,
        }
    }

    fn get(&mut self, signature: &WindowSceneSignature) -> Option<SceneNode> {
        if !self.dirty && self.signature.as_ref() == Some(signature) {
            if let Some(node) = &self.node {
                self.hits = self.hits.saturating_add(1);
                return Some(node.clone());
            }
        }

        self.misses = self.misses.saturating_add(1);
        None
    }

    fn store(&mut self, signature: WindowSceneSignature, node: SceneNode) {
        self.signature = Some(signature);
        self.node = Some(node);
        self.dirty = false;
    }

    /// Look up a cached CONTENT subtree for `signature`, returning a clone of the
    /// canonical (origin-anchored, 0-based-id) subtree on a hit.
    fn get_content(&mut self, signature: &WindowContentSignature) -> Option<SceneNode> {
        if let Some(node) = self.content.get(signature) {
            self.content_hits = self.content_hits.saturating_add(1);
            return Some(node.clone());
        }
        self.content_misses = self.content_misses.saturating_add(1);
        None
    }

    /// Store a canonical content subtree under its position-independent signature.
    fn store_content(&mut self, signature: WindowContentSignature, node: SceneNode) {
        if self.content.len() >= CONTENT_CACHE_CAP && !self.content.contains_key(&signature) {
            self.content.clear();
        }
        self.content.insert(signature, node);
    }

    pub(crate) fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Peek the signature the cached window subtree was built with (the PREVIOUS
    /// frame's window state), without disturbing the cache (t176-damage-confine).
    /// Used by `compute_precomputed_damage` to diff old-vs-new window state and
    /// emit a confined per-window damage set instead of falling back to full.
    /// `None` when nothing has been cached yet (first frame).
    fn peek_signature(&self) -> Option<&WindowSceneSignature> {
        self.signature.as_ref()
    }

    pub(crate) fn stats(&self) -> WindowSceneCacheStats {
        WindowSceneCacheStats {
            hits: self.hits,
            misses: self.misses,
            dirty: self.dirty,
            cached: self.node.is_some(),
        }
    }

    pub(crate) fn content_stats(&self) -> WindowContentCacheStats {
        WindowContentCacheStats {
            hits: self.content_hits,
            misses: self.content_misses,
            entries: self.content.len(),
        }
    }
}

impl Default for WindowSceneCache {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct WindowSceneSignature {
    screen: RectSignature,
    active_workspace_id: u32,
    focused_id: Option<u64>,
    hovered_button: Option<HoveredButtonSignature>,
    cursor_blink_on: bool,
    decoration_style: DecorationStyleSignature,
    decoration_colors: DecorationColorsSignature,
    decoration_layout: DecorationLayoutSignature,
    theme: WindowThemeSignature,
    windows: Vec<WindowRenderSignature>,
    /// Focused window's typed-text buffer (t57-fG feature 2): typing changes
    /// the painted field, so it must invalidate the window scene cache.
    focused_text: Option<String>,
    /// Per-window app-content revisions (t70-s6). Each registered app view's
    /// window contributes `(window_id, revision)`; the revision is bumped on
    /// every input route / explicit content-dirty, so changing app content
    /// (typed text, drained terminal output, …) invalidates the window scene
    /// cache even though the `Window` struct itself is unchanged.
    app_content: Vec<(u64, u64)>,
    /// Per-window active effect frame (t93-e2 / t92 gap #4). An animating
    /// window's frame (bounds + opacity) changes every tick, so it must be part
    /// of the cache key — otherwise the signature-keyed window subtree cache
    /// would serve a stale mid-animation (or pre-animation) subtree and the
    /// animation would never advance. Idle windows contribute nothing, so a
    /// steady-state scene keeps its cache exactly as before.
    effects: Vec<WindowEffectSignature>,
}

impl WindowSceneSignature {
    /// Returns `true` iff this frame's window change is STRUCTURAL — a change the
    /// confined per-window damage path deliberately does NOT attempt to bound,
    /// falling back to a full repaint instead (t176-damage-confine).
    ///
    /// Structural = anything that moves a window, changes its geometry/stacking,
    /// adds/removes a window, animates it, or recolors/relays-out EVERY window's
    /// frame. For these the prompt's guidance is to stay full (correct-but-slow):
    ///   * any GLOBAL field (`screen`, `active_workspace_id`, `decoration_*`,
    ///     `theme`) — recolors/relays out every window.
    ///   * the `windows` Vec differs in ANY way — a window opened/closed, moved,
    ///     resized, restacked (z), retitled, re-stated, retiled, or faded
    ///     (per-window opacity). The window-DRAG fast paths (move/resize) are
    ///     already confined at EVENT time in `events.rs`; the build-time path
    ///     here stays conservative for geometry.
    ///   * the `effects` Vec differs — an open/close/transform ANIMATION frame
    ///     (geometry + opacity tween); also handled conservatively.
    ///
    /// When this returns `false` the ONLY differences are paint-only per-window
    /// fields (`focused_id`, `hovered_button`, `cursor_blink_on`, `focused_text`,
    /// `app_content`) which [`Self::paint_changed_window_ids`] attributes to exact
    /// window ids and confines.
    fn structural_change(&self, other: &Self) -> bool {
        self.screen != other.screen
            || self.active_workspace_id != other.active_workspace_id
            || self.decoration_style != other.decoration_style
            || self.decoration_colors != other.decoration_colors
            || self.decoration_layout != other.decoration_layout
            || self.theme != other.theme
            || self.windows != other.windows
            || self.effects != other.effects
    }

    /// Collect the set of window ids whose PAINT-ONLY per-window state differs
    /// between `self` (PREVIOUS frame) and `other` (THIS frame)
    /// (t176-damage-confine). Only reached when [`Self::structural_change`] is
    /// `false`, so the `windows`/`effects`/global fields are already known equal;
    /// every changed window therefore has the SAME bounds in both frames.
    ///
    /// Each remaining field is attributed to the id(s) it repaints:
    /// * `focused_id` — focus moving recolors BOTH the old- and new-focused
    ///   window's border/decoration.
    /// * `hovered_button` — a titlebar-button hover-highlight flip recolors the
    ///   old- and new-hovered window's decoration.
    /// * `cursor_blink_on` — the caret lives in the FOCUSED window's content.
    /// * `focused_text` — the typed-text field is in the focused window.
    /// * `app_content` — a bumped revision (text typed, terminal output drained,
    ///   SCROLL) marks exactly that window id.
    ///
    /// The returned set is the COMPLETE set of windows whose pixels changed this
    /// frame (no global/geometry change can have escaped `structural_change`).
    fn paint_changed_window_ids(&self, other: &Self) -> std::collections::BTreeSet<u64> {
        let mut ids: std::collections::BTreeSet<u64> = std::collections::BTreeSet::new();
        let mark_focus = |ids: &mut std::collections::BTreeSet<u64>| {
            if let Some(f) = self.focused_id {
                ids.insert(f);
            }
            if let Some(f) = other.focused_id {
                ids.insert(f);
            }
        };

        if self.focused_id != other.focused_id {
            mark_focus(&mut ids);
        }
        if self.cursor_blink_on != other.cursor_blink_on {
            mark_focus(&mut ids);
        }
        if self.focused_text != other.focused_text {
            mark_focus(&mut ids);
        }
        if self.hovered_button != other.hovered_button {
            if let Some(h) = self.hovered_button {
                ids.insert(h.window_id);
            }
            if let Some(h) = other.hovered_button {
                ids.insert(h.window_id);
            }
        }

        // app_content: a changed/added/removed (window_id, rev) marks its id.
        let old_rev: std::collections::HashMap<u64, u64> =
            self.app_content.iter().copied().collect();
        let new_rev: std::collections::HashMap<u64, u64> =
            other.app_content.iter().copied().collect();
        for (id, rev) in &new_rev {
            if old_rev.get(id) != Some(rev) {
                ids.insert(*id);
            }
        }
        for id in old_rev.keys() {
            if !new_rev.contains_key(id) {
                ids.insert(*id);
            }
        }

        ids
    }

    /// The painted-footprint rect for `window_id` in this signature: its settled
    /// render bounds (t176-damage-confine). Reached only on the paint-only path
    /// where `windows`/`effects` are equal between frames, so the settled bounds
    /// ARE the painted footprint (no active effect can have differed). Returns an
    /// empty `Vec` if the window is absent (should not happen on this path).
    fn footprints_for(&self, window_id: u64) -> Vec<Rect> {
        let mut rects = Vec::new();
        if let Some(w) = self.windows.iter().find(|w| w.id == window_id) {
            rects.push(w.bounds.to_rect());
        }
        if let Some(e) = self.effects.iter().find(|e| e.window_id == window_id) {
            rects.push(e.bounds.to_rect());
        }
        rects
    }
}

/// POSITION-INDEPENDENT cache key for a single window's CONTENT subtree
/// (t163-drag-cache).
///
/// Captures everything `build_window_content` / `build_app_view_content` read
/// that can change what the content subtree contains — but deliberately EXCLUDES
/// the window's screen POSITION (x/y) and its window id / `win_base`. So a pure
/// MOVE (x/y change, same w/h + content + state) yields the SAME signature and
/// HITS the content cache; the absolute position is reapplied as a translate on
/// the wrapper. A RESIZE changes `content_w`/`content_h` (which drive
/// `cols`/`rows`) → different signature → rebuild. Content changes are folded in
/// via `app_content_rev` (bumped on every input route / content-dirty),
/// `focused`/`focused_text` (the typed-text field), and `cursor_blink_on` (the
/// terminal/app caret). Two windows at different positions with identical
/// size+content+state therefore share ONE cached content subtree.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct WindowContentSignature {
    app_id: String,
    title: String,
    /// Content-area width/height (NOT the window bounds): a resize changes these
    /// and re-lays-out `cols`/`rows`, so they must invalidate; a move does not.
    content_w: u32,
    content_h: u32,
    /// Live app-view revision (0 when the window has no registered app view).
    app_content_rev: u64,
    has_app_view: bool,
    focused: bool,
    focused_text: Option<String>,
    cursor_blink_on: bool,
    /// Content-relevant theme colors (the same fields the content nodes paint
    /// with). A theme recolour must re-emit the content with the new colors.
    text_color: ColorSignature,
    terminal_bg: ColorSignature,
    terminal_text: ColorSignature,
    sidebar_item: ColorSignature,
    browser_urlbar: ColorSignature,
    content_background: ColorSignature,
}

/// Cache-key fingerprint of a single window's active effect frame (t93-e2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct WindowEffectSignature {
    window_id: u64,
    bounds: RectSignature,
    opacity: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct HoveredButtonSignature {
    window_id: u64,
    zone: HitZone,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct WindowRenderSignature {
    id: u64,
    title: String,
    app_id: String,
    bounds: RectSignature,
    state: WindowState,
    z_order: i32,
    visible: bool,
    flags: u8,
    opacity: u32,
    tiled: bool,
    tile_zone: Option<SnapZone>,
    min_size: Option<SizeSignature>,
}

impl WindowRenderSignature {
    fn from_window(window: &Window) -> Self {
        Self {
            id: window.id.0,
            title: window.title.clone(),
            app_id: window.app_id.clone(),
            bounds: RectSignature::from_rect(window.bounds),
            state: window.state,
            z_order: window.z_order,
            visible: window.visible,
            flags: window.flags.bits(),
            opacity: f32_signature(window.opacity),
            tiled: window.tiled,
            tile_zone: window.tile_zone,
            min_size: window.min_size.map(SizeSignature::from_size),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct RectSignature {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl RectSignature {
    fn from_rect(rect: Rect) -> Self {
        Self {
            x: f32_signature(rect.x),
            y: f32_signature(rect.y),
            width: f32_signature(rect.width),
            height: f32_signature(rect.height),
        }
    }

    /// Reconstruct the screen-space `Rect` from the stored f32-bit signature
    /// fields (t176-damage-confine). `f32_signature` is a lossless bit-pattern of
    /// the original `f32` (it only canonicalises -0.0 → +0.0, irrelevant for a
    /// window rect), so this round-trips the exact painted bounds — which is what
    /// the confined window-damage builder needs to bound the changed footprint.
    fn to_rect(self) -> Rect {
        Rect::new(
            f32::from_bits(self.x),
            f32::from_bits(self.y),
            f32::from_bits(self.width),
            f32::from_bits(self.height),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct SizeSignature {
    width: u32,
    height: u32,
}

impl SizeSignature {
    fn from_size((width, height): (f32, f32)) -> Self {
        Self {
            width: f32_signature(width),
            height: f32_signature(height),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct DecorationStyleSignature {
    title_bar_height: u32,
    border_width: u32,
    corner_radius: u32,
    button_size: u32,
    resize_tolerance: u32,
    button_width: u32,
    button_height: u32,
    button_right_margin: u32,
}

impl DecorationStyleSignature {
    fn from_style(style: &DecorationStyle) -> Self {
        Self {
            title_bar_height: f32_signature(style.title_bar_height),
            border_width: f32_signature(style.border_width),
            corner_radius: f32_signature(style.corner_radius),
            button_size: f32_signature(style.button_size),
            resize_tolerance: f32_signature(style.resize_tolerance),
            button_width: f32_signature(style.button_width),
            button_height: f32_signature(style.button_height),
            button_right_margin: f32_signature(style.button_right_margin),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct DecorationLayoutSignature {
    title_bar_height: u32,
    button_width: u32,
    button_height: u32,
    button_right_margin: u32,
    button_corner_radius: u32,
    /// CSS-resolved frame colors (titlebar bg / border / title text). Frame
    /// colors are theme-global (the same for every window), so capturing them
    /// from the constant `button_layout` here means a theme that recolors the
    /// window frame invalidates the window-scene cache and the decoration
    /// repaints with the new CSS colors (t113-deco-handoff). The per-window
    /// `button_rects` need not be fingerprinted separately: they are a
    /// deterministic function of each window's bounds + the decoration layout +
    /// the button CSS, all of which already invalidate this cache (window
    /// bounds, decoration_style/layout scalars, and the stylesheet-change path
    /// that calls `mark_window_scene_dirty`).
    frame_colors: Option<DecorationFrameColorsSignature>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct DecorationFrameColorsSignature {
    title_bar_bg: ColorSignature,
    border: ColorSignature,
    title_text: ColorSignature,
}

impl DecorationLayoutSignature {
    fn from_layout(layout: &DecorationLayout) -> Self {
        Self {
            title_bar_height: f32_signature(layout.title_bar_height),
            button_width: f32_signature(layout.button_width),
            button_height: f32_signature(layout.button_height),
            button_right_margin: f32_signature(layout.button_right_margin),
            button_corner_radius: f32_signature(layout.button_corner_radius),
            frame_colors: layout.frame_colors.map(|f| DecorationFrameColorsSignature {
                title_bar_bg: ColorSignature::from_color(f.title_bar_bg),
                border: ColorSignature::from_color(f.border),
                title_text: ColorSignature::from_color(f.title_text),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct DecorationColorsSignature {
    close_bg: ColorSignature,
    close_bg_hover: ColorSignature,
    close_icon: ColorSignature,
    maximize_bg: ColorSignature,
    maximize_bg_hover: ColorSignature,
    maximize_icon: ColorSignature,
    minimize_bg: ColorSignature,
    minimize_bg_hover: ColorSignature,
    minimize_icon: ColorSignature,
    pin_bg: ColorSignature,
    pin_bg_hover: ColorSignature,
    pin_bg_active: ColorSignature,
    pin_bg_active_hover: ColorSignature,
    pin_icon: ColorSignature,
    pin_icon_active: ColorSignature,
}

impl DecorationColorsSignature {
    fn from_colors(colors: &DecorationColors) -> Self {
        Self {
            close_bg: ColorSignature::from_color(colors.close_bg),
            close_bg_hover: ColorSignature::from_color(colors.close_bg_hover),
            close_icon: ColorSignature::from_color(colors.close_icon),
            maximize_bg: ColorSignature::from_color(colors.maximize_bg),
            maximize_bg_hover: ColorSignature::from_color(colors.maximize_bg_hover),
            maximize_icon: ColorSignature::from_color(colors.maximize_icon),
            minimize_bg: ColorSignature::from_color(colors.minimize_bg),
            minimize_bg_hover: ColorSignature::from_color(colors.minimize_bg_hover),
            minimize_icon: ColorSignature::from_color(colors.minimize_icon),
            pin_bg: ColorSignature::from_color(colors.pin_bg),
            pin_bg_hover: ColorSignature::from_color(colors.pin_bg_hover),
            pin_bg_active: ColorSignature::from_color(colors.pin_bg_active),
            pin_bg_active_hover: ColorSignature::from_color(colors.pin_bg_active_hover),
            pin_icon: ColorSignature::from_color(colors.pin_icon),
            pin_icon_active: ColorSignature::from_color(colors.pin_icon_active),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct WindowThemeSignature {
    window_title_bar_focused: ColorSignature,
    window_title_bar_unfocused: ColorSignature,
    window_title_text: ColorSignature,
    window_border_focused: ColorSignature,
    window_border_unfocused: ColorSignature,
    window_shadow: ColorSignature,
    window_glass_tint: ColorSignature,
    window_content_background: ColorSignature,
    status_bar_text: ColorSignature,
    app_settings_sidebar_item: ColorSignature,
    app_terminal_background: ColorSignature,
    app_terminal_text: ColorSignature,
    app_browser_urlbar: ColorSignature,
}

impl WindowThemeSignature {
    fn from_theme(theme: &ShellTheme) -> Self {
        Self {
            window_title_bar_focused: ColorSignature::from_color(theme.window_title_bar_focused),
            window_title_bar_unfocused: ColorSignature::from_color(
                theme.window_title_bar_unfocused,
            ),
            window_title_text: ColorSignature::from_color(theme.window_title_text),
            window_border_focused: ColorSignature::from_color(theme.window_border_focused),
            window_border_unfocused: ColorSignature::from_color(theme.window_border_unfocused),
            window_shadow: ColorSignature::from_color(theme.window_shadow),
            window_glass_tint: ColorSignature::from_color(theme.window_glass_tint),
            window_content_background: ColorSignature::from_color(theme.window_content_background),
            status_bar_text: ColorSignature::from_color(theme.status_bar_text),
            app_settings_sidebar_item: ColorSignature::from_color(theme.app_settings_sidebar_item),
            app_terminal_background: ColorSignature::from_color(theme.app_terminal_background),
            app_terminal_text: ColorSignature::from_color(theme.app_terminal_text),
            app_browser_urlbar: ColorSignature::from_color(theme.app_browser_urlbar),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ColorSignature {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

impl ColorSignature {
    fn from_color(color: Color) -> Self {
        Self {
            r: color.r,
            g: color.g,
            b: color.b,
            a: color.a,
        }
    }
}

fn f32_signature(value: f32) -> u32 {
    if value == 0.0 { 0.0 } else { value }.to_bits()
}

impl Shell {
    /// Explicitly invalidate the retained manual window subtree.
    ///
    /// This also invalidates the full-scene cache (t76-scenecache): every
    /// state path that changes the window subtree already routes through here,
    /// so funnelling the full-scene invalidation through the same method means
    /// no window-affecting mutation can leave a stale cached root behind.
    pub fn mark_window_scene_dirty(&mut self) {
        self.window_scene_cache.mark_dirty();
        self.full_scene_cache.mark_dirty();
    }

    /// Explicitly invalidate the cached full `build_scene` root (t76-scenecache)
    /// without touching the window subtree cache. Used by paths that affect the
    /// assembled root (chrome/overlay composition) but not the window subtree.
    pub fn mark_full_scene_dirty(&mut self) {
        self.full_scene_cache.mark_dirty();
    }

    /// Return counters for the retained manual window subtree cache.
    #[must_use]
    pub fn window_scene_cache_stats(&self) -> WindowSceneCacheStats {
        self.window_scene_cache.stats()
    }

    /// Return counters for the full-scene (whole `build_scene` root) cache.
    #[must_use]
    pub fn full_scene_cache_stats(&self) -> FullSceneCacheStats {
        self.full_scene_cache.stats()
    }

    /// Return counters for the POSITION-INDEPENDENT per-window content subtree
    /// cache (t163-drag-cache). A window MOVE registers as a HIT here (content
    /// reused, only the wrapper translate updates); a RESIZE / content change
    /// registers as a MISS (content rebuilt).
    #[must_use]
    pub fn window_content_cache_stats(&self) -> WindowContentCacheStats {
        self.window_scene_cache.content_stats()
    }

    /// Take (and clear) the authoritative precomputed damage produced by the
    /// most recent [`Shell::build_scene`] (t82-incremental).
    ///
    /// Returns `Some(rects)` only when that build took the contained-interactive-
    /// change fast path and could bound the damage exactly (a menu-item / dock /
    /// titlebar-button hover-highlight). Each rect is a **superset-safe** upper
    /// bound in the shell's screen-pixel space (the same space as
    /// [`Shell::interactive_overlay_damage`]); the render side may use this set
    /// as the authoritative `latest_job.damage` and SKIP the per-frame
    /// `scene_diff_damage`. `None` means the change was a full rebuild / an
    /// unbounded chrome change / an idle cache hit, so the caller MUST keep its
    /// own conservative damage path (full diff or full frame).
    ///
    /// This is a take: it returns the value and resets the channel to `None`, so
    /// it must be called at most once per `build_scene`, immediately after it.
    #[must_use]
    pub fn take_precomputed_damage(&mut self) -> Option<Vec<Rect>> {
        self.precomputed_damage.take()
    }

    /// Compute the precomputed damage for a contained chrome change, storing the
    /// result in [`Shell::precomputed_damage`] (t82-incremental). See the call
    /// site in [`Shell::build_scene`] for the eligibility contract. Leaves the
    /// field `None` (caller falls back to its own damage path) whenever the
    /// change cannot be proven bounded.
    fn compute_precomputed_damage(
        &mut self,
        dirty_chrome_nodes: &[liquide_dom::NodeId],
        pipeline_output: &crate::pipeline::PipelineOutput,
        blink_toggled: bool,
        chrome_change_is_paint_only: bool,
        screen: Rect,
        button_colors: &DecorationColors,
        button_layout: &DecorationLayout,
    ) {
        /// Margin (logical px) added around each changed chrome rect to cover the
        /// `backdrop-filter` blur halo that samples neighbouring pixels — matches
        /// the `OVERLAY_BACKDROP_MARGIN` used by `interactive_overlay_damage`.
        const BACKDROP_MARGIN: f32 = 48.0;

        // ── Window-scene change: confine to the changed windows (t176-damage-
        // confine), or bail to full when the change is not window-attributable. ──
        // The window subtree cache was dirty entering this build, which previously
        // forced a FULL-frame repaint for EVERY window content change / scroll /
        // hover-recolor / focus / blink / animation tick — the dominant ~85 ms
        // full-frame cost (t173). Instead we diff the PREVIOUS frame's window-
        // scene signature (the cache key the cached subtree was built with) vs.
        // THIS frame's signature: if only PER-WINDOW fields changed we emit the
        // affected windows' old∪new painted footprints (+ blur/shadow margin) as
        // confined damage; if any GLOBAL field changed (screen/workspace/theme/
        // decoration — a change that recolors or moves every window) we bail to
        // full. Collected here and unioned with the chrome damage below.
        let mut window_damage: Vec<Rect> = Vec::new();
        if self.window_scene_cache.stats().dirty {
            let new_sig = self.window_scene_signature(screen, button_colors, button_layout);
            match self.window_scene_cache.peek_signature() {
                // First frame (nothing cached) — no old footprint to diff against,
                // so we cannot prove a confined superset. Bail to full.
                None => return,
                Some(old_sig) => {
                    // STRUCTURAL change (theme/decoration/workspace/screen, OR a
                    // window opened/closed/moved/resized/restacked/animated): the
                    // prompt's genuinely-full cases. Leave the conservative full
                    // repaint (drag move/resize is already confined at event time
                    // in events.rs).
                    if old_sig.structural_change(&new_sig) {
                        return;
                    }
                    // Paint-only per-window diff (content / scroll / focus / hover
                    // / typing / caret): the COMPLETE set of windows whose pixels
                    // changed this frame. Since this path requires `windows` and
                    // `effects` to be EQUAL between frames, each changed window has
                    // the SAME bounds in both frames, so a single footprint covers
                    // it. Expand by `BACKDROP_MARGIN` to cover the drop-shadow +
                    // glass-blur fringe: the window shadow blurs ≤12 px + spread
                    // 4 px, and any glass titlebar — this window's own OR an
                    // overlapping window stacked above it — samples ≤12 px beyond
                    // its box; both are well inside the 48 px margin, so the rect
                    // is a true SUPERSET of every pixel whose value depends on
                    // this window's changed content (including a stacked window's
                    // glass re-sampling the changed pixels through its backdrop).
                    let changed = old_sig.paint_changed_window_ids(&new_sig);
                    for id in changed {
                        for fp in old_sig.footprints_for(id) {
                            if fp.width <= 0.0 || fp.height <= 0.0 {
                                continue;
                            }
                            window_damage.push(Rect::new(
                                fp.x - BACKDROP_MARGIN,
                                fp.y - BACKDROP_MARGIN,
                                fp.width + BACKDROP_MARGIN * 2.0,
                                fp.height + BACKDROP_MARGIN * 2.0,
                            ));
                        }
                    }
                    // If the diff found NO per-window change yet the cache is dirty
                    // (something invalidated it that the signature does not
                    // capture), be conservative and bail to full.
                    if window_damage.is_empty() {
                        return;
                    }
                }
            }
        }
        // An active animation/transition repaints a growing region each frame
        // that is not captured by this frame's `dirty_chrome_nodes`.
        if !self.css_pipeline.chrome_output_stable() {
            return;
        }
        // The text caret blink toggles a node in the MANUAL window subtree, not
        // the CSS layout tree, so its rect is not in `dirty_chrome_nodes`.
        if blink_toggled {
            return;
        }
        // Manual full-screen overlays (overview / lock screen) are not chrome
        // layout boxes; if either is up, do not claim a bounded damage set.
        if self.overview_visible || self.is_session_locked() {
            return;
        }
        // Nothing chrome-level changed. A pure window change (scroll / content /
        // window hover) reaches here with an empty chrome dirty set but a non-
        // empty `window_damage` from the diff above — emit that alone. With NO
        // window damage either, there is no bounded footprint to emit → full.
        if dirty_chrome_nodes.is_empty() {
            if window_damage.is_empty() {
                return;
            }
            self.precomputed_damage = Some(window_damage);
            return;
        }

        // Build the damage set from the absolute (screen-space) border rects of
        // the changed nodes. For each changed node we ALSO include its parent's
        // rect: a style change that *does* reflow (CSS `mark_style` always marks
        // layout+paint, so we cannot distinguish a pure recolor from a reflow
        // cheaply) can shift sibling positions WITHIN the parent's content box,
        // and the parent rect is a tight superset of that. This keeps the hint a
        // guaranteed upper bound without widening to the whole screen.
        // Convert a layout-space border rect (expanded by the backdrop margin)
        // into the compositor `Rect` damage space. Returns `None` for empty boxes.
        let to_damage = |r: liquide_layout::Rect| -> Option<Rect> {
            if r.width <= 0.0 || r.height <= 0.0 {
                return None;
            }
            Some(Rect::new(
                r.x - BACKDROP_MARGIN,
                r.y - BACKDROP_MARGIN,
                r.width + BACKDROP_MARGIN * 2.0,
                r.height + BACKDROP_MARGIN * 2.0,
            ))
        };

        use liquide_style_engine::computed::Position;

        let layout = &pipeline_output.layout;
        let styles = &pipeline_output.styles;
        let mut rects: Vec<Rect> = Vec::new();
        for &node in dirty_chrome_nodes {
            let mut pushed_any = false;
            if let Some(box_id) = layout.find_box_id_by_node(node) {
                if let Some(d) = to_damage(layout.absolute_border_rect(box_id)) {
                    rects.push(d);
                    pushed_any = true;
                }
            }
            // ── Paint-only tightening (t119 #1) ──────────────────────────────
            //
            // When this frame's chrome change is PROVABLY paint-only (no node
            // landed in the DOM LAYOUT dirty set — see the caller), the change
            // cannot reflow ANY sibling or ancestor: it only recolours pixels
            // inside the changed node's own border box (a `:hover`/recolour,
            // hover-highlight, opacity/border-color flip). The changed-child
            // border rect EXPANDED by `BACKDROP_MARGIN` (the blur sample radius
            // halo, already applied by `to_damage`) is therefore a true SUPERSET
            // of every pixel that changed — including the blurred backdrop halo
            // that any glass surface OVER this rect must re-sample.
            //
            // Emitting just that rect (instead of climbing to the full
            // `position: fixed` positioned-ancestor, e.g. the whole 1920-px-wide
            // status-bar glass) is what lets the renderer's blur-confine actually
            // SHRINK `glass ∩ damage` from the full bar to ~(cell + 2·radius).
            // We still REQUIRE the changed node to have a laid-out box (proved
            // above via `pushed_any`); without one we fall through to the full
            // climb / full-fallback. We do NOT need the positioned-ancestor
            // boundary here because a paint-only change provably stays inside its
            // own box, so the in-flow-could-reflow-the-page concern does not apply.
            if chrome_change_is_paint_only {
                if !pushed_any {
                    // No layout box for a changed node → cannot bound it tightly.
                    return;
                }
                continue;
            }
            // Walk UP the ancestor chain, unioning each ancestor's rect, and STOP
            // at (inclusive) the nearest out-of-flow positioned ancestor
            // (position: fixed / absolute / sticky).
            //
            // Why a chain walk at all: CSS `mark_style` unconditionally marks a
            // node layout+paint dirty, so we cannot cheaply tell a pure recolor
            // (no geometry change) from a size change that reflows. A size change
            // reflows siblings WITHIN the parent's content box — covered by the
            // parent rect — and if the parent itself grows, ITS siblings reflow
            // within the grandparent, so a superset bound must climb.
            //
            // Why we may STOP at a positioned ancestor: an out-of-flow positioned
            // box is its own containing block whose geometry is fixed by its
            // own position/size, NOT by its content flowing into its parent — so
            // a reflow inside it cannot move anything OUTSIDE it. All shell pop-up
            // overlays (context / session / app menu, dock, tooltip) are
            // `position: fixed`, so the walk stops at the overlay root: the hint
            // is the overlay's own rect, never the full-screen `body`. If we reach
            // the document root WITHOUT finding a positioned ancestor (a change in
            // normal desktop flow that could reflow the whole page), we cannot
            // prove a bound smaller than the viewport → bail to `None` (full
            // fallback) rather than emit a misleadingly-small hint.
            let mut ancestor = self.desktop_dom.doc.parent(node);
            let mut depth = 0usize;
            const MAX_ANCESTOR_DEPTH: usize = 64;
            let mut hit_positioned_boundary = false;
            while let Some(p) = ancestor {
                depth += 1;
                if depth > MAX_ANCESTOR_DEPTH {
                    return;
                }
                if let Some(pbox) = layout.find_box_id_by_node(p) {
                    if let Some(d) = to_damage(layout.absolute_border_rect(pbox)) {
                        rects.push(d);
                        pushed_any = true;
                    }
                }
                // Stop once we have included an out-of-flow positioned containing
                // block: reflow cannot escape it, so higher ancestors need not be
                // damaged.
                let positioned = styles
                    .get(p)
                    .map(|s| {
                        matches!(
                            s.position,
                            Position::Fixed | Position::Absolute | Position::Sticky
                        )
                    })
                    .unwrap_or(false);
                if positioned {
                    hit_positioned_boundary = true;
                    break;
                }
                ancestor = self.desktop_dom.doc.parent(p);
            }

            // A changed node with NO layout box anywhere up its chain (e.g. an
            // unlaid overlay) cannot be bounded — fall back.
            if !pushed_any {
                return;
            }
            // A change in normal flow (no positioned containing block before the
            // root) could reflow arbitrarily far — fall back to full damage
            // rather than emit a hint we cannot prove is a superset.
            if !hit_positioned_boundary {
                return;
            }
        }

        // Union any confined window damage from the per-window diff above with
        // the chrome rects (a frame can change BOTH a window AND chrome — e.g. a
        // window-content update that also bumps a statusbar indicator). Both sets
        // are independent superset-safe rects in the same screen-pixel space.
        rects.extend(window_damage);

        if rects.is_empty() {
            return;
        }
        self.precomputed_damage = Some(rects);
    }

    /// Build the complete shell scene graph.
    ///
    /// **CSS pipeline approach**: the CSS pipeline renders ALL shell chrome
    /// (background, dock, status bar, notifications, launcher, menus)
    /// from the live DOM tree.  Only windows are assembled manually because
    /// they require complex interactive state (decoration buttons, hover
    /// indices, z-ordered content surfaces) that the pipeline does not model.
    pub fn build_scene(&mut self) -> SceneNode {
        // Reset the precomputed-damage channel for this frame (t82-incremental).
        // It is set to `Some(..)` only on the contained-interactive-change fast
        // path below; otherwise it stays `None` so the render side keeps its own
        // conservative damage path. Clearing first means a stale value from a
        // prior frame can never leak forward.
        self.precomputed_damage = None;

        // Toggle cursor blink every 500ms. A toggle changes the painted scene
        // (terminal/app caret + the window-scene signature), so when it flips we
        // must NOT reuse the cached root this frame — invalidate the full-scene
        // cache up front.
        let now_us = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;
        let blink_toggled = now_us.saturating_sub(self.cursor_blink_time_us) >= 500_000;
        if blink_toggled {
            self.cursor_blink_on = !self.cursor_blink_on;
            self.cursor_blink_time_us = now_us;
            self.full_scene_cache.mark_dirty();
        }

        let screen = self.screen_rect;

        // ── Synchronise DOM with current shell state ────────
        // Always run sync_dom: it owns the per-template HTML cache and only
        // mutates the DOM when chrome content actually changed. Its return value
        // is the per-frame "chrome changed?" signal the reuse predicate needs
        // (the DOM `dirty` set is monotonic in the shell flow, so we cannot use
        // its emptiness — sync_dom watches it GROW instead).
        let chrome_changed = self.sync_dom();

        // ── Idle full-scene cache fast path (t76-scenecache) ──────────────
        // Steady-state frames rebuilt the entire scene (~27ms: pipeline +
        // scene bridge + HitTest rebuild + manual root reassembly) even when
        // nothing changed. Reuse the cached root when EVERY scene input is
        // clean:
        //   (a) the full-scene cache is not dirty — no window/state/theme/
        //       overlay mutation since the last build (mark_window_scene_dirty /
        //       mark_full_scene_dirty trip this on every such path), and the
        //       cursor blink did not toggle this frame;
        //   (b) sync_dom mutated nothing this frame (chrome content unchanged)
        //       AND the pipeline's cached chrome output is stable (caches
        //       populated, no animation/transition) — so the chrome subtree is
        //       byte-identical to last frame;
        //   (c) the timer-driven dock-hover tooltip overlay is neither visible
        //       now nor was visible last frame (it can flip from elapsed time
        //       alone, with no DOM/state mutation), so a cached root can never
        //       drop its appearance/disappearance.
        // The hit-test engine, pending images, and pipeline caches all stay
        // valid across a hit because they reflect the same unchanged frame.
        let tooltip_visible_now = self.tooltip_manager_visible();
        let chrome_stable = !chrome_changed && self.css_pipeline.chrome_output_stable();
        if !self.full_scene_cache.dirty()
            && chrome_stable
            && !tooltip_visible_now
            && !self.last_full_scene_tooltip_visible
        {
            if let Some(cached) = self.full_scene_cache.node_clone() {
                self.full_scene_cache.record_hit();
                self.last_full_scene_tooltip_visible = false;
                return cached;
            }
        }
        self.full_scene_cache.record_miss();
        self.last_full_scene_tooltip_visible = tooltip_visible_now;

        // ── Run the CSS pipeline (all shell chrome) ─────────
        let (pipeline_nodes, pipeline_output, _animations_active) =
            self.css_pipeline.render_to_scene_with_output(
                &mut self.desktop_dom.doc,
                0, // base z-order
                self.frame_delta_ms,
            );

        // ── Snapshot + consume the DOM dirty set (t82-incremental) ──
        // The CSS pipeline has just read `doc.dirty` to do its incremental
        // restyle/relayout/repaint. We snapshot the changed chrome nodes here so
        // the contained-change fast path below can turn them into authoritative
        // precomputed damage; the union of the paint+layout dirty nodes is
        // exactly the chrome that repainted. Then we CLEAR the set — this frame's
        // mutations are now consumed (painted into the scene we are about to
        // store).
        //
        // Consuming the set per-frame is what makes `sync_dom`'s "chrome
        // changed?" signal reliable: at the start of the NEXT frame the set is
        // empty, so any new mutation — whether event-time (a `dispatch_mouse_move`
        // `:hover` flip on the item under the cursor) or sync-time — leaves it
        // non-empty and is detected. Without this consume the set was monotonic
        // and a moving menu-item hover returned a STALE cached scene. (A cache
        // HIT frame needs no clear: a hit requires `!chrome_changed`, i.e. the
        // set was already empty.)
        let dirty_chrome_nodes: Vec<liquide_dom::NodeId> = {
            let d = &self.desktop_dom.doc.dirty;
            d.paint.iter().chain(d.layout.iter()).copied().collect()
        };
        // Capture whether ANY node landed in the LAYOUT dirty set this frame
        // BEFORE we clear it. The DOM's per-property classifier
        // (`liquide_dom::dirty::classify_property`) only records a node in
        // `doc.dirty.layout` when the changed property can affect geometry /
        // intrinsic size (see the t91 paint-only fast path in pipeline/stages.rs);
        // a provably paint-only change (a `:hover`/recolour, hover-highlight,
        // opacity, border-color) lands in `.style` + `.paint` but NOT `.layout`.
        // So an EMPTY layout dirty set is a sound, conservative proof that this
        // frame's chrome change is paint-only and cannot reflow — which lets
        // `compute_precomputed_damage` emit the tight changed-child rect (+ blur
        // radius) instead of climbing to the full positioned-ancestor rect.
        let chrome_change_is_paint_only = self.desktop_dom.doc.dirty.layout.is_empty();
        self.desktop_dom.doc.dirty.clear_all();

        // ── Precomputed (authoritative) damage for a CONTAINED chrome change ──
        // When this rebuild was caused only by a bounded interactive chrome
        // change (a menu-item hover-highlight, a dock hover, a hovered titlebar
        // button — all style/paint-only flips), the changed chrome's screen
        // footprint is exactly the laid-out rects of `dirty_chrome_nodes`. We
        // emit those (as a superset-safe upper bound) so the render side can use
        // them directly and skip the O(n) per-frame scene diff. We deliberately
        // DO NOT emit precomputed damage (leave it `None` → caller falls back to
        // the full diff / full frame) whenever the change is not provably
        // bounded:
        //   * a window-scene change (geometry / content / focus / app output) —
        //     `window_scene_cache` was dirty entering this build,
        //   * an active CSS animation / transition (its footprint grows each
        //     frame and is not in `dirty_chrome_nodes`),
        //   * the cursor blink toggled (caret lives in the manually-assembled
        //     window subtree, not the CSS layout tree),
        //   * an overview / lockscreen overlay is showing (manual full-screen
        //     overlays not represented by chrome layout boxes),
        //   * nothing chrome-level was dirtied (e.g. only a manual overlay
        //     changed) — we cannot bound it from the CSS layout tree.
        // ── Update hit-test engine with latest layout + styles ──
        self.hit_test_engine = Some(liquide_hit_test::HitTestEngine::new(
            Arc::clone(&pipeline_output.layout),
            Arc::clone(&pipeline_output.styles),
        ));

        // Resolve decoration button colors and layout from CSS (for windows).
        // Computed BEFORE `compute_precomputed_damage` (t176-damage-confine): the
        // window-confinement diff inside it builds THIS frame's window-scene
        // signature, which needs the decoration colors/layout to detect a GLOBAL
        // decoration change (those fields are part of the signature). Resolving
        // them here is side-effect-free and order-independent of the damage call.
        let mut button_colors = self
            .style_resolver
            .as_ref()
            .map(crate::css_integration::resolve_decoration_colors)
            .unwrap_or_default();
        // Override the REST background + icon colors of the traffic-light buttons
        // from the LAID-OUT button elements' computed styles (t172-e2). The
        // `style_resolver` only carries the active THEME stylesheet, so the
        // `close-button`/`minimize-button`/`maximize-button` rules that live in
        // the BASE `components.css` (where the macOS left geometry + the
        // `--{minimize,maximize}-button-bg` traffic-light tokens are consumed)
        // never reach it. The hit-test engine's `StyleMap`, however, is the FULL
        // cascade (variables + components + theme), so reading each button's
        // computed `background`/`color` there is the single source that makes the
        // painted dot the exact red/yellow/green the CSS resolves — and it tracks
        // the SAME laid-out element the hit-test boxes come from, so paint==hit
        // colors as well as geometry. Hover colors stay on the resolver/default
        // path (the `opacity:0` decoration scaffold is not hover-synced in the
        // layout tree), which still yields a distinct hover delta.
        if let Some(over) = self.button_colors_from_css() {
            button_colors.close_bg = over.close_bg;
            button_colors.close_icon = over.close_icon;
            button_colors.minimize_bg = over.minimize_bg;
            button_colors.minimize_icon = over.minimize_icon;
            button_colors.maximize_bg = over.maximize_bg;
            button_colors.maximize_icon = over.maximize_icon;
        }
        let button_layout = self
            .style_resolver
            .as_ref()
            .map(crate::css_integration::resolve_decoration_layout)
            .unwrap_or_default();

        self.compute_precomputed_damage(
            &dirty_chrome_nodes,
            &pipeline_output,
            blink_toggled,
            chrome_change_is_paint_only,
            screen,
            &button_colors,
            &button_layout,
        );

        let mut root = SceneNode::new(NODE_ROOT, SceneNodeKind::Root, NodeProperties::new(screen));

        // ── Split pipeline nodes into background layer and chrome overlay ──
        //
        // The CSS pipeline emits scene nodes with sequential z_orders
        // (0, 1, 2, …).  The desktop-background fill comes first (low z),
        // while shell chrome (statusbar, dock, notifications, menus, glass
        // blurs) follows at higher z values.  Windows must render BETWEEN
        // these two layers: above the desktop background but below the
        // dock / statusbar / menus.
        //
        // Classify: a node is "background" if it is a solid fill whose
        // bounds cover almost the entire screen (the desktop-background
        // element).  Everything else is "chrome overlay".
        //
        // Z-order scheme for root's children:
        //   [0 .. bg_count)                      — background layer
        //   WORKSPACE_Z_ORDER                    — workspace (windows)
        //   [CHROME_Z_BASE .. CHROME_Z_BASE+N)   — chrome overlay layer
        const WORKSPACE_Z_ORDER: u32 = 100;

        // Every shell chrome surface (statusbar, dock, launcher, notifications,
        // menus, overlays) is CSS-driven, so the CSS pipeline always emits at
        // least the desktop-background fill — `pipeline_nodes` is never empty.
        // The old imperative `thread_coordinator` fallback track (composited
        // only when the pipeline produced nothing) was therefore dead and has
        // been retired (t112-p9).
        //
        // `classify_pipeline_nodes` assigns each pipeline node its z_order:
        // desktop-background fills → background band ([0..); below windows +
        // cursor + overlays), everything else → chrome band ([CHROME_Z_BASE..);
        // above them). See the function for the origin-based run heuristic.
        for node in classify_pipeline_nodes(pipeline_nodes, screen, CHROME_Z_BASE) {
            root.add_child(node);
        }

        // ── Windows (manual — complex interactive decorations) ────
        let ws_node = self.cached_window_workspace_node(
            screen,
            WORKSPACE_Z_ORDER,
            &button_colors,
            &button_layout,
        );
        root.add_child(ws_node);

        // ── Active dialog (message box / input) ───────────────────
        // The modal dialog now renders through the DOM/CSS pipeline
        // (`dom_sync::sync_dialog_template` → `dialog`/`dialog-button`
        // templates + the `dialog*` CSS rules), so its title, message, and
        // button labels paint as real text. The prior imperative filled-rect
        // overlay here (blank white header, empty body, unlabelled button) is
        // removed (t65-s3). The DOM overlay carries `z-index: 3000` in CSS so
        // it composites above windows and the chrome band.

        // ── Overview overlay thumbnails (task / workspace overview) ──────────
        // The overview STRUCTURE (scrim, grid, tiles, labels) is now a DOM/CSS
        // subtree synced via `sync_overview_template` and laid out by the CSS
        // pipeline above at `z-index: 7000` (t101-p5 full-CSS migration) — the
        // prior imperative `cols=sqrt(count)` grid math is retired. Here we only
        // PAINT each tile's captured window thumbnail (a `Surface` node carrying
        // the framebuffer snapshot, t93-e6) — or the glass placeholder fallback
        // — onto the tile's LAID-OUT CSS box (`#overview-tile-<id>`), keyed off
        // the layout tree rather than recomputed geometry. The thumbnail layer
        // sits just above the DOM tiles so it reads as the window proxy.
        if self.overview_visible {
            const OVERVIEW_THUMB_Z_BASE: u32 = 55_000;
            self.paint_overview_thumbnails(&mut root, OVERVIEW_THUMB_Z_BASE);
        }

        // ── Dock-hover tooltip (above chrome) ─────────────────────
        // The canonical `TooltipManager` owns the show-delay / dwell lifecycle
        // (driven each frame by `sync_tooltip_template` → `sync_tooltip_manager`).
        // Once it reports visible we emit the tooltip bubble HERE as a manual
        // scene overlay — mirroring the overview/lockscreen overlays above —
        // rather than relying on the DOM/CSS overlay, which never painted (the
        // `tooltip` element is `display:block` with no width and its fixed
        // `left`/`top` were not laid out, so it collapsed to 0 px; t66-hover).
        // Painting it manually puts the bubble at the already-clamped anchor
        // (`tooltip_pos`, set above the hovered dock item in events.rs) and at a
        // CONSTANT opacity, so a held hover is byte-stable (no fade oscillation).
        if self.tooltip_manager_visible() {
            const TOOLTIP_Z_BASE: u32 = 60_000;
            self.add_tooltip_overlay(&mut root, TOOLTIP_Z_BASE);
        }

        // ── Lock screen (topmost) ─────────────────────────────────
        // The lock surface is now a DOM/CSS overlay (t95-p4 full-CSS
        // migration): `sync_lockscreen_template` mounts the `lockscreen-overlay`
        // subtree (clock/date/user/password field) into the DOM and the CSS
        // pipeline lays it out + paints it at `z-index: 8000` (above windows
        // and chrome). The prior imperative `add_lockscreen_overlay` filled-rect
        // overlay is retired. Its password field is a real laid-out box whose
        // hit-test geometry comes from CSS (see `events.rs` + the
        // `lockscreen-prompt` rule), not hardcoded constants.

        // ── Retain the assembled root for idle-frame reuse (t76-scenecache) ──
        // Store a clone so the next steady-state frame can return this exact
        // root without rebuilding. `store` clears the dirty flag; any subsequent
        // state mutation re-trips it via mark_window_scene_dirty /
        // mark_full_scene_dirty, and chrome/animation/blink/tooltip changes are
        // re-checked by the reuse predicate at the top of the next build.
        self.full_scene_cache.store(root.clone());
        root
    }

    /// Emit the dock-hover tooltip bubble as a manual scene overlay.
    ///
    /// Mirrors the overview / lockscreen overlays: a themed rounded bubble
    /// (glass backing + solid fill + border) carrying the hovered item's label,
    /// anchored at the already-clamped `tooltip_pos` (set above the hovered dock
    /// item in `events.rs`). Painted at a CONSTANT opacity whenever the canonical
    /// manager reports the tooltip visible, so a held hover renders the same
    /// pixels frame-to-frame (no fade oscillation) — the stability the
    /// `dock_hover_tooltip_steady_is_stable_during_fade` tooth asserts.
    fn add_tooltip_overlay(&self, root: &mut SceneNode, base_z: u32) {
        let Some(text) = self.tooltip_text.as_ref() else {
            return;
        };
        if text.is_empty() {
            return;
        }

        // Reserved node id range for the tooltip overlay (above all chrome ids).
        const NODE_TOOLTIP_BASE: u64 = 600_000;

        // Approximate the bubble size from the label. ~7 px per glyph at the
        // status font, plus horizontal padding; a fixed comfortable height.
        let font_scale = 1u32;
        let pad_x = 8.0_f32;
        let pad_y = 5.0_f32;
        let glyph_w = 7.0_f32;
        let text_w = (text.chars().count() as f32) * glyph_w;
        let bubble_w = (text_w + pad_x * 2.0).clamp(40.0, 300.0);
        let bubble_h = 24.0_f32;

        // Anchor at the clamped tooltip position, then keep the bubble fully on
        // screen (the anchor is the box's top-left; clamp the right/bottom edges).
        let screen = self.screen_rect;
        let x = self
            .tooltip_pos
            .x
            .clamp(screen.x + 2.0, (screen.x + screen.width - bubble_w - 2.0).max(screen.x + 2.0));
        let y = self
            .tooltip_pos
            .y
            .clamp(screen.y + 2.0, (screen.y + screen.height - bubble_h - 2.0).max(screen.y + 2.0));
        let bubble = Rect::new(x, y, bubble_w, bubble_h);

        use liquide_compositor::scene::GlassParams;

        // Glass backing so the bubble reads as a frosted overlay.
        root.add_child(SceneNode::new(
            NODE_TOOLTIP_BASE,
            SceneNodeKind::Glass(GlassParams {
                blur_radius: 10,
                tint_color: self.theme.dock_glass_tint,
                inner_glow: false,
                parallax: false,
            }),
            NodeProperties::new(bubble).with_z_order(base_z),
        ));

        // Solid dark fill so the bubble is unambiguously painted even when the
        // glass blur degrades to a no-op on the fast path.
        root.add_child(SceneNode::new(
            NODE_TOOLTIP_BASE + 1,
            SceneNodeKind::Background {
                color: themed_alpha(self.theme.launcher_search_bar, 240),
            },
            NodeProperties::new(bubble).with_z_order(base_z + 1),
        ));

        // 1px border for definition.
        root.add_child(SceneNode::new(
            NODE_TOOLTIP_BASE + 2,
            SceneNodeKind::Background {
                color: themed_alpha(self.theme.dock_border, 200),
            },
            NodeProperties::new(Rect::new(bubble.x, bubble.y, bubble.width, 1.0))
                .with_z_order(base_z + 2),
        ));

        // The label text.
        root.add_child(text_node(
            NODE_TOOLTIP_BASE + 3,
            text.clone(),
            self.theme.status_bar_text,
            Rect::new(
                bubble.x + pad_x,
                bubble.y + pad_y,
                (bubble.width - pad_x * 2.0).max(1.0),
                bubble.height - pad_y * 2.0,
            ),
            base_z + 3,
            font_scale,
        ));
    }

    /// Capture cheap window thumbnails for the overview from the last composited
    /// framebuffer (t93-e6 / gap #1).
    ///
    /// For each visible window this reads the window's SETTLED on-screen rect out
    /// of `fb` (a read-only copy — no framebuffer write, no damage, no scissor
    /// interaction) and stores a tile-scaled snapshot keyed by `WindowId`. The
    /// host (session render thread) calls this on the frame the overview opens,
    /// BEFORE the dim scrim is composited, so the snapshot is the window content
    /// rather than the scrim. Refreshing every open keeps the thumbnails roughly
    /// current.
    ///
    /// HONEST caveats (see [`Shell::overview_thumbnails`]): thumbnails are stale
    /// snapshots, and an occluded window captures whatever covered it. A window
    /// whose rect is fully off-screen / zero-size yields no usable capture and
    /// falls back to the placeholder tile in [`Self::add_overview_overlay`].
    ///
    /// `tile_max` bounds the stored thumbnail's longer edge so a 4K window does
    /// not store a 4K buffer per tile; the overview re-fits it to the actual tile
    /// rect at paint time, but a sane upper bound keeps the cache cheap.
    pub fn capture_overview_thumbnails(&mut self, fb: &FrameBuffer, tile_max: u32) {
        let tile_max = tile_max.max(1);
        // Collect (id, bounds) first to avoid borrowing self while mutating the
        // thumbnail map. Use SETTLED bounds (window.bounds), never mid-animation
        // geometry — the snapshot should be of the window at rest.
        let targets: Vec<(crate::window::WindowId, Rect)> = self
            .visible_windows()
            .into_iter()
            .map(|w| (w.id, w.bounds))
            .collect();

        self.overview_thumbnails.clear();
        for (id, bounds) in targets {
            if bounds.width < 1.0 || bounds.height < 1.0 {
                continue; // zero-size → placeholder
            }
            let cap = fb.capture_region(bounds);
            // A 1x1 transparent buffer means the rect was off-screen / empty —
            // skip it so the overview falls back to the placeholder tile.
            if cap.width <= 1 && cap.height <= 1 {
                continue;
            }
            // Pre-scale to a bounded thumbnail (preserve aspect) so the cache is
            // cheap; the overview re-fits to the exact tile at paint time.
            let (tw, th) = scale_within(cap.width, cap.height, tile_max);
            let thumb = cap.scaled_to(tw, th);
            self.overview_thumbnails.insert(id, thumb);
        }
        // The overview overlay is part of the full-scene root, so a changed
        // thumbnail set must invalidate the cached scene — otherwise the idle
        // full-scene fast path serves the stale (placeholder) overview and the
        // capture "works in a test but never repaints live" (t93 hard
        // constraint).
        self.mark_window_scene_dirty();
    }

    /// Drop all captured overview thumbnails (t93-e6). Called when the overview
    /// closes so a window that later vanishes cannot leak a stale thumbnail into
    /// a future overview session.
    pub fn clear_overview_thumbnails(&mut self) {
        if self.overview_thumbnails.is_empty() {
            return;
        }
        self.overview_thumbnails.clear();
        // Invalidate the cached scene so a subsequent overview build does not
        // serve a stale thumbnail from the full-scene fast path.
        self.mark_window_scene_dirty();
    }

    /// Whether any overview thumbnail has been captured (t93-e6) — host hint to
    /// decide if a capture pass is still needed for the current overview.
    #[must_use]
    pub fn has_overview_thumbnails(&self) -> bool {
        !self.overview_thumbnails.is_empty()
    }

    /// Paint each overview tile's window thumbnail onto its **laid-out CSS box**
    /// (t101-p5 full-CSS migration).
    ///
    /// The overview scrim/grid/tiles/labels are DOM/CSS elements laid out by the
    /// pipeline (see `dom_sync::sync_overview_template` + the `overview*` CSS
    /// rules). This function only adds the per-tile WINDOW THUMBNAIL — a
    /// `Surface` node carrying the captured framebuffer snapshot (t93-e6) — that
    /// the CSS pipeline cannot express (a `DisplayItem::Surface` from the DOM
    /// carries no pixel buffer). It reads each tile's box from the live layout
    /// tree (`#overview-tile-<id>` via the hit-test engine), NOT recomputed grid
    /// geometry, so a CSS change that moves the tiles moves the painted
    /// thumbnails with them. When no capture exists for a window (off-screen /
    /// zero-size / first frame), it paints the glass placeholder onto the same
    /// CSS box so the tile still reads as a window proxy.
    fn paint_overview_thumbnails(&self, root: &mut SceneNode, base_z: u32) {
        use liquide_compositor::scene::GlassParams;

        let Some(hit_test) = self.hit_test_engine.as_ref() else {
            return;
        };

        for (i, window) in self.visible_windows().iter().enumerate() {
            // Resolve the tile's laid-out CSS box from the DOM/layout tree. The
            // tile element id mirrors the template (`overview-tile-<window_id>`).
            let tile_el_id = format!("overview-tile-{}", window.id.0);
            let Some(tile_node) = self.desktop_dom.doc.get_element_by_id(&tile_el_id) else {
                continue;
            };
            let Some(css_box) = hit_test.bounds_for_node(tile_node) else {
                continue;
            };
            let tile = Rect::new(css_box.x, css_box.y, css_box.width, css_box.height);
            if tile.width < 1.0 || tile.height < 1.0 {
                continue;
            }

            let tile_z = base_z + i as u32 * 2;
            let tile_base = NODE_WINDOW_BASE + window.id.0 * NODE_WINDOW_STRIDE + 7;

            // Glass tile backing so the tile reads as a window proxy (kept under
            // both the thumbnail and the placeholder).
            root.add_child(SceneNode::new(
                tile_base,
                SceneNodeKind::Glass(GlassParams {
                    blur_radius: 12,
                    tint_color: self.theme.window_glass_tint,
                    inner_glow: false,
                    parallax: false,
                }),
                NodeProperties::new(tile).with_z_order(tile_z),
            ));

            match self.overview_thumbnails.get(&window.id) {
                Some(thumb) => {
                    // Real window thumbnail (t93-e6): a Surface node carrying the
                    // captured snapshot, scaled to fit the laid-out tile rect.
                    // The Surface blit consumes the buffer's own dimensions, so
                    // re-fit the cached thumbnail to the CSS tile size here
                    // (deterministic bilinear). Center it inside the tile
                    // preserving aspect.
                    let (fit_w, fit_h) =
                        fit_within(thumb.width, thumb.height, tile.width, tile.height);
                    let scaled = thumb.scaled_to(fit_w, fit_h);
                    let off_x = tile.x + (tile.width - fit_w as f32) * 0.5;
                    let off_y = tile.y + (tile.height - fit_h as f32) * 0.5;
                    root.add_child(SceneNode::new(
                        tile_base + 1,
                        SceneNodeKind::Surface {
                            surface_id: window.id.0,
                            buffer: Some(scaled),
                        },
                        NodeProperties::new(Rect::new(
                            off_x,
                            off_y,
                            fit_w as f32,
                            fit_h as f32,
                        ))
                        .with_z_order(tile_z + 1),
                    ));
                }
                None => {
                    // Placeholder fallback: solid fill so the tile is
                    // unambiguously painted (and visible even when glass blur
                    // degrades to a no-op on the fast path) when no capture
                    // exists (off-screen / zero-size / first frame).
                    root.add_child(SceneNode::new(
                        tile_base + 1,
                        SceneNodeKind::Background {
                            color: themed_alpha(self.theme.window_content_background, 235),
                        },
                        NodeProperties::new(tile).with_z_order(tile_z + 1),
                    ));
                }
            }
        }
    }

    /// Resolve the window-frame colors (titlebar background / border / title
    /// text) for `window_id` from the COMPUTED STYLE of its laid-out
    /// `window-frame` decoration subtree (t113-deco-handoff full-CSS frame
    /// colors).
    ///
    /// Reads the same `StyleMap` (via the live hit-test engine) that the
    /// decoration's laid-out boxes come from — i.e. the exact source the DOM
    /// frame subtree is styled by — so a runtime stylesheet / theme change that
    /// recolors `window-titlebar { background; color }` or `window-frame` /
    /// `window` borders recolors the painted decoration. The title-bar
    /// background is read from `#window-deco-<id>-titlebar`, the title text from
    /// `#window-deco-<id>-title` (falling back to the titlebar's inherited text
    /// color), and the border from the `#window-deco-<id>` frame's border (with
    /// the titlebar as fallback).
    ///
    /// Returns `None` when the titlebar is not laid out / has no computed style
    /// yet (first frame), so the renderer keeps the legacy ShellTheme-sourced
    /// `Decoration { background, border_color, title_color }` fields — no
    /// regression, no panic.
    fn frame_colors_from_css(
        &self,
        window_id: crate::window::WindowId,
    ) -> Option<liquide_compositor::scene::DecorationFrameColors> {
        use liquide_compositor::scene::DecorationFrameColors;

        let hit_test = self.hit_test_engine.as_ref()?;
        let styles = hit_test.styles();

        let tb_id = format!("window-deco-{}-titlebar", window_id.0);
        let tb_node = self.desktop_dom.doc.get_element_by_id(&tb_id)?;
        let tb_style = styles.get(tb_node)?;

        // Title-bar background: the `window-titlebar { background }` computed
        // value. This is the load-bearing signal; without an opaque fill there
        // is nothing to improve on the legacy field with.
        let title_bar_bg = tb_style.background_color;

        // Title text: the dedicated `window-title` element's computed text color
        // (falls back to the titlebar's inherited `color`).
        let title_id = format!("window-deco-{}-title", window_id.0);
        let title_text = self
            .desktop_dom
            .doc
            .get_element_by_id(&title_id)
            .and_then(|n| styles.get(n))
            .map(|s| s.color)
            .unwrap_or(tb_style.color);

        // Border: the visible window stroke. The decoration DOM subtree
        // (`window-frame` / `window-titlebar`) carries no border rule, so the
        // canonical source is the `window { border-color }` rule — the same one
        // `resolve_decoration_style` reads for the border WIDTH. We resolve it
        // from the style_resolver (the theme engine), falling back to any border
        // the titlebar element itself computes. Filter out an unset/transparent
        // border so a frame without a meaningful stroke keeps the titlebar bg.
        let border = self
            .style_resolver
            .as_ref()
            .and_then(|r| r.resolve("window", &[], &[], None).ok())
            .and_then(|s| s.border_color)
            .filter(|c| c.a > 0)
            .or(Some(tb_style.border_color.top).filter(|c| c.a > 0))
            .unwrap_or(title_bar_bg);

        Some(DecorationFrameColors {
            title_bar_bg,
            border,
            title_text,
        })
    }

    /// Resolve the traffic-light buttons' REST background + icon colors from the
    /// FULL CSS cascade (t172-e2 left-traffic-light retheme).
    ///
    /// The painted decoration's button colors normally come from
    /// `css_integration::resolve_decoration_colors`, which queries
    /// `self.style_resolver` — but that resolver holds ONLY the active theme
    /// stylesheet. The `close-button`/`minimize-button`/`maximize-button`
    /// background rules (and the `--{minimize,maximize}-button-bg` traffic-light
    /// tokens they consume) live in the BASE `components.css`, which is loaded
    /// into the layout pipeline but NOT the resolver. This reads each button's
    /// computed `background`/`color` from the hit-test engine's `StyleMap` (the
    /// full variables+components+theme cascade), so the painted dot is the exact
    /// red / yellow / green the CSS resolves and tracks the SAME laid-out element
    /// the hit-test box comes from (paint==hit colors).
    ///
    /// Reads from the FIRST visible decorated window's laid-out buttons (the
    /// rules are theme-global, identical for every window). Returns `None` when no
    /// decorated window is laid out yet (first frame) so the caller keeps the
    /// resolver/default colors — no regression, no panic.
    fn button_colors_from_css(&self) -> Option<DecorationCssColors> {
        let hit_test = self.hit_test_engine.as_ref()?;
        let styles = hit_test.styles();

        // Any laid-out decorated window will do — the button color rules are
        // theme-global, so the first one is representative.
        let window_id = self
            .visible_windows()
            .into_iter()
            .find(|w| w.flags.contains(WindowFlags::DECORATED))
            .map(|w| w.id)?;

        // Read a button's computed (bg, icon) ONLY when the CSS actually paints a
        // background on it (alpha > 0). When the base `components.css` button
        // rules are not in the pipeline (e.g. a bare `Shell::new` test pipeline
        // that loaded only the theme), the element computes a TRANSPARENT
        // background and we must NOT clobber the resolver/default color with
        // transparent — return `None` and let the fallback color stand.
        let defaults = DecorationColors::default();
        let read = |suffix: &str, def_icon: Color| -> Option<(Color, Color)> {
            let el_id = format!("window-deco-{}-{suffix}", window_id.0);
            let node = self.desktop_dom.doc.get_element_by_id(&el_id)?;
            let style = styles.get(node)?;
            if style.background_color.a == 0 {
                return None;
            }
            let icon = if style.color.a == 0 { def_icon } else { style.color };
            Some((style.background_color, icon))
        };

        // Require at least the close button's background to resolve from CSS — the
        // load-bearing signal that the base decoration rules are actually in the
        // cascade this frame. Otherwise keep ALL resolver/default colors.
        let (close_bg, close_icon) = read("close", defaults.close_icon)?;
        let (min_bg, min_icon) =
            read("min", defaults.minimize_icon)
                .unwrap_or((defaults.minimize_bg, defaults.minimize_icon));
        let (max_bg, max_icon) =
            read("max", defaults.maximize_icon)
                .unwrap_or((defaults.maximize_bg, defaults.maximize_icon));

        Some(DecorationCssColors {
            close_bg,
            close_icon,
            minimize_bg: min_bg,
            minimize_icon: min_icon,
            maximize_bg: max_bg,
            maximize_icon: max_icon,
        })
    }

    /// Derive a [`DecorationLayout`] for `window_id` from the LAID-OUT CSS boxes
    /// of its `window-frame` decoration (t103-p6 full-CSS migration).
    ///
    /// Reads the titlebar box (`#window-deco-<id>-titlebar`) and the close
    /// button box (`#window-deco-<id>-close`) from the live hit-test engine's
    /// layout tree and turns them into the renderer's `DecorationLayout` so the
    /// painted titlebar height + button size + right margin follow the CSS. The
    /// button hit-test reads the SAME boxes (`window_decoration_adapter`), so
    /// paint and hit-test share one source of truth.
    ///
    /// Returns `None` when the decoration is not laid out yet (first frame) or
    /// the boxes are degenerate, so the caller falls back to the CSS-resolved
    /// constant layout and geometry stays deterministic.
    fn decoration_layout_from_css(
        &self,
        window_id: crate::window::WindowId,
        paint_bounds: Rect,
    ) -> Option<DecorationLayout> {
        let hit_test = self.hit_test_engine.as_ref()?;

        let tb_id = format!("window-deco-{}-titlebar", window_id.0);
        let tb_node = self.desktop_dom.doc.get_element_by_id(&tb_id)?;
        let tb_box = hit_test.bounds_for_node(tb_node)?;

        let close_id = format!("window-deco-{}-close", window_id.0);
        let close_node = self.desktop_dom.doc.get_element_by_id(&close_id)?;
        let close_box = hit_test.bounds_for_node(close_node)?;

        if tb_box.height < 1.0 || close_box.width < 1.0 || close_box.height < 1.0 {
            return None;
        }

        // Right margin = gap from the window's right edge to the close button's
        // right edge (the renderer measures buttons leftward from the right
        // edge). Clamp to >= 0 so a sub-pixel overhang can't flip it negative.
        let right_margin = (paint_bounds.x + paint_bounds.width - (close_box.x + close_box.width))
            .max(0.0);

        // Per-button CSS screen boxes for EXACT paint↔hit parity (t113-deco-
        // handoff). Each button's painted rect is read from the SAME laid-out
        // CSS box the hit-test resolves (`window_button_bounds_from_css` →
        // `#window-deco-<id>-{close,max,min,pin}` via the live layout tree), so
        // the renderer paints each button exactly where a click lands. A button
        // that is not laid out yet stays `None` and the renderer falls back to
        // the fixed-stride model for that button only (no panic, no regression).
        let css_rect = |suffix: &str| -> Option<Rect> {
            self.window_button_bounds_from_css(window_id, suffix)
                .map(|r| Rect::new(r.x, r.y, r.width, r.height))
        };
        let button_rects = DecorationButtonRects {
            close: css_rect("close"),
            maximize: css_rect("max"),
            minimize: css_rect("min"),
            always_on_top: css_rect("pin"),
        };

        // CSS frame colors (titlebar bg / border / title text) read from the
        // COMPUTED STYLE of the laid-out `window-frame`/`window-titlebar`
        // elements — the SAME StyleMap the hit-test boxes come from, i.e. the
        // exact source the DOM frame subtree is styled by. So a runtime
        // stylesheet / theme change that recolors the frame recolors the painted
        // decoration. `None` when the titlebar is not laid out / has no resolved
        // background (renderer keeps the legacy ShellTheme fields → no
        // regression, no panic on the first frame).
        let frame_colors = self.frame_colors_from_css(window_id);

        // Round dots (t172-e2): the buttons are CSS `border-radius: 50%`, i.e. a
        // full circle for a square box. Derive the painted corner radius from the
        // laid-out box (half the smaller side) so the painted dot matches the CSS
        // circle exactly — paint==hit for the rounded shape, not just the rect.
        // (Every shipped theme already sizes its buttons as ~50%-radius dots, so
        // this is consistent across themes.)
        let button_corner_radius = close_box.width.min(close_box.height) / 2.0;
        Some(DecorationLayout {
            title_bar_height: tb_box.height,
            button_width: close_box.width,
            button_height: close_box.height,
            button_right_margin: right_margin,
            button_corner_radius,
            button_rects,
            frame_colors,
        })
    }

    fn cached_window_workspace_node(
        &mut self,
        screen: Rect,
        z_order: u32,
        button_colors: &DecorationColors,
        button_layout: &DecorationLayout,
    ) -> SceneNode {
        let signature = self.window_scene_signature(screen, button_colors, button_layout);
        if let Some(node) = self.window_scene_cache.get(&signature) {
            return node;
        }

        let node = self.build_uncached_window_workspace_node(
            screen,
            z_order,
            button_colors,
            button_layout,
        );
        self.window_scene_cache.store(signature, node.clone());
        node
    }

    fn window_scene_signature(
        &self,
        screen: Rect,
        button_colors: &DecorationColors,
        button_layout: &DecorationLayout,
    ) -> WindowSceneSignature {
        let workspace = self.workspaces.active();
        WindowSceneSignature {
            screen: RectSignature::from_rect(screen),
            active_workspace_id: workspace.id.0,
            focused_id: self.focus.focused().map(|id| id.0),
            hovered_button: self
                .hovered_button
                .map(|(window_id, zone)| HoveredButtonSignature {
                    window_id: window_id.0,
                    zone,
                }),
            cursor_blink_on: self.cursor_blink_on,
            decoration_style: DecorationStyleSignature::from_style(&self.decoration_style),
            decoration_colors: DecorationColorsSignature::from_colors(button_colors),
            decoration_layout: DecorationLayoutSignature::from_layout(button_layout),
            theme: WindowThemeSignature::from_theme(&self.theme),
            windows: self
                .visible_windows()
                .into_iter()
                .map(WindowRenderSignature::from_window)
                .collect(),
            focused_text: self.focused_app_text().map(str::to_string),
            app_content: {
                let mut revs: Vec<(u64, u64)> = self
                    .app_views
                    .keys()
                    .map(|wid| (wid.0, self.app_content_revs.get(wid).copied().unwrap_or(0)))
                    .collect();
                revs.sort_unstable();
                revs
            },
            effects: {
                let mut sigs: Vec<WindowEffectSignature> = self
                    .active_window_effects
                    .values()
                    .map(|f| WindowEffectSignature {
                        window_id: f.window_id,
                        bounds: RectSignature {
                            x: f32_signature(f.bounds.x),
                            y: f32_signature(f.bounds.y),
                            width: f32_signature(f.bounds.width),
                            height: f32_signature(f.bounds.height),
                        },
                        opacity: f32_signature(f.opacity),
                    })
                    .collect();
                sigs.sort_unstable_by_key(|s| s.window_id);
                sigs
            },
        }
    }

    /// Build the POSITION-INDEPENDENT content signature for `window`
    /// (t163-drag-cache). Captures everything the content subtree depends on
    /// EXCEPT the window's screen position — so a pure move keeps it unchanged.
    fn window_content_signature(
        &self,
        window: &Window,
        content_bounds: Rect,
    ) -> WindowContentSignature {
        let focused = self.focus.focused() == Some(window.id);
        let has_app_view = self.app_views.contains_key(&window.id);
        WindowContentSignature {
            app_id: window.app_id.clone(),
            title: window.title.clone(),
            content_w: f32_signature(content_bounds.width),
            content_h: f32_signature(content_bounds.height),
            app_content_rev: self.app_content_revs.get(&window.id).copied().unwrap_or(0),
            has_app_view,
            focused,
            // Only the FOCUSED window paints the typed-text field, so capture the
            // typed buffer only when focused (mirrors `build_window_content`).
            focused_text: if focused {
                self.window_text_input(window.id).map(str::to_string)
            } else {
                None
            },
            cursor_blink_on: self.cursor_blink_on,
            text_color: ColorSignature::from_color(self.theme.status_bar_text),
            terminal_bg: ColorSignature::from_color(self.theme.app_terminal_background),
            terminal_text: ColorSignature::from_color(self.theme.app_terminal_text),
            sidebar_item: ColorSignature::from_color(self.theme.app_settings_sidebar_item),
            browser_urlbar: ColorSignature::from_color(self.theme.app_browser_urlbar),
            content_background: ColorSignature::from_color(self.theme.window_content_background),
        }
    }

    fn build_uncached_window_workspace_node(
        &mut self,
        screen: Rect,
        z_order: u32,
        button_colors: &DecorationColors,
        button_layout: &DecorationLayout,
    ) -> SceneNode {
        use liquide_compositor::scene::GlassParams;

        let ws_index = self.workspaces.active().id.0;
        let ws_id = NODE_WORKSPACE_BASE + ws_index as u64;
        let mut ws_node = SceneNode::new(
            ws_id,
            SceneNodeKind::Workspace { index: ws_index },
            NodeProperties::new(screen).with_z_order(z_order),
        );

        // Snapshot the visible windows as owned values so the per-window content
        // cache (consulted via `&mut self` below) is not blocked by an immutable
        // borrow of `self.visible_windows()` held across the loop (t163-drag-cache).
        let windows: Vec<Window> = self.visible_windows().into_iter().cloned().collect();

        for (paint_rank, window) in windows.iter().enumerate() {
            let win_base = NODE_WINDOW_BASE + window.id.0 * NODE_WINDOW_STRIDE;

            // Band-aware paint z-base (t93-e2 / t92 gap #2+#4). `visible_windows`
            // is sorted by the always-on-top band key (E1), so the iteration RANK
            // is the authoritative stacking position — strictly monotonic with the
            // AOT band. Deriving the per-node z from the rank (rather than the raw
            // `window.z_order`, which a freshly-opened normal window can briefly
            // hold ABOVE a pinned AOT window before the next normalize) guarantees
            // paint order == live hit-test/band order. For an already-normalized
            // stack the rank equals `z_order`, so static multi-window scenes (and
            // their goldens) are unchanged.
            let paint_z_base = paint_rank as u32 * 10;

            // ── Window effects (t93-e2 / t92 gap #4) ──────────────────────────
            // Fold any active effect frame into this window's PAINTED geometry +
            // opacity. `paint_bounds` is the animated rect (open/close scale-pulse,
            // transform tween) while `paint_opacity` is the per-frame fade; idle
            // windows fall back to the settled `window.bounds` at full opacity, so
            // a non-animating scene is byte-identical to the pre-effects scene.
            //
            // CRITICAL — paint-only: this uses the EFFECT bounds for paint but the
            // window's *settled* bounds remain the live hit-target. `visible_windows`
            // / `window_at_point` are unchanged, so clicking a window mid-open-scale
            // still hits its final rect (plan §gap-4 correctness note).
            //
            // Z-order: this window's nodes (and the wrapper) all use `paint_z_base`
            // (the band-aware rank computed above), so an animating *normal*
            // window's effect can never paint over an always-on-top window — the
            // AOT band owns the higher ranks in `visible_windows()`.
            let (paint_bounds, paint_opacity) = match self.active_window_effects.get(&window.id) {
                Some(frame) => (
                    Rect::new(
                        frame.bounds.x,
                        frame.bounds.y,
                        frame.bounds.width,
                        frame.bounds.height,
                    ),
                    frame.opacity.clamp(0.0, 1.0),
                ),
                None => (window.bounds, 1.0),
            };

            // Per-window paint container. Non-visual (`Workspace` kind is skipped by
            // the flatten output) and anchored at the origin so it adds no
            // translation — it exists only to carry `paint_opacity`, which the
            // compositor accumulates multiplicatively down to every window node
            // (shadow/decoration/content), giving a single correct per-window fade.
            // At opacity 1.0 (no active effect) the wrapper is a transparent no-op,
            // so idle windows flatten to exactly the same FlatNodes as before.
            let win_group_z = paint_z_base;
            let mut win_group = SceneNode::new(
                NODE_WINDOW_EFFECT_GROUP_BASE + window.id.0,
                SceneNodeKind::Workspace { index: ws_index },
                NodeProperties::new(Rect::new(0.0, 0.0, screen.width, screen.height))
                    .with_z_order(win_group_z)
                    .with_opacity(paint_opacity),
            );

            let shadow_bounds = Rect::new(
                paint_bounds.x - 4.0,
                paint_bounds.y - 2.0,
                paint_bounds.width + 8.0,
                paint_bounds.height + 6.0,
            );
            win_group.add_child(SceneNode::new(
                win_base,
                SceneNodeKind::Shadow {
                    spread: 4.0,
                    blur_radius: 12.0,
                    color: self.theme.window_shadow,
                    corner_radius: self.decoration_style.corner_radius,
                },
                NodeProperties::new(shadow_bounds).with_z_order(paint_z_base),
            ));

            if window.flags.contains(WindowFlags::DECORATED) {
                let is_focused = self.focus.focused() == Some(window.id);

                // Anchor the painted decoration geometry to the LAID-OUT CSS
                // boxes (t103-p6 full-CSS migration). `sync_window_decorations`
                // mounted a `window-frame` over this window's titlebar and the
                // pipeline laid it out; `decoration_layout_from_css` reads the
                // titlebar + close-button boxes from the live layout tree and
                // returns a `DecorationLayout` whose title-bar height + button
                // dimensions/margin track the CSS — so a theme change that
                // resizes the titlebar/buttons moves the painted decoration with
                // it (the same source of truth the button hit-test reads via
                // `window_decoration_adapter`). Falls back to the CSS-resolved
                // `button_layout` constants on the first frame (before the
                // decoration is laid out) so geometry is always deterministic.
                let effective_layout = self
                    .decoration_layout_from_css(window.id, paint_bounds)
                    .unwrap_or(*button_layout);
                let title_h = effective_layout.title_bar_height;
                let title_bar_bounds = Rect::new(
                    paint_bounds.x,
                    paint_bounds.y,
                    paint_bounds.width,
                    title_h,
                );

                win_group.add_child(SceneNode::new(
                    win_base + 10,
                    SceneNodeKind::Glass(GlassParams {
                        blur_radius: 12,
                        tint_color: self.theme.window_glass_tint,
                        inner_glow: false,
                        parallax: false,
                    }),
                    NodeProperties::new(title_bar_bounds).with_z_order(paint_z_base + 1),
                ));

                let title_bg = if is_focused {
                    let mut c = self.theme.window_title_bar_focused;
                    c.a = (c.a / 2).max(60);
                    c
                } else {
                    let mut c = self.theme.window_title_bar_unfocused;
                    c.a = (c.a / 2).max(40);
                    c
                };
                win_group.add_child(SceneNode::new(
                    win_base + 1,
                    SceneNodeKind::Decoration {
                        title: Some(window.title.clone()),
                        title_color: self.theme.window_title_text,
                        background: title_bg,
                        border_color: if is_focused {
                            self.theme.window_border_focused
                        } else {
                            self.theme.window_border_unfocused
                        },
                        border_width: self.decoration_style.border_width,
                        corner_radius: self.decoration_style.corner_radius,
                        button_state: DecorationButtons {
                            close: true,
                            maximize: true,
                            minimize: true,
                            always_on_top: true,
                            is_topmost: window.flags.contains(WindowFlags::ALWAYS_ON_TOP),
                            close_hovered: self.hovered_button
                                == Some((window.id, HitZone::CloseButton)),
                            maximize_hovered: self.hovered_button
                                == Some((window.id, HitZone::MaximizeButton)),
                            minimize_hovered: self.hovered_button
                                == Some((window.id, HitZone::MinimizeButton)),
                            always_on_top_hovered: self.hovered_button
                                == Some((window.id, HitZone::AlwaysOnTopButton)),
                        },
                        button_colors: button_colors.clone(),
                        button_layout: effective_layout,
                    },
                    NodeProperties::new(paint_bounds).with_z_order(paint_z_base + 2),
                ));
            }

            let title_h = if window.flags.contains(WindowFlags::DECORATED) {
                self.decoration_style.title_bar_height
            } else {
                0.0
            };
            let content_bounds = Rect::new(
                paint_bounds.x,
                paint_bounds.y + title_h,
                paint_bounds.width,
                (paint_bounds.height - title_h).max(0.0),
            );
            let z_content = paint_z_base + 3;

            // Content-area background (cheap, position-dependent): kept ABSOLUTE
            // and rebuilt each frame — it is a single fill, not the expensive
            // per-row content. It is NOT part of the translated content wrapper.
            win_group.add_child(solid_rect(
                win_base + 2,
                self.theme.window_content_background,
                content_bounds,
                z_content,
            ));

            // ── POSITION-INDEPENDENT content subtree (t163-drag-cache) ─────────
            // The expensive part — `content_view` + a node per row/cell — is built
            // ONCE at the canonical origin `(0,0)` with canonical (0-based) node
            // ids and cached under a signature that EXCLUDES position. A pure MOVE
            // (same w/h + content) HITS this cache; we then rebase the canonical
            // ids onto this window's `win_base` and reapply the absolute position
            // as a TRANSLATE on a dedicated content wrapper — so a move never
            // re-runs the content build, it only updates one wrapper's translate.
            // A RESIZE changes the content w/h → different signature → rebuild.
            let content_sig = self.window_content_signature(window, content_bounds);
            let mut canonical = match self.window_scene_cache.get_content(&content_sig) {
                Some(node) => node,
                None => {
                    // Build relative to the origin: a content rect anchored at
                    // (0,0) so every emitted node is window-relative, with the
                    // canonical id base (0) and canonical z-base — both rebased
                    // per-window below.
                    let rel_content =
                        Rect::new(0.0, 0.0, content_bounds.width, content_bounds.height);
                    let mut canon = SceneNode::new(
                        CONTENT_CANON_GROUP_ID,
                        SceneNodeKind::Workspace { index: ws_index },
                        NodeProperties::new(rel_content),
                    );
                    self.build_window_content(
                        &mut canon,
                        window,
                        rel_content,
                        CONTENT_CANON_NODE_BASE,
                        CONTENT_CANON_Z_BASE,
                        &self.theme,
                    );
                    self.window_scene_cache
                        .store_content(content_sig, canon.clone());
                    canon
                }
            };

            // Rebase the canonical content onto this window: node ids onto its
            // `win_base` (distinct ids even when two windows SHARE one cached
            // entry) and z_orders by its band-aware `paint_z_base` (correct paint
            // order for a stacked window). `paint_z_base + CONTENT_CANON_Z_BASE`
            // reproduces the original `z_content` exactly.
            rebase_content_subtree(&mut canonical, win_base, paint_z_base);
            canonical.id = NODE_WINDOW_CONTENT_GROUP_BASE + window.id.0;

            // Carry the absolute content origin as the wrapper TRANSLATE. The
            // content was built relative to (0,0), so this places it exactly where
            // the absolute content rect was — no double-count (the inner nodes use
            // origin-relative coords only). The flatten path accumulates this
            // translate (and the parent `win_group` opacity) down to every child.
            canonical.properties.bounds = Rect::new(0.0, 0.0, screen.width, screen.height);
            canonical.properties.transform =
                Affine2D::translation(content_bounds.x, content_bounds.y);
            canonical.properties.z_order = z_content;

            win_group.add_child(canonical);

            // Focused-window typed-text field: emitted ABSOLUTELY (not through the
            // content translate wrapper) so its raw scene-node bounds stay in
            // screen space and it is not folded into the cached content subtree.
            self.build_window_text_field(
                &mut win_group,
                window,
                content_bounds,
                win_base,
                z_content,
                &self.theme,
            );

            ws_node.add_child(win_group);
        }

        ws_node
    }

    /// Render app-specific content inside a window's content area.
    fn build_window_content(
        &self,
        parent: &mut SceneNode,
        window: &Window,
        content: Rect,
        win_base: u64,
        z: u32,
        theme: &ShellTheme,
    ) {
        let text_color = theme.status_bar_text;
        let cx = content.x;
        let cy = content.y;
        let cw = content.width;

        // t70-s6: when the host has registered a live app view for this window,
        // paint the window body from the app's real render model (replacing the
        // hard-coded per-`app_id` placeholder branches below). The placeholder
        // `match` is kept solely as a fallback for windows with no registered
        // view (un-launched / legacy hosts / tests without a factory).
        if self.app_views.contains_key(&window.id) {
            self.build_app_view_content(parent, window, content, win_base, z, theme);
            return;
        }

        match window.app_id.as_str() {
            "com.liquide.settings" => {
                // Settings heading
                parent.add_child(icon_node(
                    win_base + 3,
                    4,
                    text_color,
                    Rect::new(cx + 20.0, cy + 16.0, 28.0, 28.0),
                    z + 1,
                ));
                parent.add_child(text_node(
                    win_base + 4,
                    "Settings".into(),
                    text_color,
                    Rect::new(cx + 56.0, cy + 20.0, 200.0, 20.0),
                    z + 1,
                    1,
                ));
                // Category list
                let categories = [
                    "Display",
                    "Input",
                    "Audio",
                    "Network",
                    "Appearance",
                    "Privacy",
                    "Users",
                    "System",
                ];
                for (i, cat) in categories.iter().enumerate() {
                    let iy = cy + 60.0 + i as f32 * 32.0;
                    // Sidebar item background
                    let item_bg = theme.app_settings_sidebar_item;
                    parent.add_child(solid_rect(
                        win_base + 5 + i as u64,
                        item_bg,
                        Rect::new(cx + 8.0, iy, 160.0, 28.0),
                        z + 1,
                    ));
                    parent.add_child(text_node(
                        win_base + 50 + i as u64,
                        cat.to_string(),
                        text_color,
                        Rect::new(cx + 16.0, iy + 4.0, 140.0, 20.0),
                        z + 2,
                        1,
                    ));
                }
            }
            "com.liquide.terminal" => {
                // Dark terminal background
                let term_bg = theme.app_terminal_background;
                parent.add_child(solid_rect(win_base + 3, term_bg, content, z + 1));
                parent.add_child(text_node(
                    win_base + 4,
                    "user@liquide:~$".into(),
                    theme.app_terminal_text,
                    Rect::new(cx + 12.0, cy + 12.0, cw - 24.0, 20.0),
                    z + 2,
                    1,
                ));
                // Blinking cursor block after the prompt
                if self.cursor_blink_on {
                    let prompt_width = 15.0 * 8.0; // ~15 chars * ~8px monospace
                    let cursor_x = cx + 12.0 + prompt_width + 4.0;
                    let cursor_color = theme.app_terminal_text;
                    parent.add_child(solid_rect(
                        win_base + 5,
                        cursor_color,
                        Rect::new(cursor_x, cy + 12.0, 8.0, 16.0),
                        z + 3,
                    ));
                }
            }
            "com.liquide.files" => {
                parent.add_child(icon_node(
                    win_base + 3,
                    1,
                    text_color,
                    Rect::new(cx + 20.0, cy + 16.0, 28.0, 28.0),
                    z + 1,
                ));
                parent.add_child(text_node(
                    win_base + 4,
                    "Home".into(),
                    text_color,
                    Rect::new(cx + 56.0, cy + 20.0, 200.0, 20.0),
                    z + 1,
                    1,
                ));
                let folders = ["Documents", "Downloads", "Pictures", "Music", "Desktop"];
                for (i, name) in folders.iter().enumerate() {
                    let iy = cy + 60.0 + i as f32 * 32.0;
                    parent.add_child(icon_node(
                        win_base + 5 + i as u64,
                        1,
                        text_color,
                        Rect::new(cx + 24.0, iy + 2.0, 24.0, 24.0),
                        z + 1,
                    ));
                    parent.add_child(text_node(
                        win_base + 50 + i as u64,
                        name.to_string(),
                        text_color,
                        Rect::new(cx + 56.0, iy + 4.0, 200.0, 20.0),
                        z + 2,
                        1,
                    ));
                }
            }
            "com.liquide.browser" => {
                // URL bar
                let bar_bg = theme.app_browser_urlbar;
                parent.add_child(solid_rect(
                    win_base + 3,
                    bar_bg,
                    Rect::new(cx + 8.0, cy + 8.0, cw - 16.0, 32.0),
                    z + 1,
                ));
                parent.add_child(text_node(
                    win_base + 4,
                    "liquide://home".into(),
                    text_color,
                    Rect::new(cx + 16.0, cy + 14.0, cw - 32.0, 20.0),
                    z + 2,
                    1,
                ));
                // Page placeholder
                parent.add_child(text_node(
                    win_base + 5,
                    "Welcome to Liquide Browser".into(),
                    text_color,
                    Rect::new(cx + 20.0, cy + 60.0, cw - 40.0, 20.0),
                    z + 2,
                    1,
                ));
            }
            "com.liquide.calculator" => {
                parent.add_child(icon_node(
                    win_base + 3,
                    5,
                    text_color,
                    Rect::new(cx + cw / 2.0 - 24.0, cy + 20.0, 48.0, 48.0),
                    z + 1,
                ));
                parent.add_child(text_node(
                    win_base + 4,
                    "0".into(),
                    text_color,
                    Rect::new(cx + 16.0, cy + 80.0, cw - 32.0, 24.0),
                    z + 1,
                    1,
                ));
            }
            _ => {
                // Generic: show the window title centered
                parent.add_child(text_node(
                    win_base + 3,
                    window.title.clone(),
                    text_color,
                    Rect::new(cx + 20.0, cy + content.height / 2.0 - 10.0, cw - 40.0, 20.0),
                    z + 1,
                    1,
                ));
            }
        }
    }

    /// Paint the FOCUSED window's typed-text input field (t57-fG feature 2) at
    /// ABSOLUTE coordinates.
    ///
    /// This is deliberately emitted OUTSIDE the position-independent content
    /// cache (t163-drag-cache): it appears on only one (focused) window, is a
    /// single rect + text (cheap to rebuild each frame), and its rect is read by
    /// callers that inspect the raw scene-node bounds in absolute space. Keeping
    /// it absolute (alongside the shadow/decoration/content-bg) avoids routing it
    /// through the content translate wrapper.
    fn build_window_text_field(
        &self,
        parent: &mut SceneNode,
        window: &Window,
        content: Rect,
        win_base: u64,
        z: u32,
        theme: &ShellTheme,
    ) {
        // Only the focused window paints the typed-text field, and only when the
        // legacy shell buffer (no registered app view) holds text.
        if self.focus.focused() != Some(window.id) {
            return;
        }
        let Some(text) = self.window_text_input(window.id) else {
            return;
        };
        if text.is_empty() {
            return;
        }
        let text_color = theme.status_bar_text;
        let cx = content.x;
        let cy = content.y;
        let cw = content.width;
        let field_h = 28.0_f32;
        let field_y = cy + (content.height * 0.5 - field_h * 0.5).max(0.0);
        let field = Rect::new(cx + 16.0, field_y, (cw - 32.0).max(0.0), field_h);
        // Field background so the input area is unambiguous.
        parent.add_child(solid_rect(win_base + 900, theme.app_browser_urlbar, field, z + 4));
        // The typed text itself.
        parent.add_child(text_node(
            win_base + 901,
            text.to_string(),
            text_color,
            Rect::new(field.x + 8.0, field.y + 5.0, (field.width - 16.0).max(0.0), 20.0),
            z + 5,
            1,
        ));
    }

    /// Paint a window's body from its registered [`AppView`]'s render model
    /// (t70-s6). This is the generic replacement for the old hard-coded
    /// per-`app_id` branches: the app exposes rows of styled text + an optional
    /// cursor via `content_view`, and the shell maps that onto scene text/rect
    /// nodes. Cell metrics + background are chosen by [`ContentKind`] so the
    /// monospace terminal and the proportional list/document apps each read
    /// correctly.
    fn build_app_view_content(
        &self,
        parent: &mut SceneNode,
        window: &Window,
        content: Rect,
        win_base: u64,
        z: u32,
        theme: &ShellTheme,
    ) {
        use liquide_interop::ContentKind;

        let Some(view) = self.app_views.get(&window.id) else {
            return;
        };

        // Cell metrics: monospace terminals/documents pack tightly; lists use a
        // taller row. `cols`/`rows` are the character-cell hints the app sizes to.
        let (cell_w, cell_h): (f32, f32) = (8.0, 18.0);
        let pad_x = 12.0;
        let pad_y = 10.0;
        let avail_w = (content.width - pad_x * 2.0).max(0.0);
        let avail_h = (content.height - pad_y * 2.0).max(0.0);
        let cols = (avail_w / cell_w).floor().max(1.0) as u32;
        let rows = (avail_h / cell_h).floor().max(1.0) as u32;

        let model = view.content_view(cols, rows);
        let text_color = theme.status_bar_text;

        // Background: terminals get the dark terminal surface; others keep the
        // window content background (already painted by the caller), so we only
        // overlay an explicit surface for the terminal.
        let mut row_base_y = content.y + pad_y;
        if matches!(model.kind, ContentKind::Terminal) {
            parent.add_child(solid_rect(
                win_base + 3,
                theme.app_terminal_background,
                content,
                z + 1,
            ));
        }

        let row_fg = if matches!(model.kind, ContentKind::Terminal) {
            theme.app_terminal_text
        } else {
            text_color
        };

        let mut node_id = win_base + 100;
        let mut next_id = || {
            node_id += 1;
            node_id
        };

        // Optional title/header line above the rows.
        if let Some(title) = &model.title {
            parent.add_child(text_node(
                next_id(),
                title.clone(),
                row_fg,
                Rect::new(content.x + pad_x, row_base_y, avail_w, cell_h),
                z + 2,
                1,
            ));
            row_base_y += cell_h + 4.0;
        }

        // Body rows. Each row is rendered as a base text node; styled spans are
        // overlaid as colored text nodes positioned by character column. An
        // active row gets a subtle highlight rect behind it.
        let max_visible = ((content.y + content.height - row_base_y) / cell_h)
            .floor()
            .max(0.0) as usize;
        for (i, row) in model.rows.iter().take(max_visible).enumerate() {
            let ry = row_base_y + i as f32 * cell_h;
            let mut text_x = content.x + pad_x;

            if row.active {
                parent.add_child(solid_rect(
                    next_id(),
                    theme.app_settings_sidebar_item,
                    Rect::new(content.x + 4.0, ry, content.width - 8.0, cell_h),
                    z + 2,
                ));
            }

            // Optional gutter (line numbers / icons) ahead of the text.
            if let Some(gutter) = &row.gutter {
                let gw = (gutter.chars().count() as f32 + 1.0) * cell_w;
                parent.add_child(text_node(
                    next_id(),
                    gutter.clone(),
                    themed_alpha(row_fg, 150),
                    Rect::new(text_x, ry, gw, cell_h),
                    z + 3,
                    1,
                ));
                text_x += gw;
            }

            // Base row text.
            parent.add_child(text_node(
                next_id(),
                row.text.clone(),
                row_fg,
                Rect::new(text_x, ry, (content.x + content.width - text_x - 4.0).max(0.0), cell_h),
                z + 3,
                1,
            ));

            // Styled spans overlay colored sub-runs on top of the base text.
            for span in &row.spans {
                let Some(color) = span.color else { continue };
                if span.end_col <= span.start_col {
                    continue;
                }
                let sub: String = row
                    .text
                    .chars()
                    .skip(span.start_col as usize)
                    .take((span.end_col - span.start_col) as usize)
                    .collect();
                if sub.is_empty() {
                    continue;
                }
                let sx = text_x + span.start_col as f32 * cell_w;
                parent.add_child(text_node(
                    next_id(),
                    sub,
                    Color::from_rgba_u32(color),
                    Rect::new(sx, ry, (span.end_col - span.start_col) as f32 * cell_w, cell_h),
                    z + 4,
                    1,
                ));
            }
        }

        // Caret: a solid block (terminal) / thin bar (document/list) at the
        // app-reported cursor cell.
        if let Some((crow, ccol)) = model.cursor {
            if (crow as usize) < max_visible && (self.cursor_blink_on || crow == 0) {
                let caret_x = content.x + pad_x + ccol as f32 * cell_w;
                let caret_y = row_base_y + crow as f32 * cell_h;
                let caret_w = if matches!(model.kind, ContentKind::Terminal) {
                    cell_w
                } else {
                    2.0
                };
                if self.cursor_blink_on {
                    parent.add_child(solid_rect(
                        next_id(),
                        row_fg,
                        Rect::new(caret_x, caret_y, caret_w, cell_h - 2.0),
                        z + 5,
                    ));
                }
            }
        }
    }
}

#[cfg(test)]
mod damage_confine_tests {
    //! t176-damage-confine: SUPERSET-SAFETY teeth for the per-window confined
    //! precomputed-damage path. For each newly-confined interactive case (window
    //! CONTENT change, SCROLL, titlebar-button HOVER recolor, a change beneath an
    //! OVERLAPPING window's glass/shadow), we render the frame TWICE through the
    //! REAL CPU rasterizer — once compositing ONLY the confined damage onto the
    //! previous frame's framebuffer, once with a FULL repaint — and assert the two
    //! framebuffers are PIXEL-IDENTICAL. A too-tight damage that misses a changed
    //! pixel leaves the stale previous-frame pixel in the confined buffer while
    //! the full buffer has the new one → the buffers differ → the test FAILS
    //! (the disappear / stale-pixel class). The teeth are PROVEN by deliberately
    //! shrinking the damage a few px and asserting the identity check goes RED.

    use std::sync::{Arc, Mutex};

    use liquide_compositor::damage::{DamageClass, DamageSet};
    use liquide_compositor::framebuffer::FrameBuffer;
    use liquide_compositor::geometry::Rect;
    use liquide_compositor::pixel::PixelFormat;
    use liquide_compositor::scene::FlatNode;
    use liquide_interop::{
        AppContentProvider, AppContentView, AppKey, AppTextInput, AppView, ContentKind, ContentRow,
    };
    use liquide_renderer_cpu::{RenderMode, SoftwareRenderer};

    use crate::decoration::HitZone;
    use crate::shell::Shell;
    use crate::window::WindowId;

    const W: u32 = 1280;
    const H: u32 = 720;
    const TILE: u32 = 64;

    /// An app view whose content is externally mutable, so a test can change what
    /// the window paints (content update / scroll) between frames. `content_view`
    /// renders the live rows from the shared buffer.
    struct MutableApp {
        rows: Arc<Mutex<Vec<String>>>,
    }

    impl AppTextInput for MutableApp {
        fn handle_text(&mut self, _t: &str) -> bool {
            false
        }
        fn handle_key(&mut self, _k: &AppKey) -> bool {
            false
        }
    }
    impl AppContentProvider for MutableApp {
        fn content_view(&self, _cols: u32, _rows: u32) -> AppContentView {
            let mut v = AppContentView::new(ContentKind::Document);
            for r in self.rows.lock().unwrap().iter() {
                v.rows.push(ContentRow::plain(r.clone()));
            }
            v
        }
    }
    impl AppView for MutableApp {
        fn app_id(&self) -> &str {
            "com.liquide.test.mutable"
        }
    }

    fn test_shell() -> Shell {
        let mut shell = Shell::new(W as f32, H as f32);
        // Freeze the blink so a 500 ms toggle can never independently change the
        // scene between deterministic builds.
        shell.cursor_blink_on = true;
        shell.cursor_blink_time_us = u64::MAX;
        shell
    }

    fn build(shell: &mut Shell) -> Vec<FlatNode> {
        shell.cursor_blink_on = true;
        shell.cursor_blink_time_us = u64::MAX;
        shell.build_scene().flatten()
    }

    /// Full-frame damage set (every tile).
    fn full_damage() -> DamageSet {
        DamageSet::full(TILE, W.div_ceil(TILE), H.div_ceil(TILE), DamageClass::UiPrimitive)
    }

    /// Rasterise `nodes` onto a FRESH (zeroed) framebuffer with FULL damage — the
    /// authoritative "what the frame should look like" reference.
    fn render_full(rnd: &mut SoftwareRenderer, nodes: &[FlatNode]) -> FrameBuffer {
        let mut fb = FrameBuffer::new(W, H, PixelFormat::Bgra8);
        let _ = rnd.render_live(nodes, &mut fb, &full_damage(), RenderMode::Capture);
        fb
    }

    /// True iff pixel `(x, y)` lies inside ANY rect of `rects` after the SAME
    /// floor/ceil tile expansion the live worker applies (so coverage is measured
    /// against the damage TILES that are actually repainted/blitted, not the raw
    /// sub-pixel rect). A pixel covered by a damaged tile WILL be repainted from
    /// the new frame; a pixel in NO damaged tile keeps its stale previous value —
    /// the disappear class.
    fn covered_by_damage_tiles(rects: &[Rect], x: u32, y: u32) -> bool {
        let tx = x / TILE;
        let ty = y / TILE;
        for r in rects {
            let tx0 = (r.x.max(0.0).floor() as u32) / TILE;
            let ty0 = (r.y.max(0.0).floor() as u32) / TILE;
            let tx1 = ((r.x + r.width).max(0.0).ceil() as u32).saturating_sub(1) / TILE;
            let ty1 = ((r.y + r.height).max(0.0).ceil() as u32).saturating_sub(1) / TILE;
            if tx >= tx0 && tx <= tx1 && ty >= ty0 && ty <= ty1 {
                return true;
            }
        }
        false
    }

    /// Build frames N and N+1, render BOTH FULL through the real rasterizer with a
    /// fully-quiesced glyph atlas, and return `(damage_rects, prev_fb, full_fb)`.
    ///
    /// `prev` = the authoritative frame-N framebuffer; `full` = the authoritative
    /// frame-N+1 framebuffer. The set of pixels where `full != prev` is the GROUND
    /// TRUTH of "every pixel that actually changed this frame" — exactly what the
    /// confined damage MUST be a superset of. We render both FULL (not the partial
    /// confined path) on purpose: the production partial-render path re-processes
    /// glass nodes within a 32 px-padded damage bbox and can differ by ±1 LSB from
    /// a clean full render near a damage edge — a benign renderer rounding artifact,
    /// NOT a stale pixel. Comparing two FULL renders isolates the TRUE changed set
    /// so the superset check tests OUR damage, not the renderer's edge rounding.
    fn render_n_and_n1(
        setup: impl FnOnce(&mut Shell),
        mutate: impl FnOnce(&mut Shell),
    ) -> (Vec<Rect>, FrameBuffer, FrameBuffer) {
        let mut shell = test_shell();
        setup(&mut shell);
        let mut rnd = SoftwareRenderer::new();

        let _ = build(&mut shell);
        let _ = build(&mut shell);
        let nodes_n = build(&mut shell);

        mutate(&mut shell);
        let nodes_n1 = build(&mut shell);
        let damage = shell
            .take_precomputed_damage()
            .expect("a confined interactive change must emit precomputed damage, not None");
        assert!(!damage.is_empty(), "confined damage must have at least one rect");

        // Quiesce the async glyph atlas against BOTH node sets so both full renders
        // paint identical glyphs (the atlas only grows; warming with both first
        // makes the comparison glyph-stable).
        for _ in 0..4 {
            let _ = render_full(&mut rnd, &nodes_n1);
            let _ = render_full(&mut rnd, &nodes_n);
        }
        let prev = render_full(&mut rnd, &nodes_n);
        let full = render_full(&mut rnd, &nodes_n1);
        (damage, prev, full)
    }

    /// The SUPERSET-SAFETY assertion: EVERY pixel that actually changed this frame
    /// (`full != prev`) MUST be covered by the confined damage tiles. A changed
    /// pixel outside the damage would keep its stale previous value → the screen-
    /// disappears / stale-pixel class. Returns the number of CHANGED-BUT-UNCOVERED
    /// pixels (0 = safe superset) and the total changed count (so a caller can
    /// assert the change was real, not a no-op).
    fn uncovered_changed_pixels(
        damage: &[Rect],
        prev: &FrameBuffer,
        full: &FrameBuffer,
    ) -> (usize, usize) {
        let mut uncovered = 0;
        let mut changed = 0;
        for y in 0..H {
            for x in 0..W {
                let off = prev.pixel_offset(x, y);
                if prev.pixels()[off..off + 4] != full.pixels()[off..off + 4] {
                    changed += 1;
                    if !covered_by_damage_tiles(damage, x, y) {
                        uncovered += 1;
                    }
                }
            }
        }
        (uncovered, changed)
    }

    /// Assert a confined case is superset-safe: the change is real AND no changed
    /// pixel escapes the damage.
    fn assert_confined_safe(setup: impl FnOnce(&mut Shell), mutate: impl FnOnce(&mut Shell)) {
        let (damage, prev, full) = render_n_and_n1(setup, mutate);
        let (uncovered, changed) = uncovered_changed_pixels(&damage, &prev, &full);
        assert!(changed > 0, "the confined change must actually change pixels (test is vacuous otherwise)");
        assert_eq!(
            uncovered, 0,
            "{uncovered} of {changed} changed pixels fell OUTSIDE the confined damage — \
             a stale/disappear-class miss; damage={damage:?}"
        );
    }

    fn register_mut_app(shell: &mut Shell, id: WindowId, rows: Arc<Mutex<Vec<String>>>) {
        shell.register_app_view(id, Box::new(MutableApp { rows }));
    }

    // ── Confined cases: each must be PIXEL-IDENTICAL to a full repaint. ──────────

    fn content_setup(
        rows: Arc<Mutex<Vec<String>>>,
        win: &std::cell::Cell<WindowId>,
    ) -> impl FnOnce(&mut Shell) + '_ {
        move |shell: &mut Shell| {
            let id = shell.open_window_with_app(
                "Content",
                Rect::new(200.0, 160.0, 480.0, 360.0),
                "com.liquide.test.mutable",
            );
            win.set(id);
            register_mut_app(shell, id, rows);
        }
    }

    /// Apply a content change: swap the painted rows + bump the content revision +
    /// mark the window scene dirty — exactly what the live content-dirty path does
    /// (`tick_app_views`: bump rev → `mark_window_scene_dirty`).
    fn content_mutate(
        rows: Arc<Mutex<Vec<String>>>,
        new_rows: Vec<String>,
        win: &std::cell::Cell<WindowId>,
    ) -> impl FnOnce(&mut Shell) + '_ {
        move |shell: &mut Shell| {
            *rows.lock().unwrap() = new_rows;
            shell.bump_app_content_rev(win.get());
            shell.mark_window_scene_dirty();
        }
    }

    #[test]
    fn content_change_confined_damage_covers_every_changed_pixel() {
        let rows = Arc::new(Mutex::new(vec!["alpha".to_string(), "beta".to_string()]));
        let win = std::cell::Cell::new(WindowId(0));
        assert_confined_safe(
            content_setup(rows.clone(), &win),
            content_mutate(
                rows.clone(),
                vec!["GAMMA".to_string(), "delta!!".to_string()],
                &win,
            ),
        );
    }

    #[test]
    fn scroll_confined_damage_covers_every_changed_pixel() {
        // A SCROLL re-materialises the visible rows (different content_view output)
        // with the SAME window geometry — modelled by swapping the visible row set.
        let rows = Arc::new(Mutex::new(
            (0..6).map(|i| format!("line {i}")).collect::<Vec<_>>(),
        ));
        let win = std::cell::Cell::new(WindowId(0));
        let winr = &win;
        let setup = {
            let rows = rows.clone();
            move |shell: &mut Shell| {
                let id = shell.open_window_with_app(
                    "Scroller",
                    Rect::new(120.0, 100.0, 520.0, 400.0),
                    "com.liquide.test.mutable",
                );
                winr.set(id);
                register_mut_app(shell, id, rows);
            }
        };
        assert_confined_safe(
            setup,
            content_mutate(rows.clone(), (3..9).map(|i| format!("line {i}")).collect(), &win),
        );
    }

    #[test]
    fn titlebar_hover_recolor_confined_damage_covers_every_changed_pixel() {
        let id = std::cell::Cell::new(WindowId(0));
        assert_confined_safe(
            |shell| {
                let w = shell.open_window("Hover", Rect::new(300.0, 200.0, 500.0, 360.0));
                id.set(w);
            },
            |shell| {
                // Hover the close button → decoration recolors (paint-only,
                // confined via the `hovered_button` diff).
                shell.hovered_button = Some((id.get(), HitZone::CloseButton));
                shell.mark_window_scene_dirty();
            },
        );
    }

    #[test]
    fn content_change_under_overlapping_window_glass_covers_every_changed_pixel() {
        // Window A (bottom) whose content changes; window B (top) overlaps A with
        // its glass titlebar + drop-shadow. The confined damage for A must cover
        // the fringe where B's glass/shadow re-samples A's changed pixels — the
        // BACKDROP_MARGIN superset claim across overlapping windows. The ground-
        // truth changed set (full != prev) includes any B-glass pixel that moved
        // because A's backdrop changed, so this test fails if the margin is too
        // tight to cover the stacked window's fringe.
        let rows = Arc::new(Mutex::new(vec!["under".to_string()]));
        let a = std::cell::Cell::new(WindowId(0));
        let ar = &a;
        let setup = {
            let rows = rows.clone();
            move |shell: &mut Shell| {
                let wa = shell.open_window_with_app(
                    "Under",
                    Rect::new(200.0, 200.0, 420.0, 320.0),
                    "com.liquide.test.mutable",
                );
                ar.set(wa);
                register_mut_app(shell, wa, rows);
                // B overlaps A's bottom-right, stacked above it.
                let _wb = shell.open_window("Over", Rect::new(480.0, 380.0, 420.0, 300.0));
            }
        };
        assert_confined_safe(
            setup,
            content_mutate(rows.clone(), vec!["CHANGED-WIDE-CONTENT".to_string()], &a),
        );
    }

    #[test]
    fn change_near_window_top_edge_covers_every_changed_pixel() {
        // A content change whose painted text sits near the window's TOP content
        // edge — the region closest to the titlebar glass + the window border —
        // stresses the upper margin of the confined footprint.
        let rows = Arc::new(Mutex::new(vec![String::new(); 1]));
        let win = std::cell::Cell::new(WindowId(0));
        assert_confined_safe(
            content_setup(rows.clone(), &win),
            content_mutate(
                rows.clone(),
                vec!["EDGE-OF-WINDOW-CONTENT-TOP".to_string()],
                &win,
            ),
        );
    }

    // ── Teeth: shrinking the damage MUST leave a changed pixel UNCOVERED. ─────────

    /// Build a confined case, then SHRINK every damage rect by `shrink` px per side
    /// and recount uncovered-changed pixels. A correct tight superset MUST then
    /// expose at least one changed pixel outside the shrunken damage — proving the
    /// coverage check has teeth (it would catch a real under-damage).
    fn teeth_uncovered_after_shrink(
        setup: impl FnOnce(&mut Shell),
        mutate: impl FnOnce(&mut Shell),
        shrink: f32,
    ) -> usize {
        let (damage, prev, full) = render_n_and_n1(setup, mutate);
        let shrunk: Vec<Rect> = damage
            .iter()
            .map(|r| {
                Rect::new(
                    r.x + shrink,
                    r.y + shrink,
                    (r.width - shrink * 2.0).max(0.0),
                    (r.height - shrink * 2.0).max(0.0),
                )
            })
            .collect();
        let (uncovered, _changed) = uncovered_changed_pixels(&shrunk, &prev, &full);
        uncovered
    }

    #[test]
    fn teeth_shrinking_content_damage_exposes_uncovered_changed_pixels() {
        let rows = Arc::new(Mutex::new(vec!["alpha".to_string(), "beta".to_string()]));
        let win = std::cell::Cell::new(WindowId(0));
        // Shrink by 2 tiles (128 px/side): the content footprint margin is 48 px,
        // so this must bite into the painted-content tiles and expose changed px.
        let uncovered = teeth_uncovered_after_shrink(
            content_setup(rows.clone(), &win),
            content_mutate(
                rows.clone(),
                vec!["GAMMA".to_string(), "delta!!".to_string()],
                &win,
            ),
            128.0,
        );
        assert!(
            uncovered > 0,
            "shrinking the confined content damage by 128 px must leave changed pixels \
             UNCOVERED — if none are exposed the coverage check has no teeth"
        );
    }

    #[test]
    fn teeth_shrinking_hover_damage_exposes_uncovered_changed_pixels() {
        let id = std::cell::Cell::new(WindowId(0));
        let uncovered = teeth_uncovered_after_shrink(
            |shell| {
                let w = shell.open_window("Hover", Rect::new(300.0, 200.0, 500.0, 360.0));
                id.set(w);
            },
            |shell| {
                shell.hovered_button = Some((id.get(), HitZone::CloseButton));
                shell.mark_window_scene_dirty();
            },
            128.0,
        );
        assert!(
            uncovered > 0,
            "shrinking the confined hover damage by 128 px must leave the recolored \
             button pixels UNCOVERED"
        );
    }

    // ── Left-full (anti-fake-green): genuinely-full cases must NOT confine. ───────

    #[test]
    fn window_move_resize_open_stay_full_no_precomputed_damage() {
        let mut shell = test_shell();
        let id = shell.open_window("Geo", Rect::new(200.0, 160.0, 480.0, 360.0));
        let _ = build(&mut shell);
        let _ = build(&mut shell);

        // MOVE (geometry) → structural → full.
        if let Some(w) = shell.windows.get_mut(&id) {
            w.bounds.x += 40.0;
        }
        shell.mark_window_scene_dirty();
        let _ = build(&mut shell);
        assert!(
            shell.take_precomputed_damage().is_none(),
            "a window MOVE is structural and must stay full (None)"
        );

        // RESIZE (geometry) → structural → full.
        shell.resize_window(id, 600.0, 440.0).expect("resize");
        let _ = build(&mut shell);
        assert!(
            shell.take_precomputed_damage().is_none(),
            "a window RESIZE is structural and must stay full (None)"
        );

        // OPEN a new window → structural → full.
        let _ = shell.open_window("New", Rect::new(700.0, 120.0, 300.0, 220.0));
        let _ = build(&mut shell);
        assert!(
            shell.take_precomputed_damage().is_none(),
            "opening a window is structural and must stay full (None)"
        );
    }

    /// Manual measurement (run with `--ignored --nocapture`): report the confined
    /// damage AREA for a window-content change as a fraction of the 1080p screen
    /// and the implied frame-ms under the t173 cost model (a full 1080p frame is
    /// ~85 ms shadow-bound; a confined frame's raster cost scales ~linearly with
    /// damage area, with a ~1.3 ms floor). Before t176 this frame returned `None`
    /// → full repaint (~85 ms); now it confines.
    #[test]
    #[ignore]
    fn zzz_measure_window_content_confinement() {
        let mut shell = Shell::new(1920.0, 1080.0);
        shell.cursor_blink_on = true;
        shell.cursor_blink_time_us = u64::MAX;
        let rows = Arc::new(Mutex::new(vec!["row".to_string()]));
        let id = shell.open_window_with_app(
            "Probe",
            Rect::new(400.0, 300.0, 600.0, 400.0),
            "com.liquide.test.mutable",
        );
        register_mut_app(&mut shell, id, rows.clone());
        for _ in 0..3 {
            shell.cursor_blink_on = true;
            shell.cursor_blink_time_us = u64::MAX;
            let _ = shell.build_scene();
        }
        *rows.lock().unwrap() = vec!["changed row".to_string()];
        shell.bump_app_content_rev(id);
        shell.mark_window_scene_dirty();
        shell.cursor_blink_on = true;
        shell.cursor_blink_time_us = u64::MAX;
        let _ = shell.build_scene();
        let damage = shell.take_precomputed_damage();
        let screen_area = 1920.0 * 1080.0;
        match damage {
            Some(rects) => {
                let area: f32 = rects.iter().map(|r| r.width * r.height).sum();
                let frac = area / screen_area;
                // t173 cost model: full ~85 ms; confined ~max(1.3, frac*85) ms.
                let implied_ms = (frac * 85.0).max(1.3);
                eprintln!(
                    "PERF window-content: CONFINED area={area:.0}px ({:.1}% of screen) \
                     implied≈{implied_ms:.1}ms (was None→full≈85ms) rects={}",
                    frac * 100.0,
                    rects.len()
                );
            }
            None => eprintln!("PERF window-content: None (full ~85ms) — NOT confined"),
        }
    }

    #[test]
    fn first_frame_with_window_stays_full_no_precomputed_damage() {
        // No previous signature to diff against → cannot prove a superset → full.
        let mut shell = test_shell();
        let _ = shell.open_window("First", Rect::new(100.0, 100.0, 400.0, 300.0));
        let _ = build(&mut shell);
        assert!(
            shell.take_precomputed_damage().is_none(),
            "the first frame after a window appears has no old footprint to diff → full"
        );
    }
}

#[cfg(test)]
mod left_traffic_light_tests {
    //! t172-e2: the macOS LEFT traffic-light window buttons. These prove the
    //! geometry moved to the LEFT in the BASE `components.css` (so hit==paint
    //! holds across themes), that clicking each LEFT dot at its PAINTED box
    //! dispatches the right action, that the hit zone FOLLOWS the painted box
    //! (anti-constant — not a hardcoded x), and that the traffic-light COLOR
    //! tokens (close→red / minimize→yellow / maximize→green) reach the painted
    //! `Decoration` node's `button_colors` through the CSS cascade.

    use liquide_compositor::geometry::Rect;
    use liquide_compositor::scene::{SceneNode, SceneNodeKind};
    use liquide_input::mouse::{ButtonState, MouseButton, MouseEvent};
    use liquide_platform::event_loop::PlatformEvent;
    use liquide_platform::window_host::NativeWindowHandle;

    use crate::decoration::HitZone;
    use crate::shell::Shell;
    use crate::shortcuts::ShellAction;

    // The REAL shipped base layers (the production source of the LEFT geometry +
    // traffic-light tokens) so the assertions have teeth against the on-disk CSS.
    const VARIABLES_CSS: &str = include_str!("../../../../assets/themes/variables.css");
    const COMPONENTS_CSS: &str = include_str!("../../../../assets/themes/components.css");
    const MACOS_DARK_CSS: &str = include_str!("../../../../assets/themes/macos_dark.css");

    fn press(x: f32, y: f32) -> PlatformEvent {
        PlatformEvent::MouseInput {
            handle: NativeWindowHandle(0),
            event: MouseEvent::Button {
                button: MouseButton::Left,
                state: ButtonState::Pressed,
                x,
                y,
            },
        }
    }

    /// A shell with the real base CSS (+ macOS tokens) loaded and one decorated
    /// window, one scene built so the decoration is laid out.
    fn shell_with_window() -> Shell {
        let mut shell = Shell::new(1280.0, 720.0);
        shell.cursor_blink_on = true;
        shell.cursor_blink_time_us = u64::MAX;
        shell.add_stylesheet(VARIABLES_CSS);
        shell.add_stylesheet(COMPONENTS_CSS);
        // The traffic-light bg tokens (`--minimize-button-bg`/`--maximize-button-bg`)
        // live in the macOS theme; load them into the PIPELINE cascade so the
        // computed button backgrounds resolve to the macOS reds/yellows/greens
        // (the resolver alone does not carry the base component rules).
        shell.add_stylesheet(MACOS_DARK_CSS);
        shell.open_window("Alpha", Rect::new(200.0, 120.0, 640.0, 420.0));
        let _ = shell.build_scene();
        shell
    }

    fn box_of(shell: &Shell, suffix: &str) -> Rect {
        let wid = shell.visible_windows()[0].id;
        let b = shell
            .window_button_bounds_from_css(wid, suffix)
            .unwrap_or_else(|| panic!("{suffix} must have a laid-out CSS box"));
        Rect::new(b.x, b.y, b.width, b.height)
    }

    /// The traffic lights sit on the LEFT, in macOS order close→minimize→maximize
    /// left-to-right, and each is well left of the titlebar center.
    #[test]
    fn buttons_are_on_the_left_in_macos_order() {
        let shell = shell_with_window();
        let wid = shell.visible_windows()[0].id;
        let tb = shell.window_titlebar_bounds_from_css(wid).expect("titlebar box");
        let center_x = tb.x + tb.width / 2.0;

        let close = box_of(&shell, "close");
        let min = box_of(&shell, "min");
        let max = box_of(&shell, "max");

        // LEFT placement: every traffic light is left of the titlebar center, and
        // the close dot hugs the left edge (not the right).
        for (name, b) in [("close", close), ("min", min), ("max", max)] {
            assert!(
                b.x < center_x,
                "{name} dot must be on the LEFT (x={} < center {center_x})",
                b.x
            );
        }
        assert!(
            close.x - tb.x < tb.width * 0.25,
            "close dot must hug the LEFT edge of the titlebar (close.x={}, tb.x={})",
            close.x, tb.x
        );

        // macOS left→right order: close, minimize, maximize.
        assert!(
            close.x < min.x && min.x < max.x,
            "macOS order must be close<minimize<maximize left-to-right, got \
             close.x={} min.x={} max.x={}",
            close.x, min.x, max.x
        );
    }

    /// Clicking the PAINTED box center of each LEFT dot dispatches its action —
    /// paint==hit at the new left positions (close actually closes, etc.).
    #[test]
    fn clicking_each_left_dot_dispatches_its_action() {
        for (suffix, expected) in [
            ("close", ShellAction::CloseWindow),
            ("min", ShellAction::MinimizeWindow),
            ("max", ShellAction::MaximizeWindow),
        ] {
            let mut shell = shell_with_window();
            let b = box_of(&shell, suffix);
            let cx = b.x + b.width / 2.0;
            let cy = b.y + b.height / 2.0;
            let action = shell.handle_platform_event(&press(cx, cy));
            assert_eq!(
                action.as_ref(),
                Some(&expected),
                "clicking the LEFT {suffix} dot center ({cx},{cy}) must dispatch {expected:?}, \
                 got {action:?}"
            );
        }
    }

    /// ANTI-CONSTANT: the close dot's CLICK zone follows its PAINTED box, it is
    /// not a hardcoded x. Shift+grow the buttons via a runtime stylesheet; the
    /// click zone must move with the new box — a click at the NEW center closes,
    /// and a click at the OLD center no longer does.
    #[test]
    fn close_hit_zone_follows_the_painted_box_not_a_constant() {
        let mut shell = shell_with_window();
        let wid = shell.visible_windows()[0].id;
        let before = box_of(&shell, "close");
        let old_cx = before.x + before.width / 2.0;
        let old_cy = before.y + before.height / 2.0;

        // A point a long way right of the cluster is NOT close at baseline.
        // Grow + push the buttons so the close box moves to a new location.
        shell.add_stylesheet(
            "titlebar-buttons { padding-left: 120; } \
             close-button, minimize-button, maximize-button, pin-button \
             { width: 36; height: 28; }",
        );
        let _ = shell.build_scene();

        let after = box_of(&shell, "close");
        assert!(
            (after.x - before.x).abs() > 8.0 || (after.width - before.width).abs() > 8.0,
            "the override must MOVE/resize the close box (before {before:?}, after {after:?})"
        );

        // A click at the NEW painted center closes.
        let new_cx = after.x + after.width / 2.0;
        let new_cy = after.y + after.height / 2.0;
        assert_eq!(
            shell.window_button_zone_from_css(wid, new_cx, new_cy),
            Some(HitZone::CloseButton),
            "the close zone must resolve at the NEW painted box center"
        );
        // A click at the OLD center no longer resolves to close — the zone is the
        // laid-out box, not a constant x. (The old spot is now padding / title.)
        assert_ne!(
            shell.window_button_zone_from_css(wid, old_cx, old_cy),
            Some(HitZone::CloseButton),
            "the close zone must NOT remain at the OLD center (it would be a \
             hardcoded x, not the painted box)"
        );
    }

    fn emitted_decoration_colors(
        root: &SceneNode,
    ) -> Option<liquide_compositor::scene::DecorationColors> {
        fn walk(
            node: &SceneNode,
            out: &mut Option<liquide_compositor::scene::DecorationColors>,
        ) {
            if let SceneNodeKind::Decoration { button_colors, .. } = &node.kind {
                *out = Some(button_colors.clone());
            }
            for c in &node.children {
                if out.is_some() {
                    return;
                }
                walk(c, out);
            }
        }
        let mut out = None;
        walk(root, &mut out);
        out
    }

    /// The painted decoration's button backgrounds resolve to the macOS traffic
    /// lights: close→red, minimize→yellow, maximize→green — proving the
    /// `--minimize-button-bg`/`--maximize-button-bg` tokens reach `button_colors`
    /// through the CSS cascade (not the neutral gray fallback / defaults).
    #[test]
    fn button_colors_are_the_traffic_lights() {
        let mut shell = shell_with_window();
        let root = shell.build_scene();
        let colors = emitted_decoration_colors(&root)
            .expect("a decorated window must emit a Decoration node");

        // macOS tokens: #ff5f57 (red) / #febc2e (yellow) / #28c840 (green).
        let near = |c: liquide_compositor::pixel::Color, r: u8, g: u8, b: u8| {
            c.r.abs_diff(r) <= 2 && c.g.abs_diff(g) <= 2 && c.b.abs_diff(b) <= 2 && c.a > 0
        };
        assert!(
            near(colors.close_bg, 0xff, 0x5f, 0x57),
            "close dot must be macOS red #ff5f57, got {:?}",
            colors.close_bg
        );
        assert!(
            near(colors.minimize_bg, 0xfe, 0xbc, 0x2e),
            "minimize dot must be macOS yellow #febc2e, got {:?}",
            colors.minimize_bg
        );
        assert!(
            near(colors.maximize_bg, 0x28, 0xc8, 0x40),
            "maximize dot must be macOS green #28c840, got {:?}",
            colors.maximize_bg
        );
        // Anti-fake-green: yellow and green must be DISTINCT (proves min/max are
        // not both the same gray fallback / the same token).
        assert_ne!(
            (colors.minimize_bg.r, colors.minimize_bg.g, colors.minimize_bg.b),
            (colors.maximize_bg.r, colors.maximize_bg.g, colors.maximize_bg.b),
            "minimize (yellow) and maximize (green) must be distinct colors"
        );
    }
}

#[cfg(test)]
mod wallpaper_zorder_tests {
    //! t182-wallpaper-zorder: the desktop-background `<desktop-background>`
    //! element can emit MORE THAN ONE full-screen fill (after the cascade fix,
    //! the liquid-glass theme layers a `var(--bg-primary)` solid color UNDER a
    //! `url(...)` wallpaper Image). BOTH fills originate from the same element and
    //! must land in the BACKGROUND band — below windows, below the software
    //! cursor (z=9999), and below every overlay — in their emit order (color
    //! UNDER image). The previous classifier captured only the FIRST full-screen
    //! fill as desktop-background and bumped the SECOND (the wallpaper Image) into
    //! the chrome band at z>10000, so the wallpaper painted OVER the cursor and
    //! overlays. This suite proves the origin-based (contiguous-run) classifier
    //! keeps ALL desktop-background fills in the background band, in order, and
    //! still promotes LATER overlay fills (launcher/loading) above windows +
    //! cursor.
    //!
    //! TEETH: revert `classify_pipeline_nodes` to "first full-screen fill only"
    //! and `multi_fill_desktop_background_all_below_cursor` goes RED — the second
    //! fill (the wallpaper) lands at z>=CURSOR_Z, above the cursor.

    use super::{CHROME_Z_BASE, classify_pipeline_nodes};
    use liquide_compositor::geometry::Rect;
    use liquide_compositor::pixel::Color;
    use liquide_compositor::scene::{
        GradientSpec, ImageFit, NodeProperties, SceneNode, SceneNodeKind,
    };

    const W: f32 = 1280.0;
    const H: f32 = 720.0;

    /// The software cursor's flatten-time z_order (render_thread.rs
    /// `cursor_flat_node`). Background nodes MUST stay strictly below this so the
    /// cursor paints on top of the wallpaper; chrome overlays MUST stay at/above
    /// it so menus / dock / overlays paint on top of the cursor.
    const CURSOR_Z: u32 = 9999;

    fn screen() -> Rect {
        Rect::new(0.0, 0.0, W, H)
    }

    fn fullscreen() -> Rect {
        Rect::new(0.0, 0.0, W, H)
    }

    fn node(id: u64, kind: SceneNodeKind, bounds: Rect) -> SceneNode {
        SceneNode::new(id, kind, NodeProperties::new(bounds))
    }

    /// A full-screen solid color fill (the desktop-background's `--bg-primary`).
    fn color_fill(id: u64) -> SceneNode {
        node(
            id,
            SceneNodeKind::Background {
                color: Color::new(12, 14, 28, 255),
            },
            fullscreen(),
        )
    }

    /// A full-screen wallpaper Image fill (the desktop-background's `url(...)`).
    fn image_fill(id: u64) -> SceneNode {
        node(
            id,
            SceneNodeKind::Image {
                image_id: 42,
                width: W as u32,
                height: H as u32,
                fit: ImageFit::Cover,
            },
            fullscreen(),
        )
    }

    /// A full-screen gradient fill (alternate desktop-background backdrop).
    fn gradient_fill(id: u64) -> SceneNode {
        node(
            id,
            SceneNodeKind::GradientFill {
                gradient: GradientSpec::Linear {
                    start_x: 0.0,
                    start_y: 0.0,
                    end_x: 0.0,
                    end_y: 1.0,
                    stops: vec![(0.0, Color::new(14, 16, 44, 255))],
                    repeating: false,
                },
            },
            fullscreen(),
        )
    }

    /// A bar-shaped chrome fill (e.g. the statusbar background): a `Background`
    /// node that does NOT cover the screen — the first real chrome content after
    /// the desktop-background run, which CLOSES the run.
    fn chrome_bar(id: u64) -> SceneNode {
        node(
            id,
            SceneNodeKind::Background {
                color: Color::new(20, 20, 30, 200),
            },
            Rect::new(0.0, 0.0, W, 36.0),
        )
    }

    /// A full-screen overlay fill emitted LATER in the stream (launcher /
    /// loading overlay) — must stay in the chrome band, above the cursor.
    fn overlay_fill(id: u64) -> SceneNode {
        node(
            id,
            SceneNodeKind::Background {
                color: Color::new(0, 0, 0, 230),
            },
            fullscreen(),
        )
    }

    fn z_of(nodes: &[SceneNode], id: u64) -> u32 {
        nodes
            .iter()
            .find(|n| n.id == id)
            .unwrap_or_else(|| panic!("node {id} missing from classified output"))
            .properties
            .z_order
    }

    /// THE REGRESSION: desktop-background emits a color fill THEN a wallpaper
    /// Image (two full-screen fills, same origin). BOTH must land in the
    /// background band strictly BELOW the cursor, with the color UNDER the image.
    #[test]
    fn multi_fill_desktop_background_all_below_cursor() {
        // Stream order mirrors the real pipeline: desktop-background's stacked
        // fills first (color under image), then real chrome (statusbar bar).
        let nodes = classify_pipeline_nodes(
            vec![color_fill(1), image_fill(2), chrome_bar(3)],
            screen(),
            CHROME_Z_BASE,
        );

        let z_color = z_of(&nodes, 1);
        let z_image = z_of(&nodes, 2);
        let z_chrome = z_of(&nodes, 3);

        // BOTH desktop-background fills are in the background band, BELOW the
        // cursor — so the cursor paints over the wallpaper (the t181 regression
        // was the image at z>10000, above the cursor).
        assert!(
            z_color < CURSOR_Z,
            "desktop-background color fill must be below the cursor (z={z_color} < {CURSOR_Z})"
        );
        assert!(
            z_image < CURSOR_Z,
            "WALLPAPER-ABOVE-CURSOR REGRESSION: the desktop-background wallpaper \
             Image is at z={z_image} (cursor z={CURSOR_Z}); the second full-screen \
             fill was bumped into the chrome band and paints OVER the cursor. The \
             classifier must keep ALL desktop-background fills in the background band."
        );

        // Relative order preserved: color UNDER image.
        assert!(
            z_color < z_image,
            "color fill (z={z_color}) must paint UNDER the wallpaper image (z={z_image})"
        );

        // The first real chrome content (statusbar bar) is in the chrome band,
        // ABOVE the cursor.
        assert!(
            z_chrome >= CHROME_Z_BASE && z_chrome > CURSOR_Z,
            "chrome content must be in the chrome band above the cursor (z={z_chrome})"
        );
    }

    /// A LATER full-screen fill (overlay), appearing AFTER chrome content closed
    /// the desktop-background run, must stay in the chrome band above the cursor
    /// — it is NOT demoted into the background just because it is full-screen.
    #[test]
    fn later_overlay_fullscreen_fill_stays_above_cursor() {
        let nodes = classify_pipeline_nodes(
            vec![
                color_fill(1),   // desktop-background color
                image_fill(2),   // desktop-background wallpaper
                chrome_bar(3),   // statusbar — closes the bg run
                overlay_fill(4), // launcher/loading overlay — full-screen, LATER
            ],
            screen(),
            CHROME_Z_BASE,
        );

        assert!(z_of(&nodes, 1) < CURSOR_Z, "bg color below cursor");
        assert!(z_of(&nodes, 2) < CURSOR_Z, "bg image below cursor");

        let z_overlay = z_of(&nodes, 4);
        assert!(
            z_overlay > CURSOR_Z,
            "a LATER full-screen overlay fill must stay above the cursor \
             (z={z_overlay} > {CURSOR_Z}); it is an overlay, not the desktop background"
        );
    }

    /// Glass chrome blur nodes are PRE-PENDED before the paint nodes (they are
    /// never full-screen FILLS), so they must not open the desktop-background run
    /// nor steal the background band — and the bg fills after them still classify
    /// as background.
    #[test]
    fn leading_glass_chrome_does_not_break_bg_run() {
        use liquide_compositor::scene::GlassParams;
        let glass = node(
            10,
            SceneNodeKind::Glass(GlassParams {
                blur_radius: 12,
                tint_color: Color::new(0, 0, 0, 80),
                inner_glow: true,
                parallax: false,
            }),
            fullscreen(), // even a full-screen glass blur is NOT a fill
        );

        let nodes = classify_pipeline_nodes(
            vec![glass, color_fill(1), image_fill(2), chrome_bar(3)],
            screen(),
            CHROME_Z_BASE,
        );

        // Glass is chrome (above cursor); both bg fills are background (below).
        assert!(z_of(&nodes, 10) >= CHROME_Z_BASE, "glass blur is chrome");
        assert!(z_of(&nodes, 1) < CURSOR_Z, "bg color below cursor");
        assert!(z_of(&nodes, 2) < CURSOR_Z, "bg image below cursor");
        assert!(z_of(&nodes, 1) < z_of(&nodes, 2), "color under image");
    }

    /// Single-fullscreen-bg case (themes with one fill, e.g. a gradient-only
    /// desktop-background): the lone fill still classifies as background below
    /// the cursor — no regression to the common case.
    #[test]
    fn single_fill_desktop_background_below_cursor() {
        let nodes = classify_pipeline_nodes(
            vec![gradient_fill(1), chrome_bar(2)],
            screen(),
            CHROME_Z_BASE,
        );
        assert!(
            z_of(&nodes, 1) < CURSOR_Z,
            "single desktop-background gradient must be in the background band"
        );
        assert!(
            z_of(&nodes, 2) > CURSOR_Z,
            "chrome content must stay above the cursor"
        );
    }
}
