use crate::node::{AccessibleNode, Role};
use crate::tree::AccessibilityTree;

#[test]
fn test_new_tree() {
    let tree = AccessibilityTree::new();
    assert_eq!(tree.node_count(), 0);
    assert!(tree.root().is_none());
}

#[test]
fn test_add_root() {
    let mut tree = AccessibilityTree::new();
    let root = AccessibleNode::new(1, Role::Window, "Main");
    tree.set_root(root);
    assert_eq!(tree.root(), Some(1));
    assert_eq!(tree.node_count(), 1);
}

#[test]
fn test_add_children() {
    let mut tree = AccessibilityTree::new();
    tree.set_root(AccessibleNode::new(1, Role::Window, "Main"));
    tree.add_node(1, AccessibleNode::new(2, Role::Panel, "Panel"))
        .unwrap();
    tree.add_node(1, AccessibleNode::new(3, Role::Button, "OK"))
        .unwrap();
    assert_eq!(tree.node_count(), 3);
    assert_eq!(tree.children(1).len(), 2);
}

#[test]
fn test_remove_node() {
    let mut tree = AccessibilityTree::new();
    tree.set_root(AccessibleNode::new(1, Role::Window, "Main"));
    tree.add_node(1, AccessibleNode::new(2, Role::Panel, "Panel"))
        .unwrap();
    tree.add_node(2, AccessibleNode::new(3, Role::Button, "OK"))
        .unwrap();
    tree.remove_node(2).unwrap();
    assert_eq!(tree.node_count(), 1);
    assert!(tree.get(2).is_none());
    assert!(tree.get(3).is_none());
}

#[test]
fn test_walk() {
    let mut tree = AccessibilityTree::new();
    tree.set_root(AccessibleNode::new(1, Role::Window, "Main"));
    tree.add_node(1, AccessibleNode::new(2, Role::Button, "A"))
        .unwrap();
    tree.add_node(1, AccessibleNode::new(3, Role::Button, "B"))
        .unwrap();
    let mut visited = Vec::new();
    tree.walk(|node| visited.push(node.id));
    assert_eq!(visited.len(), 3);
    assert_eq!(visited[0], 1);
}

#[test]
fn test_find_by_role() {
    let mut tree = AccessibilityTree::new();
    tree.set_root(AccessibleNode::new(1, Role::Window, "Main"));
    tree.add_node(1, AccessibleNode::new(2, Role::Button, "A"))
        .unwrap();
    tree.add_node(1, AccessibleNode::new(3, Role::Button, "B"))
        .unwrap();
    tree.add_node(1, AccessibleNode::new(4, Role::Label, "Info"))
        .unwrap();
    let buttons = tree.find_by_role(Role::Button);
    assert_eq!(buttons.len(), 2);
}

#[test]
fn test_find_by_name() {
    let mut tree = AccessibilityTree::new();
    tree.set_root(AccessibleNode::new(1, Role::Window, "Main"));
    tree.add_node(1, AccessibleNode::new(2, Role::Button, "OK"))
        .unwrap();
    tree.add_node(1, AccessibleNode::new(3, Role::Button, "Cancel"))
        .unwrap();
    let results = tree.find_by_name("OK");
    assert_eq!(results.len(), 1);
}

#[test]
fn test_node_count() {
    let mut tree = AccessibilityTree::new();
    assert_eq!(tree.node_count(), 0);
    tree.set_root(AccessibleNode::new(1, Role::Window, "Main"));
    assert_eq!(tree.node_count(), 1);
}

#[test]
fn test_parent_lookup() {
    let mut tree = AccessibilityTree::new();
    tree.set_root(AccessibleNode::new(1, Role::Window, "Main"));
    tree.add_node(1, AccessibleNode::new(2, Role::Button, "OK"))
        .unwrap();
    assert_eq!(tree.parent(2), Some(1));
    assert_eq!(tree.parent(1), None);
}

#[test]
fn test_allocate_id() {
    let mut tree = AccessibilityTree::new();
    let id1 = tree.allocate_id();
    let id2 = tree.allocate_id();
    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
    assert_ne!(id1, id2);
}
