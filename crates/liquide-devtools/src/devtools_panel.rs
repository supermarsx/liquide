//! DevTools panel — the top-level container that composes all sub-panels
//! into a docked/floating developer tools window.
//!
//! The panel is designed to be rendered as an overlay on top of the
//! compositor scene. It handles tab switching, keyboard shortcuts,
//! and coordinates the inspector, style panel, layout overlay, element
//! picker, mutation log, and DOM serializer.

use std::time::Instant;

use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::Color;
use liquide_compositor::scene::{NodeProperties, SceneNode, SceneNodeKind};
use liquide_dom::{Document, NodeId};
use liquide_hit_test::HitTestEngine;
use liquide_layout::tree::LayoutTree;
use liquide_style_engine::StyleMap;

use crate::console::DebugConsole;
use crate::context_menu::{ContextAction, ContextMenu};
use crate::dom_serializer::DomSerializer;
use crate::element_picker::ElementPicker;
use crate::inspector::ElementTreeInspector;
use crate::layout_overlay::LayoutOverlay;
use crate::mutation_log::MutationLog;
use crate::scene_graph::SceneGraphDebugger;
use crate::style_editor::StyleEditor;
use crate::style_panel::StyleInspector;

/// Which tab is currently active in the devtools panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DevToolsTab {
    /// Element tree + styles (like Chrome's "Elements" tab).
    Elements,
    /// Computed style properties.
    Styles,
    /// Layout box model visualization (like Firefox's Layout tab).
    Layout,
    /// Interactive debug console.
    Console,
    /// Scene graph debugger.
    SceneGraph,
    /// Live style editor.
    StyleEditor,
    /// Fonts used by the selected element.
    Fonts,
    /// Animations on the selected element.
    Animations,
    /// DOM mutation log.
    Mutations,
    /// DOM tree JSON export.
    DomTree,
    /// Source files browser.
    Files,
    /// Debugger / breakpoints.
    Debugger,
}

impl DevToolsTab {
    /// All available tabs in order.
    pub const ALL: &'static [DevToolsTab] = &[
        DevToolsTab::Elements,
        DevToolsTab::Styles,
        DevToolsTab::Layout,
        DevToolsTab::Console,
        DevToolsTab::SceneGraph,
        DevToolsTab::StyleEditor,
        DevToolsTab::Fonts,
        DevToolsTab::Animations,
        DevToolsTab::Mutations,
        DevToolsTab::DomTree,
        DevToolsTab::Files,
        DevToolsTab::Debugger,
    ];

    /// Human-readable label for the tab.
    pub fn label(&self) -> &'static str {
        match self {
            DevToolsTab::Elements => "Elements",
            DevToolsTab::Styles => "Styles",
            DevToolsTab::Layout => "Layout",
            DevToolsTab::Console => "Console",
            DevToolsTab::SceneGraph => "Scene",
            DevToolsTab::StyleEditor => "Editor",
            DevToolsTab::Fonts => "Fonts",
            DevToolsTab::Animations => "Anim",
            DevToolsTab::Mutations => "Mutations",
            DevToolsTab::DomTree => "DOM",
            DevToolsTab::Files => "Files",
            DevToolsTab::Debugger => "Debug",
        }
    }
}

/// Docking position relative to the main viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockPosition {
    /// Docked to the bottom of the screen.
    Bottom,
    /// Docked to the right side of the screen.
    Right,
    /// Docked to the left side of the screen.
    Left,
    /// Floating (user-draggable).
    Float,
    /// Detached into its own desktop window.
    Detached,
}

/// Configuration for the devtools panel.
#[derive(Debug, Clone)]
pub struct DevToolsConfig {
    /// Whether the devtools panel starts visible.
    pub initially_visible: bool,
    /// Docking position.
    pub dock_position: DockPosition,
    /// Panel size (height when bottom-docked, width when side-docked).
    pub panel_size: f32,
    /// Minimum panel size.
    pub min_panel_size: f32,
    /// Background color of the panel.
    pub background_color: Color,
    /// Text color.
    pub text_color: Color,
    /// Tab bar background color.
    pub tab_bar_color: Color,
    /// Active tab indicator color.
    pub active_tab_color: Color,
    /// Border color.
    pub border_color: Color,
    /// Font size for panel text.
    pub font_size: f32,
    /// Font family for panel text.
    pub font_family: String,
    /// Auto-expand depth for the element inspector.
    pub inspector_expand_depth: u32,
    /// Whether layout overlay is on by default.
    pub show_layout_overlay: bool,
}

impl Default for DevToolsConfig {
    fn default() -> Self {
        Self {
            initially_visible: false,
            dock_position: DockPosition::Bottom,
            panel_size: 320.0,
            min_panel_size: 200.0,
            background_color: Color::new(30, 30, 30, 245),
            text_color: Color::new(212, 212, 212, 255),
            tab_bar_color: Color::new(37, 37, 38, 255),
            active_tab_color: Color::new(0, 122, 204, 255),
            border_color: Color::new(60, 60, 60, 255),
            font_size: 12.0,
            font_family: "Inter".to_string(),
            inspector_expand_depth: 3,
            show_layout_overlay: true,
        }
    }
}

/// The top-level DevTools panel.
///
/// Composes all sub-modules and manages panel visibility, tab state,
/// and the coordinate system for the devtools overlay scene nodes.
pub struct DevToolsPanel {
    /// Whether the panel is visible.
    visible: bool,
    /// Active tab.
    active_tab: DevToolsTab,
    /// Configuration.
    config: DevToolsConfig,
    /// Element tree inspector.
    pub inspector: ElementTreeInspector,
    /// Style property viewer.
    pub style_inspector: StyleInspector,
    /// Layout box overlay.
    pub layout_overlay: LayoutOverlay,
    /// Element picker.
    pub element_picker: ElementPicker,
    /// DOM mutation log.
    pub mutation_log: MutationLog,
    /// DOM serializer.
    pub dom_serializer: DomSerializer,
    /// Debug console.
    pub console: DebugConsole,
    /// Scene graph debugger.
    pub scene_debugger: SceneGraphDebugger,
    /// Live style editor.
    pub style_editor: StyleEditor,
    /// Context menu.
    pub context_menu: ContextMenu,
    /// Currently selected node (shared across panels).
    selected_node: Option<NodeId>,
    /// Screen dimensions for layout calculations.
    screen_width: f32,
    screen_height: f32,
    /// Vertical scroll offset (in pixels) for the active tab content.
    scroll_offset: f32,
    /// Whether the panel is requesting detach into a separate window.
    detach_requested: bool,
    /// Tab bar scroll offset (horizontal, for when many tabs exceed width).
    tab_scroll: f32,
    /// Whether the console input is focused for keyboard capture.
    console_focused: bool,
    /// Epoch for cursor blink animation — reset on each keystroke so the
    /// caret stays solid for 500 ms after the last input.
    caret_blink_epoch: Instant,
}

impl DevToolsPanel {
    /// Create a new devtools panel with the given configuration.
    pub fn new(config: DevToolsConfig) -> Self {
        let visible = config.initially_visible;
        let show_overlay = config.show_layout_overlay;

        let mut overlay = LayoutOverlay::new();
        if show_overlay {
            overlay.set_enabled(true);
        }

        Self {
            visible,
            active_tab: DevToolsTab::Elements,
            config,
            inspector: ElementTreeInspector::new(),
            style_inspector: StyleInspector::new(),
            layout_overlay: overlay,
            element_picker: ElementPicker::new(),
            mutation_log: MutationLog::new(),
            dom_serializer: DomSerializer::new(),
            console: DebugConsole::new(),
            scene_debugger: SceneGraphDebugger::new(),
            style_editor: StyleEditor::new(),
            context_menu: ContextMenu::new(),
            selected_node: None,
            screen_width: 1920.0,
            screen_height: 1080.0,
            scroll_offset: 0.0,
            detach_requested: false,
            tab_scroll: 0.0,
            console_focused: false,
            caret_blink_epoch: Instant::now(),
        }
    }

