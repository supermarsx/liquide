//! Top-level menu bar — File, Edit, View, Window, Help menus.
//!
//! In macOS style, these sit right after the bold app name in the status bar.

use serde::{Deserialize, Serialize};

/// A single item in a sub-menu.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubMenuItem {
    pub label: String,
    pub action_id: String,
    pub shortcut: Option<String>,
    pub enabled: bool,
    pub checked: bool,
    pub separator_after: bool,
}

impl SubMenuItem {
    pub fn new(label: impl Into<String>, action_id: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            action_id: action_id.into(),
            shortcut: None,
            enabled: true,
            checked: false,
            separator_after: false,
        }
    }

    pub fn with_shortcut(mut self, s: impl Into<String>) -> Self {
        self.shortcut = Some(s.into()); self
    }

    pub fn with_separator(mut self) -> Self {
        self.separator_after = true; self
    }

    pub fn checked(mut self, c: bool) -> Self {
        self.checked = c; self
    }
}

/// A single drop-down menu (e.g. "File", "Edit").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubMenu {
    pub label: String,
    pub items: Vec<SubMenuItem>,
}

impl SubMenu {
    pub fn new(label: impl Into<String>, items: Vec<SubMenuItem>) -> Self {
        Self { label: label.into(), items }
    }
}

/// A single top-level menu bar entry.
#[derive(Debug, Clone)]
pub struct MenuBarItem {
    pub menu: SubMenu,
    pub open: bool,
    pub hover_index: Option<usize>,
}

/// The full menu bar (File, Edit, View, …).
pub struct MenuBar {
    pub items: Vec<MenuBarItem>,
    pub active_index: Option<usize>,
}

impl MenuBar {
    pub fn new(menus: Vec<SubMenu>) -> Self {
        Self {
            items: menus.into_iter().map(|m| MenuBarItem { menu: m, open: false, hover_index: None }).collect(),
            active_index: None,
        }
    }

    /// Build the default desktop menu bar.
    pub fn default_desktop() -> Self {
        Self::new(vec![
            SubMenu::new("File", vec![
                SubMenuItem::new("New Window", "file.new_window").with_shortcut("⌘N"),
                SubMenuItem::new("Open…", "file.open").with_shortcut("⌘O").with_separator(),
                SubMenuItem::new("Close Window", "file.close").with_shortcut("⌘W"),
            ]),
            SubMenu::new("Edit", vec![
                SubMenuItem::new("Undo", "edit.undo").with_shortcut("⌘Z"),
                SubMenuItem::new("Redo", "edit.redo").with_shortcut("⇧⌘Z").with_separator(),
                SubMenuItem::new("Cut", "edit.cut").with_shortcut("⌘X"),
                SubMenuItem::new("Copy", "edit.copy").with_shortcut("⌘C"),
                SubMenuItem::new("Paste", "edit.paste").with_shortcut("⌘V"),
                SubMenuItem::new("Select All", "edit.select_all").with_shortcut("⌘A"),
            ]),
            SubMenu::new("View", vec![
                SubMenuItem::new("Toggle Fullscreen", "view.fullscreen").with_shortcut("⌃⌘F"),
                SubMenuItem::new("Zoom In", "view.zoom_in").with_shortcut("⌘+"),
                SubMenuItem::new("Zoom Out", "view.zoom_out").with_shortcut("⌘-"),
                SubMenuItem::new("Actual Size", "view.zoom_reset").with_shortcut("⌘0").with_separator(),
                SubMenuItem::new("Show Dock", "view.dock").checked(true),
                SubMenuItem::new("Show Status Bar", "view.statusbar").checked(true),
            ]),
            SubMenu::new("Window", vec![
                SubMenuItem::new("Minimize", "window.minimize").with_shortcut("⌘M"),
                SubMenuItem::new("Zoom", "window.zoom"),
                SubMenuItem::new("Tile Left", "window.tile_left"),
                SubMenuItem::new("Tile Right", "window.tile_right").with_separator(),
                SubMenuItem::new("Bring All to Front", "window.bring_all"),
            ]),
            SubMenu::new("Help", vec![
                SubMenuItem::new("Liquide Help", "help.main").with_shortcut("⌘?"),
                SubMenuItem::new("Keyboard Shortcuts", "help.shortcuts").with_separator(),
                SubMenuItem::new("Report a Bug…", "help.report_bug"),
                SubMenuItem::new("About Liquide", "help.about"),
            ]),
        ])
    }

