//! `WidgetBehavior` — the interaction + state contract every CSS widget implements.
//!
//! A widget is split into two halves, mirroring how the chrome works:
//!
//! - **appearance** is a [`Component`] (from `liquide-components`) that emits a
//!   `<lq-*>` [`TemplateNode`] subtree styled purely in CSS; the
//!   [`TemplateRenderer`] reconciles it into the live [`liquide_dom::Document`];
//! - **behavior** is a [`WidgetBehavior`]: it owns the runtime state (checked,
//!   value, selection, open, …), consumes [`DomEvent`]s + keyboard, mutates that
//!   state, and re-emits the [`TemplateNode`] so the renderer patches the new
//!   pseudo-states/classes/attrs back into the DOM.
//!
//! The behavior reads ALL hit geometry through [`LayoutQuery`] (the laid-out CSS
//! box), never a constant — see that module for the rationale.
//!
//! [`Component`]: liquide_components::template::Component
//! [`TemplateNode`]: liquide_components::template::TemplateNode
//! [`TemplateRenderer`]: liquide_components::template::TemplateRenderer

use liquide_components::template::TemplateNode;
use liquide_dom::NodeId;
use liquide_hit_test::event::{DomEvent, DomEventKind};

use crate::layout_query::LayoutQuery;

/// Stable identity of a mounted widget instance.
///
/// Equals the widget root element's `id` (the mount point). State in
/// [`WidgetHost`](crate::host::WidgetHost) is keyed by this so it survives DOM
/// reconciliation (which reuses nodes by key, never by `WidgetId`).
pub type WidgetId = String;

/// A keyboard key + modifier snapshot handed to [`WidgetBehavior::on_keyboard`].
///
/// `key`/`modifiers` use the same raw `u32` encoding the [`DomEventKind::KeyDown`]
/// variant carries, so a behavior can route either a real DOM keyboard event or
/// a synthesized one identically.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyInput {
    /// Raw key code (platform/`liquide-input` `KeyCode as u32`).
    pub key: u32,
    /// Raw modifier bitflags.
    pub modifiers: u32,
}

impl KeyInput {
    /// Construct from raw key + modifier codes.
    pub fn new(key: u32, modifiers: u32) -> Self {
        Self { key, modifiers }
    }
}

/// What kind of widget a behavior drives. Lets the host (and tests) reason about
/// a behavior without downcasting, and lets shared keyboard helpers branch on
/// the family (e.g. radio arrow-key navigation vs. button Enter/Space).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WidgetKind {
    /// A clickable command control (`<lq-button>`).
    Button,
    /// A text/link label (`<lq-label>` / `<lq-link>`).
    Label,
    /// A single-line text field (`<lq-input>`).
    Input,
    /// A two-state toggle: checkbox / radio / switch.
    Toggle,
    /// A draggable value control (`<lq-slider>`).
    Slider,
    /// A container with no own interaction (`<lq-panel>`).
    Container,
    /// A selectable collection (`<lq-list>` / `<lq-table>` / `<lq-tree>`).
    Collection,
    /// A reference/test widget used to validate the infrastructure end-to-end.
    Reference,
    /// An extension point for families not yet enumerated.
    Other,
}

/// The result of handing an event/keyboard to a [`WidgetBehavior`].
///
/// The host uses it to decide whether to re-render the widget and what to bubble
/// to the embedding surface (the window content / dialog).
#[derive(Debug, Clone, PartialEq)]
pub enum WidgetOutcome {
    /// The event was irrelevant; nothing changed, do not re-render.
    Ignored,
    /// Internal state changed; the host should re-render (reconcile) this widget.
    Changed,
    /// State changed AND the widget emits a semantic action for the owner to
    /// handle (e.g. a button's `data-action`, a list selection, a slider value).
    /// Carries the re-render obligation of [`Changed`](Self::Changed) too.
    Action {
        /// The action name (mirrors the chrome `data-action` convention).
        name: String,
        /// An optional payload (selected index, new value, href, …) as a string
        /// so it travels through the same untyped seam the chrome uses.
        payload: Option<String>,
    },
}

