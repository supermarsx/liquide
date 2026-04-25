//! Concrete UI widgets for the LiquiDE desktop.
//!
//! Provides ready-to-use widgets inspired by Qt QWidgets and GTK4 widgets:
//! - **Button** — push button with label, icon, variants (primary, secondary, ghost).
//! - **Label** — static text display with alignment and ellipsis.
//! - **TextInput** — single-line editable text field.
//! - **TextArea** — multi-line editable text area.
//! - **Checkbox** — boolean toggle with label.
//! - **Dropdown** — combo box / select with a popup list.
//! - **Slider** — continuous or stepped value slider.
//! - **ProgressBar** — determinate/indeterminate progress indicator.
//! - **Separator** — horizontal or vertical divider.
//! - **ScrollView** — scrollable container.
//! - **ListView** — virtualized list for large datasets.
//! - **TreeView** — hierarchical tree with expand/collapse.
//! - **TableView** — sortable, resizable data table.
//! - **Menu** — menu bar, popup/context menus.
//! - **TabView** — tabbed container.
//! - **Toolbar** — tool button strip.
//! - **Splitter** — resizable split panes.
//! - **Spinner** — loading indicator.

pub mod button;
pub mod checkbox;
pub mod drop_indicator;
pub mod dropdown;
pub mod label;
pub mod list_view;
pub mod menu;
pub mod progress;
pub mod scroll_view;
pub mod separator;
pub mod slider;
pub mod spinner;
pub mod splitter;
pub mod tab_view;
pub mod table_view;
pub mod text_area;
pub mod text_input;
pub mod toolbar;
pub mod tree_view;

// Re-exports
pub use button::{Button, ButtonStyle, ButtonVariant};
pub use checkbox::Checkbox;
pub use drop_indicator::{DropIndicator, DropShape, Edge as DropEdge};
pub use dropdown::{Dropdown, DropdownItem};
pub use label::Label;
pub use list_view::{ListItem, ListView, RowHeightMode, SelectionMode};
pub use menu::{ContextMenu, MenuBar, MenuItem, MenuItemId, Shortcut};
pub use progress::ProgressBar;
pub use scroll_view::ScrollView;
pub use separator::Separator;
pub use slider::Slider;
pub use spinner::Spinner;
pub use splitter::{PaneSize, SplitDirection, SplitPane, Splitter};
pub use tab_view::{Tab, TabId, TabPosition, TabView};
pub use table_view::{CellValue, Column, SortDirection, TableRow, TableView};
pub use text_area::TextArea;
pub use text_input::TextInput;
pub use toolbar::{ToolItem, ToolItemId, Toolbar};
pub use tree_view::{NodeId, TreeNode, TreeView};
