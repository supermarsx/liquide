//! Intrinsic sizing — min-content, max-content, fit-content.

use liquide_dom::{Document, NodeId};
use liquide_style_engine::StyleMap;

use crate::{TextMeasurer, TextProperties};

/// Calculate the min-content width for a node.
///
/// This is the narrowest the content can be without overflow — typically
/// the width of the longest word.
pub fn min_content_width(
    doc: &Document,
    node_id: NodeId,
    styles: &StyleMap,
    text_measurer: &dyn TextMeasurer,
) -> f32 {
    let style = styles.get(node_id).cloned().unwrap_or_default();

    if let Some(node) = doc.get(node_id) {
        if let Some(text) = node.text_content() {
            let text_props = TextProperties::from_style(&style);
            // Min-content width = width of the longest word
            let mut max_word_width = 0.0f32;
            for word in text.split_whitespace() {
                let m = text_measurer.measure(
                    word,
                    style.font_size,
                    &style.font_family,
                    style.font_weight,
                    None,
                    &text_props,
                );
                max_word_width = max_word_width.max(m.width);
            }
            return max_word_width;
        }
    }

    // For element nodes, it's the max of children's min-content widths
    let children = doc.children(node_id).to_vec();
    let mut max = 0.0f32;
    for &child_id in &children {
        max = max.max(min_content_width(doc, child_id, styles, text_measurer));
    }
    max
}

/// Calculate the max-content width for a node.
///
/// This is the widest the content wants to be — no wrapping.
pub fn max_content_width(
    doc: &Document,
    node_id: NodeId,
    styles: &StyleMap,
    text_measurer: &dyn TextMeasurer,
) -> f32 {
    let style = styles.get(node_id).cloned().unwrap_or_default();

    if let Some(node) = doc.get(node_id) {
        if let Some(text) = node.text_content() {
            let text_props = TextProperties::from_style(&style);
            let m = text_measurer.measure(
                text,
                style.font_size,
                &style.font_family,
                style.font_weight,
                None,
                &text_props,
            );
            return m.width;
        }
    }

    // For element nodes, sum children's max-content widths
    let children = doc.children(node_id).to_vec();
    let mut total = 0.0f32;
    for &child_id in &children {
        total += max_content_width(doc, child_id, styles, text_measurer);
    }
    total
}
