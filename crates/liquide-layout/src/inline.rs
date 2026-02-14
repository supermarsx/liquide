//! Inline layout — simplified inline formatting context.

use liquide_dom::{Document, NodeId};
use liquide_style_engine::StyleMap;

use crate::geometry::Rect;
use crate::tree::{BoxType, LayoutBoxId, LayoutTree, LineBox};
use crate::TextMeasurer;

/// Layout an inline element (simplified — wraps text content).
pub fn layout_inline(
    doc: &Document,
    node_id: NodeId,
    styles: &StyleMap,
    tree: &mut LayoutTree,
    text_measurer: &dyn TextMeasurer,
    max_width: f32,
    offset_x: f32,
    offset_y: f32,
) -> LayoutBoxId {
    let style = styles.get(node_id).cloned().unwrap_or_default();
    let box_id = tree.alloc(node_id, BoxType::Inline);

    // Gather all text content from children
    let mut text = String::new();
    let children = doc.children(node_id).to_vec();
    for &child_id in &children {
        if let Some(node) = doc.get(child_id) {
            if let Some(t) = node.text_content() {
                text.push_str(t);
            }
        }
    }

    if text.is_empty() {
        if let Some(node) = doc.get(node_id) {
            if let Some(t) = node.text_content() {
                text = t.to_string();
            }
        }
    }

    if !text.is_empty() {
        let metrics = text_measurer.measure(
            &text,
            style.font_size,
            &style.font_family,
            style.font_weight,
            Some(max_width),
        );

        if let Some(b) = tree.get_mut(box_id) {
            b.content_rect = Rect::new(offset_x, offset_y, metrics.width, metrics.height);
            b.padding_rect = b.content_rect;
            b.border_rect = b.content_rect;
            b.margin_rect = b.content_rect;
            b.baseline = Some(metrics.baseline);
        }
    }

    box_id
}
