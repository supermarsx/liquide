//! Reusable context menu system for the Liquide desktop shell.
//!
//! Provides a generic [`ContextMenu`] that can be used for desktop
//! right-click menus, window title-bar menus, dock item menus, status bar
//! menus, and any other popup menu surface.
//!
//! # Modules
//!
//! - [`menu`] — Core types: `MenuItem`, `ContextMenu`, `MenuAction`,
//!   `MenuItemKind`, and the builder pattern.
//! - [`layout`] — Geometry computation: `MenuLayout`, `MenuGeometry`,
//!   `MenuItemRect`. Handles screen-edge avoidance and submenu cascading.
//! - [`state`] — Interactive state: `MenuState`, `MenuKey`, `MenuResponse`.
//!   Keyboard navigation, hover tracking, submenu delay.
//! - [`theme`] — Visual configuration: `MenuTheme` with light/dark presets.
//! - [`presets`] — Built-in context menus for desktop, file manager, text
//!   editing, and window titlebar.
//! - [`dom`] — DOM-based rendering helpers for the CSS pipeline.

pub mod dom;
pub mod layout;
mod menu;
pub mod presets;
pub mod state;
pub mod theme;

#[cfg(test)]
mod tests;

pub use menu::{
    ContextMenu, ContextMenuBuilder, ContextMenuConfig, MenuAction, MenuItem, MenuItemId,
    MenuItemKind, MenuSeparator, reset_item_id_counter,
};

pub use layout::{MenuGeometry, MenuItemRect, MenuLayout};
pub use state::{MenuKey, MenuResponse, MenuState};
pub use theme::MenuTheme;
