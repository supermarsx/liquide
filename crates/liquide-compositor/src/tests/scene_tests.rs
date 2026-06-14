use std::sync::Arc;

use crate::geometry::Rect;
use crate::pixel::Color;
use crate::scene::*;

/// Helper: build a simple root → [bg, workspace → [surf_a, surf_b]] tree.
fn build_test_tree() -> SceneNode {
    let mut root = SceneNode::new(
        0,
        SceneNodeKind::Root,
        NodeProperties::new(Rect::new(0.0, 0.0, 1920.0, 1080.0)),
    );

    let bg = SceneNode::new(
        1,
        SceneNodeKind::Background {
            color: Color::BLACK,
        },
        NodeProperties::new(Rect::new(0.0, 0.0, 1920.0, 1080.0)),
    );

    let mut ws = SceneNode::new(
        2,
        SceneNodeKind::Workspace { index: 0 },
        NodeProperties::new(Rect::new(0.0, 0.0, 1920.0, 1080.0)),
    );

    let surf_a = SceneNode::new(
        3,
        SceneNodeKind::Surface {
            surface_id: 100,
            buffer: None,
        },
        NodeProperties::new(Rect::new(100.0, 100.0, 800.0, 600.0)),
    );

    let surf_b = SceneNode::new(
        4,
        SceneNodeKind::Surface {
            surface_id: 200,
            buffer: None,
        },
        NodeProperties::new(Rect::new(200.0, 150.0, 640.0, 480.0)),
    );

    ws.add_child(surf_a);
    ws.add_child(surf_b);
    root.add_child(bg);
    root.add_child(ws);
    root
}

#[test]
fn scene_node_flatten_basic() {
    let root = SceneNode::new(
        0,
        SceneNodeKind::Root,
        NodeProperties::new(Rect::new(0.0, 0.0, 1920.0, 1080.0)),
    );
    let flat = root.flatten();
    // Root alone is not visual (Root kind is excluded)
    assert!(flat.is_empty());
}

#[test]
fn scene_node_flatten_with_children() {
    let mut root = SceneNode::new(
        0,
        SceneNodeKind::Root,
        NodeProperties::new(Rect::new(0.0, 0.0, 1920.0, 1080.0)),
    );

    let bg = SceneNode::new(
        1,
        SceneNodeKind::Background {
            color: Color::new(30, 30, 40, 255),
        },
        NodeProperties::new(Rect::new(0.0, 0.0, 1920.0, 1080.0)),
    );

    let surface = SceneNode::new(
        2,
        SceneNodeKind::Surface {
            surface_id: 42,
            buffer: None,
        },
        NodeProperties::new(Rect::new(100.0, 100.0, 800.0, 600.0)),
    );

    root.add_child(bg);
    root.add_child(surface);

    let flat = root.flatten();
    assert_eq!(flat.len(), 2);
    assert_eq!(flat[0].id, 1); // Background
    assert_eq!(flat[1].id, 2); // Surface
    assert!((flat[1].absolute_bounds.x - 100.0).abs() < f32::EPSILON);
}

#[test]
fn flatten_reuses_shared_kind_handles_across_calls() {
    let tree = build_test_tree();

    let first = tree.flatten();
    let second = tree.flatten();

    let first_bg = first.iter().find(|node| node.id == 1).unwrap();
    let second_bg = second.iter().find(|node| node.id == 1).unwrap();
    let first_surface = first.iter().find(|node| node.id == 3).unwrap();
    let second_surface = second.iter().find(|node| node.id == 3).unwrap();

    assert!(Arc::ptr_eq(&first_bg.kind, &second_bg.kind));
    assert!(Arc::ptr_eq(&first_surface.kind, &second_surface.kind));
}

