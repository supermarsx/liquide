//! Core widget trait and types.

use std::fmt;

use crate::event::UiEvent;
use crate::geometry::Rect;
use crate::paint::PaintContext;

/// Unique identifier for a widget in the tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct WidgetId(pub u64);

impl fmt::Display for WidgetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Widget({})", self.0)
    }
}

impl WidgetId {
    /// Create a new widget identifier.
    #[must_use]
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

/// Result of handling a UI event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventResult {
    /// The event was consumed by this widget.
    Consumed,
    /// The event was not handled; propagation should continue.
    Ignored,
    /// The event should be redirected to another widget.
    Redirect(WidgetId),
}

/// The trait that all widgets must implement.
pub trait Widget {
    /// The unique identifier of this widget.
    fn id(&self) -> WidgetId;

    /// The bounding rectangle of this widget.
    fn bounds(&self) -> Rect;

    /// Set the bounding rectangle.
    fn set_bounds(&mut self, rect: Rect);

    /// Whether this widget is visible.
    fn visible(&self) -> bool;

    /// Set visibility.
    fn set_visible(&mut self, visible: bool);

    /// Whether this widget can receive keyboard focus.
    fn focusable(&self) -> bool;

    /// The child widget identifiers.
    fn children(&self) -> &[WidgetId];

    /// Handle a UI event.
    fn handle_event(&mut self, event: &UiEvent) -> EventResult;

    /// Paint this widget into the given context.
    fn paint(&self, ctx: &mut PaintContext);
}

/// Common state tracked for every widget.
#[derive(Debug, Clone)]
pub struct WidgetState {
    /// Widget identifier.
    pub id: WidgetId,
    /// Bounding rectangle.
    pub bounds: Rect,
    /// Whether the widget is visible.
    pub visible: bool,
    /// Whether the widget is enabled for interaction.
    pub enabled: bool,
    /// Whether the widget currently has focus.
    pub focused: bool,
    /// Whether the mouse is hovering over the widget.
    pub hovered: bool,
    /// Whether a mouse button is pressed on the widget.
    pub pressed: bool,
}

impl WidgetState {
    /// Create a new widget state.
    #[must_use]
    pub fn new(id: WidgetId) -> Self {
        Self {
            id,
            bounds: Rect::zero(),
            visible: true,
            enabled: true,
            focused: false,
            hovered: false,
            pressed: false,
        }
    }
}

/// A base widget implementation with common state tracking.
#[derive(Debug, Clone)]
pub struct BaseWidget {
    /// Internal state.
    state: WidgetState,
    /// Whether this widget can receive focus.
    focusable: bool,
    /// Child widget identifiers.
    children: Vec<WidgetId>,
}

impl BaseWidget {
    /// Create a new base widget.
    #[must_use]
    pub fn new(id: WidgetId, focusable: bool) -> Self {
        Self {
            state: WidgetState::new(id),
            focusable,
            children: Vec::new(),
        }
    }

    /// Access the internal widget state.
    #[must_use]
    pub fn state(&self) -> &WidgetState {
        &self.state
    }

    /// Access the internal widget state mutably.
    pub fn state_mut(&mut self) -> &mut WidgetState {
        &mut self.state
    }

    /// Add a child widget identifier.
    pub fn add_child(&mut self, child: WidgetId) {
        self.children.push(child);
    }

    /// Remove a child widget identifier.
    pub fn remove_child(&mut self, child: &WidgetId) {
        self.children.retain(|c| c != child);
    }
}

impl Widget for BaseWidget {
    fn id(&self) -> WidgetId {
        self.state.id
    }

    fn bounds(&self) -> Rect {
        self.state.bounds
    }

    fn set_bounds(&mut self, rect: Rect) {
        self.state.bounds = rect;
    }

    fn visible(&self) -> bool {
        self.state.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.state.visible = visible;
    }

    fn focusable(&self) -> bool {
        self.focusable
    }

    fn children(&self) -> &[WidgetId] {
        &self.children
    }

    fn handle_event(&mut self, event: &UiEvent) -> EventResult {
        match event {
            UiEvent::MouseEnter => {
                self.state.hovered = true;
                EventResult::Consumed
            }
            UiEvent::MouseLeave => {
                self.state.hovered = false;
                self.state.pressed = false;
                EventResult::Consumed
            }
            UiEvent::MouseDown { .. } => {
                self.state.pressed = true;
                EventResult::Consumed
            }
            UiEvent::MouseUp { .. } => {
                self.state.pressed = false;
                EventResult::Consumed
            }
            UiEvent::FocusIn => {
                self.state.focused = true;
                EventResult::Consumed
            }
            UiEvent::FocusOut => {
                self.state.focused = false;
                EventResult::Consumed
            }
            _ => EventResult::Ignored,
        }
    }

    fn paint(&self, _ctx: &mut PaintContext) {
        // Base widget has no visual representation by default.
    }
}
