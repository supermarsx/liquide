use serde::{Deserialize, Serialize};

use crate::node::{NodeId, State};

/// An accessibility event — notifications about tree changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AccessibilityEvent {
    NodeAdded { id: NodeId, parent: NodeId },
    NodeRemoved { id: NodeId },
    StateChanged { id: NodeId, state: State, value: bool },
    NameChanged { id: NodeId, old: String, new_name: String },
    ValueChanged { id: NodeId, old: Option<String>, new_value: Option<String> },
    FocusChanged { old: Option<NodeId>, new_focus: Option<NodeId> },
    TextChanged { id: NodeId, offset: usize, inserted: String, deleted: String },
    CaretMoved { id: NodeId, offset: usize },
    SelectionChanged { id: NodeId },
    TreeUpdated,
}

/// Fixed-capacity event queue for accessibility events.
#[derive(Debug, Clone)]
pub struct EventQueue {
    events: Vec<AccessibilityEvent>,
    max_size: usize,
}

impl EventQueue {
    #[must_use]
    pub fn new(max_size: usize) -> Self {
        Self {
            events: Vec::new(),
            max_size,
        }
    }

    /// Push an event into the queue. If full, the oldest event is dropped.
    pub fn push(&mut self, event: AccessibilityEvent) {
        if self.events.len() >= self.max_size {
            self.events.remove(0);
        }
        self.events.push(event);
    }

    /// Drain all events from the queue.
    pub fn drain(&mut self) -> Vec<AccessibilityEvent> {
        std::mem::take(&mut self.events)
    }

    /// Number of events in the queue.
    #[must_use]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Whether the queue is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Clear all events.
    pub fn clear(&mut self) {
        self.events.clear();
    }
}
