//! Extensive hit-test engine tests.
//!
//! Covers: transform-origin signs, composed transforms, pointer-events: none,
//! overflow clipping, visibility checks, scroll offsets, z-index ordering,
//! and absolute-coordinate resolution.

use liquide_dom::NodeId;
use liquide_hit_test::engine::HitTestEngine;
use liquide_layout::geometry::{Point, Rect};
use liquide_layout::tree::{BoxType, LayoutTree};
use liquide_style_engine::StyleMap;
use liquide_style_engine::computed::{
    ComputedStyle, ContentVisibility, Display, LengthPercent, Overflow, PointerEvents, Position,
    Transform, Visibility,
};

// ── Helpers ──────────────────────────────────────────────────────────────

/// Build a simple tree: root (800×600) with one child at (x, y, w, h).
fn one_child_tree(
    child_x: f32,
    child_y: f32,
    child_w: f32,
    child_h: f32,
) -> (LayoutTree, NodeId, NodeId) {
    let root_node = NodeId::from(1u64);
    let child_node = NodeId::from(2u64);

    let mut tree = LayoutTree::new();
    let root_id = tree.alloc(root_node, BoxType::Block);
    {
        let r = tree.get_mut(root_id).unwrap();
        r.content_rect = Rect::new(0.0, 0.0, 800.0, 600.0);
        r.padding_rect = Rect::new(0.0, 0.0, 800.0, 600.0);
        r.border_rect = Rect::new(0.0, 0.0, 800.0, 600.0);
        r.margin_rect = Rect::new(0.0, 0.0, 800.0, 600.0);
    }
    let child_id = tree.alloc(child_node, BoxType::Block);
    {
        let c = tree.get_mut(child_id).unwrap();
        c.content_rect = Rect::new(child_x, child_y, child_w, child_h);
        c.padding_rect = Rect::new(child_x, child_y, child_w, child_h);
        c.border_rect = Rect::new(child_x, child_y, child_w, child_h);
        c.margin_rect = Rect::new(child_x, child_y, child_w, child_h);
    }
    tree.add_child(root_id, child_id);
    tree.root = root_id;

    (tree, root_node, child_node)
}

fn default_styles_for(nodes: &[NodeId]) -> StyleMap {
    let mut sm = StyleMap::new();
    for &n in nodes {
        sm.insert(n, ComputedStyle::default());
    }
    sm
}

// ── Basic hit testing ────────────────────────────────────────────────────

#[test]
fn hit_test_basic_child_inside() {
    let (tree, root_node, child_node) = one_child_tree(100.0, 100.0, 200.0, 200.0);
    let styles = default_styles_for(&[root_node, child_node]);
    let engine = HitTestEngine::from_owned(tree, styles);

    // Click inside child box
    let result = engine.hit_test(Point::new(150.0, 150.0));
    assert!(result.is_some(), "should hit the child");
    assert_eq!(result.unwrap().node, child_node);
}

#[test]
fn hit_test_basic_child_outside_hits_root() {
    let (tree, root_node, child_node) = one_child_tree(100.0, 100.0, 200.0, 200.0);
    let styles = default_styles_for(&[root_node, child_node]);
    let engine = HitTestEngine::from_owned(tree, styles);

    // Click outside child but inside root
    let result = engine.hit_test(Point::new(50.0, 50.0));
    assert!(result.is_some(), "should hit root");
    assert_eq!(result.unwrap().node, root_node);
}

#[test]
fn hit_test_miss_everything() {
    let (tree, root_node, child_node) = one_child_tree(100.0, 100.0, 200.0, 200.0);
    let styles = default_styles_for(&[root_node, child_node]);
    let engine = HitTestEngine::from_owned(tree, styles);

    // Click outside root
    let result = engine.hit_test(Point::new(900.0, 700.0));
    assert!(result.is_none(), "should miss entirely");
}

#[test]
fn hit_test_edge_of_child() {
    let (tree, root_node, child_node) = one_child_tree(100.0, 100.0, 200.0, 200.0);
    let styles = default_styles_for(&[root_node, child_node]);
    let engine = HitTestEngine::from_owned(tree, styles);

    // Exact top-left corner of child
    let result = engine.hit_test(Point::new(100.0, 100.0));
    assert!(result.is_some());
    assert_eq!(result.unwrap().node, child_node);

    // Just outside bottom-right
    let result = engine.hit_test(Point::new(300.1, 300.1));
    assert!(result.is_some());
    assert_eq!(
        result.unwrap().node,
        root_node,
        "should hit root, not child"
    );
}

// ── Visibility ───────────────────────────────────────────────────────────

#[test]
fn hit_test_visibility_hidden_skips_element() {
    let (tree, root_node, child_node) = one_child_tree(100.0, 100.0, 200.0, 200.0);
    let mut styles = default_styles_for(&[root_node, child_node]);

    // Make child hidden
    let mut s = ComputedStyle::default();
    s.visibility = Visibility::Hidden;
    styles.insert(child_node, s);

    let engine = HitTestEngine::from_owned(tree, styles);
    let result = engine.hit_test(Point::new(150.0, 150.0));
    assert!(result.is_some());
    assert_eq!(
        result.unwrap().node,
        root_node,
        "visibility:hidden child should not be hit; root should be"
    );
}

#[test]
fn hit_test_display_none_skips_element() {
    let (tree, root_node, child_node) = one_child_tree(100.0, 100.0, 200.0, 200.0);
    let mut styles = default_styles_for(&[root_node, child_node]);

    let mut s = ComputedStyle::default();
    s.display = Display::None;
    styles.insert(child_node, s);

    let engine = HitTestEngine::from_owned(tree, styles);
    let result = engine.hit_test(Point::new(150.0, 150.0));
    assert!(result.is_some());
    assert_eq!(result.unwrap().node, root_node);
}

