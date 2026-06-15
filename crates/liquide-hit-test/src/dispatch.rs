//! DOM event dispatcher — manages hover chain, focus, and generates DOM events.

use liquide_dom::pseudo::PseudoStateFlags;
use liquide_dom::{Document, NodeId};
use liquide_layout::geometry::Point;

use crate::engine::{HitTestEngine, HitTestResult};
use crate::event::{DomEvent, DomEventKind, EventPhase, MouseButton, Propagation};

/// Callback type for event handlers.
pub type EventHandler = Box<dyn Fn(&DomEvent) -> Propagation + Send>;

/// Dispatches input into DOM events with proper hover/focus management.
pub struct EventDispatcher {
    /// Currently hovered node chain (leaf-first).
    hover_chain: Vec<NodeId>,
    /// Currently focused node.
    focus: Option<NodeId>,
    /// Last mouse position.
    last_mouse: Point,
    /// Last click target + timestamp for double-click detection.
    last_click: Option<(NodeId, std::time::Instant)>,
    /// Registered event handlers: (node, discriminant filter, capture, handler).
    ///
    /// `capture == true` => the listener fires during the W3C capture phase
    /// (root → target); `capture == false` => it fires during the bubble phase
    /// (target → root). Both fire at the target node in registration order.
    handlers: Vec<(
        NodeId,
        Option<std::mem::Discriminant<DomEventKind>>,
        bool,
        EventHandler,
    )>,
}

impl EventDispatcher {
    /// Create a new event dispatcher.
    pub fn new() -> Self {
        Self {
            hover_chain: Vec::new(),
            focus: None,
            last_mouse: Point::new(0.0, 0.0),
            last_click: None,
            handlers: Vec::new(),
        }
    }

    /// Get the currently focused node.
    pub fn focus(&self) -> Option<NodeId> {
        self.focus
    }

    /// Get the current hover chain.
    pub fn hover_chain(&self) -> &[NodeId] {
        &self.hover_chain
    }

    /// Register a **bubble-phase** event handler for a node.
    ///
    /// Convenience wrapper around [`add_event_listener`](Self::add_event_listener)
    /// with `capture = false`. Retained for the hover-chain dispatch path used by
    /// the `dispatch_mouse_*` / `dispatch_key_*` helpers.
    pub fn add_handler(
        &mut self,
        node: NodeId,
        kind_filter: Option<DomEventKind>,
        handler: EventHandler,
    ) {
        self.add_event_listener(node, kind_filter, false, handler);
    }

    /// Register an event listener for a node (W3C `addEventListener` semantics).
    ///
    /// `capture` selects the phase the listener fires in:
    /// - `true`  → capture phase (root → target),
    /// - `false` → bubble phase (target → root).
    ///
    /// At the target node, both capture and bubble listeners fire in registration
    /// order. Listeners registered here are driven by
    /// [`dispatch_events`](Self::dispatch_events), which performs full three-phase
    /// propagation using each event's `event_path`.
    pub fn add_event_listener(
        &mut self,
        node: NodeId,
        kind_filter: Option<DomEventKind>,
        capture: bool,
        handler: EventHandler,
    ) {
        let disc = kind_filter.as_ref().map(std::mem::discriminant);
        self.handlers.push((node, disc, capture, handler));
    }

    /// Dispatch a mouse move event. Updates hover chain and generates
    /// MouseEnter/MouseLeave/MouseMove events.
    pub fn dispatch_mouse_move(
        &mut self,
        pos: Point,
        doc: &mut Document,
        hit_test: &HitTestEngine,
    ) -> Vec<DomEvent> {
        self.last_mouse = pos;
        let mut events = Vec::new();

        let hit = hit_test.hit_test(pos);
        let new_target = hit.as_ref().map(|h| h.node);

        // Build new hover chain
        let new_chain = self.build_hover_chain(hit.as_ref(), doc);

        // Find nodes that lost hover
        for &old_node in &self.hover_chain {
            if !new_chain.contains(&old_node) {
                // Mouse leave
                doc.set_pseudo_state(old_node, PseudoStateFlags::HOVER, false);
                events.push(DomEvent::new(old_node, DomEventKind::MouseLeave));
            }
        }

        // Find nodes that gained hover
        for &new_node in &new_chain {
            if !self.hover_chain.contains(&new_node) {
                // Mouse enter
                doc.set_pseudo_state(new_node, PseudoStateFlags::HOVER, true);
                events.push(DomEvent::new(new_node, DomEventKind::MouseEnter));
            }
        }

        // Always dispatch mouse move to the target
        if let Some(target) = new_target {
            events.push(DomEvent::new(
                target,
                DomEventKind::MouseMove { x: pos.x, y: pos.y },
            ));
        }

        self.hover_chain = new_chain;
        self.fire_handlers(&events);
        events
    }

