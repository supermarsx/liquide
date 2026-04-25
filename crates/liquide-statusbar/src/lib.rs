//! macOS-style status bar with app menu, system indicators, and widgets.
//!
//! This crate provides two status bar implementations:
//!
//! ## Painter-based (`StatusBar`)
//!
//! A standalone status bar rendered via `Painter` for more advanced UI
//! integration. It implements:
//!
//! - **App name + menu bar** (File, Edit, View, …) on the left — macOS style
//! - **System indicators** (clock, notifications, battery, WiFi) on the right
//! - **Theme/dark-mode toggle** as a status bar dropdown widget
//! - **Plugin extension slots** for custom widgets
//!
//! ## Scene-graph-based (`ShellStatusBar`)
//!
//! The shell's runtime status bar that produces `SceneNode` output for the
//! compositor. Manages clock, notifications, connection quality, system tray,
//! and session button items across left/center/right slots.

pub mod app_menu;
pub mod config;
pub mod dom;
pub mod indicator;
pub mod items;
pub mod menu_bar;
pub mod scene;
pub mod shell_bar;
pub mod slot;
pub mod status_bar;
pub mod theme_toggle;

// Painter-based status bar
pub use app_menu::{AppMenu, AppMenuItem, MenuAction};
pub use config::StatusBarConfig;
pub use indicator::{IndicatorKind, SystemIndicator};
pub use menu_bar::{MenuBar, MenuBarItem, SubMenu, SubMenuItem};
pub use status_bar::StatusBar;
pub use theme_toggle::ThemeToggle;

// Scene-graph-based shell status bar
pub use items::{StatusBarItem, StatusBarItemKind};
pub use scene::{
    NODE_STATUS_BAR, NODE_STATUS_BAR_ITEM_BASE, StatusBarColors, StatusBarFonts, StatusBarLayout,
};
pub use shell_bar::{ShellBarConfig, ShellStatusBar};
pub use slot::StatusBarSlot;

#[cfg(test)]
mod tests;