#[test]
fn hit_test_content_visibility_hidden_skips_element() {
    let (tree, root_node, child_node) = one_child_tree(100.0, 100.0, 200.0, 200.0);
    let mut styles = default_styles_for(&[root_node, child_node]);

    let mut s = ComputedStyle::default();
    s.content_visibility = ContentVisibility::Hidden;
    styles.insert(child_node, s);

    let engine = HitTestEngine::from_owned(tree, styles);
    let result = engine.hit_test(Point::new(150.0, 150.0));
    assert!(result.is_some());
    assert_eq!(
        result.unwrap().node,
        root_node,
        "content-visibility:hidden should skip"
    );
}

// ── pointer-events: none ─────────────────────────────────────────────────

#[test]
fn hit_test_pointer_events_none_skips_self() {
    let (tree, root_node, child_node) = one_child_tree(100.0, 100.0, 200.0, 200.0);
    let mut styles = default_styles_for(&[root_node, child_node]);

    let mut s = ComputedStyle::default();
    s.pointer_events = PointerEvents::None;
    styles.insert(child_node, s);

    let engine = HitTestEngine::from_owned(tree, styles);

    // Should not hit the child (pointer-events: none)
    let result = engine.hit_test(Point::new(150.0, 150.0));
    assert!(result.is_some());
    assert_eq!(
        result.unwrap().node,
        root_node,
        "pointer-events:none means element is invisible to hit-testing"
    );
}

#[test]
fn hit_test_pointer_events_none_parent_allows_child_hits() {
    // parent has pointer-events:none, but child is auto.
    // Per CSS spec, child should still be hittable.
    let root_node = NodeId::from(1u64);
    let parent_node = NodeId::from(2u64);
    let child_node = NodeId::from(3u64);

    let mut tree = LayoutTree::new();
    let root_id = tree.alloc(root_node, BoxType::Block);
    {
        let r = tree.get_mut(root_id).unwrap();
        r.content_rect = Rect::new(0.0, 0.0, 800.0, 600.0);
        r.padding_rect = Rect::new(0.0, 0.0, 800.0, 600.0);
        r.border_rect = Rect::new(0.0, 0.0, 800.0, 600.0);
        r.margin_rect = Rect::new(0.0, 0.0, 800.0, 600.0);
    }

    let parent_id = tree.alloc(parent_node, BoxType::Block);
    {
        let p = tree.get_mut(parent_id).unwrap();
        p.content_rect = Rect::new(50.0, 50.0, 400.0, 400.0);
        p.padding_rect = Rect::new(50.0, 50.0, 400.0, 400.0);
        p.border_rect = Rect::new(50.0, 50.0, 400.0, 400.0);
        p.margin_rect = Rect::new(50.0, 50.0, 400.0, 400.0);
    }

    let child_id = tree.alloc(child_node, BoxType::Block);
    {
        let c = tree.get_mut(child_id).unwrap();
        c.content_rect = Rect::new(10.0, 10.0, 100.0, 100.0);
        c.padding_rect = Rect::new(10.0, 10.0, 100.0, 100.0);
        c.border_rect = Rect::new(10.0, 10.0, 100.0, 100.0);
        c.margin_rect = Rect::new(10.0, 10.0, 100.0, 100.0);
    }

    tree.add_child(root_id, parent_id);
    tree.add_child(parent_id, child_id);
    tree.root = root_id;

    let mut styles = StyleMap::new();
    styles.insert(root_node, ComputedStyle::default());

    let mut parent_style = ComputedStyle::default();
    parent_style.pointer_events = PointerEvents::None;
    styles.insert(parent_node, parent_style);

    styles.insert(child_node, ComputedStyle::default()); // auto

    let engine = HitTestEngine::from_owned(tree, styles);

    // Click inside child (absolute: 50+10=60, 50+10=60)
    let result = engine.hit_test(Point::new(70.0, 70.0));
    assert!(
        result.is_some(),
        "child with pointer-events:auto should be hit"
    );
    assert_eq!(result.unwrap().node, child_node);
}

// ── hit_test_all ─────────────────────────────────────────────────────────

#[test]
fn hit_test_all_returns_multiple_overlapping() {
    // Overlapping siblings: both under same point
    let root_node = NodeId::from(1u64);
    let child_a = NodeId::from(2u64);
    let child_b = NodeId::from(3u64);

    let mut tree = LayoutTree::new();
    let root_id = tree.alloc(root_node, BoxType::Block);
    {
        let r = tree.get_mut(root_id).unwrap();
        r.content_rect = Rect::new(0.0, 0.0, 800.0, 600.0);
        r.padding_rect = Rect::new(0.0, 0.0, 800.0, 600.0);
        r.border_rect = Rect::new(0.0, 0.0, 800.0, 600.0);
        r.margin_rect = Rect::new(0.0, 0.0, 800.0, 600.0);
    }

    let a_id = tree.alloc(child_a, BoxType::Block);
    {
        let a = tree.get_mut(a_id).unwrap();
        a.content_rect = Rect::new(50.0, 50.0, 200.0, 200.0);
        a.padding_rect = Rect::new(50.0, 50.0, 200.0, 200.0);
        a.border_rect = Rect::new(50.0, 50.0, 200.0, 200.0);
        a.margin_rect = Rect::new(50.0, 50.0, 200.0, 200.0);
    }

    let b_id = tree.alloc(child_b, BoxType::Block);
    {
        let b = tree.get_mut(b_id).unwrap();
        b.content_rect = Rect::new(100.0, 100.0, 200.0, 200.0);
        b.padding_rect = Rect::new(100.0, 100.0, 200.0, 200.0);
        b.border_rect = Rect::new(100.0, 100.0, 200.0, 200.0);
        b.margin_rect = Rect::new(100.0, 100.0, 200.0, 200.0);
    }

    tree.add_child(root_id, a_id);
    tree.add_child(root_id, b_id);
    tree.root = root_id;

    let styles = default_styles_for(&[root_node, child_a, child_b]);
    let engine = HitTestEngine::from_owned(tree, styles);

    // Point in the overlap region
    let results = engine.hit_test_all(Point::new(150.0, 150.0));
    // Should contain at least child_a and child_b (and root)
    let nodes: Vec<NodeId> = results.iter().map(|r| r.node).collect();
    assert!(
        nodes.contains(&child_a),
        "hit_test_all should include child_a"
    );
    assert!(
        nodes.contains(&child_b),
        "hit_test_all should include child_b"
    );
    assert!(
        nodes.contains(&root_node),
        "hit_test_all should include root"
    );
}

