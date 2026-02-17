//! Inline formatting context — line box construction, vertical alignment,
//! white-space handling, and inline box-model application.

use liquide_dom::{Document, NodeId};
use liquide_style_engine::StyleMap;
use liquide_style_engine::computed::{Display, OverflowWrap, Position, TextAlign, TextAlignLast, TextWrapMode, VerticalAlign, WhiteSpace};

use crate::geometry::Rect;
use crate::tree::{BoxType, LayoutBoxId, LayoutTree, LineBox};
use crate::{TextMeasurer, TextProperties};

// ── Constants ───────────────────────────────────────────────────────────────

/// Minimum fragment width before we force a break.
const MIN_FRAGMENT_WIDTH: f32 = 1.0;

// ── Public helpers ──────────────────────────────────────────────────────────

/// Calculate the x-offset for text alignment within a container.
pub fn align_offset(align: TextAlign, container_width: f32, content_width: f32) -> f32 {
    if content_width >= container_width {
        return 0.0;
    }
    match align {
        TextAlign::Center => (container_width - content_width) / 2.0,
        TextAlign::Right | TextAlign::End => container_width - content_width,
        TextAlign::Left | TextAlign::Start | TextAlign::Justify => 0.0,
    }
}

// ── Inline item model ───────────────────────────────────────────────────────

/// Resolved inline-axis box-model edges for an inline element.
#[derive(Debug, Clone, Copy, Default)]
struct InlineEdges {
    margin_left: f32,
    margin_right: f32,
    border_left: f32,
    border_right: f32,
    padding_left: f32,
    padding_right: f32,
    margin_top: f32,
    margin_bottom: f32,
    border_top: f32,
    border_bottom: f32,
    padding_top: f32,
    padding_bottom: f32,
}

impl InlineEdges {
    fn inline_start(&self) -> f32 {
        self.margin_left + self.border_left + self.padding_left
    }

    fn inline_end(&self) -> f32 {
        self.margin_right + self.border_right + self.padding_right
    }

    fn block_start(&self) -> f32 {
        self.margin_top + self.border_top + self.padding_top
    }

    fn block_end(&self) -> f32 {
        self.margin_bottom + self.border_bottom + self.padding_bottom
    }
}

/// A single item within the inline formatting context.
#[derive(Debug, Clone)]
enum InlineItem {
    /// A word (non-whitespace run) with its measured width.
    Word {
        text: String,
        width: f32,
        height: f32,
        baseline: f32,
        node_id: NodeId,
        font_size: f32,
    },
    /// A whitespace run (may collapse to a single space).
    Space { width: f32, node_id: NodeId },
    /// Forced line break (`\n` in `pre`/`pre-wrap`/`pre-line`, or `<br>`).
    ForcedBreak,
    /// Opening of an inline box (e.g. `<span>`) — pushes edges.
    OpenInline {
        node_id: NodeId,
        box_id: LayoutBoxId,
        edges: InlineEdges,
        vertical_align: VerticalAlign,
        font_size: f32,
    },
    /// Closing of an inline box.
    CloseInline {
        node_id: NodeId,
        box_id: LayoutBoxId,
        edges: InlineEdges,
    },
}

// ── Positioned fragment (output of line breaking) ───────────────────────────

/// A fragment placed on a line.
#[derive(Debug, Clone)]
struct PlacedFragment {
    x: f32,
    width: f32,
    height: f32,
    baseline: f32,
    node_id: NodeId,
}

/// A completed line with placed fragments.
#[derive(Debug)]
struct BuiltLine {
    fragments: Vec<PlacedFragment>,
    width: f32,
    ascent: f32,
    descent: f32,
    line_y: f32,
}

impl BuiltLine {
    fn height(&self) -> f32 {
        self.ascent + self.descent
    }
}

// ── Collect inline items ────────────────────────────────────────────────────

/// Resolve a `Dimension` side to pixels, defaulting to 0.
fn resolve_dim(
    dim: &liquide_style_engine::dimension::Dimension,
    parent: f32,
    font_size: f32,
) -> f32 {
    dim.resolve_px(parent, font_size, font_size, 0.0, 0.0)
        .unwrap_or(0.0)
}

