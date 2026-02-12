//! Plugin architecture types (spec section 23).
//!
//! Provides the trait and supporting data structures that third-party plugins
//! use to extend the task manager with custom tabs, columns, context menu
//! items, and per-tick logic.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// PluginInfo
// ---------------------------------------------------------------------------

/// Metadata describing a task manager plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    /// Unique plugin name (used as an identifier).
    pub name: String,
    /// Semantic version string (e.g. "1.2.0").
    pub version: String,
    /// Plugin author or organisation.
    pub author: String,
    /// Short human-readable description of the plugin.
    pub description: String,
    /// The plugin API version this plugin was compiled against.
    pub api_version: u32,
}

// ---------------------------------------------------------------------------
// TabDefinition
// ---------------------------------------------------------------------------

/// Declares a custom tab that a plugin contributes to the task manager.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabDefinition {
    /// Unique tab identifier (used for routing and config persistence).
    pub id: String,
    /// Human-readable label displayed on the tab header.
    pub label: String,
    /// Optional icon name or path for the tab.
    pub icon: Option<String>,
    /// Ordering hint; lower values appear first.
    pub order: u16,
}

// ---------------------------------------------------------------------------
// ColumnDefinition
// ---------------------------------------------------------------------------

/// Declares a custom column that a plugin contributes to a tab.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnDefinition {
    /// Unique column key (used for sorting and config persistence).
    pub key: String,
    /// Human-readable column header label.
    pub label: String,
    /// Default column width in pixels.
    pub width_px: u16,
    /// Whether this column supports click-to-sort.
    pub sortable: bool,
    /// Whether this column is visible by default.
    pub default_visible: bool,
}

// ---------------------------------------------------------------------------
// MenuItemDefinition
// ---------------------------------------------------------------------------

/// Declares a custom context menu item contributed by a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MenuItemDefinition {
    /// Human-readable label displayed in the menu.
    pub label: String,
    /// Identifier dispatched when the menu item is activated.
    pub action_id: String,
    /// Optional keyboard shortcut hint (e.g. "Ctrl+Shift+X").
    pub shortcut: Option<String>,
    /// Optional icon name or path for the menu item.
    pub icon: Option<String>,
    /// Whether to insert a visual separator line before this item.
    pub separator_before: bool,
}

// ---------------------------------------------------------------------------
// SystemState
// ---------------------------------------------------------------------------

/// A snapshot of top-level system metrics passed to plugins each tick.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemState {
    /// Total number of running processes.
    pub process_count: u32,
    /// Total number of threads across all processes.
    pub thread_count: u32,
    /// Overall CPU utilisation as a percentage (0.0-100.0).
    pub cpu_percent: f64,
    /// Overall memory utilisation as a percentage (0.0-100.0).
    pub memory_percent: f64,
    /// System uptime in seconds since last boot.
    pub uptime_secs: u64,
}

// ---------------------------------------------------------------------------
// TaskManagerPlugin trait
// ---------------------------------------------------------------------------

/// Extension point for third-party task manager plugins (spec section 23.1).
///
/// Implementors register custom tabs, columns, and menu items, and receive
/// periodic ticks with the current system state.
pub trait TaskManagerPlugin {
    /// Return metadata about this plugin.
    fn info(&self) -> PluginInfo;

    /// Called once when the plugin is loaded into the task manager.
    fn on_load(&mut self);

    /// Called once when the plugin is about to be unloaded.
    fn on_unload(&mut self);

    /// Return the set of custom tabs this plugin contributes.
    fn tabs(&self) -> Vec<TabDefinition>;

    /// Return the set of custom columns this plugin contributes to the
    /// given tab (identified by `tab_id`).
    fn columns(&self, tab_id: &str) -> Vec<ColumnDefinition>;

    /// Return the set of context menu items this plugin contributes.
    fn menu_items(&self) -> Vec<MenuItemDefinition>;

    /// Called on every sampling tick with the current system state.
    fn on_tick(&mut self, state: &SystemState);
}