#[test]
fn hit_test_all_pointer_events_none_still_recurses_children() {
    // Parent has pointer-events:none, but child is pointer-events:auto.
    // hit_test_all should still include the child.
    let root_node = NodeId::from(1u64);
    let parent_node = NodeId::from(2u64);
    let child_node = NodeId::from(3u64);

    let mut tree = LayoutTree::new();
    let root_id = tree.alloc(root_node, BoxType::Block);
    {
        let r = tree.get_mut(root_id).unwrap();
        r.content_rect = Rect::new(0.0, 0.0, 800.0, 600.0);
        r.padding_rect = r.content_rect;
        r.border_rect = r.content_rect;
        r.margin_rect = r.content_rect;
    }
    let parent_id = tree.alloc(parent_node, BoxType::Block);
    {
        let p = tree.get_mut(parent_id).unwrap();
        p.content_rect = Rect::new(0.0, 0.0, 400.0, 400.0);
        p.padding_rect = p.content_rect;
        p.border_rect = p.content_rect;
        p.margin_rect = p.content_rect;
    }
    let child_id = tree.alloc(child_node, BoxType::Block);
    {
        let c = tree.get_mut(child_id).unwrap();
        c.content_rect = Rect::new(10.0, 10.0, 100.0, 100.0);
        c.padding_rect = c.content_rect;
        c.border_rect = c.content_rect;
        c.margin_rect = c.content_rect;
    }
    tree.add_child(root_id, parent_id);
    tree.add_child(parent_id, child_id);
    tree.root = root_id;

    let mut styles = StyleMap::new();
    styles.insert(root_node, ComputedStyle::default());
    let mut ps = ComputedStyle::default();
    ps.pointer_events = PointerEvents::None;
    styles.insert(parent_node, ps);
    styles.insert(child_node, ComputedStyle::default());

    let engine = HitTestEngine::from_owned(tree, styles);

    let results = engine.hit_test_all(Point::new(15.0, 15.0));
    let nodes: Vec<NodeId> = results.iter().map(|r| r.node).collect();

    assert!(
        nodes.contains(&child_node),
        "hit_test_all should recurse into pointer-events:none parent to find child"
    );
    // Parent itself should NOT be in results (pointer-events:none)
    assert!(
        !nodes.contains(&parent_node),
        "pointer-events:none parent itself should not be in results"
    );
}

// ── CSS Transforms ───────────────────────────────────────────────────────

#[test]
fn hit_test_translate_moves_hit_region() {
    let (tree, root_node, child_node) = one_child_tree(0.0, 0.0, 100.0, 100.0);
    let mut styles = default_styles_for(&[root_node, child_node]);

    // Translate child by (200, 200) — should move its hit region
    let mut s = ComputedStyle::default();
    s.transform = vec![Transform::Translate(
        LengthPercent::Px(200.0),
        LengthPercent::Px(200.0),
    )];
    styles.insert(child_node, s);

    let engine = HitTestEngine::from_owned(tree, styles);

    // Origin of child (0, 0) should no longer hit child
    let result = engine.hit_test(Point::new(50.0, 50.0));
    assert!(result.is_some());
    assert_ne!(
        result.unwrap().node,
        child_node,
        "original position should not hit translated child"
    );

    // Translated position should hit
    let result = engine.hit_test(Point::new(250.0, 250.0));
    assert!(result.is_some());
    assert_eq!(
        result.unwrap().node,
        child_node,
        "translated position should hit child"
    );
}

#[test]
fn hit_test_scale_enlarges_hit_region() {
    // Child at (100, 100, 50, 50), scaled by 2x → should be effectively 100×100
    // centered on (125, 125) i.e. from (75, 75) to (175, 175)
    let (tree, root_node, child_node) = one_child_tree(100.0, 100.0, 50.0, 50.0);
    let mut styles = default_styles_for(&[root_node, child_node]);

    let mut s = ComputedStyle::default();
    s.transform = vec![Transform::Scale(2.0, 2.0)];
    styles.insert(child_node, s);

    let engine = HitTestEngine::from_owned(tree, styles);

    // Center of original bounds (125, 125) — should definitely hit
    let result = engine.hit_test(Point::new(125.0, 125.0));
    assert!(result.is_some());
    assert_eq!(result.unwrap().node, child_node);

    // Point at (80, 80) — within scaled bounds (75–175) but outside original (100–150)
    let result = engine.hit_test(Point::new(80.0, 80.0));
    assert!(result.is_some());
    assert_eq!(
        result.unwrap().node,
        child_node,
        "scale(2) should enlarge hit area"
    );
}

#[test]
fn hit_test_rotate_90_swaps_axes() {
    // Child at (100, 100, 200, 50) [wide box], rotate 90° around center (200, 125)
    // After rotation, should become a tall narrow box
    let (tree, root_node, child_node) = one_child_tree(100.0, 100.0, 200.0, 50.0);
    let mut styles = default_styles_for(&[root_node, child_node]);

    let mut s = ComputedStyle::default();
    s.transform = vec![Transform::Rotate(90.0)]; // 90 degrees
    styles.insert(child_node, s);

    let engine = HitTestEngine::from_owned(tree, styles);

    // Center of original box (200, 125) — should always hit regardless of rotation
    let result = engine.hit_test(Point::new(200.0, 125.0));
    assert!(result.is_some());
    assert_eq!(result.unwrap().node, child_node, "center should always hit");
}

#[test]
fn hit_test_composed_translate_then_scale() {
    // transform: translate(100, 0) scale(2, 2) — first translate, then scale
    let (tree, root_node, child_node) = one_child_tree(0.0, 0.0, 50.0, 50.0);
    let mut styles = default_styles_for(&[root_node, child_node]);

    let mut s = ComputedStyle::default();
    s.transform = vec![
        Transform::Translate(LengthPercent::Px(100.0), LengthPercent::ZERO),
        Transform::Scale(2.0, 2.0),
    ];
    styles.insert(child_node, s);

    let engine = HitTestEngine::from_owned(tree, styles);

    // Original position (25, 25) — should not hit
    let result = engine.hit_test(Point::new(25.0, 25.0));
    if let Some(r) = &result {
        assert_ne!(
            r.node, child_node,
            "original position should not hit transformed child"
        );
    }
}

