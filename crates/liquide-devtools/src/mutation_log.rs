//! Mutation log — records DOM changes for the devtools "Changes" panel.
//!
//! Implements the `MutationObserver` trait from `liquide-dom` to capture
//! every DOM mutation into a bounded ring buffer with timestamps.

use std::collections::VecDeque;
use std::time::Instant;

use liquide_dom::class_list::ClassList;
use liquide_dom::pseudo::PseudoStateFlags;
use liquide_dom::visitor::MutationObserver;
use liquide_dom::NodeId;
use serde::{Deserialize, Serialize};

/// Maximum number of records kept in the log before eviction.
const DEFAULT_CAPACITY: usize = 2048;

/// A single recorded DOM mutation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationRecord {
    /// Monotonic timestamp (milliseconds since log creation).
    pub timestamp_ms: u64,
    /// The kind of mutation that occurred.
    pub kind: MutationKind,
}

/// The specific mutation that was observed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MutationKind {
    /// A child was appended to a parent.
    ChildAdded {
        parent: NodeId,
        child: NodeId,
    },
    /// A child was removed from a parent.
    ChildRemoved {
        parent: NodeId,
        child: NodeId,
    },
    /// An attribute was changed on a node.
    AttributeChanged {
        node: NodeId,
        attribute: String,
        old_value: Option<String>,
        new_value: Option<String>,
    },
    /// The class list was changed on a node.
    ClassChanged {
        node: NodeId,
        classes: Vec<String>,
    },
    /// Text content was changed on a node.
    TextChanged {
        node: NodeId,
        text: String,
    },
    /// A pseudo-state was changed on a node.
    PseudoStateChanged {
        node: NodeId,
        old_flags: u32,
        new_flags: u32,
    },
    /// The element ID was changed.
    IdChanged {
        node: NodeId,
        old_id: Option<String>,
        new_id: Option<String>,
    },
}

/// Ring-buffer mutation log that implements `MutationObserver`.
pub struct MutationLog {
    /// The recorded mutations (oldest first).
    records: VecDeque<MutationRecord>,
    /// Maximum capacity before eviction.
    capacity: usize,
    /// Base instant for timestamp calculation.
    start: Instant,
    /// Whether recording is paused.
    paused: bool,
    /// Total mutations observed (including evicted ones).
    total_count: u64,
}

impl MutationLog {
    /// Create a new mutation log with default capacity.
    pub fn new() -> Self {
        Self {
            records: VecDeque::with_capacity(DEFAULT_CAPACITY),
            capacity: DEFAULT_CAPACITY,
            start: Instant::now(),
            paused: false,
            total_count: 0,
        }
    }

    /// Create a new mutation log with a specific capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            records: VecDeque::with_capacity(capacity),
            capacity,
            start: Instant::now(),
            paused: false,
            total_count: 0,
        }
    }

    /// Pause recording (mutations will be silently dropped).
    pub fn pause(&mut self) {
        self.paused = true;
    }

    /// Resume recording.
    pub fn resume(&mut self) {
        self.paused = false;
    }

    /// Whether recording is paused.
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Clear all recorded mutations.
    pub fn clear(&mut self) {
        self.records.clear();
    }

    /// Get the total number of mutations observed (including evicted).
    pub fn total_count(&self) -> u64 {
        self.total_count
    }

    /// Get the number of mutations currently in the buffer.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Iterate over all records (oldest first).
    pub fn iter(&self) -> impl Iterator<Item = &MutationRecord> {
        self.records.iter()
    }

    /// Get the most recent N records.
    pub fn recent(&self, n: usize) -> impl Iterator<Item = &MutationRecord> {
        let skip = self.records.len().saturating_sub(n);
        self.records.iter().skip(skip)
    }

    /// Get records that match a specific node ID.
    pub fn for_node(&self, node_id: NodeId) -> Vec<&MutationRecord> {
        self.records
            .iter()
            .filter(|r| record_involves_node(&r.kind, node_id))
            .collect()
    }

    /// Export all records to JSON.
    pub fn to_json(&self) -> String {
        let records: Vec<&MutationRecord> = self.records.iter().collect();
        serde_json::to_string_pretty(&records)
            .unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}"))
    }

    /// Push a new record into the log.
    fn push(&mut self, kind: MutationKind) {
        if self.paused {
            return;
        }

        self.total_count += 1;

        let timestamp_ms = self.start.elapsed().as_millis() as u64;

        if self.records.len() >= self.capacity {
            self.records.pop_front();
        }

        self.records.push_back(MutationRecord {
            timestamp_ms,
            kind,
        });
    }
}

impl Default for MutationLog {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a mutation record involves a specific node.
fn record_involves_node(kind: &MutationKind, node_id: NodeId) -> bool {
    match kind {
        MutationKind::ChildAdded { parent, child } => *parent == node_id || *child == node_id,
        MutationKind::ChildRemoved { parent, child } => *parent == node_id || *child == node_id,
        MutationKind::AttributeChanged { node, .. } => *node == node_id,
        MutationKind::ClassChanged { node, .. } => *node == node_id,
        MutationKind::TextChanged { node, .. } => *node == node_id,
        MutationKind::PseudoStateChanged { node, .. } => *node == node_id,
        MutationKind::IdChanged { node, .. } => *node == node_id,
    }
}

// Implement the `MutationObserver` trait from liquide-dom.
impl MutationObserver for MutationLog {
    fn on_child_added(&mut self, parent: NodeId, child: NodeId) {
        self.push(MutationKind::ChildAdded { parent, child });
    }

