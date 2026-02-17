//! Layout engine — the main entry point for computing layout.

use liquide_dom::{Document, NodeId};
use liquide_style_engine::StyleMap;
use liquide_style_engine::computed::Position;

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
                doc,
                root,
                styles,
                &mut tree,
                text_measurer,
                image_measurer,
                self.viewport.width,
                self.viewport.height,
                0.0,
                0.0,
                self.viewport.width,
                self.viewport.height,
                self.base_font_size,
            )
        } else if root_style.is_grid_container() {
            crate::grid::layout_grid(
                doc,
                root,
                styles,
                &mut tree,
                text_measurer,
                image_measurer,
                self.viewport.width,
                self.viewport.height,
                0.0,
                0.0,
                self.viewport.width,
                self.viewport.height,
                self.base_font_size,
            )
        } else if root_style.is_table() {
            crate::table::layout_table(
                doc,
                root,
                styles,
                &mut tree,
                text_measurer,
                image_measurer,
                self.viewport.width,
                self.viewport.height,
                0.0,
                0.0,
                self.viewport.width,
                self.viewport.height,
                self.base_font_size,
            )
        } else if root_style.is_multicol() {
            crate::multicol::layout_multicol(
                doc,
                root,
                styles,
                &mut tree,
                text_measurer,
                image_measurer,
                self.viewport.width,
                self.viewport.height,
                0.0,
                0.0,
                self.viewport.width,
                self.viewport.height,
                self.base_font_size,
            )
        } else if matches!(
            root_style.display,
            liquide_style_engine::computed::Display::Inline
        ) {
            crate::inline::layout_inline(
                doc,
                root,
                styles,
                &mut tree,
                text_measurer,
                self.viewport.width,
                0.0,
                0.0,
            )
        } else {
            crate::block::layout_block(
                doc,
                root,
                styles,
                &mut tree,
                text_measurer,
                image_measurer,
                self.viewport.width,
                self.viewport.height,
                0.0,
                0.0,
                self.viewport.width,
                self.viewport.height,
                self.base_font_size,
            )
        };

        tree.root = root_box;

        // Second pass: layout positioned elements
        self.layout_positioned_elements(
            doc,
            root,
            styles,
            &mut tree,
            text_measurer,
            image_measurer,
        );

        // Third pass: adjust sticky-positioned elements based on scroll offsets
        Self::apply_sticky_offsets(&mut tree, styles, doc);

        tree
    }

    /// Apply scroll-aware sticky positioning.
    ///
    /// For each element with `position: sticky`, we clamp its position so it
    /// stays within the visible scrollport of the nearest scroll ancestor.
    fn apply_sticky_offsets(tree: &mut LayoutTree, styles: &StyleMap, _doc: &Document) {
        // Collect all box IDs first to avoid borrow issues.
        let all_ids: Vec<LayoutBoxId> = (0..tree.boxes.len()).collect();

        for box_id in all_ids {
            let node_id = match tree.get(box_id) {
                Some(b) => b.node,
                None => continue,
            };
            let style = match styles.get(node_id) {
                Some(s) => s.clone(),
                None => continue,
            };
            if style.position != Position::Sticky {
                continue;
            }

            let font_size = style.font_size;
            let base_font_size = 16.0f32; // TODO: propagate from engine

            // Find the nearest scroll-container ancestor in the layout tree.
            let mut scroll_ancestor = tree.get(box_id).and_then(|b| b.parent);
            let mut scroll_offset = (0.0f32, 0.0f32);
            let mut scroll_viewport = (0.0f32, 0.0f32);
            while let Some(ancestor_id) = scroll_ancestor {
                if let Some(ancestor) = tree.get(ancestor_id) {
                    if ancestor.scroll_size.is_some() {
                        scroll_offset = ancestor.scroll_offset;
                        scroll_viewport = (ancestor.content_rect.width, ancestor.content_rect.height);
                        break;
                    }
                    scroll_ancestor = ancestor.parent;
                } else {
                    break;
                }
            }

            // Resolve sticky offsets (top/right/bottom/left)
            let vw = scroll_viewport.0.max(1.0);
            let vh = scroll_viewport.1.max(1.0);
            let top = style.top.resolve_px(vh, base_font_size, font_size, vw, vh);
            let bottom = style.bottom.resolve_px(vh, base_font_size, font_size, vw, vh);
            let left = style.left.resolve_px(vw, base_font_size, font_size, vw, vh);
            let right = style.right.resolve_px(vw, base_font_size, font_size, vw, vh);

            // The element's current (normal-flow) position is stored in its border_rect.
            let bx = tree.get(box_id).map(|b| b.border_rect.x).unwrap_or(0.0);
            let by = tree.get(box_id).map(|b| b.border_rect.y).unwrap_or(0.0);
            let bw = tree.get(box_id).map(|b| b.border_rect.width).unwrap_or(0.0);
            let bh = tree.get(box_id).map(|b| b.border_rect.height).unwrap_or(0.0);

            // Compute clamped position based on scroll offset.
            // The sticky constraint is: the element must stay within the
            // scrollport bounds offset by the specified edges.
            let mut new_x = bx;
            let mut new_y = by;

            // Vertical sticky clamping
            if let Some(top_val) = top {
                // Element must not go above (scroll_offset.y + top)
                let min_y = scroll_offset.1 + top_val;
                if new_y < min_y {
                    new_y = min_y;
                }
            }
            if let Some(bottom_val) = bottom {
                // Element must not go below (scroll_offset.y + viewport_h - bottom - element_h)
                let max_y = scroll_offset.1 + scroll_viewport.1 - bottom_val - bh;
                if new_y > max_y {
                    new_y = max_y;
                }
            }

            // Horizontal sticky clamping
            if let Some(left_val) = left {
                let min_x = scroll_offset.0 + left_val;
                if new_x < min_x {
                    new_x = min_x;
                }
            }
            if let Some(right_val) = right {
                let max_x = scroll_offset.0 + scroll_viewport.0 - right_val - bw;
                if new_x > max_x {
                    new_x = max_x;
                }
            }

            // Apply offset delta to all rects
            let dx = new_x - bx;
            let dy = new_y - by;
            if dx.abs() > 0.001 || dy.abs() > 0.001 {
                if let Some(b) = tree.get_mut(box_id) {
                    b.content_rect.x += dx;
                    b.content_rect.y += dy;
                    b.padding_rect.x += dx;
                    b.padding_rect.y += dy;
                    b.border_rect.x += dx;
                    b.border_rect.y += dy;
                    b.margin_rect.x += dx;
                    b.margin_rect.y += dy;
                }
            }
        }
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

        // Find the containing block rect for this node (CSS2.1 §10.1: padding edge)
        let containing_rect = tree
            .find_by_node(node_id)
            .map(|b| b.padding_rect)
            .unwrap_or(Rect::new(
                0.0,
                0.0,
                self.viewport.width,
                self.viewport.height,
            ));

        for &child_id in &children {
            let child_style = styles.get(child_id).cloned().unwrap_or_default();

            if matches!(child_style.position, Position::Absolute | Position::Fixed) {
                if let Some(pos_box) = crate::positioned::layout_positioned(
                    doc,
                    child_id,
                    styles,
                    tree,
                    text_measurer,
                    image_measurer,
                    containing_rect,
                    self.viewport.width,
                    self.viewport.height,
                    self.base_font_size,
                ) {
                    // Add to parent in tree
                    if let Some(parent_box) = tree.find_by_node(node_id).map(|b| b.id) {
                        tree.add_child(parent_box, pos_box);
                    }
                }
            }

            // Recurse
            self.layout_positioned_elements(
                doc,
                child_id,
                styles,
                tree,
                text_measurer,
                image_measurer,
            );
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
    use crate::{DefaultImageMeasurer, DefaultTextMeasurer};
    use liquide_dom::Document;
    use liquide_style_engine::engine::{StyleEngine, ViewportSize};

    #[test]
    fn basic_block_layout() {
        let mut doc = Document::new();
        let root = doc.root();
        let div = doc.create_element("div");
        doc.append_child(root, div);

        let mut style_engine = StyleEngine::new(
            ViewportSize {
                width: 1920.0,
                height: 1080.0,
            },
            16.0,
        );
        style_engine.add_stylesheet("div { width: 200px; height: 100px; }");

        let style_map = style_engine.restyle_all(&doc);
        let mut layout = LayoutEngine::new(Size::new(1920.0, 1080.0), 16.0);
        let tree = layout.layout(
            &doc,
            &style_map,
            &DefaultTextMeasurer,
            &DefaultImageMeasurer,
        );

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
        let tree = layout.layout(
            &doc,
            &style_map,
            &DefaultTextMeasurer,
            &DefaultImageMeasurer,
        );

        // Should have boxes for container + 2 items
        assert!(tree.box_count() >= 3);
    }
}
