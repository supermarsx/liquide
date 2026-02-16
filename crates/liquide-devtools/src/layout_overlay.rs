//! Layout box overlay — draws margin/padding/border/content box guides
//! on the compositor output for the selected/hovered element.

use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::Color;
use liquide_compositor::scene::{NodeProperties, SceneNode, SceneNodeKind};
use liquide_dom::NodeId;
use liquide_layout::tree::LayoutTree;
use liquide_style_engine::StyleMap;

/// Convert a layout Rect to a compositor Rect (same fields, different types).
fn to_scene_rect(r: &liquide_layout::geometry::Rect) -> Rect {
    Rect::new(r.x, r.y, r.width, r.height)
}

/// Colors for the box model overlay (matching Chromium DevTools).
pub struct BoxModelColors {
    pub content: Color,
    pub padding: Color,
    pub border: Color,
    pub margin: Color,
}

impl Default for BoxModelColors {
    fn default() -> Self {
        Self {
            content: Color::new(111, 168, 220, 100),  // blue
            padding: Color::new(147, 196, 125, 80),   // green
            border: Color::new(255, 229, 153, 80),    // yellow
            margin: Color::new(246, 178, 107, 60),    // orange
        }
    }
}

/// The layout overlay: generates scene nodes that visualize the box model
/// for a selected element.
pub struct LayoutOverlay {
    /// Whether the overlay is visible.
    enabled: bool,
    /// The node to highlight.
    target: Option<NodeId>,
    /// Colors for the box model regions.
    colors: BoxModelColors,
    /// Whether to show dimension labels.
    show_labels: bool,
    /// Whether to show guide lines extending to edges.
    show_guides: bool,
}

impl LayoutOverlay {
    pub fn new() -> Self {
        Self {
            enabled: false,
            target: None,
            colors: BoxModelColors::default(),
            show_labels: true,
            show_guides: true,
        }
    }

    /// Enable or disable the overlay.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Set the element to highlight.
    pub fn set_target(&mut self, target: Option<NodeId>) {
        self.target = target;
        if target.is_some() {
            self.enabled = true;
        }
    }

    pub fn target(&self) -> Option<NodeId> {
        self.target
    }

    /// Toggle label display.
    pub fn set_show_labels(&mut self, show: bool) {
        self.show_labels = show;
    }

    /// Toggle guide line display.
    pub fn set_show_guides(&mut self, show: bool) {
        self.show_guides = show;
    }

    /// Generate overlay scene nodes for the current target.
    ///
    /// Returns a list of scene nodes to be appended to the root scene
    /// at the highest z-order. Uses scene node IDs in the 900_000 range.
    pub fn build_overlay(
        &self,
        layout: &LayoutTree,
        styles: &StyleMap,
        screen_width: f32,
        screen_height: f32,
    ) -> Vec<SceneNode> {
        if !self.enabled {
            return vec![];
        }

        let node_id = match self.target {
            Some(id) => id,
            None => return vec![],
        };

        let layout_box = match layout.find_by_node(node_id) {
            Some(b) => b,
            None => return vec![],
        };
        let box_id = layout_box.id;

        let _style = styles.get(node_id);

        // Extract box model dimensions using absolute rects (local → screen coords).
        let content_rect = to_scene_rect(&layout.absolute_content_rect(box_id));
        let padding_rect = to_scene_rect(&layout.absolute_padding_rect(box_id));
        let border_rect = to_scene_rect(&layout.absolute_border_rect(box_id));
        let margin_rect = to_scene_rect(&layout.absolute_margin_rect(box_id));

        let mut nodes = Vec::with_capacity(12);
        let mut next_id: u64 = 900_000;

        // ── Margin region (orange) — fill between margin and border rects ──
        self.add_frame_nodes(
            &mut nodes,
            &mut next_id,
            &margin_rect,
            &border_rect,
            self.colors.margin,
        );

        // ── Border region (yellow) — fill between border and padding rects ──
        self.add_frame_nodes(
            &mut nodes,
            &mut next_id,
            &border_rect,
            &padding_rect,
            self.colors.border,
        );

        // ── Padding region (green) — fill between padding and content rects ──
        self.add_frame_nodes(
            &mut nodes,
            &mut next_id,
            &padding_rect,
            &content_rect,
            self.colors.padding,
        );

        // ── Content region (blue) ──
        nodes.push(SceneNode::new(
            next_id,
            SceneNodeKind::Background {
                color: self.colors.content,
            },
            NodeProperties::new(content_rect).with_z_order(9990),
        ));
        next_id += 1;

        // ── Dimension label ──
        if self.show_labels {
            let label = format!(
                "{:.0} × {:.0}",
                content_rect.width, content_rect.height
            );
            let label_w = label.len() as f32 * 7.0 + 12.0;
            let label_h = 18.0;
            let label_x = (content_rect.x + content_rect.width / 2.0 - label_w / 2.0)
                .clamp(0.0, screen_width - label_w);
            let label_y = if margin_rect.y > label_h + 4.0 {
                margin_rect.y - label_h - 4.0
            } else {
                margin_rect.y + margin_rect.height + 4.0
            }
            .clamp(0.0, screen_height - label_h);

            // Label background.
            nodes.push(SceneNode::new(
                next_id,
                SceneNodeKind::Background {
                    color: Color::new(40, 40, 40, 220),
                },
                NodeProperties::new(Rect::new(label_x, label_y, label_w, label_h))
                    .with_z_order(9995),
            ));
            next_id += 1;

            // Label text.
            nodes.push(SceneNode::new(
                next_id,
                SceneNodeKind::Text {
                    text: label,
                    color: Color::new(255, 255, 255, 255),
                    scale: 1,
                    font_family: "Inter".to_string(),
                    font_size: 11.0,
                    font_weight: 400,
                    font_style_italic: false,
                    letter_spacing: 0.0,
                    word_spacing: 0.0,
                    line_height: label_h - 4.0,
                    text_align: 0,
                    text_transform: 0,
                    text_overflow: 0,
                    white_space: 1,
                    text_indent: 0.0,
                    text_decoration: None,
                    text_shadows: vec![],
                },
                NodeProperties::new(Rect::new(
                    label_x + 6.0,
                    label_y + 2.0,
                    label_w - 12.0,
                    label_h - 4.0,
                ))
                .with_z_order(9996),
            ));
            next_id += 1;
        }

        // ── Guide lines ──
        if self.show_guides {
            let guide_color = Color::new(255, 100, 100, 120);

            // Horizontal guides at top and bottom of margin box.
            nodes.push(SceneNode::new(
                next_id,
                SceneNodeKind::Background {
                    color: guide_color,
                },
                NodeProperties::new(Rect::new(0.0, margin_rect.y, screen_width, 1.0))
                    .with_z_order(9985),
            ));
            next_id += 1;

            nodes.push(SceneNode::new(
                next_id,
                SceneNodeKind::Background {
                    color: guide_color,
                },
                NodeProperties::new(Rect::new(
                    0.0,
                    margin_rect.y + margin_rect.height,
                    screen_width,
                    1.0,
                ))
                .with_z_order(9985),
            ));
            next_id += 1;

            // Vertical guides at left and right.
            nodes.push(SceneNode::new(
                next_id,
                SceneNodeKind::Background {
                    color: guide_color,
                },
                NodeProperties::new(Rect::new(margin_rect.x, 0.0, 1.0, screen_height))
                    .with_z_order(9985),
            ));
            next_id += 1;

            nodes.push(SceneNode::new(
                next_id,
                SceneNodeKind::Background {
                    color: guide_color,
                },
                NodeProperties::new(Rect::new(
                    margin_rect.x + margin_rect.width,
                    0.0,
                    1.0,
                    screen_height,
                ))
                .with_z_order(9985),
            ));
        }

        nodes
    }