#[test]
fn scene_node_invisible_skipped() {
    let mut root = SceneNode::new(
        0,
        SceneNodeKind::Root,
        NodeProperties::new(Rect::new(0.0, 0.0, 1920.0, 1080.0)),
    );

    let hidden = SceneNode::new(
        1,
        SceneNodeKind::Background {
            color: Color::BLACK,
        },
        NodeProperties::new(Rect::new(0.0, 0.0, 1920.0, 1080.0)).with_visible(false),
    );

    root.add_child(hidden);
    let flat = root.flatten();
    assert!(flat.is_empty());
}

// --- Phase 9: Scene Graph Operations ---

#[test]
fn find_root() {
    let tree = build_test_tree();
    let found = tree.find(0);
    assert!(found.is_some());
    assert_eq!(found.unwrap().id, 0);
}

#[test]
fn find_nested_child() {
    let tree = build_test_tree();
    // surf_a is nested: root → workspace (2) → surf_a (3)
    let found = tree.find(3);
    assert!(found.is_some());
    assert_eq!(found.unwrap().id, 3);
}

#[test]
fn find_not_present() {
    let tree = build_test_tree();
    assert!(tree.find(999).is_none());
}

#[test]
fn find_mut_updates_node() {
    let mut tree = build_test_tree();
    let node = tree.find_mut(4).unwrap();
    node.properties.opacity = 0.5;

    // Verify the update persists
    let node = tree.find(4).unwrap();
    assert!((node.properties.opacity - 0.5).abs() < f32::EPSILON);
}

#[test]
fn remove_direct_child() {
    let mut tree = build_test_tree();
    // bg (1) is a direct child of root
    let removed = tree.remove_child(1);
    assert!(removed.is_some());
    assert_eq!(removed.unwrap().id, 1);
    // bg should no longer be findable
    assert!(tree.find(1).is_none());
    // workspace should still be present
    assert!(tree.find(2).is_some());
}

#[test]
fn remove_nested_child() {
    let mut tree = build_test_tree();
    // surf_b (4) is nested under workspace (2)
    let removed = tree.remove_child(4);
    assert!(removed.is_some());
    assert_eq!(removed.unwrap().id, 4);
    // surf_b should be gone, surf_a should remain
    assert!(tree.find(4).is_none());
    assert!(tree.find(3).is_some());
}

#[test]
fn remove_preserves_siblings() {
    let mut tree = build_test_tree();
    tree.remove_child(3); // remove surf_a
    // surf_b (4) should still be there
    let ws = tree.find(2).unwrap();
    assert_eq!(ws.children.len(), 1);
    assert_eq!(ws.children[0].id, 4);
}

#[test]
fn remove_not_found_returns_none() {
    let mut tree = build_test_tree();
    assert!(tree.remove_child(999).is_none());
}

#[test]
fn replace_child_updates_node() {
    let mut tree = build_test_tree();

    let replacement = SceneNode::new(
        3,
        SceneNodeKind::Overlay,
        NodeProperties::new(Rect::new(0.0, 0.0, 400.0, 300.0)),
    );

    let old = tree.replace_child(3, replacement);
    assert!(old.is_some());
    // The old node should be a Surface
    assert!(matches!(old.unwrap().kind, SceneNodeKind::Surface { .. }));
    // The new node should be an Overlay
    let found = tree.find(3).unwrap();
    assert!(matches!(found.kind, SceneNodeKind::Overlay));
}

#[test]
fn replace_child_not_found() {
    let mut tree = build_test_tree();
    let replacement = SceneNode::new(
        999,
        SceneNodeKind::Overlay,
        NodeProperties::new(Rect::new(0.0, 0.0, 1.0, 1.0)),
    );
    assert!(tree.replace_child(999, replacement).is_none());
}

#[test]
fn move_child_updates_bounds() {
    let mut tree = build_test_tree();
    let new_bounds = Rect::new(500.0, 400.0, 800.0, 600.0);
    tree.move_child(3, new_bounds);

    let moved = tree.find(3).unwrap();
    assert!((moved.properties.bounds.x - 500.0).abs() < f32::EPSILON);
    assert!((moved.properties.bounds.y - 400.0).abs() < f32::EPSILON);
}

