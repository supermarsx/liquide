//! Single-line text input widget.
//!
//! Supports text editing, cursor positioning, selection (basic),
//! placeholder text, and read-only mode. Inspired by Qt's QLineEdit
//! and GTK's GtkEntry.

use liquide_ui_core::{
    Constraints, Event, EventResponse, Key, LayoutResult, MouseButton, Painter, UiTheme, WidgetId,
    widget::{Widget, WidgetState},
};
use unicode_segmentation::UnicodeSegmentation;

/// A single-line text input field.
pub struct TextInput {
    state: WidgetState,
    text: String,
    placeholder: String,
    cursor_pos: usize,
    /// Anchor for the current selection. `Some(a)` with `a != cursor_pos`
    /// means the range `[min(a, cursor_pos), max(a, cursor_pos))` is
    /// selected. `None` means no selection.
    selection_start: Option<usize>,
    read_only: bool,
    max_length: Option<usize>,
    on_change: Option<Box<dyn FnMut(&str) + Send>>,
    on_submit: Option<Box<dyn FnMut(&str) + Send>>,
    /// Whether the mouse is currently drag-selecting.
    mouse_selecting: bool,
    // Layout
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    scroll_offset: f32,
}

impl TextInput {
    pub fn new() -> Self {
        Self {
            state: WidgetState::new(WidgetId::new()),
            text: String::new(),
            placeholder: String::new(),
            cursor_pos: 0,
            selection_start: None,
            read_only: false,
            max_length: None,
            on_change: None,
            on_submit: None,
            mouse_selecting: false,
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
            scroll_offset: 0.0,
        }
    }

    pub fn with_placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }

    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self.cursor_pos = self.text.len();
        self
    }

    pub fn with_tooltip(mut self, text: impl Into<String>) -> Self {
        self.state.tooltip = Some(text.into());
        self
    }

    pub fn with_max_length(mut self, len: usize) -> Self {
        self.max_length = Some(len);
        self
    }

    pub fn read_only(mut self, ro: bool) -> Self {
        self.read_only = ro;
        self
    }

    pub fn on_change(mut self, f: impl FnMut(&str) + Send + 'static) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }

    pub fn on_submit(mut self, f: impl FnMut(&str) + Send + 'static) -> Self {
        self.on_submit = Some(Box::new(f));
        self
    }

    /// Current text content.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Set the text programmatically.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.cursor_pos = clamp_to_grapheme_boundary(&self.text, self.cursor_pos.min(self.text.len()));
    }

    fn insert_char(&mut self, c: char) {
        if self.read_only {
            return;
        }
        // If there's an active selection, typing replaces it.
        if self.has_selection() {
            self.delete_selection();
        }
        if let Some(max) = self.max_length {
            if self.text.len() >= max {
                return;
            }
        }
        self.text.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
        self.selection_start = None;
        if let Some(cb) = &mut self.on_change {
            let text = self.text.clone();
            cb(&text);
        }
    }

    fn delete_backward(&mut self) {
        if self.read_only {
            return;
        }
        if self.has_selection() {
            self.delete_selection();
            return;
        }
        if self.cursor_pos == 0 {
            return;
        }
        // Step back one grapheme cluster so combining marks / flag emoji
        // are removed as a single user-visible character.
        let prev = prev_grapheme_boundary(&self.text, self.cursor_pos);
        self.text.drain(prev..self.cursor_pos);
        self.cursor_pos = prev;
        if let Some(cb) = &mut self.on_change {
            let text = self.text.clone();
            cb(&text);
        }
    }

    fn delete_forward(&mut self) {
        if self.read_only {
            return;
        }
        if self.has_selection() {
            self.delete_selection();
            return;
        }
        if self.cursor_pos >= self.text.len() {
            return;
        }
        let next = next_grapheme_boundary(&self.text, self.cursor_pos);
        self.text.drain(self.cursor_pos..next);
        if let Some(cb) = &mut self.on_change {
            let text = self.text.clone();
            cb(&text);
        }
    }

    fn has_selection(&self) -> bool {
        matches!(self.selection_start, Some(a) if a != self.cursor_pos)
    }

    fn selection_range(&self) -> Option<(usize, usize)> {
        match self.selection_start {
            Some(a) if a != self.cursor_pos => {
                Some((a.min(self.cursor_pos), a.max(self.cursor_pos)))
            }
            _ => None,
        }
    }

    fn delete_selection(&mut self) {
        if let Some((lo, hi)) = self.selection_range() {
            self.text.drain(lo..hi);
            self.cursor_pos = lo;
            self.selection_start = None;
            if let Some(cb) = &mut self.on_change {
                let text = self.text.clone();
                cb(&text);
            }
        }
    }

    /// Map widget-local x-coordinate to a byte cursor position in `text`.
    fn cursor_from_x(&self, local_x: f32, font_size: f32) -> usize {
        let padding = 8.0;
        let char_w = font_size * 0.55;
        let rel = (local_x - padding + self.scroll_offset).max(0.0);
        let want_graphemes = (rel / char_w).round() as usize;
        boundary_for_grapheme_index(&self.text, want_graphemes)
    }

    fn move_cursor(&mut self, new_pos: usize, extend_selection: bool) {
        if extend_selection {
            if self.selection_start.is_none() {
                self.selection_start = Some(self.cursor_pos);
            }
        } else {
            self.selection_start = None;
        }
        self.cursor_pos = clamp_to_grapheme_boundary(&self.text, new_pos.min(self.text.len()));
    }

    fn select_all(&mut self) {
        self.selection_start = Some(0);
        self.cursor_pos = self.text.len();
    }
}

