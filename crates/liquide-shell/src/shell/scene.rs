//! `build_scene()` method and scene graph assembly.

use std::sync::Arc;

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
    pub fn mark_window_scene_dirty(&mut self) {
        self.window_scene_cache.mark_dirty();
    }

    /// Return counters for the retained manual window subtree cache.
    #[must_use]
    pub fn window_scene_cache_stats(&self) -> WindowSceneCacheStats {
        self.window_scene_cache.stats()
    }

    /// Build the complete shell scene graph.
    ///
    /// **CSS pipeline approach**: the CSS pipeline renders ALL shell chrome
    /// (background, dock, status bar, notifications, launcher, menus)
    /// from the live DOM tree.  Only windows are assembled manually because
    /// they require complex interactive state (decoration buttons, hover
    /// indices, z-ordered content surfaces) that the pipeline does not model.
    pub fn build_scene(&mut self) -> SceneNode {
        // Toggle cursor blink every 500ms
        let now_us = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;
        if now_us.saturating_sub(self.cursor_blink_time_us) >= 500_000 {
            self.cursor_blink_on = !self.cursor_blink_on;
            self.cursor_blink_time_us = now_us;
        }

        let screen = self.screen_rect;

        // ── Synchronise DOM with current shell state ────────
        self.sync_dom();

        // ── Run the CSS pipeline (all shell chrome) ─────────
        let (pipeline_nodes, pipeline_output, _animations_active) =
            self.css_pipeline.render_to_scene_with_output(
                &mut self.desktop_dom.doc,
                0, // base z-order
                self.frame_delta_ms,
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
                SceneNodeKind::Background { .. } | SceneNodeKind::GradientFill { .. }
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

        // ── Overview overlay (task / workspace overview) ──────────
        // Emitted ABOVE both windows and chrome when the overview is toggled
        // (t57-f-overview): a dim scrim plus a tile per visible window. The
        // overview z-base sits above the chrome overlay band so it occludes the
        // dock/statusbar like a real overview.
        if self.overview_visible {
            const OVERVIEW_Z_BASE: u32 = 50_000;
            self.add_overview_overlay(&mut root, screen, OVERVIEW_Z_BASE);
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
        // When the canonical lock-screen state is engaged (driven by the Lock
        // action through `chrome_lockscreen`), paint the lock surface above
        // everything else (t57-f9): a full-screen scrim plus a centred
        // clock/prompt cluster. Previously the Lock action transitioned the
        // canonical state but nothing painted, so the desktop stayed visible.
        if self.is_session_locked() {
            const LOCK_Z_BASE: u32 = 80_000;
            self.add_lockscreen_overlay(&mut root, screen, LOCK_Z_BASE);
        }

        root
    }

    /// Emit the lock-screen surface: a full-screen dimming scrim plus a centred
    /// clock and password-prompt cluster, above all other layers (t57-f9).
    ///
    /// Uses themed filled rects for the scrim, clock, and prompt cluster so the
    /// surface unambiguously paints content the visual regression can assert on.
    /// Hit-testing the password field is a follow-up; this wires the rendering
    /// half so the Lock action is no longer a no-op visually.
    fn add_lockscreen_overlay(&self, root: &mut SceneNode, screen: Rect, base_z: u32) {
        // Full-screen lock scrim.
        root.add_child(SceneNode::new(
            NODE_ROOT + 80,
            SceneNodeKind::Background {
                color: themed_alpha(self.theme.launcher_overlay, 220),
            },
            NodeProperties::new(screen).with_z_order(base_z),
        ));

        // Centred clock band (top of the cluster).
        let cluster_w = (screen.width * 0.32).clamp(220.0, 520.0);
        let cx = (screen.width - cluster_w) / 2.0;
        let clock = Rect::new(cx, screen.height * 0.28, cluster_w, 64.0);
        root.add_child(SceneNode::new(
            NODE_ROOT + 81,
            SceneNodeKind::Background {
                color: themed_alpha(self.theme.status_bar_text, 235),
            },
            NodeProperties::new(clock).with_z_order(base_z + 1),
        ));

        // Password prompt field (below the clock).
        let prompt = Rect::new(cx, screen.height * 0.28 + 96.0, cluster_w, 44.0);
        root.add_child(SceneNode::new(
            NODE_ROOT + 82,
            SceneNodeKind::Background {
                color: themed_alpha(self.theme.launcher_search_bar, 235),
            },
            NodeProperties::new(prompt).with_z_order(base_z + 2),
        ));
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

    /// Emit the task/workspace overview overlay: a dim full-screen scrim and a
    /// grid of tiles, one per visible window, above all other layers.
    ///
    /// Kept deliberately simple (filled tiles, not live thumbnails): the goal is
    /// a real, painted overview surface the user can see and that the visual
    /// regression (`overview_paints_tiles`) can assert on. Hit-testing /
    /// click-to-activate is a follow-up; this wires the rendering half so the
    /// `TaskOverview` / `WorkspaceOverview` actions are no longer no-ops.
    fn add_overview_overlay(&self, root: &mut SceneNode, screen: Rect, base_z: u32) {
        use liquide_compositor::scene::GlassParams;

        // Dim scrim across the whole screen.
        root.add_child(SceneNode::new(
            NODE_ROOT + 50,
            SceneNodeKind::Background {
                color: themed_alpha(self.theme.launcher_overlay, 200),
            },
            NodeProperties::new(screen).with_z_order(base_z),
        ));

        let windows = self.visible_windows();
        if windows.is_empty() {
            return;
        }

        // Lay the tiles out on a grid sized to the window count.
        let count = windows.len();
        let cols = (count as f32).sqrt().ceil().max(1.0) as usize;
        let rows = count.div_ceil(cols);
        let margin = (screen.width.min(screen.height) * 0.06).max(24.0);
        let gap = margin * 0.6;
        let grid_w = screen.width - margin * 2.0;
        let grid_h = screen.height - margin * 2.0;
        let cell_w = (grid_w - gap * (cols as f32 - 1.0)) / cols as f32;
        let cell_h = (grid_h - gap * (rows as f32 - 1.0)) / rows as f32;

        for (i, window) in windows.iter().enumerate() {
            let col = i % cols;
            let row = i / cols;
            let tile = Rect::new(
                margin + col as f32 * (cell_w + gap),
                margin + row as f32 * (cell_h + gap),
                cell_w.max(1.0),
                cell_h.max(1.0),
            );
            let tile_z = base_z + 1 + i as u32 * 2;
            let tile_base = NODE_WINDOW_BASE + window.id.0 * NODE_WINDOW_STRIDE + 7;

            // Glass tile backing so the tile reads as a window proxy.
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
            // Solid fill so the tile is unambiguously painted (and visible even
            // when glass blur degrades to a no-op on the fast path).
            root.add_child(SceneNode::new(
                tile_base + 1,
                SceneNodeKind::Background {
                    color: themed_alpha(self.theme.window_content_background, 235),
                },
                NodeProperties::new(tile).with_z_order(tile_z + 1),
            ));
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

        for window in &self.visible_windows() {
            let win_base = NODE_WINDOW_BASE + window.id.0 * NODE_WINDOW_STRIDE;

            let shadow_bounds = Rect::new(
                window.bounds.x - 4.0,
                window.bounds.y - 2.0,
                window.bounds.width + 8.0,
                window.bounds.height + 6.0,
            );
            ws_node.add_child(SceneNode::new(
                win_base,
                SceneNodeKind::Shadow {
                    spread: 4.0,
                    blur_radius: 12.0,
                    color: theme.window_shadow,
                    corner_radius: self.decoration_style.corner_radius,
                },
                NodeProperties::new(shadow_bounds).with_z_order(window.z_order.max(0) as u32 * 10),
            ));

            if window.flags.contains(WindowFlags::DECORATED) {
                let is_focused = self.focus.focused() == Some(window.id);
                let title_h = self.decoration_style.title_bar_height;
                let title_bar_bounds = Rect::new(
                    window.bounds.x,
                    window.bounds.y,
                    window.bounds.width,
                    title_h,
                );

                ws_node.add_child(SceneNode::new(
                    win_base + 10,
                    SceneNodeKind::Glass(GlassParams {
                        blur_radius: 12,
                        tint_color: theme.window_glass_tint,
                        inner_glow: false,
                        parallax: false,
                    }),
                    NodeProperties::new(title_bar_bounds)
                        .with_z_order(window.z_order.max(0) as u32 * 10 + 1),
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
                ws_node.add_child(SceneNode::new(
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
                    NodeProperties::new(window.bounds)
                        .with_z_order(window.z_order.max(0) as u32 * 10 + 2),
                ));
            }

            let title_h = if window.flags.contains(WindowFlags::DECORATED) {
                self.decoration_style.title_bar_height
            } else {
                0.0
            };
            let content_bounds = Rect::new(
                window.bounds.x,
                window.bounds.y + title_h,
                window.bounds.width,
                (window.bounds.height - title_h).max(0.0),
            );
            let z_content = window.z_order.max(0) as u32 * 10 + 3;

            ws_node.add_child(solid_rect(
                win_base + 2,
                theme.window_content_background,
                content_bounds,
                z_content,
            ));

            self.build_window_content(
                &mut ws_node,
                window,
                content_bounds,
                win_base,
                z_content,
                theme,
            );
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
}
