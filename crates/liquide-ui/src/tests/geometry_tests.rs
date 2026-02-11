//! Tests for geometry types.

use crate::geometry::{Corner, Insets, Point, Rect, Size};

#[test]
fn test_point_zero() {
    let p = Point::zero();
    assert_eq!(p.x, 0.0);
    assert_eq!(p.y, 0.0);
}

#[test]
fn test_point_new() {
    let p = Point::new(3.0, 4.0);
    assert_eq!(p.x, 3.0);
    assert_eq!(p.y, 4.0);
}

#[test]
fn test_point_distance_to_same() {
    let p = Point::new(1.0, 1.0);
    assert_eq!(p.distance_to(&p), 0.0);
}

#[test]
fn test_point_distance_to_3_4_5() {
    let a = Point::new(0.0, 0.0);
    let b = Point::new(3.0, 4.0);
    let dist = a.distance_to(&b);
    assert!((dist - 5.0).abs() < 0.001);
}

#[test]
fn test_point_distance_symmetry() {
    let a = Point::new(1.0, 2.0);
    let b = Point::new(4.0, 6.0);
    assert_eq!(a.distance_to(&b), b.distance_to(&a));
}

#[test]
fn test_size_zero() {
    let s = Size::zero();
    assert_eq!(s.width, 0.0);
    assert_eq!(s.height, 0.0);
    assert!(s.is_empty());
}

#[test]
fn test_size_area() {
    let s = Size::new(10.0, 20.0);
    assert_eq!(s.area(), 200.0);
}

#[test]
fn test_size_is_empty_negative() {
    let s = Size::new(-1.0, 10.0);
    assert!(s.is_empty());
}

#[test]
fn test_size_is_not_empty() {
    let s = Size::new(5.0, 5.0);
    assert!(!s.is_empty());
}

#[test]
fn test_rect_zero() {
    let r = Rect::zero();
    assert_eq!(r.x, 0.0);
    assert_eq!(r.y, 0.0);
    assert!(r.is_empty());
}

#[test]
fn test_rect_contains_point_interior() {
    let r = Rect::new(10.0, 10.0, 100.0, 100.0);
    assert!(r.contains_point(Point::new(50.0, 50.0)));
}

#[test]
fn test_rect_contains_point_on_edge() {
    let r = Rect::new(0.0, 0.0, 10.0, 10.0);
    assert!(r.contains_point(Point::new(0.0, 0.0)));
    assert!(r.contains_point(Point::new(10.0, 10.0)));
}

#[test]
fn test_rect_does_not_contain_outside_point() {
    let r = Rect::new(10.0, 10.0, 100.0, 100.0);
    assert!(!r.contains_point(Point::new(5.0, 5.0)));
    assert!(!r.contains_point(Point::new(200.0, 200.0)));
}

#[test]
fn test_rect_intersects_overlap() {
    let a = Rect::new(0.0, 0.0, 10.0, 10.0);
    let b = Rect::new(5.0, 5.0, 10.0, 10.0);
    assert!(a.intersects(&b));
    assert!(b.intersects(&a));
}

#[test]
fn test_rect_does_not_intersect_disjoint() {
    let a = Rect::new(0.0, 0.0, 10.0, 10.0);
    let b = Rect::new(20.0, 20.0, 10.0, 10.0);
    assert!(!a.intersects(&b));
}

#[test]
fn test_rect_intersection_some() {
    let a = Rect::new(0.0, 0.0, 10.0, 10.0);
    let b = Rect::new(5.0, 5.0, 10.0, 10.0);
    let inter = a.intersection(&b).unwrap();
    assert_eq!(inter.x, 5.0);
    assert_eq!(inter.y, 5.0);
    assert_eq!(inter.width, 5.0);
    assert_eq!(inter.height, 5.0);
}

#[test]
fn test_rect_intersection_none() {
    let a = Rect::new(0.0, 0.0, 10.0, 10.0);
    let b = Rect::new(20.0, 20.0, 10.0, 10.0);
    assert!(a.intersection(&b).is_none());
}

#[test]
fn test_rect_union() {
    let a = Rect::new(0.0, 0.0, 10.0, 10.0);
    let b = Rect::new(5.0, 5.0, 10.0, 10.0);
    let u = a.union_rect(&b);
    assert_eq!(u.x, 0.0);
    assert_eq!(u.y, 0.0);
    assert_eq!(u.width, 15.0);
    assert_eq!(u.height, 15.0);
}

#[test]
fn test_rect_center() {
    let r = Rect::new(10.0, 20.0, 100.0, 200.0);
    let c = r.center();
    assert_eq!(c.x, 60.0);
    assert_eq!(c.y, 120.0);
}

#[test]
fn test_rect_origin() {
    let r = Rect::new(5.0, 10.0, 50.0, 50.0);
    let o = r.origin();
    assert_eq!(o.x, 5.0);
    assert_eq!(o.y, 10.0);
}

#[test]
fn test_rect_size() {
    let r = Rect::new(0.0, 0.0, 30.0, 40.0);
    let s = r.size();
    assert_eq!(s.width, 30.0);
    assert_eq!(s.height, 40.0);
}

#[test]
fn test_rect_is_empty_zero_width() {
    let r = Rect::new(0.0, 0.0, 0.0, 10.0);
    assert!(r.is_empty());
}

#[test]
fn test_insets_all() {
    let i = Insets::all(5.0);
    assert_eq!(i.top, 5.0);
    assert_eq!(i.right, 5.0);
    assert_eq!(i.bottom, 5.0);
    assert_eq!(i.left, 5.0);
}

#[test]
fn test_insets_symmetric() {
    let i = Insets::symmetric(10.0, 20.0);
    assert_eq!(i.top, 10.0);
    assert_eq!(i.bottom, 10.0);
    assert_eq!(i.right, 20.0);
    assert_eq!(i.left, 20.0);
}

#[test]
fn test_insets_new() {
    let i = Insets::new(1.0, 2.0, 3.0, 4.0);
    assert_eq!(i.top, 1.0);
    assert_eq!(i.right, 2.0);
    assert_eq!(i.bottom, 3.0);
    assert_eq!(i.left, 4.0);
}

#[test]
fn test_corner_all() {
    let c = Corner::all(8.0);
    assert_eq!(c.top_left, 8.0);
    assert_eq!(c.top_right, 8.0);
    assert_eq!(c.bottom_right, 8.0);
    assert_eq!(c.bottom_left, 8.0);
}

#[test]
fn test_corner_individual() {
    let c = Corner::new(1.0, 2.0, 3.0, 4.0);
    assert_eq!(c.top_left, 1.0);
    assert_eq!(c.top_right, 2.0);
    assert_eq!(c.bottom_right, 3.0);
    assert_eq!(c.bottom_left, 4.0);
}

#[test]
fn test_point_default() {
    let p = Point::default();
    assert_eq!(p, Point::zero());
}

#[test]
fn test_size_default() {
    let s = Size::default();
    assert_eq!(s, Size::zero());
}

#[test]
fn test_rect_default() {
    let r = Rect::default();
    assert_eq!(r, Rect::zero());
}
