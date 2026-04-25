//! Tests for the property tree system.

use crate::Rect;
use crate::clip_tree::{ClipTree, ClipType};
use crate::effect_tree::{BlendMode, EffectTree, FilterOp};
use crate::property_set::{NodeMapping, PropertyTreeSet};
use crate::scroll_tree::ScrollTree;
use crate::transform::Transform2D;
use crate::transform_tree::{ROOT_ID, TransformTree};

// ═════════════════════════════════════════════════════
//  Transform2D tests
// ═════════════════════════════════════════════════════

#[test]
fn transform2d_identity() {
    let t = Transform2D::identity();
    assert!(t.is_identity());
    assert!(t.is_translation_only());
    assert!(t.is_scale_translation());
    assert!((t.determinant() - 1.0).abs() < 1e-6);
}

#[test]
fn transform2d_translate() {
    let t = Transform2D::translate(10.0, 20.0);
    assert!(!t.is_identity());
    assert!(t.is_translation_only());
    let (x, y) = t.transform_point(5.0, 3.0);
    assert!((x - 15.0).abs() < 1e-6);
    assert!((y - 23.0).abs() < 1e-6);
}

#[test]
fn transform2d_scale() {
    let t = Transform2D::scale(2.0, 3.0);
    assert!(!t.is_identity());
    assert!(!t.is_translation_only());
    assert!(t.is_scale_translation());
    let (x, y) = t.transform_point(4.0, 5.0);
    assert!((x - 8.0).abs() < 1e-6);
    assert!((y - 15.0).abs() < 1e-6);
}

#[test]
fn transform2d_rotate_90() {
    let t = Transform2D::rotate(std::f32::consts::FRAC_PI_2);
    assert!(!t.is_identity());
    assert!(!t.is_translation_only());
    assert!(!t.is_scale_translation());
    let (x, y) = t.transform_point(1.0, 0.0);
    assert!(x.abs() < 1e-5);
    assert!((y - 1.0).abs() < 1e-5);
}

#[test]
fn transform2d_rotate_180() {
    let t = Transform2D::rotate(std::f32::consts::PI);
    let (x, y) = t.transform_point(1.0, 0.0);
    assert!((x - (-1.0)).abs() < 1e-5);
    assert!(y.abs() < 1e-5);
}

#[test]
fn transform2d_skew() {
    let t = Transform2D::skew(std::f32::consts::FRAC_PI_4, 0.0);
    let (x, y) = t.transform_point(0.0, 1.0);
    // skew_x = tan(pi/4) = 1.0, so point (0,1) -> (1,1)
    assert!((x - 1.0).abs() < 1e-5);
    assert!((y - 1.0).abs() < 1e-5);
}

#[test]
fn transform2d_multiply_identity() {
    let a = Transform2D::translate(5.0, 10.0);
    let id = Transform2D::identity();
    let result = a.multiply(&id);
    assert!((result.tx() - 5.0).abs() < 1e-6);
    assert!((result.ty() - 10.0).abs() < 1e-6);
}

#[test]
fn transform2d_multiply_compose() {
    // Scale then translate: first scale by 2, then translate by (10, 20)
    let scale = Transform2D::scale(2.0, 2.0);
    let translate = Transform2D::translate(10.0, 20.0);
    let composed = scale.multiply(&translate);
    let (x, y) = composed.transform_point(5.0, 3.0);
    // scale(5,3) = (10,6), then translate = (20, 26)
    assert!((x - 20.0).abs() < 1e-5);
    assert!((y - 26.0).abs() < 1e-5);
}

#[test]
fn transform2d_invert_identity() {
    let t = Transform2D::identity();
    let inv = t.invert().unwrap();
    assert!(inv.is_identity());
}

#[test]
fn transform2d_invert_translate() {
    let t = Transform2D::translate(10.0, 20.0);
    let inv = t.invert().unwrap();
    let (x, y) = inv.transform_point(15.0, 25.0);
    assert!((x - 5.0).abs() < 1e-5);
    assert!((y - 5.0).abs() < 1e-5);
}

#[test]
fn transform2d_invert_scale() {
    let t = Transform2D::scale(2.0, 4.0);
    let inv = t.invert().unwrap();
    let (x, y) = inv.transform_point(10.0, 20.0);
    assert!((x - 5.0).abs() < 1e-5);
    assert!((y - 5.0).abs() < 1e-5);
}

#[test]
fn transform2d_invert_singular() {
    let t = Transform2D::scale(0.0, 0.0);
    assert!(t.invert().is_none());
}

#[test]
fn transform2d_invert_roundtrip() {
    let t = Transform2D::new(1.5, 0.3, -0.7, 2.1, 100.0, -50.0);
    let inv = t.invert().unwrap();
    let composed = t.multiply(&inv);
    assert!(
        composed.is_identity() || {
            let (x, y) = composed.transform_point(42.0, 17.0);
            (x - 42.0).abs() < 1e-3 && (y - 17.0).abs() < 1e-3
        }
    );
}

#[test]
fn transform2d_transform_rect() {
    let t = Transform2D::translate(10.0, 20.0);
    let r = Rect::new(0.0, 0.0, 100.0, 50.0);
    let result = t.transform_rect(r);
    assert!((result.x - 10.0).abs() < 1e-5);
    assert!((result.y - 20.0).abs() < 1e-5);
    assert!((result.width - 100.0).abs() < 1e-5);
    assert!((result.height - 50.0).abs() < 1e-5);
}

