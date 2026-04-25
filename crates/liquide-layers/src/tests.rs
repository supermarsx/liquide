//! Tests for the liquide-layers crate.

#[cfg(test)]
mod layer_tests {
    use crate::layer::*;

    #[test]
    fn rect_intersection() {
        let a = Rect::new(0.0, 0.0, 100.0, 100.0);
        let b = Rect::new(50.0, 50.0, 100.0, 100.0);
        let i = a.intersection(&b).unwrap();
        assert_eq!(i, Rect::new(50.0, 50.0, 50.0, 50.0));
    }

    #[test]
    fn rect_no_intersection() {
        let a = Rect::new(0.0, 0.0, 50.0, 50.0);
        let b = Rect::new(100.0, 100.0, 50.0, 50.0);
        assert!(a.intersection(&b).is_none());
    }

    #[test]
    fn rect_union() {
        let a = Rect::new(10.0, 20.0, 30.0, 40.0);
        let b = Rect::new(50.0, 60.0, 70.0, 80.0);
        let u = a.union(&b);
        assert_eq!(u, Rect::new(10.0, 20.0, 110.0, 120.0));
    }

    #[test]
    fn rect_contains_rect() {
        let outer = Rect::new(0.0, 0.0, 200.0, 200.0);
        let inner = Rect::new(10.0, 10.0, 50.0, 50.0);
        assert!(outer.contains_rect(&inner));
        assert!(!inner.contains_rect(&outer));
    }

    #[test]
    fn rect_is_empty() {
        assert!(Rect::new(0.0, 0.0, 0.0, 100.0).is_empty());
        assert!(Rect::new(0.0, 0.0, 100.0, 0.0).is_empty());
        assert!(!Rect::new(0.0, 0.0, 1.0, 1.0).is_empty());
    }

    #[test]
    fn layer_new_is_dirty() {
        let layer = Layer::new(1, Rect::new(0.0, 0.0, 100.0, 100.0), PromotionReason::Root);
        assert!(layer.is_dirty);
        assert!(!layer.has_valid_cache());
    }

    #[test]
    fn layer_mark_clean() {
        let mut layer = Layer::new(1, Rect::new(0.0, 0.0, 100.0, 100.0), PromotionReason::Root);
        layer.pixels = Some(vec![0u8; 100 * 100 * 4]);
        layer.mark_clean();
        assert!(!layer.is_dirty);
        assert!(layer.has_valid_cache());
    }

    #[test]
    fn layer_pixel_buffer_size() {
        let layer = Layer::new(
            1,
            Rect::new(0.0, 0.0, 64.0, 32.0),
            PromotionReason::Explicit,
        );
        assert_eq!(layer.pixel_buffer_size(), 64 * 32 * 4);
    }

    #[test]
    fn layer_identity_transform() {
        let layer = Layer::new(1, Rect::ZERO, PromotionReason::Root);
        assert!(layer.is_identity_transform());
    }

    #[test]
    fn layer_non_identity_transform() {
        let mut layer = Layer::new(1, Rect::ZERO, PromotionReason::Root);
        layer.transform = [2.0, 0.0, 0.0, 2.0, 10.0, 20.0]; // scale 2x + translate
        assert!(!layer.is_identity_transform());
    }

    #[test]
    fn layer_transform_point() {
        let mut layer = Layer::new(1, Rect::ZERO, PromotionReason::Root);
        layer.transform = [1.0, 0.0, 0.0, 1.0, 10.0, 20.0]; // translate (10, 20)
        let (x, y) = layer.transform_point(5.0, 5.0);
        assert!((x - 15.0).abs() < f32::EPSILON);
        assert!((y - 25.0).abs() < f32::EPSILON);
    }

    #[test]
    fn layer_is_opaque() {
        let mut layer = Layer::new(1, Rect::ZERO, PromotionReason::Root);
        assert!(layer.is_opaque());
        layer.opacity = 0.5;
        assert!(!layer.is_opaque());
    }

    #[test]
    fn blend_mode_default() {
        assert_eq!(BlendMode::default(), BlendMode::SrcOver);
    }

    #[test]
    fn layer_extension_fields_default_none() {
        let layer = Layer::new(1, Rect::new(0.0, 0.0, 100.0, 100.0), PromotionReason::Root);
        assert!(layer.filter.is_none());
        assert!(layer.backdrop_filter.is_none());
        assert!(layer.mask.is_none());
        assert!(layer.clip_path.is_none());
        assert!(!layer.isolation);
    }

    #[test]
    fn filter_chain_is_identity_when_empty() {
        let fc = FilterChain::default();
        assert!(fc.is_identity());
    }
}

#[cfg(test)]
mod occlusion_tests {
    use crate::compositor::LayerCompositor;
    use crate::layer::*;
    use crate::tree::LayerTree;