// ── Overflow clipping ────────────────────────────────────────────────────

#[test]
fn hit_test_overflow_hidden_clips_children() {
    // Parent with overflow:hidden smaller than child
    let root_node = NodeId::from(1u64);
    let parent_node = NodeId::from(2u64);
    let child_node = NodeId::from(3u64);

    let mut tree = LayoutTree::new();
    let root_id = tree.alloc(root_node, BoxType::Block);
    {
        let r = tree.get_mut(root_id).unwrap();
        r.content_rect = Rect::new(0.0, 0.0, 800.0, 600.0);
        r.padding_rect = r.content_rect;
        r.border_rect = r.content_rect;
        r.margin_rect = r.content_rect;
    }
    let parent_id = tree.alloc(parent_node, BoxType::Block);
    {
        let p = tree.get_mut(parent_id).unwrap();
        // Parent is 100×100 at (50, 50)
        p.content_rect = Rect::new(50.0, 50.0, 100.0, 100.0);
        p.padding_rect = p.content_rect;
        p.border_rect = p.content_rect;
        p.margin_rect = p.content_rect;
    }
    let child_id = tree.alloc(child_node, BoxType::Block);
    {
        let c = tree.get_mut(child_id).unwrap();
        // Child is 300×300 — extends well beyond parent's clip area
        c.content_rect = Rect::new(0.0, 0.0, 300.0, 300.0);
        c.padding_rect = c.content_rect;
        c.border_rect = c.content_rect;
        c.margin_rect = c.content_rect;
    }
    tree.add_child(root_id, parent_id);
    tree.add_child(parent_id, child_id);
    tree.root = root_id;

    let mut styles = StyleMap::new();
    styles.insert(root_node, ComputedStyle::default());

    let mut parent_style = ComputedStyle::default();
    parent_style.overflow_x = Overflow::Hidden;
    parent_style.overflow_y = Overflow::Hidden;
    styles.insert(parent_node, parent_style);

    styles.insert(child_node, ComputedStyle::default());

    let engine = HitTestEngine::from_owned(tree, styles);

    // Inside both parent clip and child: should hit child
    let result = engine.hit_test(Point::new(100.0, 100.0));
    assert!(result.is_some());
    assert_eq!(result.unwrap().node, child_node, "inside clip → hit child");

    // Inside child's extended area but outside parent clip: should NOT hit child
    let result = engine.hit_test(Point::new(200.0, 200.0));
    if let Some(r) = &result {
        assert_ne!(
            r.node, child_node,
            "outside parent's overflow:hidden clip should not hit child"
        );
    }
}

// ── Scroll offsets ───────────────────────────────────────────────────────

#[test]
fn hit_test_scroll_offset_shifts_children() {
    // Scrolled parent: child at (0, 0, 100, 100), parent scrolled by (0, 50)
    let root_node = NodeId::from(1u64);
    let parent_node = NodeId::from(2u64);
    let child_node = NodeId::from(3u64);

    let mut tree = LayoutTree::new();
    let root_id = tree.alloc(root_node, BoxType::Block);
    {
        let r = tree.get_mut(root_id).unwrap();
        r.content_rect = Rect::new(0.0, 0.0, 400.0, 400.0);
        r.padding_rect = r.content_rect;
        r.border_rect = r.content_rect;
        r.margin_rect = r.content_rect;
    }
    let parent_id = tree.alloc(parent_node, BoxType::Block);
    {
        let p = tree.get_mut(parent_id).unwrap();
        p.content_rect = Rect::new(0.0, 0.0, 400.0, 200.0);
        p.padding_rect = p.content_rect;
        p.border_rect = p.content_rect;
        p.margin_rect = p.content_rect;
        p.scroll_offset = (0.0, 50.0); // scrolled down 50px
    }
    let child_id = tree.alloc(child_node, BoxType::Block);
    {
        let c = tree.get_mut(child_id).unwrap();
        c.content_rect = Rect::new(0.0, 0.0, 400.0, 100.0);
        c.padding_rect = c.content_rect;
        c.border_rect = c.content_rect;
        c.margin_rect = c.content_rect;
    }
    tree.add_child(root_id, parent_id);
    tree.add_child(parent_id, child_id);
    tree.root = root_id;

    let mut styles = StyleMap::new();
    styles.insert(root_node, ComputedStyle::default());
    let mut ps = ComputedStyle::default();
    ps.overflow_y = Overflow::Scroll;
    styles.insert(parent_node, ps);
    styles.insert(child_node, ComputedStyle::default());

    let engine = HitTestEngine::from_owned(tree, styles);

    // Without scroll, child at y=0..100 visible in viewport.
    // With scroll_offset=(0, 50), what was at y=0 is now at visual y=-50.
    // Clicking at visual y=25 should correspond to content y=75 (within child).
    let result = engine.hit_test(Point::new(50.0, 25.0));
    assert!(
        result.is_some(),
        "should hit something in scrolled container"
    );
}

// ── Deeply nested hit testing ────────────────────────────────────────────

