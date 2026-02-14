//! Slider widget.
//!
//! A draggable value selector. Supports continuous and stepped modes.
//! Inspired by Qt's QSlider and GTK's GtkScale.

use liquide_ui_core::{
    Constraints, Event, EventResponse, Key, LayoutResult, Painter, UiColor, UiTheme, WidgetId,
    widget::{Widget, WidgetState},
};

/// Slider orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliderOrientation {
    Horizontal,
    Vertical,
}

/// A slider widget for selecting a numeric value within a range.
pub struct Slider {
    state: WidgetState,
    value: f32,
    min: f32,
    max: f32,
    step: Option<f32>,
    orientation: SliderOrientation,
    show_value: bool,
    on_change: Option<Box<dyn FnMut(f32) + Send>>,
    dragging: bool,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

const TRACK_HEIGHT: f32 = 4.0;
const THUMB_RADIUS: f32 = 8.0;

impl Slider {
    pub fn new(min: f32, max: f32, value: f32) -> Self {
        Self {
            state: WidgetState::new(WidgetId::new()),
            value: value.clamp(min, max),
            min,
            max,
            step: None,
            orientation: SliderOrientation::Horizontal,
            show_value: false,
            on_change: None,
            dragging: false,
            x: 0.0, y: 0.0, width: 0.0, height: 0.0,
        }
    }

    pub fn with_step(mut self, step: f32) -> Self {
        self.step = Some(step);
        self
    }

    pub fn with_orientation(mut self, orient: SliderOrientation) -> Self {
        self.orientation = orient;
        self
    }

    pub fn show_value(mut self, show: bool) -> Self {
        self.show_value = show;
        self
    }

    pub fn with_tooltip(mut self, text: impl Into<String>) -> Self {
        self.state.tooltip = Some(text.into());
        self
    }

    pub fn on_change(mut self, f: impl FnMut(f32) + Send + 'static) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }

    pub fn value(&self) -> f32 { self.value }

    pub fn set_value(&mut self, v: f32) {
        self.value = self.snap(v);
    }

    fn snap(&self, v: f32) -> f32 {
        let v = v.clamp(self.min, self.max);
        if let Some(step) = self.step {
            let steps = ((v - self.min) / step).round();
            (self.min + steps * step).clamp(self.min, self.max)
        } else {
            v
        }
    }

    fn fraction(&self) -> f32 {
        if (self.max - self.min).abs() < f32::EPSILON { 0.0 }
        else { (self.value - self.min) / (self.max - self.min) }
    }

    fn value_from_pos(&self, pos: f32) -> f32 {
        let (start, len) = match self.orientation {
            SliderOrientation::Horizontal => (self.x + THUMB_RADIUS, self.width - THUMB_RADIUS * 2.0),
            SliderOrientation::Vertical => (self.y + THUMB_RADIUS, self.height - THUMB_RADIUS * 2.0),
        };
        if len <= 0.0 { return self.min; }
        let t = ((pos - start) / len).clamp(0.0, 1.0);
        self.snap(self.min + t * (self.max - self.min))
    }

    fn set_value_notifying(&mut self, v: f32) {
        let new_val = self.snap(v);
        if (new_val - self.value).abs() > f32::EPSILON {
            self.value = new_val;
            if let Some(cb) = &mut self.on_change {
                cb(new_val);
            }
        }
    }

    fn small_step(&self) -> f32 {
        self.step.unwrap_or((self.max - self.min) / 100.0)
    }
}

impl Widget for Slider {
    fn id(&self) -> WidgetId { self.state.id }
    fn visible(&self) -> bool { self.state.visible }
    fn set_visible(&mut self, v: bool) { self.state.visible = v; }
    fn enabled(&self) -> bool { self.state.enabled }
    fn set_enabled(&mut self, e: bool) { self.state.enabled = e; }
    fn focusable(&self) -> bool { true }
    fn tooltip(&self) -> Option<&str> { self.state.tooltip.as_deref() }

    fn measure(&self, constraints: &Constraints, _theme: &UiTheme) -> LayoutResult {
        match self.orientation {
            SliderOrientation::Horizontal => {
                let (w, h) = constraints.clamp(200.0, THUMB_RADIUS * 2.0 + 4.0);
                LayoutResult::new(w, h)
            }
            SliderOrientation::Vertical => {
                let (w, h) = constraints.clamp(THUMB_RADIUS * 2.0 + 4.0, 200.0);
                LayoutResult::new(w, h)
            }
        }
    }

    fn layout(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.x = x; self.y = y; self.width = w; self.height = h;
    }