    /// Create with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(DevToolsConfig::default())
    }

    // ─── Visibility ───────────────────────────────────────────

    /// Toggle the devtools panel open/closed.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        if !self.visible {
            self.element_picker.deactivate();
        }
    }

    /// Show the devtools panel.
    pub fn show(&mut self) {
        self.visible = true;
    }

    /// Hide the devtools panel.
    pub fn hide(&mut self) {
        self.visible = false;
        self.element_picker.deactivate();
    }

    /// Whether the panel is visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    // ─── Tab management ───────────────────────────────────────

    /// Switch to a specific tab.
    pub fn set_tab(&mut self, tab: DevToolsTab) {
        if self.active_tab != tab {
            self.scroll_offset = 0.0;
        }
        self.active_tab = tab;
    }

    /// Get the active tab.
    pub fn active_tab(&self) -> DevToolsTab {
        self.active_tab
    }

    /// Cycle to the next tab.
    pub fn next_tab(&mut self) {
        let tabs = DevToolsTab::ALL;
        let cur = tabs.iter().position(|t| *t == self.active_tab).unwrap_or(0);
        self.active_tab = tabs[(cur + 1) % tabs.len()];
    }

    /// Cycle to the previous tab.
    pub fn prev_tab(&mut self) {
        let tabs = DevToolsTab::ALL;
        let cur = tabs.iter().position(|t| *t == self.active_tab).unwrap_or(0);
        self.active_tab = tabs[(cur + tabs.len() - 1) % tabs.len()];
    }

    // ─── Element selection ────────────────────────────────────

    /// Select a DOM node by ID (updates all sub-panels).
    pub fn select_node(&mut self, node_id: NodeId, styles: &StyleMap) {
        self.selected_node = Some(node_id);
        self.inspector.select(node_id);
        self.style_inspector.inspect(node_id, styles);
        self.layout_overlay.set_target(Some(node_id));
    }

    /// Clear the current selection.
    pub fn clear_selection(&mut self) {
        self.selected_node = None;
        self.style_inspector.clear();
        self.layout_overlay.set_target(None);
    }

    /// Get the currently selected node.
    pub fn selected_node(&self) -> Option<NodeId> {
        self.selected_node
    }

    // ─── Element picker ───────────────────────────────────────

    /// Toggle the element picker mode (click-to-select).
    pub fn toggle_picker(&mut self) {
        if self.element_picker.is_active() {
            self.element_picker.deactivate();
        } else {
            self.element_picker.activate();
        }
    }

    // ─── Screen dimensions ────────────────────────────────────

    /// Update screen dimensions (called on resize).
    pub fn set_screen_size(&mut self, width: f32, height: f32) {
        self.screen_width = width;
        self.screen_height = height;
    }

    // ─── Keyboard shortcuts ───────────────────────────────────

    /// Process a key event. Returns `true` if the event was handled.
    ///
    /// Supported shortcuts:
    /// - F12: Toggle devtools panel
    /// - Ctrl+Shift+C: Toggle element picker
    /// - Ctrl+Shift+I: Toggle devtools panel
    /// - Tab (when devtools focused): Cycle tabs
    pub fn handle_key(
        &mut self,
        key: &str,
        ctrl: bool,
        shift: bool,
        _alt: bool,
    ) -> bool {
        // Escape always closes context menu first.
        if key == "Escape" && self.context_menu.is_visible() {
            self.context_menu.hide();
            return true;
        }

        // F12 always toggles devtools.
        match key {
            "F12" => {
                self.toggle();
                return true;
            }
            "I" | "i" if ctrl && shift => {
                self.toggle();
                return true;
            }
            "C" | "c" if ctrl && shift => {
                if !self.visible {
                    self.show();
                }
                self.toggle_picker();
                return true;
            }
            _ => {}
        }

        // If console is focused, route keys there (except global shortcuts above).
        if self.console_focused && self.active_tab == DevToolsTab::Console {
            // Any keystroke resets the caret blink so it stays solid while typing.
            let reset_blink = |s: &mut Self| { s.caret_blink_epoch = Instant::now(); };
            match key {
                "Escape" => {
                    self.console_focused = false;
                    return true;
                }
                "Enter" | "Return" => {
                    // Submit will need doc/layout/styles — handled by the desktop layer.
                    // For now, we mark it as consumed and the desktop
                    // will call handle_console_key with context.
                    reset_blink(self);
                    return true;
                }
                "Backspace" => { self.console.backspace(); reset_blink(self); return true; }
                "Delete" => { self.console.delete(); reset_blink(self); return true; }
                "ArrowLeft" | "Left" => { self.console.cursor_left(); reset_blink(self); return true; }
                "ArrowRight" | "Right" => { self.console.cursor_right(); reset_blink(self); return true; }
                "ArrowUp" | "Up" => { self.console.history_up(); reset_blink(self); return true; }
                "ArrowDown" | "Down" => { self.console.history_down(); reset_blink(self); return true; }
                "Home" => { self.console.cursor_home(); reset_blink(self); return true; }
                "End" => { self.console.cursor_end(); reset_blink(self); return true; }
                _ if key.len() == 1 && !ctrl => {
                    if let Some(c) = key.chars().next() {
                        self.console.insert_char(c);
                    }
                    reset_blink(self);
                    return true;
                }
                _ => {}
            }
        }

        if self.visible && !ctrl && !shift && key == "Tab" {
            self.next_tab();
            return true;
        }

        false
    }

    // ─── Mouse event forwarding ───────────────────────────────

    /// Forward mouse move to the element picker (when active).
    ///
    /// Also tracks hover over element tree nodes when the panel is visible.
    /// Returns `true` if the hover state changed.
    pub fn on_mouse_move(
        &mut self,
        x: f32,
        y: f32,
        hit_test: &HitTestEngine,
        doc: &Document,
        layout: &LayoutTree,
    ) -> bool {
        let mut changed = false;

        // Context menu hover.
        if self.context_menu.is_visible() {
            if self.context_menu.on_mouse_move(x, y) {
                return true;
            }
        }

        // If the cursor is inside the panel, handle element tree hover.
        if self.visible {
            let bounds = self.panel_bounds();
            let tab_bar_h = 28.0;
            let content_y = bounds.y + 1.0 + tab_bar_h + 1.0 + 8.0;

            if x >= bounds.x && x <= bounds.x + bounds.width
                && y >= content_y
                && y <= bounds.y + bounds.height - 22.0
                && self.active_tab == DevToolsTab::Elements
            {
                let line_h: f32 = 18.0;
                // Account for scroll offset when determining which line is hovered.
                let scroll_y = (y - content_y) + self.scroll_offset;
                let line_idx = (scroll_y / line_h).floor() as usize;
                let visible = self.inspector.visible_nodes();
                if let Some(node) = visible.get(line_idx) {
                    let node_id = node.id;
                    if self.inspector.hovered() != Some(node_id) {
                        self.inspector.set_hovered(Some(node_id));
                        changed = true;
                    }
                } else if self.inspector.hovered().is_some() {
                    self.inspector.set_hovered(None);
                    changed = true;
                }
                return changed;
            } else if self.inspector.hovered().is_some() {
                self.inspector.set_hovered(None);
                changed = true;
            }
        }

        // Element picker: forward to hit-test-based hover.
        if self.element_picker.on_mouse_move(x, y, hit_test, doc, layout) {
            changed = true;
        }
        changed
    }

    /// Forward click to the element picker (when active).
    ///
    /// Returns `true` if an element was picked.
    pub fn on_click(&mut self, styles: &StyleMap) -> bool {
        if let Some(node_id) = self.element_picker.on_click() {
            self.select_node(node_id, styles);
            self.set_tab(DevToolsTab::Elements);
            return true;
        }
        false
    }

    /// Handle a click inside the panel at screen coordinates (x, y).
    ///
    /// Dispatches to tab bar, element tree, style categories, etc.
    /// Returns `true` if the click was consumed.
    pub fn on_panel_click(&mut self, x: f32, y: f32, styles: &StyleMap) -> bool {
        if !self.visible {
            return false;
        }

        // If context menu is visible, left-click should either activate
        // a menu item or dismiss the menu.
        if self.context_menu.is_visible() {
            if let Some((action, node_id)) = self.context_menu.on_click(x, y) {
                // Dispatch the action immediately.
                self.handle_context_action(action, node_id, styles);
                return true;
            }
            // Click was outside menu items → close the menu.
            self.context_menu.hide();
            return true;
        }

        let bounds = self.panel_bounds();

        // Check bounds.
        if x < bounds.x || x > bounds.x + bounds.width
            || y < bounds.y || y > bounds.y + bounds.height
        {
            return false;
        }

        let tab_bar_h = 28.0;
        let tab_bar_top = bounds.y + 1.0;
        let tab_bar_bottom = tab_bar_top + tab_bar_h;

        // ── Tab bar click ──
        if y >= tab_bar_top && y < tab_bar_bottom {
            // Check detach button first (top-right).
            let detach_btn_w = 28.0;
            let detach_btn_x = bounds.x + bounds.width - detach_btn_w - 4.0;
            if x >= detach_btn_x && x < detach_btn_x + detach_btn_w {
                self.toggle_detach();
                return true;
            }

            let mut tab_x = bounds.x + 8.0;
            for tab in DevToolsTab::ALL {
                let tab_w = tab.label().len() as f32 * 7.5 + 16.0;
                if x >= tab_x && x < tab_x + tab_w {
                    self.set_tab(*tab);
                    return true;
                }
                tab_x += tab_w + 4.0;
            }
            return true; // consumed, even if between tabs
        }

        // ── Content area click ──
        let content_y = tab_bar_bottom + 1.0 + 8.0;
        let status_y = bounds.y + bounds.height - 22.0;
        if y >= content_y && y < status_y {
            match self.active_tab {
                DevToolsTab::Elements => {
                    let line_h: f32 = 18.0;
                    // Account for scroll offset.
                    let scroll_y = (y - content_y) + self.scroll_offset;
                    let line_idx = (scroll_y / line_h).floor() as usize;
                    let visible = self.inspector.visible_nodes();
                    if let Some(node) = visible.get(line_idx) {
                        let node_id = node.id;
                        let indent_px: f32 = 16.0;
                        let arrow_x = bounds.x + 8.0 + (node.depth as f32) * indent_px;

                        // Click on the arrow region (first ~16px) → toggle expand.
                        if x < arrow_x + 16.0 && node.child_count > 0 {
                            self.inspector.toggle_expand(node_id);
                        } else {
                            // Click on the node text → select it.
                            self.select_node(node_id, styles);
                        }
                        return true;
                    }
                }
                DevToolsTab::Styles => {
                    // Click on a category header toggles collapse.
                    let line_h: f32 = 17.0;
                    let line_idx = ((y - content_y) / line_h).floor() as usize;
                    let groups = self.style_inspector.grouped_properties();
                    let mut row = 0;
                    for (cat, props) in &groups {
                        if row == line_idx {
                            self.style_inspector.toggle_category(*cat);
                            return true;
                        }
                        row += 1;
                        row += props.len();
                    }
                }
                DevToolsTab::Console => {
                    // Click in console area focuses the input.
                    self.console_focused = true;
                }
                DevToolsTab::SceneGraph => {
                    // Click on a scene graph entry selects it.
                    let line_h: f32 = 16.0;
                    let scroll_y = (y - content_y) + self.scroll_offset;
                    let line_idx = (scroll_y / line_h).floor() as usize;
                    self.scene_debugger.select(Some(line_idx));
                }
                _ => {}
            }
            return true; // consumed
        }

        true // click inside panel, always consume
    }

    // ─── Scroll handling ──────────────────────────────────────

    /// Handle a scroll event inside the panel.
    ///
    /// `delta` is in pixels (positive = scroll down, negative = scroll up).
    /// Returns `true` if the scroll offset changed.
    pub fn on_scroll(&mut self, x: f32, y: f32, delta: f32) -> bool {
        if !self.visible {
            return false;
        }

        let bounds = self.panel_bounds();

        // Only handle scroll inside the panel content area.
        if x < bounds.x || x > bounds.x + bounds.width
            || y < bounds.y || y > bounds.y + bounds.height
        {
            return false;
        }

        let tab_bar_h = 28.0;
        let content_y = bounds.y + 1.0 + tab_bar_h + 1.0;
        let status_h = 22.0;
        let content_h = bounds.height - tab_bar_h - 2.0 - status_h;

        if y < content_y || y > content_y + content_h {
            return false;
        }

        // Compute total content height based on active tab.
        let total_content = match self.active_tab {
            DevToolsTab::Elements => {
                let line_h: f32 = 18.0;
                self.inspector.visible_nodes().len() as f32 * line_h
            }
            DevToolsTab::DomTree => {
                let line_h: f32 = 16.0;
                10_000.0 * line_h
            }
            DevToolsTab::Styles => {
                let line_h: f32 = 17.0;
                let groups = self.style_inspector.grouped_properties();
                let mut rows = 0usize;
                for (_cat, props) in &groups {
                    rows += 1 + props.len();
                }
                rows as f32 * line_h
            }
            DevToolsTab::Mutations => {
                let line_h: f32 = 17.0;
                (1 + self.mutation_log.len()) as f32 * line_h
            }
            DevToolsTab::Console => {
                let line_h: f32 = 16.0;
                // entries + 2 lines for input prompt
                (self.console.entries().len() + 2) as f32 * line_h
            }
            DevToolsTab::SceneGraph => {
                let line_h: f32 = 16.0;
                self.scene_debugger.entries().len() as f32 * line_h
            }
            DevToolsTab::StyleEditor => {
                let line_h: f32 = 17.0;
                (self.style_editor.pending_edits().len() + 10) as f32 * line_h
            }
            DevToolsTab::Layout => {
                let line_h: f32 = 18.0;
                // Estimate: up to ~40 lines in enhanced layout view.
                40.0 * line_h
            }
            DevToolsTab::Fonts | DevToolsTab::Animations
            | DevToolsTab::Files | DevToolsTab::Debugger => {
                return false;
            }
        };

        let old_offset = self.scroll_offset;
        self.scroll_offset += delta;

        // Clamp: can't scroll above the top.
        if self.scroll_offset < 0.0 {
            self.scroll_offset = 0.0;
        }
        // Clamp: don't scroll past the end of content.
        let max_scroll = (total_content - content_h).max(0.0);
        if self.scroll_offset > max_scroll {
            self.scroll_offset = max_scroll;
        }

        (self.scroll_offset - old_offset).abs() > 0.01
    }

    // ─── Click-to-inspect from viewport ───────────────────────

    /// Handle a click that landed OUTSIDE the panel to inspect the clicked
    /// element.  Uses hit-testing against the layout tree.
    ///
    /// Returns `true` if an element was selected.
    pub fn on_viewport_click(
        &mut self,
        x: f32,
        y: f32,
        hit_test: &HitTestEngine,
        styles: &StyleMap,
    ) -> bool {
        if !self.visible {
            return false;
        }

        // If the element picker is active, let it handle the click.
        if self.element_picker.is_active() {
            return false;
        }

        // Don't handle clicks inside the panel.
        let bounds = self.panel_bounds();
        if x >= bounds.x && x <= bounds.x + bounds.width
            && y >= bounds.y && y <= bounds.y + bounds.height
        {
            return false;
        }

        // Hit-test the clicked point.
        let point = liquide_layout::geometry::Point::new(x, y);
        if let Some(result) = hit_test.hit_test(point) {
            self.select_node(result.node, styles);
            self.set_tab(DevToolsTab::Elements);
            // Scroll to make the selected node visible in the tree.
            self.scroll_to_selected();
            return true;
        }

        false
    }

    /// Scroll the Elements tab to make the selected node visible.
    fn scroll_to_selected(&mut self) {
        if self.active_tab != DevToolsTab::Elements {
            return;
        }
        if let Some(sel_id) = self.selected_node {
            let visible = self.inspector.visible_nodes();
            if let Some(idx) = visible.iter().position(|n| n.id == sel_id) {
                let line_h: f32 = 18.0;
                let target_y = idx as f32 * line_h;

                let bounds = self.panel_bounds();
                let tab_bar_h = 28.0;
                let status_h = 22.0;
                let content_h = bounds.height - tab_bar_h - 2.0 - status_h - 16.0;

                // If target is above viewport, scroll up.
                if target_y < self.scroll_offset {
                    self.scroll_offset = target_y;
                }
                // If target is below viewport, scroll down.
                else if target_y + line_h > self.scroll_offset + content_h {
                    self.scroll_offset = target_y + line_h - content_h;
                }

                if self.scroll_offset < 0.0 {
                    self.scroll_offset = 0.0;
                }
            }
        }
    }

    // ─── Scene generation ─────────────────────────────────────

    /// Compute the panel bounds based on dock position.
    pub fn panel_bounds(&self) -> Rect {
        let size = self.config.panel_size;
        match self.config.dock_position {
            DockPosition::Bottom => Rect::new(
                0.0,
                self.screen_height - size,
                self.screen_width,
                size,
            ),
            DockPosition::Right => Rect::new(
                self.screen_width - size,
                0.0,
                size,
                self.screen_height,
            ),
            DockPosition::Left => Rect::new(0.0, 0.0, size, self.screen_height),
            DockPosition::Float => {
                let w = (self.screen_width * 0.6).min(800.0);
                let h = (self.screen_height * 0.5).min(500.0);
                Rect::new(
                    (self.screen_width - w) / 2.0,
                    (self.screen_height - h) / 2.0,
                    w,
                    h,
                )
            }
            DockPosition::Detached => {
                // When detached, use same layout as Float but the desktop
                // compositor will actually render in a separate window.
                let w = (self.screen_width * 0.6).min(800.0);
                let h = (self.screen_height * 0.5).min(500.0);
                Rect::new(
                    (self.screen_width - w) / 2.0,
                    (self.screen_height - h) / 2.0,
                    w,
                    h,
                )
            }
        }
    }

    /// Build the devtools panel scene nodes.
    ///
    /// Returns scene nodes to append to the root scene at high z-order.
    /// Uses scene node IDs in the 920_000+ range.
    pub fn build_scene(
        &self,
        doc: &Document,
        layout: &LayoutTree,
        styles: &StyleMap,
    ) -> Vec<SceneNode> {
        let mut nodes = Vec::new();

        // Layout overlay (always rendered if active, even when panel hidden).
        let overlay_nodes =
            self.layout_overlay
                .build_overlay(layout, styles, self.screen_width, self.screen_height);
        nodes.extend(overlay_nodes);

        // Element picker highlight.
        let picker_nodes = self.element_picker.build_highlight(
            layout,
            self.screen_width,
            self.screen_height,
        );
        nodes.extend(picker_nodes);

        // Hover highlight — when the user hovers over a node in the Elements
        // tree, draw a SelectionOverlay on the viewport at that element's
        // layout bounds so they can see what they're about to select.
        if self.visible && self.active_tab == DevToolsTab::Elements {
            if let Some(hovered_id) = self.inspector.hovered() {
                if let Some(layout_box) = layout.find_by_node(hovered_id) {
                    let lr = &layout_box.border_rect;
                    let rect = Rect::new(lr.x, lr.y, lr.width, lr.height);
                    nodes.push(SceneNode::new(
                        915_000,
                        SceneNodeKind::SelectionOverlay {
                            fill: Color::new(66, 133, 244, 35),
                            border_color: Color::new(66, 133, 244, 180),
                            border_width: 1.5,
                        },
                        NodeProperties::new(rect).with_z_order(9978),
                    ));
                }
            }
        }

        if !self.visible {
            return nodes;
        }

        let bounds = self.panel_bounds();
        let base_id: u64 = 920_000;

        // Panel background.
        nodes.push(SceneNode::new(
            base_id,
            SceneNodeKind::Background {
                color: self.config.background_color,
            },
            NodeProperties::new(bounds).with_z_order(9900),
        ));

        // Top border.
        nodes.push(SceneNode::new(
            base_id + 1,
            SceneNodeKind::Background {
                color: self.config.border_color,
            },
            NodeProperties::new(Rect::new(bounds.x, bounds.y, bounds.width, 1.0))
                .with_z_order(9901),
        ));

        // Tab bar background.
        let tab_bar_h = 28.0;
        nodes.push(SceneNode::new(
            base_id + 2,
            SceneNodeKind::Background {
                color: self.config.tab_bar_color,
            },
            NodeProperties::new(Rect::new(bounds.x, bounds.y + 1.0, bounds.width, tab_bar_h))
                .with_z_order(9902),
        ));

        // Tab labels.
        let mut tab_x = bounds.x + 8.0;
        for (i, tab) in DevToolsTab::ALL.iter().enumerate() {
            let label = tab.label();
            let tab_w = label.len() as f32 * 7.5 + 16.0;
            let is_active = *tab == self.active_tab;

            // Active tab indicator.
            if is_active {
                nodes.push(SceneNode::new(
                    base_id + 10 + i as u64,
                    SceneNodeKind::Background {
                        color: self.config.active_tab_color,
                    },
                    NodeProperties::new(Rect::new(
                        tab_x,
                        bounds.y + tab_bar_h - 2.0,
                        tab_w,
                        2.0,
                    ))
                    .with_z_order(9904),
                ));
            }

            // Tab text.
            let text_color = if is_active {
                Color::new(255, 255, 255, 255)
            } else {
                Color::new(160, 160, 160, 255)
            };

            nodes.push(SceneNode::new(
                base_id + 20 + i as u64,
                SceneNodeKind::Text {
                    text: label.to_string(),
                    color: text_color,
                    scale: 1,
                    font_family: self.config.font_family.clone(),
                    font_size: 12.0,
                    font_weight: if is_active { 600 } else { 400 },
                    font_style_italic: false,
                    letter_spacing: 0.0,
                    word_spacing: 0.0,
                    line_height: tab_bar_h,
                    text_align: 1, // center
                    text_transform: 0,
                    text_overflow: 0,
                    white_space: 1, // nowrap
                    text_indent: 0.0,
                    text_decoration: None,
                    text_shadows: vec![],
                },
                NodeProperties::new(Rect::new(
                    tab_x,
                    bounds.y + 1.0,
                    tab_w,
                    tab_bar_h,
                ))
                .with_z_order(9903),
            ));

            tab_x += tab_w + 4.0;
        }

        // Tab bar bottom border.
        let content_y = bounds.y + 1.0 + tab_bar_h;
        nodes.push(SceneNode::new(
            base_id + 30,
            SceneNodeKind::Background {
                color: self.config.border_color,
            },
            NodeProperties::new(Rect::new(bounds.x, content_y, bounds.width, 1.0))
                .with_z_order(9905),
        ));

        // Content area — render actual tab content.
        let content_area = Rect::new(
            bounds.x + 8.0,
            content_y + 8.0,
            bounds.width - 16.0,
            bounds.height - tab_bar_h - 10.0 - 22.0, // subtract status bar
        );

        let content_nodes = match self.active_tab {
            DevToolsTab::Elements => self.render_elements_content(content_area, base_id + 100),
            DevToolsTab::Styles => self.render_styles_content(content_area, base_id + 100),
            DevToolsTab::Layout => self.render_layout_content(content_area, base_id + 100, layout, styles),
            DevToolsTab::Mutations => self.render_mutations_content(content_area, base_id + 100),
            DevToolsTab::DomTree => self.render_dom_tree_content(content_area, base_id + 100, doc),
            DevToolsTab::Console => self.render_console_content(content_area, base_id + 100),
            DevToolsTab::SceneGraph => self.render_scene_graph_content(content_area, base_id + 100),
            DevToolsTab::StyleEditor => self.render_style_editor_content(content_area, base_id + 100),
            DevToolsTab::Fonts => self.render_fonts_content(content_area, base_id + 100, styles),
            DevToolsTab::Animations => self.render_animations_content(content_area, base_id + 100, styles),
            DevToolsTab::Files => self.render_files_content(content_area, base_id + 100),
            DevToolsTab::Debugger => self.render_debugger_content(content_area, base_id + 100),
        };
        nodes.extend(content_nodes);

        // Status bar at bottom.
        let status_h = 22.0;
        let status_y = bounds.y + bounds.height - status_h;

        nodes.push(SceneNode::new(
            base_id + 60,
            SceneNodeKind::Background {
                color: Color::new(0, 122, 204, 255), // VS Code blue
            },
            NodeProperties::new(Rect::new(bounds.x, status_y, bounds.width, status_h))
                .with_z_order(9910),
        ));

        let status_text = if self.element_picker.is_active() {
            "Picker active — click an element to inspect".to_string()
        } else {
            match self.selected_node {
                Some(id) => format!("Node #{} selected", id),
                None => "No element selected".to_string(),
            }
        };

        nodes.push(SceneNode::new(
            base_id + 61,
            SceneNodeKind::Text {
                text: status_text,
                color: Color::new(255, 255, 255, 255),
                scale: 1,
                font_family: self.config.font_family.clone(),
                font_size: 11.0,
                font_weight: 400,
                font_style_italic: false,
                letter_spacing: 0.0,
                word_spacing: 0.0,
                line_height: status_h,
                text_align: 0,
                text_transform: 0,
                text_overflow: 1,
                white_space: 1,
                text_indent: 0.0,
                text_decoration: None,
                text_shadows: vec![],
            },
            NodeProperties::new(Rect::new(
                bounds.x + 8.0,
                status_y,
                bounds.width - 16.0,
                status_h,
            ))
            .with_z_order(9911),
        ));

        // Detach button (top-right of tab bar).
        let detach_btn_w = 28.0;
        let detach_btn_x = bounds.x + bounds.width - detach_btn_w - 4.0;
        let detach_label = if self.config.dock_position == DockPosition::Detached {
            "\u{2B73}" // down-arrow = reattach
        } else {
            "\u{2197}" // up-right arrow = detach
        };
        nodes.push(SceneNode::new(
            base_id + 70,
            SceneNodeKind::Text {
                text: detach_label.to_string(),
                color: Color::new(180, 180, 180, 255),
                scale: 1,
                font_family: self.config.font_family.clone(),
                font_size: 13.0,
                font_weight: 400,
                font_style_italic: false,
                letter_spacing: 0.0,
                word_spacing: 0.0,
                line_height: tab_bar_h,
                text_align: 1,
                text_transform: 0,
                text_overflow: 0,
                white_space: 1,
                text_indent: 0.0,
                text_decoration: None,
                text_shadows: vec![],
            },
            NodeProperties::new(Rect::new(
                detach_btn_x,
                bounds.y + 1.0,
                detach_btn_w,
                tab_bar_h,
            ))
            .with_z_order(9903),
        ));

        // Context menu overlay (if visible).
        if self.context_menu.is_visible() {
            let ctx_nodes = self.render_context_menu(base_id + 6000);
            nodes.extend(ctx_nodes);
        }

        nodes
    }

    // ─── Per-tab content renderers ──────────────────────────────

    /// Render the Elements tab: actual DOM tree with expand/collapse,
    /// tag names, IDs, classes, attributes, and selection highlight.
    fn render_elements_content(&self, area: Rect, base_id: u64) -> Vec<SceneNode> {
        let mut nodes = Vec::new();
        let line_h: f32 = 18.0;
        let indent_px: f32 = 16.0;
        let max_visible = (area.height / line_h).floor() as usize;

        let visible = self.inspector.visible_nodes();
        if visible.is_empty() {
            nodes.push(self.text_node(
                base_id,
                "No elements — run refresh_inspector(doc) first",
                Color::new(140, 140, 140, 255),
                area,
                9906,
            ));
            return nodes;
        }

        // Apply scroll offset: determine which lines are visible.
        let first_line = (self.scroll_offset / line_h).floor() as usize;
        let fractional_offset = self.scroll_offset - (first_line as f32) * line_h;
        let render_count = (max_visible + 1).min(visible.len().saturating_sub(first_line));

        let selected = self.inspector.selected();
        let hovered = self.inspector.hovered();

        for vi in 0..render_count {
            let i = first_line + vi;
            let node = match visible.get(i) {
                Some(n) => *n,
                None => break,
            };
            let y = area.y + (vi as f32) * line_h - fractional_offset;
            let x = area.x + (node.depth as f32) * indent_px;
            let id_offset = base_id + (vi as u64) * 3;

            // Skip nodes rendered outside the visible area.
            if y + line_h < area.y || y > area.y + area.height {
                continue;
            }

            // Selection / hover highlight background.
            let is_selected = selected == Some(node.id);
            let is_hovered = hovered == Some(node.id);
            if is_selected || is_hovered {
                let bg_color = if is_selected {
                    Color::new(4, 57, 94, 200) // dark blue selection
                } else {
                    Color::new(45, 45, 45, 180) // subtle hover
                };
                nodes.push(SceneNode::new(
                    id_offset,
                    SceneNodeKind::Background { color: bg_color },
                    NodeProperties::new(Rect::new(area.x, y, area.width, line_h))
                        .with_z_order(9906),
                ));
            }

            // Build the display text for this node.
            let line_text = if node.is_text {
                // Text nodes: show content in quotes.
                let text = node.text.as_deref().unwrap_or("");
                let truncated = if text.len() > 60 { &text[..60] } else { text };
                format!("\"{}\"", truncated)
            } else {
                // Element nodes: <tag#id.class attr="val">
                let arrow = if node.child_count > 0 {
                    if node.children.is_empty() { "\u{25B6} " } else { "\u{25BC} " }
                } else {
                    "  "
                };

                let mut parts = String::new();
                parts.push_str(arrow);
                parts.push('<');
                parts.push_str(&node.tag);

                if let Some(ref eid) = node.element_id {
                    parts.push_str(&format!(" id=\"{}\"", eid));
                }
                if !node.classes.is_empty() {
                    parts.push_str(&format!(" class=\"{}\"", node.classes.join(" ")));
                }
                for (k, v) in &node.attributes {
                    parts.push_str(&format!(" {}=\"{}\"", k, v));
                }
                parts.push('>');

                if !node.pseudo_states.is_empty() {
                    parts.push_str(&format!(" :{}", node.pseudo_states.join(":")));
                }

                parts
            };

            // Choose color: text nodes are gray, elements are tag-colored.
            let text_color = if node.is_text {
                Color::new(206, 145, 120, 255) // string orange
            } else if is_selected {
                Color::new(255, 255, 255, 255) // bright white when selected
            } else {
                Color::new(86, 156, 214, 255) // VS Code blue for tags
            };

            nodes.push(SceneNode::new(
                id_offset + 1,
                SceneNodeKind::Text {
                    text: line_text,
                    color: text_color,
                    scale: 1,
                    font_family: self.config.font_family.clone(),
                    font_size: self.config.font_size,
                    font_weight: if is_selected { 600 } else { 400 },
                    font_style_italic: node.is_text,
                    letter_spacing: 0.0,
                    word_spacing: 0.0,
                    line_height: line_h,
                    text_align: 0,
                    text_transform: 0,
                    text_overflow: 1,
                    white_space: 1,
                    text_indent: 0.0,
                    text_decoration: None,
                    text_shadows: vec![],
                },
                NodeProperties::new(Rect::new(
                    x,
                    y,
                    area.width - (node.depth as f32) * indent_px,
                    line_h,
                ))
                .with_z_order(9907),
            ));
        }

        // Scrollbar indicator (thin bar on the right edge).
        let total_items = visible.len();
        if total_items > max_visible {
            let scrollbar_h = (area.height * (max_visible as f32 / total_items as f32)).max(20.0);
            let max_scroll = (total_items as f32 - max_visible as f32) * line_h;
            let scroll_ratio = if max_scroll > 0.0 { self.scroll_offset / max_scroll } else { 0.0 };
            let scrollbar_y = area.y + scroll_ratio * (area.height - scrollbar_h);

            nodes.push(SceneNode::new(
                base_id + 900,
                SceneNodeKind::Background {
                    color: Color::new(80, 80, 80, 120),
                },
                NodeProperties::new(Rect::new(
                    area.x + area.width - 4.0,
                    scrollbar_y,
                    4.0,
                    scrollbar_h,
                ))
                .with_z_order(9908),
            ));
        }

        nodes
    }

    /// Render the Styles tab: computed CSS properties grouped by category.
    fn render_styles_content(&self, area: Rect, base_id: u64) -> Vec<SceneNode> {
        let mut nodes = Vec::new();
        let line_h: f32 = 17.0;
        let max_visible = (area.height / line_h).floor() as usize;

        if self.selected_node.is_none() {
            nodes.push(self.text_node(
                base_id,
                "Select an element to view computed styles",
                Color::new(140, 140, 140, 255),
                area,
                9906,
            ));
            return nodes;
        }

        let groups = self.style_inspector.grouped_properties();
        if groups.is_empty() {
            nodes.push(self.text_node(
                base_id,
                "No computed styles available for this node",
                Color::new(140, 140, 140, 255),
                area,
                9906,
            ));
            return nodes;
        }

        // Build flat list of all rows, then apply scroll offset.
        let mut all_rows: Vec<(String, Color, u16, bool)> = Vec::new();
        for (category, props) in &groups {
            let header_text = format!("▸ {} ({})", category.label(), props.len());
            all_rows.push((header_text, Color::new(220, 220, 170, 255), 600, false));
            for prop in props {
                let prop_text = format!("  {}: {}", prop.name, prop.value);
                let color = if prop.inherited {
                    Color::new(128, 128, 128, 255)
                } else {
                    self.config.text_color
                };
                all_rows.push((prop_text, color, 400, prop.inherited));
            }
        }

        let first_line = (self.scroll_offset / line_h).floor() as usize;
        let fractional_offset = self.scroll_offset - (first_line as f32) * line_h;
        let render_count = (max_visible + 1).min(all_rows.len().saturating_sub(first_line));

        for vi in 0..render_count {
            let i = first_line + vi;
            let (text, color, weight, italic) = match all_rows.get(i) {
                Some(r) => r,
                None => break,
            };
            let y = area.y + (vi as f32) * line_h - fractional_offset;
            if y + line_h < area.y || y > area.y + area.height {
                continue;
            }

            nodes.push(SceneNode::new(
                base_id + vi as u64,
                SceneNodeKind::Text {
                    text: text.clone(),
                    color: *color,
                    scale: 1,
                    font_family: self.config.font_family.clone(),
                    font_size: self.config.font_size,
                    font_weight: *weight,
                    font_style_italic: *italic,
                    letter_spacing: 0.0,
                    word_spacing: 0.0,
                    line_height: line_h,
                    text_align: 0,
                    text_transform: 0,
                    text_overflow: 1,
                    white_space: 1,
                    text_indent: 0.0,
                    text_decoration: None,
                    text_shadows: vec![],
                },
                NodeProperties::new(Rect::new(area.x, y, area.width, line_h))
                    .with_z_order(9907),
            ));
        }

        // Scrollbar indicator.
        if all_rows.len() > max_visible {
            let scrollbar_h = (area.height * (max_visible as f32 / all_rows.len() as f32)).max(20.0);
            let max_scroll = (all_rows.len() as f32 - max_visible as f32) * line_h;
            let scroll_ratio = if max_scroll > 0.0 { self.scroll_offset / max_scroll } else { 0.0 };
            let scrollbar_y = area.y + scroll_ratio * (area.height - scrollbar_h);

            nodes.push(SceneNode::new(
                base_id + 900,
                SceneNodeKind::Background {
                    color: Color::new(80, 80, 80, 120),
                },
                NodeProperties::new(Rect::new(
                    area.x + area.width - 4.0,
                    scrollbar_y,
                    4.0,
                    scrollbar_h,
                ))
                .with_z_order(9908),
            ));
        }

        nodes
    }

    /// Render the Layout tab: box model for the selected element.
    fn render_layout_content(
        &self,
        area: Rect,
        base_id: u64,
        layout: &LayoutTree,
        styles: &StyleMap,
    ) -> Vec<SceneNode> {
        let mut nodes = Vec::new();
        let line_h: f32 = 18.0;

        let node_id = match self.selected_node {
            Some(id) => id,
            None => {
                nodes.push(self.text_node(
                    base_id,
                    "Select an element to view its box model",
                    Color::new(140, 140, 140, 255),
                    area,
                    9906,
                ));
                return nodes;
            }
        };

        if let Some(box_data) = layout.find_by_node(node_id) {
            let cr = &box_data.content_rect;
            let pr = &box_data.padding_rect;
            let br = &box_data.border_rect;
            let mr = &box_data.margin_rect;

            // Derive box-model edges from the nested rects.
            let margin_t = br.y - mr.y;
            let margin_r = (mr.x + mr.width) - (br.x + br.width);
            let margin_b = (mr.y + mr.height) - (br.y + br.height);
            let margin_l = br.x - mr.x;

            let border_t = pr.y - br.y;
            let border_r = (br.x + br.width) - (pr.x + pr.width);
            let border_b = (br.y + br.height) - (pr.y + pr.height);
            let border_l = pr.x - br.x;

            let pad_t = cr.y - pr.y;
            let pad_r = (pr.x + pr.width) - (cr.x + cr.width);
            let pad_b = (pr.y + pr.height) - (cr.y + cr.height);
            let pad_l = cr.x - pr.x;

            let mut lines: Vec<(String, Color, u16)> = Vec::new();

            // Header.
            lines.push((
                format!("Box Model \u{2014} node #{}", node_id),
                Color::new(220, 220, 170, 255),
                600,
            ));

            // Box type.
            lines.push((
                format!("  box-type: {:?}", box_data.box_type),
                Color::new(86, 156, 214, 255),
                400,
            ));

            // Effective (computed) dimensions.
            lines.push((
                "\u{25B6} Effective Dimensions:".to_string(),
                Color::new(220, 220, 170, 255),
                600,
            ));
            lines.push((
                format!("  content:    {:.1} \u{00D7} {:.1}", cr.width, cr.height),
                self.config.text_color,
                400,
            ));
            lines.push((
                format!("  padding-box:{:.1} \u{00D7} {:.1}", pr.width, pr.height),
                self.config.text_color,
                400,
            ));
            lines.push((
                format!("  border-box: {:.1} \u{00D7} {:.1}", br.width, br.height),
                self.config.text_color,
                400,
            ));
            lines.push((
                format!("  margin-box: {:.1} \u{00D7} {:.1}", mr.width, mr.height),
                self.config.text_color,
                400,
            ));

            // Box model edges (Firefox-style nested).
            lines.push((
                "\u{25B6} Box Model Edges (T R B L):".to_string(),
                Color::new(220, 220, 170, 255),
                600,
            ));
            lines.push((
                format!("  margin:  {:.0} {:.0} {:.0} {:.0}", margin_t, margin_r, margin_b, margin_l),
                Color::new(246, 203, 90, 255), // yellowish for margin
                400,
            ));
            lines.push((
                format!("  border:  {:.0} {:.0} {:.0} {:.0}", border_t, border_r, border_b, border_l),
                Color::new(192, 192, 192, 255), // gray for border
                400,
            ));
            lines.push((
                format!("  padding: {:.0} {:.0} {:.0} {:.0}", pad_t, pad_r, pad_b, pad_l),
                Color::new(139, 185, 93, 255), // green for padding
                400,
            ));

            // Absolute position on screen.
            lines.push((
                "\u{25B6} Absolute Position:".to_string(),
                Color::new(220, 220, 170, 255),
                600,
            ));
            lines.push((
                format!("  x: {:.1}  y: {:.1}", mr.x, mr.y),
                self.config.text_color,
                400,
            ));
            lines.push((
                format!("  right:  {:.1}  bottom: {:.1}",
                    mr.x + mr.width, mr.y + mr.height),
                self.config.text_color,
                400,
            ));

            // Scroll info if applicable.
            if let Some(ref scroll_size) = box_data.scroll_size {
                lines.push((
                    format!("  scroll-size: {:.1} \u{00D7} {:.1}", scroll_size.width, scroll_size.height),
                    Color::new(78, 201, 176, 255),
                    400,
                ));
            }

            // CSS positioning properties from computed styles.
            if let Some(computed) = styles.get(node_id) {
                lines.push((
                    "\u{25B6} CSS Properties Applied:".to_string(),
                    Color::new(220, 220, 170, 255),
                    600,
                ));
                lines.push((
                    format!("  display:    {:?}", computed.display),
                    self.config.text_color,
                    400,
                ));
                lines.push((
                    format!("  position:   {:?}", computed.position),
                    self.config.text_color,
                    400,
                ));
                lines.push((
                    format!("  box-sizing: {:?}", computed.box_sizing),
                    self.config.text_color,
                    400,
                ));

                // Width/height.
                lines.push((
                    format!("  width:  {:?}  height: {:?}", computed.width, computed.height),
                    self.config.text_color,
                    400,
                ));
                lines.push((
                    format!("  min-w:  {:?}  max-w:  {:?}", computed.min_width, computed.max_width),
                    Color::new(160, 160, 160, 255),
                    400,
                ));
                lines.push((
                    format!("  min-h:  {:?}  max-h:  {:?}", computed.min_height, computed.max_height),
                    Color::new(160, 160, 160, 255),
                    400,
                ));

                // Positioning offsets (top/right/bottom/left).
                lines.push((
                    format!("  top: {:?}  right: {:?}", computed.top, computed.right),
                    Color::new(86, 156, 214, 255),
                    400,
                ));
                lines.push((
                    format!("  bottom: {:?}  left: {:?}", computed.bottom, computed.left),
                    Color::new(86, 156, 214, 255),
                    400,
                ));

                // Z-index.
                if let Some(z) = computed.z_index {
                    lines.push((
                        format!("  z-index: {}", z),
                        Color::new(78, 201, 176, 255),
                        400,
                    ));
                }

                // Float/clear.
                lines.push((
                    format!("  float: {:?}  clear: {:?}", computed.float, computed.clear),
                    Color::new(160, 160, 160, 255),
                    400,
                ));

                // Overflow.
                lines.push((
                    format!("  overflow: {:?} / {:?}", computed.overflow_x, computed.overflow_y),
                    Color::new(160, 160, 160, 255),
                    400,
                ));

                // Flex properties (if applicable).
                if matches!(computed.display,
                    liquide_style_engine::computed::Display::Flex
                    | liquide_style_engine::computed::Display::InlineFlex)
                {
                    lines.push((
                        "\u{25B6} Flexbox:".to_string(),
                        Color::new(220, 220, 170, 255),
                        600,
                    ));
                    lines.push((
                        format!("  direction:   {:?}", computed.flex_direction),
                        self.config.text_color,
                        400,
                    ));
                    lines.push((
                        format!("  wrap:        {:?}", computed.flex_wrap),
                        self.config.text_color,
                        400,
                    ));
                    lines.push((
                        format!("  justify:     {:?}", computed.justify_content),
                        self.config.text_color,
                        400,
                    ));
                    lines.push((
                        format!("  align-items: {:?}", computed.align_items),
                        self.config.text_color,
                        400,
                    ));
                }

                // Grid properties (if applicable).
                if matches!(computed.display,
                    liquide_style_engine::computed::Display::Grid
                    | liquide_style_engine::computed::Display::InlineGrid)
                {
                    lines.push((
                        "\u{25B6} Grid:".to_string(),
                        Color::new(220, 220, 170, 255),
                        600,
                    ));
                    lines.push((
                        format!("  columns: {:?}", computed.grid_template_columns),
                        self.config.text_color,
                        400,
                    ));
                    lines.push((
                        format!("  rows:    {:?}", computed.grid_template_rows),
                        self.config.text_color,
                        400,
                    ));
                    lines.push((
                        format!("  auto-flow: {:?}", computed.grid_auto_flow),
                        self.config.text_color,
                        400,
                    ));
                }
            }

            for (i, (text, color, weight)) in lines.iter().enumerate() {
                nodes.push(SceneNode::new(
                    base_id + i as u64,
                    SceneNodeKind::Text {
                        text: text.clone(),
                        color: *color,
                        scale: 1,
                        font_family: self.config.font_family.clone(),
                        font_size: self.config.font_size,
                        font_weight: *weight,
                        font_style_italic: false,
                        letter_spacing: 0.0,
                        word_spacing: 0.0,
                        line_height: line_h,
                        text_align: 0,
                        text_transform: 0,
                        text_overflow: 0,
                        white_space: 1,
                        text_indent: 0.0,
                        text_decoration: None,
                        text_shadows: vec![],
                    },
                    NodeProperties::new(Rect::new(
                        area.x,
                        area.y + (i as f32) * line_h,
                        area.width,
                        line_h,
                    ))
                    .with_z_order(9907),
                ));
            }
        } else {
            nodes.push(self.text_node(
                base_id,
                &format!("No layout data for node #{}", node_id),
                Color::new(140, 140, 140, 255),
                area,
                9906,
            ));
        }

        nodes
    }

    /// Render the Mutations tab: recent DOM mutation log entries.
    fn render_mutations_content(&self, area: Rect, base_id: u64) -> Vec<SceneNode> {
        let mut nodes = Vec::new();
        let line_h: f32 = 17.0;
        let max_visible = (area.height / line_h).floor() as usize;

        if self.mutation_log.is_empty() {
            nodes.push(self.text_node(
                base_id,
                "No mutations recorded yet",
                Color::new(140, 140, 140, 255),
                area,
                9906,
            ));
            return nodes;
        }

        // Build all rows: header + records.
        let header = format!(
            "Mutation Log — {} in buffer, {} total",
            self.mutation_log.len(),
            self.mutation_log.total_count()
        );
        // Collect all records (newest first).
        let all_records: Vec<_> = self.mutation_log.recent(self.mutation_log.len()).collect();
        let total_rows = 1 + all_records.len(); // header + records

        // Apply scroll offset.
        let first_line = (self.scroll_offset / line_h).floor() as usize;
        let fractional_offset = self.scroll_offset - (first_line as f32) * line_h;
        let render_count = (max_visible + 1).min(total_rows.saturating_sub(first_line));

        for vi in 0..render_count {
            let row_idx = first_line + vi;
            let (text, color, weight) = if row_idx == 0 {
                (header.clone(), Color::new(220, 220, 170, 255), 600u16)
            } else {
                let rec_idx = row_idx - 1;
                if rec_idx < all_records.len() {
                    let record = &all_records[rec_idx];
                    (format_mutation_record(record), mutation_color(&record.kind), 400u16)
                } else {
                    continue;
                }
            };

            nodes.push(SceneNode::new(
                base_id + vi as u64,
                SceneNodeKind::Text {
                    text,
                    color,
                    scale: 1,
                    font_family: self.config.font_family.clone(),
                    font_size: self.config.font_size,
                    font_weight: weight,
                    font_style_italic: false,
                    letter_spacing: 0.0,
                    word_spacing: 0.0,
                    line_height: line_h,
                    text_align: 0,
                    text_transform: 0,
                    text_overflow: 1,
                    white_space: 1,
                    text_indent: 0.0,
                    text_decoration: None,
                    text_shadows: vec![],
                },
                NodeProperties::new(Rect::new(
                    area.x,
                    area.y + (vi as f32) * line_h - fractional_offset,
                    area.width,
                    line_h,
                ))
                .with_z_order(9907),
            ));
        }

        // Scrollbar indicator.
        if total_rows as f32 * line_h > area.height {
            let ratio = area.height / (total_rows as f32 * line_h);
            let bar_h = (area.height * ratio).max(12.0);
            let max_scroll = (total_rows as f32 * line_h - area.height).max(0.0);
            let bar_y = if max_scroll > 0.0 {
                area.y + (self.scroll_offset / max_scroll) * (area.height - bar_h)
            } else {
                area.y
            };
            nodes.push(SceneNode::new(
                base_id + 5000,
                SceneNodeKind::Background {
                    color: Color::new(255, 255, 255, 50),
                },
                NodeProperties::new(Rect::new(
                    area.x + area.width - 4.0,
                    bar_y,
                    4.0,
                    bar_h,
                ))
                .with_z_order(9910),
            ));
        }

        nodes
    }

    /// Render the DOM Tree tab: serialized DOM as JSON lines.
    fn render_dom_tree_content(
        &self,
        area: Rect,
        base_id: u64,
        doc: &Document,
    ) -> Vec<SceneNode> {
        let mut nodes = Vec::new();
        let line_h: f32 = 16.0;
        let max_visible = (area.height / line_h).floor() as usize;

        let json = self.dom_serializer.to_json(doc);
        let lines: Vec<&str> = json.lines().collect();

        if lines.is_empty() {
            nodes.push(self.text_node(
                base_id,
                "Empty document",
                Color::new(140, 140, 140, 255),
                area,
                9906,
            ));
            return nodes;
        }

        // Apply scroll offset.
        let first_line = (self.scroll_offset / line_h).floor() as usize;
        let fractional_offset = self.scroll_offset - (first_line as f32) * line_h;
        let render_count = (max_visible + 1).min(lines.len().saturating_sub(first_line));

        for vi in 0..render_count {
            let i = first_line + vi;
            let line = match lines.get(i) {
                Some(l) => *l,
                None => break,
            };
            let y = area.y + (vi as f32) * line_h - fractional_offset;

            if y + line_h < area.y || y > area.y + area.height {
                continue;
            }

            // Syntax-color the JSON: keys in blue, strings in orange,
            // numbers in green, braces in gray.
            let color = if line.trim_start().starts_with('"') && line.contains(':') {
                Color::new(156, 220, 254, 255) // key: light blue
            } else if line.trim_start().starts_with('"') {
                Color::new(206, 145, 120, 255) // string value: orange
            } else if line.trim_start().starts_with(|c: char| c.is_ascii_digit()) {
                Color::new(181, 206, 168, 255) // number: green
            } else {
                Color::new(180, 180, 180, 255) // braces/brackets: gray
            };

            nodes.push(SceneNode::new(
                base_id + vi as u64,
                SceneNodeKind::Text {
                    text: line.to_string(),
                    color,
                    scale: 1,
                    font_family: self.config.font_family.clone(),
                    font_size: 11.0,
                    font_weight: 400,
                    font_style_italic: false,
                    letter_spacing: 0.0,
                    word_spacing: 0.0,
                    line_height: line_h,
                    text_align: 0,
                    text_transform: 0,
                    text_overflow: 1,
                    white_space: 2, // pre (preserve whitespace for JSON indent)
                    text_indent: 0.0,
                    text_decoration: None,
                    text_shadows: vec![],
                },
                NodeProperties::new(Rect::new(
                    area.x,
                    y,
                    area.width,
                    line_h,
                ))
                .with_z_order(9907),
            ));
        }

        // Scrollbar indicator.
        if lines.len() > max_visible {
            let scrollbar_h = (area.height * (max_visible as f32 / lines.len() as f32)).max(20.0);
            let max_scroll = (lines.len() as f32 - max_visible as f32) * line_h;
            let scroll_ratio = if max_scroll > 0.0 { self.scroll_offset / max_scroll } else { 0.0 };
            let scrollbar_y = area.y + scroll_ratio * (area.height - scrollbar_h);

            nodes.push(SceneNode::new(
                base_id + 900,
                SceneNodeKind::Background {
                    color: Color::new(80, 80, 80, 120),
                },
                NodeProperties::new(Rect::new(
                    area.x + area.width - 4.0,
                    scrollbar_y,
                    4.0,
                    scrollbar_h,
                ))
                .with_z_order(9908),
            ));
        }

        nodes
    }

    // ─── New tab renderers ─────────────────────────────────────

    /// Render the Console tab: log entries + input prompt.
    fn render_console_content(&self, area: Rect, base_id: u64) -> Vec<SceneNode> {
        let mut nodes = Vec::new();
        let line_h: f32 = 16.0;

        let entries = self.console.entries();
        // Reserve last 2 lines for separator and input.
        let output_area_h = area.height - line_h * 2.0;
        let output_max = (output_area_h / line_h).floor() as usize;
        let total = entries.len();

        // Show the most recent entries that fit.
        let start = total.saturating_sub(output_max);
        let first_line = (self.scroll_offset / line_h).floor() as usize;
        let display_start = start.saturating_sub(first_line);

        for (vi, idx) in (display_start..total).enumerate() {
            if vi >= output_max {
                break;
            }
            let entry = &entries[idx];
            let color = match entry.kind {
                crate::console::ConsoleEntryKind::Input => Color::new(86, 156, 214, 255),
                crate::console::ConsoleEntryKind::Output => Color::new(212, 212, 212, 255),
                crate::console::ConsoleEntryKind::Warning => Color::new(220, 220, 170, 255),
                crate::console::ConsoleEntryKind::Error => Color::new(244, 135, 113, 255),
                crate::console::ConsoleEntryKind::Info => Color::new(128, 128, 128, 255),
            };
            nodes.push(SceneNode::new(
                base_id + vi as u64,
                SceneNodeKind::Text {
                    text: entry.text.clone(),
                    color,
                    scale: 1,
                    font_family: self.config.font_family.clone(),
                    font_size: 11.0,
                    font_weight: 400,
                    font_style_italic: false,
                    letter_spacing: 0.0,
                    word_spacing: 0.0,
                    line_height: line_h,
                    text_align: 0,
                    text_transform: 0,
                    text_overflow: 1,
                    white_space: 2,
                    text_indent: 0.0,
                    text_decoration: None,
                    text_shadows: vec![],
                },
                NodeProperties::new(Rect::new(area.x, area.y + vi as f32 * line_h, area.width, line_h))
                    .with_z_order(9907),
            ));
        }

        // Input separator line.
        let sep_y = area.y + area.height - line_h * 2.0;
        nodes.push(SceneNode::new(
            base_id + 800,
            SceneNodeKind::Background {
                color: Color::new(60, 60, 60, 255),
            },
            NodeProperties::new(Rect::new(area.x, sep_y, area.width, 1.0))
                .with_z_order(9908),
        ));

        // Input prompt.
        let input_y = sep_y + 2.0;
        let prompt = format!("> {}", self.console.input_buffer());
        let prompt_color = if self.console_focused {
            Color::new(255, 255, 255, 255)
        } else {
            Color::new(160, 160, 160, 255)
        };
        nodes.push(SceneNode::new(
            base_id + 801,
            SceneNodeKind::Text {
                text: prompt,
                color: prompt_color,
                scale: 1,
                font_family: self.config.font_family.clone(),
                font_size: 12.0,
                font_weight: 400,
                font_style_italic: false,
                letter_spacing: 0.0,
                word_spacing: 0.0,
                line_height: line_h,
                text_align: 0,
                text_transform: 0,
                text_overflow: 1,
                white_space: 2,
                text_indent: 0.0,
                text_decoration: None,
                text_shadows: vec![],
            },
            NodeProperties::new(Rect::new(area.x, input_y, area.width, line_h))
                .with_z_order(9909),
        ));

        // Blinking text caret — show when console is focused.
        if self.console_focused {
            let blink_ms = self.caret_blink_epoch.elapsed().as_millis();
            let caret_visible = (blink_ms / 500) % 2 == 0;
            if caret_visible {
                // Approximate caret X: "> " prefix (2 chars) + chars before cursor.
                // Use ~7.2px per char at font_size 12 as an approximation for
                // monospace / the configured font.   Real glyph metrics would be
                // better, but this is practical for the devtools console.
                let prefix_chars = 2; // "> "
                let cursor_char_offset = prefix_chars
                    + self.console.input_buffer()[..self.console.cursor_pos()]
                        .chars()
                        .count();
                let char_width: f32 = 7.2;
                let caret_x = area.x + cursor_char_offset as f32 * char_width;

                nodes.push(SceneNode::new(
                    base_id + 802,
                    SceneNodeKind::TextCaret {
                        color: Color::new(255, 255, 255, 230),
                        width: 1.5,
                    },
                    NodeProperties::new(Rect::new(caret_x, input_y + 1.0, 2.0, line_h - 2.0))
                        .with_z_order(9910),
                ));
            }
        }

        nodes
    }

    /// Render the Scene Graph tab.
    fn render_scene_graph_content(&self, area: Rect, base_id: u64) -> Vec<SceneNode> {
        let mut nodes = Vec::new();
        let line_h: f32 = 16.0;
        let max_visible = (area.height / line_h).floor() as usize;
        let entries = self.scene_debugger.entries();

        if entries.is_empty() {
            nodes.push(self.text_node(
                base_id,
                "Scene graph empty — build a scene first",
                Color::new(140, 140, 140, 255),
                area,
                9906,
            ));
            return nodes;
        }

        let first_line = (self.scroll_offset / line_h).floor() as usize;
        let fractional_offset = self.scroll_offset - (first_line as f32) * line_h;
        let render_count = (max_visible + 1).min(entries.len().saturating_sub(first_line));

        for vi in 0..render_count {
            let i = first_line + vi;
            let entry = match entries.get(i) {
                Some(e) => e,
                None => break,
            };
            let y = area.y + (vi as f32) * line_h - fractional_offset;
            if y + line_h < area.y || y > area.y + area.height {
                continue;
            }

            let indent = "  ".repeat(entry.depth as usize);
            let text = format!(
                "{}[{}] {} ({:.0}×{:.0}) z={}",
                indent, entry.id, entry.kind, entry.bounds.2, entry.bounds.3, entry.z_order
            );

            let is_selected = self.scene_debugger.selected() == Some(i);
            let color = if is_selected {
                Color::new(255, 255, 255, 255)
            } else if !entry.visible {
                Color::new(100, 100, 100, 255)
            } else {
                Color::new(86, 156, 214, 255)
            };

            if is_selected {
                nodes.push(SceneNode::new(
                    base_id + 2000 + vi as u64,
                    SceneNodeKind::Background {
                        color: Color::new(4, 57, 94, 200),
                    },
                    NodeProperties::new(Rect::new(area.x, y, area.width, line_h))
                        .with_z_order(9906),
                ));
            }

            nodes.push(SceneNode::new(
                base_id + vi as u64,
                SceneNodeKind::Text {
                    text,
                    color,
                    scale: 1,
                    font_family: self.config.font_family.clone(),
                    font_size: 11.0,
                    font_weight: if is_selected { 600 } else { 400 },
                    font_style_italic: false,
                    letter_spacing: 0.0,
                    word_spacing: 0.0,
                    line_height: line_h,
                    text_align: 0,
                    text_transform: 0,
                    text_overflow: 1,
                    white_space: 2,
                    text_indent: 0.0,
                    text_decoration: None,
                    text_shadows: vec![],
                },
                NodeProperties::new(Rect::new(area.x, y, area.width, line_h))
                    .with_z_order(9907),
            ));
        }

        // Scrollbar.
        if entries.len() > max_visible {
            let scrollbar_h = (area.height * (max_visible as f32 / entries.len() as f32)).max(20.0);
            let max_scroll = (entries.len() as f32 - max_visible as f32) * line_h;
            let scroll_ratio = if max_scroll > 0.0 { self.scroll_offset / max_scroll } else { 0.0 };
            let scrollbar_y = area.y + scroll_ratio * (area.height - scrollbar_h);
            nodes.push(SceneNode::new(
                base_id + 900,
                SceneNodeKind::Background {
                    color: Color::new(80, 80, 80, 120),
                },
                NodeProperties::new(Rect::new(area.x + area.width - 4.0, scrollbar_y, 4.0, scrollbar_h))
                    .with_z_order(9908),
            ));
        }

        nodes
    }

    /// Render the Style Editor tab.
    fn render_style_editor_content(&self, area: Rect, base_id: u64) -> Vec<SceneNode> {
        let mut nodes = Vec::new();
        let line_h: f32 = 17.0;

        if self.selected_node.is_none() {
            nodes.push(self.text_node(
                base_id,
                "Select an element to edit its styles",
                Color::new(140, 140, 140, 255),
                area,
                9906,
            ));
            return nodes;
        }

        // Header.
        let auto_label = if self.style_editor.auto_apply() { "ON" } else { "OFF" };
        let header = format!("Style Editor — auto-apply: {} — {} pending edits",
            auto_label, self.style_editor.edit_count());
        nodes.push(self.text_node(
            base_id,
            &header,
            Color::new(220, 220, 170, 255),
            Rect::new(area.x, area.y, area.width, line_h),
            9907,
        ));

        // Pending edits.
        let edits = self.style_editor.pending_edits();
        for (i, edit) in edits.iter().enumerate().take(15) {
            let y = area.y + ((i + 1) as f32) * line_h;
            let status = if edit.applied { "\u{2713}" } else { "\u{2026}" };
            let text = format!("{} {}: {}", status, edit.property, edit.new_value);
            let color = if edit.applied {
                Color::new(78, 201, 176, 255) // teal
            } else {
                Color::new(220, 220, 170, 255) // yellow
            };
            nodes.push(self.text_node(
                base_id + 1 + i as u64,
                &text,
                color,
                Rect::new(area.x, y, area.width, line_h),
                9907,
            ));
        }

        // Editing input.
        if let Some(prop) = self.style_editor.editing_property() {
            let y = area.y + ((edits.len() + 2) as f32) * line_h;
            let text = format!("Editing: {} = {}\u{258F}", prop, self.style_editor.editing_value());
            nodes.push(self.text_node(
                base_id + 500,
                &text,
                Color::new(255, 255, 255, 255),
                Rect::new(area.x, y, area.width, line_h),
                9909,
            ));
        }

        // Instructions.
        let help_y = area.y + area.height - line_h * 2.0;
        nodes.push(self.text_node(
            base_id + 600,
            "Click a property in Styles tab to edit · Esc to cancel · Enter to apply",
            Color::new(100, 100, 100, 255),
            Rect::new(area.x, help_y, area.width, line_h),
            9906,
        ));

        nodes
    }

    /// Render the Fonts tab: font information for the selected element.
    fn render_fonts_content(&self, area: Rect, base_id: u64, styles: &StyleMap) -> Vec<SceneNode> {
        let mut nodes = Vec::new();
        let line_h: f32 = 17.0;

        let node_id = match self.selected_node {
            Some(id) => id,
            None => {
                nodes.push(self.text_node(
                    base_id,
                    "Select an element to view font information",
                    Color::new(140, 140, 140, 255),
                    area,
                    9906,
                ));
                return nodes;
            }
        };

        // Extract font properties from styles.
        let mut row = 0usize;
        nodes.push(self.text_node(
            base_id + row as u64,
            &format!("Fonts — node #{}", node_id),
            Color::new(220, 220, 170, 255),
            Rect::new(area.x, area.y + row as f32 * line_h, area.width, line_h),
            9907,
        ));
        row += 1;

        if let Some(computed) = styles.get(node_id) {
            let font_family_str = computed.font_family.join(", ");
            let font_size = computed.font_size;
            let font_weight = computed.font_weight;
            let lh_str = match computed.line_height {
                liquide_style_engine::computed::LineHeight::Normal => "normal".to_string(),
                liquide_style_engine::computed::LineHeight::Number(n) => format!("{:.2}", n),
                liquide_style_engine::computed::LineHeight::Px(px) => format!("{:.1}px", px),
            };

            let props = [
                format!("font-family: {}", font_family_str),
                format!("font-size: {:.1}px", font_size),
                format!("font-weight: {}", font_weight),
                format!("line-height: {}", lh_str),
                format!("letter-spacing: {:.1}px", computed.letter_spacing),
                format!("word-spacing: {:.1}px", computed.word_spacing),
            ];

            for prop in &props {
                nodes.push(self.text_node(
                    base_id + row as u64,
                    prop,
                    self.config.text_color,
                    Rect::new(area.x, area.y + row as f32 * line_h, area.width, line_h),
                    9907,
                ));
                row += 1;
            }

            // Rendered font info.
            row += 1;
            nodes.push(self.text_node(
                base_id + row as u64,
                "Rendered Fonts:",
                Color::new(220, 220, 170, 255),
                Rect::new(area.x, area.y + row as f32 * line_h, area.width, line_h),
                9907,
            ));
            row += 1;
            nodes.push(self.text_node(
                base_id + row as u64,
                &format!("  \"{}\" — {:.0}px, weight {}", font_family_str, font_size, font_weight),
                Color::new(78, 201, 176, 255),
                Rect::new(area.x, area.y + row as f32 * line_h, area.width, line_h),
                9907,
            ));
        } else {
            nodes.push(self.text_node(
                base_id + row as u64,
                "No computed styles available",
                Color::new(140, 140, 140, 255),
                Rect::new(area.x, area.y + row as f32 * line_h, area.width, line_h),
                9906,
            ));
        }

        nodes
    }

    /// Render the Animations tab: animation/transition info for selected element.
    fn render_animations_content(&self, area: Rect, base_id: u64, styles: &StyleMap) -> Vec<SceneNode> {
        let mut nodes = Vec::new();
        let line_h: f32 = 17.0;

        let node_id = match self.selected_node {
            Some(id) => id,
            None => {
                nodes.push(self.text_node(
                    base_id,
                    "Select an element to view animations",
                    Color::new(140, 140, 140, 255),
                    area,
                    9906,
                ));
                return nodes;
            }
        };

        let mut row = 0usize;
        nodes.push(self.text_node(
            base_id + row as u64,
            &format!("Animations — node #{}", node_id),
            Color::new(220, 220, 170, 255),
            Rect::new(area.x, area.y + row as f32 * line_h, area.width, line_h),
            9907,
        ));
        row += 1;

        if let Some(computed) = styles.get(node_id) {
            // Show transition properties if set.
            let transitions = &computed.transition;
            if !transitions.is_empty() {
                nodes.push(self.text_node(
                    base_id + row as u64,
                    "Transitions:",
                    Color::new(86, 156, 214, 255),
                    Rect::new(area.x, area.y + row as f32 * line_h, area.width, line_h),
                    9907,
                ));
                row += 1;
                for t in transitions {
                    let text = format!(
                        "  {} — {:.0}ms {:?} delay {:.0}ms",
                        t.property, t.duration_ms, t.timing_function, t.delay_ms
                    );
                    nodes.push(self.text_node(
                        base_id + row as u64,
                        &text,
                        self.config.text_color,
                        Rect::new(area.x, area.y + row as f32 * line_h, area.width, line_h),
                        9907,
                    ));
                    row += 1;
                }
            } else {
                nodes.push(self.text_node(
                    base_id + row as u64,
                    "No active transitions.",
                    Color::new(140, 140, 140, 255),
                    Rect::new(area.x, area.y + row as f32 * line_h, area.width, line_h),
                    9906,
                ));
                row += 1;
            }

            row += 1;
            let animations = &computed.animation;
            if !animations.is_empty() {
                nodes.push(self.text_node(
                    base_id + row as u64,
                    "Animations:",
                    Color::new(86, 156, 214, 255),
                    Rect::new(area.x, area.y + row as f32 * line_h, area.width, line_h),
                    9907,
                ));
                row += 1;
                for a in animations {
                    let text = format!(
                        "  {} — {:.0}ms {:?} delay {:.0}ms {:?}",
                        a.name, a.duration_ms, a.timing_function, a.delay_ms, a.iteration_count
                    );
                    nodes.push(self.text_node(
                        base_id + row as u64,
                        &text,
                        self.config.text_color,
                        Rect::new(area.x, area.y + row as f32 * line_h, area.width, line_h),
                        9907,
                    ));
                    row += 1;
                }
            } else {
                nodes.push(self.text_node(
                    base_id + row as u64,
                    "No CSS @keyframes animations detected.",
                    Color::new(140, 140, 140, 255),
                    Rect::new(area.x, area.y + row as f32 * line_h, area.width, line_h),
                    9906,
                ));
            }
        } else {
            nodes.push(self.text_node(
                base_id + row as u64,
                "No computed styles available",
                Color::new(140, 140, 140, 255),
                Rect::new(area.x, area.y + row as f32 * line_h, area.width, line_h),
                9906,
            ));
        }

        nodes
    }

    /// Render the Files tab: project file browser (placeholder).
    fn render_files_content(&self, area: Rect, base_id: u64) -> Vec<SceneNode> {
        let mut nodes = Vec::new();
        let line_h: f32 = 17.0;
        let lines = [
            "Files — Project Source Browser",
            "",
            "  assets/",
            "    templates/",
            "    themes/",
            "    icons/",
            "  components/",
            "",
            "Use live reload (Ctrl+Shift+R) to watch for file changes.",
            "Template and CSS changes are applied automatically.",
        ];
        for (i, line) in lines.iter().enumerate() {
            let color = if i == 0 {
                Color::new(220, 220, 170, 255)
            } else if line.ends_with('/') {
                Color::new(86, 156, 214, 255)
            } else {
                Color::new(180, 180, 180, 255)
            };
            nodes.push(self.text_node(
                base_id + i as u64,
                line,
                color,
                Rect::new(area.x, area.y + i as f32 * line_h, area.width, line_h),
                9907,
            ));
        }
        nodes
    }

    /// Render the Debugger tab: breakpoints and debug state (placeholder).
    fn render_debugger_content(&self, area: Rect, base_id: u64) -> Vec<SceneNode> {
        let mut nodes = Vec::new();
        let line_h: f32 = 17.0;
        let lines = [
            "Debugger — Layout & Rendering Pipeline",
            "",
            "Breakpoints: (none set)",
            "",
            "Pipeline stages:",
            "  1. HTML Parse    \u{2713}",
            "  2. CSS Cascade   \u{2713}",
            "  3. Layout        \u{2713}",
            "  4. Paint         \u{2713}",
            "  5. Composite     \u{2713}",
            "  6. Rasterize     \u{2713}",
            "",
            "Use console commands to inspect pipeline state:",
            "  layout.stats  — Layout tree statistics",
            "  dom.stats     — Document statistics",
        ];
        for (i, line) in lines.iter().enumerate() {
            let color = if i == 0 {
                Color::new(220, 220, 170, 255)
            } else if line.contains('\u{2713}') {
                Color::new(78, 201, 176, 255) // teal for checkmarks
            } else {
                Color::new(180, 180, 180, 255)
            };
            nodes.push(self.text_node(
                base_id + i as u64,
                line,
                color,
                Rect::new(area.x, area.y + i as f32 * line_h, area.width, line_h),
                9907,
            ));
        }
        nodes
    }

    /// Render the context menu overlay.
    fn render_context_menu(&self, base_id: u64) -> Vec<SceneNode> {
        let mut nodes = Vec::new();
        if !self.context_menu.is_visible() {
            return nodes;
        }

        let bounds = self.context_menu.bounds();
        let item_h = 24.0;

        // Menu background.
        nodes.push(SceneNode::new(
            base_id,
            SceneNodeKind::Background {
                color: Color::new(37, 37, 38, 250),
            },
            NodeProperties::new(bounds).with_z_order(9950),
        ));

        // Menu border.
        nodes.push(SceneNode::new(
            base_id + 1,
            SceneNodeKind::Background {
                color: Color::new(69, 69, 69, 255),
            },
            NodeProperties::new(Rect::new(bounds.x, bounds.y, bounds.width, 1.0))
                .with_z_order(9951),
        ));

        // Menu items.
        for (i, item) in self.context_menu.items().iter().enumerate() {
            let y = bounds.y + 4.0 + i as f32 * item_h;

            if item.separator {
                nodes.push(SceneNode::new(
                    base_id + 10 + i as u64,
                    SceneNodeKind::Background {
                        color: Color::new(60, 60, 60, 255),
                    },
                    NodeProperties::new(Rect::new(bounds.x + 8.0, y + item_h / 2.0, bounds.width - 16.0, 1.0))
                        .with_z_order(9952),
                ));
                continue;
            }

            // Hover highlight.
            let is_hovered = self.context_menu.hovered_index() == Some(i);
            if is_hovered {
                nodes.push(SceneNode::new(
                    base_id + 100 + i as u64,
                    SceneNodeKind::Background {
                        color: Color::new(4, 57, 94, 200),
                    },
                    NodeProperties::new(Rect::new(bounds.x + 2.0, y, bounds.width - 4.0, item_h))
                        .with_z_order(9952),
                ));
            }

            let color = if !item.enabled {
                Color::new(100, 100, 100, 255)
            } else if is_hovered {
                Color::new(255, 255, 255, 255)
            } else {
                Color::new(212, 212, 212, 255)
            };

            nodes.push(SceneNode::new(
                base_id + 200 + i as u64,
                SceneNodeKind::Text {
                    text: item.label.clone(),
                    color,
                    scale: 1,
                    font_family: self.config.font_family.clone(),
                    font_size: 12.0,
                    font_weight: 400,
                    font_style_italic: false,
                    letter_spacing: 0.0,
                    word_spacing: 0.0,
                    line_height: item_h,
                    text_align: 0,
                    text_transform: 0,
                    text_overflow: 1,
                    white_space: 1,
                    text_indent: 0.0,
                    text_decoration: None,
                    text_shadows: vec![],
                },
                NodeProperties::new(Rect::new(bounds.x + 12.0, y, bounds.width - 24.0, item_h))
                    .with_z_order(9953),
            ));
        }

        nodes
    }

    // ─── New public APIs ──────────────────────────────────────

    /// Whether the panel is requesting to be detached into a separate window.
    pub fn detach_requested(&self) -> bool {
        self.detach_requested
    }

    /// Clear the detach request after the compositor handles it.
    pub fn clear_detach_request(&mut self) {
        self.detach_requested = false;
    }

    /// Toggle detached state.
    pub fn toggle_detach(&mut self) {
        if self.config.dock_position == DockPosition::Detached {
            self.config.dock_position = DockPosition::Bottom;
        } else {
            self.config.dock_position = DockPosition::Detached;
            self.detach_requested = true;
        }
    }

    /// Whether the console input is focused.
    pub fn is_console_focused(&self) -> bool {
        self.console_focused
    }

    /// Handle a right-click on the panel (context menu).
    pub fn on_right_click(&mut self, x: f32, y: f32, styles: &StyleMap) -> bool {
        if !self.visible {
            return false;
        }

        // If context menu is already visible, try to click it.
        if self.context_menu.is_visible() {
            if let Some((action, node_id)) = self.context_menu.on_click(x, y) {
                self.handle_context_action(action, node_id, styles);
                return true;
            }
            self.context_menu.hide();
            return true;
        }

        let bounds = self.panel_bounds();
        if x < bounds.x || x > bounds.x + bounds.width
            || y < bounds.y || y > bounds.y + bounds.height
        {
            return false;
        }

        // Only show context menu in Elements tab on a node line.
        if self.active_tab == DevToolsTab::Elements {
            let tab_bar_h = 28.0;
            let content_y = bounds.y + 1.0 + tab_bar_h + 1.0 + 8.0;
            let line_h: f32 = 18.0;
            let scroll_y = (y - content_y) + self.scroll_offset;
            let line_idx = (scroll_y / line_h).floor() as usize;
            let visible = self.inspector.visible_nodes();
            if let Some(node) = visible.get(line_idx) {
                self.context_menu.show(node.id, x, y);
                return true;
            }
        }

        false
    }

    /// Handle a context menu action dispatched by the desktop.
    pub fn handle_context_action(
        &mut self,
        action: ContextAction,
        node_id: NodeId,
        styles: &StyleMap,
    ) {
        match action {
            ContextAction::InspectElement => {
                self.select_node(node_id, styles);
                self.set_tab(DevToolsTab::Elements);
            }
            ContextAction::ShowLayout => {
                self.select_node(node_id, styles);
                self.set_tab(DevToolsTab::Layout);
            }
            ContextAction::ShowInSceneGraph => {
                self.set_tab(DevToolsTab::SceneGraph);
            }
            ContextAction::LogToConsole => {
                self.console.push_output(format!("Logged node #{}", node_id));
                self.set_tab(DevToolsTab::Console);
            }
            ContextAction::ExpandAll => {
                self.inspector.expand(node_id);
            }
            ContextAction::CollapseAll => {
                self.inspector.collapse(node_id);
            }
            ContextAction::CopyNodeId => {
                self.console.push_info(format!("Node ID: {}", node_id));
            }
            _ => {
                // Other actions (copy HTML, force states, etc.) will be
                // implemented when the DOM modification API is available.
            }
        }
    }

    /// Handle a keyboard event when the console is focused.
    pub fn handle_console_key(
        &mut self,
        key: &str,
        _ctrl: bool,
        _shift: bool,
        doc: &Document,
        layout: &LayoutTree,
        styles: &StyleMap,
    ) -> bool {
        if !self.console_focused {
            return false;
        }

        match key {
            "Escape" => {
                self.console_focused = false;
                true
            }
            "Enter" | "Return" => {
                self.console.submit(doc, layout, styles);
                true
            }
            "Backspace" => {
                self.console.backspace();
                true
            }
            "Delete" => {
                self.console.delete();
                true
            }
            "ArrowLeft" | "Left" => {
                self.console.cursor_left();
                true
            }
            "ArrowRight" | "Right" => {
                self.console.cursor_right();
                true
            }
            "ArrowUp" | "Up" => {
                self.console.history_up();
                true
            }
            "ArrowDown" | "Down" => {
                self.console.history_down();
                true
            }
            "Home" => {
                self.console.cursor_home();
                true
            }
            "End" => {
                self.console.cursor_end();
                true
            }
            _ if key.len() == 1 => {
                if let Some(c) = key.chars().next() {
                    self.console.insert_char(c);
                }
                true
            }
            _ => false,
        }
    }

    /// Update the scene graph debugger snapshot from a scene root.
    pub fn update_scene_snapshot(&mut self, root: &SceneNode) {
        self.scene_debugger.snapshot(root);
    }

    /// Helper: create a simple text scene node.
    fn text_node(
        &self,
        id: u64,
        text: &str,
        color: Color,
        rect: Rect,
        z_order: u32,
    ) -> SceneNode {
        SceneNode::new(
            id,
            SceneNodeKind::Text {
                text: text.to_string(),
                color,
                scale: 1,
                font_family: self.config.font_family.clone(),
                font_size: self.config.font_size,
                font_weight: 400,
                font_style_italic: false,
                letter_spacing: 0.0,
                word_spacing: 0.0,
                line_height: 18.0,
                text_align: 0,
                text_transform: 0,
                text_overflow: 1,
                white_space: 1,
                text_indent: 0.0,
                text_decoration: None,
                text_shadows: vec![],
            },
            NodeProperties::new(rect).with_z_order(z_order),
        )
    }

    /// Convenience: update the inspector snapshot from the document.
    pub fn refresh_inspector(&mut self, doc: &Document) {
        self.inspector.build_snapshot(doc);
    }

    /// Get the dock position.
    pub fn dock_position(&self) -> DockPosition {
        self.config.dock_position
    }

    /// Change the dock position.
    pub fn set_dock_position(&mut self, pos: DockPosition) {
        self.config.dock_position = pos;
    }

    /// Get the panel size.
    pub fn panel_size(&self) -> f32 {
        self.config.panel_size
    }

    /// Resize the panel.
    pub fn set_panel_size(&mut self, size: f32) {
        self.config.panel_size = size.max(self.config.min_panel_size);
    }
}

