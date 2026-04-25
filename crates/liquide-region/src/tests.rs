//! Comprehensive tests for the region crate.

#[cfg(test)]
mod rect_tests {
    use crate::rect::Rect;

    #[test]
    fn empty_rect() {
        let r = Rect::new(10, 10, 10, 20);
        assert!(r.is_empty());
        let r2 = Rect::new(10, 10, 20, 10);
        assert!(r2.is_empty());
        let r3 = Rect::new(20, 10, 10, 20);
        assert!(r3.is_empty());
    }

    #[test]
    fn rect_dimensions() {
        let r = Rect::new(10, 20, 50, 60);
        assert_eq!(r.width(), 40);
        assert_eq!(r.height(), 40);
        assert_eq!(r.area(), 1600);
    }

    #[test]
    fn rect_contains_point() {
        let r = Rect::new(10, 20, 50, 60);
        assert!(r.contains(10, 20)); // top-left inclusive
        assert!(r.contains(49, 59)); // bottom-right just inside
        assert!(!r.contains(50, 60)); // exclusive edges
        assert!(!r.contains(9, 20));
        assert!(!r.contains(10, 19));
    }

    #[test]
    fn rect_intersects() {
        let a = Rect::new(0, 0, 100, 100);
        let b = Rect::new(50, 50, 150, 150);
        assert!(a.intersects(&b));
        assert!(b.intersects(&a));

        let c = Rect::new(100, 0, 200, 100); // touches but doesn't overlap
        assert!(!a.intersects(&c));
    }

    #[test]
    fn rect_intersection() {
        let a = Rect::new(0, 0, 100, 100);
        let b = Rect::new(50, 50, 150, 150);
        let i = a.intersection(&b).unwrap();
        assert_eq!(i, Rect::new(50, 50, 100, 100));

        let c = Rect::new(200, 200, 300, 300);
        assert!(a.intersection(&c).is_none());
    }

    #[test]
    fn rect_union() {
        let a = Rect::new(10, 10, 20, 20);
        let b = Rect::new(30, 30, 40, 40);
        let u = a.union(&b);
        assert_eq!(
            u,
            Rect {
                left: 10,
                top: 10,
                right: 40,
                bottom: 40
            }
        );
    }

    #[test]
    fn rect_offset() {
        let r = Rect::new(10, 20, 30, 40);
        let o = r.offset(5, -5);
        assert_eq!(
            o,
            Rect {
                left: 15,
                top: 15,
                right: 35,
                bottom: 35
            }
        );
    }

    #[test]
    fn rect_inflate() {
        let r = Rect::new(10, 10, 20, 20);
        let inflated = r.inflate(5, 5);
        assert_eq!(
            inflated,
            Rect {
                left: 5,
                top: 5,
                right: 25,
                bottom: 25
            }
        );

        // Deflate to nothing
        let shrunk = r.inflate(-6, -6);
        assert!(shrunk.is_empty());
    }

    #[test]
    fn rect_contains_rect() {
        let outer = Rect::new(0, 0, 100, 100);
        let inner = Rect::new(10, 10, 90, 90);
        assert!(outer.contains_rect(&inner));
        assert!(!inner.contains_rect(&outer));

        // Empty rect is contained by everything
        let empty = Rect::new(0, 0, 0, 0);
        assert!(outer.contains_rect(&empty));
    }

    #[test]
    fn rect_union_with_empty() {
        let a = Rect::new(10, 10, 20, 20);
        let empty = Rect::new(0, 0, 0, 0);
        assert_eq!(a.union(&empty), a);
        assert_eq!(empty.union(&a), a);
    }
}

#[cfg(test)]
mod band_tests {
    use crate::band::*;
    use crate::rect::Rect;

