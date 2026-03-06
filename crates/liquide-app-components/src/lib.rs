//! Shared higher-level UI component models for LiquiDE built-in apps.
//!
//! Provides reusable data types for common app-level patterns:
//!
//! - [`SearchBar`] — search input with query, toggles, and result counts
//! - [`Sidebar`] — collapsible section-based navigation sidebar
//! - [`HeaderBar`] — app header with title and action buttons
//! - [`InfoBar`] — bottom status strip (line:col, word count, etc.)
//! - [`Dialog`] — confirmation, alert, and progress dialog models
//! - [`EmptyState`] — placeholder for empty content areas
//!
//! These are pure data/state types — rendering is handled by the shell's
//! template/component system or individual app renderers.

pub mod dialog;
pub mod empty_state;
pub mod header_bar;
pub mod info_bar;
pub mod search_bar;
pub mod sidebar;

pub use dialog::{Dialog, DialogKind, DialogResponse};
pub use empty_state::EmptyState;
pub use header_bar::{HeaderAction, HeaderBar};
pub use info_bar::{InfoBar, InfoBarItem};
pub use search_bar::SearchBar;
pub use sidebar::{Sidebar, SidebarItem, SidebarSection};