fn prev_grapheme_boundary(s: &str, cursor: usize) -> usize {
    let mut last = 0;
    for (idx, _) in s.grapheme_indices(true) {
        if idx >= cursor {
            break;
        }
        last = idx;
    }
    last
}

fn next_grapheme_boundary(s: &str, cursor: usize) -> usize {
    for (idx, _) in s.grapheme_indices(true) {
        if idx > cursor {
            return idx;
        }
    }
    s.len()
}

fn boundary_for_grapheme_index(s: &str, index: usize) -> usize {
    s.grapheme_indices(true)
        .nth(index)
        .map(|(idx, _)| idx)
        .unwrap_or(s.len())
}

fn grapheme_count_before(s: &str, cursor: usize) -> usize {
    let boundary = clamp_to_grapheme_boundary(s, cursor.min(s.len()));
    UnicodeSegmentation::graphemes(&s[..boundary], true).count()
}

fn clamp_to_grapheme_boundary(s: &str, cursor: usize) -> usize {
    if cursor >= s.len() {
        return s.len();
    }
    if s.grapheme_indices(true).any(|(idx, _)| idx == cursor) {
        return cursor;
    }
    prev_grapheme_boundary(s, cursor)
}

impl Default for TextInput {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for TextInput {
    fn id(&self) -> WidgetId {
        self.state.id
    }
    fn visible(&self) -> bool {
        self.state.visible
    }
    fn set_visible(&mut self, v: bool) {
        self.state.visible = v;
    }
    fn enabled(&self) -> bool {
        self.state.enabled
    }
    fn set_enabled(&mut self, e: bool) {
        self.state.enabled = e;
    }
    fn focusable(&self) -> bool {
        true
    }
    fn tooltip(&self) -> Option<&str> {
        self.state.tooltip.as_deref()
    }

    fn measure(&self, constraints: &Constraints, theme: &UiTheme) -> LayoutResult {
        let h = theme.font_size + 16.0; // padding
        let w = 200.0; // default preferred width
        let (w, h) = constraints.clamp(w, h);
        LayoutResult::new(w, h)
    }

    fn layout(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.x = x;
        self.y = y;
        self.width = w;
        self.height = h;
    }