#[test]
fn transform2d_transform_rect_scaled() {
    let t = Transform2D::scale(2.0, 3.0);
    let r = Rect::new(10.0, 10.0, 20.0, 30.0);
    let result = t.transform_rect(r);
    assert!((result.x - 20.0).abs() < 1e-5);
    assert!((result.y - 30.0).abs() < 1e-5);
    assert!((result.width - 40.0).abs() < 1e-5);
    assert!((result.height - 90.0).abs() < 1e-5);
}

#[test]
fn transform2d_transform_rect_rotated() {
    // 90-degree rotation of a (10x20) rect at origin
    let t = Transform2D::rotate(std::f32::consts::FRAC_PI_2);
    let r = Rect::new(0.0, 0.0, 10.0, 20.0);
    let result = t.transform_rect(r);
    // After 90-degree CCW rotation, AABB should be approximately (-20,0) to (0,10)
    assert!((result.width - 20.0).abs() < 1e-4);
    assert!((result.height - 10.0).abs() < 1e-4);
}

#[test]
fn transform2d_scale_factors() {
    let t = Transform2D::scale(3.0, 5.0);
    let (sx, sy) = t.scale_factors();
    assert!((sx - 3.0).abs() < 1e-5);
    assert!((sy - 5.0).abs() < 1e-5);
}

#[test]
fn transform2d_translation_extraction() {
    let t = Transform2D::translate(42.0, -7.0);
    let (tx, ty) = t.translation();
    assert!((tx - 42.0).abs() < 1e-6);
    assert!((ty - (-7.0)).abs() < 1e-6);
}

#[test]
fn transform2d_default_is_identity() {
    let t = Transform2D::default();
    assert!(t.is_identity());
}

#[test]
fn transform2d_pre_multiply() {
    let a = Transform2D::scale(2.0, 2.0);
    let b = Transform2D::translate(5.0, 5.0);
    // pre_multiply: apply b first, then a
    let result = a.pre_multiply(&b);
    let (x, y) = result.transform_point(0.0, 0.0);
    // translate(0,0) = (5,5), then scale = (10,10)
    assert!((x - 10.0).abs() < 1e-5);
    assert!((y - 10.0).abs() < 1e-5);
}

// ═════════════════════════════════════════════════════
//  TransformTree tests
// ═════════════════════════════════════════════════════

#[test]
fn transform_tree_root() {
    let tree = TransformTree::new();
    assert_eq!(tree.len(), 1);
    assert!(tree.is_empty());
    let root = tree.get(ROOT_ID).unwrap();
    assert!(root.local_transform.is_identity());
}

#[test]
fn transform_tree_add_child() {
    let mut tree = TransformTree::new();
    let child = tree.add(Some(ROOT_ID), Transform2D::translate(10.0, 0.0), true);
    assert_eq!(child, 1);
    assert_eq!(tree.len(), 2);
    assert!(!tree.is_empty());
}

#[test]
fn transform_tree_world_transform() {
    let mut tree = TransformTree::new();
    let a = tree.add(Some(ROOT_ID), Transform2D::translate(10.0, 0.0), true);
    let b = tree.add(Some(a), Transform2D::translate(0.0, 20.0), true);
    tree.update();

    let world_a = tree.world_transform(a);
    assert!((world_a.tx() - 10.0).abs() < 1e-5);
    assert!(world_a.ty().abs() < 1e-5);

    let world_b = tree.world_transform(b);
    assert!((world_b.tx() - 10.0).abs() < 1e-5);
    assert!((world_b.ty() - 20.0).abs() < 1e-5);
}

#[test]
fn transform_tree_dirty_propagation() {
    let mut tree = TransformTree::new();
    let a = tree.add(Some(ROOT_ID), Transform2D::translate(10.0, 0.0), true);
    let b = tree.add(Some(a), Transform2D::translate(0.0, 5.0), true);
    tree.update();

    // Modify parent
    tree.set_local_transform(a, Transform2D::translate(20.0, 0.0));
    assert!(tree.has_dirty());
    tree.update();

    let world_b = tree.world_transform(b);
    assert!((world_b.tx() - 20.0).abs() < 1e-5);
    assert!((world_b.ty() - 5.0).abs() < 1e-5);
}

#[test]
fn transform_tree_to_from_screen() {
    let mut tree = TransformTree::new();
    let node = tree.add(Some(ROOT_ID), Transform2D::translate(100.0, 200.0), true);
    tree.update();

    let (sx, sy) = tree.to_screen(node, 5.0, 10.0);
    assert!((sx - 105.0).abs() < 1e-5);
    assert!((sy - 210.0).abs() < 1e-5);

    let (lx, ly) = tree.from_screen(node, 105.0, 210.0).unwrap();
    assert!((lx - 5.0).abs() < 1e-5);
    assert!((ly - 10.0).abs() < 1e-5);
}

#[test]
fn transform_tree_screen_rect() {
    let mut tree = TransformTree::new();
    let node = tree.add(Some(ROOT_ID), Transform2D::translate(50.0, 50.0), true);
    tree.update();

    let local = Rect::new(0.0, 0.0, 100.0, 60.0);
    let screen = tree.screen_rect(node, local);
    assert!((screen.x - 50.0).abs() < 1e-5);
    assert!((screen.y - 50.0).abs() < 1e-5);
    assert!((screen.width - 100.0).abs() < 1e-5);
}

