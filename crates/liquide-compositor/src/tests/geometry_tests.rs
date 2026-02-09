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
