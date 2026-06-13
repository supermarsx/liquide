//! Tests for the layout cache crate.

#[cfg(test)]
mod constraint_tests {
    use crate::constraints::*;
    use std::collections::HashSet;

    #[test]
    fn dimension_auto_eq() {
        assert_eq!(Dimension::Auto, Dimension::Auto);
    }

    #[test]
    fn dimension_fixed_eq() {
        assert_eq!(Dimension::Fixed(100.0), Dimension::Fixed(100.0));
        assert_ne!(Dimension::Fixed(100.0), Dimension::Fixed(101.0));
    }

    #[test]
    fn dimension_minmax_eq() {
        assert_eq!(
            Dimension::MinMax(10.0, 200.0),
            Dimension::MinMax(10.0, 200.0)
        );
        assert_ne!(
            Dimension::MinMax(10.0, 200.0),
            Dimension::MinMax(10.0, 201.0)
        );
    }

    #[test]
    fn dimension_different_variants_ne() {
        assert_ne!(Dimension::Auto, Dimension::Fixed(0.0));
        assert_ne!(Dimension::Fixed(100.0), Dimension::MinMax(100.0, 100.0));
    }

    #[test]
    fn dimension_nan_eq_for_cache() {
        // NaN == NaN in our cache-key semantics
        assert_eq!(Dimension::Fixed(f32::NAN), Dimension::Fixed(f32::NAN));
    }

    #[test]
    fn dimension_negative_zero_eq_positive_zero() {
        assert_eq!(Dimension::Fixed(-0.0), Dimension::Fixed(0.0));
    }

    #[test]
    fn dimension_hash_consistency() {
        let mut set = HashSet::new();
        set.insert(Dimension::Fixed(42.0));
        assert!(set.contains(&Dimension::Fixed(42.0)));
        assert!(!set.contains(&Dimension::Fixed(43.0)));
    }

    #[test]
    fn dimension_approx_eq_within_tolerance() {
        let a = Dimension::Fixed(100.0);
        let b = Dimension::Fixed(100.3);
        assert!(a.approx_eq(&b, 0.5));
        assert!(!a.approx_eq(&b, 0.1));
    }

    #[test]
    fn dimension_approx_eq_auto() {
        assert!(Dimension::Auto.approx_eq(&Dimension::Auto, 1.0));
        assert!(!Dimension::Auto.approx_eq(&Dimension::Fixed(0.0), 1.0));
    }

    #[test]
    fn dimension_is_auto() {
        assert!(Dimension::Auto.is_auto());
        assert!(!Dimension::Fixed(0.0).is_auto());
    }

    #[test]
    fn dimension_is_fixed() {
        assert!(Dimension::Fixed(10.0).is_fixed());
        assert!(!Dimension::Auto.is_fixed());
    }

    #[test]
    fn dimension_fixed_value() {
        assert_eq!(Dimension::Fixed(42.0).fixed_value(), Some(42.0));
        assert_eq!(Dimension::Auto.fixed_value(), None);
        assert_eq!(Dimension::MinMax(1.0, 2.0).fixed_value(), None);
    }

    #[test]
    fn constraints_fixed_constructor() {
        let c = LayoutConstraints::fixed(800.0, 600.0);
        assert_eq!(c.available_width, Dimension::Fixed(800.0));
        assert_eq!(c.available_height, Dimension::Fixed(600.0));
        assert!(c.is_fully_fixed());
    }

    #[test]
    fn constraints_width_only() {
        let c = LayoutConstraints::width_only(400.0);
        assert_eq!(c.available_width, Dimension::Fixed(400.0));
        assert_eq!(c.available_height, Dimension::Auto);
        assert!(!c.is_fully_fixed());
    }

    #[test]
    fn constraints_auto() {
        let c = LayoutConstraints::auto();
        assert!(c.available_width.is_auto());
        assert!(c.available_height.is_auto());
    }

    #[test]
    fn constraints_eq() {
        let a = LayoutConstraints::fixed(100.0, 200.0);
        let b = LayoutConstraints::fixed(100.0, 200.0);
        assert_eq!(a, b);
    }

    #[test]
    fn constraints_ne_width() {
        let a = LayoutConstraints::fixed(100.0, 200.0);
        let b = LayoutConstraints::fixed(101.0, 200.0);
        assert_ne!(a, b);
    }

    #[test]
    fn constraints_hash_consistency() {
        let mut set = HashSet::new();
        set.insert(LayoutConstraints::fixed(100.0, 200.0));
        assert!(set.contains(&LayoutConstraints::fixed(100.0, 200.0)));
        assert!(!set.contains(&LayoutConstraints::fixed(100.0, 201.0)));
    }

    #[test]
    fn constraints_approx_eq() {
        let a = LayoutConstraints::fixed(100.0, 200.0);
        let b = LayoutConstraints::fixed(100.4, 200.3);
        assert!(a.approx_eq(&b, 0.5));
        assert!(!a.approx_eq(&b, 0.1));
    }

    #[test]
    fn constraints_differs_only_in_width() {
        let a = LayoutConstraints::fixed(100.0, 200.0);
        let b = LayoutConstraints::fixed(150.0, 200.0);
        assert!(a.differs_only_in_width(&b));
        assert!(!a.differs_only_in_height(&b));
    }