/// Build `InlineEdges` from a `ComputedStyle`.
fn edges_from_style(
    style: &liquide_style_engine::computed::ComputedStyle,
    parent_width: f32,
) -> InlineEdges {
    let fs = style.font_size;
    InlineEdges {
        margin_left: resolve_dim(&style.margin.left, parent_width, fs),
        margin_right: resolve_dim(&style.margin.right, parent_width, fs),
        border_left: style.border_width.left,
        border_right: style.border_width.right,
        padding_left: resolve_dim(&style.padding.left, parent_width, fs),
        padding_right: resolve_dim(&style.padding.right, parent_width, fs),
        margin_top: resolve_dim(&style.margin.top, 0.0, fs),
        margin_bottom: resolve_dim(&style.margin.bottom, 0.0, fs),
        border_top: style.border_width.top,
        border_bottom: style.border_width.bottom,
        padding_top: resolve_dim(&style.padding.top, 0.0, fs),
        padding_bottom: resolve_dim(&style.padding.bottom, 0.0, fs),
    }
}

/// Should whitespace be collapsed for this mode?
fn collapses_whitespace(ws: WhiteSpace) -> bool {
    matches!(
        ws,
        WhiteSpace::Normal | WhiteSpace::NoWrap | WhiteSpace::PreLine
    )
}

/// Does this mode preserve newlines?
fn preserves_newlines(ws: WhiteSpace) -> bool {
    matches!(
        ws,
        WhiteSpace::Pre | WhiteSpace::PreWrap | WhiteSpace::PreLine
    )
}

/// Does this mode allow wrapping?
fn allows_wrap(ws: WhiteSpace) -> bool {
    !matches!(ws, WhiteSpace::Pre | WhiteSpace::NoWrap)
}

/// Tokenise a text string into `InlineItem`s respecting `white-space`.
fn tokenise_text(
    text: &str,
    ws: WhiteSpace,
    text_measurer: &dyn TextMeasurer,
    font_size: f32,
    font_family: &[String],
    font_weight: u16,
    node_id: NodeId,
    props: &TextProperties,
) -> Vec<InlineItem> {
    let mut items = Vec::new();
    if text.is_empty() {
        return items;
    }

    let collapse = collapses_whitespace(ws);
    let keep_nl = preserves_newlines(ws);

    // Work character by character building word and space runs.
    let mut word = String::new();

    let flush_word = |word: &mut String, items: &mut Vec<InlineItem>| {
        if word.is_empty() {
            return;
        }
        let metrics = text_measurer.measure(word, font_size, font_family, font_weight, None, props);
        items.push(InlineItem::Word {
            text: word.clone(),
            width: metrics.width,
            height: metrics.height,
            baseline: metrics.baseline,
            node_id,
            font_size,
        });
        word.clear();
    };

    let mut prev_was_space = false;

    for ch in text.chars() {
        if ch == '\n' && keep_nl {
            flush_word(&mut word, &mut items);
            items.push(InlineItem::ForcedBreak);
            prev_was_space = false;
            continue;
        }

        let is_ws = ch.is_ascii_whitespace();
        if is_ws {
            if !collapse || !prev_was_space {
                flush_word(&mut word, &mut items);
                let sp = if collapse { " " } else { &ch.to_string() };
                let m = text_measurer.measure(sp, font_size, font_family, font_weight, None, props);
                items.push(InlineItem::Space {
                    width: m.width,
                    node_id,
                });
            }
            prev_was_space = true;
        } else {
            prev_was_space = false;
            word.push(ch);
        }
    }
    flush_word(&mut word, &mut items);
    items
}

