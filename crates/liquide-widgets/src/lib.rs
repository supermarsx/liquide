//! # liquide-widgets
//!
//! Reusable, CSS-styled, interactive widget toolkit rendered through the DOM+CSS
//! pipeline — the t98 P7 "all GUI elements" answer. It **generalizes** the
//! proven chrome authoring pattern (`Component` -> `TemplateNode` ->
//! `TemplateRenderer`, keyed reconciliation + pseudo-state patching) from
//! [`liquide_components`] into a shared widget infrastructure.
//!
//! This crate is **S0 (shared infrastructure)**: it provides the foundation that
//! every individual widget (Groups A-D) will build on, plus ONE reference widget
//! ([`ReferenceBox`]) that validates the infrastructure end-to-end. No
//! individual widget families ship here yet.
//!
//! ## The two halves of a widget
//!
//! - **Appearance** — a [`Component`] emitting a `<lq-*>` [`TemplateNode`]
//!   subtree styled purely in CSS (`assets/themes/widgets.css` + per-widget
//!   sections), with `pseudo_if(...)` for `:hover`/`:active`/`:focus`/
//!   `:checked`/`:disabled`.
//! - **Behavior** — a [`WidgetBehavior`]: owns runtime state, consumes
//!   [`DomEvent`]s + keyboard, emits [`WidgetOutcome`]s; the [`WidgetHost`]
//!   re-renders it through [`TemplateRenderer`] so changes reconcile into the
//!   live DOM.
//!
//! ## The anti-constant guard
//!
//! All interaction reads hit geometry through [`LayoutQuery`] — the laid-out CSS
//! box — never a hardcoded constant. This is the structural defense against the
//! menu-hit-test-mismatch bug class. The S0 gallery harness drives the REAL
//! pipeline + real event dispatcher so a constant-based widget cannot pass.
//!
//! [`Component`]: liquide_components::template::Component
//! [`TemplateNode`]: liquide_components::template::TemplateNode
//! [`TemplateRenderer`]: liquide_components::template::TemplateRenderer
//! [`DomEvent`]: liquide_hit_test::event::DomEvent

pub mod behavior;
pub mod focus;
pub mod host;
pub mod keys;
pub mod layout_query;
pub mod reference;

// ── Group A — foundational controls ─────────────────────────────────────────
pub mod button;
pub mod input;
pub mod label;
pub mod slider;
pub mod textarea;
pub mod toggle;

// ── Group B — containers ────────────────────────────────────────────────────
pub mod container;
pub mod scroll_area;
pub mod tabs;
pub mod toolbar;

// ── Group C — collections ───────────────────────────────────────────────────
pub mod list;
pub mod menu;
pub mod table;
pub mod tree;

// ── Group D — pickers / advanced ────────────────────────────────────────────
pub mod accordion;
pub mod breadcrumb;
pub mod chip;
pub mod color_picker;
pub mod date_picker;
pub mod dropdown;
pub mod pagination;
pub mod progress;
pub mod segmented;

// ── Group E — esoteric / creative PRO controls ──────────────────────────────
pub mod color_wheel;
pub mod gradient_editor;
pub mod knob;
pub mod range_slider;
pub mod xy_pad;

// ── Group F — DATA/VIZ controls (graphs/charts) ─────────────────────────────
pub mod bar_chart;
pub mod chart;
pub mod donut_chart;
pub mod gauge;
pub mod heatmap;
pub mod line_chart;
pub mod sparkline;

// ── Group COMPOSITE — composite INPUT controls ──────────────────────────────
pub mod masked_input;
pub mod rating;
pub mod spinbox;
pub mod split_button;
pub mod tag_input;
pub mod toggle_group;

// ── Group GRID — grid / listbox / classic GDI controls ──────────────────────
pub mod data_grid;
pub mod hotkey_input;
pub mod ip_input;
pub mod listbox;
pub mod listview;
pub mod month_calendar;

// The real-pipeline gallery test harness (depends on dev-deps; test-only).
#[cfg(test)]
mod gallery;

// End-to-end infra-validation tests for the reference widget.
#[cfg(test)]
mod gallery_tests;

// Per-widget real-pipeline gallery tests (Group A).
#[cfg(test)]
mod button_tests;
#[cfg(test)]
mod input_tests;
#[cfg(test)]
mod label_tests;
#[cfg(test)]
mod slider_tests;
#[cfg(test)]
mod textarea_tests;
#[cfg(test)]
mod toggle_tests;

// Per-widget real-pipeline gallery tests (Group B).
#[cfg(test)]
mod container_tests;
#[cfg(test)]
mod scroll_area_tests;
#[cfg(test)]
mod tabs_tests;
#[cfg(test)]
mod toolbar_tests;

// Per-widget real-pipeline gallery tests (Group C).
#[cfg(test)]
mod list_tests;
#[cfg(test)]
mod menu_tests;
#[cfg(test)]
mod table_tests;
#[cfg(test)]
mod tree_tests;

