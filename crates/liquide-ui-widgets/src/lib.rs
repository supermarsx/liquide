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

pub mod button;
pub mod checkbox;
pub mod dropdown;
pub mod label;
pub mod progress;
pub mod scroll_view;
pub mod separator;
pub mod slider;
pub mod text_area;
pub mod text_input;

// Re-exports
pub use button::{Button, ButtonStyle, ButtonVariant};
pub use checkbox::Checkbox;
pub use dropdown::{Dropdown, DropdownItem};
pub use label::Label;
pub use progress::ProgressBar;
pub use scroll_view::ScrollView;
pub use separator::Separator;
pub use slider::Slider;
pub use text_area::TextArea;
pub use text_input::TextInput;
