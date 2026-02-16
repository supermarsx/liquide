//! DevTools panel — the top-level container that composes all sub-panels
//! into a docked/floating developer tools window.
//!
//! The panel is designed to be rendered as an overlay on top of the
//! compositor scene. It handles tab switching, keyboard shortcuts,
//! and coordinates the inspector, style panel, layout overlay, element
//! picker, mutation log, and DOM serializer.

use std::collections::VecDeque;
use std::time::Instant;

use liquide_components::TemplateNode;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::Color;
use liquide_compositor::scene::{NodeProperties, SceneNode, SceneNodeKind};
use liquide_dom::{Document, NodeId};
use liquide_hit_test::HitTestEngine;
use liquide_layout::tree::LayoutTree;
use liquide_style_engine::StyleMap;

/// Lightweight pipeline / performance snapshot that desktop.rs pushes
/// into the devtools panel each frame so the Debugger tab has live numbers.
#[derive(Debug, Clone)]
pub struct FrameSnapshot {
    /// Monotonic frame counter.
    pub frame_number: u64,
    /// Current frames-per-second estimate.
    pub fps: f64,
    /// Average frame time in milliseconds.
    pub avg_frame_ms: f64,
    /// Total CSS rules loaded across all stylesheets.
    pub css_rule_count: usize,
    /// Total CSS variables defined.
    pub css_variable_count: usize,
    /// Number of stylesheet sources loaded.
    pub stylesheet_count: usize,
    /// Viewport width.
    pub viewport_w: f32,
    /// Viewport height.
    pub viewport_h: f32,
}

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
    /// Element tree + side-panel (Styles / Layout / Computed / Fonts / Anim).
    Elements,
    /// Interactive debug console.
    Console,
    /// Document overview + DOM tree + source files.
    Sources,
    /// Pipeline metrics, frame timing, CSS engine stats.
    Performance,
    /// DOM mutation log.
    Mutations,
    /// Scene graph debugger + live style editor.
    Scene,
}

impl DevToolsTab {
    /// All available tabs in order.
    pub const ALL: &'static [DevToolsTab] = &[
        DevToolsTab::Elements,
        DevToolsTab::Console,
        DevToolsTab::Sources,
        DevToolsTab::Performance,
        DevToolsTab::Mutations,
        DevToolsTab::Scene,
    ];

    /// Human-readable label for the tab.
    pub fn label(&self) -> &'static str {
        match self {
            DevToolsTab::Elements => "Elements",
            DevToolsTab::Console => "Console",
            DevToolsTab::Sources => "Sources",
            DevToolsTab::Performance => "Performance",
            DevToolsTab::Mutations => "Mutations",
            DevToolsTab::Scene => "Scene",
        }
    }
}

/// Which sub-tab is active in the Elements side panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SideTab {
    /// Computed CSS properties grouped by category.
    Styles,
    /// Box model + layout properties.
    Layout,
    /// Computed final values.
    Computed,
    /// Font properties and rendering info.
    Fonts,
    /// Transitions and CSS animations.
    Animations,
}