    #[test]
    fn constraints_differs_only_in_height() {
        let a = LayoutConstraints::fixed(100.0, 200.0);
        let b = LayoutConstraints::fixed(100.0, 300.0);
        assert!(a.differs_only_in_height(&b));
        assert!(!a.differs_only_in_width(&b));
    }

    #[test]
    fn constraints_writing_mode_affects_equality() {
        let mut a = LayoutConstraints::fixed(100.0, 200.0);
        let mut b = LayoutConstraints::fixed(100.0, 200.0);
        a.writing_mode = WritingMode::HorizontalTb;
        b.writing_mode = WritingMode::VerticalLr;
        assert_ne!(a, b);
    }

    #[test]
    fn constraints_direction_affects_equality() {
        let mut a = LayoutConstraints::fixed(100.0, 200.0);
        let mut b = LayoutConstraints::fixed(100.0, 200.0);
        a.direction = Direction::LTR;
        b.direction = Direction::RTL;
        assert_ne!(a, b);
    }
}

#[cfg(test)]
mod result_tests {
    use crate::result::*;

    #[test]
    fn layout_result_default() {
        let r = LayoutResult::default();
        assert_eq!(r.size, (0.0, 0.0));
        assert_eq!(r.baseline, None);
        assert!(r.child_offsets.is_empty());
    }

    #[test]
    fn layout_result_with_size() {
        let r = LayoutResult::with_size(100.0, 50.0);
        assert_eq!(r.width(), 100.0);
        assert_eq!(r.height(), 50.0);
        assert_eq!(r.overflow, (100.0, 50.0));
    }

    #[test]
    fn margin_box_dimensions() {
        let r = LayoutResult {
            size: (100.0, 50.0),
            margins: (10.0, 20.0, 10.0, 20.0),
            ..Default::default()
        };
        assert_eq!(r.margin_box_width(), 140.0);
        assert_eq!(r.margin_box_height(), 70.0);
    }

    #[test]
    fn intrinsic_sizes_new() {
        let s = IntrinsicSizes::new(50.0, 200.0);
        assert_eq!(s.min_content_width, 50.0);
        assert_eq!(s.max_content_width, 200.0);
        assert_eq!(s.min_content_height, None);
    }

    #[test]
    fn intrinsic_sizes_with_height() {
        let s = IntrinsicSizes::new(50.0, 200.0).with_height(30.0, 100.0);
        assert_eq!(s.min_content_height, Some(30.0));
        assert_eq!(s.max_content_height, Some(100.0));
    }
}

#[cfg(test)]
mod cache_tests {
    use crate::cache::*;
    use crate::constraints::*;
    use crate::result::*;

    #[test]
    fn empty_cache() {
        let mut cache = LayoutCache::new();
        assert_eq!(cache.node_count(), 0);
        assert_eq!(cache.entry_count(), 0);
        assert_eq!(cache.hit_rate(), 0.0);
        assert!(cache.lookup(1, &LayoutConstraints::auto()).is_none());
    }

    #[test]
    fn store_and_lookup() {
        let mut cache = LayoutCache::new();
        let c = LayoutConstraints::fixed(100.0, 200.0);
        let r = LayoutResult::with_size(100.0, 200.0);
        cache.store(1, c.clone(), r.clone());

        let result = cache.lookup(1, &c);
        assert!(result.is_some());
        assert_eq!(result.unwrap().size, (100.0, 200.0));
    }

    #[test]
    fn miss_on_different_constraints() {
        let mut cache = LayoutCache::new();
        let c1 = LayoutConstraints::fixed(100.0, 200.0);
        cache.store(1, c1, LayoutResult::with_size(100.0, 200.0));

        let c2 = LayoutConstraints::fixed(200.0, 200.0);
        assert!(cache.lookup(1, &c2).is_none());
    }

    #[test]
    fn miss_on_different_node() {
        let mut cache = LayoutCache::new();
        let c = LayoutConstraints::fixed(100.0, 200.0);
        cache.store(1, c.clone(), LayoutResult::with_size(100.0, 200.0));

        assert!(cache.lookup(2, &c).is_none());
    }

    #[test]
    fn fuzzy_lookup_within_tolerance() {
        let mut cache = LayoutCache::new();
        let c1 = LayoutConstraints::fixed(100.0, 200.0);
        cache.store(1, c1, LayoutResult::with_size(100.0, 200.0));

        let c2 = LayoutConstraints::fixed(100.3, 200.2);
        let result = cache.lookup_fuzzy(1, &c2, 0.5);
        assert!(result.is_some());
    }

    #[test]
    fn fuzzy_lookup_beyond_tolerance() {
        let mut cache = LayoutCache::new();
        let c1 = LayoutConstraints::fixed(100.0, 200.0);
        cache.store(1, c1, LayoutResult::with_size(100.0, 200.0));

        let c2 = LayoutConstraints::fixed(102.0, 200.0);
        let result = cache.lookup_fuzzy(1, &c2, 0.5);
        assert!(result.is_none());
    }

    #[test]
    fn replace_existing_entry() {
        let mut cache = LayoutCache::new();
        let c = LayoutConstraints::fixed(100.0, 200.0);
        cache.store(1, c.clone(), LayoutResult::with_size(100.0, 200.0));
        cache.store(1, c.clone(), LayoutResult::with_size(100.0, 250.0));

        assert_eq!(cache.entry_count(), 1);
        let result = cache.lookup(1, &c).unwrap();
        assert_eq!(result.height(), 250.0);
    }