// Per-widget real-pipeline gallery tests (Group D).
#[cfg(test)]
mod accordion_tests;
#[cfg(test)]
mod breadcrumb_tests;
#[cfg(test)]
mod chip_tests;
#[cfg(test)]
mod color_picker_tests;
#[cfg(test)]
mod date_picker_tests;
#[cfg(test)]
mod dropdown_tests;
#[cfg(test)]
mod pagination_tests;
#[cfg(test)]
mod progress_tests;
#[cfg(test)]
mod segmented_tests;

// Per-widget real-pipeline gallery tests (Group E).
#[cfg(test)]
mod color_wheel_tests;
#[cfg(test)]
mod gradient_editor_tests;
#[cfg(test)]
mod knob_tests;
#[cfg(test)]
mod range_slider_tests;
#[cfg(test)]
mod xy_pad_tests;

// Per-widget real-pipeline gallery tests (Group F — DATA/VIZ).
#[cfg(test)]
mod bar_chart_tests;
#[cfg(test)]
mod donut_chart_tests;
#[cfg(test)]
mod gauge_tests;
#[cfg(test)]
mod heatmap_tests;
#[cfg(test)]
mod line_chart_tests;
#[cfg(test)]
mod sparkline_tests;

// Per-widget real-pipeline gallery tests (Group COMPOSITE).
#[cfg(test)]
mod masked_input_tests;
#[cfg(test)]
mod rating_tests;
#[cfg(test)]
mod spinbox_tests;
#[cfg(test)]
mod split_button_tests;
#[cfg(test)]
mod tag_input_tests;
#[cfg(test)]
mod toggle_group_tests;

// Per-widget real-pipeline gallery tests (Group GRID).
#[cfg(test)]
mod data_grid_tests;
#[cfg(test)]
mod hotkey_input_tests;
#[cfg(test)]
mod ip_input_tests;
#[cfg(test)]
mod listbox_tests;
#[cfg(test)]
mod listview_tests;
#[cfg(test)]
mod month_calendar_tests;

// ── Public API surface ────────────────────────────────────────────────────

pub use behavior::{KeyInput, WidgetBehavior, WidgetId, WidgetKind, WidgetOutcome};
pub use focus::{FocusRing, FOCUSABLE_ATTR};
pub use host::{WidgetAction, WidgetHost};
pub use layout_query::LayoutQuery;
pub use reference::ReferenceBox;

// Group A widgets.
pub use button::Button;
pub use input::TextInput;
pub use label::{Label, Link};
pub use slider::Slider;
pub use textarea::TextArea;
pub use toggle::{RadioGroup, Toggle, ToggleStyle};

// Group B widgets (containers).
pub use container::{Card, GroupBox, Panel};
pub use scroll_area::ScrollArea;
pub use tabs::Tabs;
pub use toolbar::{Toolbar, ToolbarOrientation};

// Group C widgets (collections).
pub use list::{List, SelectionMode};
pub use menu::{Menu, MenuEntry};
pub use table::{SortDir, Table};
pub use tree::{Tree, TreeNode};

// Group D widgets (pickers / advanced).
pub use accordion::Accordion;
pub use breadcrumb::Breadcrumb;
pub use chip::Chip;
pub use color_picker::{ColorPicker, Rgb};
pub use date_picker::DatePicker;
pub use dropdown::Dropdown;
pub use pagination::Pagination;
pub use progress::{Progress, Spinner};
pub use segmented::Segmented;

// Group E widgets (esoteric / creative PRO controls).
pub use color_wheel::ColorWheel;
pub use gradient_editor::{GradientEditor, Stop};
pub use knob::Knob;
pub use range_slider::{RangeSlider, Thumb};
pub use xy_pad::XyPad;

// Group F widgets (DATA/VIZ — graphs/charts).
pub use bar_chart::BarChart;
pub use chart::Series;
pub use donut_chart::{DonutChart, Segment};
pub use gauge::Gauge;
pub use heatmap::Heatmap;
pub use line_chart::LineChart;
pub use sparkline::{Sparkline, SparkMode};

// Group COMPOSITE widgets (composite INPUT controls).
pub use masked_input::MaskedInput;
pub use rating::Rating;
pub use spinbox::Spinbox;
pub use split_button::SplitButton;
pub use tag_input::TagInput;
pub use toggle_group::{ToggleGroup, ToggleMode};

// Group GRID widgets (grid / listbox / classic GDI controls).
pub use data_grid::DataGrid;
pub use hotkey_input::{Chord, HotkeyInput};
pub use ip_input::IpInput;
pub use listbox::{ListBox, ListItem};
pub use listview::{ListView, ViewItem, ViewMode};
pub use month_calendar::MonthCalendar;

// Re-export the generalized authoring substrate so widget authors use ONE path:
// the same Component / TemplateNode / TemplateRenderer the chrome uses.
pub use liquide_components::template::{Component, TemplateNode, TemplateRenderer};
