//! Multi-line text area widget.
//!
//! A text editor supporting multiple lines, scrolling, cursor movement,
//! selection (basic), and read-only mode. Inspired by Qt's QPlainTextEdit
//! and GTK's GtkTextView.

use liquide_ui_core::{
    Constraints, Event, EventResponse, Key, LayoutResult, Painter, UiTheme, WidgetId,
    widget::{Widget, WidgetState},
};

/// A multi-line text editor.
pub struct TextArea {
    state: WidgetState,
    lines: Vec<String>,
    cursor_line: usize,
    cursor_col: usize,
    scroll_offset_y: f32,
    read_only: bool,
    placeholder: String,
    on_change: Option<Box<dyn FnMut(&str) + Send>>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl TextArea {
    pub fn new() -> Self {
        Self {
            state: WidgetState::new(WidgetId::new()),
            lines: vec![String::new()],
            cursor_line: 0,
            cursor_col: 0,
            scroll_offset_y: 0.0,
            read_only: false,
            placeholder: String::new(),
            on_change: None,
            x: 0.0, y: 0.0, width: 0.0, height: 0.0,
        }
    }

    pub fn with_text(mut self, text: &str) -> Self {
        self.lines = text.lines().map(String::from).collect();
        if self.lines.is_empty() { self.lines.push(String::new()); }
        self.cursor_line = 0;
        self.cursor_col = 0;
        self
    }

    pub fn with_placeholder(mut self, p: impl Into<String>) -> Self {
        self.placeholder = p.into();
        self
    }

    pub fn with_tooltip(mut self, text: impl Into<String>) -> Self {
        self.state.tooltip = Some(text.into());
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

    /// Get the full text content.
    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    /// Set text content programmatically.
    pub fn set_text(&mut self, text: &str) {
        self.lines = text.lines().map(String::from).collect();
        if self.lines.is_empty() { self.lines.push(String::new()); }
        self.cursor_line = self.cursor_line.min(self.lines.len() - 1);
        self.cursor_col = self.cursor_col.min(self.lines[self.cursor_line].len());
    }

    fn line_height(theme: &UiTheme) -> f32 {
        theme.font_size * 1.5
    }

    fn content_height(&self, theme: &UiTheme) -> f32 {
        self.lines.len() as f32 * Self::line_height(theme)
    }

    fn notify_change(&mut self) {
        if let Some(cb) = &mut self.on_change {
            let text = self.lines.join("\n");
            cb(&text);
        }
    }

    fn insert_char(&mut self, c: char) {
        if self.read_only { return; }
        self.lines[self.cursor_line].insert(self.cursor_col, c);
        self.cursor_col += c.len_utf8();
        self.notify_change();
    }

    fn insert_newline(&mut self) {
        if self.read_only { return; }
        let rest = self.lines[self.cursor_line][self.cursor_col..].to_string();
        self.lines[self.cursor_line].truncate(self.cursor_col);
        self.cursor_line += 1;
        self.lines.insert(self.cursor_line, rest);
        self.cursor_col = 0;
        self.notify_change();
    }

    fn delete_backward(&mut self) {
        if self.read_only { return; }
        if self.cursor_col > 0 {
            let prev = self.lines[self.cursor_line][..self.cursor_col]
                .char_indices().last().map(|(i, _)| i).unwrap_or(0);
            self.lines[self.cursor_line].drain(prev..self.cursor_col);
            self.cursor_col = prev;
        } else if self.cursor_line > 0 {
            let line = self.lines.remove(self.cursor_line);
            self.cursor_line -= 1;
            self.cursor_col = self.lines[self.cursor_line].len();
            self.lines[self.cursor_line].push_str(&line);
        }
        self.notify_change();
    }

    fn delete_forward(&mut self) {
        if self.read_only { return; }
        let line_len = self.lines[self.cursor_line].len();
        if self.cursor_col < line_len {
            let next = self.lines[self.cursor_line][self.cursor_col..]
                .char_indices().nth(1).map(|(i, _)| self.cursor_col + i)
                .unwrap_or(line_len);
            self.lines[self.cursor_line].drain(self.cursor_col..next);
        } else if self.cursor_line + 1 < self.lines.len() {
            let next_line = self.lines.remove(self.cursor_line + 1);
            self.lines[self.cursor_line].push_str(&next_line);
        }
        self.notify_change();
    }

    #[allow(dead_code)]
    fn ensure_cursor_visible(&mut self, theme: &UiTheme) {
        let lh = Self::line_height(theme);
        let cursor_y = self.cursor_line as f32 * lh;
        let visible_h = self.height - 8.0; // padding
        if cursor_y < self.scroll_offset_y {
            self.scroll_offset_y = cursor_y;
        } else if cursor_y + lh > self.scroll_offset_y + visible_h {
            self.scroll_offset_y = cursor_y + lh - visible_h;
        }
    }
}

impl Default for TextArea {
    fn default() -> Self { Self::new() }
}

impl Widget for TextArea {
    fn id(&self) -> WidgetId { self.state.id }
    fn visible(&self) -> bool { self.state.visible }
    fn set_visible(&mut self, v: bool) { self.state.visible = v; }
    fn enabled(&self) -> bool { self.state.enabled }
    fn set_enabled(&mut self, e: bool) { self.state.enabled = e; }
    fn focusable(&self) -> bool { true }
    fn tooltip(&self) -> Option<&str> { self.state.tooltip.as_deref() }

    fn measure(&self, constraints: &Constraints, theme: &UiTheme) -> LayoutResult {
        let lh = Self::line_height(theme);
        let preferred_h = (self.lines.len().min(10) as f32 * lh + 8.0).max(3.0 * lh);
        let (w, h) = constraints.clamp(300.0, preferred_h);
        LayoutResult::new(w, h)
    }

    fn layout(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.x = x; self.y = y; self.width = w; self.height = h;
    }

    fn paint(&self, painter: &mut Painter, theme: &UiTheme) {
        let colors = &theme.colors;
        let radius = theme.radius_md;
        let lh = Self::line_height(theme);
        let padding = 4.0;

        // Background
        let bg = if self.state.focused { colors.surface_hover } else { colors.surface };
        painter.fill_rounded_rect(self.x, self.y, self.width, self.height, radius, bg);
        let border = if self.state.focused { colors.accent } else { colors.border };
        painter.stroke_rounded_rect(self.x, self.y, self.width, self.height, radius, border, 1.0);

        // Clip
        painter.push_clip(self.x + 1.0, self.y + 1.0, self.width - 2.0, self.height - 2.0);

        // Placeholder
        let is_empty = self.lines.len() == 1 && self.lines[0].is_empty();
        if is_empty && !self.state.focused {
            painter.draw_text(
                &self.placeholder, self.x + padding, self.y + padding,
                theme.font_size, colors.text_disabled, &theme.font_family, false,
            );
        } else {
            // Lines
            let first_visible = (self.scroll_offset_y / lh) as usize;
            let visible_count = (self.height / lh) as usize + 2;
            for i in first_visible..(first_visible + visible_count).min(self.lines.len()) {
                let ly = self.y + padding + i as f32 * lh - self.scroll_offset_y;
                painter.draw_text(
                    &self.lines[i], self.x + padding, ly,
                    theme.font_size, colors.text_primary, &theme.font_family, false,
                );
            }

            // Cursor
            if self.state.focused {
                let char_w = theme.font_size * 0.55;
                let cx = self.x + padding + self.cursor_col as f32 * char_w;
                let cy = self.y + padding + self.cursor_line as f32 * lh - self.scroll_offset_y;
                painter.draw_line(cx, cy, cx, cy + lh - 2.0, colors.accent, 1.5);
            }
        }

        painter.pop_clip();
    }

    fn handle_event(&mut self, event: &Event) -> EventResponse {
        match event {
            Event::MouseEnter => { self.state.hovered = true; EventResponse::Consumed }
            Event::MouseLeave => { self.state.hovered = false; EventResponse::Consumed }
            Event::MouseDown { .. } => {
                self.state.pressed = true;
                EventResponse::RequestFocus
            }
            Event::MouseUp { .. } => { self.state.pressed = false; EventResponse::Consumed }
            Event::FocusIn => { self.state.focused = true; EventResponse::Consumed }
            Event::FocusOut => { self.state.focused = false; EventResponse::Consumed }
            Event::Scroll { dy, .. } => {
                self.scroll_offset_y = (self.scroll_offset_y + dy * 30.0)
                    .clamp(0.0, (self.content_height(&UiTheme::default()) - self.height).max(0.0));
                EventResponse::Consumed
            }
            Event::TextInput { text } if self.state.focused => {
                for c in text.chars() {
                    if c == '\n' || c == '\r' {
                        self.insert_newline();
                    } else {
                        self.insert_char(c);
                    }
                }
                EventResponse::Consumed
            }
            Event::KeyDown { key, .. } if self.state.focused => {
                match key {
                    Key::Backspace => { self.delete_backward(); EventResponse::Consumed }
                    Key::Delete => { self.delete_forward(); EventResponse::Consumed }
                    Key::Enter => { self.insert_newline(); EventResponse::Consumed }
                    Key::ArrowLeft => {
                        if self.cursor_col > 0 {
                            self.cursor_col -= 1;
                        } else if self.cursor_line > 0 {
                            self.cursor_line -= 1;
                            self.cursor_col = self.lines[self.cursor_line].len();
                        }
                        EventResponse::Consumed
                    }
                    Key::ArrowRight => {
                        let len = self.lines[self.cursor_line].len();
                        if self.cursor_col < len {
                            self.cursor_col += 1;
                        } else if self.cursor_line + 1 < self.lines.len() {
                            self.cursor_line += 1;
                            self.cursor_col = 0;
                        }
                        EventResponse::Consumed
                    }
                    Key::ArrowUp if self.cursor_line > 0 => {
                        self.cursor_line -= 1;
                        self.cursor_col = self.cursor_col.min(self.lines[self.cursor_line].len());
                        EventResponse::Consumed
                    }
                    Key::ArrowDown if self.cursor_line + 1 < self.lines.len() => {
                        self.cursor_line += 1;
                        self.cursor_col = self.cursor_col.min(self.lines[self.cursor_line].len());
                        EventResponse::Consumed
                    }
                    Key::Home => { self.cursor_col = 0; EventResponse::Consumed }
                    Key::End => { self.cursor_col = self.lines[self.cursor_line].len(); EventResponse::Consumed }
                    _ => EventResponse::Ignored,
                }
            }
            _ => EventResponse::Ignored,
        }
    }
}