    #[test]
    fn multiple_entries_per_node() {
        let mut cache = LayoutCache::new();
        let c1 = LayoutConstraints::fixed(100.0, 200.0);
        let c2 = LayoutConstraints::fixed(200.0, 300.0);
        cache.store(1, c1.clone(), LayoutResult::with_size(100.0, 200.0));
        cache.store(1, c2.clone(), LayoutResult::with_size(200.0, 300.0));

        assert_eq!(cache.entry_count(), 2);
        assert!(cache.lookup(1, &c1).is_some());
        assert!(cache.lookup(1, &c2).is_some());
    }

    #[test]
    fn evict_oldest_when_at_max_entries() {
        let mut cache = LayoutCache::with_max_entries(2);

        let c1 = LayoutConstraints::fixed(100.0, 100.0);
        let c2 = LayoutConstraints::fixed(200.0, 200.0);
        let c3 = LayoutConstraints::fixed(300.0, 300.0);

        cache.store(1, c1.clone(), LayoutResult::with_size(100.0, 100.0));
        cache.advance_generation(10);
        cache.store(1, c2.clone(), LayoutResult::with_size(200.0, 200.0));
        cache.advance_generation(10);
        cache.store(1, c3.clone(), LayoutResult::with_size(300.0, 300.0));

        // Should have at most 2 entries
        assert_eq!(cache.entries_for(1).len(), 2);
        // The oldest (c1) should have been evicted
        assert!(cache.lookup(1, &c1).is_none());
    }

    #[test]
    fn invalidate_single_node() {
        let mut cache = LayoutCache::new();
        cache.store(1, LayoutConstraints::auto(), LayoutResult::default());
        cache.store(2, LayoutConstraints::auto(), LayoutResult::default());
        cache.invalidate(1);

        assert!(!cache.has_entries(1));
        assert!(cache.has_entries(2));
    }

    #[test]
    fn invalidate_subtree() {
        let mut cache = LayoutCache::new();
        // Tree: 1 -> [2, 3], 2 -> [4]
        cache.store(1, LayoutConstraints::auto(), LayoutResult::default());
        cache.store(2, LayoutConstraints::auto(), LayoutResult::default());
        cache.store(3, LayoutConstraints::auto(), LayoutResult::default());
        cache.store(4, LayoutConstraints::auto(), LayoutResult::default());
        cache.store(5, LayoutConstraints::auto(), LayoutResult::default());

        cache.invalidate_subtree(1, |id| match id {
            1 => vec![2, 3],
            2 => vec![4],
            _ => vec![],
        });

        assert!(!cache.has_entries(1));
        assert!(!cache.has_entries(2));
        assert!(!cache.has_entries(3));
        assert!(!cache.has_entries(4));
        // Node 5 is not in the subtree, should remain.
        assert!(cache.has_entries(5));
    }

    #[test]
    fn invalidate_all() {
        let mut cache = LayoutCache::new();
        cache.store(1, LayoutConstraints::auto(), LayoutResult::default());
        cache.store(2, LayoutConstraints::auto(), LayoutResult::default());
        cache.invalidate_all();
        assert_eq!(cache.node_count(), 0);
    }

    #[test]
    fn generation_eviction() {
        let mut cache = LayoutCache::new();
        cache.store(1, LayoutConstraints::auto(), LayoutResult::default());

        // Advance 5 generations, keeping only 2
        for _ in 0..5 {
            cache.advance_generation(2);
        }

        // Entry from generation 0 should be evicted (current gen is 5, cutoff is 3)
        assert!(!cache.has_entries(1));
    }

    #[test]
    fn generation_keeps_recent() {
        let mut cache = LayoutCache::new();
        cache.store(1, LayoutConstraints::auto(), LayoutResult::default());
        cache.advance_generation(5);
        // gen is now 1, cutoff = max(1-5, 0) = 0. Entry at gen 0 >= cutoff 0 → kept
        assert!(cache.has_entries(1));
    }

    #[test]
    fn hit_miss_counters() {
        let mut cache = LayoutCache::new();
        let c = LayoutConstraints::fixed(100.0, 200.0);
        cache.store(1, c.clone(), LayoutResult::with_size(100.0, 200.0));

        cache.lookup(1, &c); // hit
        cache.lookup(1, &c); // hit
        cache.lookup(1, &LayoutConstraints::auto()); // miss

        assert_eq!(cache.hit_count(), 2);
        assert_eq!(cache.miss_count(), 1);
        assert!((cache.hit_rate() - 2.0 / 3.0).abs() < 0.01);
    }

    #[test]
    fn reset_stats() {
        let mut cache = LayoutCache::new();
        let c = LayoutConstraints::auto();
        cache.store(1, c.clone(), LayoutResult::default());
        cache.lookup(1, &c);
        cache.reset_stats();
        assert_eq!(cache.hit_count(), 0);
        assert_eq!(cache.miss_count(), 0);
    }
}

#[cfg(test)]
mod measure_tests {
    use crate::measure::*;
    use crate::result::*;

    #[test]
    fn empty_measure_cache() {
        let cache = MeasureCache::new();
        assert!(cache.is_empty());
        assert!(cache.measure(1).is_none());
    }

    #[test]
    fn store_and_lookup() {
        let mut cache = MeasureCache::new();
        let sizes = IntrinsicSizes::new(50.0, 200.0);
        cache.store_measure(1, sizes);

        let result = cache.measure(1);
        assert!(result.is_some());
        assert_eq!(result.unwrap().min_content_width, 50.0);
    }

