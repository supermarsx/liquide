//! Tray item menu — DBusMenu-style tree structure for status notifier items.
//!
//! Menus are represented as a tree of [`TrayMenuItem`] nodes. The
//! [`build_menu_tree`] function converts a flat list (with parent IDs) into the
//! tree form, mirroring how the com.canonical.dbusmenu interface transmits
//! menu layouts.

use serde::{Deserialize, Serialize};

/// Unique identifier for a menu item within a menu tree.
pub type MenuItemId = u32;

/// The sentinel ID used for the root of the menu tree.
pub const ROOT_MENU_ID: MenuItemId = 0;

/// Type of a menu item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MenuItemType {
    /// A normal clickable menu item.
    Standard,
    /// A horizontal separator line.
    Separator,
    /// A toggleable checkbox.
    Checkbox(bool),
    /// A mutually exclusive radio button.
    Radio(bool),
}

impl MenuItemType {
    /// Returns `true` if this is a separator.
    pub fn is_separator(self) -> bool {
        self == Self::Separator
    }

    /// Returns `true` if this item is interactive (not a separator).
    pub fn is_interactive(self) -> bool {
        !self.is_separator()
    }

    /// Returns `true` if this is a checkbox or radio item that is checked.
    pub fn is_checked(self) -> bool {
        matches!(self, Self::Checkbox(true) | Self::Radio(true))
    }

    /// Toggle a checkbox or radio state. Returns the new type.
    pub fn toggled(self) -> Self {
        match self {
            Self::Checkbox(v) => Self::Checkbox(!v),
            Self::Radio(v) => Self::Radio(!v),
            other => other,
        }
    }
}

/// A single item in a tray context menu tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrayMenuItem {
    /// Unique identifier within the menu.
    pub id: MenuItemId,
    /// Display label (may contain underscores for mnemonics).
    pub label: String,
    /// Icon name from the icon theme (empty if none).
    pub icon: String,
    /// Whether the item can be activated.
    pub enabled: bool,
    /// Whether the item is visible.
    pub visible: bool,
    /// The type/toggle state of the item.
    pub type_: MenuItemType,
    /// Child items (submenus).
    pub children: Vec<TrayMenuItem>,
}

impl TrayMenuItem {
    /// Create a standard menu item.
    pub fn new(id: MenuItemId, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            icon: String::new(),
            enabled: true,
            visible: true,
            type_: MenuItemType::Standard,
            children: Vec::new(),
        }
    }

    /// Create a separator item.
    pub fn separator(id: MenuItemId) -> Self {
        Self {
            id,
            label: String::new(),
            icon: String::new(),
            enabled: false,
            visible: true,
            type_: MenuItemType::Separator,
            children: Vec::new(),
        }
    }

    /// Create a checkbox item.
    pub fn checkbox(id: MenuItemId, label: impl Into<String>, checked: bool) -> Self {
        Self {
            id,
            label: label.into(),
            icon: String::new(),
            enabled: true,
            visible: true,
            type_: MenuItemType::Checkbox(checked),
            children: Vec::new(),
        }
    }

    /// Create a radio item.
    pub fn radio(id: MenuItemId, label: impl Into<String>, selected: bool) -> Self {
        Self {
            id,
            label: label.into(),
            icon: String::new(),
            enabled: true,
            visible: true,
            type_: MenuItemType::Radio(selected),
            children: Vec::new(),
        }
    }

    /// Builder: set the icon name.
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = icon.into();
        self
    }

    /// Builder: set the enabled state.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Builder: set visibility.
    pub fn with_visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    /// Builder: set child items.
    pub fn with_children(mut self, children: Vec<TrayMenuItem>) -> Self {
        self.children = children;
        self
    }

    /// Returns `true` if this item has children (is a submenu).
    pub fn has_children(&self) -> bool {
        !self.children.is_empty()
    }

    /// Count visible items in this subtree (including self if visible).
    pub fn visible_count(&self) -> usize {
        if !self.visible {
            return 0;
        }
        1 + self
            .children
            .iter()
            .map(|c| c.visible_count())
            .sum::<usize>()
    }
}