#[test]
fn transform_tree_clear() {
    let mut tree = TransformTree::new();
    tree.add(Some(ROOT_ID), Transform2D::translate(1.0, 2.0), true);
    tree.add(Some(ROOT_ID), Transform2D::translate(3.0, 4.0), true);
    assert_eq!(tree.len(), 3);
    tree.clear();
    assert_eq!(tree.len(), 1);
}

#[test]
fn transform_tree_children_of() {
    let mut tree = TransformTree::new();
    let a = tree.add(Some(ROOT_ID), Transform2D::identity(), true);
    let b = tree.add(Some(ROOT_ID), Transform2D::identity(), true);
    let _c = tree.add(Some(a), Transform2D::identity(), true);

    let root_children = tree.children_of(ROOT_ID);
    assert_eq!(root_children.len(), 2);
    assert!(root_children.contains(&a));
    assert!(root_children.contains(&b));

    let a_children = tree.children_of(a);
    assert_eq!(a_children.len(), 1);
}

#[test]
fn transform_tree_add_invalid_parent_falls_back_to_root() {
    let mut tree = TransformTree::new();
    let node = tree.add(Some(999), Transform2D::translate(3.0, 4.0), true);
    tree.update();

    let added = tree.get(node).unwrap();
    assert_eq!(added.parent, Some(ROOT_ID));

    let world = tree.world_transform(node);
    assert!((world.tx() - 3.0).abs() < 1e-5);
    assert!((world.ty() - 4.0).abs() < 1e-5);
}

#[test]
fn transform_tree_get_mut_normalizes_invalid_parent_and_marks_dirty() {
    let mut tree = TransformTree::new();
    let parent = tree.add(Some(ROOT_ID), Transform2D::translate(10.0, 0.0), true);
    let child = tree.add(Some(parent), Transform2D::translate(5.0, 0.0), true);
    tree.update();

    {
        let node = tree.get_mut(child).unwrap();
        node.parent = Some(999);
        node.local_transform = Transform2D::translate(20.0, 0.0);
    }

    assert!(tree.has_dirty());
    tree.update();

    let child_node = tree.get(child).unwrap();
    assert_eq!(child_node.parent, Some(ROOT_ID));
    assert!(tree.children_of(ROOT_ID).contains(&child));

    let world = tree.world_transform(child);
    assert!((world.tx() - 20.0).abs() < 1e-5);
}

// ═════════════════════════════════════════════════════
//  ClipTree tests
// ═════════════════════════════════════════════════════

#[test]
fn clip_tree_root() {
    let tree = ClipTree::new();
    assert_eq!(tree.len(), 1);
}

#[test]
fn clip_tree_add_and_accumulate() {
    let mut tree = ClipTree::new();
    // Set root clip to (0,0)-(100,100)
    tree.set_clip_rect(ROOT_ID, Rect::new(0.0, 0.0, 100.0, 100.0));
    // Child clip (50,50)-(150,150)
    let child = tree.add(
        Some(ROOT_ID),
        Rect::new(50.0, 50.0, 100.0, 100.0),
        ClipType::Rect,
    );
    tree.update();

    let acc = tree.accumulated_clip_rect(child).unwrap();
    assert!((acc.x - 50.0).abs() < 1e-5);
    assert!((acc.y - 50.0).abs() < 1e-5);
    assert!((acc.width - 50.0).abs() < 1e-5);
    assert!((acc.height - 50.0).abs() < 1e-5);
}

#[test]
fn clip_tree_nested_clips() {
    let mut tree = ClipTree::new();
    tree.set_clip_rect(ROOT_ID, Rect::new(0.0, 0.0, 200.0, 200.0));
    let a = tree.add(
        Some(ROOT_ID),
        Rect::new(10.0, 10.0, 100.0, 100.0),
        ClipType::Rect,
    );
    let b = tree.add(Some(a), Rect::new(30.0, 30.0, 50.0, 50.0), ClipType::Rect);
    tree.update();

    let acc = tree.accumulated_clip_rect(b).unwrap();
    assert!((acc.x - 30.0).abs() < 1e-5);
    assert!((acc.y - 30.0).abs() < 1e-5);
    assert!((acc.width - 50.0).abs() < 1e-5);
    assert!((acc.height - 50.0).abs() < 1e-5);
}

#[test]
fn clip_tree_fully_clipped() {
    let mut tree = ClipTree::new();
    tree.set_clip_rect(ROOT_ID, Rect::new(0.0, 0.0, 50.0, 50.0));
    // Child clip outside root
    let child = tree.add(
        Some(ROOT_ID),
        Rect::new(100.0, 100.0, 50.0, 50.0),
        ClipType::Rect,
    );
    tree.update();

    let acc = tree.accumulated_clip_rect(child).unwrap();
    assert!(acc.width <= 0.0 || acc.height <= 0.0);
    assert!(!tree.is_visible(child, Rect::new(110.0, 110.0, 10.0, 10.0)));
}

#[test]
fn clip_tree_is_visible() {
    let mut tree = ClipTree::new();
    tree.set_clip_rect(ROOT_ID, Rect::new(0.0, 0.0, 100.0, 100.0));
    let child = tree.add(
        Some(ROOT_ID),
        Rect::new(0.0, 0.0, 100.0, 100.0),
        ClipType::Rect,
    );
    tree.update();

    assert!(tree.is_visible(child, Rect::new(10.0, 10.0, 20.0, 20.0)));
    assert!(!tree.is_visible(child, Rect::new(200.0, 200.0, 10.0, 10.0)));
}