    fn paint(&self, painter: &mut Painter, theme: &UiTheme) {
        let colors = &theme.colors;
        let frac = self.fraction();

        match self.orientation {
            SliderOrientation::Horizontal => {
                let cy = self.y + self.height / 2.0;
                let track_y = cy - TRACK_HEIGHT / 2.0;
                let usable = self.width - THUMB_RADIUS * 2.0;
                let thumb_cx = self.x + THUMB_RADIUS + usable * frac;

                // Track background
                painter.fill_rounded_rect(
                    self.x, track_y, self.width, TRACK_HEIGHT,
                    TRACK_HEIGHT / 2.0, colors.surface_hover,
                );
                // Track fill
                painter.fill_rounded_rect(
                    self.x, track_y, thumb_cx - self.x, TRACK_HEIGHT,
                    TRACK_HEIGHT / 2.0, colors.accent,
                );
                // Thumb
                let thumb_color = if self.dragging {
                    colors.accent
                } else if self.state.hovered {
                    colors.accent
                } else {
                    colors.surface_elevated
                };
                painter.fill_circle(thumb_cx, cy, THUMB_RADIUS, thumb_color);
                painter.stroke_rounded_rect(
                    thumb_cx - THUMB_RADIUS, cy - THUMB_RADIUS,
                    THUMB_RADIUS * 2.0, THUMB_RADIUS * 2.0,
                    THUMB_RADIUS, colors.accent, 2.0,
                );
                // Focus ring
                if self.state.focused {
                    painter.stroke_rounded_rect(
                        thumb_cx - THUMB_RADIUS - 2.0, cy - THUMB_RADIUS - 2.0,
                        THUMB_RADIUS * 2.0 + 4.0, THUMB_RADIUS * 2.0 + 4.0,
                        THUMB_RADIUS + 2.0, colors.focus_ring, 1.5,
                    );
                }
                // Value label
                if self.show_value {
                    let txt = format!("{:.1}", self.value);
                    let fs = theme.font_size * 0.8;
                    painter.draw_text(&txt, thumb_cx - 10.0, self.y - fs - 4.0, fs, colors.text_secondary, &theme.font_family, false);
                }
            }
            SliderOrientation::Vertical => {
                let cx = self.x + self.width / 2.0;
                let track_x = cx - TRACK_HEIGHT / 2.0;
                let usable = self.height - THUMB_RADIUS * 2.0;
                // Vertical slider: bottom=min, top=max
                let thumb_cy = self.y + self.height - THUMB_RADIUS - usable * frac;

                painter.fill_rounded_rect(
                    track_x, self.y, TRACK_HEIGHT, self.height,
                    TRACK_HEIGHT / 2.0, colors.surface_hover,
                );
                painter.fill_rounded_rect(
                    track_x, thumb_cy, TRACK_HEIGHT, self.y + self.height - thumb_cy,
                    TRACK_HEIGHT / 2.0, colors.accent,
                );
                painter.fill_circle(cx, thumb_cy, THUMB_RADIUS, colors.surface_elevated);
                painter.stroke_rounded_rect(
                    cx - THUMB_RADIUS, thumb_cy - THUMB_RADIUS,
                    THUMB_RADIUS * 2.0, THUMB_RADIUS * 2.0,
                    THUMB_RADIUS, colors.accent, 2.0,
                );
            }
        }
    }

    fn handle_event(&mut self, event: &Event) -> EventResponse {
        match event {
            Event::MouseEnter => { self.state.hovered = true; EventResponse::Consumed }
            Event::MouseLeave => { self.state.hovered = false; EventResponse::Consumed }
            Event::MouseDown { x, y, .. } if self.state.enabled => {
                self.dragging = true;
                let pos = match self.orientation {
                    SliderOrientation::Horizontal => *x,
                    SliderOrientation::Vertical => *y,
                };
                self.set_value_notifying(self.value_from_pos(pos));
                EventResponse::RequestFocus
            }
            Event::MouseUp { .. } => {
                self.dragging = false;
                EventResponse::Consumed
            }
            Event::MouseMove { x, y, .. } if self.dragging => {
                let pos = match self.orientation {
                    SliderOrientation::Horizontal => *x,
                    SliderOrientation::Vertical => *y,
                };
                self.set_value_notifying(self.value_from_pos(pos));
                EventResponse::Consumed
            }
            Event::FocusIn => { self.state.focused = true; EventResponse::Consumed }
            Event::FocusOut => { self.state.focused = false; EventResponse::Consumed }
            Event::KeyDown { key, .. } if self.state.focused && self.state.enabled => {
                match key {
                    Key::ArrowRight | Key::ArrowUp => {
                        self.set_value_notifying(self.value + self.small_step());
                        EventResponse::Consumed
                    }
                    Key::ArrowLeft | Key::ArrowDown => {
                        self.set_value_notifying(self.value - self.small_step());
                        EventResponse::Consumed
                    }
                    Key::Home => {
                        self.set_value_notifying(self.min);
                        EventResponse::Consumed
                    }
                    Key::End => {
                        self.set_value_notifying(self.max);
                        EventResponse::Consumed
                    }
                    _ => EventResponse::Ignored,
                }
            }
            _ => EventResponse::Ignored,
        }
    }
}