    /// Dispatch a mouse button down event.
    pub fn dispatch_mouse_down(
        &mut self,
        pos: Point,
        button: MouseButton,
        doc: &mut Document,
        hit_test: &HitTestEngine,
    ) -> Vec<DomEvent> {
        let mut events = Vec::new();
        let hit = hit_test.hit_test(pos);

        if let Some(h) = &hit {
            // Set :active
            doc.set_pseudo_state(h.node, PseudoStateFlags::ACTIVE, true);

            events.push(DomEvent::new(
                h.node,
                DomEventKind::MouseDown {
                    button,
                    x: pos.x,
                    y: pos.y,
                },
            ));

            // Focus management
            self.update_focus(h.node, doc, &mut events);
        }

        self.fire_handlers(&events);
        events
    }

    /// Dispatch a mouse button up event. Also generates Click/DoubleClick.
    pub fn dispatch_mouse_up(
        &mut self,
        pos: Point,
        button: MouseButton,
        doc: &mut Document,
        hit_test: &HitTestEngine,
    ) -> Vec<DomEvent> {
        let mut events = Vec::new();
        let hit = hit_test.hit_test(pos);

        if let Some(h) = &hit {
            // Clear :active
            doc.set_pseudo_state(h.node, PseudoStateFlags::ACTIVE, false);

            events.push(DomEvent::new(
                h.node,
                DomEventKind::MouseUp {
                    button,
                    x: pos.x,
                    y: pos.y,
                },
            ));

            // Generate click
            if matches!(button, MouseButton::Left) {
                // Check for double-click
                let now = std::time::Instant::now();
                let is_double = if let Some((prev_target, prev_time)) = self.last_click.take() {
                    prev_target == h.node && now.duration_since(prev_time).as_millis() < 500
                } else {
                    false
                };

                if is_double {
                    events.push(DomEvent::new(
                        h.node,
                        DomEventKind::DoubleClick { x: pos.x, y: pos.y },
                    ));
                    self.last_click = None;
                } else {
                    events.push(DomEvent::new(
                        h.node,
                        DomEventKind::Click {
                            button: MouseButton::Left,
                            x: pos.x,
                            y: pos.y,
                        },
                    ));
                    self.last_click = Some((h.node, now));
                }
            }

            // Right-click context menu
            if matches!(button, MouseButton::Right) {
                events.push(DomEvent::new(
                    h.node,
                    DomEventKind::ContextMenu { x: pos.x, y: pos.y },
                ));
            }
        }

        self.fire_handlers(&events);
        events
    }

    /// Dispatch a scroll event.
    pub fn dispatch_scroll(
        &mut self,
        pos: Point,
        delta_x: f32,
        delta_y: f32,
        hit_test: &HitTestEngine,
    ) -> Vec<DomEvent> {
        let mut events = Vec::new();
        let hit = hit_test.hit_test(pos);

        if let Some(h) = &hit {
            events.push(DomEvent::new(
                h.node,
                DomEventKind::Scroll {
                    dx: delta_x,
                    dy: delta_y,
                },
            ));
        }

        self.fire_handlers(&events);
        events
    }

    /// Dispatch a keyboard event to the focused node.
    pub fn dispatch_key_down(&self, key: u32, modifiers: u32) -> Vec<DomEvent> {
        let mut events = Vec::new();

        if let Some(focused) = self.focus {
            events.push(DomEvent::new(
                focused,
                DomEventKind::KeyDown { key, modifiers },
            ));
        }

        self.fire_handlers(&events);
        events
    }

    /// Dispatch a key up event to the focused node.
    pub fn dispatch_key_up(&self, key: u32, modifiers: u32) -> Vec<DomEvent> {
        let mut events = Vec::new();

        if let Some(focused) = self.focus {
            events.push(DomEvent::new(
                focused,
                DomEventKind::KeyUp { key, modifiers },
            ));
        }

        self.fire_handlers(&events);
        events
    }