    #[test]
    fn invalidate_single() {
        let mut cache = MeasureCache::new();
        cache.store_measure(1, IntrinsicSizes::new(50.0, 200.0));
        cache.store_measure(2, IntrinsicSizes::new(60.0, 300.0));
        cache.invalidate_measure(1);

        assert!(cache.measure(1).is_none());
        assert!(cache.measure(2).is_some());
    }

    #[test]
    fn invalidate_subtree() {
        let mut cache = MeasureCache::new();
        cache.store_measure(1, IntrinsicSizes::default());
        cache.store_measure(2, IntrinsicSizes::default());
        cache.store_measure(3, IntrinsicSizes::default());

        cache.invalidate_subtree(1, |id| if id == 1 { vec![2] } else { vec![] });

        assert!(cache.measure(1).is_none());
        assert!(cache.measure(2).is_none());
        assert!(cache.measure(3).is_some());
    }

    #[test]
    fn invalidate_all() {
        let mut cache = MeasureCache::new();
        cache.store_measure(1, IntrinsicSizes::default());
        cache.store_measure(2, IntrinsicSizes::default());
        cache.invalidate_all();
        assert!(cache.is_empty());
    }

    #[test]
    fn len() {
        let mut cache = MeasureCache::new();
        assert_eq!(cache.len(), 0);
        cache.store_measure(1, IntrinsicSizes::default());
        assert_eq!(cache.len(), 1);
        cache.store_measure(2, IntrinsicSizes::default());
        assert_eq!(cache.len(), 2);
    }
}

#[cfg(test)]
mod dirty_tests {
    use crate::dirty::*;

    #[test]
    fn initially_clean() {
        let dirty = DirtyPropagation::new();
        assert!(!dirty.needs_layout(1));
        assert!(!dirty.needs_measure(1));
        assert!(!dirty.has_dirty_flags(1));
    }

    #[test]
    fn mark_needs_layout() {
        let mut dirty = DirtyPropagation::new();
        dirty.mark_dirty(1, LayoutDirtyFlags::NEEDS_LAYOUT);
        assert!(dirty.needs_layout(1));
        assert!(!dirty.needs_measure(1));
    }

    #[test]
    fn mark_needs_measure() {
        let mut dirty = DirtyPropagation::new();
        dirty.mark_dirty(1, LayoutDirtyFlags::NEEDS_MEASURE);
        assert!(dirty.needs_measure(1));
        assert!(!dirty.needs_layout(1));
    }

    #[test]
    fn mark_content_changed_implies_both() {
        let mut dirty = DirtyPropagation::new();
        dirty.mark_dirty(1, LayoutDirtyFlags::CONTENT_CHANGED);
        assert!(dirty.needs_layout(1));
        assert!(dirty.needs_measure(1));
    }

    #[test]
    fn mark_style_changed() {
        let mut dirty = DirtyPropagation::new();
        dirty.mark_dirty(1, LayoutDirtyFlags::STYLE_CHANGED);
        assert!(dirty.needs_layout(1));
    }

    #[test]
    fn propagate_up() {
        let mut dirty = DirtyPropagation::new();
        dirty.mark_dirty(4, LayoutDirtyFlags::NEEDS_LAYOUT);
        // Tree: 1 -> 2 -> 3 -> 4
        dirty.propagate_up(4, |id| match id {
            4 => Some(3),
            3 => Some(2),
            2 => Some(1),
            _ => None,
        });

        assert!(dirty.has_dirty_flags(3));
        assert!(dirty.has_dirty_flags(2));
        assert!(dirty.has_dirty_flags(1));
        // Ancestors should have CHILD_NEEDS_LAYOUT, not NEEDS_LAYOUT
        let flags_3 = dirty.get_flags(3);
        assert!(flags_3.contains(LayoutDirtyFlags::CHILD_NEEDS_LAYOUT));
        assert!(!flags_3.contains(LayoutDirtyFlags::NEEDS_LAYOUT));
    }

    #[test]
    fn propagate_up_stops_at_existing_flag() {
        let mut dirty = DirtyPropagation::new();
        // Pre-mark ancestor 2
        dirty.mark_dirty(2, LayoutDirtyFlags::CHILD_NEEDS_LAYOUT);

        // Mark node 4 dirty, propagate up: 4->3->2->1
        dirty.mark_dirty(4, LayoutDirtyFlags::NEEDS_LAYOUT);
        dirty.propagate_up(4, |id| match id {
            4 => Some(3),
            3 => Some(2),
            2 => Some(1),
            _ => None,
        });

        // Node 3 gets marked, but propagation stops at 2 (already had flag)
        assert!(dirty.has_dirty_flags(3));
        assert!(dirty.has_dirty_flags(2));
        // Node 1 was NOT reached because 2 already had the flag
        assert!(!dirty.has_dirty_flags(1));
    }

    #[test]
    fn clear_single() {
        let mut dirty = DirtyPropagation::new();
        dirty.mark_dirty(1, LayoutDirtyFlags::NEEDS_LAYOUT);
        dirty.clear(1);
        assert!(!dirty.has_dirty_flags(1));
    }

