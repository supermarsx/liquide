//! UI toolkit for the LiquiDE desktop environment.
//!
//! Provides widgets, layout, event handling, painting, focus management,
//! animation, panel containers, and theming primitives for the LiquiDE
//! desktop shell and built-in applications.
//!
//! # Deprecated
//!
//! This crate is superseded by the `liquide-ui-core` + `liquide-ui-widgets`
//! + `liquide-ui-window` trio for retained-mode widgets, and by
//! `liquide-components` for DOM/template content. New code MUST use those
//! crates. See `docs/ui-toolkit-stance.md` for the canonical stance and
//! migration guidance. Consumer migration is tracked under task t10.
#![deprecated(
    note = "Use liquide-ui-core + liquide-ui-widgets + liquide-ui-window. See docs/ui-toolkit-stance.md."
)]

pub mod animation;
pub mod event;
pub mod focus;
pub mod geometry;
pub mod layout;
pub mod paint;
pub mod panel;
pub mod theme;
pub mod tree;
pub mod widget;

#[cfg(test)]
mod tests;

use thiserror::Error;

// Re-exports
pub use animation::{Animation, AnimationManager, Easing};
pub use event::{KeyCode, Modifiers, MouseButton, UiEvent};
pub use focus::{FocusChain, FocusDirection};
pub use geometry::{Corner, Insets, Point, Rect, Size};
pub use layout::{
    BoxLayout, GridLayout, LayoutAlign, LayoutConstraints, LayoutDirection, Margin, Padding,
    StackLayout,
};
pub use paint::{Brush, Color, PaintContext, StrokeStyle, TextStyle};
pub use panel::{Panel, PanelPosition, StatusBar, StatusBarSlot};
pub use theme::Theme;
pub use tree::WidgetTree;
pub use widget::{EventResult, Widget, WidgetId, WidgetState};

/// Errors produced by the UI toolkit.
#[derive(Debug, Error)]
pub enum UiError {
    /// A referenced widget could not be found.
    #[error("widget not found: {0}")]
    WidgetNotFound(WidgetId),

    /// A layout operation was invalid.
    #[error("invalid layout: {0}")]
    InvalidLayout(String),

    /// A cycle was detected in the widget tree.
    #[error("tree cycle detected involving widget {0}")]
    TreeCycle(WidgetId),

    /// A paint operation failed.
    #[error("paint error: {0}")]
    PaintError(String),

    /// A theme operation failed.
    #[error("theme error: {0}")]
    ThemeError(String),

    /// An animation operation failed.
    #[error("animation error: {0}")]
    AnimationError(String),
}

/// Result type for UI operations.
pub type Result<T> = std::result::Result<T, UiError>;