    #[test]
    fn merge_spans_non_overlapping() {
        let spans = vec![Span::new(0, 10), Span::new(20, 30)];
        let merged = merge_spans(spans);
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn merge_spans_overlapping() {
        let spans = vec![Span::new(0, 15), Span::new(10, 25)];
        let merged = merge_spans(spans);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0], Span::new(0, 25));
    }

    #[test]
    fn merge_spans_touching() {
        let spans = vec![Span::new(0, 10), Span::new(10, 20)];
        let merged = merge_spans(spans);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0], Span::new(0, 20));
    }

    #[test]
    fn rects_to_bands_single() {
        let bands = rects_to_bands(&[Rect::new(10, 20, 50, 60)]);
        assert_eq!(bands.len(), 1);
        assert_eq!(bands[0].y_top, 20);
        assert_eq!(bands[0].y_bottom, 60);
        assert_eq!(bands[0].spans.len(), 1);
        assert_eq!(bands[0].spans[0], Span::new(10, 50));
    }

    #[test]
    fn rects_to_bands_overlapping() {
        // Two overlapping rectangles
        let bands = rects_to_bands(&[Rect::new(0, 0, 20, 20), Rect::new(10, 10, 30, 30)]);
        // Should produce 3 bands:
        // y=[0,10): span [0,20)
        // y=[10,20): span [0,30) (merged)
        // y=[20,30): span [10,30)
        assert_eq!(bands.len(), 3);
        assert_eq!(bands[0].y_top, 0);
        assert_eq!(bands[0].y_bottom, 10);
        assert_eq!(bands[0].spans, vec![Span::new(0, 20)]);

        assert_eq!(bands[1].y_top, 10);
        assert_eq!(bands[1].y_bottom, 20);
        assert_eq!(bands[1].spans, vec![Span::new(0, 30)]);

        assert_eq!(bands[2].y_top, 20);
        assert_eq!(bands[2].y_bottom, 30);
        assert_eq!(bands[2].spans, vec![Span::new(10, 30)]);
    }

    #[test]
    fn rects_to_bands_coalesce() {
        // Two vertically adjacent rects with same x range -> coalesced
        let bands = rects_to_bands(&[Rect::new(0, 0, 100, 50), Rect::new(0, 50, 100, 100)]);
        assert_eq!(bands.len(), 1);
        assert_eq!(bands[0].y_top, 0);
        assert_eq!(bands[0].y_bottom, 100);
    }

    #[test]
    fn rects_to_bands_empty_rects_ignored() {
        let bands = rects_to_bands(&[
            Rect::new(0, 0, 0, 0), // empty
            Rect::new(10, 10, 20, 20),
        ]);
        assert_eq!(bands.len(), 1);
    }

    #[test]
    fn spans_union_basic() {
        let a = vec![Span::new(0, 10), Span::new(20, 30)];
        let b = vec![Span::new(5, 25)];
        let u = spans_union(&a, &b);
        assert_eq!(u, vec![Span::new(0, 30)]);
    }

    #[test]
    fn spans_intersect_basic() {
        let a = vec![Span::new(0, 20)];
        let b = vec![Span::new(10, 30)];
        let i = spans_intersect(&a, &b);
        assert_eq!(i, vec![Span::new(10, 20)]);
    }

    #[test]
    fn spans_intersect_no_overlap() {
        let a = vec![Span::new(0, 10)];
        let b = vec![Span::new(20, 30)];
        let i = spans_intersect(&a, &b);
        assert!(i.is_empty());
    }

    #[test]
    fn spans_subtract_basic() {
        let a = vec![Span::new(0, 30)];
        let b = vec![Span::new(10, 20)];
        let s = spans_subtract(&a, &b);
        assert_eq!(s, vec![Span::new(0, 10), Span::new(20, 30)]);
    }

    #[test]
    fn spans_subtract_no_overlap() {
        let a = vec![Span::new(0, 10)];
        let b = vec![Span::new(20, 30)];
        let s = spans_subtract(&a, &b);
        assert_eq!(s, vec![Span::new(0, 10)]);
    }

    #[test]
    fn spans_xor_basic() {
        let a = vec![Span::new(0, 20)];
        let b = vec![Span::new(10, 30)];
        let x = spans_xor(&a, &b);
        assert_eq!(x, vec![Span::new(0, 10), Span::new(20, 30)]);
    }
}