    #[test]
    fn clear_all() {
        let mut dirty = DirtyPropagation::new();
        dirty.mark_dirty(1, LayoutDirtyFlags::NEEDS_LAYOUT);
        dirty.mark_dirty(2, LayoutDirtyFlags::STYLE_CHANGED);
        dirty.clear_all();
        assert_eq!(dirty.dirty_count(), 0);
    }

    #[test]
    fn mark_all_dirty() {
        let mut dirty = DirtyPropagation::new();
        dirty.mark_all_dirty(vec![1, 2, 3]);
        assert!(dirty.needs_layout(1));
        assert!(dirty.needs_layout(2));
        assert!(dirty.needs_layout(3));
        assert_eq!(dirty.dirty_count(), 3);
    }

    #[test]
    fn mark_dirty_and_propagate() {
        let mut dirty = DirtyPropagation::new();
        dirty.mark_dirty_and_propagate(3, LayoutDirtyFlags::NEEDS_LAYOUT, |id| match id {
            3 => Some(2),
            2 => Some(1),
            _ => None,
        });

        assert!(dirty.needs_layout(3));
        assert!(
            dirty
                .get_flags(2)
                .contains(LayoutDirtyFlags::CHILD_NEEDS_LAYOUT)
        );
        assert!(
            dirty
                .get_flags(1)
                .contains(LayoutDirtyFlags::CHILD_NEEDS_LAYOUT)
        );
    }

    #[test]
    fn flags_accumulate() {
        let mut dirty = DirtyPropagation::new();
        dirty.mark_dirty(1, LayoutDirtyFlags::NEEDS_LAYOUT);
        dirty.mark_dirty(1, LayoutDirtyFlags::STYLE_CHANGED);
        let flags = dirty.get_flags(1);
        assert!(flags.contains(LayoutDirtyFlags::NEEDS_LAYOUT));
        assert!(flags.contains(LayoutDirtyFlags::STYLE_CHANGED));
    }

    #[test]
    fn needs_any_work() {
        assert!(!LayoutDirtyFlags::empty().needs_any_work());
        assert!(LayoutDirtyFlags::NEEDS_LAYOUT.needs_any_work());
        assert!(LayoutDirtyFlags::CHILD_NEEDS_LAYOUT.needs_any_work());
        assert!(LayoutDirtyFlags::CONTENT_CHANGED.needs_any_work());
    }
}

#[cfg(test)]
mod policy_tests {
    use crate::policy::*;

    fn leaf_hints() -> SizingHints {
        SizingHints {
            is_leaf: true,
            ..Default::default()
        }
    }

    fn fixed_hints() -> SizingHints {
        SizingHints {
            has_fixed_width: true,
            has_fixed_height: true,
            ..Default::default()
        }
    }

    #[test]
    fn never_cache_display_none() {
        let p = CachePolicy::new();
        assert!(!p.should_cache(
            DisplayType::None,
            PositionType::Static,
            &Default::default(),
            0
        ));
    }

    #[test]
    fn never_cache_display_contents() {
        let p = CachePolicy::new();
        assert!(!p.should_cache(
            DisplayType::Contents,
            PositionType::Static,
            &Default::default(),
            0
        ));
    }

    #[test]
    fn always_cache_text() {
        let p = CachePolicy::new();
        assert!(p.should_cache(DisplayType::Text, PositionType::Static, &leaf_hints(), 0));
    }

    #[test]
    fn always_cache_replaced() {
        let p = CachePolicy::new();
        assert!(p.should_cache(
            DisplayType::Replaced,
            PositionType::Static,
            &leaf_hints(),
            0
        ));
    }

    #[test]
    fn always_cache_fixed_size() {
        let p = CachePolicy::new();
        assert!(p.should_cache(DisplayType::Block, PositionType::Static, &fixed_hints(), 5));
    }

    #[test]
    fn skip_absolute_by_default() {
        let p = CachePolicy::new();
        assert!(!p.should_cache(
            DisplayType::Block,
            PositionType::Absolute,
            &Default::default(),
            5
        ));
    }

    #[test]
    fn cache_absolute_when_policy_allows() {
        let p = CachePolicy::cache_all();
        assert!(p.should_cache(
            DisplayType::Block,
            PositionType::Absolute,
            &Default::default(),
            5
        ));
    }

    #[test]
    fn skip_percentage_sizing_by_default() {
        let p = CachePolicy::new();
        let hints = SizingHints {
            has_percentage_sizing: true,
            ..Default::default()
        };
        assert!(!p.should_cache(DisplayType::Block, PositionType::Static, &hints, 5));
    }

    #[test]
    fn skip_flex_auto_margins() {
        let p = CachePolicy::new();
        let hints = SizingHints {
            has_flex_auto_margins: true,
            ..Default::default()
        };
        assert!(!p.should_cache(DisplayType::Flex, PositionType::Static, &hints, 5));
    }

    #[test]
    fn default_convenience() {
        assert!(should_cache_default(
            DisplayType::Text,
            PositionType::Static,
            &leaf_hints(),
            0,
        ));
    }

    #[test]
    fn min_children_threshold() {
        let p = CachePolicy {
            min_children_to_cache: 3,
            ..CachePolicy::new()
        };
        // Container with 2 children: below threshold
        assert!(!p.should_cache(
            DisplayType::Block,
            PositionType::Static,
            &Default::default(),
            2
        ));
        // Container with 5 children: above threshold
        assert!(p.should_cache(
            DisplayType::Block,
            PositionType::Static,
            &Default::default(),
            5
        ));
        // Leaf node: exempt from threshold
        assert!(p.should_cache(DisplayType::Block, PositionType::Static, &leaf_hints(), 0));
    }
}

