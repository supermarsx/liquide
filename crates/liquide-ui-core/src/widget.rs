//! Core widget trait — the fundamental building block.

use crate::event::{Event, EventResponse};
use crate::id::WidgetId;
use crate::painter::Painter;
use crate::constraints::Constraints;
use crate::layout::LayoutResult;
use crate::theme::UiTheme;

/// Result of handling a UI event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventResult {
    Consumed,
    Ignored,
}

/// Lifecycle events for widgets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetLifecycle {
    /// Widget was added to the tree.
    Mounted,
    /// Widget is about to be removed from the tree.
    Unmounting,
    /// The theme changed.
    ThemeChanged,
}

/// Common state tracked for every widget.
#[derive(Debug, Clone)]
pub struct WidgetState {
    pub id: WidgetId,
    pub visible: bool,
    pub enabled: bool,
    pub focused: bool,
    pub hovered: bool,
    pub pressed: bool,
    pub tooltip: Option<String>,
}

impl WidgetState {
    pub fn new(id: WidgetId) -> Self {
        Self {
            id,
            visible: true,
            enabled: true,
            focused: false,
            hovered: false,
            pressed: false,
            tooltip: None,
        }
    }

    pub fn with_tooltip(mut self, text: impl Into<String>) -> Self {
        self.tooltip = Some(text.into());
        self
    }
}

/// The trait that all widgets implement.
///
/// Inspired by Qt's `QWidget` and GTK's `GtkWidget`, this provides a
/// unified interface for:
/// - **Measurement** — determine preferred size given constraints.
/// - **Layout** — position the widget at a concrete rect.
/// - **Painting** — draw the widget via the `Painter` abstraction.
/// - **Event handling** — keyboard, mouse, focus.
/// - **Lifecycle** — mount/unmount callbacks.
pub trait Widget: Send {
    /// The unique identifier of this widget.
    fn id(&self) -> WidgetId;

    /// Whether this widget is visible.
    fn visible(&self) -> bool;

    /// Set visibility.
    fn set_visible(&mut self, visible: bool);

    /// Whether this widget is enabled for interaction.
    fn enabled(&self) -> bool;

    /// Set enabled state.
    fn set_enabled(&mut self, enabled: bool);

    /// Whether this widget can receive keyboard focus.
    fn focusable(&self) -> bool { false }

    /// Get the tooltip text for this widget, if any.
    fn tooltip(&self) -> Option<&str> { None }

    /// Measure the preferred size given parent constraints.
    fn measure(&self, constraints: &Constraints, theme: &UiTheme) -> LayoutResult;

    /// Layout this widget at the given position and size.
    fn layout(&mut self, x: f32, y: f32, width: f32, height: f32);

    /// Paint this widget.
    fn paint(&self, painter: &mut Painter, theme: &UiTheme);

    /// Handle a UI event. Return whether it was consumed.
    fn handle_event(&mut self, event: &Event) -> EventResponse;

    /// Called when lifecycle events occur.
    fn lifecycle(&mut self, _event: WidgetLifecycle) {}

    /// Child widgets (for composite widgets).
    fn children(&self) -> &[WidgetId] { &[] }
}
