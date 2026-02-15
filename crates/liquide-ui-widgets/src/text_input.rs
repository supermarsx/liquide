//! Single-line text input widget.
//!
//! Supports text editing, cursor positioning, selection (basic),
//! placeholder text, and read-only mode. Inspired by Qt's QLineEdit
//! and GTK's GtkEntry.

use liquide_ui_core::{
    Constraints, Event, EventResponse, Key, LayoutResult, Painter, UiTheme, WidgetId,
    widget::{Widget, WidgetState},
};

/// A single-line text input field.
pub struct TextInput {
    state: WidgetState,
    text: String,
    placeholder: String,
    cursor_pos: usize,
    #[allow(dead_code)]
    selection_start: Option<usize>,
    read_only: bool,
    max_length: Option<usize>,
    on_change: Option<Box<dyn FnMut(&str) + Send>>,
    on_submit: Option<Box<dyn FnMut(&str) + Send>>,
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
        self.cursor_pos = self.cursor_pos.min(self.text.len());
    }

    fn insert_char(&mut self, c: char) {
        if self.read_only {
            return;
        }
        if let Some(max) = self.max_length {
            if self.text.len() >= max {
                return;
            }
        }
        self.text.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
        if let Some(cb) = &mut self.on_change {
            let text = self.text.clone();
            cb(&text);
        }
    }

    fn delete_backward(&mut self) {
        if self.read_only || self.cursor_pos == 0 {
            return;
        }
        // Find the previous character boundary
        let prev = self.text[..self.cursor_pos]
            .char_indices()
            .last()
            .map(|(i, _)| i)
            .unwrap_or(0);
        self.text.drain(prev..self.cursor_pos);
        self.cursor_pos = prev;
        if let Some(cb) = &mut self.on_change {
            let text = self.text.clone();
            cb(&text);
        }
    }

    fn delete_forward(&mut self) {
        if self.read_only || self.cursor_pos >= self.text.len() {
            return;
        }
        let next = self.text[self.cursor_pos..]
            .char_indices()
            .nth(1)
            .map(|(i, _)| self.cursor_pos + i)
            .unwrap_or(self.text.len());
        self.text.drain(self.cursor_pos..next);
        if let Some(cb) = &mut self.on_change {
            let text = self.text.clone();
            cb(&text);
        }
    }
}

impl Default for TextInput {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for TextInput {
    fn id(&self) -> WidgetId { self.state.id }
    fn visible(&self) -> bool { self.state.visible }
    fn set_visible(&mut self, v: bool) { self.state.visible = v; }
    fn enabled(&self) -> bool { self.state.enabled }
    fn set_enabled(&mut self, e: bool) { self.state.enabled = e; }
    fn focusable(&self) -> bool { true }
    fn tooltip(&self) -> Option<&str> { self.state.tooltip.as_deref() }

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
        painter.stroke_rounded_rect(self.x, self.y, self.width, self.height, radius, border_color, 1.0);

        // Text or placeholder
        let padding = 8.0;
        let text_y = self.y + (self.height - theme.font_size) / 2.0;
        if self.text.is_empty() && !self.state.focused {
            painter.draw_text(
                &self.placeholder, self.x + padding, text_y,
                theme.font_size, colors.text_disabled, &theme.font_family, false,
            );
        } else {
            painter.draw_text(
                &self.text, self.x + padding - self.scroll_offset, text_y,
                theme.font_size, colors.text_primary, &theme.font_family, false,
            );
        }

        // Cursor
        if self.state.focused {
            let char_w = theme.font_size * 0.55;
            let cursor_x = self.x + padding + self.cursor_pos as f32 * char_w - self.scroll_offset;
            painter.draw_line(cursor_x, self.y + 4.0, cursor_x, self.y + self.height - 4.0, colors.accent, 1.5);
        }
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
            Event::TextInput { text } if self.state.focused => {
                for c in text.chars() {
                    self.insert_char(c);
                }
                EventResponse::Consumed
            }
            Event::KeyDown { key, .. } if self.state.focused => {
                match key {
                    Key::Backspace => { self.delete_backward(); EventResponse::Consumed }
                    Key::Delete => { self.delete_forward(); EventResponse::Consumed }
                    Key::ArrowLeft if self.cursor_pos > 0 => {
                        self.cursor_pos -= 1;
                        EventResponse::Consumed
                    }
                    Key::ArrowRight if self.cursor_pos < self.text.len() => {
                        self.cursor_pos += 1;
                        EventResponse::Consumed
                    }
                    Key::Home => { self.cursor_pos = 0; EventResponse::Consumed }
                    Key::End => { self.cursor_pos = self.text.len(); EventResponse::Consumed }
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
