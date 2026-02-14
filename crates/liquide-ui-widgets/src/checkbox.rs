//! Checkbox (tick box) widget.
//!
//! A boolean toggle with an optional label. Supports indeterminate state.
//! Inspired by Qt's QCheckBox and GTK's GtkCheckButton.

use liquide_ui_core::{
    Constraints, Event, EventResponse, Key, LayoutResult, Painter, UiColor, UiTheme, WidgetId,
    widget::{Widget, WidgetState},
};

/// Checkbox checked state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckState {
    Unchecked,
    Checked,
    Indeterminate,
}

/// A checkbox widget with optional label.
pub struct Checkbox {
    state: WidgetState,
    check_state: CheckState,
    label: String,
    on_toggle: Option<Box<dyn FnMut(CheckState) + Send>>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl Checkbox {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            state: WidgetState::new(WidgetId::new()),
            check_state: CheckState::Unchecked,
            label: label.into(),
            on_toggle: None,
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        }
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.check_state = if checked { CheckState::Checked } else { CheckState::Unchecked };
        self
    }

    pub fn with_state(mut self, s: CheckState) -> Self {
        self.check_state = s;
        self
    }

    pub fn with_tooltip(mut self, text: impl Into<String>) -> Self {
        self.state.tooltip = Some(text.into());
        self
    }

    pub fn on_toggle(mut self, f: impl FnMut(CheckState) + Send + 'static) -> Self {
        self.on_toggle = Some(Box::new(f));
        self
    }

    pub fn is_checked(&self) -> bool { self.check_state == CheckState::Checked }

    pub fn check_state(&self) -> CheckState { self.check_state }

    fn toggle(&mut self) {
        self.check_state = match self.check_state {
            CheckState::Unchecked | CheckState::Indeterminate => CheckState::Checked,
            CheckState::Checked => CheckState::Unchecked,
        };
        if let Some(cb) = &mut self.on_toggle {
            cb(self.check_state);
        }
    }
}

const BOX_SIZE: f32 = 18.0;
const BOX_LABEL_GAP: f32 = 8.0;

impl Widget for Checkbox {
    fn id(&self) -> WidgetId { self.state.id }
    fn visible(&self) -> bool { self.state.visible }
    fn set_visible(&mut self, v: bool) { self.state.visible = v; }
    fn enabled(&self) -> bool { self.state.enabled }
    fn set_enabled(&mut self, e: bool) { self.state.enabled = e; }
    fn focusable(&self) -> bool { true }
    fn tooltip(&self) -> Option<&str> { self.state.tooltip.as_deref() }

    fn measure(&self, constraints: &Constraints, theme: &UiTheme) -> LayoutResult {
        let label_w = self.label.len() as f32 * theme.font_size * 0.55;
        let total_w = BOX_SIZE + BOX_LABEL_GAP + label_w;
        let h = BOX_SIZE.max(theme.font_size + 4.0);
        let (w, h) = constraints.clamp(total_w, h);
        LayoutResult::new(w, h)
    }

    fn layout(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.x = x; self.y = y; self.width = w; self.height = h;
    }

    fn paint(&self, painter: &mut Painter, theme: &UiTheme) {
        let colors = &theme.colors;
        let radius = theme.radius_sm;

        // Checkbox box
        let box_y = self.y + (self.height - BOX_SIZE) / 2.0;
        let (bg, border) = match self.check_state {
            CheckState::Checked | CheckState::Indeterminate => (colors.accent, colors.accent),
            CheckState::Unchecked => {
                if self.state.hovered { (colors.surface_hover, colors.border) }
                else { (colors.surface, colors.border) }
            }
        };
        painter.fill_rounded_rect(self.x, box_y, BOX_SIZE, BOX_SIZE, radius, bg);
        painter.stroke_rounded_rect(self.x, box_y, BOX_SIZE, BOX_SIZE, radius, border, 1.0);

        // Checkmark / dash
        let mark_color = colors.text_on_accent;
        match self.check_state {
            CheckState::Checked => {
                // Draw a checkmark as two lines
                let cx = self.x + 4.0;
                let cy = box_y + BOX_SIZE * 0.5;
                painter.draw_line(cx, cy, cx + 3.0, cy + 4.0, mark_color, 2.0);
                painter.draw_line(cx + 3.0, cy + 4.0, cx + 10.0, cy - 4.0, mark_color, 2.0);
            }
            CheckState::Indeterminate => {
                // Draw a horizontal dash
                let dash_y = box_y + BOX_SIZE / 2.0;
                painter.draw_line(self.x + 4.0, dash_y, self.x + BOX_SIZE - 4.0, dash_y, mark_color, 2.0);
            }
            CheckState::Unchecked => {}
        }

        // Focus ring
        if self.state.focused {
            painter.stroke_rounded_rect(
                self.x - 2.0, box_y - 2.0, BOX_SIZE + 4.0, BOX_SIZE + 4.0,
                radius + 1.0, colors.focus_ring, 1.5,
            );
        }

        // Label
        if !self.label.is_empty() {
            let text_x = self.x + BOX_SIZE + BOX_LABEL_GAP;
            let text_y = self.y + (self.height - theme.font_size) / 2.0;
            let text_color = if self.state.enabled { colors.text_primary } else { colors.text_disabled };
            painter.draw_text(&self.label, text_x, text_y, theme.font_size, text_color, &theme.font_family, false);
        }
    }

    fn handle_event(&mut self, event: &Event) -> EventResponse {
        match event {
            Event::MouseEnter => { self.state.hovered = true; EventResponse::Consumed }
            Event::MouseLeave => { self.state.hovered = false; EventResponse::Consumed }
            Event::MouseDown { .. } => { self.state.pressed = true; EventResponse::RequestFocus }
            Event::MouseUp { .. } if self.state.pressed => {
                self.state.pressed = false;
                if self.state.enabled {
                    self.toggle();
                }
                EventResponse::Consumed
            }
            Event::FocusIn => { self.state.focused = true; EventResponse::Consumed }
            Event::FocusOut => { self.state.focused = false; EventResponse::Consumed }
            Event::KeyDown { key: Key::Space, .. } if self.state.focused && self.state.enabled => {
                self.toggle();
                EventResponse::Consumed
            }
            _ => EventResponse::Ignored,
        }
    }
}
