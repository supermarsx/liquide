use crate::event::{AccessibilityEvent, EventQueue};
use crate::focus::FocusManager;
use crate::node::{AccessibleNode, Role};
use crate::tree::AccessibilityTree;

#[test]
fn test_empty_tree_operations() {
    let tree = AccessibilityTree::new();
    assert!(tree.root().is_none());
    assert_eq!(tree.node_count(), 0);
    assert!(tree.get(1).is_none());
    assert!(tree.children(1).is_empty());
    assert!(tree.parent(1).is_none());
    assert!(tree.find_by_role(Role::Button).is_empty());
    assert!(tree.find_by_name("anything").is_empty());
}

#[test]
fn test_remove_root() {
    let mut tree = AccessibilityTree::new();
    tree.set_root(AccessibleNode::new(1, Role::Window, "Main"));
    tree.remove_node(1).unwrap();
    assert!(tree.root().is_none());
    assert_eq!(tree.node_count(), 0);
}

#[test]
fn test_focus_on_removed_node() {
    let mut tree = AccessibilityTree::new();
    tree.set_root(AccessibleNode::new(1, Role::Window, "Main"));
    tree.add_node(1, AccessibleNode::new(2, Role::Button, "A")).unwrap();
    let mut fm = FocusManager::new();
    fm.build_tab_order(&tree);
    fm.set_focus(2);
    assert_eq!(fm.focused(), Some(2));
    tree.remove_node(2).unwrap();
    // Focus manager still holds the old ID — caller must sync
    assert_eq!(fm.focused(), Some(2));
}

#[test]
fn test_event_queue_serde_roundtrip() {
    let mut q = EventQueue::new(10);
    q.push(AccessibilityEvent::TreeUpdated);
    q.push(AccessibilityEvent::NodeAdded { id: 5, parent: 1 });
    let events = q.drain();
    let json = serde_json::to_string(&events).unwrap();
    let d: Vec<AccessibilityEvent> = serde_json::from_str(&json).unwrap();
    assert_eq!(d.len(), 2);
}
