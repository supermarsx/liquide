//! `<lq-textarea>` — a multi-line editable text widget (P8 follow-up, t116).
//!
//! Where [`TextInput`](crate::input::TextInput) is a single-line field, this is a
//! full multi-line editor body: a buffer of lines, an optional line-number gutter,
//! a real in-flow caret positioned BY LAYOUT at a `(line, column)`, and vertical
//! scrolling when the content outgrows the viewport.
//!
//! ## Layout-derived geometry (the anti-constant guard)
//!
//! Everything spatial reads from the laid-out CSS boxes via [`LayoutQuery`], never
//! a px-per-line / px-per-char constant:
//!
//! - **Line rows** are real in-flow `<lq-textarea-row>` elements. Layout — not a
//!   constant stride — decides each row's y. The caret line's y therefore differs
//!   from another line's y because layout positioned the rows.
//! - **The caret** is a real `<lq-caret>` element rendered IN FLOW inside the caret
//!   line's row, between the text-before-caret and text-after-caret spans. Layout
//!   positions the caret box in BOTH axes: its x is the real glyph advance of the
//!   text before it; its y is the row's laid-out top. Reading
//!   `box_of_part(root, "caret")` returns the true on-screen caret rect.
//! - **Click-to-place-caret** resolves the line by finding which laid-out
//!   ROW BOX contains the click y (a per-line constant would mis-resolve once CSS
//!   changes the row height / gutter / padding), then the column from where the
//!   click x falls along that row's laid-out CONTENT box.
//!
//! ## Scrolling (the in-lock approach, mirrors `ScrollArea`)
//!
//! The engine's `scroll_offset` is a transient layout-tree field not persisted in
//! the DOM, and driving it needs a pipeline API outside this crate's lock (see
//! `scroll_area.rs`). So the scroll offset lives in WIDGET STATE: the content
//! block is translated UP by a negative `margin-top` while the viewport clips with
//! `overflow: hidden`. Vertical extent is derived from the laid-out content vs
//! viewport boxes — never a constant.
//!
//! Emits [`CHANGED_ACTION`] (the full text) on every edit.

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};
use liquide_layout::geometry::{Point, Rect};

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// The action name emitted when the text changes (carries the full buffer).
pub const CHANGED_ACTION: &str = "changed";

/// A multi-line editable text area with a caret and an optional line gutter.
#[derive(Debug, Clone)]
pub struct TextArea {
    /// The buffer, split into lines (never empty — at least one line, possibly "").
    lines: Vec<String>,
    /// Caret line index (0-based, always `< lines.len()`).
    caret_line: usize,
    /// Caret column as a BYTE offset into `lines[caret_line]` (on a char boundary).
    caret_col: usize,
    /// Placeholder shown when the whole buffer is empty.
    placeholder: String,
    /// Whether to render the line-number gutter.
    show_gutter: bool,
    disabled: bool,
    focused: bool,
    /// Vertical scroll offset (px from the top of the content), always clamped.
    scroll_y: f32,
}

impl TextArea {
    /// An empty text area with the given `placeholder`.
    pub fn new(placeholder: impl Into<String>) -> Self {
        Self {
            lines: vec![String::new()],
            caret_line: 0,
            caret_col: 0,
            placeholder: placeholder.into(),
            show_gutter: false,
            disabled: false,
            focused: false,
            scroll_y: 0.0,
        }
    }