    /// Set focus explicitly.
    pub fn set_focus(&mut self, node: Option<NodeId>, doc: &mut Document) -> Vec<DomEvent> {
        let mut events = Vec::new();
        if self.focus == node {
            return events;
        }

        // Blur old
        if let Some(old) = self.focus.take() {
            doc.set_pseudo_state(old, PseudoStateFlags::FOCUS, false);
            doc.set_pseudo_state(old, PseudoStateFlags::FOCUS_VISIBLE, false);
            events.push(DomEvent::new(old, DomEventKind::Blur));

            // Clear :focus-within on ancestors
            for ancestor in doc.ancestors(old) {
                doc.set_pseudo_state(ancestor, PseudoStateFlags::FOCUS_WITHIN, false);
            }
        }

        // Focus new
        if let Some(new_node) = node {
            doc.set_pseudo_state(new_node, PseudoStateFlags::FOCUS, true);
            events.push(DomEvent::new(new_node, DomEventKind::Focus));

            // Set :focus-within on ancestors
            for ancestor in doc.ancestors(new_node) {
                doc.set_pseudo_state(ancestor, PseudoStateFlags::FOCUS_WITHIN, true);
            }

            self.focus = Some(new_node);
        }

        self.fire_handlers(&events);
        events
    }

    // ---- Private helpers ----

    fn build_hover_chain(&self, hit: Option<&HitTestResult>, doc: &Document) -> Vec<NodeId> {
        match hit {
            Some(h) => {
                let mut chain = vec![h.node];
                // Walk up through DOM ancestors
                for ancestor in doc.ancestors(h.node) {
                    chain.push(ancestor);
                }
                chain
            }
            None => Vec::new(),
        }
    }

    fn update_focus(&mut self, new_focus: NodeId, doc: &mut Document, events: &mut Vec<DomEvent>) {
        if self.focus == Some(new_focus) {
            return;
        }

        // Blur old
        if let Some(old) = self.focus.take() {
            doc.set_pseudo_state(old, PseudoStateFlags::FOCUS, false);
            doc.set_pseudo_state(old, PseudoStateFlags::FOCUS_VISIBLE, false);
            events.push(DomEvent::new(old, DomEventKind::Blur));

            for ancestor in doc.ancestors(old) {
                doc.set_pseudo_state(ancestor, PseudoStateFlags::FOCUS_WITHIN, false);
            }
        }

        // Focus new
        doc.set_pseudo_state(new_focus, PseudoStateFlags::FOCUS, true);
        events.push(DomEvent::new(new_focus, DomEventKind::Focus));

        for ancestor in doc.ancestors(new_focus) {
            doc.set_pseudo_state(ancestor, PseudoStateFlags::FOCUS_WITHIN, true);
        }

        self.focus = Some(new_focus);
    }

    fn fire_handlers(&self, events: &[DomEvent]) {
        for event in events {
            let event_disc = std::mem::discriminant(&event.kind);

            // Fire handlers on the target node.
            let mut stopped = false;
            for (node, filter, capture, handler) in &self.handlers {
                if *node != event.target {
                    continue;
                }
                if *capture {
                    // Capture-phase listeners are driven by `dispatch_events`,
                    // not the hover-chain bubble path.
                    continue;
                }
                if let Some(f) = filter {
                    if *f != event_disc {
                        continue;
                    }
                }
                let result = handler(event);
                match result {
                    Propagation::StopImmediate => {
                        stopped = true;
                        break;
                    }
                    Propagation::StopPropagation => {
                        stopped = true;
                        break;
                    }
                    Propagation::Continue | Propagation::PreventDefault => {}
                }
            }

            // Bubble up through ancestors in the hover chain.
            // Only bubble events that are defined as bubbling per W3C spec.
            if !stopped && event.bubbles {
                for &ancestor in &self.hover_chain {
                    if ancestor == event.target {
                        continue; // already handled above
                    }
                    let mut ancestor_stopped = false;
                    for (node, filter, capture, handler) in &self.handlers {
                        if *node != ancestor {
                            continue;
                        }
                        if *capture {
                            continue;
                        }
                        if let Some(f) = filter {
                            if *f != event_disc {
                                continue;
                            }
                        }
                        let mut bubbled = DomEvent::new(event.target, event.kind.clone());
                        bubbled.current_target = ancestor;
                        let result = handler(&bubbled);
                        match result {
                            Propagation::StopImmediate | Propagation::StopPropagation => {
                                ancestor_stopped = true;
                                break;
                            }
                            Propagation::Continue | Propagation::PreventDefault => {}
                        }
                    }
                    if ancestor_stopped {
                        break;
                    }
                }
            }
        }
    }