#[cfg(test)]
mod region_tests {
    use crate::rect::Rect;
    use crate::region::{Region, RegionComplexity};

    #[test]
    fn empty_region() {
        let r = Region::empty();
        assert!(r.is_empty());
        assert!(!r.is_full());
        assert_eq!(r.complexity(), RegionComplexity::Empty);
        assert_eq!(r.rect_count(), 0);
        assert!(r.bounding_rect().is_none());
    }

    #[test]
    fn full_region() {
        let r = Region::FULL;
        assert!(r.is_full());
        assert!(!r.is_empty());
        assert!(r.contains_point(0, 0));
        assert!(r.contains_point(i32::MAX / 2, i32::MAX / 2));
    }

    #[test]
    fn single_rect_region() {
        let r = Region::from_rect(Rect::new(10, 20, 50, 60));
        assert_eq!(r.complexity(), RegionComplexity::Simple);
        assert_eq!(r.rect_count(), 1);
        assert_eq!(r.bounding_rect(), Some(Rect::new(10, 20, 50, 60)));
        assert!(r.contains_point(10, 20));
        assert!(!r.contains_point(50, 60));
    }

    #[test]
    fn region_from_empty_rect() {
        let r = Region::from_rect(Rect::new(0, 0, 0, 0));
        assert!(r.is_empty());
    }

    #[test]
    fn multi_rect_region() {
        let r = Region::from_rects(&[Rect::new(0, 0, 10, 10), Rect::new(20, 0, 30, 10)]);
        assert_eq!(r.complexity(), RegionComplexity::Complex);
        assert_eq!(r.rect_count(), 2);
        assert!(r.contains_point(5, 5));
        assert!(r.contains_point(25, 5));
        assert!(!r.contains_point(15, 5));
    }

    #[test]
    fn region_union_disjoint() {
        let a = Region::from_rect(Rect::new(0, 0, 10, 10));
        let b = Region::from_rect(Rect::new(20, 20, 30, 30));
        let u = a.union(&b);
        assert_eq!(u.rect_count(), 2);
        assert!(u.contains_point(5, 5));
        assert!(u.contains_point(25, 25));
        assert!(!u.contains_point(15, 15));
    }

    #[test]
    fn region_union_overlapping() {
        let a = Region::from_rect(Rect::new(0, 0, 20, 20));
        let b = Region::from_rect(Rect::new(10, 10, 30, 30));
        let u = a.union(&b);
        assert!(u.contains_point(5, 5));
        assert!(u.contains_point(15, 15));
        assert!(u.contains_point(25, 25));
        assert!(!u.contains_point(5, 25));
    }

    #[test]
    fn region_union_with_empty() {
        let a = Region::from_rect(Rect::new(0, 0, 10, 10));
        let empty = Region::empty();
        assert!(a.union(&empty).equals(&a));
        assert!(empty.union(&a).equals(&a));
    }

    #[test]
    fn region_union_with_full() {
        let a = Region::from_rect(Rect::new(0, 0, 10, 10));
        assert!(a.union(&Region::FULL).is_full());
        assert!(Region::FULL.union(&a).is_full());
    }

    #[test]
    fn region_intersect_overlapping() {
        let a = Region::from_rect(Rect::new(0, 0, 20, 20));
        let b = Region::from_rect(Rect::new(10, 10, 30, 30));
        let i = a.intersect(&b);
        assert_eq!(i.bounding_rect(), Some(Rect::new(10, 10, 20, 20)));
        assert!(i.contains_point(15, 15));
        assert!(!i.contains_point(5, 5));
        assert!(!i.contains_point(25, 25));
    }