#[test]
fn clip_tree_clip_chain() {
    let mut tree = ClipTree::new();
    let a = tree.add(
        Some(ROOT_ID),
        Rect::new(10.0, 10.0, 80.0, 80.0),
        ClipType::Rect,
    );
    let b = tree.add(
        Some(a),
        Rect::new(20.0, 20.0, 40.0, 40.0),
        ClipType::RoundedRect {
            radii: (5.0, 5.0, 5.0, 5.0),
        },
    );

    let chain = tree.accumulated_clip(b);
    assert_eq!(chain.clips.len(), 3); // root + a + b
}

#[test]
fn clip_tree_clip_type_variants() {
    let mut tree = ClipTree::new();
    let _c1 = tree.add(
        Some(ROOT_ID),
        Rect::new(0.0, 0.0, 50.0, 50.0),
        ClipType::Rect,
    );
    let _c2 = tree.add(
        Some(ROOT_ID),
        Rect::new(0.0, 0.0, 50.0, 50.0),
        ClipType::RoundedRect {
            radii: (8.0, 8.0, 8.0, 8.0),
        },
    );
    let _c3 = tree.add(
        Some(ROOT_ID),
        Rect::new(0.0, 0.0, 50.0, 50.0),
        ClipType::CircleEllipse {
            cx: 25.0,
            cy: 25.0,
            rx: 25.0,
            ry: 25.0,
        },
    );
    let _c4 = tree.add(
        Some(ROOT_ID),
        Rect::new(0.0, 0.0, 50.0, 50.0),
        ClipType::Path(vec![(0.0, 0.0), (50.0, 0.0), (25.0, 50.0)]),
    );
    assert_eq!(tree.len(), 5); // root + 4 clips
}

#[test]
fn clip_tree_dirty_after_set() {
    let mut tree = ClipTree::new();
    let child = tree.add(
        Some(ROOT_ID),
        Rect::new(0.0, 0.0, 50.0, 50.0),
        ClipType::Rect,
    );
    tree.update();
    assert!(!tree.has_dirty());

    tree.set_clip_rect(child, Rect::new(10.0, 10.0, 40.0, 40.0));
    assert!(tree.has_dirty());
}

#[test]
fn clip_tree_add_invalid_parent_falls_back_to_root() {
    let mut tree = ClipTree::new();
    let node = tree.add(Some(999), Rect::new(0.0, 0.0, 10.0, 10.0), ClipType::Rect);
    tree.update();

    assert_eq!(tree.get(node).unwrap().parent, Some(ROOT_ID));
}

// ═════════════════════════════════════════════════════
//  EffectTree tests
// ═════════════════════════════════════════════════════

#[test]
fn effect_tree_root() {
    let tree = EffectTree::new();
    assert_eq!(tree.len(), 1);
    assert!((tree.accumulated_opacity(ROOT_ID) - 1.0).abs() < 1e-6);
}

#[test]
fn effect_tree_accumulated_opacity() {
    let mut tree = EffectTree::new();
    let a = tree.add(Some(ROOT_ID), 0.5, BlendMode::Normal, Vec::new(), false);
    let b = tree.add(Some(a), 0.5, BlendMode::Normal, Vec::new(), false);
    tree.update();

    assert!((tree.accumulated_opacity(a) - 0.5).abs() < 1e-6);
    assert!((tree.accumulated_opacity(b) - 0.25).abs() < 1e-6);
}

#[test]
fn effect_tree_needs_isolation_blend() {
    let mut tree = EffectTree::new();
    let node = tree.add(Some(ROOT_ID), 1.0, BlendMode::Multiply, Vec::new(), false);
    assert!(tree.needs_isolation(node));
}

#[test]
fn effect_tree_needs_isolation_filter() {
    let mut tree = EffectTree::new();
    let node = tree.add(
        Some(ROOT_ID),
        1.0,
        BlendMode::Normal,
        vec![FilterOp::Blur(5.0)],
        false,
    );
    assert!(tree.needs_isolation(node));
}

#[test]
fn effect_tree_needs_isolation_opacity() {
    let mut tree = EffectTree::new();
    let partial = tree.add(Some(ROOT_ID), 0.5, BlendMode::Normal, Vec::new(), false);
    let opaque = tree.add(Some(ROOT_ID), 1.0, BlendMode::Normal, Vec::new(), false);
    assert!(tree.needs_isolation(partial));
    assert!(!tree.needs_isolation(opaque));
}

#[test]
fn effect_tree_needs_isolation_explicit() {
    let mut tree = EffectTree::new();
    let node = tree.add(Some(ROOT_ID), 1.0, BlendMode::Normal, Vec::new(), true);
    assert!(tree.needs_isolation(node));
}

#[test]
fn effect_tree_set_opacity() {
    let mut tree = EffectTree::new();
    let node = tree.add(Some(ROOT_ID), 1.0, BlendMode::Normal, Vec::new(), false);
    tree.update();
    assert!((tree.accumulated_opacity(node) - 1.0).abs() < 1e-6);

    tree.set_opacity(node, 0.3);
    tree.update();
    assert!((tree.accumulated_opacity(node) - 0.3).abs() < 1e-6);
}

