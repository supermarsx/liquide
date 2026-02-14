//! Tooltip manager — singleton that tracks hover state and renders tooltips.
//!
//! ## Hot-path performance
//!
//! The `update()` method is called every frame and is designed to be
//! allocation-free. It only tracks timers and state transitions.
//! Paint commands are generated only when a tooltip is actually visible.

use crate::config::TooltipConfig;
use crate::position::{self, TooltipPosition, TooltipRect};
use liquide_ui_core::{Painter, UiColor, UiTheme, WidgetId};

/// Internal tooltip display state.
#[derive(Debug, Clone, Copy, PartialEq)]
enum TooltipState {
    /// No tooltip active.
    Hidden,
    /// Waiting for the show delay to expire.
    Pending { elapsed_ms: f32 },
    /// Fading in.
    FadingIn { elapsed_ms: f32 },
    /// Fully visible.
    Visible { elapsed_ms: f32 },
    /// Fading out.
    FadingOut { elapsed_ms: f32 },
}

/// Manages tooltip lifecycle across all widgets.
pub struct TooltipManager {
    config: TooltipConfig,
    state: TooltipState,
    /// The widget currently being hovered.
    hovered_widget: Option<WidgetId>,
    /// Tooltip text to display (only allocated when text changes).
    text: String,
    /// Anchor position of the hovered widget.
    anchor_x: f32,
    anchor_y: f32,
    anchor_w: f32,
    anchor_h: f32,
    /// Screen dimensions for edge clamping.
    screen_w: f32,
    screen_h: f32,
    /// Cached computed tooltip rect.
    cached_rect: Option<TooltipRect>,
}

impl TooltipManager {
    pub fn new(config: TooltipConfig) -> Self {
        Self {
            config,
            state: TooltipState::Hidden,
            hovered_widget: None,
            text: String::new(),
            anchor_x: 0.0,
            anchor_y: 0.0,
            anchor_w: 0.0,
            anchor_h: 0.0,
            screen_w: 1920.0,
            screen_h: 1080.0,
            cached_rect: None,
        }
    }

    /// Set the screen dimensions (call on resize).
    pub fn set_screen_size(&mut self, w: f32, h: f32) {
        self.screen_w = w;
        self.screen_h = h;
        self.cached_rect = None;
    }

    /// Notify the manager that a widget is being hovered.
    ///
    /// Call this from the event dispatch loop when `MouseEnter` is received
    /// on a widget that has tooltip text.
    pub fn on_hover_begin(
        &mut self,
        widget_id: WidgetId,
        tooltip_text: &str,
        anchor_x: f32,
        anchor_y: f32,
        anchor_w: f32,
        anchor_h: f32,
    ) {
        if !self.config.enabled || tooltip_text.is_empty() {
            return;
        }

        // If it's the same widget, don't reset the timer
        if self.hovered_widget == Some(widget_id) {
            return;
        }

        self.hovered_widget = Some(widget_id);
        self.text.clear();
        self.text.push_str(tooltip_text);
        self.anchor_x = anchor_x;
        self.anchor_y = anchor_y;
        self.anchor_w = anchor_w;
        self.anchor_h = anchor_h;
        self.cached_rect = None;

        // Start the show delay
        self.state = TooltipState::Pending { elapsed_ms: 0.0 };
    }

    /// Notify the manager that a widget is no longer hovered.
    pub fn on_hover_end(&mut self, widget_id: WidgetId) {
        if self.hovered_widget == Some(widget_id) {
            match self.state {
                TooltipState::Visible { .. } | TooltipState::FadingIn { .. } => {
                    self.state = TooltipState::FadingOut { elapsed_ms: 0.0 };
                }
                _ => {
                    self.state = TooltipState::Hidden;
                }
            }
            self.hovered_widget = None;
        }
    }

    /// Update tooltip timers. Call once per frame with the frame delta time.
    ///
    /// This is allocation-free in the normal case.
    #[inline]
    pub fn update(&mut self, dt_ms: f32) {
        self.state = match self.state {
            TooltipState::Hidden => TooltipState::Hidden,

            TooltipState::Pending { elapsed_ms } => {
                let new_elapsed = elapsed_ms + dt_ms;
                if new_elapsed >= self.config.show_delay_ms as f32 {
                    TooltipState::FadingIn { elapsed_ms: 0.0 }
                } else {
                    TooltipState::Pending { elapsed_ms: new_elapsed }
                }
            }

            TooltipState::FadingIn { elapsed_ms } => {
                let new_elapsed = elapsed_ms + dt_ms;
                if new_elapsed >= self.config.fade_in_ms as f32 {
                    TooltipState::Visible { elapsed_ms: 0.0 }
                } else {
                    TooltipState::FadingIn { elapsed_ms: new_elapsed }
                }
            }

            TooltipState::Visible { elapsed_ms } => {
                if self.config.display_duration_ms == 0 {
                    // Indefinite
                    TooltipState::Visible { elapsed_ms }
                } else {
                    let new_elapsed = elapsed_ms + dt_ms;
                    if new_elapsed >= self.config.display_duration_ms as f32 {
                        TooltipState::FadingOut { elapsed_ms: 0.0 }
                    } else {
                        TooltipState::Visible { elapsed_ms: new_elapsed }
                    }
                }
            }

            TooltipState::FadingOut { elapsed_ms } => {
                let new_elapsed = elapsed_ms + dt_ms;
                if new_elapsed >= self.config.fade_out_ms as f32 {
                    TooltipState::Hidden
                } else {
                    TooltipState::FadingOut { elapsed_ms: new_elapsed }
                }
            }
        };
    }

