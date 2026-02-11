use crate::cursor::{CursorPosition, CursorPredictor, SmoothingStrategy};

#[test]
fn test_initial_positions_are_zero() {
    let predictor = CursorPredictor::new(3, SmoothingStrategy::Spring);
    let pos = predictor.predicted_position();
    assert!((pos.x - 0.0).abs() < f64::EPSILON);
    assert!((pos.y - 0.0).abs() < f64::EPSILON);
}

#[test]
fn test_update_local_changes_prediction() {
    let mut predictor = CursorPredictor::new(3, SmoothingStrategy::None);
    predictor.update_local(10.0, 20.0);
    let pos = predictor.predicted_position();
    // Within max_correction_distance (50.0) of server (0,0), so no
    // correction is applied and the local position is returned directly.
    assert!((pos.x - 10.0).abs() < f64::EPSILON);
    assert!((pos.y - 20.0).abs() < f64::EPSILON);
}

#[test]
fn test_needs_correction_when_diverged() {
    let mut predictor = CursorPredictor::new(5, SmoothingStrategy::Linear);
    predictor.update_local(0.0, 0.0);
    predictor.update_server(200.0, 200.0);
    assert!(predictor.needs_correction());
}

#[test]
fn test_no_correction_when_close() {
    let mut predictor = CursorPredictor::new(5, SmoothingStrategy::Linear);
    predictor.update_local(100.0, 100.0);
    predictor.update_server(110.0, 110.0);
    assert!(!predictor.needs_correction());
}

#[test]
fn test_correction_converges() {
    let mut predictor = CursorPredictor::new(10, SmoothingStrategy::Linear);
    predictor.update_local(0.0, 0.0);
    predictor.update_server(200.0, 200.0);

    // Apply several frames of correction.
    for _ in 0..10 {
        predictor.apply_correction();
    }

    let pos = predictor.predicted_position();
    // Should have moved significantly towards the server position.
    assert!(pos.x > 100.0, "x should converge: got {}", pos.x);
    assert!(pos.y > 100.0, "y should converge: got {}", pos.y);
}

#[test]
fn test_reset_clears_state() {
    let mut predictor = CursorPredictor::new(3, SmoothingStrategy::Spring);
    predictor.update_local(500.0, 300.0);
    predictor.update_server(600.0, 400.0);
    predictor.reset();

    let pos = predictor.predicted_position();
    assert!((pos.x - 0.0).abs() < f64::EPSILON);
    assert!((pos.y - 0.0).abs() < f64::EPSILON);
}

#[test]
fn test_cursor_position_distance() {
    let a = CursorPosition { x: 0.0, y: 0.0 };
    let b = CursorPosition { x: 3.0, y: 4.0 };
    let dist = a.distance_to(&b);
    assert!((dist - 5.0).abs() < f64::EPSILON);
}

#[test]
fn test_server_position_getter() {
    let mut predictor = CursorPredictor::new(3, SmoothingStrategy::None);
    predictor.update_server(42.0, 84.0);
    let sp = predictor.server_position();
    assert!((sp.x - 42.0).abs() < f64::EPSILON);
    assert!((sp.y - 84.0).abs() < f64::EPSILON);
}
