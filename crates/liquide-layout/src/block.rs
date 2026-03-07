//! Block layout — CSS block formatting context.

use liquide_dom::{Document, NodeId};
use liquide_style_engine::StyleMap;
use liquide_style_engine::computed::{AspectRatio, BoxSizing, Display, Float, LineClamp, ListStylePosition, ListStyleType, Overflow, Position};
use liquide_style_engine::dimension::Dimension;
use liquide_style_engine::style_map::PseudoKind;

use crate::float::{ClearSide, FloatContext, FloatSide};
use crate::geometry::{Rect, Size};
use crate::tree::{BoxType, LayoutBoxId, LayoutTree, PseudoElementKind};
use crate::writing_mode::WritingModeContext;
use crate::{ImageMeasurer, TextMeasurer};

/// Perform block layout for a node and its children.
///
/// Block layout places children vertically, one below another, each taking
/// the full available width (unless the child has its own width constraint).
pub fn layout_block(
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

    let box_id = tree.alloc(node_id, BoxType::Block);

    // Writing mode context — determines which physical axis is inline vs block.
    let wm = WritingModeContext::with_direction(style.writing_mode, style.direction);
    let (_container_inline, _container_block) = wm.to_logical(container_width, container_height);

    // ── Counter processing ──
    // Apply counter-reset/increment/set to the thread-local counter registry.
    let _quotes = &style.quotes;
    if let Some(ref decl) = style.counter_reset {
        crate::counter::COUNTER_REGISTRY.with(|reg| reg.borrow_mut().apply_reset(decl));
    } else {
        // Push an empty scope so pop_scope is always balanced
        crate::counter::COUNTER_REGISTRY.with(|reg| reg.borrow_mut().push_empty_scope());
    }
    if let Some(ref decl) = style.counter_increment {
        crate::counter::COUNTER_REGISTRY.with(|reg| reg.borrow_mut().apply_increment(decl));
    }
    if let Some(ref decl) = style.counter_set {
        crate::counter::COUNTER_REGISTRY.with(|reg| reg.borrow_mut().apply_set(decl));
    }
    // For list-item display, implicitly increment the "list-item" counter
    if style.is_list_item() && style.counter_increment.is_none() {
        crate::counter::COUNTER_REGISTRY.with(|reg| reg.borrow_mut().apply_increment("list-item 1"));
    }

    // Resolve own dimensions.
    // In vertical writing modes, CSS `width` maps to the block-size and `height`
    // to the inline-size. However, we resolve the explicit width/height properties
    // as written — the writing-mode swapping is handled by using the correct
    // container dimension for percentage resolution.
    let font_size = style.font_size;
    let explicit_width = style.width.resolve_px(
        container_width,
        base_font_size,
        font_size,
        viewport_w,
        viewport_h,
    );

    // contain:size — use contain-intrinsic-width when no explicit width is set
    // (mirrors the contain-intrinsic-height logic used for content_height below)
    let explicit_width = if explicit_width.is_none() && style.contain.size {
        style.contain_intrinsic_width.resolve_px(
            container_width,
            base_font_size,
            font_size,
            viewport_w,
            viewport_h,
        ).or(explicit_width)
    } else {
        explicit_width
    };

    // Early check for explicit height (needed for margin collapsing detection)
    let resolved_explicit_height = style.height.resolve_px(
        container_height,
        base_font_size,
        font_size,
        viewport_w,
        viewport_h,
    );
    let has_explicit_height = resolved_explicit_height.is_some();

    // Aspect-ratio: if width is auto but height is explicit, derive width from height * ratio
    let explicit_width = if explicit_width.is_none() {
        if let (Some(h), &AspectRatio::Ratio(rw, rh)) = (resolved_explicit_height, &style.aspect_ratio) {
            if rh > 0.0 {
                Some(h * (rw / rh))
            } else {
                explicit_width
            }
        } else {
            explicit_width
        }
    } else {
        explicit_width
    };

    // Resolve intrinsic sizing keywords: width: min-content | max-content | fit-content(...)
    let explicit_width = match &style.width {
        Dimension::MinContent if explicit_width.is_none() => {
            Some(crate::intrinsic::min_content_width(doc, node_id, styles, text_measurer))
        }
        Dimension::MaxContent if explicit_width.is_none() => {
            Some(crate::intrinsic::max_content_width(doc, node_id, styles, text_measurer))
        }
        Dimension::FitContent(limit) if explicit_width.is_none() => {
            let min_cw = crate::intrinsic::min_content_width(doc, node_id, styles, text_measurer);
            let max_cw = crate::intrinsic::max_content_width(doc, node_id, styles, text_measurer);
            let limit_px = limit
                .resolve_px(container_width, base_font_size, font_size, viewport_w, viewport_h)
                .unwrap_or(container_width);
            // fit-content(limit) = clamp(min-content, limit, max-content)
            Some(min_cw.max(limit_px.min(max_cw)))
        }
        _ => explicit_width,
    };

    let width = explicit_width.unwrap_or(container_width);

    // Resolve padding
    let pad_top = resolve_dim(
        &style.padding.top,
        width,
        base_font_size,
        font_size,
        viewport_w,
        viewport_h,
    );
    let pad_right = resolve_dim(
        &style.padding.right,
        width,
        base_font_size,
        font_size,
        viewport_w,
        viewport_h,
    );
    let pad_bottom = resolve_dim(
        &style.padding.bottom,
        width,
        base_font_size,
        font_size,
        viewport_w,
        viewport_h,
    );
    let pad_left = resolve_dim(
        &style.padding.left,
        width,
        base_font_size,
        font_size,
        viewport_w,
        viewport_h,
    );

    let border_top = style.border_width.top;
    let border_right = style.border_width.right;
    let border_bottom = style.border_width.bottom;
    let border_left = style.border_width.left;

    // box-sizing: content-box (default) — `width` is the content width
    // box-sizing: border-box — `width` includes padding + border
    // width: auto — fills container, subtract padding + border regardless
    let content_width = match (explicit_width, style.box_sizing) {
        (Some(w), BoxSizing::ContentBox) => w,
        (Some(w), BoxSizing::BorderBox) => {
            (w - pad_left - pad_right - border_left - border_right).max(0.0)
        }
        (None, _) => (width - pad_left - pad_right - border_left - border_right).max(0.0),
    };

    // Apply min-width / max-width constraints
    let content_width = {
        let min_w = style
            .min_width
            .resolve_px(
                container_width,
                base_font_size,
                font_size,
                viewport_w,
                viewport_h,
            )
            .unwrap_or(0.0);
        let max_w = style
            .max_width
            .resolve_px(
                container_width,
                base_font_size,
                font_size,
                viewport_w,
                viewport_h,
            )
            .unwrap_or(f32::INFINITY);
        content_width.max(min_w).min(max_w)
    };

    // Resolve vertical margins (top/bottom)
    let mar_top = resolve_dim(
        &style.margin.top,
        container_width,
        base_font_size,
        font_size,
        viewport_w,
        viewport_h,
    );
    let mar_bottom = resolve_dim(
        &style.margin.bottom,
        container_width,
        base_font_size,
        font_size,
        viewport_w,
        viewport_h,
    );

    // Resolve horizontal margins with auto-margin centering support
    let (mar_left, mar_right) = {
        let ml = style.margin.left.resolve_px(
            container_width,
            base_font_size,
            font_size,
            viewport_w,
            viewport_h,
        );
        let mr = style.margin.right.resolve_px(
            container_width,
            base_font_size,
            font_size,
            viewport_w,
            viewport_h,
        );
        let ml_auto = matches!(style.margin.left, Dimension::Auto);
        let mr_auto = matches!(style.margin.right, Dimension::Auto);
        let outer = content_width + pad_left + pad_right + border_left + border_right;
        if ml_auto && mr_auto {
            let remaining = (container_width - outer).max(0.0);
            (remaining / 2.0, remaining / 2.0)
        } else if ml_auto {
            let remaining = (container_width - outer - mr.unwrap_or(0.0)).max(0.0);
            (remaining, mr.unwrap_or(0.0))
        } else if mr_auto {
            let remaining = (container_width - outer - ml.unwrap_or(0.0)).max(0.0);
            (ml.unwrap_or(0.0), remaining)
        } else {
            (ml.unwrap_or(0.0), mr.unwrap_or(0.0))
        }
    };

    // ── Replaced element short-circuit ──
    // If this node is a replaced element (img, video, canvas, etc.), delegate
    // to the replaced-element layout algorithm and return early.
    if crate::replaced::is_replaced_element(doc, node_id) {
        return crate::replaced::layout_replaced(
            doc,
            node_id,
            styles,
            tree,
            image_measurer,
            container_width,
            container_height,
            offset_x,
            offset_y,
            viewport_w,
            viewport_h,
            base_font_size,
        );
    }

    // ── Margin collapsing state ──
    // CSS §8.3.1: Vertical margins of adjacent block-level boxes collapse.
    // The collapsed margin is the max of the two adjoining margins.
    // Negative margins: the collapsed margin = max(positive) - abs(max(negative)).
    let mut child_y = 0.0f32;
    let mut prev_margin_bottom: Option<f32> = None;

    // Parent-child margin collapsing detection
    // A new BFC prevents margin collapsing with children (CSS §8.3.1)
    let parent_establishes_bfc = style.establishes_bfc();

    // ── Float context ──
    // A block formatting context (BFC) contains its own floats.
    // We always create a float context; it is only meaningful when
    // children have float:left/right or clear:left/right/both.
    let mut float_ctx = FloatContext::new(content_width);

    // Parent's top margin can collapse with first child's top margin if:
    // - No top border/padding separating them
    // - Parent doesn't establish a BFC
    let can_collapse_top = !parent_establishes_bfc 
        && border_top == 0.0 
        && pad_top == 0.0;

    // Parent's bottom margin can collapse with last child's bottom margin if:
    // - No bottom border/padding separating them
    // - No explicit height on parent
    // - Parent doesn't establish a BFC
    let can_collapse_bottom = !parent_establishes_bfc 
        && border_bottom == 0.0 
        && pad_bottom == 0.0
        && !has_explicit_height;

    // Track first and last child margins for parent-child collapsing
    let mut first_child_margin_top: Option<f32> = None;
    let mut last_child_margin_bottom: Option<f32> = None;

    let children = doc.children(node_id).to_vec();

    // CSS 2.1 §9.2.1.1 — block-in-inline: when a block container has both
    // inline-level and block-level children in normal flow, consecutive runs
    // of inline content must be wrapped in anonymous block boxes.
    let mixed = has_mixed_inline_block_children(doc, node_id, styles);

    // Generate ::before pseudo-element box if present
    if let Some(before_style) = styles.get_pseudo(node_id, PseudoKind::Before) {
        if let Some(ref content) = before_style.content {
            if !content.is_empty() {
                let text_props = crate::TextProperties::from_style(before_style);
                let metrics = text_measurer.measure(
                    content,
                    before_style.font_size,
                    &before_style.font_family,
                    before_style.font_weight,
                    Some(content_width),
                    &text_props,
                );
                let pe_box = tree.alloc(node_id, BoxType::PseudoElement {
                    kind: PseudoElementKind::Before,
                    content: content.clone(),
                });
                if let Some(pb) = tree.get_mut(pe_box) {
                    pb.content_rect = Rect::new(0.0, child_y, metrics.width.min(content_width), metrics.height);
                    pb.border_rect = pb.content_rect;
                    pb.padding_rect = pb.content_rect;
                    pb.margin_rect = pb.content_rect;
                }
                tree.add_child(box_id, pe_box);
                child_y += metrics.height;
            }
        }
    }

    // When children mix inline and block display types (CSS 2.1 §9.2.1.1),
    // we accumulate consecutive inline-level children and flush them into an
    // anonymous block box before processing the next block-level child.
    let mut pending_inline_run: Vec<NodeId> = Vec::new();

    /// Flush accumulated inline children into an anonymous block box.
    ///
    /// Each child in the run is laid out (text nodes via measure, inline
    /// elements via `layout_inline`) and appended as a child of the anonymous
    /// block.  The anonymous block is then appended to `parent_box`.
    #[allow(clippy::too_many_arguments)]
    fn flush_inline_run(
        run: &mut Vec<NodeId>,
        doc: &Document,
        styles: &StyleMap,
        tree: &mut LayoutTree,
        text_measurer: &dyn TextMeasurer,
        image_measurer: &dyn ImageMeasurer,
        content_width: f32,
        container_height: f32,
        viewport_w: f32,
        viewport_h: f32,
        base_font_size: f32,
        parent_box: LayoutBoxId,
        child_y: &mut f32,
        prev_margin_bottom: &mut Option<f32>,
        parent_node: NodeId,
    ) {
        if run.is_empty() {
            return;
        }

        // Create the anonymous block box — it inherits the parent node id
        // but has its own layout box.
        let anon_box = tree.alloc(parent_node, BoxType::AnonBlock);
        let mut inner_y = 0.0f32;

        for &cid in run.iter() {
            let cs = styles.get(cid).cloned().unwrap_or_default();

            // Text nodes
            if let Some(child_node) = doc.get(cid) {
                if child_node.is_text() {
                    if let Some(text) = child_node.text_content() {
                        let text_props = crate::TextProperties::from_style(&cs);
                        let metrics = text_measurer.measure(
                            text,
                            cs.font_size,
                            &cs.font_family,
                            cs.font_weight,
                            Some(content_width),
                            &text_props,
                        );
                        let clamped_text_w = metrics.width.min(content_width);
                        let text_x = crate::inline::align_offset(
                            cs.text_align,
                            content_width,
                            clamped_text_w,
                        );
                        let text_box = tree.alloc(
                            cid,
                            BoxType::Text {
                                line_boxes: Vec::new(),
                            },
                        );
                        if let Some(tb) = tree.get_mut(text_box) {
                            tb.content_rect =
                                Rect::new(text_x, inner_y, clamped_text_w, metrics.height);
                            tb.padding_rect = tb.content_rect;
                            tb.border_rect = tb.content_rect;
                            tb.margin_rect = tb.content_rect;
                            tb.baseline = Some(metrics.baseline);
                        }
                        tree.add_child(anon_box, text_box);
                        inner_y += metrics.height;
                        continue;
                    }
                }
            }

            // Inline elements — lay out via the inline formatting context
            if matches!(cs.display, Display::Inline) {
                let inline_box = crate::inline::layout_inline(
                    doc,
                    cid,
                    styles,
                    tree,
                    text_measurer,
                    content_width,
                    0.0,
                    inner_y,
                );
                tree.add_child(anon_box, inline_box);
                if let Some(ib) = tree.get(inline_box) {
                    inner_y += ib.margin_rect.height;
                }
            } else if matches!(cs.display, Display::InlineBlock) {
                let ib_width = if cs.width.resolve_px(
                    content_width, base_font_size, cs.font_size, viewport_w, viewport_h,
                ).is_some() {
                    content_width
                } else {
                    let min_cw = crate::intrinsic::min_content_width(doc, cid, styles, text_measurer);
                    let max_cw = crate::intrinsic::max_content_width(doc, cid, styles, text_measurer);
                    min_cw.max(content_width.min(max_cw))
                };
                let ib_box = layout_block(
                    doc,
                    cid,
                    styles,
                    tree,
                    text_measurer,
                    image_measurer,
                    ib_width,
                    container_height,
                    0.0,
                    inner_y,
                    viewport_w,
                    viewport_h,
                    base_font_size,
                );
                tree.add_child(anon_box, ib_box);
                if let Some(cb) = tree.get(ib_box) {
                    inner_y += cb.margin_rect.height;
                }
            } else {
                // Other inline-level types (inline-flex, inline-grid, ruby)
                // fall back to inline layout.
                let inline_box = crate::inline::layout_inline(
                    doc,
                    cid,
                    styles,
                    tree,
                    text_measurer,
                    content_width,
                    0.0,
                    inner_y,
                );
                tree.add_child(anon_box, inline_box);
                if let Some(ib) = tree.get(inline_box) {
                    inner_y += ib.margin_rect.height;
                }
            }
        }

        // Set the anonymous block's geometry — it spans full content_width
        // and is as tall as its inline content.
        if let Some(ab) = tree.get_mut(anon_box) {
            ab.content_rect = Rect::new(0.0, *child_y, content_width, inner_y);
            ab.padding_rect = ab.content_rect;
            ab.border_rect = ab.content_rect;
            ab.margin_rect = ab.content_rect;
        }
        tree.add_child(parent_box, anon_box);
        *child_y += inner_y;
        // Anonymous blocks break the margin collapsing sequence
        *prev_margin_bottom = None;

        run.clear();
    }

    for (_idx, &child_id) in children.iter().enumerate() {
        let child_style = styles.get(child_id).cloned().unwrap_or_default();

        // Skip display: none
        if child_style.display == Display::None {
            continue;
        }

        // Positioned children are handled separately
        if matches!(child_style.position, Position::Absolute | Position::Fixed) {
            continue;
        }

        // ── Handle `clear` property ──
        // `clear: left/right/both` forces this child below the cleared floats.
        let clear: ClearSide = child_style.clear.into();
        if clear != ClearSide::None {
            let clear_y = float_ctx.clear_y(clear);
            if clear_y > child_y {
                child_y = clear_y;
                // Clearing breaks the margin collapsing sequence
                prev_margin_bottom = None;
            }
        }

        // ── Handle floated children ──
        // Floated elements are taken out of normal flow: they don't advance
        // child_y but they create exclusion zones that shrink the available
        // width for subsequent in-flow content.
        let float_side = match child_style.float {
            Float::Left | Float::InlineStart => Some(FloatSide::Left),
            Float::Right | Float::InlineEnd => Some(FloatSide::Right),
            Float::None => None,
        };

        if let Some(side) = float_side {
            // Flush any pending inline run before the float
            if mixed {
                flush_inline_run(
                    &mut pending_inline_run,
                    doc, styles, tree, text_measurer, image_measurer,
                    content_width, container_height,
                    viewport_w, viewport_h, base_font_size,
                    box_id, &mut child_y, &mut prev_margin_bottom, node_id,
                );
            }

            // Layout the float to determine its intrinsic size.
            let float_box = layout_block(
                doc,
                child_id,
                styles,
                tree,
                text_measurer,
                image_measurer,
                content_width,
                container_height,
                0.0,
                0.0,
                viewport_w,
                viewport_h,
                base_font_size,
            );

            let (fw, fh) = tree
                .get(float_box)
                .map(|b| (b.margin_rect.width, b.margin_rect.height))
                .unwrap_or((0.0, 0.0));

            // Place the float via the float context. The float's top may not
            // be higher than child_y (the current block-flow position).
            let placed = match side {
                FloatSide::Left => float_ctx.place_left(fw, fh, child_y),
                FloatSide::Right => float_ctx.place_right(fw, fh, child_y),
            };

            // Reposition the float box to its placed coordinates (relative
            // to the parent's content area).
            if let Some(b) = tree.get_mut(float_box) {
                let dx = placed.x - b.margin_rect.x;
                let dy = placed.y - b.margin_rect.y;
                b.content_rect.x += dx;
                b.content_rect.y += dy;
                b.padding_rect.x += dx;
                b.padding_rect.y += dy;
                b.border_rect.x += dx;
                b.border_rect.y += dy;
                b.margin_rect.x += dx;
                b.margin_rect.y += dy;
            }

            tree.add_child(box_id, float_box);

            // Floats do NOT advance child_y — they are out of normal flow.
            // They also break the margin collapsing sequence.
            prev_margin_bottom = None;
            continue;
        }

        // ── Block-in-inline: accumulate inline children when mixed ──
        // When the parent has mixed inline/block children, inline-level
        // children and text nodes are accumulated into a pending run.
        // They will be flushed into an anonymous block box when the next
        // block-level child is encountered or at the end of the child list.
        if mixed {
            let is_text = doc.get(child_id).map_or(false, |n| n.is_text());
            if is_text || is_inline_level(child_style.display) {
                pending_inline_run.push(child_id);
                continue;
            }
            // Block-level child in mixed context — flush the pending inline run
            flush_inline_run(
                &mut pending_inline_run,
                doc, styles, tree, text_measurer, image_measurer,
                content_width, container_height,
                viewport_w, viewport_h, base_font_size,
                box_id, &mut child_y, &mut prev_margin_bottom, node_id,
            );
            // Fall through to normal block-level child processing below
        }

        // ── Query float exclusion zone for in-flow children ──
        // Estimate child height for the float query (refined after actual layout).
        let est_h = child_style.font_size * 1.2;
        let (float_left_edge, _float_right_edge, float_avail_w) =
            float_ctx.available_width_at(child_y, est_h);
        // Effective available width and x-offset for this child, accounting for floats.
        let child_avail_w = if float_avail_w < content_width {
            float_avail_w
        } else {
            content_width
        };
        let child_float_offset_x = if float_left_edge > 0.0 {
            float_left_edge
        } else {
            0.0
        };

        // Check if child is a text node
        if let Some(child_node) = doc.get(child_id) {
            if child_node.is_text() {
                if let Some(text) = child_node.text_content() {
                    let text_props = crate::TextProperties::from_style(&child_style);
                    let metrics = text_measurer.measure(
                        text,
                        child_style.font_size,
                        &child_style.font_family,
                        child_style.font_weight,
                        Some(child_avail_w),
                        &text_props,
                    );
                    // Clamp text width to available width so it doesn't overflow
                    let clamped_text_w = metrics.width.min(child_avail_w);
                    let child_is_rtl = matches!(child_style.direction, liquide_style_engine::computed::Direction::Rtl);
                    let text_x = child_float_offset_x + crate::inline::align_offset_directional(
                        child_style.text_align,
                        child_avail_w,
                        clamped_text_w,
                        child_is_rtl,
                    );
                    let text_box = tree.alloc(
                        child_id,
                        BoxType::Text {
                            line_boxes: Vec::new(),
                        },
                    );
                    if let Some(tb) = tree.get_mut(text_box) {
                        tb.content_rect =
                            Rect::new(text_x, child_y, clamped_text_w, metrics.height);
                        tb.padding_rect = tb.content_rect;
                        tb.border_rect = tb.content_rect;
                        tb.margin_rect = tb.content_rect;
                        tb.baseline = Some(metrics.baseline);
                    }
                    tree.add_child(box_id, text_box);
                    child_y += metrics.height;
                    // Text nodes break margin collapsing sequence
                    prev_margin_bottom = None;
                    continue;
                }
            }
        }

        // Resolve child top/bottom margins for collapse calculation
        let child_mar_top = resolve_dim(
            &child_style.margin.top,
            container_width,
            base_font_size,
            child_style.font_size,
            viewport_w,
            viewport_h,
        );
        let child_mar_bottom = resolve_dim(
            &child_style.margin.bottom,
            container_width,
            base_font_size,
            child_style.font_size,
            viewport_w,
            viewport_h,
        );

        // Track first child margin for parent-child collapsing
        if first_child_margin_top.is_none() {
            first_child_margin_top = Some(child_mar_top);
        }
        // Always update last child margin (will be last non-skipped child)
        last_child_margin_bottom = Some(child_mar_bottom);

        // Collapse adjacent margins: instead of prev_margin_bottom + child_margin_top,
        // use the larger of the two (for positive margins) or the more negative.
        if let Some(prev_mb) = prev_margin_bottom {
            let collapsed = collapse_margins(prev_mb, child_mar_top);
            // We already added prev_margin_bottom to child_y when we advanced
            // past the previous child. Remove both prev_mb and child_mar_top,
            // then add the single collapsed margin to avoid double-counting.
            child_y = child_y - prev_mb - child_mar_top + collapsed;
        } else if can_collapse_top && first_child_margin_top == Some(child_mar_top) {
            // First child: if parent-child top margin collapsing applies,
            // the child's top margin should be collapsed with parent's top margin
            // This is handled by the parent's margin calculation, so we don't
            // add the child's top margin here as extra space
        }

        // Recurse for element children.
        // Use float-adjusted available width and x-offset so in-flow content
        // flows around active floats.
        let child_box = if child_style.is_flex_container() {
            crate::flex::layout_flex(
                doc,
                child_id,
                styles,
                tree,
                text_measurer,
                image_measurer,
                child_avail_w,
                container_height,
                child_float_offset_x,
                child_y,
                viewport_w,
                viewport_h,
                base_font_size,
            )
        } else if child_style.is_grid_container() {
            crate::grid::layout_grid(
                doc,
                child_id,
                styles,
                tree,
                text_measurer,
                image_measurer,
                child_avail_w,
                container_height,
                child_float_offset_x,
                child_y,
                viewport_w,
                viewport_h,
                base_font_size,
            )
        } else if child_style.is_table() {
            crate::table::layout_table(
                doc,
                child_id,
                styles,
                tree,
                text_measurer,
                image_measurer,
                child_avail_w,
                container_height,
                child_float_offset_x,
                child_y,
                viewport_w,
                viewport_h,
                base_font_size,
            )
        } else if child_style.is_multicol() {
            crate::multicol::layout_multicol(
                doc,
                child_id,
                styles,
                tree,
                text_measurer,
                image_measurer,
                child_avail_w,
                container_height,
                child_float_offset_x,
                child_y,
                viewport_w,
                viewport_h,
                base_font_size,
            )
        } else if matches!(child_style.display, Display::Inline) {
            // Inline elements get proper inline formatting context
            crate::inline::layout_inline(
                doc,
                child_id,
                styles,
                tree,
                text_measurer,
                child_avail_w,
                child_float_offset_x,
                child_y,
            )
        } else if matches!(child_style.display, Display::InlineBlock) {
            // Inline-block: lay out as block internally with shrink-to-fit width.
            // Per CSS 2.1 §10.3.9, if width is auto, use:
            //   min(max(min-content, available), max-content)
            let ib_width = if child_style.width.resolve_px(
                content_width, base_font_size, child_style.font_size, viewport_w, viewport_h,
            ).is_some() {
                content_width // explicit width — let layout_block handle it
            } else {
                let min_cw = crate::intrinsic::min_content_width(doc, child_id, styles, text_measurer);
                let max_cw = crate::intrinsic::max_content_width(doc, child_id, styles, text_measurer);
                min_cw.max(content_width.min(max_cw))
            };
            layout_block(
                doc,
                child_id,
                styles,
                tree,
                text_measurer,
                image_measurer,
                ib_width.min(child_avail_w),
                container_height,
                child_float_offset_x,
                child_y,
                viewport_w,
                viewport_h,
                base_font_size,
            )
        } else if matches!(child_style.display, Display::Contents) {
            // display: contents — skip the element, layout its children
            // directly into this block context
            let grandchildren = doc.children(child_id).to_vec();
            let mut contents_last_box: Option<LayoutBoxId> = None;
            for &gc_id in &grandchildren {
                let gc_box = layout_block(
                    doc,
                    gc_id,
                    styles,
                    tree,
                    text_measurer,
                    image_measurer,
                    child_avail_w,
                    container_height,
                    child_float_offset_x,
                    child_y,
                    viewport_w,
                    viewport_h,
                    base_font_size,
                );
                tree.add_child(box_id, gc_box);
                if let Some(cb) = tree.get(gc_box) {
                    child_y += cb.margin_rect.height;
                }
                contents_last_box = Some(gc_box);
            }
            // Skip the normal add_child + height advancement below
            prev_margin_bottom = None;
            if let Some(_) = contents_last_box {
                continue;
            }
            // If no grandchildren, just create an empty box
            tree.alloc(child_id, BoxType::Block)
        } else if child_style.is_list_item() {
            // display: list-item — generate a marker box, then lay out as block.
            // Use the "list-item" counter value (updated by the child's own block layout)
            // instead of computing from the sibling index.
            let counter_val = crate::counter::COUNTER_REGISTRY
                .with(|reg| reg.borrow().counter_value("list-item"));
            let marker_text = list_marker_text(&child_style.list_style_type, counter_val.max(1) as usize);
            let marker_width = marker_text.len() as f32 * child_style.font_size * 0.6 + 4.0;

            // Create the block for the list item content
            let item_box = layout_block(
                doc,
                child_id,
                styles,
                tree,
                text_measurer,
                image_measurer,
                child_avail_w,
                container_height,
                child_float_offset_x,
                child_y,
                viewport_w,
                viewport_h,
                base_font_size,
            );

            // Create a marker box and position it
            let marker_box_id = tree.alloc(child_id, BoxType::ListMarker);
            let marker_h = child_style.font_size * 1.2;
            if let Some(mb) = tree.get_mut(marker_box_id) {
                match child_style.list_style_position {
                    ListStylePosition::Inside => {
                        // Inside: marker is first child, shifts content
                        mb.content_rect = Rect::new(child_float_offset_x, child_y, marker_width, marker_h);
                        mb.border_rect = mb.content_rect;
                        mb.padding_rect = mb.content_rect;
                        mb.margin_rect = mb.content_rect;
                    }
                    ListStylePosition::Outside => {
                        // Outside: marker is positioned to the left
                        mb.content_rect = Rect::new(child_float_offset_x - marker_width, child_y, marker_width, marker_h);
                        mb.border_rect = mb.content_rect;
                        mb.padding_rect = mb.content_rect;
                        mb.margin_rect = mb.content_rect;
                    }
                }
            }

            item_box
        } else {
            layout_block(
                doc,
                child_id,
                styles,
                tree,
                text_measurer,
                image_measurer,
                child_avail_w,
                container_height,
                child_float_offset_x,
                child_y,
                viewport_w,
                viewport_h,
                base_font_size,
            )
        };

        tree.add_child(box_id, child_box);

        // CSS §8.3.1: Empty block collapsing — if a child has zero height,
        // no border, no padding, and doesn't establish a BFC, its own top
        // and bottom margins collapse through it. The collapsed margin is
        // then used for sibling collapsing.
        let child_is_empty = {
            let ch_border_t = child_style.border_width.top;
            let ch_border_b = child_style.border_width.bottom;
            let ch_pad_t = child_style.padding.top.resolve_px(
                container_width, base_font_size, child_style.font_size, viewport_w, viewport_h,
            ).unwrap_or(0.0);
            let ch_pad_b = child_style.padding.bottom.resolve_px(
                container_width, base_font_size, child_style.font_size, viewport_w, viewport_h,
            ).unwrap_or(0.0);
            let ch_min_h = child_style.min_height.resolve_px(
                container_height, base_font_size, child_style.font_size, viewport_w, viewport_h,
            ).unwrap_or(0.0);
            let ch_content_h = tree.get(child_box)
                .map(|cb| cb.content_rect.height)
                .unwrap_or(0.0);
            ch_content_h == 0.0
                && ch_border_t == 0.0
                && ch_border_b == 0.0
                && ch_pad_t == 0.0
                && ch_pad_b == 0.0
                && ch_min_h == 0.0
                && !child_style.establishes_bfc()
        };

        if child_is_empty {
            // Margins collapse through: treat as a single collapsed margin
            let collapsed_through = collapse_margins(child_mar_top, child_mar_bottom);
            // This acts as the "previous margin bottom" for next sibling
            prev_margin_bottom = Some(collapsed_through);
            // Don't advance child_y — the box has zero height
        } else {
            if let Some(cb) = tree.get(child_box) {
                child_y += cb.margin_rect.height;
            }
            // Track this child's bottom margin for collapsing with next sibling
            prev_margin_bottom = Some(child_mar_bottom);
        }
    }

    // Flush any trailing inline run that was accumulated in mixed mode.
    if mixed {
        flush_inline_run(
            &mut pending_inline_run,
            doc, styles, tree, text_measurer, image_measurer,
            content_width, container_height,
            viewport_w, viewport_h, base_font_size,
            box_id, &mut child_y, &mut prev_margin_bottom, node_id,
        );
    }

    // ── BFC float containment ──
    // A block formatting context must be tall enough to contain all of its
    // floats. Ensure child_y is at least past the bottom of every float.
    let float_clear_all = float_ctx.clear_y(ClearSide::Both);
    if float_clear_all > child_y {
        child_y = float_clear_all;
    }

    // Generate ::after pseudo-element box if present
    if let Some(after_style) = styles.get_pseudo(node_id, PseudoKind::After) {
        if let Some(ref content) = after_style.content {
            if !content.is_empty() {
                let text_props = crate::TextProperties::from_style(after_style);
                let metrics = text_measurer.measure(
                    content,
                    after_style.font_size,
                    &after_style.font_family,
                    after_style.font_weight,
                    Some(content_width),
                    &text_props,
                );
                let pe_box = tree.alloc(node_id, BoxType::PseudoElement {
                    kind: PseudoElementKind::After,
                    content: content.clone(),
                });
                if let Some(pb) = tree.get_mut(pe_box) {
                    pb.content_rect = Rect::new(0.0, child_y, metrics.width.min(content_width), metrics.height);
                    pb.border_rect = pb.content_rect;
                    pb.padding_rect = pb.content_rect;
                    pb.margin_rect = pb.content_rect;
                }
                tree.add_child(box_id, pe_box);
                child_y += metrics.height;
            }
        }
    }

    // Pop the counter scope we pushed at the start of this block
    crate::counter::COUNTER_REGISTRY.with(|reg| reg.borrow_mut().pop_scope());

    // Content height: explicit or sum of children
    let explicit_height = style.height.resolve_px(
        container_height,
        base_font_size,
        font_size,
        viewport_w,
        viewport_h,
    );
    let content_height = match (explicit_height, style.box_sizing) {
        (Some(h), BoxSizing::ContentBox) => h,
        (Some(h), BoxSizing::BorderBox) => {
            (h - pad_top - pad_bottom - border_top - border_bottom).max(0.0)
        }
        (None, _) => {
            // contain:size takes priority — don't use children or aspect-ratio
            // for sizing; use contain-intrinsic-height if set, otherwise 0.
            if style.contain.size {
                style.contain_intrinsic_height.resolve_px(
                    container_height,
                    base_font_size,
                    font_size,
                    viewport_w,
                    viewport_h,
                ).unwrap_or(0.0)
            } else {
                // Check aspect-ratio before falling back to child_y
                match style.aspect_ratio {
                    AspectRatio::Ratio(w, h) if w > 0.0 => {
                        content_width * (h / w)
                    }
                    _ => child_y,
                }
            }
        }
    };

    // Apply min-height / max-height constraints
    let content_height = {
        let min_h = style
            .min_height
            .resolve_px(
                container_height,
                base_font_size,
                font_size,
                viewport_w,
                viewport_h,
            )
            .unwrap_or(0.0);
        let max_h = style
            .max_height
            .resolve_px(
                container_height,
                base_font_size,
                font_size,
                viewport_w,
                viewport_h,
            )
            .unwrap_or(f32::INFINITY);
        content_height.max(min_h).min(max_h)
    };

    // Apply line-clamp: limit height to N lines of text
    let content_height = match style.line_clamp {
        LineClamp::Count(n) if n > 0 => {
            let line_h = match &style.line_height {
                liquide_style_engine::computed::LineHeight::Px(px) => *px,
                liquide_style_engine::computed::LineHeight::Number(n) => n * style.font_size,
                liquide_style_engine::computed::LineHeight::Normal => style.font_size * 1.2,
            };
            let max_lines_h = n as f32 * line_h;
            content_height.min(max_lines_h)
        }
        _ => content_height,
    };

    // Apply zoom factor
    let content_width = if style.zoom != 1.0 && style.zoom > 0.0 {
        content_width * style.zoom
    } else {
        content_width
    };
    let content_height = if style.zoom != 1.0 && style.zoom > 0.0 {
        content_height * style.zoom
    } else {
        content_height
    };

    // scrollbar-gutter: stable — reserve space for scrollbar even when not needed
    let gutter_width = match style.scrollbar_gutter {
        liquide_style_engine::computed::ScrollbarGutter::Stable => {
            match style.scrollbar_width {
                liquide_style_engine::computed::ScrollbarWidth::Auto => 6.0,
                liquide_style_engine::computed::ScrollbarWidth::Thin => 4.0,
                liquide_style_engine::computed::ScrollbarWidth::None => 0.0,
            }
        }
        _ => 0.0,
    };
    let content_width = content_width - gutter_width;

    // Set geometry.
    // In horizontal-tb the block cursor (child_y) advances along y.
    // In vertical modes, the block cursor advances along x (via position_block_child).
    let content_x;
    let content_y;
    if wm.is_vertical() {
        // Vertical writing mode: inline offset → y, block offset → x
        let (px, py) = wm.position_block_child(
            mar_left + border_left + pad_left,
            mar_top + border_top + pad_top,
            container_width,
        );
        content_x = offset_x + px;
        content_y = offset_y + py;
    } else {
        content_x = offset_x + mar_left + border_left + pad_left;
        content_y = offset_y + mar_top + border_top + pad_top;
    }

    // Compute scroll_size for scroll containers before the mutable borrow.
    let is_scroll_container = matches!(
        style.overflow_x,
        Overflow::Auto | Overflow::Scroll
    ) || matches!(
        style.overflow_y,
        Overflow::Auto | Overflow::Scroll
    );
    let scroll_size = if is_scroll_container {
        // Find max child width for horizontal scroll
        let children = tree.get(box_id).map(|b| b.children.clone()).unwrap_or_default();
        let mut max_child_w = 0.0f32;
        for &child_box_id in &children {
            if let Some(cb) = tree.get(child_box_id) {
                max_child_w = max_child_w.max(cb.margin_rect.width);
            }
        }
        let scroll_w = max_child_w.max(content_width);
        let scroll_h = child_y.max(content_height);
        if scroll_w > content_width || scroll_h > content_height {
            Some(Size::new(scroll_w, scroll_h))
        } else {
            None
        }
    } else {
        None
    };

    if let Some(b) = tree.get_mut(box_id) {
        // Calculate effective margins after parent-child collapsing
        // If parent's top margin can collapse with first child's top margin,
        // the effective margin is the max of the two (margin transfer)
        let effective_mar_top = if can_collapse_top {
            if let Some(child_top) = first_child_margin_top {
                collapse_margins(mar_top, child_top)
            } else {
                mar_top
            }
        } else {
            mar_top
        };

        // Similarly for bottom margin collapsing
        let effective_mar_bottom = if can_collapse_bottom {
            if let Some(child_bottom) = last_child_margin_bottom {
                collapse_margins(mar_bottom, child_bottom)
            } else {
                mar_bottom
            }
        } else {
            mar_bottom
        };

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
            b.border_rect.y - effective_mar_top,
            b.border_rect.width + mar_left + mar_right,
            b.border_rect.height + effective_mar_top + effective_mar_bottom,
        );
        b.scroll_size = scroll_size;
    }

    box_id
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