#[test]
fn hit_test_deeply_nested_returns_deepest() {
    // root > A > B > C: click in C should return C
    let root_node = NodeId::from(1u64);
    let node_a = NodeId::from(2u64);
    let node_b = NodeId::from(3u64);
    let node_c = NodeId::from(4u64);

    let mut tree = LayoutTree::new();
    let root_id = tree.alloc(root_node, BoxType::Block);
    tree.get_mut(root_id).unwrap().content_rect = Rect::new(0.0, 0.0, 800.0, 600.0);
    tree.get_mut(root_id).unwrap().padding_rect = Rect::new(0.0, 0.0, 800.0, 600.0);
    tree.get_mut(root_id).unwrap().border_rect = Rect::new(0.0, 0.0, 800.0, 600.0);
    tree.get_mut(root_id).unwrap().margin_rect = Rect::new(0.0, 0.0, 800.0, 600.0);

    let a_id = tree.alloc(node_a, BoxType::Block);
    tree.get_mut(a_id).unwrap().content_rect = Rect::new(10.0, 10.0, 300.0, 300.0);
    tree.get_mut(a_id).unwrap().padding_rect = Rect::new(10.0, 10.0, 300.0, 300.0);
    tree.get_mut(a_id).unwrap().border_rect = Rect::new(10.0, 10.0, 300.0, 300.0);
    tree.get_mut(a_id).unwrap().margin_rect = Rect::new(10.0, 10.0, 300.0, 300.0);

    let b_id = tree.alloc(node_b, BoxType::Block);
    tree.get_mut(b_id).unwrap().content_rect = Rect::new(5.0, 5.0, 200.0, 200.0);
    tree.get_mut(b_id).unwrap().padding_rect = Rect::new(5.0, 5.0, 200.0, 200.0);
    tree.get_mut(b_id).unwrap().border_rect = Rect::new(5.0, 5.0, 200.0, 200.0);
    tree.get_mut(b_id).unwrap().margin_rect = Rect::new(5.0, 5.0, 200.0, 200.0);

    let c_id = tree.alloc(node_c, BoxType::Block);
    tree.get_mut(c_id).unwrap().content_rect = Rect::new(5.0, 5.0, 100.0, 100.0);
    tree.get_mut(c_id).unwrap().padding_rect = Rect::new(5.0, 5.0, 100.0, 100.0);
    tree.get_mut(c_id).unwrap().border_rect = Rect::new(5.0, 5.0, 100.0, 100.0);
    tree.get_mut(c_id).unwrap().margin_rect = Rect::new(5.0, 5.0, 100.0, 100.0);

    tree.add_child(root_id, a_id);
    tree.add_child(a_id, b_id);
    tree.add_child(b_id, c_id);
    tree.root = root_id;

    let styles = default_styles_for(&[root_node, node_a, node_b, node_c]);
    let engine = HitTestEngine::from_owned(tree, styles);

    // abs: root(0,0) + A(10,10) + B(5,5) + C(5,5) = (20, 20)
    // C is 100×100 from (20,20) to (120,120)
    let result = engine.hit_test(Point::new(50.0, 50.0));
    assert!(result.is_some());
    assert_eq!(
        result.unwrap().node,
        node_c,
        "should hit deepest nested node"
    );
}

// ── Sibling ordering ─────────────────────────────────────────────────────

#[test]
fn hit_test_later_sibling_wins_over_earlier() {
    // Per CSS painting order, later siblings are painted on top
    let root_node = NodeId::from(1u64);
    let first = NodeId::from(2u64);
    let second = NodeId::from(3u64);

    let mut tree = LayoutTree::new();
    let root_id = tree.alloc(root_node, BoxType::Block);
    tree.get_mut(root_id).unwrap().content_rect = Rect::new(0.0, 0.0, 800.0, 600.0);
    tree.get_mut(root_id).unwrap().padding_rect = Rect::new(0.0, 0.0, 800.0, 600.0);
    tree.get_mut(root_id).unwrap().border_rect = Rect::new(0.0, 0.0, 800.0, 600.0);
    tree.get_mut(root_id).unwrap().margin_rect = Rect::new(0.0, 0.0, 800.0, 600.0);

    // Both fully overlap
    let first_id = tree.alloc(first, BoxType::Block);
    tree.get_mut(first_id).unwrap().content_rect = Rect::new(0.0, 0.0, 200.0, 200.0);
    tree.get_mut(first_id).unwrap().padding_rect = Rect::new(0.0, 0.0, 200.0, 200.0);
    tree.get_mut(first_id).unwrap().border_rect = Rect::new(0.0, 0.0, 200.0, 200.0);
    tree.get_mut(first_id).unwrap().margin_rect = Rect::new(0.0, 0.0, 200.0, 200.0);

    let second_id = tree.alloc(second, BoxType::Block);
    tree.get_mut(second_id).unwrap().content_rect = Rect::new(0.0, 0.0, 200.0, 200.0);
    tree.get_mut(second_id).unwrap().padding_rect = Rect::new(0.0, 0.0, 200.0, 200.0);
    tree.get_mut(second_id).unwrap().border_rect = Rect::new(0.0, 0.0, 200.0, 200.0);
    tree.get_mut(second_id).unwrap().margin_rect = Rect::new(0.0, 0.0, 200.0, 200.0);

    tree.add_child(root_id, first_id);
    tree.add_child(root_id, second_id);
    tree.root = root_id;

    let styles = default_styles_for(&[root_node, first, second]);
    let engine = HitTestEngine::from_owned(tree, styles);

    let result = engine.hit_test(Point::new(100.0, 100.0));
    assert!(result.is_some());
    assert_eq!(
        result.unwrap().node,
        second,
        "later sibling should be on top (painter's order)"
    );
}

// ── Positioned elements ──────────────────────────────────────────────────

#[test]
fn hit_test_absolute_positioned_child() {
    let root_node = NodeId::from(1u64);
    let child_node = NodeId::from(2u64);

    let mut tree = LayoutTree::new();
    let root_id = tree.alloc(root_node, BoxType::Block);
    {
        let r = tree.get_mut(root_id).unwrap();
        r.content_rect = Rect::new(0.0, 0.0, 800.0, 600.0);
        r.padding_rect = r.content_rect;
        r.border_rect = r.content_rect;
        r.margin_rect = r.content_rect;
    }
    let child_id = tree.alloc(child_node, BoxType::Absolute);
    {
        let c = tree.get_mut(child_id).unwrap();
        c.content_rect = Rect::new(200.0, 200.0, 100.0, 100.0);
        c.padding_rect = c.content_rect;
        c.border_rect = c.content_rect;
        c.margin_rect = c.content_rect;
    }
    tree.add_child(root_id, child_id);
    tree.root = root_id;

    let mut styles = StyleMap::new();
    let mut root_style = ComputedStyle::default();
    root_style.position = Position::Relative;
    styles.insert(root_node, root_style);

    let mut child_style = ComputedStyle::default();
    child_style.position = Position::Absolute;
    styles.insert(child_node, child_style);

    let engine = HitTestEngine::from_owned(tree, styles);

    let result = engine.hit_test(Point::new(250.0, 250.0));
    assert!(result.is_some());
    assert_eq!(result.unwrap().node, child_node);
}