#[cfg(test)]
mod stats_tests {
    use crate::stats::*;

    #[test]
    fn default_zeroed() {
        let s = FrameStatistics::new();
        assert_eq!(s.total_nodes(), 0);
        assert_eq!(s.cache_hit_rate(), 0.0);
        assert_eq!(s.layout_time_us, 0);
    }

    #[test]
    fn record_counters() {
        let mut s = FrameStatistics::new();
        s.record_layout();
        s.record_layout();
        s.record_cache_hit();
        s.record_skipped();
        assert_eq!(s.nodes_laid_out, 2);
        assert_eq!(s.nodes_cache_hit, 1);
        assert_eq!(s.nodes_skipped, 1);
        assert_eq!(s.total_nodes(), 4);
    }

    #[test]
    fn cache_hit_rate() {
        let mut s = FrameStatistics::new();
        s.nodes_laid_out = 3;
        s.nodes_cache_hit = 7;
        // rate = 7 / (3 + 7) = 0.7
        assert!((s.cache_hit_rate() - 0.7).abs() < 0.001);
    }

    #[test]
    fn cache_hit_rate_excludes_skipped() {
        let mut s = FrameStatistics::new();
        s.nodes_laid_out = 2;
        s.nodes_cache_hit = 8;
        s.nodes_skipped = 100;
        // rate = 8 / (2 + 8) = 0.8  (skipped not in denominator)
        assert!((s.cache_hit_rate() - 0.8).abs() < 0.001);
    }

    #[test]
    fn merge() {
        let mut a = FrameStatistics {
            nodes_laid_out: 10,
            nodes_cache_hit: 5,
            nodes_skipped: 3,
            layout_time_us: 1000,
        };
        let b = FrameStatistics {
            nodes_laid_out: 20,
            nodes_cache_hit: 15,
            nodes_skipped: 7,
            layout_time_us: 2000,
        };
        a.merge(&b);
        assert_eq!(a.nodes_laid_out, 30);
        assert_eq!(a.nodes_cache_hit, 20);
        assert_eq!(a.nodes_skipped, 10);
        assert_eq!(a.layout_time_us, 3000);
    }

    #[test]
    fn avg_layout_time() {
        let s = FrameStatistics {
            nodes_laid_out: 4,
            layout_time_us: 1000,
            ..Default::default()
        };
        assert!((s.avg_layout_time_per_node_us() - 250.0).abs() < 0.001);
    }

    #[test]
    fn avg_layout_time_zero_nodes() {
        let s = FrameStatistics::new();
        assert_eq!(s.avg_layout_time_per_node_us(), 0.0);
    }
}

#[cfg(test)]
mod text_measure_tests {
    use std::mem;

    use crate::constraints::{Direction, WritingMode};
    use crate::text_measure::*;

    fn key(text: &str) -> TextMeasureKey {
        TextMeasureKey::from_text(text, vec!["Manrope".to_string()], 16.0)
            .with_width_constraint(240.0)
            .with_wrap_mode(TextWrapMode::Normal)
            .with_language("en")
    }

    fn value(width: f32) -> TextMeasureValue {
        TextMeasureValue::new(width, 19.2, 12.8, 1)
            .with_vertical_metrics(12.8, 6.4)
            .with_intrinsic_widths(width, width)
    }