    /// Regression test for t8 §3.5 "occlusion walk".
    ///
    /// Builds a tree with two sibling layers covering identical bounds,
    /// where the higher-z child is fully opaque and has valid pixel
    /// data. The rear sibling (lower z, same bounds) must be marked
    /// occluded and skipped — only the front layer should be drawn.
    #[test]
    fn opaque_front_layer_culls_rear_sibling() {
        let mut tree = LayerTree::new(Rect::new(0.0, 0.0, 100.0, 100.0));

        // Rear layer (lower z, opaque pixels, covers viewport).
        let rear = tree.create_layer(Rect::new(0.0, 0.0, 100.0, 100.0), PromotionReason::Explicit);
        {
            let l = tree.get_mut(rear).unwrap();
            l.pixels = Some(vec![255u8; 100 * 100 * 4]);
            l.mark_clean();
            l.z_order = 0;
            l.opacity = 1.0;
            l.blend_mode = BlendMode::SrcOver;
        }

        // Front layer (higher z, opaque pixels, identical bounds).
        let front = tree.create_layer(Rect::new(0.0, 0.0, 100.0, 100.0), PromotionReason::Explicit);
        {
            let l = tree.get_mut(front).unwrap();
            l.pixels = Some(vec![255u8; 100 * 100 * 4]);
            l.mark_clean();
            l.z_order = 1;
            l.opacity = 1.0;
            l.blend_mode = BlendMode::SrcOver;
        }

        let mut output = vec![0u8; 100 * 100 * 4];
        let mut compositor = LayerCompositor::new();
        let stats = compositor.composite(&tree, &mut output, 100, 100);

        // The rear sibling must be culled (covered by front opaque rect).
        assert!(
            stats.occluded >= 1,
            "expected rear layer to be occluded, got stats: {stats:?}"
        );
        // And the compositor should have drawn at most the front layer
        // (plus the root if it had pixels — it doesn't in this test).
        assert!(
            stats.drawn <= 1,
            "expected at most 1 draw, got {} (stats: {stats:?})",
            stats.drawn
        );
    }

    /// Sanity: a transparent front layer must NOT occlude the rear
    /// (opacity < 1 means per-pixel alpha can show through).
    #[test]
    fn transparent_front_layer_does_not_cull_rear() {
        let mut tree = LayerTree::new(Rect::new(0.0, 0.0, 100.0, 100.0));
        let rear = tree.create_layer(Rect::new(0.0, 0.0, 100.0, 100.0), PromotionReason::Explicit);
        let l = tree.get_mut(rear).unwrap();
        l.pixels = Some(vec![255u8; 100 * 100 * 4]);
        l.mark_clean();
        l.z_order = 0;

        let front = tree.create_layer(Rect::new(0.0, 0.0, 100.0, 100.0), PromotionReason::Explicit);
        let l = tree.get_mut(front).unwrap();
        l.pixels = Some(vec![255u8; 100 * 100 * 4]);
        l.mark_clean();
        l.z_order = 1;
        l.opacity = 0.5; // translucent

        let mut output = vec![0u8; 100 * 100 * 4];
        let mut compositor = LayerCompositor::new();
        let stats = compositor.composite(&tree, &mut output, 100, 100);
        // Root has no pixels so it is always skipped; the two children
        // should both be composited (front is translucent, so it cannot
        // occlude the rear).
        assert_eq!(
            stats.drawn, 2,
            "rear + translucent front should both draw (stats: {stats:?})"
        );
    }
}

#[cfg(test)]
mod update_tests {
    use crate::layer::*;
    use crate::promote::{ElementInfo, LayerPromotionHeuristics};
    use crate::tree::LayerTree;
    use std::collections::HashMap;

    #[test]
    fn update_returns_no_demotable_while_animating() {
        let mut tree = LayerTree::new(Rect::new(0.0, 0.0, 100.0, 100.0));
        let id = tree.create_layer(
            Rect::new(0.0, 0.0, 50.0, 50.0),
            PromotionReason::HasTransform,
        );
        // Pre-age the layer well past the demotion threshold.
        tree.get_mut(id).unwrap().frames_since_dirty = 10_000;

        let mut signals = HashMap::new();
        signals.insert(
            id,
            ElementInfo {
                animation_active: true,
                has_transform: true,
                ..Default::default()
            },
        );

        let demotable = tree.update(&LayerPromotionHeuristics::default(), &signals);
        assert!(
            demotable.is_empty(),
            "animating layer should not be demoted, got {demotable:?}"
        );
    }

    #[test]
    fn update_returns_demotable_when_idle() {
        let mut tree = LayerTree::new(Rect::new(0.0, 0.0, 100.0, 100.0));
        let id = tree.create_layer(Rect::new(0.0, 0.0, 50.0, 50.0), PromotionReason::Explicit);
        tree.get_mut(id).unwrap().frames_since_dirty = 10_000;

        let signals: HashMap<_, _> = HashMap::new();
        let demotable = tree.update(&LayerPromotionHeuristics::default(), &signals);
        assert_eq!(demotable, vec![id]);
    }
}

#[cfg(test)]
mod tree_tests {
    use crate::layer::*;
    use crate::tree::LayerTree;

    #[test]
    fn tree_creation() {
        let tree = LayerTree::new(Rect::new(0.0, 0.0, 1920.0, 1080.0));
        assert_eq!(tree.len(), 1);
        assert!(tree.get(tree.root).is_some());
    }

    #[test]
    fn create_layer_under_root() {
        let mut tree = LayerTree::new(Rect::new(0.0, 0.0, 1920.0, 1080.0));
        let child = tree.create_layer(
            Rect::new(100.0, 100.0, 200.0, 200.0),
            PromotionReason::HasTransform,
        );
        assert_eq!(tree.len(), 2);
        assert_eq!(tree.children_of(tree.root).len(), 1);
        assert_eq!(tree.children_of(tree.root)[0], child);
        assert_eq!(tree.parent(child), Some(tree.root));
    }

