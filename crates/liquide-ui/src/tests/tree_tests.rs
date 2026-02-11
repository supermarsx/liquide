//! Tests for the widget tree.

use crate::geometry::{Point, Rect};
use crate::tree::WidgetTree;
use crate::widget::WidgetId;

// ---------------------------------------------------------------------------
// Basic operations
// ---------------------------------------------------------------------------

#[test]
fn test_tree_new_is_empty() {
    let tree = WidgetTree::new();
    assert!(tree.is_empty());
    assert_eq!(tree.len(), 0);
    assert!(tree.root().is_none());
}

#[test]
fn test_tree_add_root() {
    let mut tree = WidgetTree::new();
    let root = tree.add_root(0);
    assert_eq!(tree.len(), 1);
    assert_eq!(tree.root(), Some(root));
}

#[test]
fn test_tree_add_child() {
    let mut tree = WidgetTree::new();
    let root = tree.add_root(0);
    let child = tree.add_child(root, 0);

    assert_eq!(tree.len(), 2);
    assert_eq!(tree.children_of(root), vec![child]);
}

#[test]
fn test_tree_add_multiple_children() {
    let mut tree = WidgetTree::new();
    let root = tree.add_root(0);
    let c1 = tree.add_child(root, 0);
    let c2 = tree.add_child(root, 0);
    let c3 = tree.add_child(root, 0);

    assert_eq!(tree.len(), 4);
    assert_eq!(tree.children_of(root), vec![c1, c2, c3]);
}

#[test]
fn test_tree_unique_ids() {
    let mut tree = WidgetTree::new();
    let r = tree.add_root(0);
    let c1 = tree.add_child(r, 0);
    let c2 = tree.add_child(r, 0);

    assert_ne!(r, c1);
    assert_ne!(r, c2);
    assert_ne!(c1, c2);
}

// ---------------------------------------------------------------------------
// Remove
// ---------------------------------------------------------------------------

#[test]
fn test_tree_remove_leaf() {
    let mut tree = WidgetTree::new();
    let root = tree.add_root(0);
    let child = tree.add_child(root, 0);

    tree.remove(child);
    assert_eq!(tree.len(), 1);
    assert!(tree.children_of(root).is_empty());
}

#[test]
fn test_tree_remove_with_descendants() {
    let mut tree = WidgetTree::new();
    let root = tree.add_root(0);
    let child = tree.add_child(root, 0);
    let _grandchild = tree.add_child(child, 0);

    tree.remove(child);
    assert_eq!(tree.len(), 1); // only root remains
    assert!(tree.children_of(root).is_empty());
}

#[test]
fn test_tree_remove_root() {
    let mut tree = WidgetTree::new();
    let root = tree.add_root(0);
    tree.add_child(root, 0);

    tree.remove(root);
    assert!(tree.root().is_none());
}

// ---------------------------------------------------------------------------
// Reparent
// ---------------------------------------------------------------------------

#[test]
fn test_tree_reparent() {
    let mut tree = WidgetTree::new();
    let root = tree.add_root(0);
    let a = tree.add_child(root, 0);
    let b = tree.add_child(root, 0);

    // Move b under a
    tree.reparent(b, a);
    assert_eq!(tree.children_of(root), vec![a]);
    assert_eq!(tree.children_of(a), vec![b]);
}

#[test]
fn test_tree_reparent_updates_parent() {
    let mut tree = WidgetTree::new();
    let root = tree.add_root(0);
    let a = tree.add_child(root, 0);
    let b = tree.add_child(root, 0);

    tree.reparent(b, a);
    let node = tree.get(b).unwrap();
    assert_eq!(node.parent, Some(a));
}

// ---------------------------------------------------------------------------
// Ancestors
// ---------------------------------------------------------------------------

#[test]
fn test_tree_ancestors() {
    let mut tree = WidgetTree::new();
    let root = tree.add_root(0);
    let child = tree.add_child(root, 0);
    let grandchild = tree.add_child(child, 0);

    let ancestors = tree.ancestors(grandchild);
    assert_eq!(ancestors, vec![child, root]);
}