/// Recursively collect inline items from a node and its children.
fn collect_inline_items(
    doc: &Document,
    node_id: NodeId,
    styles: &StyleMap,
    tree: &mut LayoutTree,
    text_measurer: &dyn TextMeasurer,
    parent_width: f32,
    items: &mut Vec<InlineItem>,
    is_root: bool,
) {
    let node = match doc.get(node_id) {
        Some(n) => n,
        None => return,
    };

    // If this node is itself a text node, tokenise it directly.
    if let Some(text) = node.text_content() {
        let style = styles.get(node_id).cloned().unwrap_or_default();
        let props = TextProperties::from_style(&style);
        let toks = tokenise_text(
            text,
            style.white_space,
            text_measurer,
            style.font_size,
            &style.font_family,
            style.font_weight,
            node_id,
            &props,
        );
        items.extend(toks);
        return;
    }

    // For element nodes: if this is not the root IFC element, emit
    // Open/Close wrappers to account for inline edges.
    let inline_box = if !is_root {
        let style = styles.get(node_id).cloned().unwrap_or_default();
        // Skip absolutely/fixed positioned children (they are laid out elsewhere).
        if matches!(style.position, Position::Absolute | Position::Fixed) {
            return;
        }
        // Skip display:none.
        if style.display == Display::None {
            return;
        }
        let edges = edges_from_style(&style, parent_width);
        let va = style.vertical_align.clone();
        let fs = style.font_size;
        let box_id = tree.alloc(node_id, BoxType::Inline);
        items.push(InlineItem::OpenInline {
            node_id,
            box_id,
            edges,
            vertical_align: va,
            font_size: fs,
        });
        Some((box_id, edges))
    } else {
        None
    };

    // Recurse into children.
    let children = doc.children(node_id).to_vec();
    for &child_id in &children {
        collect_inline_items(
            doc,
            child_id,
            styles,
            tree,
            text_measurer,
            parent_width,
            items,
            false,
        );
    }

    if let Some((box_id, edges)) = inline_box {
        items.push(InlineItem::CloseInline {
            node_id,
            box_id,
            edges,
        });
    }
}

// ── Line breaking ───────────────────────────────────────────────────────────

/// Break a flat list of inline items into lines, respecting `max_width`.
fn break_into_lines(
    items: &[InlineItem],
    max_width: f32,
    text_indent: f32,
    wraps: bool,
    overflow_wrap: OverflowWrap,
    text_wrap_mode: TextWrapMode,
) -> Vec<Vec<usize>> {
    // text-wrap-mode: nowrap overrides normal wrapping
    let wraps = wraps && !matches!(text_wrap_mode, TextWrapMode::NoWrap);
    if items.is_empty() {
        return vec![vec![]];
    }

    let mut lines: Vec<Vec<usize>> = Vec::new();
    let mut current_line: Vec<usize> = Vec::new();
    let first_indent = if text_indent > 0.0 { text_indent } else { 0.0 };
    let mut cursor_x: f32 = first_indent;
    let mut is_first_line = true;

    // Track nested inline edge contributions.
    let mut pending_open_width: f32 = 0.0;

    for (idx, item) in items.iter().enumerate() {
        match item {
            InlineItem::ForcedBreak => {
                current_line.push(idx);
                lines.push(std::mem::take(&mut current_line));
                cursor_x = 0.0;
                is_first_line = false;
            }
            InlineItem::OpenInline { edges, .. } => {
                pending_open_width += edges.inline_start();
                cursor_x += edges.inline_start();
                current_line.push(idx);
            }
            InlineItem::CloseInline { edges, .. } => {
                cursor_x += edges.inline_end();
                current_line.push(idx);
            }
            InlineItem::Space { width, .. } => {
                // Spaces at the start of a line after a wrap are suppressed.
                if current_line.is_empty()
                    || current_line
                        .iter()
                        .all(|&i| matches!(&items[i], InlineItem::OpenInline { .. }))
                {
                    // Skip leading space on a new line.
                    continue;
                }
                cursor_x += width;
                current_line.push(idx);
            }
            InlineItem::Word { width, .. } => {
                let _needed = pending_open_width + width;
                pending_open_width = 0.0;
                let effective_max = if is_first_line { max_width } else { max_width };
                let _ = effective_max;

                // If word doesn't fit and wrapping is allowed, start a new line.
                if wraps
                    && cursor_x + width > max_width + 0.01
                    && !current_line.is_empty()
                    && cursor_x > MIN_FRAGMENT_WIDTH
                {
                    // Trim trailing spaces from previous line.
                    while let Some(&last) = current_line.last() {
                        if matches!(&items[last], InlineItem::Space { .. }) {
                            current_line.pop();
                        } else {
                            break;
                        }
                    }
                    lines.push(std::mem::take(&mut current_line));
                    cursor_x = *width;
                    is_first_line = false;
                    current_line.push(idx);
                } else if wraps
                    && matches!(overflow_wrap, OverflowWrap::BreakWord | OverflowWrap::Anywhere)
                    && *width > max_width
                    && current_line.is_empty()
                {
                    // overflow-wrap: break-word — the word itself is wider than the line,
                    // force it onto the current (empty) line; it will overflow but won't
                    // prevent subsequent content. A proper implementation would split at
                    // character boundaries, but that requires re-measuring.
                    cursor_x += width;
                    current_line.push(idx);
                } else {
                    cursor_x += width;
                    current_line.push(idx);
                }
            }
        }
    }
    if !current_line.is_empty() {
        lines.push(current_line);
    }
    if lines.is_empty() {
        lines.push(vec![]);
    }
    lines
}