    #[test]
    fn create_nested_layers() {
        let mut tree = LayerTree::new(Rect::new(0.0, 0.0, 1920.0, 1080.0));
        let parent = tree.create_layer(
            Rect::new(0.0, 0.0, 500.0, 500.0),
            PromotionReason::ScrollingContent,
        );
        let child = tree.create_layer_under(
            parent,
            Rect::new(10.0, 10.0, 100.0, 100.0),
            PromotionReason::HasOpacity,
        );
        assert_eq!(tree.len(), 3);
        assert_eq!(tree.parent(child), Some(parent));
        assert_eq!(tree.children_of(parent), &[child]);
    }

    #[test]
    fn remove_layer() {
        let mut tree = LayerTree::new(Rect::new(0.0, 0.0, 1920.0, 1080.0));
        let child = tree.create_layer(Rect::new(0.0, 0.0, 100.0, 100.0), PromotionReason::Explicit);
        assert_eq!(tree.len(), 2);
        tree.remove_layer(child);
        assert_eq!(tree.len(), 1);
        assert!(tree.children_of(tree.root).is_empty());
    }

    #[test]
    fn remove_subtree() {
        let mut tree = LayerTree::new(Rect::new(0.0, 0.0, 1920.0, 1080.0));
        let parent = tree.create_layer(
            Rect::new(0.0, 0.0, 500.0, 500.0),
            PromotionReason::ScrollingContent,
        );
        let _child1 = tree.create_layer_under(
            parent,
            Rect::new(0.0, 0.0, 100.0, 100.0),
            PromotionReason::Explicit,
        );
        let _child2 = tree.create_layer_under(
            parent,
            Rect::new(0.0, 0.0, 100.0, 100.0),
            PromotionReason::Explicit,
        );
        assert_eq!(tree.len(), 4);
        tree.remove_layer(parent);
        assert_eq!(tree.len(), 1); // only root remains
    }

    #[test]
    fn cannot_remove_root() {
        let mut tree = LayerTree::new(Rect::new(0.0, 0.0, 1920.0, 1080.0));
        let root = tree.root;
        tree.remove_layer(root);
        assert_eq!(tree.len(), 1); // root still exists
    }

    #[test]
    fn reparent_layer() {
        let mut tree = LayerTree::new(Rect::new(0.0, 0.0, 1920.0, 1080.0));
        let a = tree.create_layer(Rect::new(0.0, 0.0, 100.0, 100.0), PromotionReason::Explicit);
        let b = tree.create_layer(Rect::new(0.0, 0.0, 100.0, 100.0), PromotionReason::Explicit);
        let c = tree.create_layer_under(
            a,
            Rect::new(0.0, 0.0, 50.0, 50.0),
            PromotionReason::HasOpacity,
        );
        assert_eq!(tree.parent(c), Some(a));

        tree.reparent(c, b);
        assert_eq!(tree.parent(c), Some(b));
        assert!(tree.children_of(a).is_empty());
        assert_eq!(tree.children_of(b), &[c]);
    }

    #[test]
    fn reparent_prevents_cycle() {
        let mut tree = LayerTree::new(Rect::new(0.0, 0.0, 1920.0, 1080.0));
        let a = tree.create_layer(Rect::new(0.0, 0.0, 100.0, 100.0), PromotionReason::Explicit);
        let b = tree.create_layer_under(
            a,
            Rect::new(0.0, 0.0, 50.0, 50.0),
            PromotionReason::Explicit,
        );

        // Trying to reparent a under its child b should be a no-op.
        tree.reparent(a, b);
        assert_eq!(tree.parent(a), Some(tree.root)); // unchanged
    }

    #[test]
    fn mark_dirty() {
        let mut tree = LayerTree::new(Rect::new(0.0, 0.0, 1920.0, 1080.0));
        let child = tree.create_layer(Rect::new(0.0, 0.0, 100.0, 100.0), PromotionReason::Explicit);

        // Initially all layers are dirty.
        let dirty = tree.dirty_layers();
        assert_eq!(dirty.len(), 2);

        // Clean the child, then mark it dirty again.
        tree.get_mut(child).unwrap().mark_clean();
        assert_eq!(tree.dirty_layers().len(), 1);
        tree.mark_dirty(child);
        assert_eq!(tree.dirty_layers().len(), 2);
    }

    #[test]
    fn set_transform_does_not_dirty() {
        let mut tree = LayerTree::new(Rect::new(0.0, 0.0, 1920.0, 1080.0));
        let child = tree.create_layer(
            Rect::new(0.0, 0.0, 100.0, 100.0),
            PromotionReason::HasTransform,
        );
        tree.get_mut(child).unwrap().mark_clean();
        tree.get_mut(child).unwrap().pixels = Some(vec![0u8; 100 * 100 * 4]);

        tree.set_transform(child, [1.0, 0.0, 0.0, 1.0, 50.0, 50.0]);
        assert!(!tree.get(child).unwrap().is_dirty);
        assert!(tree.get(child).unwrap().has_valid_cache());
    }