#[test]
fn set_opacity_updates_node() {
    let mut tree = build_test_tree();
    tree.set_opacity(4, 0.3);

    let node = tree.find(4).unwrap();
    assert!((node.properties.opacity - 0.3).abs() < f32::EPSILON);
}

#[test]
fn descendants_lists_all() {
    let tree = build_test_tree();
    let desc = tree.descendants();
    // root has children: bg(1), ws(2); ws has: surf_a(3), surf_b(4)
    assert_eq!(desc.len(), 4);
    assert!(desc.contains(&1));
    assert!(desc.contains(&2));
    assert!(desc.contains(&3));
    assert!(desc.contains(&4));
}

#[test]
fn descendants_depth_first_order() {
    let tree = build_test_tree();
    let desc = tree.descendants();
    // bg(1), ws(2), surf_a(3), surf_b(4)
    assert_eq!(desc, vec![1, 2, 3, 4]);
}

#[test]
fn descendants_empty_for_leaf() {
    let leaf = SceneNode::new(
        10,
        SceneNodeKind::Cursor {
            shape: CursorShape::Arrow,
        },
        NodeProperties::new(Rect::new(0.0, 0.0, 24.0, 24.0)),
    );
    assert!(leaf.descendants().is_empty());
}

#[test]
fn depth_leaf_is_zero() {
    let leaf = SceneNode::new(
        10,
        SceneNodeKind::Cursor {
            shape: CursorShape::Arrow,
        },
        NodeProperties::new(Rect::new(0.0, 0.0, 24.0, 24.0)),
    );
    assert_eq!(leaf.depth(), 0);
}

#[test]
fn depth_nested_tree() {
    let tree = build_test_tree();
    // root → ws → surf = depth 2; root → bg = depth 1; max = 2
    assert_eq!(tree.depth(), 2);
}

#[test]
fn depth_single_child() {
    let mut root = SceneNode::new(
        0,
        SceneNodeKind::Root,
        NodeProperties::new(Rect::new(0.0, 0.0, 100.0, 100.0)),
    );
    root.add_child(SceneNode::new(
        1,
        SceneNodeKind::Content,
        NodeProperties::new(Rect::new(0.0, 0.0, 50.0, 50.0)),
    ));
    assert_eq!(root.depth(), 1);
}

#[test]
fn walk_visits_all_visible() {
    let tree = build_test_tree();
    let mut count = 0;
    tree.walk(&mut |_node, _transform, _opacity| {
        count += 1;
    });
    // Root + bg + workspace + surf_a + surf_b = 5 visible nodes
    assert_eq!(count, 5);
}

#[test]
fn flatten_z_order_sorting() {
    let mut root = SceneNode::new(
        100,
        SceneNodeKind::Root,
        NodeProperties::new(Rect::new(0.0, 0.0, 100.0, 100.0)),
    );
    // Add children with different z-orders
    root.add_child(SceneNode::new(
        101,
        SceneNodeKind::Background {
            color: Color::BLACK,
        },
        NodeProperties::new(Rect::new(0.0, 0.0, 100.0, 100.0)).with_z_order(2),
    ));
    root.add_child(SceneNode::new(
        102,
        SceneNodeKind::Background {
            color: Color::WHITE,
        },
        NodeProperties::new(Rect::new(0.0, 0.0, 100.0, 100.0)).with_z_order(1),
    ));
    let flat = root.flatten();
    // Lower z-order should come first due to walk order
    assert!(flat.len() >= 2);
    // z_order=1 should appear before z_order=2
    let pos_z1 = flat.iter().position(|n| n.id == 102).unwrap();
    let pos_z2 = flat.iter().position(|n| n.id == 101).unwrap();
    assert!(pos_z1 < pos_z2);
}

