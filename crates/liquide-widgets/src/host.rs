//! `WidgetHost` — the runtime state + interaction layer for CSS widgets.
//!
//! `WidgetHost` is to widgets what the shell's chrome subsystems are to the
//! dock/menus: it owns the mutable widget state, registers the real
//! [`EventDispatcher`] handlers, routes incoming [`DomEvent`]s to the right
//! [`WidgetBehavior`] by hit-node ancestry, and — when a behavior reports
//! [`WidgetOutcome::Changed`]/`Action` — re-renders that widget through the
//! [`TemplateRenderer`] so the new pseudo-states/classes reconcile into the DOM.
//! Emitted [`WidgetOutcome::Action`]s are collected for the embedding surface,
//! exactly like the chrome's "read `data-action`, dispatch to owner" contract.
//!
//! ## Event flow (single source of truth, no double-dispatch)
//!
//! The [`EventDispatcher`] fires `Send` closures that cannot borrow the host
//! mutably. So registered handlers do the minimal thing: push the raw event into
//! a shared queue ([`Arc<Mutex<Vec<DomEvent>>>`]). The owner then calls
//! [`WidgetHost::process_pending`] with a [`LayoutQuery`], which drains the queue
//! and applies each event to the owning behavior. This keeps state mutation on
//! the owner's thread (no interior-mutable behavior state, no lock around the
//! behaviors themselves) and means the dispatcher remains the single event
//! source — the host never re-derives events from input, avoiding the
//! double-fire class of bug.
//!
//! [`EventDispatcher`]: liquide_hit_test::EventDispatcher
//! [`TemplateRenderer`]: liquide_components::template::TemplateRenderer

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use liquide_components::template::TemplateRenderer;
use liquide_dom::{Document, NodeId};
use liquide_hit_test::event::{DomEvent, DomEventKind, Propagation};
use liquide_hit_test::{EventDispatcher, HitTestEngine};

use crate::behavior::{KeyInput, WidgetBehavior, WidgetId, WidgetOutcome};
use crate::layout_query::LayoutQuery;

/// A queued raw DOM event awaiting host-side dispatch to a behavior.
type EventQueue = Arc<Mutex<Vec<DomEvent>>>;

/// An action emitted by a widget for the embedding surface to handle.
#[derive(Debug, Clone, PartialEq)]
pub struct WidgetAction {
    /// The widget that emitted the action.
    pub widget: WidgetId,
    /// The action name.
    pub name: String,
    /// Optional payload.
    pub payload: Option<String>,
}

/// Owns mounted widget behaviors + their state, and bridges the
/// [`EventDispatcher`] to them.
///
/// [`EventDispatcher`]: liquide_hit_test::EventDispatcher
pub struct WidgetHost {
    /// Mounted behaviors, keyed by mount-point id (stable across reconciliation).
    widgets: HashMap<WidgetId, Box<dyn WidgetBehavior>>,
    /// DOM root node of each mounted widget (its mount element), for re-render
    /// reconciliation and hit-node -> widget resolution.
    roots: HashMap<WidgetId, NodeId>,
    /// Shared queue the dispatcher handlers push raw events into.
    queue: EventQueue,
    /// The currently keyboard-focused widget (mirrors dispatcher focus).
    focused: Option<WidgetId>,
}

impl WidgetHost {
    /// Create an empty host.
    pub fn new() -> Self {
        Self {
            widgets: HashMap::new(),
            roots: HashMap::new(),
            queue: Arc::new(Mutex::new(Vec::new())),
            focused: None,
        }
    }

    /// Number of mounted widgets.
    pub fn len(&self) -> usize {
        self.widgets.len()
    }

    /// Whether no widgets are mounted.
    pub fn is_empty(&self) -> bool {
        self.widgets.is_empty()
    }

    /// Borrow a mounted behavior (for state inspection / tests).
    pub fn behavior(&self, id: &str) -> Option<&dyn WidgetBehavior> {
        self.widgets.get(id).map(|b| b.as_ref())
    }

    /// The DOM root node of a mounted widget.
    pub fn root_of(&self, id: &str) -> Option<NodeId> {
        self.roots.get(id).copied()
    }