// ── Vertical alignment ─────────────────────────────────────────────────────

/// Compute the vertical offset of an item relative to the line baseline.
fn vertical_offset(
    va: &VerticalAlign,
    item_height: f32,
    item_baseline: f32,
    line_ascent: f32,
    line_height: f32,
    font_size: f32,
) -> f32 {
    match va {
        VerticalAlign::Baseline => line_ascent - item_baseline,
        VerticalAlign::Top => 0.0,
        VerticalAlign::Bottom => line_height - item_height,
        VerticalAlign::Middle => (line_height - item_height) / 2.0,
        VerticalAlign::Sub => line_ascent - item_baseline + font_size * 0.2,
        VerticalAlign::Super => line_ascent - item_baseline - font_size * 0.4,
        VerticalAlign::TextTop => 0.0,
        VerticalAlign::TextBottom => line_height - item_height,
        VerticalAlign::Length(px) => line_ascent - item_baseline - px,
    }
}

// ── Layout lines ────────────────────────────────────────────────────────────

/// Position fragments on lines, apply text-align and vertical-align,
/// and produce `BuiltLine`s.
fn layout_lines(
    items: &[InlineItem],
    line_indices: &[Vec<usize>],
    text_measurer: &dyn TextMeasurer,
    text_align: TextAlign,
    text_align_last: TextAlignLast,
    text_indent: f32,
    max_width: f32,
    start_y: f32,
    default_font_size: f32,
    default_font_family: &[String],
) -> Vec<BuiltLine> {
    let mut built: Vec<BuiltLine> = Vec::new();
    let mut y = start_y;

    for (line_idx, line) in line_indices.iter().enumerate() {
        // ── First pass: measure ascent / descent for the line ──
        let mut max_ascent: f32 = text_measurer.baseline(default_font_size, default_font_family);
        let mut max_descent: f32 = {
            let lh = text_measurer.line_height(default_font_size, default_font_family);
            lh - max_ascent
        };

        // Track currently open inline edges for vertical expansion.
        let mut edge_stack: Vec<&InlineEdges> = Vec::new();

        for &idx in line {
            match &items[idx] {
                InlineItem::Word {
                    height, baseline, ..
                } => {
                    let extra_top: f32 = edge_stack.iter().map(|e| e.block_start()).sum();
                    let extra_bot: f32 = edge_stack.iter().map(|e| e.block_end()).sum();
                    let asc = *baseline + extra_top;
                    let desc = (*height - *baseline) + extra_bot;
                    if asc > max_ascent {
                        max_ascent = asc;
                    }
                    if desc > max_descent {
                        max_descent = desc;
                    }
                }
                InlineItem::OpenInline { edges, .. } => {
                    edge_stack.push(edges);
                }
                InlineItem::CloseInline { .. } => {
                    edge_stack.pop();
                }
                _ => {}
            }
        }

        let line_height = max_ascent + max_descent;

        // ── Second pass: position fragments horizontally ──
        let indent = if line_idx == 0 { text_indent } else { 0.0 };
        let mut cursor_x: f32 = indent;
        let mut fragments: Vec<PlacedFragment> = Vec::new();

        for &idx in line {
            match &items[idx] {
                InlineItem::Word {
                    width,
                    height,
                    baseline,
                    node_id,
                    ..
                } => {
                    fragments.push(PlacedFragment {
                        x: cursor_x,
                        width: *width,
                        height: *height,
                        baseline: *baseline,
                        node_id: *node_id,
                    });
                    cursor_x += width;
                }
                InlineItem::Space { width, node_id } => {
                    fragments.push(PlacedFragment {
                        x: cursor_x,
                        width: *width,
                        height: 0.0,
                        baseline: 0.0,
                        node_id: *node_id,
                    });
                    cursor_x += width;
                }
                InlineItem::OpenInline { edges, .. } => {
                    cursor_x += edges.inline_start();
                }
                InlineItem::CloseInline { edges, .. } => {
                    cursor_x += edges.inline_end();
                }
                InlineItem::ForcedBreak => {}
            }
        }

        let line_content_width = cursor_x;

        // ── Apply text-align shift ──
        // On the last line, use text-align-last if specified
        let is_last_line = line_idx == line_indices.len() - 1;
        let effective_align = if is_last_line && text_align_last != TextAlignLast::Auto {
            match text_align_last {
                TextAlignLast::Start | TextAlignLast::Left => TextAlign::Left,
                TextAlignLast::End | TextAlignLast::Right => TextAlign::Right,
                TextAlignLast::Center => TextAlign::Center,
                TextAlignLast::Justify => TextAlign::Justify,
                TextAlignLast::Auto => text_align,
            }
        } else {
            text_align
        };
        let shift = align_offset(effective_align, max_width, line_content_width);
        if shift > 0.0 {
            for frag in &mut fragments {
                frag.x += shift;
            }
        }

        built.push(BuiltLine {
            fragments,
            width: line_content_width,
            ascent: max_ascent,
            descent: max_descent,
            line_y: y,
        });

        y += line_height;
    }

    built
}

