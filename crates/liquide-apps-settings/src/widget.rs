//! Root widget for the Settings app — a minimal placeholder that exists so
//! [`crate::build_root`] can wire the app onto `liquide-app-harness`. The widget
//! tree does not render any settings UI yet; it simply satisfies the
//! `Widget` trait with a clear background so the harness's measure → layout
//! → paint pipeline is exercised at every frame.
//!
//! Rich UI (category sidebar, sections, entry editors) is follow-up work —
//! see the per-app deferral list in `.orchestration/logs/t9-e15.md`.

use liquide_ui_core::{
    Constraints, Event, EventResponse, LayoutResult, Painter, UiTheme, WidgetId,
    widget::{Widget, WidgetState},
};

/// Top-level root widget for the Settings window.
pub struct SettingsRoot {
    state: WidgetState,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl SettingsRoot {
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: WidgetState::new(WidgetId::new()),
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        }
    }
}

impl Default for SettingsRoot {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for SettingsRoot {
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

    fn measure(&self, constraints: &Constraints, _theme: &UiTheme) -> LayoutResult {
        let (w, h) = constraints.clamp(constraints.max_width, constraints.max_height);
        LayoutResult::new(w, h)
    }

    fn layout(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.x = x;
        self.y = y;
        self.width = w;
        self.height = h;
    }

    fn paint(&self, _painter: &mut Painter, _theme: &UiTheme) {
        // Placeholder — intentionally no draw commands. The harness's paint
        // buffer is zero-filled per frame so the presenter path is still
        // exercised. Pixel-content assertions intentionally avoided
        // (see t9-e13 integration notes).
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_measure_reports_non_zero_size() {
        let root = SettingsRoot::new();
        let cons = Constraints::new(0.0, 0.0, 800.0, 600.0);
        let result = root.measure(&cons, &UiTheme::default());
        assert!(result.width > 0.0);
        assert!(result.height > 0.0);
    }

    #[test]
    fn root_layout_records_rect() {
        let mut root = SettingsRoot::new();
        root.layout(10.0, 20.0, 300.0, 200.0);
        assert_eq!(root.x, 10.0);
        assert_eq!(root.width, 300.0);
    }
}