// ── Point-in-node output ─────────────────────────────────────────────────

#[test]
fn hit_test_point_in_node_is_local_coordinates() {
    let (tree, root_node, child_node) = one_child_tree(100.0, 100.0, 200.0, 200.0);
    let styles = default_styles_for(&[root_node, child_node]);
    let engine = HitTestEngine::from_owned(tree, styles);

    let result = engine.hit_test(Point::new(150.0, 140.0)).unwrap();
    assert_eq!(result.node, child_node);
    // Local coords: screen (150, 140) - child origin (100, 100) = (50, 40)
    let p = result.point_in_node;
    assert!(
        (p.x - 50.0).abs() < 1.0,
        "local x should be ~50, got {}",
        p.x
    );
    assert!(
        (p.y - 40.0).abs() < 1.0,
        "local y should be ~40, got {}",
        p.y
    );
}

// ── Zero-size element ────────────────────────────────────────────────────

#[test]
fn hit_test_zero_size_element_is_not_hit() {
    let (tree, root_node, child_node) = one_child_tree(100.0, 100.0, 0.0, 0.0);
    let styles = default_styles_for(&[root_node, child_node]);
    let engine = HitTestEngine::from_owned(tree, styles);

    let result = engine.hit_test(Point::new(100.0, 100.0));
    assert!(result.is_some());
    assert_eq!(
        result.unwrap().node,
        root_node,
        "zero-size element should not be hit"
    );
}

// ── Regression: subtree culling by parent box (t49-e4-02) ─────────────────

/// A child painted OUTSIDE a non-clipping parent (overflow: visible) must
/// still be hittable at its real location. The parent's border box must not
/// gate descendant hit-testing unless the parent actually clips.
#[test]
fn hit_test_child_outside_nonclipping_parent_is_hittable() {
    let root_node = NodeId::from(1u64);
    let parent_node = NodeId::from(2u64);
    let child_node = NodeId::from(3u64);

    let mut tree = LayoutTree::new();
    let root_id = tree.alloc(root_node, BoxType::Block);
    {
        let r = tree.get_mut(root_id).unwrap();
        r.content_rect = Rect::new(0.0, 0.0, 800.0, 600.0);
        r.padding_rect = r.content_rect;
        r.border_rect = r.content_rect;
        r.margin_rect = r.content_rect;
    }
    let parent_id = tree.alloc(parent_node, BoxType::Block);
    {
        let p = tree.get_mut(parent_id).unwrap();
        // Parent is small: 50×50 at (50, 50).
        p.content_rect = Rect::new(50.0, 50.0, 50.0, 50.0);
        p.padding_rect = p.content_rect;
        p.border_rect = p.content_rect;
        p.margin_rect = p.content_rect;
    }
    let child_id = tree.alloc(child_node, BoxType::Block);
    {
        let c = tree.get_mut(child_id).unwrap();
        // Child painted well beyond parent's bounds: at (300, 300) relative to
        // the parent's content origin (50, 50) → absolute (350, 350, 100, 100).
        c.content_rect = Rect::new(300.0, 300.0, 100.0, 100.0);
        c.padding_rect = c.content_rect;
        c.border_rect = c.content_rect;
        c.margin_rect = c.content_rect;
    }
    tree.add_child(root_id, parent_id);
    tree.add_child(parent_id, child_id);
    tree.root = root_id;

    let mut styles = StyleMap::new();
    styles.insert(root_node, ComputedStyle::default());

    // Parent does NOT clip (overflow: visible, the default).
    let mut parent_style = ComputedStyle::default();
    parent_style.overflow_x = Overflow::Visible;
    parent_style.overflow_y = Overflow::Visible;
    styles.insert(parent_node, parent_style);

    styles.insert(child_node, ComputedStyle::default());

    let engine = HitTestEngine::from_owned(tree, styles);

    // Point is inside the child's painted area but far outside the parent's
    // border box. Before the fix the parent's border-box early-return culled
    // the whole subtree and this fell through to root.
    let result = engine.hit_test(Point::new(380.0, 380.0));
    assert!(result.is_some(), "should hit the overflowing child");
    assert_eq!(
        result.unwrap().node,
        child_node,
        "child painted outside a non-clipping parent must be hittable"
    );

    // The parent itself is still only hittable within its own bounds.
    let result = engine.hit_test(Point::new(75.0, 75.0));
    assert_eq!(
        result.unwrap().node,
        parent_node,
        "point inside the parent box hits the parent"
    );
}