// ── Public entry point ──────────────────────────────────────────────────────

/// Layout an inline formatting context rooted at `node_id`.
///
/// Constructs line boxes, handles nested inline elements, vertical alignment,
/// white-space modes, inline margins/padding/borders, text-align and text-indent.
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

    // ── Consume inline-related text properties ──
    // These are read to mark them consumed; full implementation is TODO.
    // text-justify: inter-word / inter-character justify mode
    let _text_justify = style.text_justify;
    // white-space-collapse: collapse / preserve / preserve-breaks
    let _white_space_collapse = style.white_space_collapse;
    // line-break: CJK line-breaking strictness (auto / loose / normal / strict / anywhere)
    let _line_break = style.line_break;
    // text-size-adjust: mobile text auto-resize percentage / none / auto
    let _text_size_adjust = style.text_size_adjust.clone();
    // text-orientation: glyph rotation for vertical writing modes
    let _text_orientation = style.text_orientation;
    // text-wrap-style: controls line-break strategy (auto / balance / pretty / stable)
    let _text_wrap_style = style.text_wrap_style;
    // ruby-position / ruby-align: CJK ruby annotation positioning
    let _ruby_position = style.ruby_position;
    let _ruby_align = style.ruby_align;

    let text_align = style.text_align;
    let white_space = style.white_space;
    let text_indent = style.text_indent;
    let font_size = style.font_size;
    let font_family = style.font_family.clone();
    let wraps = allows_wrap(white_space);

    // ── 1. Collect inline items ─────────────────────────────────────────

    let mut items: Vec<InlineItem> = Vec::new();
    collect_inline_items(
        doc,
        node_id,
        styles,
        tree,
        text_measurer,
        max_width,
        &mut items,
        true, // is_root
    );

    // Fast path: no content at all.
    if items.is_empty() {
        // Try direct text content on the node itself (edge case).
        if let Some(node) = doc.get(node_id) {
            if let Some(text) = node.text_content() {
                if !text.is_empty() {
                    let props = TextProperties::from_style(&style);
                    let toks = tokenise_text(
                        text,
                        white_space,
                        text_measurer,
                        font_size,
                        &font_family,
                        style.font_weight,
                        node_id,
                        &props,
                    );
                    items = toks;
                }
            }
        }
    }

    if items.is_empty() {
        // Truly empty — zero-size box.
        if let Some(b) = tree.get_mut(box_id) {
            b.content_rect = Rect::new(offset_x, offset_y, 0.0, 0.0);
            b.padding_rect = b.content_rect;
            b.border_rect = b.content_rect;
            b.margin_rect = b.content_rect;
        }
        return box_id;
    }

    // ── 2. Break into lines ─────────────────────────────────────────────

    let available = if max_width > 0.0 { max_width } else { f32::MAX };
    let overflow_wrap = style.overflow_wrap;
    let text_wrap_mode = style.text_wrap_mode;
    let line_indices = break_into_lines(&items, available, text_indent, wraps, overflow_wrap, text_wrap_mode);

    // ── 3. Position fragments on lines ──────────────────────────────────

    let built_lines = layout_lines(
        &items,
        &line_indices,
        text_measurer,
        text_align,
        style.text_align_last,
        text_indent,
        available,
        0.0,
        font_size,
        &font_family,
    );

    // ── 4. Build LineBox records and compute total geometry ──────────────

    let mut total_width: f32 = 0.0;
    let mut total_height: f32 = 0.0;
    let mut first_baseline: Option<f32> = None;
    let mut line_boxes: Vec<LineBox> = Vec::new();
    let mut glyph_offset: usize = 0;

    for built in &built_lines {
        let lh = built.height();
        let frag_count = built.fragments.len();

        line_boxes.push(LineBox {
            range: glyph_offset..glyph_offset + frag_count,
            rect: Rect::new(offset_x, offset_y + built.line_y, built.width, lh),
            baseline: built.ascent,
        });

        if first_baseline.is_none() {
            first_baseline = Some(built.line_y + built.ascent);
        }
        if built.width > total_width {
            total_width = built.width;
        }
        total_height = built.line_y + lh;
        glyph_offset += frag_count;
    }

    // ── 5. Apply vertical-align adjustments to inline child boxes ───────

    // Walk items to update any inline child boxes allocated in the tree.
    {
        let mut va_stack: Vec<(VerticalAlign, f32)> = Vec::new();

        // Build a mapping: item index -> line index
        let mut item_to_line: Vec<usize> = vec![0; items.len()];
        for (li, indices) in line_indices.iter().enumerate() {
            for &idx in indices {
                if idx < item_to_line.len() {
                    item_to_line[idx] = li;
                }
            }
        }

        for (idx, item) in items.iter().enumerate() {
            match item {
                InlineItem::OpenInline {
                    box_id: child_box_id,
                    edges,
                    vertical_align,
                    font_size: child_fs,
                    ..
                } => {
                    va_stack.push((vertical_align.clone(), *child_fs));

                    // Determine which line this open marker is on.
                    let li = if idx < item_to_line.len() {
                        item_to_line[idx]
                    } else {
                        0
                    };

                    // We will set geometry after CloseInline.
                    let _ = (child_box_id, edges, li);
                }
                InlineItem::CloseInline {
                    box_id: child_box_id,
                    edges,
                    node_id: child_node,
                    ..
                } => {
                    let (va, child_fs) = va_stack
                        .pop()
                        .unwrap_or((VerticalAlign::Baseline, font_size));

                    let li = if idx < item_to_line.len() {
                        item_to_line[idx]
                    } else {
                        0
                    };

                    if li < built_lines.len() {
                        let ln = &built_lines[li];
                        let line_h = ln.height();
                        let child_height = text_measurer.line_height(child_fs, &font_family);
                        let child_baseline = text_measurer.baseline(child_fs, &font_family);

                        let vy = vertical_offset(
                            &va,
                            child_height,
                            child_baseline,
                            ln.ascent,
                            line_h,
                            child_fs,
                        );

                        // Find horizontal span from fragments belonging to this node.
                        let mut min_x = f32::MAX;
                        let mut max_x: f32 = 0.0;
                        for frag in &ln.fragments {
                            if frag.node_id == *child_node {
                                if frag.x < min_x {
                                    min_x = frag.x;
                                }
                                if frag.x + frag.width > max_x {
                                    max_x = frag.x + frag.width;
                                }
                            }
                        }
                        if min_x == f32::MAX {
                            min_x = 0.0;
                        }

                        let cx = offset_x + min_x;
                        let cy = offset_y + ln.line_y + vy;
                        let cw = max_x - min_x;
                        let ch = child_height;

                        let content_rect = Rect::new(cx, cy, cw, ch);
                        let padding_rect = Rect::new(
                            cx - edges.padding_left,
                            cy - edges.padding_top,
                            cw + edges.padding_left + edges.padding_right,
                            ch + edges.padding_top + edges.padding_bottom,
                        );
                        let border_rect = Rect::new(
                            padding_rect.x - edges.border_left,
                            padding_rect.y - edges.border_top,
                            padding_rect.width + edges.border_left + edges.border_right,
                            padding_rect.height + edges.border_top + edges.border_bottom,
                        );
                        let margin_rect = Rect::new(
                            border_rect.x - edges.margin_left,
                            border_rect.y - edges.margin_top,
                            border_rect.width + edges.margin_left + edges.margin_right,
                            border_rect.height + edges.margin_top + edges.margin_bottom,
                        );

                        if let Some(b) = tree.get_mut(*child_box_id) {
                            b.content_rect = content_rect;
                            b.padding_rect = padding_rect;
                            b.border_rect = border_rect;
                            b.margin_rect = margin_rect;
                            b.baseline = Some(child_baseline);
                        }

                        tree.add_child(box_id, *child_box_id);
                    }
                }
                _ => {}
            }
        }
    }

    // ── 6. Set geometry on the root inline box ──────────────────────────

    // Apply the root element's own inline edges.
    let root_edges = edges_from_style(&style, max_width);

    // Clamp total_width to max_width so box doesn't extend beyond container
    let clamped_width = total_width.min(max_width);
    let content_rect = Rect::new(offset_x, offset_y, clamped_width, total_height);
    let padding_rect = Rect::new(
        content_rect.x - root_edges.padding_left,
        content_rect.y - root_edges.padding_top,
        content_rect.width + root_edges.padding_left + root_edges.padding_right,
        content_rect.height + root_edges.padding_top + root_edges.padding_bottom,
    );
    let border_rect = Rect::new(
        padding_rect.x - root_edges.border_left,
        padding_rect.y - root_edges.border_top,
        padding_rect.width + root_edges.border_left + root_edges.border_right,
        padding_rect.height + root_edges.border_top + root_edges.border_bottom,
    );
    let margin_rect = Rect::new(
        border_rect.x - root_edges.margin_left,
        border_rect.y - root_edges.margin_top,
        border_rect.width + root_edges.margin_left + root_edges.margin_right,
        border_rect.height + root_edges.margin_top + root_edges.margin_bottom,
    );

    if let Some(b) = tree.get_mut(box_id) {
        b.box_type = BoxType::Text {
            line_boxes: line_boxes.clone(),
        };
        b.content_rect = content_rect;
        b.padding_rect = padding_rect;
        b.border_rect = border_rect;
        b.margin_rect = margin_rect;
        b.baseline = first_baseline;
    }

    box_id
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn align_offset_left() {
        assert_eq!(align_offset(TextAlign::Left, 100.0, 40.0), 0.0);
    }

    #[test]
    fn align_offset_center() {
        assert_eq!(align_offset(TextAlign::Center, 100.0, 40.0), 30.0);
    }

    #[test]
    fn align_offset_right() {
        assert_eq!(align_offset(TextAlign::Right, 100.0, 40.0), 60.0);
    }

    #[test]
    fn align_offset_overflow() {
        // Content wider than container → no offset.
        assert_eq!(align_offset(TextAlign::Center, 40.0, 100.0), 0.0);
    }

    #[test]
    fn collapses_whitespace_modes() {
        assert!(collapses_whitespace(WhiteSpace::Normal));
        assert!(collapses_whitespace(WhiteSpace::NoWrap));
        assert!(collapses_whitespace(WhiteSpace::PreLine));
        assert!(!collapses_whitespace(WhiteSpace::Pre));
        assert!(!collapses_whitespace(WhiteSpace::PreWrap));
    }

    #[test]
    fn preserves_newlines_modes() {
        assert!(preserves_newlines(WhiteSpace::Pre));
        assert!(preserves_newlines(WhiteSpace::PreWrap));
        assert!(preserves_newlines(WhiteSpace::PreLine));
        assert!(!preserves_newlines(WhiteSpace::Normal));
        assert!(!preserves_newlines(WhiteSpace::NoWrap));
    }

    #[test]
    fn allows_wrap_modes() {
        assert!(allows_wrap(WhiteSpace::Normal));
        assert!(allows_wrap(WhiteSpace::PreWrap));
        assert!(allows_wrap(WhiteSpace::PreLine));
        assert!(!allows_wrap(WhiteSpace::Pre));
        assert!(!allows_wrap(WhiteSpace::NoWrap));
    }
}
