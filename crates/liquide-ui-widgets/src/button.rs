//! Push button widget — the most fundamental interactive widget.
//!
//! Supports multiple visual variants (Primary, Secondary, Ghost, Danger),
//! icons, disabled state, hover/press animation, and keyboard activation.
//! Inspired by Qt's QPushButton and GTK's GtkButton.

use liquide_ui_core::{
    Constraints, Event, EventResponse, LayoutResult, Painter, UiColor, UiTheme, WidgetId,
    widget::{Widget, WidgetState},
};

/// Visual variant of a button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ButtonVariant {
    /// Filled accent color — for primary actions.
    Primary,
    /// Subtle surface background — for secondary actions.
    Secondary,
    /// No background, just text — for tertiary actions.
    Ghost,
    /// Red/destructive — for dangerous actions.
    Danger,
}

impl Default for ButtonVariant {
    fn default() -> Self {
        Self::Secondary
    }
}

/// Resolved style for a button (computed from theme + variant + state).
#[derive(Debug, Clone, Copy)]
pub struct ButtonStyle {
    pub background: UiColor,
    pub foreground: UiColor,
    pub border: UiColor,
    pub border_width: f32,
    pub radius: f32,
}

impl ButtonStyle {
    /// Resolve the style from theme + variant + widget state.
    pub fn resolve(theme: &UiTheme, variant: ButtonVariant, state: &WidgetState) -> Self {
        let colors = &theme.colors;
        let (bg, fg, border) = match variant {
            ButtonVariant::Primary => {
                if !state.enabled {
                    (colors.accent.with_alpha(77), colors.text_on_accent.with_alpha(128), UiColor::transparent())
                } else if state.pressed {
                    (colors.accent_active, colors.text_on_accent, UiColor::transparent())
                } else if state.hovered {
                    (colors.accent_hover, colors.text_on_accent, UiColor::transparent())
                } else {
                    (colors.accent, colors.text_on_accent, UiColor::transparent())
                }
            }
            ButtonVariant::Secondary => {
                if !state.enabled {
                    (colors.surface.with_alpha(10), colors.text_disabled, colors.border_subtle)
                } else if state.pressed {
                    (colors.surface_active, colors.text_primary, colors.border)
                } else if state.hovered {
                    (colors.surface_hover, colors.text_primary, colors.border)
                } else {
                    (colors.surface, colors.text_primary, colors.border_subtle)
                }
            }
            ButtonVariant::Ghost => {
                if !state.enabled {
                    (UiColor::transparent(), colors.text_disabled, UiColor::transparent())
                } else if state.pressed {
                    (colors.surface_active, colors.text_primary, UiColor::transparent())
                } else if state.hovered {
                    (colors.surface_hover, colors.text_primary, UiColor::transparent())
                } else {
                    (UiColor::transparent(), colors.text_secondary, UiColor::transparent())
                }
            }
            ButtonVariant::Danger => {
                if !state.enabled {
                    (colors.error.with_alpha(77), colors.text_on_accent.with_alpha(128), UiColor::transparent())
                } else if state.pressed {
                    (UiColor::new(200, 50, 40, 255), colors.text_on_accent, UiColor::transparent())
                } else if state.hovered {
                    (UiColor::new(255, 90, 80, 255), colors.text_on_accent, UiColor::transparent())
                } else {
                    (colors.error, colors.text_on_accent, UiColor::transparent())
                }
            }
        };

        Self {
            background: bg,
            foreground: fg,
            border,
            border_width: if border.a > 0 { 1.0 } else { 0.0 },
            radius: theme.radius_md,
        }
    }
}

/// A push button widget.
pub struct Button {
    state: WidgetState,
    label: String,
    variant: ButtonVariant,
    icon_id: Option<u32>,
    on_click: Option<Box<dyn FnMut() + Send>>,
    // Layout
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    padding_h: f32,
    padding_v: f32,
}