    /// Seed the buffer with `text` (caret to the very end). Splits on `\n`.
    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.set_text_internal(&text.into());
        let last = self.lines.len() - 1;
        self.caret_line = last;
        self.caret_col = self.lines[last].len();
        self
    }

    /// Enable the line-number gutter.
    pub fn with_gutter(mut self, show: bool) -> Self {
        self.show_gutter = show;
        self
    }

    /// Mark disabled.
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    /// Whether the gutter is shown.
    pub fn gutter_visible(&self) -> bool {
        self.show_gutter
    }

    /// Toggle the gutter at runtime.
    pub fn set_gutter(&mut self, show: bool) {
        self.show_gutter = show;
    }

    /// The full buffer text (lines joined with `\n`).
    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    /// The number of lines.
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// The caret position as `(line, byte_column)`.
    pub fn caret(&self) -> (usize, usize) {
        (self.caret_line, self.caret_col)
    }

    /// Whether the field is focused.
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Set focus (the host calls this when the field is focused/blurred).
    pub fn set_focused(&mut self, f: bool) {
        self.focused = f;
    }

    /// The current vertical scroll offset.
    pub fn scroll_y(&self) -> f32 {
        self.scroll_y
    }

    fn set_text_internal(&mut self, text: &str) {
        self.lines = text.split('\n').map(|s| s.to_string()).collect();
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
    }

    fn current_line(&self) -> &str {
        &self.lines[self.caret_line]
    }

    /// Whether the whole buffer is empty (one empty line).
    fn is_empty(&self) -> bool {
        self.lines.len() == 1 && self.lines[0].is_empty()
    }

    // ── char-boundary helpers (within the caret line) ──────────────────────

    fn prev_boundary(line: &str, idx: usize) -> usize {
        if idx == 0 {
            return 0;
        }
        let mut i = idx - 1;
        while i > 0 && !line.is_char_boundary(i) {
            i -= 1;
        }
        i
    }

    fn next_boundary(line: &str, idx: usize) -> usize {
        if idx >= line.len() {
            return line.len();
        }
        let mut i = idx + 1;
        while i < line.len() && !line.is_char_boundary(i) {
            i += 1;
        }
        i
    }

    /// Clamp `col` to the nearest char boundary `<=` the current line length.
    fn clamp_col(line: &str, col: usize) -> usize {
        let col = col.min(line.len());
        if line.is_char_boundary(col) {
            col
        } else {
            Self::prev_boundary(line, col)
        }
    }

    fn changed(&self) -> WidgetOutcome {
        WidgetOutcome::action_with(CHANGED_ACTION, self.text())
    }

    // ── editing ────────────────────────────────────────────────────────────

    fn insert_char(&mut self, c: char) -> WidgetOutcome {
        let line = &mut self.lines[self.caret_line];
        line.insert(self.caret_col, c);
        self.caret_col += c.len_utf8();
        self.changed()
    }

    /// Enter: split the current line at the caret into two lines.
    fn newline(&mut self) -> WidgetOutcome {
        let line = &mut self.lines[self.caret_line];
        let rest = line.split_off(self.caret_col);
        self.lines.insert(self.caret_line + 1, rest);
        self.caret_line += 1;
        self.caret_col = 0;
        self.changed()
    }

    /// Backspace: delete the char before the caret; at column 0 JOIN with the
    /// previous line (caret lands at the join seam).
    fn backspace(&mut self) -> WidgetOutcome {
        if self.caret_col > 0 {
            let start = Self::prev_boundary(self.current_line(), self.caret_col);
            self.lines[self.caret_line].replace_range(start..self.caret_col, "");
            self.caret_col = start;
            return self.changed();
        }
        // Column 0: join with the previous line.
        if self.caret_line == 0 {
            return WidgetOutcome::Ignored;
        }
        let cur = self.lines.remove(self.caret_line);
        self.caret_line -= 1;
        let seam = self.lines[self.caret_line].len();
        self.lines[self.caret_line].push_str(&cur);
        self.caret_col = seam;
        self.changed()
    }

    /// Delete (forward): delete the char after the caret; at end-of-line JOIN the
    /// next line onto this one (caret stays at the seam).
    fn delete_forward(&mut self) -> WidgetOutcome {
        let len = self.current_line().len();
        if self.caret_col < len {
            let end = Self::next_boundary(self.current_line(), self.caret_col);
            self.lines[self.caret_line].replace_range(self.caret_col..end, "");
            return self.changed();
        }
        // End of line: join the next line onto this one.
        if self.caret_line + 1 >= self.lines.len() {
            return WidgetOutcome::Ignored;
        }
        let next = self.lines.remove(self.caret_line + 1);
        self.lines[self.caret_line].push_str(&next);
        self.changed()
    }

    // ── caret movement ──────────────────────────────────────────────────────

    fn move_left(&mut self) -> WidgetOutcome {
        if self.caret_col > 0 {
            self.caret_col = Self::prev_boundary(self.current_line(), self.caret_col);
            return WidgetOutcome::Changed;
        }
        // At column 0: wrap to the end of the previous line.
        if self.caret_line == 0 {
            return WidgetOutcome::Ignored;
        }
        self.caret_line -= 1;
        self.caret_col = self.current_line().len();
        WidgetOutcome::Changed
    }

    fn move_right(&mut self) -> WidgetOutcome {
        let len = self.current_line().len();
        if self.caret_col < len {
            self.caret_col = Self::next_boundary(self.current_line(), self.caret_col);
            return WidgetOutcome::Changed;
        }
        // At end of line: wrap to the start of the next line.
        if self.caret_line + 1 >= self.lines.len() {
            return WidgetOutcome::Ignored;
        }
        self.caret_line += 1;
        self.caret_col = 0;
        WidgetOutcome::Changed
    }

    fn move_up(&mut self) -> WidgetOutcome {
        if self.caret_line == 0 {
            // Already on the top line: jump to its start.
            if self.caret_col == 0 {
                return WidgetOutcome::Ignored;
            }
            self.caret_col = 0;
            return WidgetOutcome::Changed;
        }
        self.caret_line -= 1;
        self.caret_col = Self::clamp_col(self.current_line(), self.caret_col);
        WidgetOutcome::Changed
    }

    fn move_down(&mut self) -> WidgetOutcome {
        if self.caret_line + 1 >= self.lines.len() {
            // Already on the bottom line: jump to its end.
            let len = self.current_line().len();
            if self.caret_col == len {
                return WidgetOutcome::Ignored;
            }
            self.caret_col = len;
            return WidgetOutcome::Changed;
        }
        self.caret_line += 1;
        self.caret_col = Self::clamp_col(self.current_line(), self.caret_col);
        WidgetOutcome::Changed
    }

    fn home(&mut self) -> WidgetOutcome {
        if self.caret_col == 0 {
            return WidgetOutcome::Ignored;
        }
        self.caret_col = 0;
        WidgetOutcome::Changed
    }

    fn end(&mut self) -> WidgetOutcome {
        let len = self.current_line().len();
        if self.caret_col == len {
            return WidgetOutcome::Ignored;
        }
        self.caret_col = len;
        WidgetOutcome::Changed
    }

    /// Move the caret by `delta` lines (PageUp/PageDown), clamping the column.
    fn move_page(&mut self, delta: isize) -> WidgetOutcome {
        let target = (self.caret_line as isize + delta).clamp(0, self.lines.len() as isize - 1)
            as usize;
        if target == self.caret_line {
            // Clamp to the line ends instead of a no-op so the caret still moves.
            return if delta < 0 { self.home() } else { self.end() };
        }
        self.caret_line = target;
        self.caret_col = Self::clamp_col(self.current_line(), self.caret_col);
        WidgetOutcome::Changed
    }

    // ── scroll (in-lock margin-shim; extent from layout) ───────────────────

    /// `max(0, content_h - viewport_h)` from the laid-out boxes.
    fn max_scroll(root: NodeId, layout: &LayoutQuery) -> f32 {
        let (Some(viewport), Some(content)) = (
            layout.box_of_part(root, "viewport"),
            layout.box_of_part(root, "content"),
        ) else {
            return 0.0;
        };
        (content.height - viewport.height).max(0.0)
    }

    fn apply_scroll(&mut self, new_y: f32, max: f32) -> WidgetOutcome {
        let clamped = new_y.clamp(0.0, max);
        if (clamped - self.scroll_y).abs() < f32::EPSILON {
            return WidgetOutcome::Ignored;
        }
        self.scroll_y = clamped;
        WidgetOutcome::Changed
    }

    /// Keep the caret line visible: if its laid-out row sits outside the viewport,
    /// nudge `scroll_y` so it comes into view. Geometry from the laid-out boxes.
    fn ensure_caret_visible(&mut self, root: NodeId, layout: &LayoutQuery) {
        let (Some(viewport), Some(content)) = (
            layout.box_of_part(root, "viewport"),
            layout.box_of_part(root, "content"),
        ) else {
            return;
        };
        let Some(row) = self.caret_row_box(root, layout) else {
            return;
        };
        let max = (content.height - viewport.height).max(0.0);
        // The row's y already includes the current negative-margin translate, so
        // work in content-relative space: row top relative to the content top.
        let row_top_in_content = (row.y - content.y) + self.scroll_y;
        let row_bottom_in_content = row_top_in_content + row.height;
        let view_top = self.scroll_y;
        let view_bottom = self.scroll_y + viewport.height;
        let new_scroll = if row_top_in_content < view_top {
            row_top_in_content
        } else if row_bottom_in_content > view_bottom {
            row_bottom_in_content - viewport.height
        } else {
            self.scroll_y
        };
        self.scroll_y = new_scroll.clamp(0.0, max);
    }

    fn caret_row_box(&self, root: NodeId, layout: &LayoutQuery) -> Option<Rect> {
        layout.box_of_part(root, &format!("row-{}", self.caret_line))
    }

    // ── click-to-place-caret (line from row box y; column from row content) ──

    /// Resolve a click `(x, y)` to a `(line, byte_col)` from the LAID-OUT row
    /// boxes. The line is the row whose box contains `y` (a px-per-line constant
    /// would mis-resolve when CSS changes the row height / padding / gutter); the
    /// column is the char boundary nearest where `x` falls along that row's
    /// laid-out content box.
    fn caret_from_point(&self, root: NodeId, x: f32, y: f32, layout: &LayoutQuery) -> Option<(usize, usize)> {
        // Find the row whose laid-out box vertically contains the click. Each row
        // carries data-part="row-<i>" so it is addressed by identity, not stride.
        let mut hit_line = None;
        for i in 0..self.lines.len() {
            let part = format!("row-{i}");
            if let Some(rb) = layout.box_of_part(root, &part) {
                if y >= rb.y && y < rb.y + rb.height {
                    hit_line = Some((i, rb));
                    break;
                }
            }
        }
        // If the click fell above/below all rows (in the padding), clamp to the
        // nearest row by y using the laid-out boxes (still layout-derived).
        let (line, row_box) = match hit_line {
            Some(v) => v,
            None => {
                let mut best: Option<(usize, Rect, f32)> = None;
                for i in 0..self.lines.len() {
                    let part = format!("row-{i}");
                    if let Some(rb) = layout.box_of_part(root, &part) {
                        let cy = rb.y + rb.height / 2.0;
                        let d = (cy - y).abs();
                        if best.as_ref().map(|b| d < b.2).unwrap_or(true) {
                            best = Some((i, rb, d));
                        }
                    }
                }
                let (i, rb, _) = best?;
                (i, rb)
            }
        };
        let col = self.column_at_x(line, &row_box, x, root, layout);
        Some((line, col))
    }

    /// Resolve the column (byte offset) at horizontal `x` along a row, using the
    /// row's laid-out TEXT content box. We map x to a fraction of the laid-out
    /// text width and pick the nearest char boundary — derived from the real text
    /// box (a CSS padding/width/gutter change moves it), not a glyph constant.
    fn column_at_x(
        &self,
        line: usize,
        row_box: &Rect,
        x: f32,
        root: NodeId,
        layout: &LayoutQuery,
    ) -> usize {
        let text = &self.lines[line];
        if text.is_empty() {
            return 0;
        }
        // The text run extent: its LEFT is the row content's left edge; its WIDTH
        // is the sum of the laid-out widths of every `lq-textarea-text` span in
        // the row. On the caret line the run is split into before/after spans, so
        // we must sum them (a single span is only the before-caret slice). This is
        // the real glyph extent from layout, not a per-char constant — a CSS
        // change to the row padding/width/gutter moves it.
        let row_content = layout
            .content_of_part(root, &format!("row-{line}"))
            .unwrap_or(*row_box);
        let row_node = layout.find_part(root, &format!("row-{line}"));
        let text_w = row_node
            .map(|n| self.sum_text_span_widths(n, layout))
            .filter(|w| *w > 0.0)
            .unwrap_or(row_content.width);
        let text_box = Rect::new(row_content.x, row_content.y, text_w, row_content.height);
        let frac = LayoutQuery::fraction_along_x(text_box, Point::new(x, text_box.y));
        let char_count = text.chars().count();
        let target = (frac * char_count as f32).round() as usize;
        // Map the target char index to a byte offset on a char boundary.
        text.char_indices()
            .nth(target)
            .map(|(b, _)| b)
            .unwrap_or(text.len())
    }

    /// Sum the laid-out widths of all `lq-textarea-text` spans under `row_node`
    /// (depth-first) — the real on-screen glyph-run width of the line.
    fn sum_text_span_widths(&self, row_node: NodeId, layout: &LayoutQuery) -> f32 {
        fn walk(doc: &liquide_dom::Document, hit: &LayoutQuery, node: NodeId, acc: &mut f32) {
            if doc.tag_name(node).as_deref() == Some("lq-textarea-text") {
                if let Some(b) = hit.box_of(node) {
                    *acc += b.width;
                }
            }
            for &child in doc.children(node) {
                walk(doc, hit, child, acc);
            }
        }
        let mut acc = 0.0;
        walk(layout.doc(), layout, row_node, &mut acc);
        acc
    }
}

