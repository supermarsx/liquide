use crate::focus::FocusManager;
use crate::navigation::*;
use crate::node::{AccessibleNode, Role};
use crate::tree::AccessibilityTree;

fn setup() -> (AccessibilityTree, FocusManager, KeyboardNavigation) {
    let mut tree = AccessibilityTree::new();
    tree.set_root(AccessibleNode::new(1, Role::Window, "Main"));
    tree.add_node(1, AccessibleNode::new(2, Role::Button, "A"))
        .unwrap();
    tree.add_node(1, AccessibleNode::new(3, Role::Button, "B"))
        .unwrap();
    tree.add_node(1, AccessibleNode::new(4, Role::Button, "C"))
        .unwrap();
    let mut fm = FocusManager::new();
    fm.build_tab_order(&tree);
    let nav = KeyboardNavigation::new();
    (tree, fm, nav)
}

#[test]
fn test_tab_forward() {
    let (tree, mut fm, mut nav) = setup();
    let result = nav.handle_action(NavigationAction::TabForward, &tree, &mut fm);
    assert!(matches!(result, NavigationResult::FocusMoved(_)));
}

#[test]
fn test_tab_backward() {
    let (tree, mut fm, mut nav) = setup();
    let _ = fm.focus_next(); // move to first
    let result = nav.handle_action(NavigationAction::TabBackward, &tree, &mut fm);
    assert!(matches!(result, NavigationResult::FocusMoved(_)));
}

#[test]
fn test_arrow_keys() {
    let (tree, mut fm, mut nav) = setup();
    let r1 = nav.handle_action(NavigationAction::ArrowDown, &tree, &mut fm);
    assert!(matches!(r1, NavigationResult::FocusMoved(_)));
    let r2 = nav.handle_action(NavigationAction::ArrowUp, &tree, &mut fm);
    assert!(matches!(r2, NavigationResult::FocusMoved(_)));
}

#[test]
fn test_activate() {
    let (tree, mut fm, mut nav) = setup();
    fm.set_focus(2);
    let result = nav.handle_action(NavigationAction::Activate, &tree, &mut fm);
    assert_eq!(result, NavigationResult::Activated(2));
}

#[test]
fn test_escape() {
    let (tree, mut fm, mut nav) = setup();
    fm.set_focus(2);
    let result = nav.handle_action(NavigationAction::Escape, &tree, &mut fm);
    assert_eq!(result, NavigationResult::Escaped);
    assert!(fm.focused().is_none());
}

#[test]
fn test_region_cycling() {
    let (tree, mut fm, mut nav) = setup();
    nav.set_regions(vec![10, 20, 30]);
    let r1 = nav.handle_action(NavigationAction::RegionNext, &tree, &mut fm);
    assert!(matches!(r1, NavigationResult::FocusMoved(20)));
    let r2 = nav.handle_action(NavigationAction::RegionNext, &tree, &mut fm);
    assert!(matches!(r2, NavigationResult::FocusMoved(30)));
}

#[test]
fn test_no_focusable_nodes() {
    let mut tree = AccessibilityTree::new();
    tree.set_root(AccessibleNode::new(1, Role::Window, "Main"));
    tree.add_node(1, AccessibleNode::new(2, Role::Label, "Info"))
        .unwrap();
    let mut fm = FocusManager::new();
    fm.build_tab_order(&tree);
    let mut nav = KeyboardNavigation::new();
    let result = nav.handle_action(NavigationAction::TabForward, &tree, &mut fm);
    assert_eq!(result, NavigationResult::NoChange);
}

#[test]
fn test_navigation_result_types() {
    assert_eq!(
        NavigationResult::FocusMoved(1),
        NavigationResult::FocusMoved(1)
    );
    assert_ne!(NavigationResult::FocusMoved(1), NavigationResult::Escaped);
    assert_eq!(NavigationResult::NoChange, NavigationResult::NoChange);
}
