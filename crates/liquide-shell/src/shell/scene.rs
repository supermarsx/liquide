//! `build_scene()` method and scene graph assembly.

use std::sync::Arc;

use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::Color;
use liquide_compositor::scene::{
    DecorationButtons, DecorationColors, DecorationLayout, NodeProperties, SceneNode, SceneNodeKind,
};

use crate::decoration::{DecorationStyle, HitZone};
use crate::scene_builder::*;
use crate::theme::ShellTheme;
use crate::tiling::SnapZone;
use crate::window::{Window, WindowFlags, WindowState};

use super::Shell;

/// Base id for the per-window effect/paint container (t93-e2 / t92 gap #4).
///
/// Each window's nodes are wrapped in one non-visual `Workspace`-kind container
/// (id = base + `window_id`) that carries the per-window effect opacity. The
/// container is stripped from the flattened paint output, so this id never
/// reaches a `FlatNode`; it sits in its own reserved range purely to keep the
/// scene-tree ids distinct from the window leaf-node ids.
const NODE_WINDOW_EFFECT_GROUP_BASE: u64 = 50_000_000;

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

/// Retains the manually assembled active-workspace/window subtree.
#[derive(Debug)]
pub(crate) struct WindowSceneCache {
    signature: Option<WindowSceneSignature>,
    node: Option<SceneNode>,
    hits: u64,
    misses: u64,
    dirty: bool,
}

