//! DOM event dispatch — capturing, target, and bubbling phases.
//!
//! Implements the W3C DOM Events model:
//!
//! 1. **Capturing phase**: event travels from the root down to the target.
//! 2. **Target phase**: event fires on the target element.
//! 3. **Bubbling phase**: event travels from the target back up to the root.
//!
//! Event listeners can be registered for capture or bubble phase.  
//! `stopPropagation()` and `stopImmediatePropagation()` are supported.

use std::collections::HashMap;
use std::sync::Arc;

use crate::document::Document;
use crate::node::NodeId;

// ─── Event types ───────────────────────────────────────────────────

/// DOM event dispatch phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventPhase {
    /// Not yet dispatched.
    None,
    /// Traveling from root toward target.
    Capturing,
    /// Arrived at the target node.
    AtTarget,
    /// Traveling from target back toward root.
    Bubbling,
}

/// A DOM event that can be dispatched through the tree.
#[derive(Debug, Clone)]
pub struct Event {
    /// Event type name (e.g. "click", "keydown").
    pub event_type: String,
    /// The original target node.
    pub target: NodeId,
    /// The node currently processing the event.
    pub current_target: NodeId,
    /// Current phase of dispatch.
    pub phase: EventPhase,
    /// Whether the event bubbles.
    pub bubbles: bool,
    /// Whether `preventDefault()` was called.
    pub default_prevented: bool,
    /// Stop propagation after current node.
    propagation_stopped: bool,
    /// Stop even other listeners on the same node.
    immediate_propagation_stopped: bool,
}

impl Event {
    /// Create a new event.
    pub fn new(event_type: impl Into<String>, target: NodeId, bubbles: bool) -> Self {
        Self {
            event_type: event_type.into(),
            target,
            current_target: target,
            phase: EventPhase::None,
            bubbles,
            default_prevented: false,
            propagation_stopped: false,
            immediate_propagation_stopped: false,
        }
    }

    /// Prevent the default action associated with the event.
    pub fn prevent_default(&mut self) {
        self.default_prevented = true;
    }

    /// Stop event from propagating to further nodes.
    pub fn stop_propagation(&mut self) {
        self.propagation_stopped = true;
    }

    /// Stop event from reaching any further listeners, even on the same node.
    pub fn stop_immediate_propagation(&mut self) {
        self.immediate_propagation_stopped = true;
        self.propagation_stopped = true;
    }

    /// Whether propagation has been stopped.
    pub fn is_propagation_stopped(&self) -> bool {
        self.propagation_stopped
    }

    /// Whether immediate propagation has been stopped.
    pub fn is_immediate_propagation_stopped(&self) -> bool {
        self.immediate_propagation_stopped
    }
}

// ─── Listener registration ────────────────────────────────────────

/// Options for an event listener.
#[derive(Debug, Clone, Copy)]
pub struct ListenerOptions {
    /// If true, the listener fires during the capture phase.
    pub capture: bool,
    /// If true, the listener is removed after it fires once.
    pub once: bool,
}

impl Default for ListenerOptions {
    fn default() -> Self {
        Self {
            capture: false,
            once: false,
        }
    }
}

/// A registered event listener with its options.
#[derive(Clone)]
pub struct EventListener {
    /// Unique id for this listener (for removal).
    pub id: u64,
    /// Event type to listen for.
    pub event_type: String,
    /// Options.
    pub options: ListenerOptions,
    /// The callback function — supports closures with captured state.
    pub callback: Arc<dyn Fn(&mut Event) + Send + Sync>,
}

impl std::fmt::Debug for EventListener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventListener")
            .field("id", &self.id)
            .field("event_type", &self.event_type)
            .field("options", &self.options)
            .finish()
    }
}

// ─── Event target map ──────────────────────────────────────────────

/// Registry of event listeners for all nodes.
#[derive(Debug, Default)]
pub struct EventTargetMap {
    /// node_id -> list of listeners
    listeners: HashMap<NodeId, Vec<EventListener>>,
    /// Auto-incrementing listener ID.
    next_id: u64,
}