    #[test]
    fn text_measure_cache_records_hit_and_miss_stats() {
        let mut cache = TextMeasureCache::new();
        let cache_key = key("Hello cache");

        assert!(cache.lookup(&cache_key).is_none());
        assert!(cache.insert(cache_key.clone(), value(94.0)));

        let cached = cache.lookup(&cache_key).unwrap();
        assert_eq!(cached.width, 94.0);

        let stats = cache.stats();
        assert_eq!(stats.requests, 2);
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.inserts, 1);
        assert_eq!(stats.entries, 1);
        assert!(stats.approximate_bytes > 0);
        assert!((stats.hit_rate() - 0.5).abs() < 0.001);
    }

    #[test]
    fn text_measure_key_distinguishes_layout_and_font_dimensions() {
        let mut cache = TextMeasureCache::new();
        let base_key = key("Hello cache");
        assert!(cache.insert(base_key.clone(), value(94.0)));

        let different_width = key("Hello cache").with_width_constraint(320.0);
        let different_weight = key("Hello cache").with_font_weight(700);
        let different_spacing = key("Hello cache").with_letter_spacing(1.0);
        let different_direction = key("Hello cache").with_direction(Direction::RTL);
        let different_writing_mode = key("Hello cache").with_writing_mode(WritingMode::VerticalRl);
        let different_language = key("Hello cache").with_language("ja");
        let different_hash_identity = TextMeasureKey::from_text_hash(
            0x0123_4567_89ab_cdef,
            "Hello cache".len(),
            vec!["Manrope".to_string()],
            16.0,
        )
        .with_width_constraint(240.0)
        .with_language("en");

        assert!(cache.lookup(&base_key).is_some());
        assert!(cache.lookup(&different_width).is_none());
        assert!(cache.lookup(&different_weight).is_none());
        assert!(cache.lookup(&different_spacing).is_none());
        assert!(cache.lookup(&different_direction).is_none());
        assert!(cache.lookup(&different_writing_mode).is_none());
        assert!(cache.lookup(&different_language).is_none());
        assert!(cache.lookup(&different_hash_identity).is_none());
    }

    #[test]
    fn text_measure_cache_evicts_least_recent_entry_at_capacity() {
        let mut cache = TextMeasureCache::with_limits(TextMeasureCacheLimits::new(2, 64 * 1024));
        let first_key = key("first");
        let second_key = key("second");
        let third_key = key("third");

        assert!(cache.insert(first_key.clone(), value(40.0)));
        assert!(cache.insert(second_key.clone(), value(50.0)));
        assert!(cache.lookup(&first_key).is_some());
        assert!(cache.insert(third_key.clone(), value(60.0)));

        assert_eq!(cache.len(), 2);
        assert!(cache.contains_key(&first_key));
        assert!(!cache.contains_key(&second_key));
        assert!(cache.contains_key(&third_key));
        assert_eq!(cache.stats().evictions, 1);
        assert_eq!(cache.entry_utilization(), 1.0);
        assert!(cache.byte_utilization() > 0.0);
        assert!(cache.byte_utilization() <= 1.0);
    }

    #[test]
    fn text_measure_cache_stats_report_rates_and_eviction_pressure() {
        let stats = TextMeasureCacheStats {
            requests: 4,
            hits: 1,
            misses: 3,
            inserts: 4,
            evictions: 1,
            entries: 3,
            approximate_bytes: 128,
        };

        assert!((stats.hit_rate() - 0.25).abs() < 0.001);
        assert!((stats.miss_rate() - 0.75).abs() < 0.001);
        assert!((stats.eviction_rate() - 0.25).abs() < 0.001);
        assert!(stats.has_eviction_pressure());

        let empty = TextMeasureCacheStats::default();
        assert_eq!(empty.hit_rate(), 0.0);
        assert_eq!(empty.miss_rate(), 0.0);
        assert_eq!(empty.eviction_rate(), 0.0);
        assert!(!empty.has_eviction_pressure());
    }

    #[test]
    fn text_measure_cache_rejects_entries_over_byte_limit() {
        let mut cache = TextMeasureCache::with_limits(TextMeasureCacheLimits::new(8, 32));
        let cache_key = key("this entry is intentionally larger than the tiny byte budget");

        assert!(!cache.insert(cache_key.clone(), value(80.0)));
        assert!(!cache.contains_key(&cache_key));

        let stats = cache.stats();
        assert_eq!(stats.inserts, 0);
        assert_eq!(stats.evictions, 0);
        assert_eq!(stats.entries, 0);
        assert_eq!(stats.approximate_bytes, 0);
    }

    #[test]
    fn text_measure_cache_can_clear_entries_and_reset_counters() {
        let mut cache = TextMeasureCache::new();
        let cache_key = key("clear me");

        assert!(cache.insert(cache_key.clone(), value(70.0)));
        assert!(cache.lookup(&cache_key).is_some());
        cache.clear();

        let stats_after_clear = cache.stats();
        assert_eq!(stats_after_clear.entries, 0);
        assert_eq!(stats_after_clear.approximate_bytes, 0);
        assert_eq!(stats_after_clear.requests, 1);
        assert!(cache.lookup(&cache_key).is_none());

        cache.reset_stats();
        let stats_after_reset = cache.stats();
        assert_eq!(stats_after_reset.requests, 0);
        assert_eq!(stats_after_reset.hits, 0);
        assert_eq!(stats_after_reset.misses, 0);
        assert_eq!(stats_after_reset.inserts, 0);
        assert_eq!(stats_after_reset.entries, 0);
    }

    #[test]
    fn text_measure_cache_supports_batched_lookup_and_insert() {
        let mut cache = TextMeasureCache::new();
        let first_key = key("alpha");
        let second_key = key("beta");
        let third_key = key("gamma");

        let inserted = cache.insert_batch([
            (first_key.clone(), value(40.0)),
            (second_key.clone(), value(48.0)),
        ]);
        assert_eq!(inserted, 2);

        let results = cache.lookup_batch([&first_key, &second_key, &third_key]);
        assert_eq!(results[0].unwrap().width, 40.0);
        assert_eq!(results[1].unwrap().width, 48.0);
        assert!(results[2].is_none());

        let stats = cache.stats();
        assert_eq!(stats.requests, 3);
        assert_eq!(stats.hits, 2);
        assert_eq!(stats.misses, 1);
    }

    #[test]
    fn text_measure_value_is_plain_metrics_without_pixel_storage() {
        assert!(!mem::needs_drop::<TextMeasureValue>());
        assert!(mem::size_of::<TextMeasureValue>() <= 40);

        let metrics = value(128.0);
        assert_eq!(metrics.width, 128.0);
        assert_eq!(metrics.height, 19.2);
        assert_eq!(metrics.baseline, 12.8);
        assert_eq!(metrics.ascent, 12.8);
        assert_eq!(metrics.descent, 6.4);
        assert_eq!(metrics.line_count, 1);

        let mut cache = TextMeasureCache::new();
        assert!(cache.insert(key("pixels are not accepted here"), value(128.0)));

        let synthetic_rgba_glyph_bytes = 64 * 64 * 4;
        assert!(cache.approximate_bytes() < synthetic_rgba_glyph_bytes);
    }

    #[test]
    fn text_measure_api_is_reexported_from_crate_root() {
        let mut cache =
            crate::TextMeasureCache::with_limits(crate::TextMeasureCacheLimits::new(4, 64 * 1024));
        let cache_key = crate::TextMeasureKey::from_text(
            "crate root export",
            vec!["Manrope".to_string()],
            16.0,
        );
        let cache_value =
            crate::TextMeasureValue::new(120.0, 20.0, 14.0, 1).with_vertical_metrics(14.0, 6.0);

        assert!(cache.insert(cache_key.clone(), cache_value));
        assert_eq!(cache.lookup(&cache_key).unwrap().width, 120.0);
    }
}