#[test]
fn effect_tree_blend_modes_all() {
    let modes = [
        BlendMode::Normal,
        BlendMode::Multiply,
        BlendMode::Screen,
        BlendMode::Overlay,
        BlendMode::Darken,
        BlendMode::Lighten,
        BlendMode::ColorDodge,
        BlendMode::ColorBurn,
        BlendMode::SoftLight,
        BlendMode::Difference,
        BlendMode::Exclusion,
        BlendMode::Hue,
        BlendMode::Saturation,
        BlendMode::Color,
        BlendMode::Luminosity,
    ];
    assert_eq!(modes.len(), 15);
}

#[test]
fn effect_tree_filter_ops_all() {
    let filters = vec![
        FilterOp::Blur(5.0),
        FilterOp::Brightness(1.2),
        FilterOp::Contrast(0.8),
        FilterOp::Grayscale(1.0),
        FilterOp::Sepia(0.5),
        FilterOp::Saturate(1.5),
        FilterOp::HueRotate(90.0),
        FilterOp::Invert(1.0),
        FilterOp::Opacity(0.5),
        FilterOp::DropShadow {
            dx: 2.0,
            dy: 4.0,
            blur: 8.0,
            color: [0, 0, 0, 128],
        },
    ];
    assert_eq!(filters.len(), 10);
}

#[test]
fn effect_tree_clear() {
    let mut tree = EffectTree::new();
    tree.add(Some(ROOT_ID), 0.5, BlendMode::Normal, Vec::new(), false);
    tree.add(Some(ROOT_ID), 0.8, BlendMode::Normal, Vec::new(), false);
    assert_eq!(tree.len(), 3);
    tree.clear();
    assert_eq!(tree.len(), 1);
}

#[test]
fn effect_tree_add_invalid_parent_falls_back_to_root() {
    let mut tree = EffectTree::new();
    let node = tree.add(Some(999), 1.0, BlendMode::Normal, Vec::new(), false);
    tree.update();

    assert_eq!(tree.get(node).unwrap().parent, Some(ROOT_ID));
}

#[test]
fn effect_tree_effect_mutations_mark_dirty() {
    let mut tree = EffectTree::new();
    let node = tree.add(Some(ROOT_ID), 1.0, BlendMode::Normal, Vec::new(), false);
    tree.update();
    assert!(!tree.has_dirty());

    tree.set_filters(node, vec![FilterOp::Blur(4.0)]);
    assert!(tree.has_dirty());
    tree.update();
    assert!(!tree.has_dirty());

    tree.set_blend_mode(node, BlendMode::Multiply);
    assert!(tree.has_dirty());
}

// ═════════════════════════════════════════════════════
//  ScrollTree tests
// ═════════════════════════════════════════════════════

#[test]
fn scroll_tree_root() {
    let tree = ScrollTree::new();
    assert_eq!(tree.len(), 1);
    assert_eq!(tree.accumulated_scroll(ROOT_ID), (0.0, 0.0));
}

#[test]
fn scroll_tree_set_offset() {
    let mut tree = ScrollTree::new();
    let node = tree.add(Some(ROOT_ID), (0.0, 0.0), (500.0, 1000.0), true);
    tree.set_scroll_offset(node, 100.0, 200.0);
    tree.update();

    let (dx, dy) = tree.accumulated_scroll(node);
    assert!((dx - 100.0).abs() < 1e-5);
    assert!((dy - 200.0).abs() < 1e-5);
}

#[test]
fn scroll_tree_clamp_offset() {
    let mut tree = ScrollTree::new();
    let node = tree.add(Some(ROOT_ID), (0.0, 0.0), (100.0, 200.0), true);
    tree.set_scroll_offset(node, 999.0, -50.0);
    tree.update();

    let (dx, dy) = tree.accumulated_scroll(node);
    assert!((dx - 100.0).abs() < 1e-5); // Clamped to max
    assert!(dy.abs() < 1e-5); // Clamped to 0
}

#[test]
fn scroll_tree_accumulated_nested() {
    let mut tree = ScrollTree::new();
    let a = tree.add(Some(ROOT_ID), (0.0, 0.0), (500.0, 500.0), true);
    let b = tree.add(Some(a), (0.0, 0.0), (300.0, 300.0), true);
    tree.set_scroll_offset(a, 10.0, 20.0);
    tree.set_scroll_offset(b, 5.0, 15.0);
    tree.update();

    let (dx, dy) = tree.accumulated_scroll(b);
    assert!((dx - 15.0).abs() < 1e-5);
    assert!((dy - 35.0).abs() < 1e-5);
}

#[test]
fn scroll_tree_scroll_into_view_already_visible() {
    let mut tree = ScrollTree::new();
    let node = tree.add(Some(ROOT_ID), (50.0, 50.0), (500.0, 500.0), true);
    // Target is within the current visible area
    let (new_dx, new_dy) = tree.scroll_into_view(node, (60.0, 60.0, 20.0, 20.0));
    assert!((new_dx - 50.0).abs() < 1e-5);
    assert!((new_dy - 50.0).abs() < 1e-5);
}

#[test]
fn scroll_tree_scroll_into_view_above() {
    let mut tree = ScrollTree::new();
    let node = tree.add(Some(ROOT_ID), (0.0, 100.0), (500.0, 500.0), true);
    // Target is above the current scroll position
    let (_, new_dy) = tree.scroll_into_view(node, (0.0, 50.0, 20.0, 20.0));
    assert!((new_dy - 50.0).abs() < 1e-5);
}