    fn paint(&self, painter: &mut Painter, theme: &UiTheme) {
        let colors = &theme.colors;
        let radius = theme.radius_md;

        // Background
        let bg = if self.state.focused {
            colors.surface_hover
        } else {
            colors.surface
        };
        painter.fill_rounded_rect(self.x, self.y, self.width, self.height, radius, bg);

        // Border
        let border_color = if self.state.focused {
            colors.accent
        } else {
            colors.border
        };
        painter.stroke_rounded_rect(
            self.x,
            self.y,
            self.width,
            self.height,
            radius,
            border_color,
            1.0,
        );

        // Text or placeholder
        let padding = 8.0;
        let text_y = self.y + (self.height - theme.font_size) / 2.0;
        let char_w = theme.font_size * 0.55;
        if self.text.is_empty() && !self.state.focused {
            painter.draw_text(
                &self.placeholder,
                self.x + padding,
                text_y,
                theme.font_size,
                colors.text_disabled,
                &theme.font_family,
                false,
            );
        } else {
            // Selection highlight
            if let Some((lo, hi)) = self.selection_range() {
                let chars_before = grapheme_count_before(&self.text, lo) as f32;
                let chars_in = UnicodeSegmentation::graphemes(&self.text[lo..hi], true).count() as f32;
                let sel_x = self.x + padding + chars_before * char_w - self.scroll_offset;
                let sel_w = chars_in * char_w;
                let sel_h = theme.font_size + 4.0;
                let sel_y = self.y + (self.height - sel_h) / 2.0;
                painter.fill_rounded_rect(
                    sel_x,
                    sel_y,
                    sel_w,
                    sel_h,
                    2.0,
                    colors.accent.with_alpha(80),
                );
            }
            painter.draw_text(
                &self.text,
                self.x + padding - self.scroll_offset,
                text_y,
                theme.font_size,
                colors.text_primary,
                &theme.font_family,
                false,
            );
        }

        // Cursor — position by char count up to cursor_pos so non-ASCII
        // doesn't visually drift from the glyph grid.
        if self.state.focused {
            let chars_before = grapheme_count_before(&self.text, self.cursor_pos) as f32;
            let cursor_x = self.x + padding + chars_before * char_w - self.scroll_offset;
            painter.draw_line(
                cursor_x,
                self.y + 4.0,
                cursor_x,
                self.y + self.height - 4.0,
                colors.accent,
                1.5,
            );
        }
    }

