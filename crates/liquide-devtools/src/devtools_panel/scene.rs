//! Scene-node overlay generation for the DevTools panel.
//!
//! Builds compositor scene nodes for element picker highlights, hover
//! highlights, and selection overlays that render on top of the page viewport.

use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::Color;
use liquide_compositor::scene::{NodeProperties, SceneNode, SceneNodeKind};
use liquide_dom::Document;
use liquide_layout::tree::LayoutTree;
use liquide_style_engine::StyleMap;

use super::{DevToolsPanel, DevToolsTab};

impl DevToolsPanel {
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
        let overlay_nodes = self.layout_overlay.build_overlay(
            layout,
            styles,
            self.screen_width,
            self.screen_height,
        );
        nodes.extend(overlay_nodes);

        // Element picker highlight.
        let picker_nodes =
            self.element_picker
                .build_highlight(layout, self.screen_width, self.screen_height);
        nodes.extend(picker_nodes);

        // Hover highlight — when the user hovers over a node in the Elements
        // tree, draw a SelectionOverlay on the viewport at that element's
        // layout bounds so they can see what they're about to select.
        if self.visible && self.active_tab == DevToolsTab::Elements {
            if let Some(hovered_id) = self.inspector.hovered() {
                if let Some(layout_box) = layout.find_by_node(hovered_id) {
                    let lr = layout.absolute_border_rect(layout_box.id);
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
                        let lr = layout.absolute_border_rect(layout_box.id);
                        let rect = Rect::new(lr.x, lr.y, lr.width, lr.height);
                        nodes.push(SceneNode::new(
                            915_010,
                            SceneNodeKind::SelectionOverlay {
                                fill: Color::new(255, 152, 0, 15),          // subtle orange fill
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
}