impl EventTargetMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an event listener on a node.
    pub fn add_listener(
        &mut self,
        node_id: NodeId,
        event_type: impl Into<String>,
        callback: impl Fn(&mut Event) + Send + Sync + 'static,
        options: ListenerOptions,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let listener = EventListener {
            id,
            event_type: event_type.into(),
            options,
            callback: Arc::new(callback),
        };
        self.listeners.entry(node_id).or_default().push(listener);
        id
    }

    /// Remove a specific listener by its ID.
    pub fn remove_listener(&mut self, node_id: NodeId, listener_id: u64) {
        if let Some(list) = self.listeners.get_mut(&node_id) {
            list.retain(|l| l.id != listener_id);
        }
    }

    /// Remove all listeners for a node.
    pub fn remove_all(&mut self, node_id: NodeId) {
        self.listeners.remove(&node_id);
    }

    /// Get listeners for a node, filtered by event type and phase.
    fn matching_listeners(
        &self,
        node_id: NodeId,
        event_type: &str,
        capture_phase: bool,
    ) -> Vec<EventListener> {
        self.listeners
            .get(&node_id)
            .map(|list| {
                list.iter()
                    .filter(|l| l.event_type == event_type && l.options.capture == capture_phase)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }
}

// ─── Event dispatcher ──────────────────────────────────────────────

/// Dispatch an event through the DOM tree following the W3C model.
///
/// Returns `true` if `preventDefault()` was called.
pub fn dispatch_event(doc: &Document, targets: &mut EventTargetMap, event: &mut Event) -> bool {
    let target = event.target;

    // Build the event path: root → ... → parent → target
    let mut path = Vec::new();
    {
        let mut current = target;
        loop {
            path.push(current);
            match doc.parent(current) {
                Some(parent) => current = parent,
                None => break,
            }
        }
    }
    path.reverse(); // root first, target last

    let target_index = path.len() - 1;

    // ── Capturing phase: root → target (exclusive) ──
    event.phase = EventPhase::Capturing;
    for i in 0..target_index {
        if event.is_propagation_stopped() {
            break;
        }
        event.current_target = path[i];
        fire_listeners(targets, path[i], event, true);
    }

    // ── Target phase ──
    if !event.is_propagation_stopped() {
        event.phase = EventPhase::AtTarget;
        event.current_target = target;
        // At target, both capture and bubble listeners fire.
        fire_listeners(targets, target, event, true);
        if !event.is_immediate_propagation_stopped() {
            fire_listeners(targets, target, event, false);
        }
    }

    // ── Bubbling phase: target (exclusive) → root ──
    if event.bubbles && !event.is_propagation_stopped() {
        event.phase = EventPhase::Bubbling;
        for i in (0..target_index).rev() {
            if event.is_propagation_stopped() {
                break;
            }
            event.current_target = path[i];
            fire_listeners(targets, path[i], event, false);
        }
    }

    event.default_prevented
}

/// Fire all matching listeners on a node.
fn fire_listeners(targets: &mut EventTargetMap, node_id: NodeId, event: &mut Event, capture: bool) {
    let listeners = targets.matching_listeners(node_id, &event.event_type, capture);
    let mut to_remove = Vec::new();

    for listener in &listeners {
        if event.is_immediate_propagation_stopped() {
            break;
        }
        // Catch panics from individual listeners to ensure:
        // 1. Remaining listeners still fire
        // 2. `once` listeners are still cleaned up
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            (listener.callback)(event);
        }));
        if let Err(_) = result {
            eprintln!(
                "event listener panicked during '{}' on node {:?}",
                event.event_type, node_id,
            );
        }
        if listener.options.once {
            to_remove.push(listener.id);
        }
    }

    // Remove `once` listeners.
    for id in to_remove {
        targets.remove_listener(node_id, id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Document;
    use std::sync::{
        Mutex,
        atomic::{AtomicU32, Ordering},
    };

    static CALL_COUNT: AtomicU32 = AtomicU32::new(0);
    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    fn reset_counter() {
        CALL_COUNT.store(0, Ordering::SeqCst);
    }

    fn increment_handler(_event: &mut Event) {
        CALL_COUNT.fetch_add(1, Ordering::SeqCst);
    }

    fn stop_handler(event: &mut Event) {
        CALL_COUNT.fetch_add(1, Ordering::SeqCst);
        event.stop_propagation();
    }

    fn prevent_handler(event: &mut Event) {
        event.prevent_default();
    }

    #[test]
    fn test_bubbling_order() {
        let _guard = TEST_MUTEX.lock().unwrap();
        reset_counter();
        let mut doc = Document::new();
        let parent = doc.create_element("div");
        doc.append_child(doc.root(), parent);
        let child = doc.create_element("span");
        doc.append_child(parent, child);

        let mut targets = EventTargetMap::new();
        targets.add_listener(
            parent,
            "click",
            increment_handler,
            ListenerOptions::default(),
        );
        targets.add_listener(
            child,
            "click",
            increment_handler,
            ListenerOptions::default(),
        );

        let mut event = Event::new("click", child, true);
        dispatch_event(&doc, &mut targets, &mut event);

        // child (target) + parent (bubbling) = 2
        assert_eq!(CALL_COUNT.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_capturing() {
        let _guard = TEST_MUTEX.lock().unwrap();
        reset_counter();
        let mut doc = Document::new();
        let parent = doc.create_element("div");
        doc.append_child(doc.root(), parent);
        let child = doc.create_element("span");
        doc.append_child(parent, child);

        let mut targets = EventTargetMap::new();
        targets.add_listener(
            parent,
            "click",
            increment_handler,
            ListenerOptions {
                capture: true,
                once: false,
            },
        );
        targets.add_listener(
            child,
            "click",
            increment_handler,
            ListenerOptions::default(),
        );

        let mut event = Event::new("click", child, true);
        dispatch_event(&doc, &mut targets, &mut event);

        // parent (capture) + child (target) = 2
        assert_eq!(CALL_COUNT.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_stop_propagation() {
        let _guard = TEST_MUTEX.lock().unwrap();
        reset_counter();
        let mut doc = Document::new();
        let grandparent = doc.create_element("section");
        doc.append_child(doc.root(), grandparent);
        let parent = doc.create_element("div");
        doc.append_child(grandparent, parent);
        let child = doc.create_element("span");
        doc.append_child(parent, child);

        let mut targets = EventTargetMap::new();
        targets.add_listener(
            grandparent,
            "click",
            increment_handler,
            ListenerOptions::default(),
        );
        targets.add_listener(parent, "click", stop_handler, ListenerOptions::default());
        targets.add_listener(
            child,
            "click",
            increment_handler,
            ListenerOptions::default(),
        );

        let mut event = Event::new("click", child, true);
        dispatch_event(&doc, &mut targets, &mut event);

        // child fires (target), parent fires + stops, grandparent does NOT fire = 2
        assert_eq!(CALL_COUNT.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_prevent_default() {
        let mut doc = Document::new();
        let node = doc.create_element("a");
        doc.append_child(doc.root(), node);

        let mut targets = EventTargetMap::new();
        targets.add_listener(node, "click", prevent_handler, ListenerOptions::default());

        let mut event = Event::new("click", node, true);
        let prevented = dispatch_event(&doc, &mut targets, &mut event);
        assert!(prevented);
    }

    #[test]
    fn test_no_bubbling() {
        let _guard = TEST_MUTEX.lock().unwrap();
        reset_counter();
        let mut doc = Document::new();
        let parent = doc.create_element("div");
        doc.append_child(doc.root(), parent);
        let child = doc.create_element("span");
        doc.append_child(parent, child);

        let mut targets = EventTargetMap::new();
        targets.add_listener(
            parent,
            "focus",
            increment_handler,
            ListenerOptions::default(),
        );
        targets.add_listener(
            child,
            "focus",
            increment_handler,
            ListenerOptions::default(),
        );

        // focus does NOT bubble
        let mut event = Event::new("focus", child, false);
        dispatch_event(&doc, &mut targets, &mut event);

        // Only child fires
        assert_eq!(CALL_COUNT.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_once_listener() {
        let _guard = TEST_MUTEX.lock().unwrap();
        reset_counter();
        let mut doc = Document::new();
        let node = doc.create_element("button");
        doc.append_child(doc.root(), node);

        let mut targets = EventTargetMap::new();
        targets.add_listener(
            node,
            "click",
            increment_handler,
            ListenerOptions {
                capture: false,
                once: true,
            },
        );

        let mut event = Event::new("click", node, true);
        dispatch_event(&doc, &mut targets, &mut event);
        assert_eq!(CALL_COUNT.load(Ordering::SeqCst), 1);

        // Second dispatch — listener was removed.
        let mut event2 = Event::new("click", node, true);
        dispatch_event(&doc, &mut targets, &mut event2);
        assert_eq!(CALL_COUNT.load(Ordering::SeqCst), 1); // Still 1.
    }
}