    /// Dispatch a batch of pre-built [`DomEvent`]s using full W3C three-phase
    /// propagation (capture → target → bubble).
    ///
    /// Each event must carry its ancestor `event_path` (root-first, excluding the
    /// target). Phase order, per the W3C UI Events spec:
    ///
    /// 1. **Capture** — walk `event_path` front-to-back (root → parent); fire only
    ///    `capture == true` listeners; `phase = Capturing`.
    /// 2. **At target** — fire ALL listeners on the target (capture *and* bubble)
    ///    in registration order; `phase = AtTarget`.
    /// 3. **Bubble** — walk `event_path` back-to-front (parent → root); fire only
    ///    `capture == false` listeners; `phase = Bubbling`. Skipped entirely when
    ///    the event does not bubble (`event.bubbles == false`).
    ///
    /// Propagation control:
    /// - [`Propagation::StopPropagation`] — finishes the listeners on the current
    ///   node, then stops every later node and phase.
    /// - [`Propagation::StopImmediate`] — stops immediately, including any
    ///   remaining listeners on the current node.
    ///
    /// This is the capability the shell uses for modal / overlay event capture
    /// (a capturing root listener can swallow a descendant click) and for
    /// `preventDefault`. It is independent of the hover-chain
    /// [`fire_handlers`](Self::fire_handlers) path used by `dispatch_mouse_*`.
    pub fn dispatch_events(&self, events: &[DomEvent]) {
        for event in events {
            let event_disc = std::mem::discriminant(&event.kind);

            // ── 1. Capture phase: root → parent ──────────────────────────────
            let mut stopped = false;
            for &ancestor in &event.event_path {
                if self.fire_phase(event, event_disc, ancestor, true, EventPhase::Capturing) {
                    stopped = true;
                    break;
                }
            }
            if stopped {
                continue;
            }

            // ── 2. At-target phase: both capture & bubble listeners ──────────
            if self.fire_phase(event, event_disc, event.target, false, EventPhase::AtTarget) {
                continue;
            }

            // ── 3. Bubble phase: parent → root (bubbling events only) ────────
            if event.bubbles {
                for &ancestor in event.event_path.iter().rev() {
                    if self.fire_phase(event, event_disc, ancestor, false, EventPhase::Bubbling) {
                        break;
                    }
                }
            }
        }
    }

    /// Fire the listeners registered on `node` that match the current phase.
    ///
    /// `at_target == true` selects the at-target phase, where BOTH capture and
    /// bubble listeners on the node fire (in registration order); otherwise only
    /// listeners whose `capture` flag is `want_capture` fire.
    ///
    /// Returns `true` if propagation should stop for all subsequent nodes/phases
    /// (i.e. a handler returned `StopPropagation` or `StopImmediate`).
    fn fire_phase(
        &self,
        event: &DomEvent,
        event_disc: std::mem::Discriminant<DomEventKind>,
        node: NodeId,
        want_capture: bool,
        phase: EventPhase,
    ) -> bool {
        let at_target = matches!(phase, EventPhase::AtTarget);
        let mut stop_all = false;

        for (handler_node, filter, capture, handler) in &self.handlers {
            if *handler_node != node {
                continue;
            }
            // At the target node, both capture and bubble listeners fire.
            // Elsewhere, only listeners whose phase matches.
            if !at_target && *capture != want_capture {
                continue;
            }
            if let Some(f) = filter {
                if *f != event_disc {
                    continue;
                }
            }

            // Per-handler view of the event with the correct phase/current_target.
            let mut scoped = event.clone();
            scoped.current_target = node;
            scoped.phase = phase;

            // A panicking handler must not freeze the whole event pipeline:
            // isolate it, treat the panic as "Continue", and keep dispatching.
            let result = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                handler(&scoped)
            })) {
                Ok(r) => r,
                Err(_) => Propagation::Continue,
            };

            match result {
                Propagation::StopImmediate => {
                    // Stop this node AND all subsequent nodes/phases.
                    return true;
                }
                Propagation::StopPropagation => {
                    // Finish remaining listeners on THIS node, then stop later
                    // nodes/phases.
                    stop_all = true;
                }
                Propagation::Continue | Propagation::PreventDefault => {}
            }
        }

        stop_all
    }
}

impl Default for EventDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hover_chain_management() {
        let dispatcher = EventDispatcher::new();
        assert!(dispatcher.hover_chain().is_empty());
        assert!(dispatcher.focus().is_none());
    }
}
