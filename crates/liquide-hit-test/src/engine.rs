//! Hit test engine — finds which DOM node is under a point.

use liquide_dom::NodeId;
use liquide_layout::geometry::Point;
use liquide_layout::tree::{LayoutBoxId, LayoutTree};
use liquide_style_engine::computed::PointerEvents;
use liquide_style_engine::StyleMap;

/// Result of a hit test.
#[derive(Debug, Clone)]
pub struct HitTestResult {
    /// The target DOM node.
    pub node: NodeId,
    /// The point relative to the node's content box.
    pub point_in_node: Point,
    /// Ancestor chain from target up to root (bubble path).
    pub ancestors: Vec<NodeId>,
}

/// The hit test engine.
pub struct HitTestEngine {
    /// Cached layout tree reference.
    layout: LayoutTree,
    /// Cached style map.
    styles: StyleMap,
}

impl HitTestEngine {
    /// Create a new hit test engine.
    pub fn new(layout: LayoutTree, styles: StyleMap) -> Self {
        Self { layout, styles }
    }

    /// Update with new layout/styles (after relayout).
    pub fn update(&mut self, layout: LayoutTree, styles: StyleMap) {
        self.layout = layout;
        self.styles = styles;
    }

    /// Get a reference to the layout tree.
    pub fn layout(&self) -> &LayoutTree {
        &self.layout
    }

    /// Get a reference to the style map.
    pub fn styles(&self) -> &StyleMap {
        &self.styles
    }

    /// Hit test a single point. Returns the topmost matching node.
    pub fn hit_test(&self, point: Point) -> Option<HitTestResult> {
        self.hit_test_box(self.layout.root, point, (0.0, 0.0))
    }

    /// Hit test all overlapping nodes at a point (front to back).
    pub fn hit_test_all(&self, point: Point) -> Vec<HitTestResult> {
        let mut results = Vec::new();
        self.hit_test_box_all(self.layout.root, point, (0.0, 0.0), &mut results);
        results
    }

    fn hit_test_box(
        &self,
        box_id: LayoutBoxId,
        point: Point,
        paint_offset: (f32, f32),
    ) -> Option<HitTestResult> {
        let layout_box = self.layout.get(box_id)?;
        let (ox, oy) = paint_offset;

        // Compute absolute border rect for containment check
        let abs_border = layout_box.border_rect.offset(ox, oy);

        // Check if point is within the absolute border box
        if !abs_border.contains(point) {
            return None;
        }

        // Check pointer-events
        if let Some(style) = self.styles.get(layout_box.node) {
            if style.pointer_events == PointerEvents::None {
                return None;
            }
        }

        // Child paint offset = parent offset + parent content area origin
        let child_offset = (
            ox + layout_box.content_rect.x,
            oy + layout_box.content_rect.y,
        );

        // Test children in reverse order (topmost first, z-order)
        let children = layout_box.children.clone();
        for &child_id in children.iter().rev() {
            if let Some(result) = self.hit_test_box(child_id, point, child_offset) {
                return Some(result);
            }
        }

        // No child matched — this box is the target
        let abs_content = layout_box.content_rect.offset(ox, oy);
        let point_in_node = Point::new(
            point.x - abs_content.x,
            point.y - abs_content.y,
        );

        // Build ancestor chain
        let mut ancestors = Vec::new();
        let mut current = box_id;
        // Walk up by finding parent boxes
        for b in &self.layout.boxes {
            if b.children.contains(&current) {
                ancestors.push(b.node);
                current = b.id;
            }
        }

        Some(HitTestResult {
            node: layout_box.node,
            point_in_node,
            ancestors,
        })
    }

    fn hit_test_box_all(
        &self,
        box_id: LayoutBoxId,
        point: Point,
        paint_offset: (f32, f32),
        results: &mut Vec<HitTestResult>,
    ) {
        let layout_box = match self.layout.get(box_id) {
            Some(b) => b,
            None => return,
        };
        let (ox, oy) = paint_offset;

        let abs_border = layout_box.border_rect.offset(ox, oy);

        if !abs_border.contains(point) {
            return;
        }

        if let Some(style) = self.styles.get(layout_box.node) {
            if style.pointer_events == PointerEvents::None {
                return;
            }
        }

        // Add this box with absolute point-in-node
        let abs_content = layout_box.content_rect.offset(ox, oy);
        let point_in_node = Point::new(
            point.x - abs_content.x,
            point.y - abs_content.y,
        );
        results.push(HitTestResult {
            node: layout_box.node,
            point_in_node,
            ancestors: Vec::new(), // simplified for all-results mode
        });

        // Recurse children with accumulated offset
        let child_offset = (
            ox + layout_box.content_rect.x,
            oy + layout_box.content_rect.y,
        );
        let children = layout_box.children.clone();
        for &child_id in children.iter().rev() {
            self.hit_test_box_all(child_id, point, child_offset, results);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_dom::Document;
    use liquide_layout::{DefaultImageMeasurer, DefaultTextMeasurer, LayoutEngine, Size};
    use liquide_style_engine::engine::{StyleEngine, ViewportSize};

    #[test]
    fn basic_hit_test() {
        let mut doc = Document::new();
        let root = doc.root();
        let div = doc.create_element("div");
        doc.append_child(root, div);

        let mut se = StyleEngine::default();
        se.add_stylesheet("div { width: 200px; height: 100px; }");

        let style_map = se.restyle_all(&doc);
        let mut le = LayoutEngine::new(Size::new(1920.0, 1080.0), 16.0);
        let layout_tree = le.layout(&doc, &style_map, &DefaultTextMeasurer, &DefaultImageMeasurer);

        let engine = HitTestEngine::new(layout_tree, style_map);
        let result = engine.hit_test(Point::new(100.0, 50.0));

        assert!(result.is_some(), "Should hit something within the viewport");
    }
}
