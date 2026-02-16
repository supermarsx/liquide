//! Element picker — click-to-select tool with hover highlighting.
//!
//! When active, the picker intercepts mouse events to highlight elements
//! under the cursor and select them on click for the style inspector.

use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::Color;
use liquide_compositor::scene::{NodeProperties, SceneNode, SceneNodeKind};
use liquide_dom::NodeId;
use liquide_hit_test::HitTestEngine;
use liquide_layout::tree::LayoutTree;

/// Convert a layout Rect to a compositor Rect.
fn to_scene_rect(r: &liquide_layout::geometry::Rect) -> Rect {
    Rect::new(r.x, r.y, r.width, r.height)
}

/// Callback invoked when an element is picked.
pub type PickCallback = Box<dyn FnMut(NodeId) + Send>;

/// The element picker: highlights elements on hover, selects on click.
pub struct ElementPicker {
    /// Whether the picker is active (intercepting mouse events).
    active: bool,
    /// Currently hovered node (for highlight overlay).
    hovered: Option<NodeId>,
    /// The node that was last picked (clicked).
    picked: Option<NodeId>,
    /// Color of the highlight border.
    highlight_color: Color,
    /// Width of the highlight border.
    highlight_width: f32,
    /// Tooltip info for the hovered element.
    tooltip_text: String,
}

impl ElementPicker {
    pub fn new() -> Self {
        Self {
            active: false,
            hovered: None,
            picked: None,
            highlight_color: Color::new(66, 133, 244, 180), // Google blue
            highlight_width: 2.0,
            tooltip_text: String::new(),
        }
    }

    /// Activate the picker (starts intercepting mouse events).
    pub fn activate(&mut self) {
        self.active = true;
        self.hovered = None;
        self.picked = None;
        self.tooltip_text.clear();
    }

    /// Deactivate the picker.
    pub fn deactivate(&mut self) {
        self.active = false;
        self.hovered = None;
        self.tooltip_text.clear();
    }

    /// Whether the picker is active.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Get the last picked (clicked) node.
    pub fn picked(&self) -> Option<NodeId> {
        self.picked
    }

    /// Get the current hover target.
    pub fn hovered(&self) -> Option<NodeId> {
        self.hovered
    }

    /// Get the tooltip text for the hovered element.
    pub fn tooltip_text(&self) -> &str {
        &self.tooltip_text
    }

    /// Process a mouse move — update hover target.
    ///
    /// Returns `true` if the hover changed (needs redraw).
    pub fn on_mouse_move(
        &mut self,
        x: f32,
        y: f32,
        hit_test: &HitTestEngine,
        doc: &liquide_dom::Document,
        layout: &LayoutTree,
    ) -> bool {
        if !self.active {
            return false;
        }

        let point = liquide_layout::geometry::Point::new(x, y);
        let new_hover = hit_test.hit_test(point).map(|r| r.node);

        if new_hover == self.hovered {
            return false;
        }

        self.hovered = new_hover;

        // Build tooltip text.
        self.tooltip_text = match self.hovered {
            Some(node_id) => {
                let mut parts = Vec::new();

                if let Some(node) = doc.get(node_id) {
                    parts.push(node.tag.as_str().to_string());

                    if let Some(ref eid) = node.element_id {
                        parts.push(format!("#{eid}"));
                    }
                    for class in node.classes.iter() {
                        parts.push(format!(".{class}"));
                    }
                }

                if let Some(lb) = layout.find_by_node(node_id) {
                    let r = to_scene_rect(&lb.border_rect);
                    parts.push(format!("  {:.0}×{:.0}", r.width, r.height));
                }

                parts.join("")
            }
            None => String::new(),
        };

        true
    }

    /// Process a mouse click — pick the hovered element.
    ///
    /// Returns `Some(node_id)` if an element was picked, `None` otherwise.
    /// Automatically deactivates the picker after a successful pick.
    pub fn on_click(&mut self) -> Option<NodeId> {
        if !self.active {
            return None;
        }

        if let Some(node_id) = self.hovered {
            self.picked = Some(node_id);
            self.deactivate();
            return Some(node_id);
        }

        None
    }

