use crate::geometry::*;

#[test]
fn rect_contains() {
    let r = Rect::new(10.0, 20.0, 100.0, 50.0);
    assert!(r.contains(Point::new(10.0, 20.0)));
    assert!(r.contains(Point::new(50.0, 40.0)));
    assert!(!r.contains(Point::new(110.0, 70.0)));
    assert!(!r.contains(Point::new(9.0, 20.0)));
}

#[test]
fn rect_intersection() {
    let a = Rect::new(0.0, 0.0, 100.0, 100.0);
    let b = Rect::new(50.0, 50.0, 100.0, 100.0);
    let i = a.intersection(&b).unwrap();
    assert_eq!(i.x, 50.0);
    assert_eq!(i.y, 50.0);
    assert_eq!(i.width, 50.0);
    assert_eq!(i.height, 50.0);
}

#[test]
fn rect_no_intersection() {
    let a = Rect::new(0.0, 0.0, 10.0, 10.0);
    let b = Rect::new(20.0, 20.0, 10.0, 10.0);
    assert!(a.intersection(&b).is_none());
}

#[test]
fn rect_tile_coords() {
    let r = Rect::new(30.0, 70.0, 100.0, 50.0);
    let (sc, sr, ec, er) = r.to_tile_coords(64);
    assert_eq!(sc, 0);
    assert_eq!(sr, 1);
    assert_eq!(ec, 3);
    assert_eq!(er, 2);
}

#[test]
fn affine_identity() {
    let id = Affine2D::identity();
    let p = Point::new(5.0, 10.0);
    let tp = id.transform_point(p);
    assert!((tp.x - 5.0).abs() < f32::EPSILON);
    assert!((tp.y - 10.0).abs() < f32::EPSILON);
}

#[test]
fn affine_translation() {
    let t = Affine2D::translation(10.0, -5.0);
    let p = t.transform_point(Point::new(1.0, 2.0));
    assert!((p.x - 11.0).abs() < f32::EPSILON);
    assert!((p.y - -3.0).abs() < f32::EPSILON);
}

#[test]
fn affine_scale() {
    let s = Affine2D::scale(2.0, 3.0);
    let p = s.transform_point(Point::new(4.0, 5.0));
    assert!((p.x - 8.0).abs() < f32::EPSILON);
    assert!((p.y - 15.0).abs() < f32::EPSILON);
}

#[test]
fn affine_compose() {
    let t = Affine2D::translation(10.0, 0.0);
    let s = Affine2D::scale(2.0, 2.0);
    // Apply translation first, then scale: (1+10)*2 = 22
    let composed = t.then(&s);
    let p = composed.transform_point(Point::new(1.0, 0.0));
    assert!((p.x - 22.0).abs() < f32::EPSILON);
}

#[test]
fn rect_area() {
    let r = Rect::new(0.0, 0.0, 10.0, 5.0);
    assert_eq!(r.area(), 50.0);
}

#[test]
fn rect_intersects_true() {
    let a = Rect::new(0.0, 0.0, 100.0, 100.0);
    let b = Rect::new(50.0, 50.0, 100.0, 100.0);
    assert!(a.intersects(&b));
}

#[test]
fn rect_intersects_false() {
    let a = Rect::new(0.0, 0.0, 10.0, 10.0);
    let b = Rect::new(20.0, 20.0, 10.0, 10.0);
    assert!(!a.intersects(&b));
}

#[test]
fn rect_union() {
    let a = Rect::new(10.0, 10.0, 20.0, 20.0);
    let b = Rect::new(25.0, 25.0, 20.0, 20.0);
    let u = a.union(&b);
    assert_eq!(u.x, 10.0);
    assert_eq!(u.y, 10.0);
    assert_eq!(u.width, 35.0);
    assert_eq!(u.height, 35.0);
}

#[test]
fn rect_zero_size() {
    let r = Rect::ZERO;
    assert_eq!(r.area(), 0.0);
    assert!(!r.contains(Point::ZERO)); // zero-size rect contains nothing
}

#[test]
fn point_new() {
    let p = Point::new(3.5, -7.2);
    assert_eq!(p.x, 3.5);
    assert_eq!(p.y, -7.2);
}

#[test]
fn size_area() {
    let s = Size::new(1920.0, 1080.0);
    assert_eq!(s.area(), 1920.0 * 1080.0);
}

#[test]
fn affine_rotation_90() {
    let r = Affine2D::rotation(std::f32::consts::FRAC_PI_2);
    let p = r.transform_point(Point::new(1.0, 0.0));
    assert!((p.x).abs() < 1e-5);
    assert!((p.y - 1.0).abs() < 1e-5);
}

#[test]
fn affine_transform_rect() {
    let t = Affine2D::translation(10.0, 20.0);
    let r = Rect::new(0.0, 0.0, 50.0, 30.0);
    let tr = t.transform_rect(r);
    assert!((tr.x - 10.0).abs() < 1e-5);
    assert!((tr.y - 20.0).abs() < 1e-5);
    assert!((tr.width - 50.0).abs() < 1e-5);
    assert!((tr.height - 30.0).abs() < 1e-5);
}

#[test]
fn affine_is_identity() {
    assert!(Affine2D::identity().is_identity());
    assert!(!Affine2D::translation(1.0, 0.0).is_identity());
    assert!(!Affine2D::scale(2.0, 1.0).is_identity());
}

#[test]
fn rect_from_point_size() {
    let r = Rect::from_point_size(Point::new(5.0, 10.0), Size::new(20.0, 30.0));
    assert_eq!(r.x, 5.0);
    assert_eq!(r.y, 10.0);
    assert_eq!(r.width, 20.0);
    assert_eq!(r.height, 30.0);
}

#[test]
fn rect_origin_and_size() {
    let r = Rect::new(5.0, 10.0, 20.0, 30.0);
    let o = r.origin();
    assert_eq!(o.x, 5.0);
    assert_eq!(o.y, 10.0);
    let s = r.size();
    assert_eq!(s.width, 20.0);
    assert_eq!(s.height, 30.0);
}

#[test]
fn rect_right_bottom() {
    let r = Rect::new(10.0, 20.0, 100.0, 50.0);
    assert_eq!(r.right(), 110.0);
    assert_eq!(r.bottom(), 70.0);
}

#[test]
fn rect_center() {
    let r = Rect::new(10.0, 20.0, 100.0, 50.0);
    let c = r.center();
    assert_eq!(c.x, 60.0);
    assert_eq!(c.y, 45.0);
}

#[test]
fn rect_expand() {
    let r = Rect::new(10.0, 10.0, 20.0, 20.0);
    let e = r.expand(5.0);
    assert_eq!(e.x, 5.0);
    assert_eq!(e.y, 5.0);
    assert_eq!(e.width, 30.0);
    assert_eq!(e.height, 30.0);
}

#[test]
fn rect_shrink() {
    let r = Rect::new(10.0, 10.0, 20.0, 20.0);
    let s = r.shrink(5.0);
    assert_eq!(s.x, 15.0);
    assert_eq!(s.y, 15.0);
    assert_eq!(s.width, 10.0);
    assert_eq!(s.height, 10.0);
}

#[test]
fn rect_shrink_clamps_to_zero() {
    let r = Rect::new(0.0, 0.0, 4.0, 4.0);
    let s = r.shrink(10.0);
    assert_eq!(s.width, 0.0);
    assert_eq!(s.height, 0.0);
}
