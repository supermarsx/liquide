use crate::effects::*;

#[test]
fn degradation_step_down() {
    assert_eq!(DegradationLevel::L0.step_down(), DegradationLevel::L1);
    assert_eq!(DegradationLevel::L12.step_down(), DegradationLevel::L13);
    assert_eq!(DegradationLevel::L13.step_down(), DegradationLevel::L13);
}

#[test]
fn degradation_step_up() {
    assert_eq!(DegradationLevel::L5.step_up(), DegradationLevel::L4);
    assert_eq!(DegradationLevel::L0.step_up(), DegradationLevel::L0);
}

#[test]
fn controller_descends_after_threshold() {
    let mut ctrl = DegradationController::new();
    assert_eq!(ctrl.current_level(), DegradationLevel::L0);

    // 3 consecutive over-budget frames → descend to L1
    for _ in 0..3 {
        ctrl.report_frame_time(20.0, 16.0);
    }
    assert_eq!(ctrl.current_level(), DegradationLevel::L1);
}

#[test]
fn controller_ascends_after_threshold() {
    let mut ctrl = DegradationController::new();
    ctrl.set_level(DegradationLevel::L5);

    // 10 consecutive far-under-budget frames → ascend to L4
    for _ in 0..10 {
        ctrl.report_frame_time(5.0, 16.0);
    }
    assert_eq!(ctrl.current_level(), DegradationLevel::L4);
}

#[test]
fn controller_resets_on_mix() {
    let mut ctrl = DegradationController::new();

    // 2 over-budget, then 1 under-budget → no change
    ctrl.report_frame_time(20.0, 16.0);
    ctrl.report_frame_time(20.0, 16.0);
    ctrl.report_frame_time(5.0, 16.0);
    assert_eq!(ctrl.current_level(), DegradationLevel::L0);
}

#[test]
fn effect_params_degradation_disables_blur() {
    let params = EffectParams::for_profile(QualityProfile::Quality);
    assert!(params.blur_radius > 0);

    let degraded = params.apply_degradation(DegradationLevel::L7);
    assert_eq!(degraded.blur_radius, 0);
    assert_eq!(degraded.max_backdrop_blurs, 0);
}

#[test]
fn effect_budget_quality() {
    let budget = EffectBudget::for_profile(QualityProfile::Quality);
    assert_eq!(budget.target_fps, 60);
    assert!(budget.total_effects_budget_ms > 0.0);
}