impl WindowSceneCache {
    pub(crate) fn new() -> Self {
        Self {
            signature: None,
            node: None,
            hits: 0,
            misses: 0,
            dirty: true,
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

    pub(crate) fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub(crate) fn stats(&self) -> WindowSceneCacheStats {
        WindowSceneCacheStats {
            hits: self.hits,
            misses: self.misses,
            dirty: self.dirty,
            cached: self.node.is_some(),
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
}

impl DecorationLayoutSignature {
    fn from_layout(layout: &DecorationLayout) -> Self {
        Self {
            title_bar_height: f32_signature(layout.title_bar_height),
            button_width: f32_signature(layout.button_width),
            button_height: f32_signature(layout.button_height),
            button_right_margin: f32_signature(layout.button_right_margin),
            button_corner_radius: f32_signature(layout.button_corner_radius),
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
    ) {
        /// Margin (logical px) added around each changed chrome rect to cover the
        /// `backdrop-filter` blur halo that samples neighbouring pixels — matches
        /// the `OVERLAY_BACKDROP_MARGIN` used by `interactive_overlay_damage`.
        const BACKDROP_MARGIN: f32 = 48.0;

        // ── Unbounded-change guards: bail to `None` (full fallback). ──
        // A window-scene change (the window cache was dirty entering this build)
        // can move/resize windows or change their content arbitrarily — not
        // represented in the CSS chrome layout tree, so we cannot bound it here.
        if self.window_scene_cache.stats().dirty {
            return;
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
        // Nothing chrome-level changed → we have no bounded footprint to emit.
        if dirty_chrome_nodes.is_empty() {
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
        self.compute_precomputed_damage(
            &dirty_chrome_nodes,
            &pipeline_output,
            blink_toggled,
        );

        // Collect threaded fallback nodes. These are composited only when the
        // main pipeline returns no chrome nodes, to avoid duplicate rendering.
        let mut threaded_nodes = self
            .thread_coordinator
            .as_ref()
            .map(|coordinator| coordinator.render_all(self.frame_delta_ms))
            .unwrap_or_default();
        let pipeline_empty = pipeline_nodes.is_empty();

        // ── Update hit-test engine with latest layout + styles ──
        self.hit_test_engine = Some(liquide_hit_test::HitTestEngine::new(
            Arc::clone(&pipeline_output.layout),
            Arc::clone(&pipeline_output.styles),
        ));

        // Resolve decoration button colors and layout from CSS (for windows).
        let button_colors = self
            .style_resolver
            .as_ref()
            .map(crate::css_integration::resolve_decoration_colors)
            .unwrap_or_default();
        let button_layout = self
            .style_resolver
            .as_ref()
            .map(crate::css_integration::resolve_decoration_layout)
            .unwrap_or_default();

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
        const CHROME_Z_BASE: u32 = 10_000;

        let screen_area = screen.width * screen.height;
        let mut bg_z = 0u32;
        let mut chrome_z = CHROME_Z_BASE;
        // Only the first full-screen fill is the desktop background.
        // Subsequent full-screen fills (launcher-overlay, loading-overlay)
        // are overlays that must render ABOVE windows, not below.
        let mut found_desktop_bg = false;

        let all_nodes = if pipeline_empty && !threaded_nodes.is_empty() {
            Self::normalize_threaded_scene_nodes(&mut threaded_nodes);
            threaded_nodes
        } else {
            pipeline_nodes
        };

        for mut node in all_nodes {
            let nb = &node.properties.bounds;
            let node_area = nb.width * nb.height;
            let is_fullscreen_fill = matches!(
                node.kind,
                SceneNodeKind::Background { .. }
                    | SceneNodeKind::GradientFill { .. }
                    // t74-realimg: a `background-image: url(...)` desktop wallpaper
                    // becomes a full-screen Image node. It is the backdrop exactly
                    // like a gradient fill, so it must join the background layer
                    // (below windows), not the chrome overlay (above them). Without
                    // this, an opaque full-screen wallpaper paints OVER every
                    // window. The 0.9 screen-area guard keeps small images (icons,
                    // thumbnails) out of the background layer.
                    | SceneNodeKind::Image { .. }
            ) && node_area >= screen_area * 0.9;

            let is_bg = is_fullscreen_fill && !found_desktop_bg;
            if is_bg {
                found_desktop_bg = true;
            }

            if is_bg {
                node.properties.z_order = bg_z;
                bg_z += 1;
            } else {
                node.properties.z_order = chrome_z;
                chrome_z += 1;
            }
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

    fn build_uncached_window_workspace_node(
        &self,
        screen: Rect,
        z_order: u32,
        button_colors: &DecorationColors,
        button_layout: &DecorationLayout,
    ) -> SceneNode {
        use liquide_compositor::scene::GlassParams;

        let theme = &self.theme;
        let ws = self.workspaces.active();
        let ws_id = NODE_WORKSPACE_BASE + ws.id.0 as u64;
        let mut ws_node = SceneNode::new(
            ws_id,
            SceneNodeKind::Workspace { index: ws.id.0 },
            NodeProperties::new(screen).with_z_order(z_order),
        );

        for (paint_rank, window) in self.visible_windows().iter().enumerate() {
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
                SceneNodeKind::Workspace { index: ws.id.0 },
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
                    color: theme.window_shadow,
                    corner_radius: self.decoration_style.corner_radius,
                },
                NodeProperties::new(shadow_bounds).with_z_order(paint_z_base),
            ));

            if window.flags.contains(WindowFlags::DECORATED) {
                let is_focused = self.focus.focused() == Some(window.id);
                let title_h = self.decoration_style.title_bar_height;
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
                        tint_color: theme.window_glass_tint,
                        inner_glow: false,
                        parallax: false,
                    }),
                    NodeProperties::new(title_bar_bounds).with_z_order(paint_z_base + 1),
                ));

                let title_bg = if is_focused {
                    let mut c = theme.window_title_bar_focused;
                    c.a = (c.a / 2).max(60);
                    c
                } else {
                    let mut c = theme.window_title_bar_unfocused;
                    c.a = (c.a / 2).max(40);
                    c
                };
                win_group.add_child(SceneNode::new(
                    win_base + 1,
                    SceneNodeKind::Decoration {
                        title: Some(window.title.clone()),
                        title_color: theme.window_title_text,
                        background: title_bg,
                        border_color: if is_focused {
                            theme.window_border_focused
                        } else {
                            theme.window_border_unfocused
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
                        button_layout: *button_layout,
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

            win_group.add_child(solid_rect(
                win_base + 2,
                theme.window_content_background,
                content_bounds,
                z_content,
            ));

            self.build_window_content(
                &mut win_group,
                window,
                content_bounds,
                win_base,
                z_content,
                theme,
            );

            ws_node.add_child(win_group);
        }

        ws_node
    }

    fn normalize_threaded_scene_nodes(nodes: &mut Vec<SceneNode>) {
        let mut flattened = Vec::new();
        for mut node in nodes.drain(..) {
            if matches!(node.kind, SceneNodeKind::Root) {
                flattened.extend(node.children.drain(..));
            } else {
                flattened.push(node);
            }
        }

        let mut sequence = 0u64;
        for node in &mut flattened {
            Self::remap_thread_scene_ids(node, &mut sequence);
        }
        *nodes = flattened;
    }

    fn remap_thread_scene_ids(node: &mut SceneNode, sequence: &mut u64) {
        const THREAD_NODE_ID_BASE: u64 = 9_000_000_000_000;
        *sequence = sequence.saturating_add(1);
        node.id = THREAD_NODE_ID_BASE.saturating_add(*sequence);
        for child in &mut node.children {
            Self::remap_thread_scene_ids(child, sequence);
        }
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

        // Typed-text input field (t57-fG feature 2): when this window is focused
        // and the shell has routed keyboard text into its buffer, paint the text
        // as an input field in the body so the typed glyphs appear. This is the
        // visible end of the shell↔app text-input seam; the field sits at the
        // body's vertical midpoint so it reads as an editable text area.
        if self.focus.focused() == Some(window.id) {
            if let Some(text) = self.window_text_input(window.id) {
                if !text.is_empty() {
                    let field_h = 28.0_f32;
                    let field_y = cy + (content.height * 0.5 - field_h * 0.5).max(0.0);
                    let field = Rect::new(cx + 16.0, field_y, (cw - 32.0).max(0.0), field_h);
                    // Field background so the input area is unambiguous.
                    parent.add_child(solid_rect(
                        win_base + 900,
                        theme.app_browser_urlbar,
                        field,
                        z + 4,
                    ));
                    // The typed text itself.
                    parent.add_child(text_node(
                        win_base + 901,
                        text.to_string(),
                        text_color,
                        Rect::new(
                            field.x + 8.0,
                            field.y + 5.0,
                            (field.width - 16.0).max(0.0),
                            20.0,
                        ),
                        z + 5,
                        1,
                    ));
                }
            }
        }
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