/// Returns `true` if the given display value is inline-level (CSS 2.1 §9.2.1).
///
/// Inline-level elements generate inline-level boxes that participate in an
/// inline formatting context.  Everything else is block-level for the purpose
/// of anonymous block box generation.
fn is_inline_level(display: Display) -> bool {
    matches!(
        display,
        Display::Inline
            | Display::InlineBlock
            | Display::InlineFlex
            | Display::InlineGrid
            | Display::Ruby
            | Display::RubyText
    )
}

/// Check whether a node's children mix inline-level and block-level display
/// types (ignoring `display: none`, absolutely positioned, and floated
/// children, which do not participate in normal flow).
///
/// Returns `true` when at least one normal-flow child is inline-level and at
/// least one is block-level.  When this is the case, CSS 2.1 §9.2.1.1
/// requires that consecutive runs of inline content be wrapped in anonymous
/// block boxes.
fn has_mixed_inline_block_children(
    doc: &Document,
    parent: NodeId,
    styles: &StyleMap,
) -> bool {
    let children = doc.children(parent);
    let mut has_inline = false;
    let mut has_block = false;
    for &child_id in children {
        // Text nodes are always inline-level.
        if let Some(node) = doc.get(child_id) {
            if node.is_text() {
                has_inline = true;
                if has_block {
                    return true;
                }
                continue;
            }
        }
        let child_style = match styles.get(child_id) {
            Some(s) => s,
            None => continue,
        };
        // Skip out-of-flow children — they don't affect anonymous box creation.
        if child_style.display == Display::None
            || matches!(child_style.position, Position::Absolute | Position::Fixed)
            || child_style.float != Float::None
        {
            continue;
        }
        if is_inline_level(child_style.display) {
            has_inline = true;
        } else {
            has_block = true;
        }
        if has_inline && has_block {
            return true;
        }
    }
    false
}

