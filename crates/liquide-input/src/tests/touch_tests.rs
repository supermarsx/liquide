use crate::touch::*;

#[test]
fn touch_begin() {
    let pt = TouchPoint::new(1, 100.0, 200.0, 1.0);
    let evt = TouchEvent::new(TouchPhase::Begin, pt, 5000);
    assert_eq!(evt.phase, TouchPhase::Begin);
    assert_eq!(evt.point.id, 1);
    assert_eq!(evt.timestamp_us, 5000);
}

#[test]
fn touch_move() {
    let pt = TouchPoint::new(1, 110.0, 210.0, 0.8);
    let evt = TouchEvent::new(TouchPhase::Move, pt, 5100);
    assert_eq!(evt.phase, TouchPhase::Move);
    assert_eq!(evt.point.x, 110.0);
    assert_eq!(evt.point.y, 210.0);
}

#[test]
fn touch_end() {
    let pt = TouchPoint::new(1, 120.0, 220.0, 0.0);
    let evt = TouchEvent::new(TouchPhase::End, pt, 5200);
    assert_eq!(evt.phase, TouchPhase::End);
}

#[test]
fn touch_cancel() {
    let pt = TouchPoint::new(2, 0.0, 0.0, 0.0);
    let evt = TouchEvent::new(TouchPhase::Cancel, pt, 0);
    assert_eq!(evt.phase, TouchPhase::Cancel);
    assert_eq!(evt.point.id, 2);
}

#[test]
fn touch_point_pressure() {
    let pt = TouchPoint::new(3, 50.0, 50.0, 0.75);
    assert_eq!(pt.pressure, 0.75);
}

#[test]
fn touch_phase_variants() {
    let phases = [
        TouchPhase::Begin,
        TouchPhase::Move,
        TouchPhase::End,
        TouchPhase::Cancel,
    ];
    for i in 0..phases.len() {
        for j in (i + 1)..phases.len() {
            assert_ne!(phases[i], phases[j]);
        }
    }
}