    /// Whether a tooltip is currently visible (including fade animations).
    pub fn is_visible(&self) -> bool {
        !matches!(self.state, TooltipState::Hidden | TooltipState::Pending { .. })
    }

    /// Current opacity (0.0 – 1.0) for the tooltip.
    pub fn opacity(&self) -> f32 {
        match self.state {
            TooltipState::Hidden | TooltipState::Pending { .. } => 0.0,
            TooltipState::FadingIn { elapsed_ms } => {
                (elapsed_ms / self.config.fade_in_ms as f32).clamp(0.0, 1.0)
            }
            TooltipState::Visible { .. } => 1.0,
            TooltipState::FadingOut { elapsed_ms } => {
                1.0 - (elapsed_ms / self.config.fade_out_ms as f32).clamp(0.0, 1.0)
            }
        }
    }

    /// Paint the tooltip overlay. Call this after all windows are painted.
    pub fn paint(&mut self, painter: &mut Painter, theme: &UiTheme) {
        if !self.is_visible() || self.text.is_empty() {
            return;
        }

        let opacity = self.opacity();
        let colors = &theme.colors;

        // Measure tooltip text
        let font_size = theme.font_size * 0.9;
        let char_w = font_size * 0.55;
        let text_w = (self.text.len() as f32 * char_w).min(self.config.max_width);
        let text_h = font_size + 4.0;
        let padding = self.config.padding;
        let tooltip_w = text_w + padding * 2.0;
        let tooltip_h = text_h + padding * 2.0;

        // Position
        let rect = self.cached_rect.get_or_insert_with(|| {
            position::compute_tooltip_position(
                self.anchor_x, self.anchor_y, self.anchor_w, self.anchor_h,
                tooltip_w, tooltip_h,
                self.config.offset_x, self.config.offset_y,
                self.screen_w, self.screen_h,
                TooltipPosition::Below,
            )
        });

        // Apply opacity to colors
        let alpha = (opacity * 255.0) as u8;
        let bg = colors.surface_elevated.with_alpha(
            ((colors.surface_elevated.a as f32) * opacity) as u8,
        );
        let border = colors.border.with_alpha(
            ((colors.border.a as f32) * opacity) as u8,
        );
        let text_color = colors.text_primary.with_alpha(alpha);

        // Shadow
        let shadow = UiColor::new(0, 0, 0, (40.0 * opacity) as u8);
        painter.fill_rounded_rect(
            rect.x + 1.0, rect.y + 2.0, rect.width, rect.height,
            self.config.corner_radius, shadow,
        );

        // Background
        painter.fill_rounded_rect(
            rect.x, rect.y, rect.width, rect.height,
            self.config.corner_radius, bg,
        );

        // Border
        painter.stroke_rounded_rect(
            rect.x, rect.y, rect.width, rect.height,
            self.config.corner_radius, border, 1.0,
        );

        // Text
        painter.draw_text(
            &self.text,
            rect.x + padding,
            rect.y + padding,
            font_size,
            text_color,
            &theme.font_family,
            false,
        );
    }

    /// Get the current tooltip config.
    pub fn config(&self) -> &TooltipConfig {
        &self.config
    }

    /// Update the tooltip config.
    pub fn set_config(&mut self, config: TooltipConfig) {
        self.config = config;
    }
}

impl Default for TooltipManager {
    fn default() -> Self {
        Self::new(TooltipConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tooltip_lifecycle() {
        let mut mgr = TooltipManager::new(TooltipConfig {
            show_delay_ms: 100,
            fade_in_ms: 50,
            display_duration_ms: 200,
            fade_out_ms: 50,
            ..TooltipConfig::default()
        });

        let wid = WidgetId::new();
        mgr.on_hover_begin(wid, "Hello", 100.0, 100.0, 80.0, 20.0);

        // Pending
        assert!(!mgr.is_visible());
        mgr.update(101.0); // past show_delay
        assert!(mgr.is_visible()); // now fading in
        assert!(mgr.opacity() < 1.0);

        mgr.update(51.0); // past fade_in
        assert_eq!(mgr.opacity(), 1.0); // fully visible

        mgr.update(201.0); // past display_duration
        assert!(mgr.is_visible()); // fading out
        assert!(mgr.opacity() < 1.0);

        mgr.update(51.0); // past fade_out
        assert!(!mgr.is_visible()); // hidden
    }

    #[test]
    fn test_hover_end_triggers_fadeout() {
        let mut mgr = TooltipManager::default();
        let wid = WidgetId::new();

        mgr.on_hover_begin(wid, "Test", 0.0, 0.0, 50.0, 20.0);
        mgr.update(600.0); // past delay + fade in
        mgr.update(200.0);
        assert!(mgr.is_visible());

        mgr.on_hover_end(wid);
        assert!(mgr.is_visible()); // still fading out
        mgr.update(200.0);
        assert!(!mgr.is_visible()); // gone
    }
}