    /// Toggle a menu open/closed.
    pub fn toggle_menu(&mut self, index: usize) {
        if self.active_index == Some(index) {
            self.close_all();
        } else {
            self.close_all();
            if let Some(item) = self.items.get_mut(index) {
                item.open = true;
                self.active_index = Some(index);
            }
        }
    }

    /// Close all menus.
    pub fn close_all(&mut self) {
        for item in &mut self.items {
            item.open = false;
            item.hover_index = None;
        }
        self.active_index = None;
    }

    /// Paint the menu bar items starting at `x`.
    /// Returns the total width consumed.
    pub fn paint(
        &self,
        painter: &mut liquide_ui_core::Painter,
        theme: &liquide_ui_core::UiTheme,
        start_x: f32,
        bar_y: f32,
        bar_h: f32,
    ) -> f32 {
        let colors = &theme.colors;
        let font_size = theme.font_size;
        let char_w = font_size * 0.55;
        let padding_h = 10.0;
        let text_y = bar_y + (bar_h - font_size) / 2.0;
        let mut x = start_x;

        for (i, item) in self.items.iter().enumerate() {
            let label = &item.menu.label;
            let label_w = label.len() as f32 * char_w;
            let item_w = label_w + padding_h * 2.0;

            // Highlight if open
            if item.open {
                painter.fill_rounded_rect(
                    x, bar_y + 2.0, item_w, bar_h - 4.0,
                    theme.radius_sm, colors.surface_active,
                );
            }

            // Label
            let tc = if item.open { colors.text_primary } else { colors.text_secondary };
            painter.draw_text(label, x + padding_h, text_y, font_size, tc, &theme.font_family, false);

            // Dropdown
            if item.open {
                let menu_x = x;
                let menu_y = bar_y + bar_h + 2.0;
                let item_h = font_size + 12.0;
                let menu_w = 240.0;
                let menu_h = item.menu.items.len() as f32 * item_h + 8.0;
                let radius = theme.radius_md;

                // Shadow
                painter.fill_rounded_rect(
                    menu_x + 1.0, menu_y + 2.0, menu_w, menu_h, radius,
                    liquide_ui_core::UiColor::new(0, 0, 0, 50),
                );
                // Background
                painter.fill_rounded_rect(menu_x, menu_y, menu_w, menu_h, radius, colors.surface_elevated);
                painter.stroke_rounded_rect(menu_x, menu_y, menu_w, menu_h, radius, colors.border, 1.0);

                for (j, sub_item) in item.menu.items.iter().enumerate() {
                    let iy = menu_y + 4.0 + j as f32 * item_h;
                    let is_hover = item.hover_index == Some(j);

                    if is_hover && sub_item.enabled {
                        painter.fill_rounded_rect(
                            menu_x + 4.0, iy, menu_w - 8.0, item_h,
                            radius * 0.5, colors.accent,
                        );
                    }

                    // Checkmark
                    if sub_item.checked {
                        painter.draw_text(
                            "✓", menu_x + 8.0, iy + (item_h - font_size) / 2.0,
                            font_size, colors.accent, &theme.font_family, false,
                        );
                    }

                    let stc = if !sub_item.enabled {
                        colors.text_disabled
                    } else if is_hover {
                        colors.text_on_accent
                    } else {
                        colors.text_primary
                    };

                    painter.draw_text(
                        &sub_item.label, menu_x + 24.0, iy + (item_h - font_size) / 2.0,
                        font_size, stc, &theme.font_family, false,
                    );

                    if let Some(shortcut) = &sub_item.shortcut {
                        let sw = shortcut.len() as f32 * font_size * 0.5;
                        painter.draw_text(
                            shortcut, menu_x + menu_w - sw - 16.0, iy + (item_h - font_size) / 2.0,
                            font_size * 0.9, colors.text_secondary, &theme.font_family, false,
                        );
                    }

                    if sub_item.separator_after {
                        let sep_y = iy + item_h - 1.0;
                        painter.draw_line(
                            menu_x + 12.0, sep_y, menu_x + menu_w - 12.0, sep_y,
                            colors.border_subtle, 1.0,
                        );
                    }
                }
            }

            x += item_w;
        }

        x - start_x
    }
}
