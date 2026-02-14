//! Core UI building blocks for the LiquiDE desktop environment.
//!
//! This crate provides the foundational primitives for building user
//! interfaces: a retained-mode widget trait, layout engine, event
//! dispatch, paint commands, and theming integration.
//!
//! Higher-level widget crates (buttons, inputs, dropdowns, etc.) build
//! on top of these primitives.

pub mod callback;
pub mod color;
pub mod constraints;
pub mod event;
pub mod focus;
pub mod id;
pub mod layout;
pub mod painter;
pub mod style;
pub mod text;
pub mod theme;
pub mod widget;

// Re-exports
pub use callback::{Callback, CallbackId};
pub use color::UiColor;
pub use constraints::Constraints;
pub use event::{Event, EventResponse, MouseButton, Modifiers, Key};
pub use focus::{FocusChain, FocusDirection, FocusId};
pub use id::WidgetId;
pub use layout::{Alignment, Direction, LayoutNode, LayoutResult, Padding, Spacing};
pub use painter::{PaintCommand, Painter};
pub use style::{BorderStyle, BoxShadow, StyleSheet};
pub use text::{FontMetrics, TextMeasure};
pub use theme::{ThemeColors, ThemeMode, UiTheme};
pub use widget::{EventResult, Widget, WidgetLifecycle, WidgetState};

use thiserror::Error;

/// Errors produced by the UI core.
#[derive(Debug, Error)]
pub enum UiCoreError {
    #[error("widget not found: {0}")]
    WidgetNotFound(WidgetId),

    #[error("layout error: {0}")]
    LayoutError(String),

    #[error("paint error: {0}")]
    PaintError(String),

    #[error("theme error: {0}")]
    ThemeError(String),
}

/// Result type for UI core operations.
pub type Result<T> = std::result::Result<T, UiCoreError>;