    #[test]
    fn set_opacity_does_not_dirty() {
        let mut tree = LayerTree::new(Rect::new(0.0, 0.0, 1920.0, 1080.0));
        let child = tree.create_layer(
            Rect::new(0.0, 0.0, 100.0, 100.0),
            PromotionReason::HasOpacity,
        );
        tree.get_mut(child).unwrap().mark_clean();
        tree.get_mut(child).unwrap().pixels = Some(vec![0u8; 100 * 100 * 4]);

        tree.set_opacity(child, 0.5);
        assert!(!tree.get(child).unwrap().is_dirty);
    }

    #[test]
    fn set_bounds_dirties_on_resize() {
        let mut tree = LayerTree::new(Rect::new(0.0, 0.0, 1920.0, 1080.0));
        let child = tree.create_layer(Rect::new(0.0, 0.0, 100.0, 100.0), PromotionReason::Explicit);
        tree.get_mut(child).unwrap().mark_clean();
        tree.get_mut(child).unwrap().pixels = Some(vec![0u8; 100 * 100 * 4]);

        // Move without resize — should not dirty.
        tree.set_bounds(child, Rect::new(10.0, 10.0, 100.0, 100.0));
        assert!(!tree.get(child).unwrap().is_dirty);

        // Resize — should dirty and invalidate pixels.
        tree.set_bounds(child, Rect::new(10.0, 10.0, 200.0, 200.0));
        assert!(tree.get(child).unwrap().is_dirty);
        assert!(tree.get(child).unwrap().pixels.is_none());
    }

    #[test]
    fn tick_frame_counters() {
        let mut tree = LayerTree::new(Rect::new(0.0, 0.0, 1920.0, 1080.0));
        let child = tree.create_layer(Rect::new(0.0, 0.0, 100.0, 100.0), PromotionReason::Explicit);
        tree.get_mut(child).unwrap().mark_clean();

        for _ in 0..10 {
            tree.tick_frame_counters();
        }

        assert_eq!(tree.get(child).unwrap().frames_since_dirty, 10);
        // Root is still dirty, so its counter should not increment.
        assert_eq!(tree.get(tree.root).unwrap().frames_since_dirty, 0);
    }

    #[test]
    fn total_cached_bytes() {
        let mut tree = LayerTree::new(Rect::new(0.0, 0.0, 1920.0, 1080.0));
        let child = tree.create_layer(Rect::new(0.0, 0.0, 100.0, 100.0), PromotionReason::Explicit);
        tree.get_mut(child).unwrap().pixels = Some(vec![0u8; 100 * 100 * 4]);
        assert_eq!(tree.total_cached_bytes(), 100 * 100 * 4);
    }
}

#[cfg(test)]
mod draw_cmd_tests {
    use crate::draw_cmd::flatten;
    use crate::layer::*;
    use crate::tree::LayerTree;

    #[test]
    fn flatten_empty_tree() {
        let mut tree = LayerTree::new(Rect::new(0.0, 0.0, 1920.0, 1080.0));
        // Mark root as having pixels so it appears in flatten.
        tree.get_mut(tree.root).unwrap().pixels = Some(vec![0u8; 4]);
        let cmds = flatten(&tree, Rect::new(0.0, 0.0, 1920.0, 1080.0));
        assert_eq!(cmds.len(), 1); // just the root
    }

    #[test]
    fn flatten_with_children() {
        let mut tree = LayerTree::new(Rect::new(0.0, 0.0, 1920.0, 1080.0));
        tree.get_mut(tree.root).unwrap().pixels = Some(vec![0u8; 4]);
        let c1 = tree.create_layer(
            Rect::new(100.0, 100.0, 200.0, 200.0),
            PromotionReason::Explicit,
        );
        tree.get_mut(c1).unwrap().pixels = Some(vec![0u8; 200 * 200 * 4]);
        tree.get_mut(c1).unwrap().mark_clean();
        let c2 = tree.create_layer(
            Rect::new(500.0, 500.0, 100.0, 100.0),
            PromotionReason::Explicit,
        );
        tree.get_mut(c2).unwrap().pixels = Some(vec![0u8; 100 * 100 * 4]);

        let cmds = flatten(&tree, Rect::new(0.0, 0.0, 1920.0, 1080.0));
        assert_eq!(cmds.len(), 3);
    }

    #[test]
    fn flatten_skips_fully_transparent() {
        let mut tree = LayerTree::new(Rect::new(0.0, 0.0, 1920.0, 1080.0));
        tree.get_mut(tree.root).unwrap().pixels = Some(vec![0u8; 4]);
        let child = tree.create_layer(
            Rect::new(0.0, 0.0, 100.0, 100.0),
            PromotionReason::HasOpacity,
        );
        tree.set_opacity(child, 0.0);
        tree.get_mut(child).unwrap().pixels = Some(vec![0u8; 100 * 100 * 4]);

        let cmds = flatten(&tree, Rect::new(0.0, 0.0, 1920.0, 1080.0));
        assert_eq!(cmds.len(), 1); // only root, child is fully transparent
    }

