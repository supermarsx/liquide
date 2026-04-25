//! Application menu — the bold app name on the left of the status bar.
//!
//! Clicking the app name opens a global application menu (About, Preferences,
//! Quit, etc.) — just like macOS.

use serde::{Deserialize, Serialize};

/// An action triggered by a menu item.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MenuAction {
    /// Open the "About" dialog.
    About,
    /// Open preferences / settings.
    Preferences,
    /// Hide the application.
    Hide,
    /// Quit the application.
    Quit,
    /// Lock the session.
    LockSession,
    /// Logout.
    Logout,
    /// Shutdown the system.
    Shutdown,
    /// Restart the system.
    Restart,
    /// Custom action with an arbitrary string id.
    Custom(String),
}

/// A single item in the app menu.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppMenuItem {
    pub label: String,
    pub action: MenuAction,
    pub enabled: bool,
    pub shortcut: Option<String>,
    pub separator_after: bool,
}

impl AppMenuItem {
    pub fn new(label: impl Into<String>, action: MenuAction) -> Self {
        Self {
            label: label.into(),
            action,
            enabled: true,
            shortcut: None,
            separator_after: false,
        }
    }

    pub fn with_shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    pub fn with_separator(mut self) -> Self {
        self.separator_after = true;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

/// The application menu that appears when clicking the app name.
pub struct AppMenu {
    pub app_name: String,
    pub items: Vec<AppMenuItem>,
    pub open: bool,
    pub hover_index: Option<usize>,
}

impl AppMenu {
    pub fn new(app_name: impl Into<String>) -> Self {
        let app_name = app_name.into();
        Self {
            items: Self::default_items(&app_name),
            app_name,
            open: false,
            hover_index: None,
        }
    }

    fn default_items(app_name: &str) -> Vec<AppMenuItem> {
        vec![
            AppMenuItem::new(format!("About {app_name}"), MenuAction::About).with_separator(),
            AppMenuItem::new("Preferences…", MenuAction::Preferences)
                .with_shortcut("⌘,")
                .with_separator(),
            AppMenuItem::new(format!("Hide {app_name}"), MenuAction::Hide).with_shortcut("⌘H"),
            AppMenuItem::new("Lock Session", MenuAction::LockSession).with_shortcut("⌘⇧L"),
            AppMenuItem::new("Logout…", MenuAction::Logout).with_separator(),
            AppMenuItem::new("Restart", MenuAction::Restart),
            AppMenuItem::new("Shut Down…", MenuAction::Shutdown).with_separator(),
            AppMenuItem::new(format!("Quit {app_name}"), MenuAction::Quit).with_shortcut("⌘Q"),
        ]
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
        self.hover_index = None;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.hover_index = None;
    }

    /// Paint the app name button and the dropdown menu if open.
    pub fn paint(
        &self,
        painter: &mut liquide_ui_core::Painter,
        theme: &liquide_ui_core::UiTheme,
        x: f32,
        bar_y: f32,
        bar_h: f32,
    ) -> f32 {
        let colors = &theme.colors;
        let font_size = theme.font_size;
        let char_w = font_size * 0.6;
        let name_w = self.app_name.len() as f32 * char_w + 12.0;
        let text_y = bar_y + (bar_h - font_size) / 2.0;

        // App name (bold)
        painter.draw_text(
            &self.app_name,
            x + 6.0,
            text_y,
            font_size,
            colors.text_primary,
            &theme.font_family,
            true,
        );

        // Dropdown menu
        if self.open {
            let menu_x = x;
            let menu_y = bar_y + bar_h + 2.0;
            let item_h = font_size + 12.0;
            let menu_w = 220.0;
            let menu_h = self.items.len() as f32 * item_h + 8.0;
            let radius = theme.radius_md;

            // Shadow
            painter.fill_rounded_rect(
                menu_x + 1.0,
                menu_y + 2.0,
                menu_w,
                menu_h,
                radius,
                liquide_ui_core::UiColor::new(0, 0, 0, 50),
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

            // Items
            for (i, item) in self.items.iter().enumerate() {
                let iy = menu_y + 4.0 + i as f32 * item_h;
                let is_hover = self.hover_index == Some(i);

                if is_hover && item.enabled {
                    painter.fill_rounded_rect(
                        menu_x + 4.0,
                        iy,
                        menu_w - 8.0,
                        item_h,
                        radius * 0.5,
                        colors.accent,
                    );
                }

                let tc = if !item.enabled {
                    colors.text_disabled
                } else if is_hover {
                    colors.text_on_accent
                } else {
                    colors.text_primary
                };

                painter.draw_text(
                    &item.label,
                    menu_x + 16.0,
                    iy + (item_h - font_size) / 2.0,
                    font_size,
                    tc,
                    &theme.font_family,
                    false,
                );

                // Shortcut on the right
                if let Some(shortcut) = &item.shortcut {
                    let shortcut_w = shortcut.len() as f32 * font_size * 0.5;
                    painter.draw_text(
                        shortcut,
                        menu_x + menu_w - shortcut_w - 16.0,
                        iy + (item_h - font_size) / 2.0,
                        font_size * 0.9,
                        colors.text_secondary,
                        &theme.font_family,
                        false,
                    );
                }

                // Separator
                if item.separator_after {
                    let sep_y = iy + item_h - 1.0;
                    painter.draw_line(
                        menu_x + 12.0,
                        sep_y,
                        menu_x + menu_w - 12.0,
                        sep_y,
                        colors.border_subtle,
                        1.0,
                    );
                }
            }
        }

        name_w
    }
}
