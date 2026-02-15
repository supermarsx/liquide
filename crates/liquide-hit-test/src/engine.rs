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

    /// Hit test a single point. Returns the topmost matching node.
    pub fn hit_test(&self, point: Point) -> Option<HitTestResult> {
        self.hit_test_box(self.layout.root, point)
    }

    /// Hit test all overlapping nodes at a point (front to back).
    pub fn hit_test_all(&self, point: Point) -> Vec<HitTestResult> {
        let mut results = Vec::new();
        self.hit_test_box_all(self.layout.root, point, &mut results);
        results
    }

    fn hit_test_box(&self, box_id: LayoutBoxId, point: Point) -> Option<HitTestResult> {
        let layout_box = self.layout.get(box_id)?;

        // Check if point is within the border box
        if !layout_box.border_rect.contains(point) {
            return None;
        }

        // Check pointer-events
        if let Some(style) = self.styles.get(layout_box.node) {
            if style.pointer_events == PointerEvents::None {
                return None;
            }
        }

        // Test children in reverse order (topmost first, z-order)
        let children = layout_box.children.clone();
        for &child_id in children.iter().rev() {
            if let Some(result) = self.hit_test_box(child_id, point) {
                return Some(result);
            }
        }

        // No child matched — this box is the target
        let point_in_node = Point::new(
            point.x - layout_box.content_rect.x,
            point.y - layout_box.content_rect.y,
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
        results: &mut Vec<HitTestResult>,
    ) {
        let layout_box = match self.layout.get(box_id) {
            Some(b) => b,
            None => return,
        };

        if !layout_box.border_rect.contains(point) {
            return;
        }

        if let Some(style) = self.styles.get(layout_box.node) {
            if style.pointer_events == PointerEvents::None {
                return;
            }
        }

        // Add this box
        let point_in_node = Point::new(
            point.x - layout_box.content_rect.x,
            point.y - layout_box.content_rect.y,
        );
        results.push(HitTestResult {
            node: layout_box.node,
            point_in_node,
            ancestors: Vec::new(), // simplified for all-results mode
        });

        // Recurse children
        let children = layout_box.children.clone();
        for &child_id in children.iter().rev() {
            self.hit_test_box_all(child_id, point, results);
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