    #[test]
    fn flatten_skips_outside_viewport() {
        let mut tree = LayerTree::new(Rect::new(0.0, 0.0, 1920.0, 1080.0));
        tree.get_mut(tree.root).unwrap().pixels = Some(vec![0u8; 4]);
        let child = tree.create_layer(
            Rect::new(2000.0, 2000.0, 100.0, 100.0),
            PromotionReason::Explicit,
        );
        tree.get_mut(child).unwrap().pixels = Some(vec![0u8; 100 * 100 * 4]);

        let cmds = flatten(&tree, Rect::new(0.0, 0.0, 1920.0, 1080.0));
        assert_eq!(cmds.len(), 1); // only root, child is outside viewport
    }

    #[test]
    fn flatten_z_order() {
        let mut tree = LayerTree::new(Rect::new(0.0, 0.0, 1920.0, 1080.0));
        tree.get_mut(tree.root).unwrap().pixels = Some(vec![0u8; 4]);

        let c1 = tree.create_layer(Rect::new(0.0, 0.0, 100.0, 100.0), PromotionReason::Explicit);
        tree.set_z_order(c1, 2);
        tree.get_mut(c1).unwrap().pixels = Some(vec![0u8; 4]);

        let c2 = tree.create_layer(Rect::new(0.0, 0.0, 100.0, 100.0), PromotionReason::Explicit);
        tree.set_z_order(c2, 1);
        tree.get_mut(c2).unwrap().pixels = Some(vec![0u8; 4]);

        let cmds = flatten(&tree, Rect::new(0.0, 0.0, 1920.0, 1080.0));
        assert_eq!(cmds.len(), 3);
        // After root, c2 (z=1) should come before c1 (z=2).
        assert_eq!(cmds[1].layer_id, c2);
        assert_eq!(cmds[2].layer_id, c1);
    }

    #[test]
    fn flatten_accumulates_opacity() {
        let mut tree = LayerTree::new(Rect::new(0.0, 0.0, 1920.0, 1080.0));
        tree.get_mut(tree.root).unwrap().pixels = Some(vec![0u8; 4]);
        tree.set_opacity(tree.root, 0.5);

        let child = tree.create_layer(
            Rect::new(0.0, 0.0, 100.0, 100.0),
            PromotionReason::HasOpacity,
        );
        tree.set_opacity(child, 0.5);
        tree.get_mut(child).unwrap().pixels = Some(vec![0u8; 4]);

        let cmds = flatten(&tree, Rect::new(0.0, 0.0, 1920.0, 1080.0));
        assert_eq!(cmds.len(), 2);
        assert!((cmds[0].opacity - 0.5).abs() < f32::EPSILON); // root
        assert!((cmds[1].opacity - 0.25).abs() < f32::EPSILON); // child: 0.5 * 0.5
    }

    #[test]
    fn flatten_accumulates_transform() {
        let mut tree = LayerTree::new(Rect::new(0.0, 0.0, 1920.0, 1080.0));
        tree.get_mut(tree.root).unwrap().pixels = Some(vec![0u8; 4]);
        tree.set_transform(tree.root, [1.0, 0.0, 0.0, 1.0, 100.0, 0.0]);

        let child = tree.create_layer(
            Rect::new(0.0, 0.0, 50.0, 50.0),
            PromotionReason::HasTransform,
        );
        tree.set_transform(child, [1.0, 0.0, 0.0, 1.0, 50.0, 0.0]);
        tree.get_mut(child).unwrap().pixels = Some(vec![0u8; 4]);

        let cmds = flatten(&tree, Rect::new(0.0, 0.0, 1920.0, 1080.0));
        assert_eq!(cmds.len(), 2);
        // Child's accumulated tx should be 100 + 50 = 150.
        assert!((cmds[1].transform[4] - 150.0).abs() < f32::EPSILON);
    }
}

#[cfg(test)]
mod sync_tests {
    use crate::layer::*;
    use crate::sync::*;
    use crate::tree::LayerTree;

    #[test]
    fn create_initial_pair_produces_matching_trees() {
        let tree = LayerTree::new(Rect::new(0.0, 0.0, 1920.0, 1080.0));
        let (active, pending) = create_initial_pair(tree);
        assert_eq!(active.tree.len(), pending.tree.len());
    }

    #[test]
    fn commit_swaps_trees() {
        let tree = LayerTree::new(Rect::new(0.0, 0.0, 1920.0, 1080.0));
        let (active, mut pending) = create_initial_pair(tree);

        // Modify the pending tree.
        let new_layer = pending
            .tree
            .create_layer(Rect::new(0.0, 0.0, 100.0, 100.0), PromotionReason::Explicit);

        let (new_active, returned, sync) = commit(pending, active);
        assert_eq!(new_active.tree.len(), 2); // root + new layer
        assert!(new_active.tree.get(new_layer).is_some());
        assert_eq!(returned.len(), 1); // old active had only root
        assert!(!sync.is_empty());
        assert_eq!(sync.added.len(), 1);
    }

    #[test]
    fn commit_detects_removed_layers() {
        let mut tree = LayerTree::new(Rect::new(0.0, 0.0, 1920.0, 1080.0));
        let child = tree.create_layer(Rect::new(0.0, 0.0, 100.0, 100.0), PromotionReason::Explicit);
        let (active, mut pending) = create_initial_pair(tree);

        // Remove the child from pending.
        pending.tree.remove_layer(child);

        let (_new_active, _returned, sync) = commit(pending, active);
        assert_eq!(sync.removed.len(), 1);
        assert!(sync.removed.contains(&child));
    }