    #[test]
    fn region_intersect_disjoint() {
        let a = Region::from_rect(Rect::new(0, 0, 10, 10));
        let b = Region::from_rect(Rect::new(20, 20, 30, 30));
        let i = a.intersect(&b);
        assert!(i.is_empty());
    }

    #[test]
    fn region_intersect_with_empty() {
        let a = Region::from_rect(Rect::new(0, 0, 10, 10));
        assert!(a.intersect(&Region::empty()).is_empty());
        assert!(Region::empty().intersect(&a).is_empty());
    }

    #[test]
    fn region_intersect_with_full() {
        let a = Region::from_rect(Rect::new(0, 0, 10, 10));
        assert!(a.intersect(&Region::FULL).equals(&a));
        assert!(Region::FULL.intersect(&a).equals(&a));
    }

    #[test]
    fn region_subtract_basic() {
        let a = Region::from_rect(Rect::new(0, 0, 30, 10));
        let b = Region::from_rect(Rect::new(10, 0, 20, 10));
        let s = a.subtract(&b);
        assert_eq!(s.rect_count(), 2);
        assert!(s.contains_point(5, 5));
        assert!(!s.contains_point(15, 5));
        assert!(s.contains_point(25, 5));
    }

    #[test]
    fn region_subtract_no_overlap() {
        let a = Region::from_rect(Rect::new(0, 0, 10, 10));
        let b = Region::from_rect(Rect::new(20, 20, 30, 30));
        let s = a.subtract(&b);
        assert!(s.equals(&a));
    }

    #[test]
    fn region_subtract_from_empty() {
        let a = Region::empty();
        let b = Region::from_rect(Rect::new(0, 0, 10, 10));
        assert!(a.subtract(&b).is_empty());
    }

    #[test]
    fn region_subtract_empty() {
        let a = Region::from_rect(Rect::new(0, 0, 10, 10));
        let s = a.subtract(&Region::empty());
        assert!(s.equals(&a));
    }

    #[test]
    fn region_subtract_full() {
        let a = Region::from_rect(Rect::new(0, 0, 10, 10));
        let s = a.subtract(&Region::FULL);
        assert!(s.is_empty());
    }

    #[test]
    fn region_xor_basic() {
        let a = Region::from_rect(Rect::new(0, 0, 20, 10));
        let b = Region::from_rect(Rect::new(10, 0, 30, 10));
        let x = a.xor(&b);
        assert!(x.contains_point(5, 5));
        assert!(!x.contains_point(15, 5));
        assert!(x.contains_point(25, 5));
    }

    #[test]
    fn region_xor_identical() {
        let a = Region::from_rect(Rect::new(0, 0, 10, 10));
        let b = Region::from_rect(Rect::new(0, 0, 10, 10));
        let x = a.xor(&b);
        assert!(x.is_empty());
    }

    #[test]
    fn region_xor_with_empty() {
        let a = Region::from_rect(Rect::new(0, 0, 10, 10));
        assert!(a.xor(&Region::empty()).equals(&a));
        assert!(Region::empty().xor(&a).equals(&a));
    }

    #[test]
    fn region_offset() {
        let r = Region::from_rect(Rect::new(10, 20, 30, 40));
        let o = r.offset(5, -5);
        assert_eq!(o.bounding_rect(), Some(Rect::new(15, 15, 35, 35)));
        assert!(o.contains_point(20, 20));
    }

    #[test]
    fn region_offset_empty() {
        let r = Region::empty();
        let o = r.offset(10, 10);
        assert!(o.is_empty());
    }

    #[test]
    fn region_offset_full() {
        let r = Region::FULL;
        let o = r.offset(10, 10);
        assert!(o.is_full());
    }

    #[test]
    fn region_contains_point_complex() {
        // L-shaped region
        let r = Region::from_rects(&[
            Rect::new(0, 0, 10, 30),  // vertical bar
            Rect::new(0, 20, 30, 30), // horizontal bar
        ]);
        assert!(r.contains_point(5, 5)); // in vertical bar
        assert!(r.contains_point(5, 25)); // in overlap
        assert!(r.contains_point(25, 25)); // in horizontal bar only
        assert!(!r.contains_point(25, 5)); // outside
    }