    fn handle_event(&mut self, event: &Event) -> EventResponse {
        match event {
            Event::MouseEnter => {
                self.state.hovered = true;
                EventResponse::Consumed
            }
            Event::MouseLeave => {
                self.state.hovered = false;
                EventResponse::Consumed
            }
            Event::MouseDown {
                x,
                button: MouseButton::Left,
                ..
            } => {
                self.state.pressed = true;
                self.mouse_selecting = true;
                let pos = self.cursor_from_x(*x - self.x, 14.0);
                self.cursor_pos = pos;
                self.selection_start = Some(pos);
                EventResponse::RequestFocus
            }
            Event::MouseMove { x, .. } if self.mouse_selecting => {
                let pos = self.cursor_from_x(*x - self.x, 14.0);
                self.cursor_pos = pos;
                EventResponse::Consumed
            }
            Event::MouseUp { .. } => {
                self.state.pressed = false;
                self.mouse_selecting = false;
                // If the drag didn't move, clear the anchor so a simple
                // click places the cursor without leaving a stray selection.
                if !self.has_selection() {
                    self.selection_start = None;
                }
                EventResponse::Consumed
            }
            Event::FocusIn => {
                self.state.focused = true;
                EventResponse::Consumed
            }
            Event::FocusOut => {
                self.state.focused = false;
                EventResponse::Consumed
            }
            Event::TextInput { text } if self.state.focused => {
                for c in text.chars() {
                    self.insert_char(c);
                }
                EventResponse::Consumed
            }
            Event::KeyDown { key, modifiers } if self.state.focused => {
                // Ctrl shortcuts
                if modifiers.ctrl {
                    match key {
                        Key::Char('a') | Key::Char('A') => {
                            self.select_all();
                            return EventResponse::Consumed;
                        }
                        Key::Char('c') | Key::Char('C') => {
                            return EventResponse::Ignored;
                        }
                        Key::Char('x') | Key::Char('X') if !self.read_only => {
                            return EventResponse::Ignored;
                        }
                        Key::Char('v') | Key::Char('V') if !self.read_only => {
                            return EventResponse::Ignored;
                        }
                        _ => {}
                    }
                }

                let extend = modifiers.shift;
                match key {
                    Key::Backspace => {
                        self.delete_backward();
                        EventResponse::Consumed
                    }
                    Key::Delete => {
                        self.delete_forward();
                        EventResponse::Consumed
                    }
                    Key::ArrowLeft => {
                        let new_pos = if self.cursor_pos == 0 {
                            0
                        } else {
                            prev_grapheme_boundary(&self.text, self.cursor_pos)
                        };
                        self.move_cursor(new_pos, extend);
                        EventResponse::Consumed
                    }
                    Key::ArrowRight => {
                        let new_pos = next_grapheme_boundary(&self.text, self.cursor_pos);
                        self.move_cursor(new_pos, extend);
                        EventResponse::Consumed
                    }
                    Key::Home => {
                        self.move_cursor(0, extend);
                        EventResponse::Consumed
                    }
                    Key::End => {
                        let len = self.text.len();
                        self.move_cursor(len, extend);
                        EventResponse::Consumed
                    }
                    Key::Enter => {
                        if let Some(cb) = &mut self.on_submit {
                            let text = self.text.clone();
                            cb(&text);
                        }
                        EventResponse::Consumed
                    }
                    _ => EventResponse::Ignored,
                }
            }
            _ => EventResponse::Ignored,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_ui_core::{Event, Key, Modifiers};

    fn focused(text: &str) -> TextInput {
        let mut t = TextInput::new().with_text(text);
        t.state.focused = true;
        t
    }

    #[test]
    fn shift_arrow_extends_selection() {
        let mut t = focused("hello");
        t.cursor_pos = 0;
        let m = Modifiers {
            shift: true,
            ..Modifiers::NONE
        };
        let _ = t.handle_event(&Event::KeyDown {
            key: Key::ArrowRight,
            modifiers: m,
        });
        let _ = t.handle_event(&Event::KeyDown {
            key: Key::ArrowRight,
            modifiers: m,
        });
        assert_eq!(t.selection_range(), Some((0, 2)));
    }

    #[test]
    fn ctrl_a_selects_all() {
        let mut t = focused("hello");
        let m = Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        };
        let _ = t.handle_event(&Event::KeyDown {
            key: Key::Char('a'),
            modifiers: m,
        });
        assert_eq!(t.selection_range(), Some((0, 5)));
    }

    #[test]
    fn typing_replaces_selection() {
        let mut t = focused("abc");
        t.select_all();
        let _ = t.handle_event(&Event::TextInput { text: "Z".into() });
        assert_eq!(t.text(), "Z");
        assert!(t.selection_range().is_none());
    }

    #[test]
    fn backspace_deletes_grapheme_cluster() {
        // "e" + combining acute should be removed as one user-visible char.
        let mut t = focused("e\u{0301}");
        t.cursor_pos = t.text().len();
        let _ = t.handle_event(&Event::KeyDown {
            key: Key::Backspace,
            modifiers: Modifiers::NONE,
        });
        assert_eq!(t.text(), "");
    }

    #[test]
    fn mouse_cursor_lands_on_grapheme_boundaries() {
        let t = focused("e\u{0301}b");
        let boundary = t.cursor_from_x(8.0 + 14.0 * 0.55, 14.0);
        assert_eq!(boundary, "e\u{0301}".len());
    }

    #[test]
    fn unimplemented_clipboard_shortcuts_are_not_consumed() {
        let mut t = focused("hello");
        t.select_all();
        let modifiers = Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        };

        let copy = t.handle_event(&Event::KeyDown {
            key: Key::Char('c'),
            modifiers,
        });
        let cut = t.handle_event(&Event::KeyDown {
            key: Key::Char('x'),
            modifiers,
        });
        let paste = t.handle_event(&Event::KeyDown {
            key: Key::Char('v'),
            modifiers,
        });

        assert_eq!(copy, EventResponse::Ignored);
        assert_eq!(cut, EventResponse::Ignored);
        assert_eq!(paste, EventResponse::Ignored);
        assert_eq!(t.text(), "hello");
    }
}