    #[test]
    fn commit_detects_modified_layers() {
        let mut tree = LayerTree::new(Rect::new(0.0, 0.0, 1920.0, 1080.0));
        let child = tree.create_layer(Rect::new(0.0, 0.0, 100.0, 100.0), PromotionReason::Explicit);
        tree.get_mut(child).unwrap().mark_clean();
        let (active, mut pending) = create_initial_pair(tree);

        // Modify opacity in pending.
        pending.tree.set_opacity(child, 0.5);

        let (_new_active, _returned, sync) = commit(pending, active);
        assert!(sync.modified.contains(&child));
    }

    #[test]
    fn sync_state_total_changes() {
        let state = TreeSyncState {
            added: vec![1, 2],
            removed: vec![3],
            modified: vec![4, 5, 6],
        };
        assert_eq!(state.total_changes(), 6);
    }

    #[test]
    fn sync_state_empty() {
        let state = TreeSyncState::default();
        assert!(state.is_empty());
    }
}

#[cfg(test)]
mod promote_tests {
    use crate::layer::*;
    use crate::promote::*;
    use crate::tree::LayerTree;

    #[test]
    fn promote_will_change() {
        let h = LayerPromotionHeuristics::new();
        let info = ElementInfo {
            has_will_change: true,
            ..Default::default()
        };
        assert_eq!(h.should_promote(&info), Some(PromotionReason::WillChange));
    }

    #[test]
    fn promote_animated_transform() {
        let h = LayerPromotionHeuristics::new();
        let info = ElementInfo {
            has_transform: true,
            animation_active: true,
            ..Default::default()
        };
        assert_eq!(h.should_promote(&info), Some(PromotionReason::HasTransform));
    }

    #[test]
    fn promote_static_transform() {
        let h = LayerPromotionHeuristics::new();
        let info = ElementInfo {
            has_transform: true,
            ..Default::default()
        };
        assert_eq!(h.should_promote(&info), Some(PromotionReason::HasTransform));
    }

    #[test]
    fn promote_opacity() {
        let h = LayerPromotionHeuristics::new();
        let info = ElementInfo {
            has_opacity: true,
            ..Default::default()
        };
        assert_eq!(h.should_promote(&info), Some(PromotionReason::HasOpacity));
    }

    #[test]
    fn promote_filter() {
        let h = LayerPromotionHeuristics::new();
        let info = ElementInfo {
            has_filter: true,
            ..Default::default()
        };
        assert_eq!(h.should_promote(&info), Some(PromotionReason::HasFilter));
    }

    #[test]
    fn promote_fixed_position() {
        let h = LayerPromotionHeuristics::new();
        let info = ElementInfo {
            is_fixed: true,
            ..Default::default()
        };
        assert_eq!(
            h.should_promote(&info),
            Some(PromotionReason::FixedPosition)
        );
    }

    #[test]
    fn promote_scrollable_large_content() {
        let h = LayerPromotionHeuristics::new();
        let info = ElementInfo {
            is_scrollable: true,
            scroll_content_height: 5000.0,
            scroll_viewport_height: 500.0,
            ..Default::default()
        };
        assert_eq!(
            h.should_promote(&info),
            Some(PromotionReason::ScrollingContent)
        );
    }

    #[test]
    fn no_promote_scrollable_small_content() {
        let h = LayerPromotionHeuristics::new();
        let info = ElementInfo {
            is_scrollable: true,
            scroll_content_height: 600.0,
            scroll_viewport_height: 500.0,
            ..Default::default()
        };
        // Ratio 1.2 < threshold 2.0 — should not promote.
        assert!(h.should_promote(&info).is_none());
    }

    #[test]
    fn promote_frequent_repaint() {
        let h = LayerPromotionHeuristics::new();
        let info = ElementInfo {
            paint_count: 5,
            ..Default::default()
        };
        assert_eq!(h.should_promote(&info), Some(PromotionReason::Explicit));
    }

    #[test]
    fn no_promote_plain_element() {
        let h = LayerPromotionHeuristics::new();
        let info = ElementInfo::default();
        assert!(h.should_promote(&info).is_none());
    }

    #[test]
    fn demotion_after_threshold() {
        let h = LayerPromotionHeuristics::new();
        assert!(h.demotion_check(PromotionReason::HasOpacity, 60));
        assert!(!h.demotion_check(PromotionReason::HasOpacity, 59));
    }

    #[test]
    fn no_demotion_for_persistent_reasons() {
        let h = LayerPromotionHeuristics::new();
        assert!(!h.demotion_check(PromotionReason::Root, 1000));
        assert!(!h.demotion_check(PromotionReason::WillChange, 1000));
        assert!(!h.demotion_check(PromotionReason::Video, 1000));
        assert!(!h.demotion_check(PromotionReason::FixedPosition, 1000));
        assert!(!h.demotion_check(PromotionReason::ScrollingContent, 1000));
    }

