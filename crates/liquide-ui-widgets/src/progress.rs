//! Progress bar widget.
//!
//! Determinate and indeterminate progress display. Inspired by
//! Qt's QProgressBar and GTK's GtkProgressBar.

use liquide_ui_core::{
    Constraints, Event, EventResponse, LayoutResult, Painter, UiColor, UiTheme, WidgetId,
    widget::{Widget, WidgetState},
};

/// Progress mode.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProgressMode {
    /// Shows progress as a fraction of `value / max`.
    Determinate { value: f32, max: f32 },
    /// Animated indeterminate spinner-bar.
    Indeterminate { phase: f32 },
}

/// A progress bar widget.
pub struct ProgressBar {
    state: WidgetState,
    mode: ProgressMode,
    show_label: bool,
    color_override: Option<UiColor>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl ProgressBar {
    pub fn new(value: f32, max: f32) -> Self {
        Self {
            state: WidgetState::new(WidgetId::new()),
            mode: ProgressMode::Determinate { value, max },
            show_label: false,
            color_override: None,
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        }
    }

    pub fn indeterminate() -> Self {
        Self {
            state: WidgetState::new(WidgetId::new()),
            mode: ProgressMode::Indeterminate { phase: 0.0 },
            show_label: false,
            color_override: None,
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        }
    }

    pub fn with_label(mut self, show: bool) -> Self {
        self.show_label = show;
        self
    }

    pub fn with_tooltip(mut self, text: impl Into<String>) -> Self {
        self.state.tooltip = Some(text.into());
        self
    }

    pub fn with_color(mut self, color: UiColor) -> Self {
        self.color_override = Some(color);
        self
    }

    /// Set the progress value (only relevant in Determinate mode).
    pub fn set_value(&mut self, value: f32) {
        if let ProgressMode::Determinate {
            value: ref mut v, ..
        } = self.mode
        {
            *v = value;
        }
    }

    /// Advance indeterminate phase by dt seconds.
    pub fn tick(&mut self, dt: f32) {
        if let ProgressMode::Indeterminate { ref mut phase } = self.mode {
            *phase = (*phase + dt) % 2.0;
        }
    }

    fn fraction(&self) -> f32 {
        match self.mode {
            ProgressMode::Determinate { value, max } if max > 0.0 => (value / max).clamp(0.0, 1.0),
            _ => 0.0,
        }
    }
}

impl Widget for ProgressBar {
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
    fn set_enabled(&mut self, _e: bool) {}
    fn focusable(&self) -> bool {
        false
    }
    fn tooltip(&self) -> Option<&str> {
        self.state.tooltip.as_deref()
    }

    fn measure(&self, constraints: &Constraints, _theme: &UiTheme) -> LayoutResult {
        let (w, h) = constraints.clamp(200.0, 8.0);
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
        let radius = self.height / 2.0;
        let bar_color = self.color_override.unwrap_or(colors.accent);

        // Track
        painter.fill_rounded_rect(
            self.x,
            self.y,
            self.width,
            self.height,
            radius,
            colors.surface_hover,
        );

        // Fill
        match self.mode {
            ProgressMode::Determinate { .. } => {
                let frac = self.fraction();
                if frac > 0.0 {
                    let fill_w = self.width * frac;
                    painter.fill_rounded_rect(
                        self.x,
                        self.y,
                        fill_w,
                        self.height,
                        radius,
                        bar_color,
                    );
                }
                // Label
                if self.show_label {
                    let pct = format!("{}%", (frac * 100.0) as u32);
                    let fs = self.height.max(10.0);
                    let tw = pct.len() as f32 * fs * 0.55;
                    let tx = self.x + (self.width - tw) / 2.0;
                    let ty = self.y + (self.height - fs) / 2.0;
                    painter.draw_text(
                        &pct,
                        tx,
                        ty,
                        fs,
                        colors.text_primary,
                        &theme.font_family,
                        false,
                    );
                }
            }
            ProgressMode::Indeterminate { phase } => {
                // Bouncing bar effect
                let bar_w = self.width * 0.3;
                let travel = self.width - bar_w;
                let t = if phase < 1.0 { phase } else { 2.0 - phase };
                let offset = travel * t;
                painter.fill_rounded_rect(
                    self.x + offset,
                    self.y,
                    bar_w,
                    self.height,
                    radius,
                    bar_color,
                );
            }
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
            _ => EventResponse::Ignored,
        }
    }
}
