//! Accessibility event system for the bridge layer.
//!
//! Provides a typed event model aligned with WAI-ARIA / AT-SPI event
//! categories, plus a batching queue that accumulates events within a
//! single frame and delivers them to assistive technology in one pass.

use crate::tree::NodeId;

// ---------------------------------------------------------------------------
// Event types
// ---------------------------------------------------------------------------

/// An accessibility event produced by the bridge.
#[derive(Debug, Clone, PartialEq)]
pub enum A11yEvent {
    /// Focus moved to a new node.
    FocusChanged {
        old: Option<NodeId>,
        new: Option<NodeId>,
    },
    /// Selection changed within a container (list, tree, table).
    SelectionChanged {
        container_id: NodeId,
        selected_ids: Vec<NodeId>,
    },
    /// A node's value changed (e.g. slider, text input).
    ValueChanged {
        node_id: NodeId,
        old_value: String,
        new_value: String,
    },
    /// Editable text content changed.
    TextChanged {
        node_id: NodeId,
        offset: usize,
        inserted: String,
        deleted: String,
    },
    /// One or more state flags on a node changed.
    StateChanged {
        node_id: NodeId,
        state_name: String,
        value: bool,
    },
    /// The children of a container were added / removed / reordered.
    ChildrenChanged {
        parent_id: NodeId,
    },
    /// A node's bounding rectangle changed.
    BoundsChanged {
        node_id: NodeId,
    },
    /// The active descendant of a composite widget changed.
    ActiveDescendantChanged {
        container_id: NodeId,
        descendant_id: NodeId,
    },
    /// A document (or application) finished loading.
    DocumentLoadComplete,
}

// ---------------------------------------------------------------------------
// Event target
// ---------------------------------------------------------------------------

/// An event together with its originating node.
#[derive(Debug, Clone)]
pub struct A11yEventTarget {
    pub node_id: NodeId,
    pub event: A11yEvent,
}

impl A11yEventTarget {
    #[must_use]
    pub fn new(node_id: NodeId, event: A11yEvent) -> Self {
        Self { node_id, event }
    }
}

// ---------------------------------------------------------------------------
// Event queue
// ---------------------------------------------------------------------------

/// Fixed-capacity event queue for batching accessibility events within a
/// single frame.  When the capacity is exceeded the oldest events are
/// silently dropped (assistive technology can always resync from the tree).
#[derive(Debug, Clone)]
pub struct A11yEventQueue {
    events: Vec<A11yEventTarget>,
    capacity: usize,
}

impl A11yEventQueue {
    /// Create a new queue with the given maximum capacity.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            events: Vec::with_capacity(capacity.min(256)),
            capacity,
        }
    }

    /// Push an event.  If the queue is full the oldest event is dropped.
    pub fn push(&mut self, target: A11yEventTarget) {
        if self.events.len() >= self.capacity {
            self.events.remove(0);
        }
        self.events.push(target);
    }

    /// Convenience: push an event for a given node.
    pub fn push_event(&mut self, node_id: NodeId, event: A11yEvent) {
        self.push(A11yEventTarget::new(node_id, event));
    }

    /// Drain all events, returning them as a `Vec` and leaving the queue
    /// empty.
    pub fn drain(&mut self) -> Vec<A11yEventTarget> {
        std::mem::take(&mut self.events)
    }

    /// Number of pending events.
    #[must_use]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Whether the queue is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Clear all pending events without returning them.
    pub fn clear(&mut self) {
        self.events.clear();
    }

    /// Peek at the most recently pushed event.
    #[must_use]
    pub fn last(&self) -> Option<&A11yEventTarget> {
        self.events.last()
    }

    /// Maximum capacity.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_and_drain() {
        let mut q = A11yEventQueue::new(16);
        q.push_event(1, A11yEvent::FocusChanged { old: None, new: Some(1) });
        q.push_event(2, A11yEvent::DocumentLoadComplete);
        assert_eq!(q.len(), 2);

        let events = q.drain();
        assert_eq!(events.len(), 2);
        assert!(q.is_empty());
    }

    #[test]
    fn overflow_drops_oldest() {
        let mut q = A11yEventQueue::new(2);
        q.push_event(1, A11yEvent::DocumentLoadComplete);
        q.push_event(2, A11yEvent::DocumentLoadComplete);
        q.push_event(3, A11yEvent::DocumentLoadComplete);
        assert_eq!(q.len(), 2);
        // The first event (node_id=1) was dropped.
        let events = q.drain();
        assert_eq!(events[0].node_id, 2);
        assert_eq!(events[1].node_id, 3);
    }

    #[test]
    fn clear() {
        let mut q = A11yEventQueue::new(8);
        q.push_event(1, A11yEvent::DocumentLoadComplete);
        q.clear();
        assert!(q.is_empty());
    }

    #[test]
    fn last_returns_most_recent() {
        let mut q = A11yEventQueue::new(8);
        assert!(q.last().is_none());
        q.push_event(1, A11yEvent::DocumentLoadComplete);
        q.push_event(2, A11yEvent::BoundsChanged { node_id: 2 });
        assert_eq!(q.last().unwrap().node_id, 2);
    }

    #[test]
    fn focus_changed_event() {
        let evt = A11yEvent::FocusChanged { old: Some(1), new: Some(2) };
        let target = A11yEventTarget::new(2, evt.clone());
        assert_eq!(target.node_id, 2);
        assert_eq!(target.event, evt);
    }

    #[test]
    fn selection_changed_event() {
        let evt = A11yEvent::SelectionChanged {
            container_id: 10,
            selected_ids: vec![11, 12],
        };
        if let A11yEvent::SelectionChanged { container_id, selected_ids } = &evt {
            assert_eq!(*container_id, 10);
            assert_eq!(selected_ids.len(), 2);
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn value_changed_event() {
        let evt = A11yEvent::ValueChanged {
            node_id: 5,
            old_value: "10".into(),
            new_value: "20".into(),
        };
        if let A11yEvent::ValueChanged { node_id, old_value, new_value } = &evt {
            assert_eq!(*node_id, 5);
            assert_eq!(old_value, "10");
            assert_eq!(new_value, "20");
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn text_changed_event() {
        let evt = A11yEvent::TextChanged {
            node_id: 3,
            offset: 5,
            inserted: "abc".into(),
            deleted: String::new(),
        };
        if let A11yEvent::TextChanged { offset, inserted, .. } = &evt {
            assert_eq!(*offset, 5);
            assert_eq!(inserted, "abc");
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn state_changed_event() {
        let evt = A11yEvent::StateChanged {
            node_id: 7,
            state_name: "checked".into(),
            value: true,
        };
        if let A11yEvent::StateChanged { state_name, value, .. } = &evt {
            assert_eq!(state_name, "checked");
            assert!(*value);
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn children_changed_event() {
        let evt = A11yEvent::ChildrenChanged { parent_id: 1 };
        assert_eq!(evt, A11yEvent::ChildrenChanged { parent_id: 1 });
    }

    #[test]
    fn active_descendant_event() {
        let evt = A11yEvent::ActiveDescendantChanged {
            container_id: 10,
            descendant_id: 15,
        };
        if let A11yEvent::ActiveDescendantChanged { container_id, descendant_id } = &evt {
            assert_eq!(*container_id, 10);
            assert_eq!(*descendant_id, 15);
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn capacity() {
        let q = A11yEventQueue::new(64);
        assert_eq!(q.capacity(), 64);
    }
}
