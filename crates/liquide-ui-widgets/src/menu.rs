//! Menu system: menu bar, popup menus, context menus.
//!
//! Supports:
//! - Menu bars with top-level items
//! - Cascading sub-menus
//! - Keyboard shortcuts with display
//! - Checkable items and radio groups
//! - Separators
//! - Icons

use liquide_ui_core::WidgetId;
use serde::{Deserialize, Serialize};

/// Unique menu item identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MenuItemId(pub u64);

/// A keyboard shortcut for a menu item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shortcut {
    /// Modifier keys (Ctrl, Alt, Shift, Super).
    pub modifiers: Vec<Modifier>,
    /// The key name (e.g., "S", "F5", "Delete").
    pub key: String,
}

impl Shortcut {
    #[must_use]
    pub fn new(modifiers: &[Modifier], key: impl Into<String>) -> Self {
        Self {
            modifiers: modifiers.to_vec(),
            key: key.into(),
        }
    }

    /// Display string (e.g., "Ctrl+S").
    #[must_use]
    pub fn display(&self) -> String {
        let mut parts: Vec<&str> = self
            .modifiers
            .iter()
            .map(|m| match m {
                Modifier::Ctrl => "Ctrl",
                Modifier::Alt => "Alt",
                Modifier::Shift => "Shift",
                Modifier::Super => "Super",
            })
            .collect();
        parts.push(&self.key);
        parts.join("+")
    }
}

/// Keyboard modifier keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Modifier {
    Ctrl,
    Alt,
    Shift,
    Super,
}

/// The type of a menu item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MenuItemKind {
    /// A normal action item.
    Action,
    /// A checkable item (toggle).
    Checkable { checked: bool },
    /// A radio item (part of a group).
    Radio { group: String, selected: bool },
    /// A separator line.
    Separator,
    /// A sub-menu.
    SubMenu { items: Vec<MenuItem> },
}

/// A single menu item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MenuItem {
    /// Unique identifier.
    pub id: MenuItemId,
    /// Display text (with optional mnemonic, e.g., "&File").
    pub text: String,
    /// The kind of item.
    pub kind: MenuItemKind,
    /// Optional keyboard shortcut.
    pub shortcut: Option<Shortcut>,
    /// Optional icon identifier.
    pub icon: Option<String>,
    /// Whether this item is enabled.
    pub enabled: bool,
    /// Whether this item is visible.
    pub visible: bool,
}

impl MenuItem {
    /// Create a normal action item.
    #[must_use]
    pub fn action(id: MenuItemId, text: impl Into<String>) -> Self {
        Self {
            id,
            text: text.into(),
            kind: MenuItemKind::Action,
            shortcut: None,
            icon: None,
            enabled: true,
            visible: true,
        }
    }

    /// Create a separator.
    #[must_use]
    pub fn separator() -> Self {
        Self {
            id: MenuItemId(0),
            text: String::new(),
            kind: MenuItemKind::Separator,
            shortcut: None,
            icon: None,
            enabled: false,
            visible: true,
        }
    }

    /// Create a sub-menu.
    #[must_use]
    pub fn submenu(id: MenuItemId, text: impl Into<String>, items: Vec<MenuItem>) -> Self {
        Self {
            id,
            text: text.into(),
            kind: MenuItemKind::SubMenu { items },
            shortcut: None,
            icon: None,
            enabled: true,
            visible: true,
        }
    }

    /// Create a checkable item.
    #[must_use]
    pub fn checkable(id: MenuItemId, text: impl Into<String>, checked: bool) -> Self {
        Self {
            id,
            text: text.into(),
            kind: MenuItemKind::Checkable { checked },
            shortcut: None,
            icon: None,
            enabled: true,
            visible: true,
        }
    }

    #[must_use]
    pub fn with_shortcut(mut self, shortcut: Shortcut) -> Self {
        self.shortcut = Some(shortcut);
        self
    }