    /// Build highlight overlay scene nodes for the hovered element.
    ///
    /// Returns scene nodes to append at the highest z-order.
    /// Uses scene node IDs in the 910_000 range.
    pub fn build_highlight(
        &self,
        layout: &LayoutTree,
        screen_width: f32,
        screen_height: f32,
    ) -> Vec<SceneNode> {
        if !self.active {
            return vec![];
        }

        let node_id = match self.hovered {
            Some(id) => id,
            None => return vec![],
        };

        let layout_box = match layout.find_by_node(node_id) {
            Some(b) => b,
            None => return vec![],
        };

        let rect = to_scene_rect(&layout_box.border_rect);
        let w = self.highlight_width;
        let color = self.highlight_color;
        let mut nodes = Vec::with_capacity(6);
        let base_id: u64 = 910_000;

        // Semi-transparent fill.
        let fill_color = Color::new(color.r, color.g, color.b, 30);
        nodes.push(SceneNode::new(
            base_id,
            SceneNodeKind::Background { color: fill_color },
            NodeProperties::new(rect).with_z_order(9980),
        ));

        // Top border.
        nodes.push(SceneNode::new(
            base_id + 1,
            SceneNodeKind::Background { color },
            NodeProperties::new(Rect::new(rect.x, rect.y, rect.width, w)).with_z_order(9981),
        ));
        // Bottom border.
        nodes.push(SceneNode::new(
            base_id + 2,
            SceneNodeKind::Background { color },
            NodeProperties::new(Rect::new(
                rect.x,
                rect.y + rect.height - w,
                rect.width,
                w,
            ))
            .with_z_order(9981),
        ));
        // Left border.
        nodes.push(SceneNode::new(
            base_id + 3,
            SceneNodeKind::Background { color },
            NodeProperties::new(Rect::new(rect.x, rect.y, w, rect.height)).with_z_order(9981),
        ));
        // Right border.
        nodes.push(SceneNode::new(
            base_id + 4,
            SceneNodeKind::Background { color },
            NodeProperties::new(Rect::new(
                rect.x + rect.width - w,
                rect.y,
                w,
                rect.height,
            ))
            .with_z_order(9981),
        ));

        // Tooltip with element info.
        if !self.tooltip_text.is_empty() {
            let tip_w = (self.tooltip_text.len() as f32 * 7.0 + 16.0).min(400.0);
            let tip_h = 22.0;
            let tip_x = (rect.x).clamp(0.0, screen_width - tip_w);
            let tip_y = if rect.y > tip_h + 6.0 {
                rect.y - tip_h - 4.0
            } else {
                rect.y + rect.height + 4.0
            }
            .clamp(0.0, screen_height - tip_h);

            // Tooltip background.
            nodes.push(SceneNode::new(
                base_id + 5,
                SceneNodeKind::Background {
                    color: Color::new(30, 30, 30, 230),
                },
                NodeProperties::new(Rect::new(tip_x, tip_y, tip_w, tip_h)).with_z_order(9998),
            ));

            // Tooltip text.
            nodes.push(SceneNode::new(
                base_id + 6,
                SceneNodeKind::Text {
                    text: self.tooltip_text.clone(),
                    color: Color::new(220, 220, 220, 255),
                    scale: 1,
                    font_family: "Inter".to_string(),
                    font_size: 11.0,
                    font_weight: 400,
                    font_style_italic: false,
                    letter_spacing: 0.0,
                    word_spacing: 0.0,
                    line_height: tip_h - 6.0,
                    text_align: 0,
                    text_transform: 0,
                    text_overflow: 0,
                    white_space: 1,
                    text_indent: 0.0,
                    text_decoration: None,
                    text_shadows: vec![],
                },
                NodeProperties::new(Rect::new(
                    tip_x + 8.0,
                    tip_y + 3.0,
                    tip_w - 16.0,
                    tip_h - 6.0,
                ))
                .with_z_order(9999),
            ));
        }

        nodes
    }
}

impl Default for ElementPicker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_picker_lifecycle() {
        let mut picker = ElementPicker::new();
        assert!(!picker.is_active());

        picker.activate();
        assert!(picker.is_active());
        assert_eq!(picker.hovered(), None);
        assert_eq!(picker.picked(), None);

        picker.deactivate();
        assert!(!picker.is_active());
    }

    #[test]
    fn test_click_without_hover() {
        let mut picker = ElementPicker::new();
        picker.activate();
        assert_eq!(picker.on_click(), None);
        assert!(picker.is_active()); // stays active if nothing was hovered
    }
}
