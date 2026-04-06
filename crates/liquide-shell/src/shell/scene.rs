//! `build_scene()` method and scene graph assembly.

use std::sync::Arc;

use liquide_compositor::geometry::Rect;
use liquide_compositor::scene::{
    DecorationButtons, NodeProperties, SceneNode, SceneNodeKind,
};

use crate::decoration::HitZone;
use crate::scene_builder::*;
use crate::theme::ShellTheme;
use crate::window::{Window, WindowFlags};

use super::Shell;

impl Shell {
    /// Build the complete shell scene graph.
    ///
    /// **CSS pipeline approach**: the CSS pipeline renders ALL shell chrome
    /// (background, dock, status bar, notifications, launcher, menus)
    /// from the live DOM tree.  Only windows are assembled manually because
    /// they require complex interactive state (decoration buttons, hover
    /// indices, z-ordered content surfaces) that the pipeline does not model.
    pub fn build_scene(&mut self) -> SceneNode {
        use liquide_compositor::scene::GlassParams;

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
        let (pipeline_nodes, pipeline_output) = self.css_pipeline.render_to_scene_with_output(
            &self.desktop_dom.doc,
            0, // base z-order
        );

        // Collect threaded fallback nodes. These are composited only when the
        // main pipeline returns no chrome nodes, to avoid duplicate rendering.
        let mut threaded_nodes = self
            .thread_coordinator
            .as_ref()
            .map(|coordinator| coordinator.render_all())
            .unwrap_or_default();
        let pipeline_empty = pipeline_nodes.is_empty();

        // ── Update hit-test engine with latest layout + styles ──
        self.hit_test_engine = Some(liquide_hit_test::HitTestEngine::new(
            Arc::clone(&pipeline_output.layout),
            Arc::clone(&pipeline_output.styles),
        ));
        self.desktop_dom.doc.dirty.clear_all();

        let theme = &self.theme;

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
        let ws = self.workspaces.active();
        let ws_id = NODE_WORKSPACE_BASE + ws.id.0 as u64;
        let mut ws_node = SceneNode::new(
            ws_id,
            SceneNodeKind::Workspace { index: ws.id.0 },
            NodeProperties::new(screen).with_z_order(WORKSPACE_Z_ORDER),
        );

        for window in &self.visible_windows() {
            let win_base = NODE_WINDOW_BASE + window.id.0 * NODE_WINDOW_STRIDE;

            // Shadow
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

            // Decoration with liquid glass title bar
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
                        button_layout: button_layout.clone(),
                    },
                    NodeProperties::new(window.bounds).with_z_order(window.z_order.max(0) as u32 * 10 + 2),
                ));
            }

            // Content surface
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

            let content_bg = theme.window_content_background;
            ws_node.add_child(solid_rect(
                win_base + 2,
                content_bg,
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
        root.add_child(ws_node);

        root
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
    }
}