    #[test]
    fn region_contains_rect() {
        let r = Region::from_rect(Rect::new(0, 0, 100, 100));
        assert!(r.contains_rect(&Rect::new(10, 10, 90, 90)));
        assert!(r.contains_rect(&Rect::new(0, 0, 100, 100))); // exact match
        assert!(!r.contains_rect(&Rect::new(-10, 0, 50, 50))); // extends left
    }

    #[test]
    fn region_contains_rect_complex() {
        // Two horizontal bars with a gap
        let r = Region::from_rects(&[Rect::new(0, 0, 100, 10), Rect::new(0, 20, 100, 30)]);
        assert!(r.contains_rect(&Rect::new(10, 0, 90, 10))); // fits in first bar
        assert!(!r.contains_rect(&Rect::new(0, 0, 100, 30))); // spans the gap
    }

    #[test]
    fn region_intersects_rect() {
        let r = Region::from_rect(Rect::new(10, 10, 50, 50));
        assert!(r.intersects_rect(&Rect::new(0, 0, 20, 20)));
        assert!(!r.intersects_rect(&Rect::new(50, 50, 60, 60)));
        assert!(!r.intersects_rect(&Rect::new(0, 0, 10, 10))); // touching edge
    }

    #[test]
    fn region_equals() {
        let a = Region::from_rect(Rect::new(0, 0, 10, 10));
        let b = Region::from_rect(Rect::new(0, 0, 10, 10));
        assert!(a.equals(&b));
        assert_eq!(a, b);

        let c = Region::from_rect(Rect::new(0, 0, 20, 20));
        assert!(!a.equals(&c));
    }

    #[test]
    fn region_rects_roundtrip() {
        let original = vec![
            Rect::new(0, 0, 10, 10),
            Rect::new(20, 0, 30, 10),
            Rect::new(0, 20, 30, 30),
        ];
        let r = Region::from_rects(&original);
        let rects = r.rects();
        // Verify all original rectangles are covered.
        for orig in &original {
            for y in orig.top..orig.bottom {
                for x in orig.left..orig.right {
                    assert!(
                        rects.iter().any(|r| r.contains(x, y)),
                        "Point ({}, {}) should be in region rects",
                        x,
                        y
                    );
                }
            }
        }
    }

    #[test]
    fn region_complex_union_and_subtract() {
        // Build a complex shape through operations
        let a = Region::from_rect(Rect::new(0, 0, 50, 50));
        let b = Region::from_rect(Rect::new(25, 25, 75, 75));
        let u = a.union(&b);

        // Subtract a hole
        let hole = Region::from_rect(Rect::new(20, 20, 30, 30));
        let result = u.subtract(&hole);

        assert!(result.contains_point(10, 10)); // top-left
        assert!(!result.contains_point(25, 25)); // in hole
        assert!(result.contains_point(60, 60)); // bottom-right
    }

    #[test]
    fn region_union_adjacent_rects_coalesce() {
        // Two adjacent rects with same X range should coalesce into one band
        let a = Region::from_rect(Rect::new(0, 0, 100, 50));
        let b = Region::from_rect(Rect::new(0, 50, 100, 100));
        let u = a.union(&b);
        // Should be one coalesced band (simple rect)
        assert_eq!(u.complexity(), RegionComplexity::Simple);
        assert_eq!(u.bounding_rect(), Some(Rect::new(0, 0, 100, 100)));
    }