    #[test]
    fn find_demotable_layers() {
        let h = LayerPromotionHeuristics::new();
        let mut tree = LayerTree::new(Rect::new(0.0, 0.0, 1920.0, 1080.0));
        let c1 = tree.create_layer(
            Rect::new(0.0, 0.0, 100.0, 100.0),
            PromotionReason::HasOpacity,
        );
        let c2 = tree.create_layer(
            Rect::new(0.0, 0.0, 100.0, 100.0),
            PromotionReason::WillChange,
        );

        // Clean both and advance frames past threshold.
        tree.get_mut(c1).unwrap().mark_clean();
        tree.get_mut(c2).unwrap().mark_clean();
        for _ in 0..65 {
            tree.tick_frame_counters();
        }

        let demotable = h.find_demotable_layers(&tree);
        assert!(demotable.contains(&c1));
        assert!(!demotable.contains(&c2)); // WillChange is never demoted
    }
}

#[cfg(test)]
mod pool_tests {
    use crate::pool::*;

    #[test]
    fn allocate_and_release() {
        let mut pool = SurfacePool::new();
        let handle = pool.allocate(100, 100);
        assert_eq!(handle.width, 100);
        assert_eq!(handle.height, 100);
        assert_eq!(handle.stride, 400);
        assert!(handle.data.len() >= 100 * 100 * 4);

        let stats_before = pool.stats();
        assert_eq!(stats_before.allocated, 1);
        assert_eq!(stats_before.fresh, 1);
        assert_eq!(stats_before.reused, 0);

        pool.release(handle);
        let stats_after = pool.stats();
        assert_eq!(stats_after.allocated, 0);
        assert_eq!(stats_after.pooled, 1);
    }

    #[test]
    fn reuse_from_pool() {
        let mut pool = SurfacePool::new();
        let handle1 = pool.allocate(100, 100);
        pool.release(handle1);

        let handle2 = pool.allocate(100, 100);
        let stats = pool.stats();
        assert_eq!(stats.reused, 1);
        assert_eq!(stats.fresh, 1); // only the first allocation was fresh
        pool.release(handle2);
    }

    #[test]
    fn different_sizes_different_buckets() {
        let mut pool = SurfacePool::new();
        let h1 = pool.allocate(100, 100);
        pool.release(h1);

        // A larger allocation should not reuse the smaller bucket.
        let h2 = pool.allocate(300, 300);
        let stats = pool.stats();
        assert_eq!(stats.reused, 0);
        assert_eq!(stats.fresh, 2);
        pool.release(h2);
    }

    #[test]
    fn pool_clear() {
        let mut pool = SurfacePool::new();
        let h = pool.allocate(200, 200);
        pool.release(h);
        assert_eq!(pool.pooled_count(), 1);

        pool.clear();
        assert_eq!(pool.pooled_count(), 0);
    }

    #[test]
    fn max_per_bucket_enforced() {
        let mut pool = SurfacePool::with_max_per_bucket(2);
        let h1 = pool.allocate(100, 100);
        let h2 = pool.allocate(100, 100);
        let h3 = pool.allocate(100, 100);
        pool.release(h1);
        pool.release(h2);
        pool.release(h3); // should be dropped, bucket is full

        assert_eq!(pool.pooled_count(), 2);
    }

    #[test]
    fn allocated_buffer_is_zeroed() {
        let mut pool = SurfacePool::new();
        let h = pool.allocate(64, 64);
        assert!(h.data.iter().all(|&b| b == 0));
        pool.release(h);
    }

    #[test]
    fn reused_buffer_is_zeroed() {
        let mut pool = SurfacePool::new();
        let mut h = pool.allocate(64, 64);
        // Write some data.
        for byte in h.data.iter_mut() {
            *byte = 0xFF;
        }
        pool.release(h);

        // Reuse should be zeroed.
        let h2 = pool.allocate(64, 64);
        assert!(h2.data.iter().all(|&b| b == 0));
        pool.release(h2);
    }

    #[test]
    fn pool_stats_total_bytes() {
        let mut pool = SurfacePool::new();
        let h = pool.allocate(64, 64);
        pool.release(h);
        let stats = pool.stats();
        assert!(stats.pooled_bytes > 0);
        assert_eq!(
            stats.total_bytes(),
            stats.pooled_bytes + stats.allocated_bytes
        );
    }
}

#[cfg(test)]
mod compositor_tests {
    use crate::compositor::*;
    use crate::layer::*;
    use crate::tree::LayerTree;

    #[test]
    fn occlusion_tracker_empty() {
        let tracker = OcclusionTracker::new();
        assert!(!tracker.is_fully_occluded(&Rect::new(0.0, 0.0, 100.0, 100.0)));
    }

    #[test]
    fn occlusion_tracker_full_occlusion() {
        let mut tracker = OcclusionTracker::new();
        tracker.add_opaque_rect(Rect::new(0.0, 0.0, 1920.0, 1080.0));
        assert!(tracker.is_fully_occluded(&Rect::new(100.0, 100.0, 200.0, 200.0)));
    }

    #[test]
    fn occlusion_tracker_partial_not_occluded() {
        let mut tracker = OcclusionTracker::new();
        tracker.add_opaque_rect(Rect::new(0.0, 0.0, 500.0, 500.0));
        // Partially overlapping — not fully occluded.
        assert!(!tracker.is_fully_occluded(&Rect::new(400.0, 400.0, 200.0, 200.0)));
    }