/// A flat menu item with a parent ID, used as input to [`build_menu_tree`].
#[derive(Debug, Clone)]
pub struct FlatMenuItem {
    /// The item's own ID.
    pub id: MenuItemId,
    /// The parent item's ID (use [`ROOT_MENU_ID`] for top-level items).
    pub parent_id: MenuItemId,
    /// Display label.
    pub label: String,
    /// Icon name.
    pub icon: String,
    /// Whether the item is enabled.
    pub enabled: bool,
    /// Whether the item is visible.
    pub visible: bool,
    /// The item type.
    pub type_: MenuItemType,
}

/// The root of a tray menu tree.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrayMenu {
    /// Top-level menu items.
    pub items: Vec<TrayMenuItem>,
}

impl TrayMenu {
    /// Create an empty menu.
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Builder: add a top-level item.
    pub fn add_item(mut self, item: TrayMenuItem) -> Self {
        self.items.push(item);
        self
    }

    /// Builder: add a separator.
    pub fn add_separator(self, id: MenuItemId) -> Self {
        self.add_item(TrayMenuItem::separator(id))
    }

    /// Returns `true` if the menu has no items.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Number of top-level items.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Total visible items in the tree (recursive).
    pub fn total_visible(&self) -> usize {
        self.items.iter().map(|i| i.visible_count()).sum()
    }

    /// Find a menu item by ID (depth-first search).
    pub fn find_item(&self, id: MenuItemId) -> Option<&TrayMenuItem> {
        fn search(items: &[TrayMenuItem], target: MenuItemId) -> Option<&TrayMenuItem> {
            for item in items {
                if item.id == target {
                    return Some(item);
                }
                if let Some(found) = search(&item.children, target) {
                    return Some(found);
                }
            }
            None
        }
        search(&self.items, id)
    }

    /// Find a menu item by ID mutably (depth-first search).
    pub fn find_item_mut(&mut self, id: MenuItemId) -> Option<&mut TrayMenuItem> {
        fn search(items: &mut [TrayMenuItem], target: MenuItemId) -> Option<&mut TrayMenuItem> {
            for item in items {
                if item.id == target {
                    return Some(item);
                }
                if let Some(found) = search(&mut item.children, target) {
                    return Some(found);
                }
            }
            None
        }
        search(&mut self.items, id)
    }

    /// Activate a menu item by ID. For checkbox/radio items, this toggles
    /// the check state. Returns `true` if the item was found and is enabled.
    pub fn activate_item(&mut self, id: MenuItemId) -> bool {
        if let Some(item) = self.find_item_mut(id) {
            if !item.enabled {
                return false;
            }
            item.type_ = item.type_.toggled();
            true
        } else {
            false
        }
    }
}

impl std::fmt::Display for TrayMenu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "TrayMenu({} top-level, {} visible)",
            self.len(),
            self.total_visible()
        )
    }
}

/// Build a menu tree from a flat list of items with parent IDs.
///
/// Items whose `parent_id` is [`ROOT_MENU_ID`] become top-level entries.
/// All other items are nested under their parent. Items whose parent is not
/// found in the list are silently discarded.
///
/// Children are ordered by their position in the input slice.
pub fn build_menu_tree(flat_items: &[FlatMenuItem]) -> TrayMenu {
    use std::collections::HashMap;

    // Group children by parent_id.
    let mut children_map: HashMap<MenuItemId, Vec<&FlatMenuItem>> = HashMap::new();
    for item in flat_items {
        children_map.entry(item.parent_id).or_default().push(item);
    }

    fn build_children(
        parent_id: MenuItemId,
        children_map: &HashMap<MenuItemId, Vec<&FlatMenuItem>>,
    ) -> Vec<TrayMenuItem> {
        let Some(children) = children_map.get(&parent_id) else {
            return Vec::new();
        };
        children
            .iter()
            .map(|flat| {
                let grandchildren = build_children(flat.id, children_map);
                TrayMenuItem {
                    id: flat.id,
                    label: flat.label.clone(),
                    icon: flat.icon.clone(),
                    enabled: flat.enabled,
                    visible: flat.visible,
                    type_: flat.type_,
                    children: grandchildren,
                }
            })
            .collect()
    }

    let top_level = build_children(ROOT_MENU_ID, &children_map);
    TrayMenu { items: top_level }
}
