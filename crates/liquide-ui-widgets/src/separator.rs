//! Separator widget.
//!
//! Horizontal or vertical divider line. Inspired by Qt's QFrame
//! with HLine/VLine and GTK's GtkSeparator.

use liquide_ui_core::{
    Constraints, Event, EventResponse, LayoutResult, Painter, UiColor, UiTheme, WidgetId,
    widget::{Widget, WidgetState},
};

/// Separator orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeparatorKind {
    Horizontal,
    Vertical,
}

/// A thin line separator between content sections.
pub struct Separator {
    state: WidgetState,
    kind: SeparatorKind,
    thickness: f32,
    color_override: Option<UiColor>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl Separator {
    pub fn horizontal() -> Self {
        Self {
            state: WidgetState::new(WidgetId::new()),
            kind: SeparatorKind::Horizontal,
            thickness: 1.0,
            color_override: None,
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        }
    }

    pub fn vertical() -> Self {
        Self {
            state: WidgetState::new(WidgetId::new()),
            kind: SeparatorKind::Vertical,
            thickness: 1.0,
            color_override: None,
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        }
    }

    pub fn with_thickness(mut self, t: f32) -> Self {
        self.thickness = t;
        self
    }

    pub fn with_color(mut self, color: UiColor) -> Self {
        self.color_override = Some(color);
        self
    }
}

impl Widget for Separator {
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
        true
    }
    fn set_enabled(&mut self, _: bool) {}
    fn focusable(&self) -> bool {
        false
    }
    fn tooltip(&self) -> Option<&str> {
        None
    }

    fn measure(&self, constraints: &Constraints, _theme: &UiTheme) -> LayoutResult {
        match self.kind {
            SeparatorKind::Horizontal => {
                let (w, h) = constraints.clamp(100.0, self.thickness);
                LayoutResult::new(w, h)
            }
            SeparatorKind::Vertical => {
                let (w, h) = constraints.clamp(self.thickness, 100.0);
                LayoutResult::new(w, h)
            }
        }
    }

    fn layout(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.x = x;
        self.y = y;
        self.width = w;
        self.height = h;
    }

    fn paint(&self, painter: &mut Painter, theme: &UiTheme) {
        let color = self.color_override.unwrap_or(theme.colors.border);
        match self.kind {
            SeparatorKind::Horizontal => {
                let cy = self.y + self.height / 2.0;
                painter.draw_line(self.x, cy, self.x + self.width, cy, color, self.thickness);
            }
            SeparatorKind::Vertical => {
                let cx = self.x + self.width / 2.0;
                painter.draw_line(cx, self.y, cx, self.y + self.height, color, self.thickness);
            }
        }
    }

    fn handle_event(&mut self, _event: &Event) -> EventResponse {
        EventResponse::Ignored
    }
}
