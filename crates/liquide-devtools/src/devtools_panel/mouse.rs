//! Mouse, scroll, and click event handling for the DevTools panel.

use liquide_dom::{Document, NodeId};
use liquide_hit_test::HitTestEngine;
use liquide_layout::tree::LayoutTree;
use liquide_style_engine::StyleMap;

use crate::context_menu::ContextAction;

use super::{DevToolsPanel, DevToolsTab, DockPosition, SideTab};

impl DevToolsPanel {
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
            let tab_bar_h = 30.0;
            let content_y = bounds.y + 1.0 + tab_bar_h + 1.0 + 8.0;

            if x >= bounds.x
                && x <= bounds.x + bounds.width
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
        if self
            .element_picker
            .on_mouse_move(x, y, hit_test, doc, layout)
        {
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

    /// Handle a click inside the panel using the hit-test engine.
    ///
    /// This uses the internal hit-test mechanism to find which element was
    /// clicked, then walks up the ancestor chain looking for data attributes
    /// that indicate actionable elements.
    ///
    /// Returns `true` if the click was consumed.
    pub fn on_panel_click(
        &mut self,
        x: f32,
        y: f32,
        styles: &StyleMap,
        doc: &Document,
        hit_test: &HitTestEngine,
    ) -> bool {
        if !self.visible {
            return false;
        }

        // If context menu is visible, left-click should either activate
        // a menu item or dismiss the menu.
        if self.context_menu.is_visible() {
            if let Some((action, node_id)) = self.context_menu.on_click(x, y) {
                self.handle_context_action(action, node_id, styles);
                return true;
            }
            self.context_menu.hide();
            return true;
        }

        let bounds = self.panel_bounds();

        // Check bounds.
        if x < bounds.x
            || x > bounds.x + bounds.width
            || y < bounds.y
            || y > bounds.y + bounds.height
        {
            return false;
        }

        // ── Hit-test and attribute-based dispatch ──
        let point = liquide_layout::geometry::Point::new(x, y);
        #[cfg(debug_assertions)]
        eprintln!(
            "[devtools] on_panel_click({}, {}) inside bounds {:?}",
            x, y, bounds
        );
        if let Some(result) = hit_test.hit_test(point) {
            #[cfg(debug_assertions)]
            eprintln!(
                "[devtools]   -> hit node {:?}, bounds {:?}, ancestors: {:?}",
                result.node, result.bounds, result.ancestors
            );
            // Walk up from the hit node through all ancestors, checking for
            // actionable data attributes. This works regardless of whether
            // we hit a text node, icon, or the element itself.
            for node_id in result.node_and_ancestors() {
                #[cfg(debug_assertions)]
                {
                    let tag = doc.tag_name(node_id).unwrap_or_default();
                    let has_tab = doc.get_attribute(node_id, "data-tab").is_some();
                    let has_action = doc.get_attribute(node_id, "data-action").is_some();
                    eprintln!(
                        "[devtools]     checking {:?} tag={:?} has_tab={} has_action={}",
                        node_id, tag, has_tab, has_action
                    );
                }
                // Check for main tab (data-tab attribute)
                if let Some(tab_id) = doc.get_attribute(node_id, "data-tab") {
                    if let Some(t) = Self::parse_tab_id(&tab_id) {
                        self.set_tab(t);
                        return true;
                    }
                }

                // Check for toolbar button (data-action attribute)
                if let Some(action) = doc.get_attribute(node_id, "data-action") {
                    return self.handle_btn_action(&action);
                }

                // Check for side tab (data-sidetab attribute)
                if let Some(side_id) = doc.get_attribute(node_id, "data-sidetab") {
                    if let Some(st) = Self::parse_sidetab_id(&side_id) {
                        self.side_tab = st;
                        return true;
                    }
                }

                // Check for tree node (data-node attribute)
                // Arrow elements also have data-tree-arrow to distinguish them
                if let Some(node_id_str) = doc.get_attribute(node_id, "data-node") {
                    if let Ok(target_id) = node_id_str.parse::<NodeId>() {
                        // Check if this is an arrow element
                        if doc.get_attribute(node_id, "data-tree-arrow").is_some() {
                            self.inspector.toggle_expand(target_id);
                            return true;
                        }
                        // Otherwise it's a row/node click - select the node
                        self.select_node(target_id, styles);
                        return true;
                    }
                }

                // Check for scene row (data-scene-idx attribute)
                if let Some(idx_str) = doc.get_attribute(node_id, "data-scene-idx") {
                    if let Ok(idx) = idx_str.parse::<usize>() {
                        self.scene_debugger.select(Some(idx));
                        return true;
                    }
                }

                // Check for style category (data-style-category attribute)
                if let Some(cat_str) = doc.get_attribute(node_id, "data-style-category") {
                    if let Some(cat) = crate::style_panel::StyleCategory::from_id(&cat_str) {
                        self.style_inspector.toggle_category(cat);
                        return true;
                    }
                }

                // Check for style property editing (data-style-prop attribute)
                if let Some(prop_name) = doc.get_attribute(node_id, "data-style-prop") {
                    if let Some(prop_value) = doc.get_attribute(node_id, "data-style-value") {
                        if let Some(selected) = self.selected_node {
                            self.style_editor.set_target(Some(selected));
                        }
                        self.style_editor.start_edit(&prop_name, &prop_value);
                        return true;
                    }
                }
            }

            // Fallback: check for console focus
            if self.active_tab == DevToolsTab::Console {
                self.console_focused = true;
                return true;
            }
        } else {
            #[cfg(debug_assertions)]
            eprintln!(
                "[devtools]   -> hit_test returned None for point ({}, {})",
                x, y
            );
        }

        // Click inside panel always consumed.
        true
    }

    /// Parse a tab ID string to DevToolsTab.
    fn parse_tab_id(id: &str) -> Option<DevToolsTab> {
        match id {
            "elements" => Some(DevToolsTab::Elements),
            "console" => Some(DevToolsTab::Console),
            "sources" => Some(DevToolsTab::Sources),
            "perf" => Some(DevToolsTab::Performance),
            "mutations" => Some(DevToolsTab::Mutations),
            "scene" => Some(DevToolsTab::Scene),
            _ => None,
        }
    }

    /// Parse a side tab ID string to SideTab.
    fn parse_sidetab_id(id: &str) -> Option<SideTab> {
        match id {
            "styles" => Some(SideTab::Styles),
            "layout" => Some(SideTab::Layout),
            "computed" => Some(SideTab::Computed),
            "fonts" => Some(SideTab::Fonts),
            "animations" => Some(SideTab::Animations),
            _ => None,
        }
    }

    /// Handle a devtools button action.
    fn handle_btn_action(&mut self, action: &str) -> bool {
        match action {
            "picker" => {
                self.toggle_picker();
                true
            }
            "detach" => {
                self.toggle_detach();
                true
            }
            "dock-bottom" => {
                self.config.dock_position = DockPosition::Bottom;
                true
            }
            "dock-right" => {
                self.config.dock_position = DockPosition::Right;
                true
            }
            "close" => {
                self.hide();
                true
            }
            _ => false,
        }
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
        if x < bounds.x
            || x > bounds.x + bounds.width
            || y < bounds.y
            || y > bounds.y + bounds.height
        {
            return false;
        }

        let tab_bar_h = 30.0;
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
        if x >= bounds.x
            && x <= bounds.x + bounds.width
            && y >= bounds.y
            && y <= bounds.y + bounds.height
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
        if x < bounds.x
            || x > bounds.x + bounds.width
            || y < bounds.y
            || y > bounds.y + bounds.height
        {
            return false;
        }

        // Only show context menu in Elements tab on a node line.
        if self.active_tab == DevToolsTab::Elements {
            let tab_bar_h = 30.0;
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
                self.console
                    .push_output(format!("Logged node #{}", node_id));
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
}