    /// Add 4 rectangles forming a frame between outer and inner rects.
    fn add_frame_nodes(
        &self,
        nodes: &mut Vec<SceneNode>,
        next_id: &mut u64,
        outer: &Rect,
        inner: &Rect,
        color: Color,
    ) {
        // Top strip.
        if inner.y > outer.y {
            nodes.push(SceneNode::new(
                *next_id,
                SceneNodeKind::Background { color },
                NodeProperties::new(Rect::new(
                    outer.x,
                    outer.y,
                    outer.width,
                    inner.y - outer.y,
                ))
                .with_z_order(9988),
            ));
            *next_id += 1;
        }
        // Bottom strip.
        let inner_bottom = inner.y + inner.height;
        let outer_bottom = outer.y + outer.height;
        if outer_bottom > inner_bottom {
            nodes.push(SceneNode::new(
                *next_id,
                SceneNodeKind::Background { color },
                NodeProperties::new(Rect::new(
                    outer.x,
                    inner_bottom,
                    outer.width,
                    outer_bottom - inner_bottom,
                ))
                .with_z_order(9988),
            ));
            *next_id += 1;
        }
        // Left strip (between top and bottom strips).
        if inner.x > outer.x {
            nodes.push(SceneNode::new(
                *next_id,
                SceneNodeKind::Background { color },
                NodeProperties::new(Rect::new(
                    outer.x,
                    inner.y,
                    inner.x - outer.x,
                    inner.height,
                ))
                .with_z_order(9988),
            ));
            *next_id += 1;
        }
        // Right strip.
        let inner_right = inner.x + inner.width;
        let outer_right = outer.x + outer.width;
        if outer_right > inner_right {
            nodes.push(SceneNode::new(
                *next_id,
                SceneNodeKind::Background { color },
                NodeProperties::new(Rect::new(
                    inner_right,
                    inner.y,
                    outer_right - inner_right,
                    inner.height,
                ))
                .with_z_order(9988),
            ));
            *next_id += 1;
        }
    }
}

impl Default for LayoutOverlay {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disabled_returns_empty() {
        let overlay = LayoutOverlay::new();
        let layout = LayoutTree::new();
        let styles = StyleMap::new();
        assert!(overlay.build_overlay(&layout, &styles, 1920.0, 1080.0).is_empty());
    }

    #[test]
    fn test_default_colors() {
        let colors = BoxModelColors::default();
        assert!(colors.content.a > 0);
        assert!(colors.padding.a > 0);
        assert!(colors.border.a > 0);
        assert!(colors.margin.a > 0);
    }
}