    #[test]
    fn occlusion_tracker_reset() {
        let mut tracker = OcclusionTracker::new();
        tracker.add_opaque_rect(Rect::new(0.0, 0.0, 1920.0, 1080.0));
        tracker.reset();
        assert!(!tracker.is_fully_occluded(&Rect::new(0.0, 0.0, 100.0, 100.0)));
    }

    #[test]
    fn clear_output_fills_opaque_black() {
        let mut buf = vec![0u8; 4 * 4 * 4]; // 4x4 RGBA
        clear_output(&mut buf);
        for pixel in buf.chunks(4) {
            assert_eq!(pixel, &[0, 0, 0, 255]);
        }
    }

    #[test]
    fn clear_output_color_fills_correctly() {
        let mut buf = vec![0u8; 2 * 2 * 4];
        clear_output_color(&mut buf, 128, 64, 32, 200);
        for pixel in buf.chunks(4) {
            assert_eq!(pixel, &[128, 64, 32, 200]);
        }
    }

    #[test]
    fn composite_empty_tree() {
        let mut compositor = LayerCompositor::new();
        let tree = LayerTree::new(Rect::new(0.0, 0.0, 4.0, 4.0));
        let mut output = vec![0u8; 4 * 4 * 4];
        let stats = compositor.composite(&tree, &mut output, 4, 4);
        assert_eq!(stats.total_commands, 1); // root (but no pixels)
        assert_eq!(stats.skipped_no_pixels, 1);
    }

    #[test]
    fn composite_opaque_layer() {
        let mut compositor = LayerCompositor::new();
        let mut tree = LayerTree::new(Rect::new(0.0, 0.0, 4.0, 4.0));

        // Fill root with red pixels.
        let root = tree.root;
        let mut root_pixels = vec![0u8; 4 * 4 * 4];
        for pixel in root_pixels.chunks_mut(4) {
            pixel.copy_from_slice(&[255, 0, 0, 255]); // RGBA red
        }
        tree.get_mut(root).unwrap().pixels = Some(root_pixels);
        tree.get_mut(root).unwrap().mark_clean();

        let mut output = vec![0u8; 4 * 4 * 4];
        let stats = compositor.composite(&tree, &mut output, 4, 4);
        assert_eq!(stats.drawn, 1);

        // Check that output has red pixels.
        for pixel in output.chunks(4) {
            assert_eq!(pixel[0], 255); // R
            assert_eq!(pixel[1], 0); // G
            assert_eq!(pixel[2], 0); // B
            assert_eq!(pixel[3], 255); // A
        }
    }

    #[test]
    fn composite_with_opacity() {
        let mut compositor = LayerCompositor::new();
        let mut tree = LayerTree::new(Rect::new(0.0, 0.0, 2.0, 2.0));

        // Root: solid white.
        let root = tree.root;
        let mut root_pixels = vec![0u8; 2 * 2 * 4];
        for pixel in root_pixels.chunks_mut(4) {
            pixel.copy_from_slice(&[255, 255, 255, 255]);
        }
        tree.get_mut(root).unwrap().pixels = Some(root_pixels);
        tree.get_mut(root).unwrap().mark_clean();

        // Child: solid red at 50% opacity covering 1x1 at (0,0).
        let child = tree.create_layer(Rect::new(0.0, 0.0, 1.0, 1.0), PromotionReason::HasOpacity);
        tree.set_opacity(child, 0.5);
        let child_pixels = vec![255, 0, 0, 255]; // one pixel, RGBA red
        tree.get_mut(child).unwrap().pixels = Some(child_pixels);
        tree.get_mut(child).unwrap().mark_clean();

        let mut output = vec![0u8; 2 * 2 * 4];
        let _stats = compositor.composite(&tree, &mut output, 2, 2);

        // Pixel (0,0) should be a blend of red over white at 50%.
        // SrcOver: out = src*0.5 + dst*(1-0.5)
        // R: 255*0.5 + 255*0.5 = 255 (both are 255 so result is 255)
        // G: 0*0.5 + 255*0.5 = ~128
        // B: 0*0.5 + 255*0.5 = ~128
        let r = output[0];
        let g = output[1];
        let b = output[2];
        assert_eq!(r, 255);
        assert!(g > 100 && g < 160, "green={g} expected ~128");
        assert!(b > 100 && b < 160, "blue={b} expected ~128");
    }

    #[test]
    fn composite_stats_counts() {
        let mut compositor = LayerCompositor::new();
        let mut tree = LayerTree::new(Rect::new(0.0, 0.0, 100.0, 100.0));
        let root = tree.root;
        tree.get_mut(root).unwrap().pixels = Some(vec![0u8; 100 * 100 * 4]);

        let c1 = tree.create_layer(Rect::new(0.0, 0.0, 50.0, 50.0), PromotionReason::Explicit);
        tree.get_mut(c1).unwrap().pixels = Some(vec![0u8; 50 * 50 * 4]);

        // Child with no pixels.
        let _c2 = tree.create_layer(Rect::new(0.0, 0.0, 50.0, 50.0), PromotionReason::Explicit);

        let mut output = vec![0u8; 100 * 100 * 4];
        let stats = compositor.composite(&tree, &mut output, 100, 100);
        assert_eq!(stats.total_commands, 3);
        assert_eq!(stats.drawn, 2);
        assert_eq!(stats.skipped_no_pixels, 1);
    }
}