impl WidgetBehavior for TextArea {
    fn kind(&self) -> WidgetKind {
        WidgetKind::Input
    }

    fn wanted_events(&self) -> Vec<DomEventKind> {
        vec![
            DomEventKind::MouseEnter,
            DomEventKind::MouseLeave,
            DomEventKind::Click {
                button: MouseButton::Left,
                x: 0.0,
                y: 0.0,
            },
            DomEventKind::Scroll { dx: 0.0, dy: 0.0 },
        ]
    }

    fn on_dom_event(
        &mut self,
        root: NodeId,
        event: &DomEvent,
        layout: &LayoutQuery,
    ) -> WidgetOutcome {
        if self.disabled {
            return WidgetOutcome::Ignored;
        }
        match &event.kind {
            DomEventKind::Click {
                button: MouseButton::Left,
                x,
                y,
            } => {
                // Only react to clicks inside the laid-out widget box.
                let inside = layout
                    .box_of(root)
                    .map(|r| r.contains(Point::new(*x, *y)))
                    .unwrap_or(false);
                if !inside {
                    return WidgetOutcome::Ignored;
                }
                let was_focused = self.focused;
                self.focused = true;
                // Place the caret at the clicked line/column from the laid-out rows.
                if let Some((line, col)) = self.caret_from_point(root, *x, *y, layout) {
                    self.caret_line = line;
                    self.caret_col = col;
                    self.ensure_caret_visible(root, layout);
                    return WidgetOutcome::Changed;
                }
                if !was_focused {
                    return WidgetOutcome::Changed;
                }
                WidgetOutcome::Ignored
            }
            DomEventKind::Scroll { dy, .. } => {
                let max = Self::max_scroll(root, layout);
                self.apply_scroll(self.scroll_y + *dy, max)
            }
            _ => WidgetOutcome::Ignored,
        }
    }

