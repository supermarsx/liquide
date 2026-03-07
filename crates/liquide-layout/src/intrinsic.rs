//! Intrinsic sizing — min-content, max-content, fit-content.
//!
//! Implements CSS Intrinsic & Extrinsic Sizing Level 3 §4-5.

use liquide_dom::{Document, NodeId};
use liquide_style_engine::StyleMap;
use liquide_style_engine::computed::Display;

use crate::{TextMeasurer, TextProperties};

/// Calculate the min-content width for a node.
///
/// This is the narrowest the content can be without overflow — typically
/// the width of the longest word for text, or the max of children's
/// min-content widths for block containers.
pub fn min_content_width(
    doc: &Document,
    node_id: NodeId,
    styles: &StyleMap,
    text_measurer: &dyn TextMeasurer,
) -> f32 {
    let style = styles.get(node_id).cloned().unwrap_or_default();

    // If the element has an explicit width, that's its intrinsic contribution
    if let Some(w) = style.width.resolve_px(0.0, 16.0, style.font_size, 0.0, 0.0) {
        return apply_min_max_width(&style, w);
    }

    // Text nodes: longest word
    if let Some(node) = doc.get(node_id) {
        if let Some(text) = node.text_content() {
            let text_props = TextProperties::from_style(&style);
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
            return max_word_width + horizontal_box_edges(&style);
        }
    }

    // Flex containers in row direction: sum of children's min-content widths
    // (each item needs at least its min-content width on the main axis)
    if style.is_flex_container() && style.is_flex_row() {
        let children = doc.children(node_id).to_vec();
        let mut total = 0.0f32;
        let gap = style.gap.width
            .resolve_px(0.0, 16.0, style.font_size, 0.0, 0.0)
            .unwrap_or(0.0);
        let mut count = 0usize;
        for &child_id in &children {
            let child_style = styles.get(child_id).cloned().unwrap_or_default();
            if child_style.display == Display::None {
                continue;
            }
            total += min_content_width(doc, child_id, styles, text_measurer);
            count += 1;
        }
        if count > 1 {
            total += (count - 1) as f32 * gap;
        }
        return total + horizontal_box_edges(&style);
    }

    // Grid containers: max of column min-content widths
    // (simplified: treat as block)

    // Block/inline containers: max of children's min-content widths
    let children = doc.children(node_id).to_vec();
    let mut max = 0.0f32;
    for &child_id in &children {
        let child_style = styles.get(child_id).cloned().unwrap_or_default();
        if child_style.display == Display::None {
            continue;
        }
        max = max.max(min_content_width(doc, child_id, styles, text_measurer));
    }
    max + horizontal_box_edges(&style)
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

    // If the element has an explicit width, that's its max-content contribution
    if let Some(w) = style.width.resolve_px(0.0, 16.0, style.font_size, 0.0, 0.0) {
        return apply_min_max_width(&style, w);
    }

    // Text nodes: full text width without wrapping
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
            return m.width + horizontal_box_edges(&style);
        }
    }

    // Flex containers in row direction: sum of children's max-content widths
    if style.is_flex_container() && style.is_flex_row() {
        let children = doc.children(node_id).to_vec();
        let mut total = 0.0f32;
        let gap = style.gap.width
            .resolve_px(0.0, 16.0, style.font_size, 0.0, 0.0)
            .unwrap_or(0.0);
        let mut count = 0usize;
        for &child_id in &children {
            let child_style = styles.get(child_id).cloned().unwrap_or_default();
            if child_style.display == Display::None {
                continue;
            }
            total += max_content_width(doc, child_id, styles, text_measurer);
            count += 1;
        }
        if count > 1 {
            total += (count - 1) as f32 * gap;
        }
        return total + horizontal_box_edges(&style);
    }

    // Block-level: max of children's max-content widths
    // (children stack vertically in block flow)
    let children = doc.children(node_id).to_vec();
    let mut max = 0.0f32;
    for &child_id in &children {
        let child_style = styles.get(child_id).cloned().unwrap_or_default();
        if child_style.display == Display::None {
            continue;
        }
        max = max.max(max_content_width(doc, child_id, styles, text_measurer));
    }
    max + horizontal_box_edges(&style)
}

/// Apply min-width / max-width constraints to a computed width.
fn apply_min_max_width(style: &liquide_style_engine::computed::ComputedStyle, w: f32) -> f32 {
    let min_w = style.min_width.resolve_px(0.0, 16.0, style.font_size, 0.0, 0.0).unwrap_or(0.0);
    let max_w = style.max_width.resolve_px(0.0, 16.0, style.font_size, 0.0, 0.0).unwrap_or(f32::INFINITY);
    w.max(min_w).min(max_w)
}

/// Calculate the fit-content width: clamp(min-content, stretch-fit, max-content).
///
/// CSS Intrinsic & Extrinsic Sizing Level 3 §4.1:
/// fit-content = min(max-content, max(min-content, stretch-fit))
/// where stretch-fit is the available width.
pub fn fit_content_width(
    doc: &Document,
    node_id: NodeId,
    styles: &StyleMap,
    text_measurer: &dyn TextMeasurer,
    available_width: f32,
) -> f32 {
    let min_cw = min_content_width(doc, node_id, styles, text_measurer);
    let max_cw = max_content_width(doc, node_id, styles, text_measurer);
    min_cw.max(available_width.min(max_cw))
}

/// Compute the horizontal padding + border contribution of an element.
/// Used to add box-model edges to intrinsic content sizes.
fn horizontal_box_edges(style: &liquide_style_engine::computed::ComputedStyle) -> f32 {
    let pad_l = style.padding.left.resolve_px(0.0, 16.0, style.font_size, 0.0, 0.0).unwrap_or(0.0);
    let pad_r = style.padding.right.resolve_px(0.0, 16.0, style.font_size, 0.0, 0.0).unwrap_or(0.0);
    pad_l + pad_r + style.border_width.left + style.border_width.right
}