#[test]
fn find_mut_not_present_returns_none() {
    let mut tree = build_test_tree();
    assert!(tree.find_mut(9999).is_none());
}

#[test]
fn move_child_not_found_noop() {
    let mut tree = build_test_tree();
    // Should not panic - just no-op
    tree.move_child(9999, Rect::new(0.0, 0.0, 10.0, 10.0));
}

#[test]
fn scene_node_depth_with_hidden_children() {
    let mut root = SceneNode::new(
        1,
        SceneNodeKind::Root,
        NodeProperties::new(Rect::new(0.0, 0.0, 100.0, 100.0)),
    );
    root.add_child(SceneNode::new(
        2,
        SceneNodeKind::Background {
            color: Color::BLACK,
        },
        NodeProperties::new(Rect::new(0.0, 0.0, 100.0, 100.0)).with_visible(false),
    ));
    // Depth still counts invisible children (depth is structural)
    assert_eq!(root.depth(), 1);
}

#[test]
fn child_count_recursive() {
    let tree = build_test_tree();
    // build_test_tree creates: root -> [bg, workspace -> [surf_a, surf_b]]
    // That's 4 descendants total
    assert!(tree.child_count() >= 4);
}

#[test]
fn walk_mut_modifies_nodes() {
    let mut tree = build_test_tree();
    tree.walk_mut(&mut |node| {
        node.properties.opacity = 0.5;
    });
    // Verify root opacity changed
    assert_eq!(tree.properties.opacity, 0.5);
}

// --- Regression: CRITICAL-002 zero-area / non-overlapping clip pruning ---

/// A node whose own clip does not overlap its parent's clip must be pruned
/// entirely from the flattened output rather than emitted with a zero-area
/// clip (which renders invisible and used to leave a "hole").
#[test]
fn flatten_non_overlapping_clips_prunes_node() {
    let mut root = SceneNode::new(
        0,
        SceneNodeKind::Root,
        NodeProperties::new(Rect::new(0.0, 0.0, 1000.0, 1000.0)),
    );

    // Parent clips to the left half; its child clips to a disjoint region on
    // the right. The intersection is empty -> the child (id 2) must vanish.
    let mut parent = SceneNode::new(
        1,
        SceneNodeKind::Background {
            color: Color::BLACK,
        },
        NodeProperties::new(Rect::new(0.0, 0.0, 1000.0, 1000.0))
            .with_clip(Rect::new(0.0, 0.0, 100.0, 100.0)),
    );
    let child = SceneNode::new(
        2,
        SceneNodeKind::Surface {
            surface_id: 7,
            buffer: None,
        },
        // Clip (in parent coords) is far to the right, disjoint from parent's.
        NodeProperties::new(Rect::new(0.0, 0.0, 100.0, 100.0))
            .with_clip(Rect::new(500.0, 500.0, 100.0, 100.0)),
    );
    parent.add_child(child);
    root.add_child(parent);

    let flat = root.flatten();
    // Parent is still visible (it has a non-empty clip).
    assert!(flat.iter().any(|n| n.id == 1), "parent must remain visible");
    // Child must be pruned: empty clip intersection.
    assert!(
        !flat.iter().any(|n| n.id == 2),
        "node with empty clip intersection must be pruned, not emitted invisible"
    );
}