    fn on_keyboard(
        &mut self,
        root: NodeId,
        key: KeyInput,
        layout: &LayoutQuery,
    ) -> WidgetOutcome {
        if self.disabled {
            return WidgetOutcome::Ignored;
        }
        let page = {
            // A "page" is a viewport-height worth of rows; derive the row count
            // from the laid-out viewport vs a single row box (layout, not a const).
            let rows = match (
                layout.box_of_part(root, "viewport"),
                self.caret_row_box(root, layout),
            ) {
                (Some(vp), Some(row)) if row.height > 0.0 => {
                    (vp.height / row.height).floor().max(1.0) as isize
                }
                _ => 1,
            };
            rows.max(1)
        };

        let outcome = match key.key {
            keys::ENTER => self.newline(),
            keys::BACKSPACE => self.backspace(),
            keys::DELETE => self.delete_forward(),
            keys::ARROW_LEFT => self.move_left(),
            keys::ARROW_RIGHT => self.move_right(),
            keys::ARROW_UP => self.move_up(),
            keys::ARROW_DOWN => self.move_down(),
            keys::HOME => self.home(),
            keys::END => self.end(),
            keys::PAGE_UP => self.move_page(-page),
            keys::PAGE_DOWN => self.move_page(page),
            other => {
                if key.modifiers & (keys::modifiers::CTRL | keys::modifiers::ALT | keys::modifiers::SUPER) != 0 {
                    return WidgetOutcome::Ignored;
                }
                match keys::printable_char(other) {
                    Some(c) => self.insert_char(c),
                    None => WidgetOutcome::Ignored,
                }
            }
        };
        // After any caret-affecting change, keep the caret line in view.
        if outcome.needs_render() {
            self.ensure_caret_visible(root, layout);
        }
        outcome
    }