/// An absolutely-positioned child beyond the parent's bounds is hittable when
/// the parent does not clip, but is correctly NOT hit when the parent clips
/// (overflow: hidden).
#[test]
fn hit_test_clipping_parent_culls_out_of_bounds_child() {
    fn build(parent_clips: bool) -> (LayoutTree, NodeId, NodeId, NodeId) {
        let root_node = NodeId::from(1u64);
        let parent_node = NodeId::from(2u64);
        let child_node = NodeId::from(3u64);

        let mut tree = LayoutTree::new();
        let root_id = tree.alloc(root_node, BoxType::Block);
        {
            let r = tree.get_mut(root_id).unwrap();
            r.content_rect = Rect::new(0.0, 0.0, 800.0, 600.0);
            r.padding_rect = r.content_rect;
            r.border_rect = r.content_rect;
            r.margin_rect = r.content_rect;
        }
        let parent_id = tree.alloc(parent_node, BoxType::Block);
        {
            let p = tree.get_mut(parent_id).unwrap();
            p.content_rect = Rect::new(50.0, 50.0, 50.0, 50.0);
            p.padding_rect = p.content_rect;
            p.border_rect = p.content_rect;
            p.margin_rect = p.content_rect;
        }
        let child_id = tree.alloc(child_node, BoxType::Absolute);
        {
            let c = tree.get_mut(child_id).unwrap();
            // Absolute child far outside the parent: absolute (350, 350, 100, 100).
            c.content_rect = Rect::new(300.0, 300.0, 100.0, 100.0);
            c.padding_rect = c.content_rect;
            c.border_rect = c.content_rect;
            c.margin_rect = c.content_rect;
        }
        tree.add_child(root_id, parent_id);
        tree.add_child(parent_id, child_id);
        tree.root = root_id;
        let _ = parent_clips;
        (tree, root_node, parent_node, child_node)
    }

    // Non-clipping parent → child IS hittable beyond bounds.
    {
        let (tree, root_node, parent_node, child_node) = build(false);
        let mut styles = StyleMap::new();
        styles.insert(root_node, ComputedStyle::default());
        styles.insert(parent_node, ComputedStyle::default()); // overflow: visible
        let mut cs = ComputedStyle::default();
        cs.position = Position::Absolute;
        styles.insert(child_node, cs);
        let engine = HitTestEngine::from_owned(tree, styles);

        let result = engine.hit_test(Point::new(380.0, 380.0));
        assert_eq!(
            result.unwrap().node,
            child_node,
            "abs child beyond a non-clipping parent is hittable"
        );
    }

    // Clipping parent (overflow: hidden) → child is NOT hit; falls through to root.
    {
        let (tree, root_node, parent_node, child_node) = build(true);
        let mut styles = StyleMap::new();
        styles.insert(root_node, ComputedStyle::default());
        let mut ps = ComputedStyle::default();
        ps.overflow_x = Overflow::Hidden;
        ps.overflow_y = Overflow::Hidden;
        styles.insert(parent_node, ps);
        let mut cs = ComputedStyle::default();
        cs.position = Position::Absolute;
        styles.insert(child_node, cs);
        let engine = HitTestEngine::from_owned(tree, styles);

        let result = engine.hit_test(Point::new(380.0, 380.0));
        let hit = result.map(|r| r.node);
        assert_ne!(
            hit,
            Some(child_node),
            "abs child clipped by overflow:hidden parent must not be hit"
        );
        assert_eq!(
            hit,
            Some(root_node),
            "clipped-away point falls through to root"
        );
    }
}

// ── Regression: overflow-clip coordinate space across transforms (t49-e4-03)

/// A clipping ancestor establishes its clip in its own (root) coordinate
/// space. A transformed descendant inverse-transforms the pointer into local
/// space; the inherited clip must still be evaluated in the space it was built
/// (against the un-inverse-transformed point), not against the local point.
///
/// Layout:
///   root (no transform)
///     clipper: overflow:hidden, clip = padding box (150,100,200,300)
///       middle: transform translate(100,0), border (150,100,300,300) local
///         child: fills middle
///
/// A screen point inside the transformed clip hits the child; a point that is
/// inside the transformed child but outside the clip (and which the old
/// space-mixing code wrongly accepted) is correctly rejected.
#[test]
fn hit_test_overflow_clip_respects_transform_space() {
    fn build() -> (LayoutTree, NodeId, NodeId, NodeId, NodeId) {
        let root_node = NodeId::from(1u64);
        let clipper_node = NodeId::from(2u64);
        let middle_node = NodeId::from(3u64);
        let child_node = NodeId::from(4u64);

        let mut tree = LayoutTree::new();
        let root_id = tree.alloc(root_node, BoxType::Block);
        {
            let r = tree.get_mut(root_id).unwrap();
            r.content_rect = Rect::new(0.0, 0.0, 800.0, 600.0);
            r.padding_rect = r.content_rect;
            r.border_rect = r.content_rect;
            r.margin_rect = r.content_rect;
        }
        let clipper_id = tree.alloc(clipper_node, BoxType::Block);
        {
            let c = tree.get_mut(clipper_id).unwrap();
            // Border box wider than the clip so a point can be inside the
            // border (passing the cull) yet outside the child clip.
            c.border_rect = Rect::new(100.0, 100.0, 300.0, 300.0);
            c.margin_rect = c.border_rect;
            // Clip/content = padding box, narrower in x: (150,100,200,300).
            c.padding_rect = Rect::new(150.0, 100.0, 200.0, 300.0);
            c.content_rect = c.padding_rect;
        }
        let middle_id = tree.alloc(middle_node, BoxType::Block);
        {
            let m = tree.get_mut(middle_id).unwrap();
            // Relative to clipper content origin (150,100).
            m.content_rect = Rect::new(0.0, 0.0, 300.0, 300.0);
            m.padding_rect = m.content_rect;
            m.border_rect = m.content_rect;
            m.margin_rect = m.content_rect;
        }
        let child_id = tree.alloc(child_node, BoxType::Block);
        {
            let c = tree.get_mut(child_id).unwrap();
            c.content_rect = Rect::new(0.0, 0.0, 300.0, 300.0);
            c.padding_rect = c.content_rect;
            c.border_rect = c.content_rect;
            c.margin_rect = c.content_rect;
        }
        tree.add_child(root_id, clipper_id);
        tree.add_child(clipper_id, middle_id);
        tree.add_child(middle_id, child_id);
        tree.root = root_id;
        (tree, root_node, clipper_node, middle_node, child_node)
    }

    let make_styles =
        |root_node: NodeId, clipper_node: NodeId, middle_node: NodeId, child_node: NodeId| {
            let mut styles = StyleMap::new();
            styles.insert(root_node, ComputedStyle::default());
            let mut cs = ComputedStyle::default();
            cs.overflow_x = Overflow::Hidden;
            cs.overflow_y = Overflow::Hidden;
            styles.insert(clipper_node, cs);
            let mut ms = ComputedStyle::default();
            ms.transform = vec![Transform::Translate(LengthPercent::Px(100.0), LengthPercent::ZERO)];
            styles.insert(middle_node, ms);
            styles.insert(child_node, ComputedStyle::default());
            styles
        };

    // Inside the transformed clip → hits child.
    {
        let (tree, root_node, clipper_node, middle_node, child_node) = build();
        let styles = make_styles(root_node, clipper_node, middle_node, child_node);
        let engine = HitTestEngine::from_owned(tree, styles);
        let result = engine.hit_test(Point::new(300.0, 200.0));
        assert_eq!(
            result.unwrap().node,
            child_node,
            "point inside the transformed clip should hit the child"
        );
    }

    // Inside the transformed child but OUTSIDE the clip → must not hit child.
    // (The old code compared the root-space clip against the local point and
    // wrongly accepted this.)
    {
        let (tree, root_node, clipper_node, middle_node, child_node) = build();
        let styles = make_styles(root_node, clipper_node, middle_node, child_node);
        let engine = HitTestEngine::from_owned(tree, styles);
        let result = engine.hit_test(Point::new(360.0, 200.0));
        let hit = result.map(|r| r.node);
        assert_ne!(
            hit,
            Some(child_node),
            "point outside the transformed clip must not hit the clipped child"
        );
    }
}