impl Button {
    /// Create a new button with a text label.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            state: WidgetState::new(WidgetId::new()),
            label: label.into(),
            variant: ButtonVariant::Secondary,
            icon_id: None,
            on_click: None,
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
            padding_h: 16.0,
            padding_v: 6.0,
        }
    }

    /// Set the button variant.
    pub fn with_variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Set an icon (icon atlas index).
    pub fn with_icon(mut self, icon_id: u32) -> Self {
        self.icon_id = Some(icon_id);
        self
    }

    /// Set a tooltip.
    pub fn with_tooltip(mut self, text: impl Into<String>) -> Self {
        self.state.tooltip = Some(text.into());
        self
    }

    /// Set a click handler.
    pub fn on_click(mut self, f: impl FnMut() + Send + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }

    /// The button's label text.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Set the label text.
    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = label.into();
    }

    /// The button variant.
    pub fn variant(&self) -> ButtonVariant {
        self.variant
    }
}

impl Widget for Button {
    fn id(&self) -> WidgetId { self.state.id }
    fn visible(&self) -> bool { self.state.visible }
    fn set_visible(&mut self, v: bool) { self.state.visible = v; }
    fn enabled(&self) -> bool { self.state.enabled }
    fn set_enabled(&mut self, e: bool) { self.state.enabled = e; }
    fn focusable(&self) -> bool { true }
    fn tooltip(&self) -> Option<&str> { self.state.tooltip.as_deref() }

    fn measure(&self, constraints: &Constraints, theme: &UiTheme) -> LayoutResult {
        let char_w = theme.font_size * 0.55;
        let text_w = self.label.len() as f32 * char_w;
        let icon_w = if self.icon_id.is_some() { theme.font_size + 4.0 } else { 0.0 };
        let w = text_w + icon_w + self.padding_h * 2.0;
        let h = theme.font_size + self.padding_v * 2.0;
        let (w, h) = constraints.clamp(w, h);
        LayoutResult::new(w, h)
    }

    fn layout(&mut self, x: f32, y: f32, width: f32, height: f32) {
        self.x = x;
        self.y = y;
        self.width = width;
        self.height = height;
    }

    fn paint(&self, painter: &mut Painter, theme: &UiTheme) {
        let style = ButtonStyle::resolve(theme, self.variant, &self.state);

        // Background
        painter.fill_rounded_rect(self.x, self.y, self.width, self.height, style.radius, style.background);

        // Border
        if style.border_width > 0.0 {
            painter.stroke_rounded_rect(
                self.x, self.y, self.width, self.height,
                style.radius, style.border, style.border_width,
            );
        }

        // Icon
        let mut text_x = self.x + self.padding_h;
        if let Some(icon_id) = self.icon_id {
            let icon_size = theme.font_size;
            let icon_y = self.y + (self.height - icon_size) / 2.0;
            painter.draw_icon(icon_id, text_x, icon_y, icon_size, style.foreground);
            text_x += icon_size + 4.0;
        }

        // Label
        let text_y = self.y + (self.height - theme.font_size) / 2.0;
        painter.draw_text(&self.label, text_x, text_y, theme.font_size, style.foreground, &theme.font_family, false);
    }

    fn handle_event(&mut self, event: &Event) -> EventResponse {
        match event {
            Event::MouseEnter => {
                self.state.hovered = true;
                EventResponse::Consumed
            }
            Event::MouseLeave => {
                self.state.hovered = false;
                self.state.pressed = false;
                EventResponse::Consumed
            }
            Event::MouseDown { .. } if self.state.enabled => {
                self.state.pressed = true;
                EventResponse::Consumed
            }
            Event::MouseUp { .. } if self.state.enabled && self.state.pressed => {
                self.state.pressed = false;
                if let Some(cb) = &mut self.on_click {
                    cb();
                }
                EventResponse::Consumed
            }
            Event::KeyDown { key, .. } if self.state.focused && self.state.enabled => {
                if matches!(key, liquide_ui_core::Key::Enter | liquide_ui_core::Key::Space) {
                    if let Some(cb) = &mut self.on_click {
                        cb();
                    }
                    return EventResponse::Consumed;
                }
                EventResponse::Ignored
            }
            _ => EventResponse::Ignored,
        }
    }
}
