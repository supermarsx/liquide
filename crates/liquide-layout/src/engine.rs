//! Layout engine — the main entry point for computing layout.

use liquide_dom::{Document, NodeId};
use liquide_style_engine::computed::{Display, Position};
use liquide_style_engine::StyleMap;

use crate::geometry::{Rect, Size};
use crate::tree::{LayoutBoxId, LayoutTree};
use crate::{ImageMeasurer, TextMeasurer};

/// The layout engine. Computes geometry for all elements in the document.
pub struct LayoutEngine {
    /// Viewport size.
    pub viewport: Size,
    /// Root font size for `rem` units.
    pub base_font_size: f32,
}

impl LayoutEngine {
    /// Create a new layout engine.
    pub fn new(viewport: Size, base_font_size: f32) -> Self {
        Self {
            viewport,
            base_font_size,
        }
    }

    /// Run layout on the entire document.
    pub fn layout(
        &mut self,
        doc: &Document,
        styles: &StyleMap,
        text_measurer: &dyn TextMeasurer,
        image_measurer: &dyn ImageMeasurer,
    ) -> LayoutTree {
        let mut tree = LayoutTree::new();
        let root = doc.root();

        let root_style = styles.get(root).cloned().unwrap_or_default();

        // Root layout starts as block
        let root_box = if root_style.is_flex_container() {
            crate::flex::layout_flex(
                doc, root, styles, &mut tree, text_measurer, image_measurer,
                self.viewport.width, self.viewport.height, 0.0, 0.0,
                self.viewport.width, self.viewport.height, self.base_font_size,
            )
        } else if root_style.is_grid_container() {
            crate::grid::layout_grid(
                doc, root, styles, &mut tree, text_measurer, image_measurer,
                self.viewport.width, self.viewport.height, 0.0, 0.0,
                self.viewport.width, self.viewport.height, self.base_font_size,
            )
        } else {
            crate::block::layout_block(
                doc, root, styles, &mut tree, text_measurer, image_measurer,
                self.viewport.width, self.viewport.height, 0.0, 0.0,
                self.viewport.width, self.viewport.height, self.base_font_size,
            )
        };

        tree.root = root_box;

        // Second pass: layout positioned elements
        self.layout_positioned_elements(doc, root, styles, &mut tree, text_measurer, image_measurer);

        tree
    }

    /// Layout positioned elements (absolute/fixed) in a second pass.
    fn layout_positioned_elements(
        &self,
        doc: &Document,
        node_id: NodeId,
        styles: &StyleMap,
        tree: &mut LayoutTree,
        text_measurer: &dyn TextMeasurer,
        image_measurer: &dyn ImageMeasurer,
    ) {
        let children = doc.children(node_id).to_vec();

        // Find the containing block rect for this node
        let containing_rect = tree
            .find_by_node(node_id)
            .map(|b| b.border_rect)
            .unwrap_or(Rect::new(0.0, 0.0, self.viewport.width, self.viewport.height));

        for &child_id in &children {
            let child_style = styles.get(child_id).cloned().unwrap_or_default();

            if matches!(child_style.position, Position::Absolute | Position::Fixed) {
                if let Some(pos_box) = crate::positioned::layout_positioned(
                    doc, child_id, styles, tree, text_measurer, image_measurer,
                    containing_rect, self.viewport.width, self.viewport.height, self.base_font_size,
                ) {
                    // Add to parent in tree
                    if let Some(parent_box) = tree
                        .find_by_node(node_id)
                        .map(|b| b.id)
                    {
                        tree.add_child(parent_box, pos_box);
                    }
                }
            }

            // Recurse
            self.layout_positioned_elements(doc, child_id, styles, tree, text_measurer, image_measurer);
        }
    }
}

impl Default for LayoutEngine {
    fn default() -> Self {
        Self::new(Size::new(1920.0, 1080.0), 16.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_dom::Document;
    use liquide_style_engine::engine::{StyleEngine, ViewportSize};
    use crate::{DefaultTextMeasurer, DefaultImageMeasurer};

    #[test]
    fn basic_block_layout() {
        let mut doc = Document::new();
        let root = doc.root();
        let div = doc.create_element("div");
        doc.append_child(root, div);

        let mut style_engine = StyleEngine::new(ViewportSize { width: 1920.0, height: 1080.0 }, 16.0);
        style_engine.add_stylesheet("div { width: 200px; height: 100px; }");

        let style_map = style_engine.restyle_all(&doc);
        let mut layout = LayoutEngine::new(Size::new(1920.0, 1080.0), 16.0);
        let tree = layout.layout(&doc, &style_map, &DefaultTextMeasurer, &DefaultImageMeasurer);

        assert!(tree.box_count() > 0);
    }

    #[test]
    fn flex_layout() {
        let mut doc = Document::new();
        let root = doc.root();
        let container = doc.create_element("dock");
        let item1 = doc.create_element("dock-item");
        let item2 = doc.create_element("dock-item");
        doc.append_child(root, container);
        doc.append_child(container, item1);
        doc.append_child(container, item2);

        let mut style_engine = StyleEngine::default();
        style_engine.add_stylesheet(
            r#"
            dock { display: flex; width: 200px; gap: 8px; }
            dock-item { width: 50px; height: 50px; }
            "#,
        );

        let style_map = style_engine.restyle_all(&doc);
        let mut layout = LayoutEngine::default();
        let tree = layout.layout(&doc, &style_map, &DefaultTextMeasurer, &DefaultImageMeasurer);

        // Should have boxes for container + 2 items
        assert!(tree.box_count() >= 3);
    }
}