impl Default for DevToolsPanel {
    fn default() -> Self {
        Self::with_defaults()
    }
}

// ── Mutation record formatting helpers ──

use crate::mutation_log::{MutationKind, MutationRecord};

/// Format a mutation record as a single-line description.
fn format_mutation_record(record: &MutationRecord) -> String {
    let ts = record.timestamp_ms;
    let desc = match &record.kind {
        MutationKind::ChildAdded { parent, child } => {
            format!("+child #{} → parent #{}", child, parent)
        }
        MutationKind::ChildRemoved { parent, child } => {
            format!("-child #{} ← parent #{}", child, parent)
        }
        MutationKind::AttributeChanged {
            node,
            attribute,
            new_value,
            ..
        } => {
            let val = new_value.as_deref().unwrap_or("(removed)");
            format!("attr #{} {}=\"{}\"", node, attribute, val)
        }
        MutationKind::ClassChanged { node, classes } => {
            format!("class #{} → [{}]", node, classes.join(" "))
        }
        MutationKind::TextChanged { node, text } => {
            let t = if text.len() > 40 { &text[..40] } else { text };
            format!("text #{} \"{}\"", node, t)
        }
        MutationKind::PseudoStateChanged {
            node,
            new_flags,
            ..
        } => {
            format!("pseudo #{} flags={:#x}", node, new_flags)
        }
        MutationKind::IdChanged { node, new_id, .. } => {
            let id = new_id.as_deref().unwrap_or("(none)");
            format!("id #{} → \"{}\"", node, id)
        }
    };
    format!("[{:>6}ms] {}", ts, desc)
}

