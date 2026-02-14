//! Flex layout — CSS Flexbox Level 1.

use liquide_dom::{Document, NodeId};
use liquide_style_engine::computed::{Display, FlexDirection, FlexWrap, JustifyContent, AlignItems, Position};
use liquide_style_engine::dimension::Dimension;
use liquide_style_engine::StyleMap;

use crate::geometry::Rect;
use crate::tree::{BoxType, LayoutBoxId, LayoutTree};
use crate::{TextMeasurer, ImageMeasurer};

/// Perform flexbox layout.
pub fn layout_flex(
    doc: &Document,
    node_id: NodeId,
    styles: &StyleMap,
    tree: &mut LayoutTree,
    text_measurer: &dyn TextMeasurer,
    image_measurer: &dyn ImageMeasurer,
    container_width: f32,
    container_height: f32,
    offset_x: f32,
    offset_y: f32,
    viewport_w: f32,
    viewport_h: f32,
    base_font_size: f32,
) -> LayoutBoxId {
    let style = styles.get(node_id).cloned().unwrap_or_default();
    let box_id = tree.alloc(node_id, BoxType::Flex);

    let font_size = style.font_size;
    let width = style
        .width
        .resolve_px(container_width, base_font_size, font_size, viewport_w, viewport_h)
        .unwrap_or(container_width);

    let pad_top = resolve_dim(&style.padding.top, width, base_font_size, font_size, viewport_w, viewport_h);
    let pad_right = resolve_dim(&style.padding.right, width, base_font_size, font_size, viewport_w, viewport_h);
    let pad_bottom = resolve_dim(&style.padding.bottom, width, base_font_size, font_size, viewport_w, viewport_h);
    let pad_left = resolve_dim(&style.padding.left, width, base_font_size, font_size, viewport_w, viewport_h);

    let mar_top = resolve_dim(&style.margin.top, container_width, base_font_size, font_size, viewport_w, viewport_h);
    let mar_right = resolve_dim(&style.margin.right, container_width, base_font_size, font_size, viewport_w, viewport_h);
    let mar_bottom = resolve_dim(&style.margin.bottom, container_width, base_font_size, font_size, viewport_w, viewport_h);
    let mar_left = resolve_dim(&style.margin.left, container_width, base_font_size, font_size, viewport_w, viewport_h);

    let border_top = style.border_width.top;
    let border_right = style.border_width.right;
    let border_bottom = style.border_width.bottom;
    let border_left = style.border_width.left;

    let content_width = width - pad_left - pad_right - border_left - border_right;
    let content_x = offset_x + mar_left + border_left + pad_left;
    let content_y = offset_y + mar_top + border_top + pad_top;

    let direction = style.flex_direction;
    let is_row = matches!(direction, FlexDirection::Row | FlexDirection::RowReverse);
    let is_reverse = matches!(direction, FlexDirection::RowReverse | FlexDirection::ColumnReverse);

    let gap_dim = if is_row { &style.gap.width } else { &style.gap.height };
    let gap = gap_dim.resolve_px(content_width, base_font_size, font_size, viewport_w, viewport_h)
        .unwrap_or(0.0);

    // Collect flex items
    let children = doc.children(node_id).to_vec();
    let mut items: Vec<FlexItem> = Vec::new();

    for &child_id in &children {
        let child_style = styles.get(child_id).cloned().unwrap_or_default();
        if child_style.display == Display::None {
            continue;
        }
        if matches!(child_style.position, Position::Absolute | Position::Fixed) {
            continue;
        }

        // Lay out child to get its intrinsic size
        let child_box = crate::block::layout_block(
            doc, child_id, styles, tree, text_measurer, image_measurer,
            content_width, container_height, 0.0, 0.0,
            viewport_w, viewport_h, base_font_size,
        );

        let intrinsic = tree.get(child_box).map(|b| b.margin_rect).unwrap_or(Rect::zero());

        let flex_basis = child_style.flex_basis
            .resolve_px(content_width, base_font_size, child_style.font_size, viewport_w, viewport_h);

        let main_size = if is_row {
            flex_basis.unwrap_or(intrinsic.width)
        } else {
            flex_basis.unwrap_or(intrinsic.height)
        };

        items.push(FlexItem {
            box_id: child_box,
            node_id: child_id,
            flex_grow: child_style.flex_grow,
            flex_shrink: child_style.flex_shrink,
            main_size,
            cross_size: if is_row { intrinsic.height } else { intrinsic.width },
            order: child_style.order,
        });

        tree.add_child(box_id, child_box);
    }

    // Sort by order
    items.sort_by_key(|i| i.order);

    if is_reverse {
        items.reverse();
    }

    // Calculate total main size
    let total_gaps = if items.len() > 1 { (items.len() - 1) as f32 * gap } else { 0.0 };
    let total_main: f32 = items.iter().map(|i| i.main_size).sum::<f32>() + total_gaps;
    let available_main = if is_row { content_width } else {
        style.height
            .resolve_px(container_height, base_font_size, font_size, viewport_w, viewport_h)
            .unwrap_or(container_height)
    };
    let free_space = available_main - total_main;

    // Grow / shrink
    if free_space > 0.0 {
        let total_grow: f32 = items.iter().map(|i| i.flex_grow).sum();
        if total_grow > 0.0 {
            for item in &mut items {
                item.main_size += free_space * (item.flex_grow / total_grow);
            }
        }
    } else if free_space < 0.0 {
        let total_shrink: f32 = items.iter().map(|i| i.flex_shrink * i.main_size).sum();
        if total_shrink > 0.0 {
            for item in &mut items {
                let factor = (item.flex_shrink * item.main_size) / total_shrink;
                item.main_size += free_space * factor;
                item.main_size = item.main_size.max(0.0);
            }
        }
    }

    // Justify content — calculate start offset and inter-item spacing
    let used_main: f32 = items.iter().map(|i| i.main_size).sum::<f32>() + total_gaps;
    let remaining = available_main - used_main;
    let (mut main_offset, extra_gap) = match style.justify_content {
        JustifyContent::FlexStart => (0.0, 0.0),
        JustifyContent::FlexEnd => (remaining, 0.0),
        JustifyContent::Center => (remaining / 2.0, 0.0),
        JustifyContent::SpaceBetween => {
            if items.len() > 1 {
                (0.0, remaining / (items.len() - 1) as f32)
            } else {
                (0.0, 0.0)
            }
        }
        JustifyContent::SpaceAround => {
            let s = remaining / items.len() as f32;
            (s / 2.0, s)
        }
        JustifyContent::SpaceEvenly => {
            let s = remaining / (items.len() + 1) as f32;
            (s, s)
        }
    };

    // Position items
    let mut max_cross = 0.0f32;
    let item_count = items.len();
    for (i, item) in items.iter_mut().enumerate() {
        let (x, y, w, h) = if is_row {
            let x = content_x + main_offset;
            let y = content_y;
            (x, y, item.main_size, item.cross_size)
        } else {
            let x = content_x;
            let y = content_y + main_offset;
            (x, y, item.cross_size, item.main_size)
        };

        // Update box geometry
        if let Some(b) = tree.get_mut(item.box_id) {
            b.content_rect = Rect::new(x, y, w, h);
            b.padding_rect = b.content_rect;
            b.border_rect = b.content_rect;
            b.margin_rect = b.content_rect;
        }

        main_offset += item.main_size + gap + if i < item_count - 1 { extra_gap } else { 0.0 };

        if is_row {
            max_cross = max_cross.max(item.cross_size);
        } else {
            max_cross = max_cross.max(item.cross_size);
        }
    }

    // Align items on cross axis
    let cross_size = if is_row {
        style.height
            .resolve_px(container_height, base_font_size, font_size, viewport_w, viewport_h)
            .unwrap_or(max_cross)
    } else {
        content_width
    };

    for item in &items {
        let child_style = styles.get(item.node_id).cloned().unwrap_or_default();
        let align = match child_style.align_self {
            liquide_style_engine::computed::AlignSelf::Auto => style.align_items,
            liquide_style_engine::computed::AlignSelf::FlexStart => AlignItems::FlexStart,
            liquide_style_engine::computed::AlignSelf::FlexEnd => AlignItems::FlexEnd,
            liquide_style_engine::computed::AlignSelf::Center => AlignItems::Center,
            liquide_style_engine::computed::AlignSelf::Baseline => AlignItems::Baseline,
            liquide_style_engine::computed::AlignSelf::Stretch => AlignItems::Stretch,
        };

        if let Some(b) = tree.get_mut(item.box_id) {
            let item_cross = if is_row { b.content_rect.height } else { b.content_rect.width };
            let cross_offset = match align {
                AlignItems::FlexStart => 0.0,
                AlignItems::FlexEnd => cross_size - item_cross,
                AlignItems::Center => (cross_size - item_cross) / 2.0,
                AlignItems::Stretch => {
                    if is_row {
                        b.content_rect.height = cross_size;
                    } else {
                        b.content_rect.width = cross_size;
                    }
                    0.0
                }
                AlignItems::Baseline => 0.0, // simplified
            };

            if is_row {
                b.content_rect.y += cross_offset;
            } else {
                b.content_rect.x += cross_offset;
            }
            b.padding_rect = b.content_rect;
            b.border_rect = b.content_rect;
            b.margin_rect = b.content_rect;
        }
    }

    // Set container geometry
    let total_main_used = main_offset;
    let content_height = if is_row {
        style.height
            .resolve_px(container_height, base_font_size, font_size, viewport_w, viewport_h)
            .unwrap_or(max_cross)
    } else {
        total_main_used
    };

    if let Some(b) = tree.get_mut(box_id) {
        b.content_rect = Rect::new(content_x, content_y, content_width, content_height);
        b.padding_rect = Rect::new(
            content_x - pad_left,
            content_y - pad_top,
            content_width + pad_left + pad_right,
            content_height + pad_top + pad_bottom,
        );
        b.border_rect = Rect::new(
            b.padding_rect.x - border_left,
            b.padding_rect.y - border_top,
            b.padding_rect.width + border_left + border_right,
            b.padding_rect.height + border_top + border_bottom,
        );
        b.margin_rect = Rect::new(
            b.border_rect.x - mar_left,
            b.border_rect.y - mar_top,
            b.border_rect.width + mar_left + mar_right,
            b.border_rect.height + mar_top + mar_bottom,
        );
    }

    box_id
}

struct FlexItem {
    box_id: LayoutBoxId,
    node_id: NodeId,
    flex_grow: f32,
    flex_shrink: f32,
    main_size: f32,
    cross_size: f32,
    order: i32,
}

fn resolve_dim(
    dim: &Dimension,
    parent_px: f32,
    base_font_size: f32,
    font_size: f32,
    vw: f32,
    vh: f32,
) -> f32 {
    dim.resolve_px(parent_px, base_font_size, font_size, vw, vh)
        .unwrap_or(0.0)
}
