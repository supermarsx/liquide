//! DevTools panel — the top-level container that composes all sub-panels
//! into a docked/floating developer tools window.
//!
//! The panel is designed to be rendered as an overlay on top of the
//! compositor scene. It handles tab switching, keyboard shortcuts,
//! and coordinates the inspector, style panel, layout overlay, element
//! picker, mutation log, and DOM serializer.

use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::Color;
use liquide_compositor::scene::{NodeProperties, SceneNode, SceneNodeKind};
use liquide_dom::{Document, NodeId};
use liquide_hit_test::HitTestEngine;
use liquide_layout::tree::LayoutTree;
use liquide_style_engine::StyleMap;

use crate::dom_serializer::DomSerializer;
use crate::element_picker::ElementPicker;
use crate::inspector::ElementTreeInspector;
use crate::layout_overlay::LayoutOverlay;
use crate::mutation_log::MutationLog;
use crate::style_panel::StyleInspector;

/// Which tab is currently active in the devtools panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DevToolsTab {
    /// Element tree + styles (like Chrome's "Elements" tab).
    Elements,
    /// Computed style properties.
    Styles,
    /// Layout box model visualization.
    Layout,
    /// DOM mutation log.
    Mutations,
    /// DOM tree JSON export.
    DomTree,
}

impl DevToolsTab {
    /// All available tabs in order.
    pub const ALL: &'static [DevToolsTab] = &[
        DevToolsTab::Elements,
        DevToolsTab::Styles,
        DevToolsTab::Layout,
        DevToolsTab::Mutations,
        DevToolsTab::DomTree,
    ];

    /// Human-readable label for the tab.
    pub fn label(&self) -> &'static str {
        match self {
            DevToolsTab::Elements => "Elements",
            DevToolsTab::Styles => "Styles",
            DevToolsTab::Layout => "Layout",
            DevToolsTab::Mutations => "Mutations",
            DevToolsTab::DomTree => "DOM",
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
    /// Currently selected node (shared across panels).
    selected_node: Option<NodeId>,
    /// Screen dimensions for layout calculations.
    screen_width: f32,
    screen_height: f32,
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
            selected_node: None,
            screen_width: 1920.0,
            screen_height: 1080.0,
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
        match key {
            "F12" => {
                self.toggle();
                true
            }
            "I" | "i" if ctrl && shift => {
                self.toggle();
                true
            }
            "C" | "c" if ctrl && shift => {
                if !self.visible {
                    self.show();
                }
                self.toggle_picker();
                true
            }
            "Tab" if self.visible && !ctrl && !shift => {
                self.next_tab();
                true
            }
            _ => false,
        }
    }

    // ─── Mouse event forwarding ───────────────────────────────

    /// Forward mouse move to the element picker (when active).
    ///
    /// Returns `true` if the hover state changed.
    pub fn on_mouse_move(
        &mut self,
        x: f32,
        y: f32,
        hit_test: &HitTestEngine,
        doc: &Document,
        layout: &LayoutTree,
    ) -> bool {
        self.element_picker.on_mouse_move(x, y, hit_test, doc, layout)
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
        }
    }

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
                    font_family: String::new(),
                    font_size: 12.0,
                    font_weight: if is_active { 600 } else { 400 },
                    font_style_italic: false,
                    letter_spacing: 0.0,
                    word_spacing: 0.0,
                    line_height: tab_bar_h,
                    text_align: 0,
                    text_transform: 0,
                    text_overflow: 0,
                    white_space: 1, // nowrap
                    text_indent: 0.0,
                    text_decoration: None,
                    text_shadows: vec![],
                },
                NodeProperties::new(Rect::new(
                    tab_x + 8.0,
                    bounds.y + 1.0,
                    tab_w - 16.0,
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

        // Content area info text (placeholder — actual content is rendered
        // by per-tab renderers which are compositor-level integrations).
        let content_area = Rect::new(
            bounds.x + 8.0,
            content_y + 8.0,
            bounds.width - 16.0,
            bounds.height - tab_bar_h - 10.0,
        );

        let info_text = match self.active_tab {
            DevToolsTab::Elements => {
                let count = self.inspector.visible_nodes().len();
                format!("Element Tree — {} visible nodes", count)
            }
            DevToolsTab::Styles => match self.selected_node {
                Some(id) => format!("Computed Styles — node #{}", id),
                None => "Select an element to view styles".to_string(),
            },
            DevToolsTab::Layout => match self.selected_node {
                Some(id) => format!("Box Model — node #{}", id),
                None => "Select an element to view layout".to_string(),
            },
            DevToolsTab::Mutations => {
                let count = self.mutation_log.len();
                format!(
                    "Mutation Log — {} records ({} total)",
                    count,
                    self.mutation_log.total_count()
                )
            }
            DevToolsTab::DomTree => "DOM Tree — JSON Export".to_string(),
        };

        nodes.push(SceneNode::new(
            base_id + 40,
            SceneNodeKind::Text {
                text: info_text,
                color: self.config.text_color,
                scale: 1,
                font_family: String::new(),
                font_size: self.config.font_size,
                font_weight: 400,
                font_style_italic: false,
                letter_spacing: 0.0,
                word_spacing: 0.0,
                line_height: 20.0,
                text_align: 0,
                text_transform: 0,
                text_overflow: 1, // ellipsis
                white_space: 1, // nowrap
                text_indent: 0.0,
                text_decoration: None,
                text_shadows: vec![],
            },
            NodeProperties::new(content_area).with_z_order(9906),
        ));

        // If on Elements tab and we have a selected node, show brief node info.
        if self.active_tab == DevToolsTab::Elements {
            if let Some(node_id) = self.selected_node {
                let selected_info = format!("Selected: node #{}", node_id);
                nodes.push(SceneNode::new(
                    base_id + 50,
                    SceneNodeKind::Text {
                        text: selected_info,
                        color: Color::new(86, 156, 214, 255), // VS Code keyword blue
                        scale: 1,
                        font_family: String::new(),
                        font_size: 11.0,
                        font_weight: 400,
                        font_style_italic: false,
                        letter_spacing: 0.0,
                        word_spacing: 0.0,
                        line_height: 18.0,
                        text_align: 0,
                        text_transform: 0,
                        text_overflow: 0,
                        white_space: 1,
                        text_indent: 0.0,
                        text_decoration: None,
                        text_shadows: vec![],
                    },
                    NodeProperties::new(Rect::new(
                        content_area.x,
                        content_area.y + 24.0,
                        content_area.width,
                        18.0,
                    ))
                    .with_z_order(9906),
                ));
            }
        }

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
                font_family: String::new(),
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

        nodes
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