    #[test]
    fn region_intersect_complex() {
        // Cross shape: horizontal + vertical bars
        let h = Region::from_rect(Rect::new(0, 40, 100, 60));
        let v = Region::from_rect(Rect::new(40, 0, 60, 100));
        let cross = h.union(&v);

        // Intersect with a square
        let sq = Region::from_rect(Rect::new(30, 30, 70, 70));
        let result = cross.intersect(&sq);

        assert!(result.contains_point(50, 50)); // center
        assert!(result.contains_point(35, 50)); // in h bar, within sq
        assert!(result.contains_point(50, 35)); // in v bar, within sq
        assert!(!result.contains_point(10, 50)); // in h bar, outside sq
        assert!(!result.contains_point(50, 10)); // in v bar, outside sq
    }

    #[test]
    fn region_many_small_rects() {
        // Stress test: 100 small rects
        let rects: Vec<Rect> = (0..100)
            .map(|i| Rect::new(i * 3, 0, i * 3 + 2, 10))
            .collect();
        let r = Region::from_rects(&rects);
        assert_eq!(r.rect_count(), 100);
        for i in 0..100 {
            assert!(r.contains_point(i * 3, 5));
            assert!(r.contains_point(i * 3 + 1, 5));
            assert!(!r.contains_point(i * 3 + 2, 5)); // gap
        }
    }

    #[test]
    fn region_subtract_splits_band() {
        // Subtracting from the middle of a span should split it
        let r = Region::from_rect(Rect::new(0, 0, 100, 10));
        let hole = Region::from_rect(Rect::new(40, 0, 60, 10));
        let result = r.subtract(&hole);
        assert_eq!(result.rect_count(), 2);
        assert!(result.contains_point(20, 5));
        assert!(!result.contains_point(50, 5));
        assert!(result.contains_point(80, 5));
    }

    #[test]
    fn region_subtract_vertical_split() {
        // Subtracting a horizontal band from a tall rect
        let r = Region::from_rect(Rect::new(0, 0, 10, 30));
        let hole = Region::from_rect(Rect::new(0, 10, 10, 20));
        let result = r.subtract(&hole);
        assert_eq!(result.rect_count(), 2);
        assert!(result.contains_point(5, 5)); // above hole
        assert!(!result.contains_point(5, 15)); // in hole
        assert!(result.contains_point(5, 25)); // below hole
    }
}

#[cfg(test)]
mod builder_tests {
    use crate::rect::Rect;
    use crate::region::RegionBuilder;

    #[test]
    fn builder_empty() {
        let b = RegionBuilder::new();
        let r = b.build();
        assert!(r.is_empty());
    }

    #[test]
    fn builder_single_rect() {
        let mut b = RegionBuilder::new();
        b.add_rect(Rect::new(10, 10, 50, 50));
        let r = b.build();
        assert_eq!(r.rect_count(), 1);
    }

    #[test]
    fn builder_multiple_rects() {
        let mut b = RegionBuilder::with_capacity(3);
        b.add_rect(Rect::new(0, 0, 10, 10));
        b.add_rect(Rect::new(5, 5, 15, 15));
        b.add_rect(Rect::new(20, 20, 30, 30));
        let r = b.build();
        assert!(r.contains_point(5, 5));
        assert!(r.contains_point(12, 12));
        assert!(r.contains_point(25, 25));
        assert!(!r.contains_point(17, 17));
    }

    #[test]
    fn builder_ignores_empty_rects() {
        let mut b = RegionBuilder::new();
        b.add_rect(Rect::new(0, 0, 0, 0));
        b.add_rect(Rect::new(10, 10, 5, 5)); // inverted = empty
        let r = b.build();
        assert!(r.is_empty());
    }
}

#[cfg(test)]
mod invalid_tests {
    use crate::invalid::InvalidRegion;
    use crate::rect::Rect;

    #[test]
    fn initially_clean() {
        let inv = InvalidRegion::new();
        assert!(!inv.is_dirty());
    }

    #[test]
    fn initially_full() {
        let inv = InvalidRegion::new_full();
        assert!(inv.is_dirty());
        assert!(inv.region().is_full());
    }

    #[test]
    fn invalidate_rect() {
        let mut inv = InvalidRegion::new();
        inv.invalidate(Some(Rect::new(0, 0, 100, 100)));
        assert!(inv.is_dirty());
        assert!(inv.region().contains_point(50, 50));
    }