impl SideTab {
    /// All side-panel sub-tabs.
    pub const ALL: &'static [SideTab] = &[
        SideTab::Styles,
        SideTab::Layout,
        SideTab::Computed,
        SideTab::Fonts,
        SideTab::Animations,
    ];

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            SideTab::Styles => "Styles",
            SideTab::Layout => "Layout",
            SideTab::Computed => "Computed",
            SideTab::Fonts => "Fonts",
            SideTab::Animations => "Anim",
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
    /// Active sub-tab in the Elements side panel.
    side_tab: SideTab,
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
    #[allow(dead_code)]
    tab_scroll: f32,
    /// Whether the console input is focused for keyboard capture.
    console_focused: bool,
    /// Epoch for cursor blink animation — reset on each keystroke so the
    /// caret stays solid for 500 ms after the last input.
    caret_blink_epoch: Instant,
    /// Latest frame snapshot from the pipeline (Debugger tab).
    frame_snapshot: Option<FrameSnapshot>,
    /// Recent frame times (ms) for sparkline display — last ~120 frames.
    frame_times: VecDeque<f64>,
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
            side_tab: SideTab::Styles,
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
            frame_snapshot: None,
            frame_times: VecDeque::with_capacity(128),
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

    // ─── Pipeline Stats ───────────────────────────────────────

    /// Push a frame snapshot so the Debugger tab can display live numbers.
    pub fn push_frame_snapshot(&mut self, snap: FrameSnapshot) {
        let ft = snap.avg_frame_ms;
        self.frame_snapshot = Some(snap);
        self.frame_times.push_back(ft);
        if self.frame_times.len() > 120 {
            self.frame_times.pop_front();
        }
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

    /// Get the active side tab.
    pub fn side_tab(&self) -> SideTab {
        self.side_tab
    }

    /// Set the active side tab.
    pub fn set_side_tab(&mut self, tab: SideTab) {
        self.side_tab = tab;
    }

    // ─── Virtual scroll helpers ───────────────────────────────

    /// Compute the available content height (panel height minus toolbar and
    /// status bar) in pixels.
    fn content_height(&self) -> f32 {
        let bounds = self.panel_bounds();
        let toolbar_h = 30.0;
        let statusbar_h = 20.0;
        let borders = 2.0; // top + bottom border
        (bounds.height - toolbar_h - statusbar_h - borders).max(0.0)
    }

    /// Given a fixed `row_height`, return `(first_visible_index, count)` for
    /// virtual scrolling so that only visible rows are emitted.
    fn visible_row_range(&self, total_rows: usize, row_height: f32) -> (usize, usize) {
        let ch = self.content_height();
        if ch <= 0.0 || total_rows == 0 {
            return (0, 0);
        }
        let first = (self.scroll_offset / row_height).floor() as usize;
        let count = (ch / row_height).ceil() as usize + 1; // +1 for partial row
        let first = first.min(total_rows.saturating_sub(1));
        let count = count.min(total_rows - first);
        (first, count)
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
                let line_h: f32 = 20.0;
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
            // Check toolbar buttons (right side of tab bar).
            // Layout mirrors toolbar: [picker][detach][dock-bottom][dock-right][close]
            // from right to left.
            let btn_size = 20.0;
            let btn_gap = 2.0;
            let mut btn_right = bounds.x + bounds.width - 8.0;

            // Close button
            let close_left = btn_right - btn_size;
            if x >= close_left && x < btn_right {
                self.hide();
                return true;
            }
            btn_right = close_left - btn_gap;

            // Dock-right button
            let dr_left = btn_right - btn_size;
            if x >= dr_left && x < btn_right {
                self.config.dock_position = DockPosition::Right;
                return true;
            }
            btn_right = dr_left - btn_gap;

            // Dock-bottom button
            let db_left = btn_right - btn_size;
            if x >= db_left && x < btn_right {
                self.config.dock_position = DockPosition::Bottom;
                return true;
            }
            btn_right = db_left - btn_gap;

            // Detach button
            let det_left = btn_right - btn_size;
            if x >= det_left && x < btn_right {
                self.toggle_detach();
                return true;
            }
            btn_right = det_left - btn_gap;

            // Picker toggle button
            let pk_left = btn_right - btn_size;
            if x >= pk_left && x < btn_right {
                self.toggle_picker();
                return true;
            }

            // Tab labels region
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
                    // Check if click is in the side panel region (right 260px).
                    let side_w = 260.0f32;
                    let side_left = bounds.x + bounds.width - side_w;

                    if x >= side_left {
                        // Side panel — check sub-tab bar (top 24px of side pane).
                        let side_tab_bottom = content_y + 24.0;
                        if y < side_tab_bottom {
                            // Side tab click.
                            let mut stab_x = side_left + 4.0;
                            for stab in SideTab::ALL {
                                let tw = stab.label().len() as f32 * 6.5 + 16.0;
                                if x >= stab_x && x < stab_x + tw {
                                    self.side_tab = *stab;
                                    return true;
                                }
                                stab_x += tw;
                            }
                        }
                        // Otherwise, side panel content click — handle in Styles sub-tab.
                        if self.side_tab == SideTab::Styles {
                            let side_content_y = content_y + 24.0;
                            let line_h: f32 = 17.0;
                            let line_idx = ((y - side_content_y) / line_h).floor() as usize;
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
                        return true;
                    }

                    // Main pane — DOM tree click.
                    let line_h: f32 = 20.0;
                    let scroll_y = (y - content_y) + self.scroll_offset;
                    let line_idx = (scroll_y / line_h).floor() as usize;
                    let visible = self.inspector.visible_nodes();
                    if let Some(node) = visible.get(line_idx) {
                        let node_id = node.id;
                        let indent_px: f32 = 16.0;
                        let arrow_x = bounds.x + 8.0 + (node.depth as f32) * indent_px;

                        if x < arrow_x + 16.0 && node.child_count > 0 {
                            self.inspector.toggle_expand(node_id);
                        } else {
                            self.select_node(node_id, styles);
                        }
                        return true;
                    }
                }
                DevToolsTab::Console => {
                    self.console_focused = true;
                }
                DevToolsTab::Scene => {
                    let line_h: f32 = 16.0;
                    let scroll_y = (y - content_y) + self.scroll_offset;
                    let line_idx = (scroll_y / line_h).floor() as usize;
                    self.scene_debugger.select(Some(line_idx));
                }
                _ => {}
            }
            return true;
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
                let line_h: f32 = 20.0;
                self.inspector.visible_nodes().len() as f32 * line_h
            }
            DevToolsTab::Sources => {
                let line_h: f32 = 16.0;
                10_000.0 * line_h // DOM JSON can be very long
            }
            DevToolsTab::Mutations => {
                let line_h: f32 = 17.0;
                (1 + self.mutation_log.len()) as f32 * line_h
            }
            DevToolsTab::Console => {
                let line_h: f32 = 18.0;
                (self.console.entries().len() + 2) as f32 * line_h
            }
            DevToolsTab::Scene => {
                let line_h: f32 = 16.0;
                self.scene_debugger.entries().len() as f32 * line_h
            }
            DevToolsTab::Performance => {
                let line_h: f32 = 16.0;
                40.0 * line_h // performance tab is relatively short
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
                let line_h: f32 = 20.0;
                let target_y = idx as f32 * line_h;
                let content_h = self.content_height();

                if target_y < self.scroll_offset {
                    self.scroll_offset = target_y;
                } else if target_y + line_h > self.scroll_offset + content_h {
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

    // ─── Template-based rendering ─────────────────────────────

    /// Produce a declarative `TemplateNode` tree for the entire devtools panel.
    ///
    /// The shell applies this to its document via `TemplateRenderer::apply_or_create`,
    /// and the CSS pipeline renders it as part of the normal scene.
    /// This replaces all the hand-built `SceneNode` vectors that were here before.
    ///
    /// Overlay highlights (picker, selection, hover) remain as direct scene nodes
    /// because they must render on top of the page viewport, not inside the panel.
    pub fn render_template(
        &self,
        doc: &Document,
        layout: &LayoutTree,
        styles: &StyleMap,
    ) -> TemplateNode {
        let dock_class = match self.config.dock_position {
            DockPosition::Bottom => "dock-bottom",
            DockPosition::Right => "dock-right",
            DockPosition::Left => "dock-left",
            DockPosition::Float => "dock-float",
            DockPosition::Detached => "dock-detached",
        };

        let tab_id = self.tab_data_id();

        TemplateNode::el("devtools-panel")
            .id("devtools-panel")
            .class(dock_class)
            .child(self.render_toolbar_template())
            .child(self.render_content_template(doc, layout, styles, tab_id))
            .child(self.render_statusbar_template())
    }

    /// Map active tab to its data-tab string.
    fn tab_data_id(&self) -> &'static str {
        match self.active_tab {
            DevToolsTab::Elements => "elements",
            DevToolsTab::Console => "console",
            DevToolsTab::Sources => "sources",
            DevToolsTab::Performance => "perf",
            DevToolsTab::Mutations => "mutations",
            DevToolsTab::Scene => "scene",
        }
    }

    /// Toolbar: tab labels + action buttons.
    fn render_toolbar_template(&self) -> TemplateNode {
        let tabs = TemplateNode::el("devtools-tabs").children(
            DevToolsTab::ALL.iter().map(|tab| {
                let id = match tab {
                    DevToolsTab::Elements => "elements",
                    DevToolsTab::Console => "console",
                    DevToolsTab::Sources => "sources",
                    DevToolsTab::Performance => "perf",
                    DevToolsTab::Mutations => "mutations",
                    DevToolsTab::Scene => "scene",
                };
                TemplateNode::el("devtools-tab")
                    .key(&format!("tab-{}", id))
                    .attr("data-tab", id)
                    .class_if("active", *tab == self.active_tab)
                    .child(TemplateNode::text(tab.label()))
            }),
        );

        let actions = TemplateNode::el("devtools-actions")
            .child(
                TemplateNode::el("devtools-btn")
                    .key("btn-picker")
                    .attr("data-action", "picker")
                    .class_if("active", self.element_picker.is_active())
                    .child(TemplateNode::text("\u{2295}")), // ⊕
            )
            .child(
                TemplateNode::el("devtools-btn")
                    .key("btn-detach")
                    .attr("data-action", "detach")
                    .class_if("active", self.config.dock_position == DockPosition::Detached)
                    .child(TemplateNode::text("\u{29C9}")), // ⧉ detach icon
            )
            .child(
                TemplateNode::el("devtools-btn")
                    .key("btn-dock-bottom")
                    .attr("data-action", "dock-bottom")
                    .class_if("active", self.config.dock_position == DockPosition::Bottom)
                    .child(TemplateNode::text("\u{22A5}")), // ⊥
            )
            .child(
                TemplateNode::el("devtools-btn")
                    .key("btn-dock-right")
                    .attr("data-action", "dock-right")
                    .class_if("active", self.config.dock_position == DockPosition::Right)
                    .child(TemplateNode::text("\u{22A2}")), // ⊢
            )
            .child(
                TemplateNode::el("devtools-btn")
                    .key("btn-close")
                    .attr("data-action", "close")
                    .child(TemplateNode::text("\u{00D7}")), // ×
            );

        TemplateNode::el("devtools-toolbar")
            .child(tabs)
            .child(actions)
    }

    /// Content area: active tab panel with its children.
    fn render_content_template(
        &self,
        doc: &Document,
        layout: &LayoutTree,
        styles: &StyleMap,
        active_tab: &str,
    ) -> TemplateNode {
        let tab_defs: &[(&str, fn(&Self, &Document, &LayoutTree, &StyleMap) -> Vec<TemplateNode>)] = &[
            ("elements", Self::template_elements),
            ("console", Self::template_console),
            ("sources", Self::template_sources),
            ("perf", Self::template_performance),
            ("mutations", Self::template_mutations),
            ("scene", Self::template_scene),
        ];

        TemplateNode::el("devtools-content")
            .children(tab_defs.iter().map(|(id, render_fn)| {
                let is_active = *id == active_tab;
                let mut panel = TemplateNode::el("devtools-tab-panel")
                    .key(&format!("panel-{}", id))
                    .attr("data-tab", id)
                    .class_if("active", is_active);
                if is_active {
                    panel = panel.children(render_fn(self, doc, layout, styles));
                }
                panel
            }))
    }

    /// Status bar at the bottom.
    fn render_statusbar_template(&self) -> TemplateNode {
        let text = if self.element_picker.is_active() {
            "Picker active \u{2014} click an element to inspect".to_string()
        } else {
            match self.selected_node {
                Some(id) => format!("Node #{} selected", id),
                None => "No element selected".to_string(),
            }
        };

        TemplateNode::el("devtools-statusbar")
            .child(
                TemplateNode::el("devtools-status-text")
                    .child(TemplateNode::text(&text)),
            )
    }

    // ─── Per-tab template renderers ─────────────────────────────

    /// Elements tab: split pane with DOM tree (left) and side panel (right).
    ///
    /// The side-panel has sub-tabs: Styles, Layout, Computed, Fonts, Anim.
    /// The DOM tree uses virtual scrolling — only visible rows are emitted.
    fn template_elements(
        &self,
        _doc: &Document,
        layout: &LayoutTree,
        styles: &StyleMap,
    ) -> Vec<TemplateNode> {
        // ── Left pane: DOM tree with virtual scrolling ──
        let main_pane = {
            let visible = self.inspector.visible_nodes();
            let row_h: f32 = 20.0;
            let (first, count) = self.visible_row_range(visible.len(), row_h);

            let selected = self.inspector.selected();
            let hovered = self.inspector.hovered();

            if visible.is_empty() {
                TemplateNode::el("devtools-main-pane")
                    .child(TemplateNode::el("devtools-tree")
                        .child(TemplateNode::el("devtools-row")
                            .child(TemplateNode::el("devtools-value").class("dim")
                                .child(TemplateNode::text("No elements \u{2014} run refresh_inspector(doc)")))))
            } else {
                let tree = TemplateNode::el("devtools-tree").children(
                    visible[first..first + count].iter().map(|node| {
                        let is_selected = selected == Some(node.id);
                        let is_hovered = hovered == Some(node.id);

                        let mut row = TemplateNode::el("devtools-tree-row")
                            .key(&format!("n-{}", node.id))
                            .attr("data-node", &node.id.to_string())
                            .style("padding-left", &format!("{}px", node.depth as u32 * 16 + 4))
                            .class_if("selected", is_selected)
                            .class_if("hovered", is_hovered);

                        if node.is_text {
                            let text = node.text.as_deref().unwrap_or("");
                            let truncated = if text.len() > 60 { &text[..60] } else { text };
                            row = row.child(
                                TemplateNode::el("devtools-tree-text")
                                    .child(TemplateNode::text(&format!("\"{}\"", truncated))),
                            );
                        } else {
                            let arrow = if node.child_count > 0 {
                                if node.children.is_empty() { "\u{25B6}" } else { "\u{25BC}" }
                            } else { "" };

                            if node.child_count > 0 {
                                row = row.child(
                                    TemplateNode::el("devtools-tree-arrow")
                                        .child(TemplateNode::text(arrow)),
                                );
                            }

                            row = row.child(
                                TemplateNode::el("devtools-tree-tag")
                                    .child(TemplateNode::text(&format!("<{}", node.tag))),
                            );

                            if let Some(ref eid) = node.element_id {
                                row = row.child(
                                    TemplateNode::el("devtools-tree-attr")
                                        .child(TemplateNode::text(&format!(" id=\"{}\"", eid))),
                                );
                            }
                            if !node.classes.is_empty() {
                                row = row.child(
                                    TemplateNode::el("devtools-tree-attr")
                                        .child(TemplateNode::text(&format!(" class=\"{}\"", node.classes.join(" ")))),
                                );
                            }
                            for (k, v) in &node.attributes {
                                row = row.child(
                                    TemplateNode::el("devtools-tree-attr")
                                        .child(TemplateNode::text(&format!(" {}=\"{}\"", k, v))),
                                );
                            }
                            row = row.child(
                                TemplateNode::el("devtools-tree-tag")
                                    .child(TemplateNode::text(">")),
                            );
                        }
                        row
                    }),
                );
                TemplateNode::el("devtools-main-pane").child(tree)
            }
        };

        // ── Right pane: side panel with sub-tabs ──
        let side_pane = {
            // Sub-tab bar.
            let side_tabs = TemplateNode::el("devtools-side-tabs").children(
                SideTab::ALL.iter().map(|st| {
                    TemplateNode::el("devtools-side-tab")
                        .key(&format!("st-{}", st.label()))
                        .class_if("active", *st == self.side_tab)
                        .child(TemplateNode::text(st.label()))
                }),
            );

            // Sub-tab content.
            let body_children = match self.side_tab {
                SideTab::Styles => self.side_styles(styles),
                SideTab::Layout => self.side_layout(layout, styles),
                SideTab::Computed => self.side_computed(styles),
                SideTab::Fonts => self.side_fonts(styles),
                SideTab::Animations => self.side_animations(styles),
            };

            TemplateNode::el("devtools-side-pane")
                .child(side_tabs)
                .child(TemplateNode::el("devtools-side-body").children(body_children))
        };

        let split = TemplateNode::el("devtools-split")
            .child(main_pane)
            .child(side_pane);

        vec![split]
    }

    // ── Side panel sub-tab content ──────────────────────────────

    /// Side: Styles — computed CSS properties grouped by category.
    fn side_styles(&self, styles: &StyleMap) -> Vec<TemplateNode> {
        let Some(id) = self.selected_node else {
            return vec![TemplateNode::el("devtools-row")
                .child(TemplateNode::el("devtools-value").class("dim")
                    .child(TemplateNode::text("Select an element")))];
        };
        if styles.get(id).is_none() {
            return vec![TemplateNode::el("devtools-row")
                .child(TemplateNode::el("devtools-value").class("dim")
                    .child(TemplateNode::text("No styles")))];
        }

        let mut sections = Vec::new();
        for (cat, props) in self.style_inspector.grouped_properties() {
            let cat_label = format!("{:?}", cat);
            let mut section = TemplateNode::el("devtools-style-section")
                .key(&cat_label)
                .child(
                    TemplateNode::el("devtools-section-header")
                        .child(TemplateNode::text(&format!("\u{25BC} {}", cat_label))),
                );
            for prop in &props {
                let is_editing = self.style_editor.editing_property() == Some(prop.name.as_str());
                section = section.child(
                    TemplateNode::el("devtools-prop")
                        .key(&prop.name)
                        .class_if("inherited", prop.inherited)
                        .class_if("editing", is_editing)
                        .child(
                            TemplateNode::el("devtools-prop-name")
                                .child(TemplateNode::text(&format!("{}:", prop.name))),
                        )
                        .child(
                            TemplateNode::el("devtools-prop-value")
                                .class("editable")
                                .child(TemplateNode::text(
                                    if is_editing { self.style_editor.editing_value() } else { &prop.value }
                                )),
                        ),
                );
            }
            sections.push(section);
        }
        sections
    }

    /// Side: Layout — box model + layout properties.
    fn side_layout(&self, layout: &LayoutTree, styles: &StyleMap) -> Vec<TemplateNode> {
        let Some(id) = self.selected_node else {
            return vec![TemplateNode::el("devtools-row")
                .child(TemplateNode::el("devtools-value").class("dim")
                    .child(TemplateNode::text("Select an element")))];
        };
        let layout_box = match layout.find_by_node(id) {
            Some(b) => b,
            None => return vec![TemplateNode::el("devtools-row")
                .child(TemplateNode::el("devtools-value").class("dim")
                    .child(TemplateNode::text("No layout box")))],
        };

        let mut nodes = Vec::new();

        // Box model.
        let mr = &layout_box.margin_rect;
        let br = &layout_box.border_rect;
        let pr = &layout_box.padding_rect;
        let cr = &layout_box.content_rect;

        let margin_t = br.y - mr.y;
        let margin_r_val = (mr.x + mr.width) - (br.x + br.width);
        let margin_b = (mr.y + mr.height) - (br.y + br.height);
        let margin_l = br.x - mr.x;

        let border_t = pr.y - br.y;
        let border_r_val = (br.x + br.width) - (pr.x + pr.width);
        let border_b = (br.y + br.height) - (pr.y + pr.height);
        let border_l = pr.x - br.x;

        let padding_t = cr.y - pr.y;
        let padding_r_val = (pr.x + pr.width) - (cr.x + cr.width);
        let padding_b = (pr.y + pr.height) - (cr.y + cr.height);
        let padding_l = cr.x - pr.x;

        nodes.push(
            TemplateNode::el("devtools-box-model")
                .child(
                    TemplateNode::el("devtools-box-margin")
                        .attr("data-label", &format!("m: {:.0} {:.0} {:.0} {:.0}", margin_t, margin_r_val, margin_b, margin_l))
                        .child(TemplateNode::el("devtools-box-border")
                            .attr("data-label", &format!("b: {:.0} {:.0} {:.0} {:.0}", border_t, border_r_val, border_b, border_l))
                            .child(TemplateNode::el("devtools-box-padding")
                                .attr("data-label", &format!("p: {:.0} {:.0} {:.0} {:.0}", padding_t, padding_r_val, padding_b, padding_l))
                                .child(TemplateNode::el("devtools-box-content")
                                    .child(TemplateNode::text(&format!("{:.0}\u{00D7}{:.0}", cr.width, cr.height)))))),
                ),
        );

        // Layout properties.
        if let Some(computed) = styles.get(id) {
            let prop_list = [
                ("position", format!("{:?}", computed.position)),
                ("display", format!("{:?}", computed.display)),
                ("box-sizing", format!("{:?}", computed.box_sizing)),
                ("overflow-x", format!("{:?}", computed.overflow_x)),
                ("overflow-y", format!("{:?}", computed.overflow_y)),
                ("float", format!("{:?}", computed.float)),
                ("clear", format!("{:?}", computed.clear)),
            ];
            for (name, value) in &prop_list {
                nodes.push(TemplateNode::el("devtools-row").key(name)
                    .child(TemplateNode::el("devtools-label").child(TemplateNode::text(&format!("{}:", name))))
                    .child(TemplateNode::el("devtools-value").child(TemplateNode::text(value))));
            }

            if format!("{:?}", computed.display).contains("Flex") {
                nodes.push(TemplateNode::el("devtools-heading").child(TemplateNode::text("Flexbox")));
                let flex_props = [
                    ("flex-direction", format!("{:?}", computed.flex_direction)),
                    ("flex-wrap", format!("{:?}", computed.flex_wrap)),
                    ("justify-content", format!("{:?}", computed.justify_content)),
                    ("align-items", format!("{:?}", computed.align_items)),
                    ("align-content", format!("{:?}", computed.align_content)),
                    ("gap", format!("{:?}", computed.gap)),
                ];
                for (name, value) in &flex_props {
                    nodes.push(TemplateNode::el("devtools-row").key(name)
                        .child(TemplateNode::el("devtools-label").child(TemplateNode::text(&format!("{}:", name))))
                        .child(TemplateNode::el("devtools-value").child(TemplateNode::text(value))));
                }
            }
        }
        nodes
    }

    /// Side: Computed — all visible (filtered) properties as a flat list.
    fn side_computed(&self, styles: &StyleMap) -> Vec<TemplateNode> {
        let Some(id) = self.selected_node else {
            return vec![TemplateNode::el("devtools-row")
                .child(TemplateNode::el("devtools-value").class("dim")
                    .child(TemplateNode::text("Select an element")))];
        };
        if styles.get(id).is_none() {
            return vec![TemplateNode::el("devtools-row")
                .child(TemplateNode::el("devtools-value").class("dim")
                    .child(TemplateNode::text("No styles")))];
        }

        let props = self.style_inspector.visible_properties();
        props.iter().map(|prop| {
            TemplateNode::el("devtools-prop")
                .key(&prop.name)
                .class_if("inherited", prop.inherited)
                .child(
                    TemplateNode::el("devtools-prop-name")
                        .child(TemplateNode::text(&format!("{}:", prop.name))),
                )
                .child(
                    TemplateNode::el("devtools-prop-value")
                        .child(TemplateNode::text(&prop.value)),
                )
        }).collect()
    }

    /// Side: Fonts — font properties from computed style.
    fn side_fonts(&self, styles: &StyleMap) -> Vec<TemplateNode> {
        let Some(id) = self.selected_node else {
            return vec![TemplateNode::el("devtools-row")
                .child(TemplateNode::el("devtools-value").class("dim")
                    .child(TemplateNode::text("Select an element")))];
        };
        let computed = match styles.get(id) {
            Some(c) => c,
            None => return vec![TemplateNode::el("devtools-row")
                .child(TemplateNode::el("devtools-value").class("dim")
                    .child(TemplateNode::text("No styles")))],
        };

        let mut nodes = Vec::new();
        let families: Vec<String> = computed.font_family.iter().map(|f| format!("\"{}\"", f)).collect();
        let primary_props = [
            ("font-family", families.join(", ")),
            ("font-size", format!("{:.1}px", computed.font_size)),
            ("font-weight", format!("{}", computed.font_weight)),
            ("font-style", format!("{:?}", computed.font_style)),
            ("line-height", format!("{:?}", computed.line_height)),
            ("letter-spacing", format!("{:?}", computed.letter_spacing)),
            ("word-spacing", format!("{:?}", computed.word_spacing)),
        ];
        for (name, value) in &primary_props {
            nodes.push(TemplateNode::el("devtools-row").key(name)
                .child(TemplateNode::el("devtools-label").child(TemplateNode::text(&format!("{}:", name))))
                .child(TemplateNode::el("devtools-value").child(TemplateNode::text(value))));
        }

        nodes.push(TemplateNode::el("devtools-heading").child(TemplateNode::text("Typography")));
        let typo_props = [
            ("text-align", format!("{:?}", computed.text_align)),
            ("text-transform", format!("{:?}", computed.text_transform)),
            ("white-space", format!("{:?}", computed.white_space)),
            ("word-break", format!("{:?}", computed.word_break)),
        ];
        for (name, value) in &typo_props {
            nodes.push(TemplateNode::el("devtools-row").key(&format!("f-{}", name))
                .child(TemplateNode::el("devtools-label").child(TemplateNode::text(&format!("{}:", name))))
                .child(TemplateNode::el("devtools-value").child(TemplateNode::text(value))));
        }

        nodes.push(TemplateNode::el("devtools-heading").child(TemplateNode::text("Rendered Font")));
        for (i, family) in computed.font_family.iter().enumerate() {
            let marker = if i == 0 { "\u{25B6}" } else { "\u{2003}\u{25B7}" };
            nodes.push(TemplateNode::el("devtools-row").key(&format!("rf-{}", i))
                .child(TemplateNode::el("devtools-value")
                    .class_if("teal", i == 0)
                    .class_if("dim", i > 0)
                    .child(TemplateNode::text(&format!("{} \"{}\" \u{2014} {:.0}px, wt {}", marker, family, computed.font_size, computed.font_weight)))));
        }
        nodes
    }

    /// Side: Animations — transitions and CSS animations.
    fn side_animations(&self, styles: &StyleMap) -> Vec<TemplateNode> {
        let Some(id) = self.selected_node else {
            return vec![TemplateNode::el("devtools-row")
                .child(TemplateNode::el("devtools-value").class("dim")
                    .child(TemplateNode::text("Select an element")))];
        };
        let computed = match styles.get(id) {
            Some(c) => c,
            None => return vec![TemplateNode::el("devtools-row")
                .child(TemplateNode::el("devtools-value").class("dim")
                    .child(TemplateNode::text("No styles")))],
        };

        let mut nodes = Vec::new();
        nodes.push(TemplateNode::el("devtools-heading").child(TemplateNode::text("Transitions")));
        if computed.transition.is_empty() {
            nodes.push(TemplateNode::el("devtools-row")
                .child(TemplateNode::el("devtools-value").class("dim").child(TemplateNode::text("none"))));
        } else {
            for (i, tr) in computed.transition.iter().enumerate() {
                let timing_str = format_timing_function(&tr.timing_function);
                nodes.push(TemplateNode::el("devtools-row").key(&format!("tr-{}", i))
                    .child(TemplateNode::el("devtools-label").child(TemplateNode::text(&tr.property)))
                    .child(TemplateNode::el("devtools-value").child(TemplateNode::text(
                        &format!("{}ms {} delay {}ms", tr.duration_ms, timing_str, tr.delay_ms)))));
            }
        }

        nodes.push(TemplateNode::el("devtools-heading").child(TemplateNode::text("CSS Animations")));
        if computed.animation.is_empty() {
            nodes.push(TemplateNode::el("devtools-row")
                .child(TemplateNode::el("devtools-value").class("dim").child(TemplateNode::text("none"))));
        } else {
            for (i, anim) in computed.animation.iter().enumerate() {
                let timing_str = format_timing_function(&anim.timing_function);
                nodes.push(TemplateNode::el("devtools-row").key(&format!("an-{}", i))
                    .child(TemplateNode::el("devtools-label").child(TemplateNode::text(&anim.name)))
                    .child(TemplateNode::el("devtools-value").child(TemplateNode::text(
                        &format!("{}ms {} x{:?} {:?} {:?}", anim.duration_ms, timing_str, anim.iteration_count, anim.direction, anim.fill_mode)))));
            }
        }

        if !computed.transform.is_empty() || computed.opacity < 1.0 {
            nodes.push(TemplateNode::el("devtools-heading").child(TemplateNode::text("Related")));
            if !computed.transform.is_empty() {
                nodes.push(TemplateNode::el("devtools-row").key("rp-transform")
                    .child(TemplateNode::el("devtools-label").child(TemplateNode::text("transform:")))
                    .child(TemplateNode::el("devtools-value").child(TemplateNode::text(&format!("{:?}", computed.transform)))));
            }
            if computed.opacity < 1.0 {
                nodes.push(TemplateNode::el("devtools-row").key("rp-opacity")
                    .child(TemplateNode::el("devtools-label").child(TemplateNode::text("opacity:")))
                    .child(TemplateNode::el("devtools-value").child(TemplateNode::text(&format!("{:.2}", computed.opacity)))));
            }
        }
        nodes
    }

    // ── Main tab content renderers ──────────────────────────────

    /// Console tab: log entries + input line.
    fn template_console(
        &self,
        _doc: &Document,
        _layout: &LayoutTree,
        _styles: &StyleMap,
    ) -> Vec<TemplateNode> {
        let entries = self.console.entries();

        // Virtual scroll the console log entries.
        let entry_h: f32 = 18.0;
        // Reserve 26px for the input bar.
        let avail = (self.content_height() - 26.0).max(0.0);
        let max_visible = (avail / entry_h).ceil() as usize + 1;
        let total = entries.len();
        let first = if total > max_visible { total - max_visible } else { 0 };

        let log = TemplateNode::el("devtools-console-log").children(
            entries.iter().enumerate().skip(first).map(|(i, entry)| {
                let kind_class = match entry.kind {
                    crate::console::ConsoleEntryKind::Input => "info",
                    crate::console::ConsoleEntryKind::Output => "log",
                    crate::console::ConsoleEntryKind::Warning => "warn",
                    crate::console::ConsoleEntryKind::Error => "error",
                    crate::console::ConsoleEntryKind::Info => "info",
                };
                TemplateNode::el("devtools-console-entry")
                    .key(&format!("ce-{}", i))
                    .class(kind_class)
                    .child(TemplateNode::text(&entry.text))
            }),
        );

        let input_text = self.console.input_buffer().to_string();
        let input = TemplateNode::el("devtools-console-input")
            .class_if("focused", self.console_focused)
            .child(TemplateNode::el("devtools-console-prompt").child(TemplateNode::text(">")))
            .child(TemplateNode::el("devtools-console-field").child(TemplateNode::text(&input_text)));

        vec![log, input]
    }

    /// Sources tab: document overview + DOM JSON.
    fn template_sources(
        &self,
        doc: &Document,
        layout: &LayoutTree,
        styles: &StyleMap,
    ) -> Vec<TemplateNode> {
        let mut nodes = Vec::new();

        nodes.push(TemplateNode::el("devtools-heading").child(TemplateNode::text("Document Overview")));

        let total_nodes = doc.node_count();
        let total_boxes = layout.box_count();
        let styled_count = styles.len();

        nodes.push(row_kv("Total DOM nodes", &total_nodes.to_string(), "teal"));
        nodes.push(row_kv("Layout boxes", &total_boxes.to_string(), "teal"));
        nodes.push(row_kv("Styled elements", &styled_count.to_string(), "teal"));

        let mut element_count = 0u32;
        let mut text_count = 0u32;
        for descendant in doc.descendants(doc.root()) {
            if let Some(node) = doc.get(descendant) {
                if node.is_text() {
                    text_count += 1;
                } else {
                    element_count += 1;
                }
            }
        }

        nodes.push(TemplateNode::el("devtools-heading").child(TemplateNode::text("Node Types")));
        nodes.push(row_kv("Elements", &element_count.to_string(), "blue"));
        nodes.push(row_kv("Text nodes", &text_count.to_string(), "blue"));

        // Tag distribution.
        let mut tag_counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
        for descendant in doc.descendants(doc.root()) {
            if let Some(node) = doc.get(descendant) {
                if !node.is_text() {
                    *tag_counts.entry(node.tag_name().to_string()).or_insert(0) += 1;
                }
            }
        }
        let mut sorted_tags: Vec<_> = tag_counts.into_iter().collect();
        sorted_tags.sort_by(|a, b| b.1.cmp(&a.1));

        if !sorted_tags.is_empty() {
            nodes.push(TemplateNode::el("devtools-heading").child(TemplateNode::text("Tag Distribution")));
            for (tag, count) in sorted_tags.iter().take(15) {
                nodes.push(row_kv(&format!("<{}>", tag), &count.to_string(), "purple"));
            }
        }

        // DOM JSON (virtual scrolled).
        nodes.push(TemplateNode::el("devtools-separator"));
        nodes.push(TemplateNode::el("devtools-heading").child(TemplateNode::text("DOM Tree (JSON)")));

        let json = self.dom_serializer.to_json(doc);
        let lines: Vec<&str> = json.lines().collect();
        let line_h: f32 = 16.0;
        let (first, count) = self.visible_row_range(lines.len(), line_h);

        let json_block = TemplateNode::el("devtools-dom-json").children(
            lines[first..first + count].iter().enumerate().map(|(i, line)| {
                TemplateNode::el("devtools-row")
                    .key(&format!("dj-{}", first + i))
                    .child(TemplateNode::el("devtools-value").class("teal").child(TemplateNode::text(line)))
            }),
        );
        nodes.push(json_block);

        nodes
    }

    /// Performance tab: pipeline metrics, frame timing, CSS engine stats.
    fn template_performance(
        &self,
        doc: &Document,
        layout: &LayoutTree,
        _styles: &StyleMap,
    ) -> Vec<TemplateNode> {
        let mut nodes = Vec::new();

        nodes.push(TemplateNode::el("devtools-heading").child(TemplateNode::text("Pipeline Metrics")));

        if let Some(ref snap) = self.frame_snapshot {
            let fps_class = if snap.fps >= 55.0 { "ok" } else if snap.fps >= 25.0 { "warn" } else { "error" };
            let frame_class = if snap.avg_frame_ms < 16.7 { "ok" } else if snap.avg_frame_ms < 33.3 { "warn" } else { "error" };

            nodes.push(row_kv_class("Frame", &snap.frame_number.to_string(), "teal"));
            nodes.push(row_kv_class("FPS", &format!("{:.1}", snap.fps), fps_class));
            nodes.push(row_kv_class("Avg frame time", &format!("{:.2}ms", snap.avg_frame_ms), frame_class));
            nodes.push(row_kv_class("Viewport", &format!("{:.0}\u{00D7}{:.0}", snap.viewport_w, snap.viewport_h), "dim"));
        } else {
            nodes.push(TemplateNode::el("devtools-row")
                .child(TemplateNode::el("devtools-value").class("dim")
                    .child(TemplateNode::text("Waiting for frame data..."))));
        }

        // Frame time sparkline.
        if self.frame_times.len() > 1 {
            nodes.push(TemplateNode::el("devtools-heading").child(TemplateNode::text("Frame Times")));
            let chart = TemplateNode::el("devtools-bar-chart").children(
                self.frame_times.iter().enumerate().map(|(i, &ms)| {
                    let height_frac = (ms / 50.0).min(1.0);
                    let bar_class = if ms < 16.7 { "ok" } else if ms < 33.3 { "warn" } else { "error" };
                    TemplateNode::el("devtools-bar")
                        .key(&format!("ft-{}", i))
                        .class(bar_class)
                        .style("height", &format!("{:.0}%", height_frac * 100.0))
                }),
            );
            nodes.push(chart);
        }

        // CSS engine stats.
        if let Some(ref snap) = self.frame_snapshot {
            nodes.push(TemplateNode::el("devtools-heading").child(TemplateNode::text("CSS Engine")));
            nodes.push(row_kv("Stylesheets", &snap.stylesheet_count.to_string(), "teal"));
            nodes.push(row_kv("Rules", &snap.css_rule_count.to_string(), "teal"));
            nodes.push(row_kv("Variables", &snap.css_variable_count.to_string(), "teal"));
        }

        // DOM stats.
        nodes.push(TemplateNode::el("devtools-heading").child(TemplateNode::text("DOM Statistics")));
        nodes.push(row_kv("Node count", &doc.node_count().to_string(), "blue"));
        nodes.push(row_kv("Layout boxes", &layout.box_count().to_string(), "blue"));

        nodes
    }

    /// Mutations tab: mutation log entries with virtual scrolling.
    fn template_mutations(
        &self,
        _doc: &Document,
        _layout: &LayoutTree,
        _styles: &StyleMap,
    ) -> Vec<TemplateNode> {
        let records: Vec<_> = self.mutation_log.iter().collect();
        if records.is_empty() {
            return vec![TemplateNode::el("devtools-row")
                .child(TemplateNode::el("devtools-value").class("dim")
                    .child(TemplateNode::text("No mutations recorded")))];
        }

        let mut nodes = Vec::new();
        nodes.push(TemplateNode::el("devtools-heading")
            .child(TemplateNode::text(&format!("Mutations ({})", records.len()))));

        let row_h: f32 = 16.0;
        let (first, count) = self.visible_row_range(records.len(), row_h);

        let list = TemplateNode::el("devtools-mutations-list").children(
            records[first..first + count].iter().enumerate().map(|(i, record)| {
                let kind_class = mutation_class(&record.kind);
                TemplateNode::el("devtools-mutation-entry")
                    .key(&format!("mut-{}", first + i))
                    .class(kind_class)
                    .child(TemplateNode::text(&format_mutation_record(record)))
            }),
        );
        nodes.push(list);
        nodes
    }

    /// Scene tab: scene graph debugger + live style editor.
    fn template_scene(
        &self,
        _doc: &Document,
        _layout: &LayoutTree,
        _styles: &StyleMap,
    ) -> Vec<TemplateNode> {
        let mut nodes = Vec::new();

        // Scene graph entries.
        let entries = self.scene_debugger.entries();
        if entries.is_empty() {
            nodes.push(TemplateNode::el("devtools-row")
                .child(TemplateNode::el("devtools-value").class("dim")
                    .child(TemplateNode::text("No scene graph captured"))));
        } else {
            let selected = self.scene_debugger.selected();
            let row_h: f32 = 16.0;
            let (first, count) = self.visible_row_range(entries.len(), row_h);

            nodes.push(TemplateNode::el("devtools-heading")
                .child(TemplateNode::text(&format!("Scene Graph ({} nodes)", entries.len()))));

            let list = TemplateNode::el("devtools-scene-list").children(
                entries[first..first + count].iter().enumerate().map(|(i, entry)| {
                    TemplateNode::el("devtools-scene-entry")
                        .key(&format!("sg-{}", first + i))
                        .class_if("selected", selected == Some(first + i))
                        .style("padding-left", &format!("{}px", entry.depth * 12 + 4))
                        .child(TemplateNode::text(&entry.kind))
                }),
            );
            nodes.push(list);
        }

        // Style editor section.
        nodes.push(TemplateNode::el("devtools-separator"));
        nodes.push(TemplateNode::el("devtools-heading").child(TemplateNode::text("Live Style Editor")));

        let edits = self.style_editor.pending_edits();
        if edits.is_empty() {
            nodes.push(TemplateNode::el("devtools-row")
                .child(TemplateNode::el("devtools-value").class("dim")
                    .child(TemplateNode::text("No style edits \u{2014} type property: value"))));
        } else {
            let list = TemplateNode::el("devtools-editor-list").children(
                edits.iter().enumerate().map(|(i, edit)| {
                    TemplateNode::el("devtools-editor-entry")
                        .key(&format!("ed-{}", i))
                        .class_if("applied", edit.applied)
                        .class_if("pending", !edit.applied)
                        .child(TemplateNode::text(&format!(
                            "{} {}: {}",
                            if edit.applied { "\u{2713}" } else { "\u{25CB}" },
                            edit.property, edit.new_value
                        )))
                }),
            );
            nodes.push(list);
        }

        let input_text = self.style_editor.editing_value().to_string();
        nodes.push(TemplateNode::el("devtools-editor-input")
            .child(TemplateNode::text(&format!("> {}", input_text))));

        nodes
    }

    // ─── Scene-node overlays ─────────────────────────────────────

    /// Build the devtools panel scene nodes.
    ///
    /// Returns scene nodes to append to the root scene at high z-order.
    /// Uses scene node IDs in the 920_000+ range.
    pub fn build_scene(
        &self,
        _doc: &Document,
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

        // Selected element highlight — persistent border around the currently
        // inspected element so the user always knows what's selected.
        if self.visible {
            if let Some(sel_id) = self.selected_node {
                // Don't double-draw if hover is the same node.
                let is_hovered = self.inspector.hovered() == Some(sel_id);
                if !is_hovered {
                    if let Some(layout_box) = layout.find_by_node(sel_id) {
                        let lr = &layout_box.border_rect;
                        let rect = Rect::new(lr.x, lr.y, lr.width, lr.height);
                        nodes.push(SceneNode::new(
                            915_010,
                            SceneNodeKind::SelectionOverlay {
                                fill: Color::new(255, 152, 0, 15),    // subtle orange fill
                                border_color: Color::new(255, 152, 0, 140), // orange border
                                border_width: 1.0,
                            },
                            NodeProperties::new(rect).with_z_order(9977),
                        ));
                    }
                }
            }
        }

        if !self.visible {
            return nodes;
        }

        // The panel itself is now rendered via render_template() → CSS pipeline.
        // Only overlays (above) are direct scene nodes.

        nodes
    }

    // Old per-tab scene-node renderers removed — now using render_template() + CSS pipeline.
    // See template_elements(), template_console(), etc. above.

    // ─── Public APIs ──────────────────────────────────────────

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
            let line_h: f32 = 20.0;
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
                self.set_tab(DevToolsTab::Elements);
                self.side_tab = SideTab::Layout;
            }
            ContextAction::ShowInSceneGraph => {
                self.set_tab(DevToolsTab::Scene);
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

/// Pick a CSS class name for different mutation kinds.
fn mutation_class(kind: &MutationKind) -> &'static str {
    match kind {
        MutationKind::ChildAdded { .. } => "ok",
        MutationKind::ChildRemoved { .. } => "error",
        MutationKind::AttributeChanged { .. } => "blue",
        MutationKind::ClassChanged { .. } => "warn",
        MutationKind::TextChanged { .. } => "dim",
        MutationKind::PseudoStateChanged { .. } => "teal",
        MutationKind::IdChanged { .. } => "purple",
    }
}