#[test]
fn scroll_tree_clear() {
    let mut tree = ScrollTree::new();
    tree.add(Some(ROOT_ID), (0.0, 0.0), (100.0, 100.0), true);
    assert_eq!(tree.len(), 2);
    tree.clear();
    assert_eq!(tree.len(), 1);
}

#[test]
fn scroll_tree_add_invalid_parent_falls_back_to_root() {
    let mut tree = ScrollTree::new();
    let node = tree.add(Some(999), (0.0, 0.0), (100.0, 100.0), true);
    tree.update();

    assert_eq!(tree.get(node).unwrap().parent, Some(ROOT_ID));
}

#[test]
fn scroll_tree_scroll_into_view_uses_explicit_viewport_size() {
    let mut tree = ScrollTree::new();
    let node = tree.add(Some(ROOT_ID), (50.0, 50.0), (400.0, 400.0), true);
    tree.set_viewport_size(node, 100.0, 80.0);

    let (new_dx, new_dy) = tree.scroll_into_view(node, (170.0, 140.0, 40.0, 30.0));
    assert!((new_dx - 110.0).abs() < 1e-5);
    assert!((new_dy - 90.0).abs() < 1e-5);
}

// ═════════════════════════════════════════════════════
//  PropertyTreeSet tests
// ═════════════════════════════════════════════════════

#[test]
fn property_set_new() {
    let set = PropertyTreeSet::new();
    assert_eq!(set.element_count(), 0);
    assert_eq!(set.total_tree_nodes(), 4); // 1 root per tree
}

#[test]
fn property_set_add_element() {
    let mut set = PropertyTreeSet::new();
    let elem = set.add_element(NodeMapping::default(), Rect::new(0.0, 0.0, 100.0, 50.0));
    assert_eq!(elem, 0);
    assert_eq!(set.element_count(), 1);
    let mapping = set.mapping(elem).unwrap();
    assert_eq!(mapping.transform_id, ROOT_ID);
}

#[test]
fn property_set_map_point_identity() {
    let mut set = PropertyTreeSet::new();
    let elem = set.add_element(NodeMapping::default(), Rect::new(0.0, 0.0, 100.0, 100.0));
    set.update();

    let (sx, sy) = set.map_point_to_screen(elem, (50.0, 25.0));
    assert!((sx - 50.0).abs() < 1e-5);
    assert!((sy - 25.0).abs() < 1e-5);
}

#[test]
fn property_set_map_point_with_transform() {
    let mut set = PropertyTreeSet::new();
    let t_id = set
        .transform_tree
        .add(Some(ROOT_ID), Transform2D::translate(100.0, 200.0), true);
    let mapping = NodeMapping {
        transform_id: t_id,
        ..Default::default()
    };
    let elem = set.add_element(mapping, Rect::new(0.0, 0.0, 50.0, 50.0));
    set.update();

    let (sx, sy) = set.map_point_to_screen(elem, (10.0, 10.0));
    assert!((sx - 110.0).abs() < 1e-5);
    assert!((sy - 210.0).abs() < 1e-5);
}

#[test]
fn property_set_map_point_with_scroll() {
    let mut set = PropertyTreeSet::new();
    let s_id = set
        .scroll_tree
        .add(Some(ROOT_ID), (0.0, 0.0), (500.0, 500.0), true);
    set.scroll_tree.set_scroll_offset(s_id, 30.0, 60.0);
    let mapping = NodeMapping {
        scroll_id: s_id,
        ..Default::default()
    };
    let elem = set.add_element(mapping, Rect::new(0.0, 0.0, 100.0, 100.0));
    set.update();

    let (sx, sy) = set.map_point_to_screen(elem, (50.0, 50.0));
    assert!((sx - 20.0).abs() < 1e-5); // 50 - 30
    assert!((sy - (-10.0)).abs() < 1e-5); // 50 - 60
}

#[test]
fn property_set_hit_test() {
    let mut set = PropertyTreeSet::new();
    let elem0 = set.add_element(NodeMapping::default(), Rect::new(0.0, 0.0, 100.0, 100.0));
    let elem1 = set.add_element(NodeMapping::default(), Rect::new(200.0, 200.0, 50.0, 50.0));
    set.update();

    let hits = set.map_point_from_screen((50.0, 50.0));
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].0, elem0);

    let hits = set.map_point_from_screen((225.0, 225.0));
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].0, elem1);

    let hits = set.map_point_from_screen((150.0, 150.0));
    assert_eq!(hits.len(), 0);
}

#[test]
fn property_set_visible_rect() {
    let mut set = PropertyTreeSet::new();
    let c_id = set.clip_tree.add(
        Some(ROOT_ID),
        Rect::new(0.0, 0.0, 50.0, 50.0),
        ClipType::Rect,
    );
    set.clip_tree
        .set_clip_rect(ROOT_ID, Rect::new(0.0, 0.0, 1000.0, 1000.0));
    let mapping = NodeMapping {
        clip_id: c_id,
        ..Default::default()
    };
    let elem = set.add_element(mapping, Rect::new(0.0, 0.0, 100.0, 100.0));
    set.update();

    let visible = set.visible_rect(elem);
    assert!(visible.is_some());
    let v = visible.unwrap();
    // Element (0,0,100,100) clipped to (0,0,50,50) => visible is (0,0,50,50)
    assert!((v.width - 50.0).abs() < 1e-5);
    assert!((v.height - 50.0).abs() < 1e-5);
}

