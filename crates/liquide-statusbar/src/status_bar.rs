//! The assembled status bar — combines app menu, menu bar, indicators, and theme toggle.

use crate::app_menu::AppMenu;
use crate::config::StatusBarConfig;
use crate::indicator::SystemIndicator;
use crate::menu_bar::MenuBar;
use crate::theme_toggle::ThemeToggle;
use liquide_ui_core::{Painter, UiColor, UiTheme};
use liquide_ui_core::theme::ThemeMode;

/// The full status bar widget.
pub struct StatusBar {
    pub config: StatusBarConfig,
    pub app_menu: AppMenu,
    pub menu_bar: MenuBar,
    pub indicators: Vec<SystemIndicator>,
    pub theme_toggle: ThemeToggle,
    dirty: bool,
}

impl StatusBar {
    pub fn new(config: StatusBarConfig) -> Self {
        let app_name = config.app_name.clone();
        let mut indicators = Vec::new();

        if config.show_clock {
            indicators.push(SystemIndicator::clock());
        }
        if config.show_notifications {
            indicators.push(SystemIndicator::notification());
        }
        if config.show_tray {
            indicators.push(SystemIndicator::wifi(100));
            indicators.push(SystemIndicator::battery(85));
            indicators.push(SystemIndicator::volume(70));
        }

        Self {
            config: config.clone(),
            app_menu: AppMenu::new(app_name),
            menu_bar: MenuBar::default_desktop(),
            indicators,
            theme_toggle: ThemeToggle::new(ThemeMode::Dark),
            dirty: true,
        }
    }

    /// Update the clock.
    pub fn update_clock(&mut self, timestamp_us: u64) {
        for ind in &mut self.indicators {
            if let crate::indicator::IndicatorKind::Clock { ref mut timestamp_us: ts, .. } = ind.kind {
                *ts = timestamp_us;
                self.dirty = true;
            }
        }
    }

    /// Update notification count.
    pub fn update_notifications(&mut self, count: u32) {
        for ind in &mut self.indicators {
            if let crate::indicator::IndicatorKind::Notification { ref mut unread_count, .. } = ind.kind {
                *unread_count = count;
                self.dirty = true;
            }
        }
    }

    /// Update WiFi quality.
    pub fn update_wifi(&mut self, quality: u8) {
        for ind in &mut self.indicators {
            if let crate::indicator::IndicatorKind::Wifi { ref mut quality_percent, .. } = ind.kind {
                *quality_percent = quality;
                self.dirty = true;
            }
        }
    }

    /// Set the theme mode from the toggle.
    pub fn set_theme_mode(&mut self, mode: ThemeMode) {
        self.theme_toggle.current_mode = mode;
        self.dirty = true;
    }

    /// Whether the bar needs repainting.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Clear the dirty flag.
    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    /// Paint the full status bar.
    ///
    /// Layout: `[ AppName | File Edit View Window Help | ........ | 🌙 🔔 ▂▄▆█ 🔋 14:30 ]`
    pub fn paint(
        &self,
        painter: &mut Painter,
        theme: &UiTheme,
        screen_width: f32,
    ) {
        if !self.config.enabled {
            return;
        }

        let bar_h = self.config.height;
        let bar_y = 0.0;
        let colors = &theme.colors;
        let padding = self.config.padding;
        let spacing = self.config.item_spacing;

        // Background (glass-like)
        painter.fill_rect(0.0, bar_y, screen_width, bar_h, colors.glass_tint);

        // Bottom border
        painter.draw_line(
            0.0, bar_y + bar_h - 0.5,
            screen_width, bar_y + bar_h - 0.5,
            colors.border_subtle, 1.0,
        );

        // === LEFT SIDE: App name + menu bar ===
        let mut left_x = padding;

        if self.config.show_app_menu {
            let app_name_w = self.app_menu.paint(painter, theme, left_x, bar_y, bar_h);
            left_x += app_name_w + spacing;
        }

        // Menu bar (File, Edit, View, …)
        let _menu_w = self.menu_bar.paint(painter, theme, left_x, bar_y, bar_h);

        // === RIGHT SIDE: Indicators + theme toggle ===
        let mut right_x = screen_width - padding;

        // Clock (rightmost)
        for ind in self.indicators.iter().rev() {
            let w = ind.paint(painter, theme, 0.0, 0.0, 0.0); // measure only
            right_x -= w + spacing;
        }

        // Actually paint right-to-left
        right_x = screen_width - padding;
        for ind in self.indicators.iter().rev() {
            // Estimate width
            let estimated_w = match &ind.kind {
                crate::indicator::IndicatorKind::Clock { .. } => 42.0,
                crate::indicator::IndicatorKind::Battery { .. } => 48.0,
                crate::indicator::IndicatorKind::Wifi { .. } => 28.0,
                crate::indicator::IndicatorKind::Notification { .. } => 28.0,
                crate::indicator::IndicatorKind::Volume { .. } => 20.0,
            };
            right_x -= estimated_w;
            ind.paint(painter, theme, right_x, bar_y, bar_h);
            right_x -= spacing;
        }

        // Theme toggle (before indicators)
        if self.config.show_theme_toggle {
            right_x -= 28.0;
            self.theme_toggle.paint(painter, theme, right_x, bar_y, bar_h);
        }
    }
}

impl Default for StatusBar {
    fn default() -> Self {
        Self::new(StatusBarConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_bar_default() {
        let bar = StatusBar::default();
        assert!(bar.config.enabled);
        assert!(bar.config.show_app_menu);
        assert_eq!(bar.config.app_name, "Liquide");
        assert!(!bar.indicators.is_empty());
    }

    #[test]
    fn test_theme_toggle() {
        let mut bar = StatusBar::default();
        bar.set_theme_mode(ThemeMode::Light);
        assert_eq!(bar.theme_toggle.current_mode, ThemeMode::Light);
    }
}