/// An overlapping clip hierarchy must still emit the child with the trimmed
/// (non-empty) intersection clip — the prune must not be over-eager.
#[test]
fn flatten_overlapping_clips_keeps_node_with_intersection() {
    let mut root = SceneNode::new(
        0,
        SceneNodeKind::Root,
        NodeProperties::new(Rect::new(0.0, 0.0, 1000.0, 1000.0)),
    );
    let mut parent = SceneNode::new(
        1,
        SceneNodeKind::Background {
            color: Color::BLACK,
        },
        NodeProperties::new(Rect::new(0.0, 0.0, 1000.0, 1000.0))
            .with_clip(Rect::new(0.0, 0.0, 200.0, 200.0)),
    );
    let child = SceneNode::new(
        2,
        SceneNodeKind::Surface {
            surface_id: 7,
            buffer: None,
        },
        NodeProperties::new(Rect::new(0.0, 0.0, 200.0, 200.0))
            .with_clip(Rect::new(100.0, 100.0, 200.0, 200.0)),
    );
    parent.add_child(child);
    root.add_child(parent);

    let flat = root.flatten();
    let child = flat
        .iter()
        .find(|n| n.id == 2)
        .expect("overlapping clip child must be emitted");
    let clip = child.clip.expect("child must carry an intersection clip");
    // Intersection of [0,200]x[0,200] and [100,300]x[100,300] = [100,200]^2.
    assert!((clip.x - 100.0).abs() < f32::EPSILON);
    assert!((clip.y - 100.0).abs() < f32::EPSILON);
    assert!((clip.width - 100.0).abs() < f32::EPSILON);
    assert!((clip.height - 100.0).abs() < f32::EPSILON);
}

/// A node clipping to an explicitly zero-area rect is unpaintable and must be
/// pruned even when there is no parent clip.
#[test]
fn flatten_zero_area_clip_prunes_node() {
    let mut root = SceneNode::new(
        0,
        SceneNodeKind::Root,
        NodeProperties::new(Rect::new(0.0, 0.0, 1000.0, 1000.0)),
    );
    let node = SceneNode::new(
        1,
        SceneNodeKind::Surface {
            surface_id: 7,
            buffer: None,
        },
        NodeProperties::new(Rect::new(0.0, 0.0, 100.0, 100.0))
            .with_clip(Rect::new(10.0, 10.0, 0.0, 0.0)),
    );
    root.add_child(node);
    let flat = root.flatten();
    assert!(
        flat.is_empty(),
        "node clipped to a zero-area rect must be pruned"
    );
}

// --- Regression: HIGH-001 zero-opacity culling ---

/// A fully transparent node (and its subtree) must not be emitted.
#[test]
fn flatten_zero_opacity_node_culled() {
    let mut root = SceneNode::new(
        0,
        SceneNodeKind::Root,
        NodeProperties::new(Rect::new(0.0, 0.0, 1000.0, 1000.0)),
    );
    let mut invisible = SceneNode::new(
        1,
        SceneNodeKind::Background {
            color: Color::BLACK,
        },
        NodeProperties::new(Rect::new(0.0, 0.0, 1000.0, 1000.0)).with_opacity(0.0),
    );
    // A child under a zero-opacity parent inherits opacity 0 and must also be
    // pruned (opacity is multiplicative down the tree).
    invisible.add_child(SceneNode::new(
        2,
        SceneNodeKind::Surface {
            surface_id: 7,
            buffer: None,
        },
        NodeProperties::new(Rect::new(0.0, 0.0, 100.0, 100.0)),
    ));
    root.add_child(invisible);

    let flat = root.flatten();
    assert!(
        flat.is_empty(),
        "zero-opacity node and its subtree must be culled"
    );
}

/// A nearly-but-not-fully transparent node must still be emitted (the cull
/// threshold must not swallow faint-but-visible content).
#[test]
fn flatten_low_opacity_node_kept() {
    let mut root = SceneNode::new(
        0,
        SceneNodeKind::Root,
        NodeProperties::new(Rect::new(0.0, 0.0, 1000.0, 1000.0)),
    );
    root.add_child(SceneNode::new(
        1,
        SceneNodeKind::Background {
            color: Color::BLACK,
        },
        NodeProperties::new(Rect::new(0.0, 0.0, 1000.0, 1000.0)).with_opacity(0.02),
    ));
    let flat = root.flatten();
    assert!(
        flat.iter().any(|n| n.id == 1),
        "faint-but-visible node must not be culled"
    );
}