#[test]
fn property_set_visible_rect_fully_clipped() {
    let mut set = PropertyTreeSet::new();
    let c_id = set.clip_tree.add(
        Some(ROOT_ID),
        Rect::new(200.0, 200.0, 50.0, 50.0),
        ClipType::Rect,
    );
    set.clip_tree
        .set_clip_rect(ROOT_ID, Rect::new(0.0, 0.0, 1000.0, 1000.0));
    let mapping = NodeMapping {
        clip_id: c_id,
        ..Default::default()
    };
    let elem = set.add_element(mapping, Rect::new(0.0, 0.0, 100.0, 100.0));
    set.update();

    let visible = set.visible_rect(elem);
    assert!(visible.is_none());
}

#[test]
fn property_set_visible_rect_transforms_clipped_local_bounds() {
    let mut set = PropertyTreeSet::new();
    let t_id = set
        .transform_tree
        .add(Some(ROOT_ID), Transform2D::translate(100.0, 100.0), true);
    let c_id = set.clip_tree.add(
        Some(ROOT_ID),
        Rect::new(0.0, 0.0, 50.0, 50.0),
        ClipType::Rect,
    );
    set.clip_tree
        .set_clip_rect(ROOT_ID, Rect::new(-1000.0, -1000.0, 2000.0, 2000.0));
    let mapping = NodeMapping {
        transform_id: t_id,
        clip_id: c_id,
        ..Default::default()
    };
    let elem = set.add_element(mapping, Rect::new(0.0, 0.0, 100.0, 100.0));
    set.update();

    let visible = set.visible_rect(elem).unwrap();
    assert!((visible.x - 100.0).abs() < 1e-5);
    assert!((visible.y - 100.0).abs() < 1e-5);
    assert!((visible.width - 50.0).abs() < 1e-5);
    assert!((visible.height - 50.0).abs() < 1e-5);
}

#[test]
fn property_set_damage_rect_basic() {
    let mut set = PropertyTreeSet::new();
    let t_id = set
        .transform_tree
        .add(Some(ROOT_ID), Transform2D::translate(50.0, 50.0), true);
    let mapping = NodeMapping {
        transform_id: t_id,
        ..Default::default()
    };
    let elem = set.add_element(mapping, Rect::new(0.0, 0.0, 80.0, 60.0));
    set.update();

    let damage = set.damage_rect(elem);
    assert!((damage.x - 50.0).abs() < 1e-5);
    assert!((damage.y - 50.0).abs() < 1e-5);
    assert!((damage.width - 80.0).abs() < 1e-5);
    assert!((damage.height - 60.0).abs() < 1e-5);
}

#[test]
fn property_set_damage_rect_with_blur() {
    let mut set = PropertyTreeSet::new();
    let e_id = set.effect_tree.add(
        Some(ROOT_ID),
        1.0,
        BlendMode::Normal,
        vec![FilterOp::Blur(10.0)],
        false,
    );
    let mapping = NodeMapping {
        effect_id: e_id,
        ..Default::default()
    };
    let elem = set.add_element(mapping, Rect::new(0.0, 0.0, 100.0, 100.0));
    set.update();

    let damage = set.damage_rect(elem);
    // Blur should expand by 30px (3 * radius) on each side
    assert!(damage.x < 0.0);
    assert!(damage.y < 0.0);
    assert!(damage.width > 100.0);
    assert!(damage.height > 100.0);
}

#[test]
fn property_set_damage_rect_includes_ancestor_filters() {
    let mut set = PropertyTreeSet::new();
    let ancestor = set.effect_tree.add(
        Some(ROOT_ID),
        1.0,
        BlendMode::Normal,
        vec![FilterOp::Blur(5.0)],
        false,
    );
    let child = set
        .effect_tree
        .add(Some(ancestor), 1.0, BlendMode::Normal, Vec::new(), false);
    let mapping = NodeMapping {
        effect_id: child,
        ..Default::default()
    };
    let elem = set.add_element(mapping, Rect::new(0.0, 0.0, 100.0, 100.0));
    set.update();

    let damage = set.damage_rect(elem);
    assert!((damage.x - (-15.0)).abs() < 1e-5);
    assert!((damage.y - (-15.0)).abs() < 1e-5);
    assert!((damage.width - 130.0).abs() < 1e-5);
    assert!((damage.height - 130.0).abs() < 1e-5);
}

#[test]
fn property_set_hit_test_respects_transformed_clip() {
    let mut set = PropertyTreeSet::new();
    let t_id = set
        .transform_tree
        .add(Some(ROOT_ID), Transform2D::translate(100.0, 0.0), true);
    let c_id = set.clip_tree.add(
        Some(ROOT_ID),
        Rect::new(0.0, 0.0, 50.0, 50.0),
        ClipType::Rect,
    );
    set.clip_tree
        .set_clip_rect(ROOT_ID, Rect::new(-1000.0, -1000.0, 2000.0, 2000.0));
    let mapping = NodeMapping {
        transform_id: t_id,
        clip_id: c_id,
        ..Default::default()
    };
    let elem = set.add_element(mapping, Rect::new(0.0, 0.0, 100.0, 100.0));
    set.update();

    let hits = set.map_point_from_screen((125.0, 25.0));
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].0, elem);

    let hits = set.map_point_from_screen((175.0, 25.0));
    assert!(hits.is_empty());
}

