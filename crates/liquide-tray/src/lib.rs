//! System tray / status notifier items for the LiquiDE desktop.
//!
//! This crate implements the data model and layout engine for the system tray
//! (notification area / indicator area) based on the freedesktop.org
//! StatusNotifierItem specification.
//!
//! # Architecture
//!
//! - [`StatusNotifierItem`] — a single status notifier item with identity,
//!   category, status, icon set (primary/overlay/attention), tooltip, and menu.
//! - [`TrayHost`] — the visual tray manager that registers items, enforces
//!   ordering (by category then registration time), and emits events.
//! - [`TrayWatcher`] — central registry tracking which hosts and items are
//!   alive (StatusNotifierWatcher role).
//! - [`TrayMenu`] / [`TrayMenuItem`] — DBusMenu-style tree menu for items.
//! - [`TrayLayout`] / [`compute_tray_bounds`] — spatial layout and hit-testing.

pub mod host;
pub mod item;
pub mod menu;
pub mod renderer;
pub mod watcher;

pub use host::{TrayEvent, TrayHost};
pub use item::{
    ItemCategory, ItemId, ItemStatus, Pixmap, StatusNotifierItem, StatusNotifierItemBuilder,
    ToolTip,
};
pub use menu::{
    build_menu_tree, FlatMenuItem, MenuItemId, MenuItemType, TrayMenu, TrayMenuItem,
    ROOT_MENU_ID,
};
pub use renderer::{
    compute_tray_bounds, item_at_point, ItemRect, TrayBounds, TrayLayout, TrayOrientation,
};
pub use watcher::{StatusNotifierWatcherSignal, TrayWatcher};

#[cfg(test)]
mod tests;