impl WidgetOutcome {
    /// An action with a name and no payload.
    pub fn action(name: impl Into<String>) -> Self {
        WidgetOutcome::Action {
            name: name.into(),
            payload: None,
        }
    }

    /// An action with a name and a payload.
    pub fn action_with(name: impl Into<String>, payload: impl Into<String>) -> Self {
        WidgetOutcome::Action {
            name: name.into(),
            payload: Some(payload.into()),
        }
    }

    /// Whether the host should re-render the widget after this outcome.
    pub fn needs_render(&self) -> bool {
        matches!(self, WidgetOutcome::Changed | WidgetOutcome::Action { .. })
    }
}

/// The interaction + state contract implemented by each widget family.
///
/// Implementors are `Send` so a host can live behind the shell's threading model
/// (handlers are registered on the `Send` [`EventDispatcher`]).
///
/// [`EventDispatcher`]: liquide_hit_test::EventDispatcher
pub trait WidgetBehavior: Send {
    /// The widget family.
    fn kind(&self) -> WidgetKind;

    /// The DOM event kinds this widget wants delivered. The host registers
    /// dispatcher handlers only for these (so a static label asks for nothing
    /// and costs nothing). Returned as concrete [`DomEventKind`] samples; the
    /// host filters by discriminant (variant), ignoring the payload fields.
    fn wanted_events(&self) -> Vec<DomEventKind>;

    /// Consume a DOM event, mutate state, and report the outcome.
    ///
    /// `root` is this widget's own root [`NodeId`] (the event's `target` may be a
    /// descendant sub-element); use it with `layout` to read the WIDGET's
    /// geometry, and `layout.box_of_part(root, "...")` for sub-parts. `layout`
    /// gives the laid-out CSS boxes so all hit math is geometry-derived — never a
    /// constant.
    fn on_dom_event(
        &mut self,
        root: NodeId,
        event: &DomEvent,
        layout: &LayoutQuery,
    ) -> WidgetOutcome;

    /// Handle a keyboard key (for the focused widget). `root` is the widget's own
    /// root node. Default: ignore — leaf widgets without keyboard semantics need
    /// not implement it.
    fn on_keyboard(
        &mut self,
        _root: NodeId,
        _key: KeyInput,
        _layout: &LayoutQuery,
    ) -> WidgetOutcome {
        WidgetOutcome::Ignored
    }

    /// Re-emit the widget's `<lq-*>` template subtree from current state. The
    /// host feeds this to [`TemplateRenderer`] to reconcile the DOM.
    ///
    /// [`TemplateRenderer`]: liquide_components::template::TemplateRenderer
    fn render(&self) -> TemplateNode;

    /// Whether this widget participates in the keyboard focus ring. Default:
    /// `true` for interactive families, `false` for pure containers/labels.
    fn focusable(&self) -> bool {
        !matches!(self.kind(), WidgetKind::Container | WidgetKind::Label)
    }

    /// Downcast hook for typed state inspection (the host stores behaviors as
    /// trait objects). Implementors return `self`; the default suffices for
    /// concrete types via the blanket impl note below.
    fn as_any(&self) -> &dyn std::any::Any;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outcome_render_obligation() {
        assert!(!WidgetOutcome::Ignored.needs_render());
        assert!(WidgetOutcome::Changed.needs_render());
        assert!(WidgetOutcome::action("ok").needs_render());
    }

    #[test]
    fn outcome_action_constructors() {
        assert_eq!(
            WidgetOutcome::action("nav"),
            WidgetOutcome::Action {
                name: "nav".into(),
                payload: None
            }
        );
        assert_eq!(
            WidgetOutcome::action_with("nav", "/home"),
            WidgetOutcome::Action {
                name: "nav".into(),
                payload: Some("/home".into())
            }
        );
    }
}