    #[must_use]
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    #[must_use]
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Extract the mnemonic character (the one after '&').
    #[must_use]
    pub fn mnemonic(&self) -> Option<char> {
        let mut chars = self.text.chars();
        while let Some(ch) = chars.next() {
            if ch == '&' {
                return chars.next();
            }
        }
        None
    }

    /// Display text without mnemonic markers.
    #[must_use]
    pub fn display_text(&self) -> String {
        self.text.replace('&', "")
    }
}

/// A menu bar containing top-level menus.
#[derive(Debug, Clone)]
pub struct MenuBar {
    pub id: WidgetId,
    /// Top-level menu items (each typically a SubMenu).
    pub items: Vec<MenuItem>,
    /// Currently focused top-level item index.
    pub active_index: Option<usize>,
    /// Whether the menu bar is currently active (keyboard mode).
    pub is_active: bool,
}

impl MenuBar {
    #[must_use]
    pub fn new(id: WidgetId, items: Vec<MenuItem>) -> Self {
        Self {
            id,
            items,
            active_index: None,
            is_active: false,
        }
    }

    /// Activate the menu bar (e.g., pressing Alt).
    pub fn activate(&mut self) {
        self.is_active = true;
        if self.active_index.is_none() && !self.items.is_empty() {
            self.active_index = Some(0);
        }
    }

    /// Deactivate the menu bar.
    pub fn deactivate(&mut self) {
        self.is_active = false;
        self.active_index = None;
    }

    /// Navigate to the next top-level item.
    pub fn next_item(&mut self) {
        if self.items.is_empty() {
            return;
        }
        let next = self.active_index.map_or(0, |i| (i + 1) % self.items.len());
        self.active_index = Some(next);
    }

    /// Navigate to the previous top-level item.
    pub fn prev_item(&mut self) {
        if self.items.is_empty() {
            return;
        }
        let prev = self.active_index.map_or(self.items.len() - 1, |i| {
            if i == 0 { self.items.len() - 1 } else { i - 1 }
        });
        self.active_index = Some(prev);
    }

    /// Find a menu item by mnemonic character.
    #[must_use]
    pub fn find_by_mnemonic(&self, ch: char) -> Option<usize> {
        let lower = ch.to_ascii_lowercase();
        self.items.iter().position(|item| {
            item.mnemonic()
                .map(|m| m.to_ascii_lowercase() == lower)
                .unwrap_or(false)
        })
    }
}

/// A context (popup) menu.
#[derive(Debug, Clone)]
pub struct ContextMenu {
    pub id: WidgetId,
    /// Menu items.
    pub items: Vec<MenuItem>,
    /// Position where the menu was opened.
    pub position: (f32, f32),
    /// Currently focused item index.
    pub focused_index: Option<usize>,
    /// Whether this menu is visible.
    pub visible: bool,
}

impl ContextMenu {
    #[must_use]
    pub fn new(id: WidgetId, items: Vec<MenuItem>) -> Self {
        Self {
            id,
            items,
            position: (0.0, 0.0),
            focused_index: None,
            visible: false,
        }
    }

    /// Show the context menu at the given position.
    pub fn show_at(&mut self, x: f32, y: f32) {
        self.position = (x, y);
        self.visible = true;
        self.focused_index = None;
    }

    /// Hide the context menu.
    pub fn hide(&mut self) {
        self.visible = false;
        self.focused_index = None;
    }

    /// Navigate to the next item (skipping separators).
    pub fn focus_next(&mut self) {
        let len = self.items.len();
        if len == 0 {
            return;
        }
        let start = self.focused_index.map_or(0, |i| i + 1);
        for offset in 0..len {
            let idx = (start + offset) % len;
            if !matches!(self.items[idx].kind, MenuItemKind::Separator) && self.items[idx].enabled {
                self.focused_index = Some(idx);
                return;
            }
        }
    }