#[cfg(test)]
mod integration_tests {
    use crate::cache::LayoutCache;
    use crate::constraints::*;
    use crate::dirty::*;
    use crate::measure::MeasureCache;
    use crate::policy::*;
    use crate::result::*;
    use crate::stats::FrameStatistics;

    /// Simulates a two-frame layout pass where a single node changes.
    #[test]
    fn two_frame_incremental_layout() {
        let mut cache = LayoutCache::new();
        let mut measure = MeasureCache::new();
        let mut dirty = DirtyPropagation::new();
        let mut stats = FrameStatistics::new();

        // Frame 1: full layout of 3 nodes (root=1, child=2, child=3)
        dirty.mark_all_dirty(vec![1, 2, 3]);

        for id in [1, 2, 3] {
            let c = LayoutConstraints::fixed(800.0, 600.0);
            let r = LayoutResult::with_size(if id == 1 { 800.0 } else { 400.0 }, 100.0);
            cache.store(id, c, r);
            measure.store_measure(id, IntrinsicSizes::new(50.0, 400.0));
            dirty.clear(id);
            stats.record_layout();
        }

        assert_eq!(stats.nodes_laid_out, 3);
        cache.advance_generation(3);

        // Frame 2: only node 3's content changed
        let mut stats2 = FrameStatistics::new();
        dirty.mark_dirty_and_propagate(3, LayoutDirtyFlags::CONTENT_CHANGED, |id| match id {
            3 => Some(1),
            2 => Some(1),
            _ => None,
        });

        // Node 1 has CHILD_NEEDS_LAYOUT — needs to re-layout children
        // Node 2 is clean — cache hit
        // Node 3 is dirty — full re-layout

        // Node 2: not dirty itself, so try cache
        if !dirty.needs_layout(2) {
            let c = LayoutConstraints::fixed(800.0, 600.0);
            let hit = cache.lookup(2, &c);
            assert!(hit.is_some());
            stats2.record_cache_hit();
        }

        // Node 3: dirty, re-layout
        assert!(dirty.needs_layout(3));
        let c3 = LayoutConstraints::fixed(800.0, 600.0);
        let new_r3 = LayoutResult::with_size(400.0, 120.0);
        cache.store(3, c3, new_r3);
        measure.invalidate_measure(3);
        measure.store_measure(3, IntrinsicSizes::new(60.0, 400.0));
        dirty.clear(3);
        stats2.record_layout();

        // Node 1: re-layout (has dirty child)
        let c1 = LayoutConstraints::fixed(800.0, 600.0);
        let new_r1 = LayoutResult::with_size(800.0, 220.0);
        cache.store(1, c1, new_r1);
        dirty.clear(1);
        stats2.record_layout();

        assert_eq!(stats2.nodes_laid_out, 2);
        assert_eq!(stats2.nodes_cache_hit, 1);
        assert!((stats2.cache_hit_rate() - 1.0 / 3.0).abs() < 0.01);
    }

    /// Tests cache policy interaction with the cache.
    #[test]
    fn policy_guided_caching() {
        let mut cache = LayoutCache::new();
        let policy = CachePolicy::new();

        let text_hints = SizingHints {
            is_leaf: true,
            ..Default::default()
        };
        let abs_hints = SizingHints::default();

        // Text node: policy says cache
        if policy.should_cache(DisplayType::Text, PositionType::Static, &text_hints, 0) {
            cache.store(
                10,
                LayoutConstraints::auto(),
                LayoutResult::with_size(50.0, 14.0),
            );
        }

        // Absolutely positioned: policy says skip
        if policy.should_cache(DisplayType::Block, PositionType::Absolute, &abs_hints, 2) {
            cache.store(20, LayoutConstraints::auto(), LayoutResult::default());
        }

        assert!(cache.has_entries(10));
        assert!(!cache.has_entries(20));
    }

    /// Tests that constraints with different writing modes are distinct cache keys.
    #[test]
    fn writing_mode_cache_separation() {
        let mut cache = LayoutCache::new();

        let mut c_htb = LayoutConstraints::fixed(800.0, 600.0);
        c_htb.writing_mode = WritingMode::HorizontalTb;

        let mut c_vlr = LayoutConstraints::fixed(800.0, 600.0);
        c_vlr.writing_mode = WritingMode::VerticalLr;

        cache.store(1, c_htb.clone(), LayoutResult::with_size(800.0, 600.0));
        cache.store(1, c_vlr.clone(), LayoutResult::with_size(600.0, 800.0));

        let htb_result = cache.lookup(1, &c_htb).unwrap();
        assert_eq!(htb_result.size, (800.0, 600.0));

        let vlr_result = cache.lookup(1, &c_vlr).unwrap();
        assert_eq!(vlr_result.size, (600.0, 800.0));
    }
}