/// Format a `TimingFunction` as a human-readable string.
fn format_timing_function(tf: &liquide_style_engine::computed::TimingFunction) -> String {
    use liquide_style_engine::computed::TimingFunction;
    match tf {
        TimingFunction::Linear => "linear".to_string(),
        TimingFunction::Ease => "ease".to_string(),
        TimingFunction::EaseIn => "ease-in".to_string(),
        TimingFunction::EaseOut => "ease-out".to_string(),
        TimingFunction::EaseInOut => "ease-in-out".to_string(),
        TimingFunction::CubicBezier(x1, y1, x2, y2) => {
            format!("cubic-bezier({:.2},{:.2},{:.2},{:.2})", x1, y1, x2, y2)
        }
        TimingFunction::Steps(n, pos) => {
            format!("steps({}, {:?})", n, pos)
        }
    }
}

/// Build a generic key-value row: `<devtools-row><devtools-label/><devtools-value class="$cls"/></devtools-row>`
fn row_kv(label: &str, value: &str, cls: &str) -> TemplateNode {
    TemplateNode::el("devtools-row")
        .child(
            TemplateNode::el("devtools-label")
                .child(TemplateNode::text(label)),
        )
        .child(
            TemplateNode::el("devtools-value")
                .class(cls)
                .child(TemplateNode::text(value)),
        )
}

/// Build a key-value row identical to `row_kv` — alias kept for call-site clarity
/// where the class indicates a status (ok/warn/error) rather than a colour.
fn row_kv_class(label: &str, value: &str, cls: &str) -> TemplateNode {
    row_kv(label, value, cls)
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
        assert_eq!(panel.active_tab(), DevToolsTab::Console);
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