    /// Mount a widget: create/patch its DOM subtree under `parent`, record its
    /// behavior + root, and register dispatcher handlers for the events it wants.
    ///
    /// `id` is the stable mount-point id; the rendered template's root element id
    /// is set to it so reconciliation and hit-resolution agree.
    ///
    /// Returns the widget's root [`NodeId`].
    pub fn mount(
        &mut self,
        id: impl Into<WidgetId>,
        behavior: Box<dyn WidgetBehavior>,
        doc: &mut Document,
        parent: NodeId,
        dispatcher: &mut EventDispatcher,
    ) -> NodeId {
        let id: WidgetId = id.into();

        // 1. Render initial DOM, mounting the root under `parent` with id == `id`.
        let mut template = behavior.render();
        template.element_id = Some(id.clone());
        let root = TemplateRenderer::apply_or_create(doc, parent, &id, &template);

        // 2. Register one handler per wanted event-kind that forwards the raw
        //    event into the shared queue. For BUBBLING events we register on the
        //    widget ROOT only: the dispatcher bubbles descendant events up to it
        //    (hover chain / event path), so a click on a sub-part still reaches
        //    the root handler. For NON-BUBBLING events (notably `Scroll`/wheel,
        //    which per the W3C UI Events spec do NOT bubble) the dispatcher fires
        //    only on the exact hit node, so we additionally register the handler
        //    on every descendant of the widget root that exists at mount time.
        //    `process_pending` resolves the owning widget from the event target
        //    by ancestor walk, so a descendant-hit non-bubbling event is still
        //    attributed to this widget.
        for sample in behavior.wanted_events() {
            let bubbles = sample.bubbles();
            if bubbles {
                self.register_handler(dispatcher, root, sample);
            } else {
                // Register on root + all current descendants so a wheel landing on
                // any sub-part reaches the queue.
                let mut targets = vec![root];
                Self::collect_descendants(doc, root, &mut targets);
                for node in targets {
                    self.register_handler(dispatcher, node, sample.clone());
                }
            }
        }

        self.widgets.insert(id.clone(), behavior);
        self.roots.insert(id, root);
        root
    }

    /// Register one queue-forwarding handler on `node` for `sample`'s event kind.
    fn register_handler(
        &self,
        dispatcher: &mut EventDispatcher,
        node: NodeId,
        sample: DomEventKind,
    ) {
        let queue = Arc::clone(&self.queue);
        dispatcher.add_handler(
            node,
            Some(sample),
            Box::new(move |ev: &DomEvent| {
                if let Ok(mut q) = queue.lock() {
                    q.push(ev.clone());
                }
                Propagation::Continue
            }),
        );
    }

    /// Collect every descendant of `node` (pre-order) into `out`.
    fn collect_descendants(doc: &Document, node: NodeId, out: &mut Vec<NodeId>) {
        for &child in doc.children(node) {
            out.push(child);
            Self::collect_descendants(doc, child, out);
        }
    }

    /// Drain the queued dispatcher events and apply each to its owning widget,
    /// re-rendering changed widgets and collecting emitted actions.
    ///
    /// Call this once per frame (or after pumping input) on the owner's thread.
    /// Takes the [`HitTestEngine`] (carrying the real laid-out tree) and builds a
    /// short-lived [`LayoutQuery`] per event, so the immutable doc borrow for
    /// geometry is dropped before the mutable re-render.
    pub fn process_pending(
        &mut self,
        doc: &mut Document,
        hit_test: &HitTestEngine,
    ) -> Vec<WidgetAction> {
        let drained: Vec<DomEvent> = {
            let mut q = self.queue.lock().expect("widget event queue poisoned");
            std::mem::take(&mut *q)
        };

        let mut actions = Vec::new();
        for event in drained {
            // Resolve which mounted widget owns the event target: the target
            // itself or any of its DOM ancestors must be a widget root.
            let Some(id) = self.owner_of(event.target, doc) else {
                continue;
            };
            let Some(&root) = self.roots.get(&id) else {
                continue;
            };
            // Geometry read (immutable doc borrow) scoped tightly, then dropped
            // before apply_outcome re-renders (mutable doc borrow).
            let outcome = {
                let q = LayoutQuery::new(hit_test, doc);
                let Some(behavior) = self.widgets.get_mut(&id) else {
                    continue;
                };
                behavior.on_dom_event(root, &event, &q)
            };
            self.apply_outcome(&id, outcome, doc, &mut actions);
        }
        actions
    }