/// Pick a color for different mutation kinds.
fn mutation_color(kind: &MutationKind) -> Color {
    match kind {
        MutationKind::ChildAdded { .. } => Color::new(78, 201, 176, 255), // teal (add)
        MutationKind::ChildRemoved { .. } => Color::new(244, 135, 113, 255), // red (remove)
        MutationKind::AttributeChanged { .. } => Color::new(156, 220, 254, 255), // blue
        MutationKind::ClassChanged { .. } => Color::new(206, 145, 120, 255), // orange
        MutationKind::TextChanged { .. } => Color::new(212, 212, 212, 255), // white
        MutationKind::PseudoStateChanged { .. } => Color::new(181, 206, 168, 255), // green
        MutationKind::IdChanged { .. } => Color::new(220, 220, 170, 255), // yellow
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toggle() {
        let mut panel = DevToolsPanel::with_defaults();
        assert!(!panel.is_visible());
        panel.toggle();
        assert!(panel.is_visible());
        panel.toggle();
        assert!(!panel.is_visible());
    }

    #[test]
    fn test_tab_cycling() {
        let mut panel = DevToolsPanel::with_defaults();
        assert_eq!(panel.active_tab(), DevToolsTab::Elements);
        panel.next_tab();
        assert_eq!(panel.active_tab(), DevToolsTab::Styles);
        panel.prev_tab();
        assert_eq!(panel.active_tab(), DevToolsTab::Elements);
    }

    #[test]
    fn test_keyboard_f12() {
        let mut panel = DevToolsPanel::with_defaults();
        assert!(!panel.is_visible());
        assert!(panel.handle_key("F12", false, false, false));
        assert!(panel.is_visible());
    }

    #[test]
    fn test_keyboard_ctrl_shift_i() {
        let mut panel = DevToolsPanel::with_defaults();
        assert!(panel.handle_key("I", true, true, false));
        assert!(panel.is_visible());
    }

    #[test]
    fn test_panel_bounds_bottom() {
        let mut panel = DevToolsPanel::with_defaults();
        panel.set_screen_size(1920.0, 1080.0);
        let bounds = panel.panel_bounds();
        assert_eq!(bounds.y, 1080.0 - 320.0);
        assert_eq!(bounds.width, 1920.0);
    }

    #[test]
    fn test_dock_position_change() {
        let mut panel = DevToolsPanel::with_defaults();
        panel.set_screen_size(1920.0, 1080.0);
        panel.set_dock_position(DockPosition::Right);
        let bounds = panel.panel_bounds();
        assert_eq!(bounds.x, 1920.0 - 320.0);
        assert_eq!(bounds.height, 1080.0);
    }

    #[test]
    fn test_hidden_scene_minimal() {
        let panel = DevToolsPanel::with_defaults();
        let layout = LayoutTree::new();
        let styles = StyleMap::new();
        let doc = Document::new();
        let scene = panel.build_scene(&doc, &layout, &styles);
        // When hidden, only overlay/picker nodes (both inactive → 0).
        assert!(scene.is_empty());
    }
}
