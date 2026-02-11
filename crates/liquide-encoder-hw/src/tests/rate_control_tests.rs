use crate::rate_control::QualityController;

#[test]
fn initial_state() {
    let ctrl = QualityController::new(60);
    assert_eq!(ctrl.current_quality(), 23);
    assert_eq!(ctrl.current_fps(), 60);
    assert_eq!(ctrl.target_fps(), 60);
}

#[test]
fn high_loss_degrades_quality_and_fps() {
    let mut ctrl = QualityController::new(60);
    let adj = ctrl.adjust(0.05, 0.0, 0.0, 0);
    assert!(adj.quality_delta > 0);
    assert!(adj.fps_delta < 0);
}

#[test]
fn moderate_loss_degrades_quality_only() {
    let mut ctrl = QualityController::new(60);
    let adj = ctrl.adjust(0.02, 0.0, 0.0, 0);
    assert_eq!(adj.quality_delta, 2);
    assert_eq!(adj.fps_delta, 0);
}

#[test]
fn high_cpu_reduces_fps() {
    let mut ctrl = QualityController::new(60);
    let adj = ctrl.adjust(0.0, 0.0, 0.95, 0);
    assert!(adj.fps_delta < 0);
}

#[test]
fn good_conditions_improve_quality() {
    let mut ctrl = QualityController::new(60);
    let adj = ctrl.adjust(0.0, 0.0, 0.5, 0);
    assert_eq!(adj.quality_delta, -1);
    assert_eq!(adj.fps_delta, 0);
}

#[test]
fn quality_clamped_to_valid_range() {
    let mut ctrl = QualityController::new(60);
    // Drive quality down to 0
    for _ in 0..30 {
        ctrl.adjust(0.0, 0.0, 0.0, 0);
    }
    assert_eq!(ctrl.current_quality(), 0);
}