    #[test]
    fn invalidate_none_goes_full() {
        let mut inv = InvalidRegion::new();
        inv.invalidate(None);
        assert!(inv.is_dirty());
        assert!(inv.region().is_full());
    }

    #[test]
    fn validate_rect() {
        let mut inv = InvalidRegion::new();
        inv.invalidate(Some(Rect::new(0, 0, 100, 100)));
        inv.validate(Some(Rect::new(0, 0, 50, 100)));
        assert!(inv.is_dirty());
        assert!(!inv.region().contains_point(25, 50));
        assert!(inv.region().contains_point(75, 50));
    }

    #[test]
    fn validate_none_clears_all() {
        let mut inv = InvalidRegion::new();
        inv.invalidate(Some(Rect::new(0, 0, 100, 100)));
        inv.validate(None);
        assert!(!inv.is_dirty());
    }

    #[test]
    fn take_clears_region() {
        let mut inv = InvalidRegion::new();
        inv.invalidate(Some(Rect::new(0, 0, 100, 100)));
        let taken = inv.take();
        assert!(!inv.is_dirty());
        assert!(taken.contains_point(50, 50));
    }

    #[test]
    fn multiple_invalidations_accumulate() {
        let mut inv = InvalidRegion::new();
        inv.invalidate(Some(Rect::new(0, 0, 10, 10)));
        inv.invalidate(Some(Rect::new(20, 20, 30, 30)));
        assert!(inv.region().contains_point(5, 5));
        assert!(inv.region().contains_point(25, 25));
        assert!(!inv.region().contains_point(15, 15));
    }

    #[test]
    fn invalidate_empty_rect_is_noop() {
        let mut inv = InvalidRegion::new();
        inv.invalidate(Some(Rect::new(10, 10, 10, 10)));
        assert!(!inv.is_dirty());
    }
}

#[cfg(test)]
mod clip_tests {
    use crate::clip::ClipRegion;
    use crate::rect::Rect;

    #[test]
    fn new_clip_is_full() {
        let clip = ClipRegion::new();
        assert!(clip.current().is_full());
        assert_eq!(clip.depth(), 1);
    }

    #[test]
    fn push_pop_clip() {
        let mut clip = ClipRegion::new();
        clip.push_clip(Rect::new(0, 0, 100, 100));
        assert_eq!(clip.depth(), 2);
        assert!(!clip.current().is_full());
        assert!(clip.current().contains_point(50, 50));
        assert!(!clip.current().contains_point(150, 150));

        clip.pop_clip();
        assert_eq!(clip.depth(), 1);
        assert!(clip.current().is_full());
    }

    #[test]
    fn nested_clips_intersect() {
        let mut clip = ClipRegion::new();
        clip.push_clip(Rect::new(0, 0, 100, 100));
        clip.push_clip(Rect::new(50, 50, 150, 150));
        // Intersection should be [50,50)x[100,100)
        assert!(clip.current().contains_point(75, 75));
        assert!(!clip.current().contains_point(25, 25));
        assert!(!clip.current().contains_point(125, 125));

        clip.pop_clip();
        assert!(clip.current().contains_point(25, 25)); // back to first clip
    }

    #[test]
    fn is_visible() {
        let mut clip = ClipRegion::new();
        clip.push_clip(Rect::new(10, 10, 50, 50));
        assert!(clip.is_visible(&Rect::new(20, 20, 30, 30))); // fully inside
        assert!(clip.is_visible(&Rect::new(0, 0, 20, 20))); // partially inside
        assert!(!clip.is_visible(&Rect::new(50, 50, 60, 60))); // fully outside
        assert!(!clip.is_visible(&Rect::new(0, 0, 0, 0))); // empty rect
    }