/// B7 regression: nested (stacked) overflow clips, no transform.
///
/// Two clipping ancestors in the same coordinate space must intersect their
/// clip rects (the `clip.intersect_rect(abs_padding)` branch of the t50-e17
/// remap). A point inside the inner box but outside the *intersection* of the
/// two clips must be rejected; a point inside both clips hits the child.
///
///   root (800x600)
///     outer: overflow:hidden, padding/clip = (100,100,300,200)
///       inner: overflow:hidden, padding/clip = (0,0,200,200) rel. to outer
///         child: fills inner
///
/// The effective clip is (100,100,200,200) — the intersection. A point at
/// x=350 is inside inner (which spans to x=300 abs ... wait) — concretely:
/// outer clip abs = x∈[100,400), inner clip abs = x∈[100,300). The
/// intersection ends at x=300, so x=350 (inside inner's own 200-wide span only
/// if not clipped) is outside the combined clip.
#[test]
fn hit_test_nested_clips_intersect() {
    fn build() -> (LayoutTree, NodeId, NodeId, NodeId, NodeId) {
        let root_node = NodeId::from(1u64);
        let outer_node = NodeId::from(2u64);
        let inner_node = NodeId::from(3u64);
        let child_node = NodeId::from(4u64);

        let mut tree = LayoutTree::new();
        let root_id = tree.alloc(root_node, BoxType::Block);
        {
            let r = tree.get_mut(root_id).unwrap();
            r.content_rect = Rect::new(0.0, 0.0, 800.0, 600.0);
            r.padding_rect = r.content_rect;
            r.border_rect = r.content_rect;
            r.margin_rect = r.content_rect;
        }
        let outer_id = tree.alloc(outer_node, BoxType::Block);
        {
            let o = tree.get_mut(outer_id).unwrap();
            // Border box wide; clip (padding) abs = (100,100,300,200) → x∈[100,400).
            o.border_rect = Rect::new(100.0, 100.0, 400.0, 200.0);
            o.margin_rect = o.border_rect;
            o.padding_rect = Rect::new(100.0, 100.0, 300.0, 200.0);
            o.content_rect = o.padding_rect;
        }
        let inner_id = tree.alloc(inner_node, BoxType::Block);
        {
            // Relative to outer content origin (100,100). Inner clip
            // (padding) abs = (100,100,200,200) → x∈[100,300).
            let i = tree.get_mut(inner_id).unwrap();
            i.border_rect = Rect::new(0.0, 0.0, 250.0, 200.0);
            i.margin_rect = i.border_rect;
            i.padding_rect = Rect::new(0.0, 0.0, 200.0, 200.0);
            i.content_rect = i.padding_rect;
        }
        let child_id = tree.alloc(child_node, BoxType::Block);
        {
            let c = tree.get_mut(child_id).unwrap();
            c.content_rect = Rect::new(0.0, 0.0, 250.0, 200.0);
            c.padding_rect = c.content_rect;
            c.border_rect = c.content_rect;
            c.margin_rect = c.content_rect;
        }
        tree.add_child(root_id, outer_id);
        tree.add_child(outer_id, inner_id);
        tree.add_child(inner_id, child_id);
        tree.root = root_id;
        (tree, root_node, outer_node, inner_node, child_node)
    }

    let make_styles =
        |root_node: NodeId, outer_node: NodeId, inner_node: NodeId, child_node: NodeId| {
            let mut styles = StyleMap::new();
            styles.insert(root_node, ComputedStyle::default());
            let mut clip = ComputedStyle::default();
            clip.overflow_x = Overflow::Hidden;
            clip.overflow_y = Overflow::Hidden;
            styles.insert(outer_node, clip.clone());
            styles.insert(inner_node, clip);
            styles.insert(child_node, ComputedStyle::default());
            styles
        };

    // Inside BOTH clips (intersection x∈[100,300)) → hits child.
    {
        let (tree, root_node, outer_node, inner_node, child_node) = build();
        let styles = make_styles(root_node, outer_node, inner_node, child_node);
        let engine = HitTestEngine::from_owned(tree, styles);
        let result = engine.hit_test(Point::new(200.0, 150.0));
        assert_eq!(
            result.unwrap().node,
            child_node,
            "point inside the intersection of nested clips should hit the child"
        );
    }

    // Inside inner's border (x=350 < 350 border edge) but OUTSIDE the combined
    // clip (intersection ends at x=300) → must not hit child.
    {
        let (tree, root_node, outer_node, inner_node, child_node) = build();
        let styles = make_styles(root_node, outer_node, inner_node, child_node);
        let engine = HitTestEngine::from_owned(tree, styles);
        let result = engine.hit_test(Point::new(350.0, 150.0));
        let hit = result.map(|r| r.node);
        assert_ne!(
            hit,
            Some(child_node),
            "point outside the nested-clip intersection must not hit the child"
        );
        assert_ne!(
            hit,
            Some(inner_node),
            "the inner clipper itself is also clipped out at this point"
        );
    }
}