/// CSS margin collapsing: when two vertical margins meet, they collapse into one.
/// For positive margins: the collapsed margin = max of the two.
/// For negative margins: the collapsed margin = min of the two (most negative).
/// For mixed: collapsed = max(positive) + min(negative).
fn collapse_margins(a: f32, b: f32) -> f32 {
    if a >= 0.0 && b >= 0.0 {
        a.max(b)
    } else if a < 0.0 && b < 0.0 {
        a.min(b)
    } else {
        // Mixed: add them (larger positive + negative = net)
        a + b
    }
}

/// Generate the marker text for a list item at the given 1-based index.
fn list_marker_text(style_type: &ListStyleType, index: usize) -> String {
    match style_type {
        ListStyleType::None => String::new(),
        ListStyleType::Disc => "\u{2022} ".to_string(),    // •
        ListStyleType::Circle => "\u{25E6} ".to_string(),  // ◦
        ListStyleType::Square => "\u{25AA} ".to_string(),  // ▪
        ListStyleType::Decimal => format!("{}. ", index),
        ListStyleType::DecimalLeadingZero => format!("{:02}. ", index),
        ListStyleType::LowerRoman | ListStyleType::LowerLatin => {
            format!("{}. ", to_lower_roman(index))
        }
        ListStyleType::UpperRoman | ListStyleType::UpperLatin => {
            format!("{}. ", to_lower_roman(index).to_uppercase())
        }
        ListStyleType::LowerAlpha => {
            let ch = (b'a' + ((index - 1) % 26) as u8) as char;
            format!("{}. ", ch)
        }
        ListStyleType::UpperAlpha => {
            let ch = (b'A' + ((index - 1) % 26) as u8) as char;
            format!("{}. ", ch)
        }
    }
}

/// Convert a number to lowercase Roman numerals.
fn to_lower_roman(mut n: usize) -> String {
    let values = [
        (1000, "m"), (900, "cm"), (500, "d"), (400, "cd"),
        (100, "c"), (90, "xc"), (50, "l"), (40, "xl"),
        (10, "x"), (9, "ix"), (5, "v"), (4, "iv"), (1, "i"),
    ];
    let mut result = String::new();
    for &(val, sym) in &values {
        while n >= val {
            result.push_str(sym);
            n -= val;
        }
    }
    result
}
