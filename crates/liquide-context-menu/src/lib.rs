//! Reusable context menu system for the Liquide desktop shell.
//!
//! Provides a generic [`ContextMenu<A>`] that can be used for desktop
//! right-click menus, window title-bar menus, dock item menus, status bar
//! menus, and any other popup menu surface.

mod menu;pub mod presets;
pub use menu::{
    ContextMenu, ContextMenuConfig, MenuAction, MenuItem, MenuItemKind, MenuSeparator,
};