#[test]
fn test_tree_ancestors_root_is_empty() {
    let mut tree = WidgetTree::new();
    let root = tree.add_root(0);
    let ancestors = tree.ancestors(root);
    assert!(ancestors.is_empty());
}

// ---------------------------------------------------------------------------
// Hit testing
// ---------------------------------------------------------------------------

#[test]
fn test_tree_hit_test_single_node() {
    let mut tree = WidgetTree::new();
    let root = tree.add_root(0);
    tree.set_bounds(root, Rect::new(0.0, 0.0, 100.0, 100.0));

    assert_eq!(tree.hit_test(Point::new(50.0, 50.0)), Some(root));
}

#[test]
fn test_tree_hit_test_miss() {
    let mut tree = WidgetTree::new();
    let root = tree.add_root(0);
    tree.set_bounds(root, Rect::new(0.0, 0.0, 100.0, 100.0));

    assert_eq!(tree.hit_test(Point::new(200.0, 200.0)), None);
}

#[test]
fn test_tree_hit_test_child_over_parent() {
    let mut tree = WidgetTree::new();
    let root = tree.add_root(0);
    tree.set_bounds(root, Rect::new(0.0, 0.0, 200.0, 200.0));

    let child = tree.add_child(root, 1);
    tree.set_bounds(child, Rect::new(50.0, 50.0, 100.0, 100.0));

    // Point inside child should hit child.
    assert_eq!(tree.hit_test(Point::new(75.0, 75.0)), Some(child));
    // Point outside child but inside root should hit root.
    assert_eq!(tree.hit_test(Point::new(10.0, 10.0)), Some(root));
}

#[test]
fn test_tree_hit_test_z_order() {
    let mut tree = WidgetTree::new();
    let root = tree.add_root(0);
    tree.set_bounds(root, Rect::new(0.0, 0.0, 200.0, 200.0));

    let back = tree.add_child(root, 0);
    tree.set_bounds(back, Rect::new(0.0, 0.0, 100.0, 100.0));

    let front = tree.add_child(root, 10);
    tree.set_bounds(front, Rect::new(0.0, 0.0, 100.0, 100.0));

    // Front widget (higher z-index) should win.
    assert_eq!(tree.hit_test(Point::new(50.0, 50.0)), Some(front));
}

#[test]
fn test_tree_hit_test_invisible_skipped() {
    let mut tree = WidgetTree::new();
    let root = tree.add_root(0);
    tree.set_bounds(root, Rect::new(0.0, 0.0, 200.0, 200.0));

    let child = tree.add_child(root, 1);
    tree.set_bounds(child, Rect::new(0.0, 0.0, 100.0, 100.0));

    // Make child invisible.
    tree.get_mut(child).unwrap().visible = false;

    // Should hit root even though child overlaps.
    assert_eq!(tree.hit_test(Point::new(50.0, 50.0)), Some(root));
}

// ---------------------------------------------------------------------------
// Set bounds
// ---------------------------------------------------------------------------

#[test]
fn test_tree_set_bounds() {
    let mut tree = WidgetTree::new();
    let root = tree.add_root(0);
    let r = Rect::new(10.0, 20.0, 300.0, 400.0);
    tree.set_bounds(root, r);

    let node = tree.get(root).unwrap();
    assert_eq!(node.bounds, r);
}

// ---------------------------------------------------------------------------
// WidgetId display
// ---------------------------------------------------------------------------

#[test]
fn test_widget_id_display() {
    let id = WidgetId::new(42);
    assert_eq!(id.to_string(), "Widget(42)");
}

#[test]
fn test_widget_id_equality() {
    assert_eq!(WidgetId(1), WidgetId(1));
    assert_ne!(WidgetId(1), WidgetId(2));
}

// ---------------------------------------------------------------------------
// Default tree
// ---------------------------------------------------------------------------

#[test]
fn test_tree_default() {
    let tree = WidgetTree::default();
    assert!(tree.is_empty());
    assert!(tree.root().is_none());
}