#[test]
fn property_set_hit_test_skips_fully_transparent_effect_chains() {
    let mut set = PropertyTreeSet::new();
    let e_id = set
        .effect_tree
        .add(Some(ROOT_ID), 0.0, BlendMode::Normal, Vec::new(), false);
    let mapping = NodeMapping {
        effect_id: e_id,
        ..Default::default()
    };
    set.add_element(mapping, Rect::new(0.0, 0.0, 100.0, 100.0));
    set.update();

    let hits = set.map_point_from_screen((10.0, 10.0));
    assert!(hits.is_empty());
}

#[test]
fn property_set_clear() {
    let mut set = PropertyTreeSet::new();
    set.transform_tree
        .add(Some(ROOT_ID), Transform2D::identity(), true);
    set.add_element(NodeMapping::default(), Rect::new(0.0, 0.0, 10.0, 10.0));
    set.clear();
    assert_eq!(set.element_count(), 0);
    assert_eq!(set.transform_tree.len(), 1);
}

#[test]
fn property_set_update_all_trees() {
    let mut set = PropertyTreeSet::new();
    let t_id = set
        .transform_tree
        .add(Some(ROOT_ID), Transform2D::translate(5.0, 5.0), true);
    let c_id = set.clip_tree.add(
        Some(ROOT_ID),
        Rect::new(0.0, 0.0, 200.0, 200.0),
        ClipType::Rect,
    );
    set.clip_tree
        .set_clip_rect(ROOT_ID, Rect::new(0.0, 0.0, 1000.0, 1000.0));
    let e_id = set
        .effect_tree
        .add(Some(ROOT_ID), 0.8, BlendMode::Normal, Vec::new(), false);
    let s_id = set
        .scroll_tree
        .add(Some(ROOT_ID), (0.0, 0.0), (100.0, 100.0), true);

    let mapping = NodeMapping {
        transform_id: t_id,
        clip_id: c_id,
        effect_id: e_id,
        scroll_id: s_id,
    };
    let elem = set.add_element(mapping, Rect::new(0.0, 0.0, 50.0, 50.0));
    set.update();

    // Verify all trees computed correctly
    let world = set.transform_tree.world_transform(t_id);
    assert!((world.tx() - 5.0).abs() < 1e-5);

    let acc_clip = set.clip_tree.accumulated_clip_rect(c_id);
    assert!(acc_clip.is_some());

    let acc_opacity = set.effect_tree.accumulated_opacity(e_id);
    assert!((acc_opacity - 0.8).abs() < 1e-5);

    let acc_scroll = set.scroll_tree.accumulated_scroll(s_id);
    assert_eq!(acc_scroll, (0.0, 0.0));

    let visible = set.visible_rect(elem);
    assert!(visible.is_some());
}

// ═════════════════════════════════════════════════════
//  Rect tests
// ═════════════════════════════════════════════════════

#[test]
fn rect_basic() {
    let r = Rect::new(10.0, 20.0, 30.0, 40.0);
    assert!((r.right() - 40.0).abs() < 1e-6);
    assert!((r.bottom() - 60.0).abs() < 1e-6);
    assert!((r.area() - 1200.0).abs() < 1e-3);
}

#[test]
fn rect_contains() {
    let r = Rect::new(0.0, 0.0, 100.0, 100.0);
    assert!(r.contains(50.0, 50.0));
    assert!(r.contains(0.0, 0.0));
    assert!(!r.contains(100.0, 100.0)); // exclusive upper bound
    assert!(!r.contains(-1.0, 50.0));
}

#[test]
fn rect_intersects() {
    let a = Rect::new(0.0, 0.0, 50.0, 50.0);
    let b = Rect::new(25.0, 25.0, 50.0, 50.0);
    let c = Rect::new(100.0, 100.0, 50.0, 50.0);
    assert!(a.intersects(&b));
    assert!(!a.intersects(&c));
}

#[test]
fn rect_intersection() {
    let a = Rect::new(0.0, 0.0, 50.0, 50.0);
    let b = Rect::new(25.0, 25.0, 50.0, 50.0);
    let i = a.intersection(&b).unwrap();
    assert!((i.x - 25.0).abs() < 1e-6);
    assert!((i.y - 25.0).abs() < 1e-6);
    assert!((i.width - 25.0).abs() < 1e-6);
    assert!((i.height - 25.0).abs() < 1e-6);
}

#[test]
fn rect_union() {
    let a = Rect::new(0.0, 0.0, 50.0, 50.0);
    let b = Rect::new(25.0, 25.0, 50.0, 50.0);
    let u = a.union(&b);
    assert!(u.x.abs() < 1e-6);
    assert!(u.y.abs() < 1e-6);
    assert!((u.width - 75.0).abs() < 1e-6);
    assert!((u.height - 75.0).abs() < 1e-6);
}

#[test]
fn rect_expand() {
    let r = Rect::new(10.0, 10.0, 20.0, 20.0);
    let expanded = r.expand(5.0);
    assert!((expanded.x - 5.0).abs() < 1e-6);
    assert!((expanded.y - 5.0).abs() < 1e-6);
    assert!((expanded.width - 30.0).abs() < 1e-6);
    assert!((expanded.height - 30.0).abs() < 1e-6);
}