    /// Route a keyboard key to the focused widget (set via [`set_focus`]).
    ///
    /// [`set_focus`]: Self::set_focus
    pub fn on_keyboard(
        &mut self,
        key: KeyInput,
        doc: &mut Document,
        hit_test: &HitTestEngine,
    ) -> Vec<WidgetAction> {
        let mut actions = Vec::new();
        let Some(id) = self.focused.clone() else {
            return actions;
        };
        let Some(&root) = self.roots.get(&id) else {
            return actions;
        };
        let outcome = {
            let q = LayoutQuery::new(hit_test, doc);
            let Some(behavior) = self.widgets.get_mut(&id) else {
                return actions;
            };
            behavior.on_keyboard(root, key, &q)
        };
        self.apply_outcome(&id, outcome, doc, &mut actions);
        actions
    }

    /// Set the keyboard-focused widget and mirror it onto the dispatcher (so DOM
    /// `:focus` / keyboard routing agree). Pass `None` to clear focus.
    pub fn set_focus(
        &mut self,
        id: Option<&str>,
        doc: &mut Document,
        dispatcher: &mut EventDispatcher,
    ) {
        match id {
            Some(id) if self.widgets.contains_key(id) => {
                let node = self.roots.get(id).copied();
                dispatcher.set_focus(node, doc);
                self.focused = Some(id.to_string());
            }
            _ => {
                dispatcher.set_focus(None, doc);
                self.focused = None;
            }
        }
    }

    /// The currently focused widget id, if any.
    pub fn focused(&self) -> Option<&str> {
        self.focused.as_deref()
    }

    /// Force a re-render (reconcile) of a single mounted widget — e.g. after the
    /// owner mutated its state directly. Returns `false` if no such widget.
    pub fn rerender(&mut self, id: &str, doc: &mut Document) -> bool {
        let (Some(behavior), Some(&root)) = (self.widgets.get(id), self.roots.get(id)) else {
            return false;
        };
        let mut template = behavior.render();
        template.element_id = Some(id.to_string());
        TemplateRenderer::apply_to_node(doc, root, &template);
        true
    }

    // ── internals ────────────────────────────────────────────────────────

    fn apply_outcome(
        &mut self,
        id: &str,
        outcome: WidgetOutcome,
        doc: &mut Document,
        actions: &mut Vec<WidgetAction>,
    ) {
        if outcome.needs_render() {
            self.rerender(id, doc);
        }
        if let WidgetOutcome::Action { name, payload } = outcome {
            actions.push(WidgetAction {
                widget: id.to_string(),
                name,
                payload,
            });
        }
    }

    /// Resolve the mounted widget that owns `target`: `target` itself if it is a
    /// widget root, else the nearest ancestor that is.
    fn owner_of(&self, target: NodeId, doc: &Document) -> Option<WidgetId> {
        if let Some(id) = self.id_for_root(target) {
            return Some(id);
        }
        for ancestor in doc.ancestors(target) {
            if let Some(id) = self.id_for_root(ancestor) {
                return Some(id);
            }
        }
        None
    }

    fn id_for_root(&self, node: NodeId) -> Option<WidgetId> {
        self.roots
            .iter()
            .find(|&(_, &root)| root == node)
            .map(|(id, _)| id.clone())
    }
}

impl Default for WidgetHost {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper: the discriminant set of event kinds a host registered for, useful in
/// tests to assert a behavior's `wanted_events` reached the dispatcher.
pub fn wanted_discriminants(kinds: &[DomEventKind]) -> Vec<std::mem::Discriminant<DomEventKind>> {
    kinds.iter().map(std::mem::discriminant).collect()
}
