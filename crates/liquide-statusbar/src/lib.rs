//! macOS-style status bar with app menu, system indicators, and widgets.
//!
//! This crate provides a standalone status bar that can be used independently
//! of the shell's built-in status bar for more advanced UI integration. It
//! implements:
//!
//! - **App name + menu bar** (File, Edit, View, …) on the left — macOS style
//! - **System indicators** (clock, notifications, battery, WiFi) on the right
//! - **Theme/dark-mode toggle** as a status bar dropdown widget
//! - **Plugin extension slots** for custom widgets

pub mod app_menu;
pub mod config;
pub mod dom;
pub mod indicator;
pub mod menu_bar;
pub mod status_bar;
pub mod theme_toggle;

pub use app_menu::{AppMenu, AppMenuItem, MenuAction};
pub use config::StatusBarConfig;
pub use indicator::{SystemIndicator, IndicatorKind};
pub use menu_bar::{MenuBar, MenuBarItem, SubMenu, SubMenuItem};
pub use status_bar::StatusBar;
pub use theme_toggle::ThemeToggle;
