//! Theme / dark-light mode toggle widget for the status bar.

use liquide_ui_core::theme::ThemeMode;
use liquide_ui_core::{Painter, UiColor, UiTheme};
use serde::{Deserialize, Serialize};

/// Theme toggle state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeToggle {
    pub current_mode: ThemeMode,
    pub open: bool,
    pub hover_index: Option<usize>,
}

impl ThemeToggle {
    pub fn new(mode: ThemeMode) -> Self {
        Self {
            current_mode: mode,
            open: false,
            hover_index: None,
        }
    }

    pub fn toggle_dropdown(&mut self) {
        self.open = !self.open;
        self.hover_index = None;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.hover_index = None;
    }

    fn icon(&self) -> &str {
        match self.current_mode {
            ThemeMode::Dark => "🌙",
            ThemeMode::Light => "☀️",
            ThemeMode::System => "🖥️",
        }
    }

    fn mode_label(mode: ThemeMode) -> &'static str {
        match mode {
            ThemeMode::Dark => "Dark Mode",
            ThemeMode::Light => "Light Mode",
            ThemeMode::System => "System Default",
        }
    }

    /// Paint the toggle button and dropdown. Returns width consumed.
    pub fn paint(
        &self,
        painter: &mut Painter,
        theme: &UiTheme,
        x: f32,
        bar_y: f32,
        bar_h: f32,
    ) -> f32 {
        let colors = &theme.colors;
        let font_size = theme.font_size;
        let text_y = bar_y + (bar_h - font_size) / 2.0;

        // Icon button
        let icon = self.icon();
        let btn_w = 28.0;
        painter.draw_text(
            icon,
            x + 4.0,
            text_y,
            font_size,
            colors.text_primary,
            &theme.font_family,
            false,
        );

        // Dropdown
        if self.open {
            let modes = [ThemeMode::Dark, ThemeMode::Light, ThemeMode::System];
            let item_h = font_size + 12.0;
            let menu_w = 160.0;
            let menu_h = modes.len() as f32 * item_h + 8.0;
            let menu_x = x - menu_w + btn_w; // right-aligned
            let menu_y = bar_y + bar_h + 2.0;
            let radius = theme.radius_md;

            // Shadow
            painter.fill_rounded_rect(
                menu_x + 1.0,
                menu_y + 2.0,
                menu_w,
                menu_h,
                radius,
                UiColor::new(0, 0, 0, 50),
            );
            // Background
            painter.fill_rounded_rect(
                menu_x,
                menu_y,
                menu_w,
                menu_h,
                radius,
                colors.surface_elevated,
            );
            painter.stroke_rounded_rect(menu_x, menu_y, menu_w, menu_h, radius, colors.border, 1.0);

            for (i, mode) in modes.iter().enumerate() {
                let iy = menu_y + 4.0 + i as f32 * item_h;
                let is_hover = self.hover_index == Some(i);
                let is_current = *mode == self.current_mode;

                if is_hover {
                    painter.fill_rounded_rect(
                        menu_x + 4.0,
                        iy,
                        menu_w - 8.0,
                        item_h,
                        radius * 0.5,
                        colors.accent,
                    );
                }

                // Checkmark for current mode
                if is_current {
                    painter.draw_text(
                        "✓",
                        menu_x + 8.0,
                        iy + (item_h - font_size) / 2.0,
                        font_size,
                        colors.accent,
                        &theme.font_family,
                        false,
                    );
                }

                let tc = if is_hover {
                    colors.text_on_accent
                } else {
                    colors.text_primary
                };
                painter.draw_text(
                    Self::mode_label(*mode),
                    menu_x + 28.0,
                    iy + (item_h - font_size) / 2.0,
                    font_size,
                    tc,
                    &theme.font_family,
                    false,
                );
            }
        }

        btn_w
    }
}

impl Default for ThemeToggle {
    fn default() -> Self {
        Self::new(ThemeMode::Dark)
    }
}