    fn focusable(&self) -> bool {
        !self.disabled
    }

    fn render(&self) -> TemplateNode {
        let empty = self.is_empty();

        // ── line rows (the in-flow content; layout positions each row's y) ──
        let mut rows: Vec<TemplateNode> = Vec::with_capacity(self.lines.len());
        for (i, line) in self.lines.iter().enumerate() {
            let is_caret_line = i == self.caret_line && self.focused;
            let mut row = TemplateNode::el("lq-textarea-row")
                .attr("data-part", &format!("row-{i}"))
                .class_if("caret-row", is_caret_line);
            // Mark the caret row with the stable "caret-row" part for scroll math.
            if is_caret_line {
                row = row.attr("data-caret-row", "true");
            }

            if is_caret_line {
                // Split the line at the caret so the caret element sits BETWEEN the
                // before/after spans in flow — layout positions the caret box.
                let col = Self::clamp_col(line, self.caret_col);
                let (before, after) = line.split_at(col);
                row = row
                    .child(
                        TemplateNode::el("lq-textarea-text")
                            .attr("data-part", &format!("text-{i}"))
                            .attr("data-sub", "before")
                            .child(TemplateNode::text(before)),
                    )
                    .child(
                        TemplateNode::el("lq-caret")
                            .attr("data-part", "caret")
                            .pseudo_if(PseudoStateFlags::FOCUS, self.focused),
                    )
                    .child(
                        TemplateNode::el("lq-textarea-text")
                            .attr("data-sub", "after")
                            .child(TemplateNode::text(after)),
                    );
            } else {
                // A placeholder on the (only) empty line when the whole buffer is
                // empty and unfocused.
                let show_placeholder = empty && i == 0 && !self.placeholder.is_empty();
                row = row.child(
                    TemplateNode::el("lq-textarea-text")
                        .attr("data-part", &format!("text-{i}"))
                        .class_if("placeholder", show_placeholder)
                        .child(TemplateNode::text(if show_placeholder {
                            &self.placeholder
                        } else {
                            line
                        })),
                );
            }
            rows.push(row);
        }

        // The scrolled content block: translated UP by the scroll offset via a
        // negative top-margin (the in-lock scroll; the viewport clips).
        let content = TemplateNode::el("lq-textarea-content")
            .attr("data-part", "content")
            .style("margin-top", &format!("{}px", -self.scroll_y))
            .children(rows);

        let viewport = TemplateNode::el("lq-textarea-viewport")
            .attr("data-part", "viewport")
            .child(content);

        // ── optional gutter (line numbers, one row per line, same flow) ──
        let body: TemplateNode = if self.show_gutter {
            let mut gutter_rows: Vec<TemplateNode> = Vec::with_capacity(self.lines.len());
            for i in 0..self.lines.len() {
                gutter_rows.push(
                    TemplateNode::el("lq-textarea-lineno")
                        .attr("data-part", &format!("lineno-{i}"))
                        .class_if("current", i == self.caret_line && self.focused)
                        .child(TemplateNode::text(&format!("{}", i + 1))),
                );
            }
            // The gutter scrolls in lockstep with the content (same translate).
            let gutter_inner = TemplateNode::el("lq-textarea-gutter-inner")
                .attr("data-part", "gutter-inner")
                .style("margin-top", &format!("{}px", -self.scroll_y))
                .children(gutter_rows);
            let gutter = TemplateNode::el("lq-textarea-gutter")
                .attr("data-part", "gutter")
                .child(gutter_inner);
            TemplateNode::el("lq-textarea-body")
                .attr("data-part", "body")
                .child(gutter)
                .child(viewport)
        } else {
            TemplateNode::el("lq-textarea-body")
                .attr("data-part", "body")
                .child(viewport)
        };

        let mut node = TemplateNode::el("lq-textarea")
            .attr(FOCUSABLE_ATTR, if self.disabled { "false" } else { "true" })
            .attr("data-scroll-y", &format!("{}", self.scroll_y))
            .class_if("with-gutter", self.show_gutter)
            .pseudo_if(PseudoStateFlags::FOCUS, self.focused && !self.disabled)
            .pseudo_if(PseudoStateFlags::DISABLED, self.disabled)
            .child(body);
        if self.disabled {
            node = node.attr("disabled", "true");
        }
        node
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}