    /// Navigate to the previous item (skipping separators).
    pub fn focus_prev(&mut self) {
        let len = self.items.len();
        if len == 0 {
            return;
        }
        let start = self.focused_index.unwrap_or(0);
        for offset in 1..=len {
            let idx = (start + len - offset) % len;
            if !matches!(self.items[idx].kind, MenuItemKind::Separator) && self.items[idx].enabled {
                self.focused_index = Some(idx);
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_menu() -> Vec<MenuItem> {
        vec![
            MenuItem::submenu(
                MenuItemId(1),
                "&File",
                vec![
                    MenuItem::action(MenuItemId(10), "&New")
                        .with_shortcut(Shortcut::new(&[Modifier::Ctrl], "N")),
                    MenuItem::action(MenuItemId(11), "&Open")
                        .with_shortcut(Shortcut::new(&[Modifier::Ctrl], "O")),
                    MenuItem::separator(),
                    MenuItem::action(MenuItemId(12), "&Save")
                        .with_shortcut(Shortcut::new(&[Modifier::Ctrl], "S")),
                    MenuItem::separator(),
                    MenuItem::action(MenuItemId(13), "E&xit"),
                ],
            ),
            MenuItem::submenu(
                MenuItemId(2),
                "&Edit",
                vec![
                    MenuItem::action(MenuItemId(20), "&Undo")
                        .with_shortcut(Shortcut::new(&[Modifier::Ctrl], "Z")),
                    MenuItem::action(MenuItemId(21), "&Redo")
                        .with_shortcut(Shortcut::new(&[Modifier::Ctrl, Modifier::Shift], "Z")),
                ],
            ),
        ]
    }

    #[test]
    fn test_menu_bar() {
        let bar = MenuBar::new(WidgetId::from_raw(1), sample_menu());
        assert_eq!(bar.items.len(), 2);
    }

    #[test]
    fn test_mnemonic() {
        let item = MenuItem::action(MenuItemId(1), "&File");
        assert_eq!(item.mnemonic(), Some('F'));
        assert_eq!(item.display_text(), "File");
    }

    #[test]
    fn test_shortcut_display() {
        let s = Shortcut::new(&[Modifier::Ctrl, Modifier::Shift], "S");
        assert_eq!(s.display(), "Ctrl+Shift+S");
    }

    #[test]
    fn test_find_mnemonic() {
        let bar = MenuBar::new(WidgetId::from_raw(1), sample_menu());
        assert_eq!(bar.find_by_mnemonic('f'), Some(0));
        assert_eq!(bar.find_by_mnemonic('e'), Some(1));
        assert_eq!(bar.find_by_mnemonic('z'), None);
    }

    #[test]
    fn test_menu_navigation() {
        let mut bar = MenuBar::new(WidgetId::from_raw(1), sample_menu());
        bar.activate();
        assert_eq!(bar.active_index, Some(0));
        bar.next_item();
        assert_eq!(bar.active_index, Some(1));
        bar.next_item();
        assert_eq!(bar.active_index, Some(0)); // wraps around
    }

    #[test]
    fn test_context_menu() {
        let mut ctx = ContextMenu::new(
            WidgetId::from_raw(1),
            vec![
                MenuItem::action(MenuItemId(1), "Cut"),
                MenuItem::separator(),
                MenuItem::action(MenuItemId(2), "Copy"),
                MenuItem::action(MenuItemId(3), "Paste"),
            ],
        );
        ctx.show_at(100.0, 200.0);
        assert!(ctx.visible);

        ctx.focus_next();
        assert_eq!(ctx.focused_index, Some(0)); // "Cut" (skips nothing)

        ctx.focus_next();
        assert_eq!(ctx.focused_index, Some(2)); // "Copy" (skips separator)
    }

    #[test]
    fn test_checkable_item() {
        let item = MenuItem::checkable(MenuItemId(1), "Show Grid", true);
        assert!(matches!(
            item.kind,
            MenuItemKind::Checkable { checked: true }
        ));
    }
}