    #[test]
    fn from_rect() {
        let clip = ClipRegion::from_rect(Rect::new(10, 10, 50, 50));
        assert!(!clip.current().is_full());
        assert!(clip.current().contains_point(30, 30));
    }

    #[test]
    fn reset() {
        let mut clip = ClipRegion::new();
        clip.push_clip(Rect::new(0, 0, 10, 10));
        clip.push_clip(Rect::new(0, 0, 5, 5));
        assert_eq!(clip.depth(), 3);
        clip.reset();
        assert_eq!(clip.depth(), 1);
        assert!(clip.current().is_full());
    }

    #[test]
    #[should_panic(expected = "cannot pop the initial clip")]
    fn pop_underflow_panics() {
        let mut clip = ClipRegion::new();
        clip.pop_clip(); // should panic
    }

    #[test]
    fn deeply_nested_clips() {
        let mut clip = ClipRegion::new();
        // Nest 10 clips, each shrinking by 10px on each side
        for i in 0..10 {
            let margin = (i * 10) as i32;
            clip.push_clip(Rect::new(margin, margin, 200 - margin, 200 - margin));
        }
        assert_eq!(clip.depth(), 11);
        // Innermost clip is [90,90)x[110,110)
        assert!(clip.current().contains_point(100, 100));
        assert!(!clip.current().contains_point(80, 80));

        // Pop all back
        for _ in 0..10 {
            clip.pop_clip();
        }
        assert!(clip.current().is_full());
    }
}

#[cfg(test)]
mod paint_tests {
    use crate::invalid::InvalidRegion;
    use crate::paint::{begin_paint, begin_paint_bounded, end_paint};
    use crate::rect::Rect;

    #[test]
    fn begin_paint_empty_returns_none() {
        let mut inv = InvalidRegion::new();
        assert!(begin_paint(1, &mut inv).is_none());
    }

    #[test]
    fn begin_paint_takes_region() {
        let mut inv = InvalidRegion::new();
        inv.invalidate(Some(Rect::new(10, 10, 50, 50)));
        let ctx = begin_paint(1, &mut inv).unwrap();
        assert_eq!(ctx.window_id, 1);
        assert_eq!(ctx.update_rect, Rect::new(10, 10, 50, 50));
        assert!(ctx.erase_background);
        assert!(!inv.is_dirty()); // region was taken
        end_paint(ctx, &mut inv);
    }

    #[test]
    fn begin_paint_bounded_resolves_full() {
        let mut inv = InvalidRegion::new_full();
        let window = Rect::new(0, 0, 800, 600);
        let ctx = begin_paint_bounded(42, &mut inv, window).unwrap();
        assert_eq!(ctx.update_rect, window);
        assert!(!ctx.clip.is_full()); // resolved to actual rect
        assert!(ctx.clip.contains_point(400, 300));
        end_paint(ctx, &mut inv);
    }

    #[test]
    fn begin_paint_full_region() {
        let mut inv = InvalidRegion::new_full();
        let ctx = begin_paint(1, &mut inv).unwrap();
        assert!(ctx.clip.is_full());
        end_paint(ctx, &mut inv);
    }

    #[test]
    fn paint_cycle_leaves_clean() {
        let mut inv = InvalidRegion::new();
        inv.invalidate(Some(Rect::new(0, 0, 100, 100)));
        let ctx = begin_paint(1, &mut inv).unwrap();
        end_paint(ctx, &mut inv);
        assert!(!inv.is_dirty());
    }

    #[test]
    fn new_invalidation_after_begin_paint_survives() {
        let mut inv = InvalidRegion::new();
        inv.invalidate(Some(Rect::new(0, 0, 50, 50)));
        let ctx = begin_paint(1, &mut inv).unwrap();
        // Simulate new damage arriving during paint
        inv.invalidate(Some(Rect::new(60, 60, 80, 80)));
        end_paint(ctx, &mut inv);
        // The new damage should survive
        assert!(inv.is_dirty());
        assert!(inv.region().contains_point(70, 70));
    }
}
