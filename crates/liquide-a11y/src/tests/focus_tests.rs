use crate::focus::FocusManager;
use crate::node::{AccessibleNode, Role};
use crate::tree::AccessibilityTree;

fn make_tree_with_buttons() -> AccessibilityTree {
    let mut tree = AccessibilityTree::new();
    tree.set_root(AccessibleNode::new(1, Role::Window, "Main"));
    tree.add_node(1, AccessibleNode::new(2, Role::Button, "A"))
        .unwrap();
    tree.add_node(1, AccessibleNode::new(3, Role::Button, "B"))
        .unwrap();
    tree.add_node(1, AccessibleNode::new(4, Role::Button, "C"))
        .unwrap();
    tree
}

#[test]
fn test_new_focus() {
    let fm = FocusManager::new();
    assert!(fm.focused().is_none());
    assert!(!fm.is_focus_ring_visible());
}

#[test]
fn test_set_get_clear() {
    let mut fm = FocusManager::new();
    fm.set_focus(42);
    assert_eq!(fm.focused(), Some(42));
    fm.clear_focus();
    assert!(fm.focused().is_none());
}

#[test]
fn test_tab_order_build() {
    let tree = make_tree_with_buttons();
    let mut fm = FocusManager::new();
    fm.build_tab_order(&tree);
    assert_eq!(fm.tab_order().len(), 3);
}

#[test]
fn test_focus_next() {
    let tree = make_tree_with_buttons();
    let mut fm = FocusManager::new();
    fm.build_tab_order(&tree);
    let id1 = fm.focus_next().unwrap();
    let id2 = fm.focus_next().unwrap();
    assert_ne!(id1, id2);
}

#[test]
fn test_focus_previous_cycle() {
    let tree = make_tree_with_buttons();
    let mut fm = FocusManager::new();
    fm.build_tab_order(&tree);
    // Move to first (wraps around from start)
    let id = fm.focus_previous().unwrap();
    assert!(id > 0);
}

#[test]
fn test_focus_ring_visible() {
    let mut fm = FocusManager::new();
    assert!(!fm.is_focus_ring_visible());
    fm.show_focus_ring();
    assert!(fm.is_focus_ring_visible());
    fm.hide_focus_ring();
    assert!(!fm.is_focus_ring_visible());
}

#[test]
fn test_empty_tree() {
    let tree = AccessibilityTree::new();
    let mut fm = FocusManager::new();
    fm.build_tab_order(&tree);
    assert!(fm.tab_order().is_empty());
    assert!(fm.focus_next().is_none());
    assert!(fm.focus_previous().is_none());
}

#[test]
fn test_focus_next_cycle() {
    let tree = make_tree_with_buttons();
    let mut fm = FocusManager::new();
    fm.build_tab_order(&tree);
    // Cycle through all 3 and wrap back
    let first = fm.focus_next().unwrap();
    let _ = fm.focus_next();
    let _ = fm.focus_next();
    let wrapped = fm.focus_next().unwrap();
    assert_eq!(first, wrapped);
}