    fn on_child_removed(&mut self, parent: NodeId, child: NodeId) {
        self.push(MutationKind::ChildRemoved { parent, child });
    }

    fn on_attribute_changed(
        &mut self,
        node: NodeId,
        attr: &str,
        old_value: Option<&str>,
        new_value: Option<&str>,
    ) {
        self.push(MutationKind::AttributeChanged {
            node,
            attribute: attr.to_string(),
            old_value: old_value.map(|s| s.to_string()),
            new_value: new_value.map(|s| s.to_string()),
        });
    }

    fn on_class_changed(&mut self, node: NodeId, classes: &ClassList) {
        self.push(MutationKind::ClassChanged {
            node,
            classes: classes.iter().map(|s| s.to_string()).collect(),
        });
    }

    fn on_text_changed(&mut self, node: NodeId, text: &str) {
        self.push(MutationKind::TextChanged {
            node,
            text: text.to_string(),
        });
    }

    fn on_pseudo_state_changed(
        &mut self,
        node: NodeId,
        old_state: PseudoStateFlags,
        new_state: PseudoStateFlags,
    ) {
        self.push(MutationKind::PseudoStateChanged {
            node,
            old_flags: old_state.bits(),
            new_flags: new_state.bits(),
        });
    }

    fn on_id_changed(&mut self, node: NodeId, old_id: Option<&str>, new_id: Option<&str>) {
        self.push(MutationKind::IdChanged {
            node,
            old_id: old_id.map(|s| s.to_string()),
            new_id: new_id.map(|s| s.to_string()),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_recording() {
        let mut log = MutationLog::new();
        log.on_child_added(1, 2);
        log.on_child_added(1, 3);
        assert_eq!(log.len(), 2);
        assert_eq!(log.total_count(), 2);
    }

    #[test]
    fn test_capacity_eviction() {
        let mut log = MutationLog::with_capacity(3);
        for i in 0..5 {
            log.on_child_added(1, i + 10);
        }
        assert_eq!(log.len(), 3);
        assert_eq!(log.total_count(), 5);

        // Oldest records should have been evicted; newest remain.
        let records: Vec<_> = log.iter().collect();
        match &records[0].kind {
            MutationKind::ChildAdded { child, .. } => assert_eq!(*child, 12),
            _ => panic!("wrong kind"),
        }
    }

    #[test]
    fn test_pause_resume() {
        let mut log = MutationLog::new();
        log.on_child_added(1, 2);
        assert_eq!(log.len(), 1);

        log.pause();
        log.on_child_added(1, 3);
        assert_eq!(log.len(), 1); // nothing added while paused

        log.resume();
        log.on_child_added(1, 4);
        assert_eq!(log.len(), 2);
    }

    #[test]
    fn test_for_node() {
        let mut log = MutationLog::new();
        log.on_child_added(1, 2);
        log.on_child_added(3, 4);
        log.on_attribute_changed(2, "class", None, Some("active"));

        let for_2 = log.for_node(2);
        assert_eq!(for_2.len(), 2); // child_added(1,2) + attr_changed(2,...)
    }

    #[test]
    fn test_clear() {
        let mut log = MutationLog::new();
        log.on_child_added(1, 2);
        log.on_child_added(1, 3);
        log.clear();
        assert!(log.is_empty());
        assert_eq!(log.total_count(), 2); // total still tracked
    }

    #[test]
    fn test_all_mutation_kinds_via_observer() {
        use liquide_dom::class_list::ClassList;
        use liquide_dom::pseudo::PseudoStateFlags;

        let mut log = MutationLog::new();

        log.on_child_added(1, 2);
        log.on_child_removed(1, 2);
        log.on_attribute_changed(3, "href", None, Some("https://example.com"));
        log.on_class_changed(3, &ClassList::from_class_string("active highlighted"));
        log.on_text_changed(4, "new text");
        log.on_pseudo_state_changed(5, PseudoStateFlags::empty(), PseudoStateFlags::HOVER);
        log.on_id_changed(6, None, Some("main-panel"));

        assert_eq!(log.len(), 7);
        assert_eq!(log.total_count(), 7);

        // Verify each kind was recorded correctly.
        let records: Vec<_> = log.iter().collect();
        assert!(matches!(&records[0].kind, MutationKind::ChildAdded { parent: 1, child: 2 }));
        assert!(matches!(&records[1].kind, MutationKind::ChildRemoved { parent: 1, child: 2 }));
        assert!(matches!(&records[2].kind, MutationKind::AttributeChanged { node: 3, .. }));
        assert!(matches!(&records[3].kind, MutationKind::ClassChanged { node: 3, .. }));
        assert!(matches!(&records[4].kind, MutationKind::TextChanged { node: 4, .. }));
        assert!(matches!(&records[5].kind, MutationKind::PseudoStateChanged { node: 5, .. }));
        assert!(matches!(&records[6].kind, MutationKind::IdChanged { node: 6, .. }));
    }

    #[test]
    fn test_recent() {
        let mut log = MutationLog::new();
        for i in 0..10 {
            log.on_child_added(1, i + 100);
        }
        let recent: Vec<_> = log.recent(3).collect();
        assert_eq!(recent.len(), 3);
        // Most recent should be the last added.
        match &recent[2].kind {
            MutationKind::ChildAdded { child, .. } => assert_eq!(*child, 109),
            _ => panic!("expected ChildAdded"),
        }
    }

    #[test]
    fn test_json_export() {
        let mut log = MutationLog::new();
        log.on_child_added(1, 2);
        let json = log.to_json();
        assert!(json.contains("ChildAdded"));
        assert!(json.contains("\"parent\": 1"));
    }
}
