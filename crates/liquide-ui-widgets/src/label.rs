//! Label widget.
//!
//! Static text display with alignment, wrapping, and styling options.
//! Inspired by Qt's QLabel and GTK's GtkLabel.

use liquide_ui_core::{
    Constraints, Event, EventResponse, LayoutResult, Painter, UiColor, UiTheme, WidgetId,
    text::{SimpleTextMeasure, TextMeasure},
    widget::{Widget, WidgetState},
};

/// Text alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

/// Label style variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelStyle {
    /// Normal body text.
    Body,
    /// Section heading — larger, bold.
    Heading,
    /// Small caption text.
    Caption,
    /// Monospaced / code text.
    Code,
}

/// A static text label.
pub struct Label {
    state: WidgetState,
    text: String,
    align: TextAlign,
    label_style: LabelStyle,
    bold: bool,
    color_override: Option<UiColor>,
    selectable: bool,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl Label {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            state: WidgetState::new(WidgetId::new()),
            text: text.into(),
            align: TextAlign::Left,
            label_style: LabelStyle::Body,
            bold: false,
            color_override: None,
            selectable: false,
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        }
    }

    pub fn heading(text: impl Into<String>) -> Self {
        Self::new(text)
            .with_style(LabelStyle::Heading)
            .with_bold(true)
    }

    pub fn caption(text: impl Into<String>) -> Self {
        Self::new(text).with_style(LabelStyle::Caption)
    }

    pub fn code(text: impl Into<String>) -> Self {
        Self::new(text).with_style(LabelStyle::Code)
    }

    pub fn with_style(mut self, style: LabelStyle) -> Self {
        self.label_style = style;
        self
    }

    pub fn with_align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }

    pub fn with_bold(mut self, bold: bool) -> Self {
        self.bold = bold;
        self
    }

    pub fn with_color(mut self, color: UiColor) -> Self {
        self.color_override = Some(color);
        self
    }

    pub fn with_tooltip(mut self, text: impl Into<String>) -> Self {
        self.state.tooltip = Some(text.into());
        self
    }

    pub fn selectable(mut self, sel: bool) -> Self {
        self.selectable = sel;
        self
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    fn font_size(&self, theme: &UiTheme) -> f32 {
        match self.label_style {
            LabelStyle::Body => theme.font_size,
            LabelStyle::Heading => theme.font_size * 1.4,
            LabelStyle::Caption => theme.font_size * 0.85,
            LabelStyle::Code => theme.font_size * 0.95,
        }
    }

    fn font_family<'a>(&self, theme: &'a UiTheme) -> &'a str {
        match self.label_style {
            LabelStyle::Code => "JetBrains Mono",
            _ => &theme.font_family,
        }
    }
}

impl Widget for Label {
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
        self.selectable
    }
    fn tooltip(&self) -> Option<&str> {
        self.state.tooltip.as_deref()
    }

    fn measure(&self, constraints: &Constraints, theme: &UiTheme) -> LayoutResult {
        let fs = self.font_size(theme);
        let measurer = SimpleTextMeasure;
        let w = measurer.measure_text(&self.text, fs, self.bold).0;
        let h = fs + 4.0;
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
        let fs = self.font_size(theme);
        let font_family = self.font_family(theme);
        let measurer = SimpleTextMeasure;
        let color = self.color_override.unwrap_or_else(|| {
            if self.state.enabled {
                theme.colors.text_primary
            } else {
                theme.colors.text_disabled
            }
        });

        let text_w = measurer.measure_text(&self.text, fs, self.bold).0;
        let text_x = match self.align {
            TextAlign::Left => self.x,
            TextAlign::Center => self.x + (self.width - text_w) / 2.0,
            TextAlign::Right => self.x + self.width - text_w,
        };
        let text_y = self.y + (self.height - fs) / 2.0;

        painter.draw_text(
            &self.text,
            text_x,
            text_y,
            fs,
            color,
            font_family,
            self.bold,
        );
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
            _ => EventResponse::Ignored,
        }
    }
}
