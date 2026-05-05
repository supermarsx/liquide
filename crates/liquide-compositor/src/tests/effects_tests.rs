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

#[test]
fn effect_budget_retargets_to_high_refresh() {
    let budget = EffectBudget::for_profile_with_target_fps(QualityProfile::Quality, 1000);

    assert_eq!(budget.target_fps, 1000);
    assert!((budget.total_frame_budget_ms - 1.0002).abs() < 0.01);
    assert!(budget.total_effects_budget_ms > 0.0);
    assert!(budget.total_effects_budget_ms < 1.0);
}

#[test]
fn degradation_from_u8_invalid() {
    assert_eq!(DegradationLevel::from_u8(14), None);
    assert_eq!(DegradationLevel::from_u8(255), None);
}

#[test]
fn degradation_as_u8_roundtrip() {
    for i in 0..=13 {
        let level = DegradationLevel::from_u8(i).unwrap();
        assert_eq!(level.as_u8(), i);
    }
}

#[test]
fn controller_set_level_directly() {
    let mut ctrl = DegradationController::new();
    ctrl.set_level(DegradationLevel::L5);
    assert_eq!(ctrl.current_level(), DegradationLevel::L5);
    ctrl.set_level(DegradationLevel::L0);
    assert_eq!(ctrl.current_level(), DegradationLevel::L0);
}

#[test]
fn controller_with_custom_thresholds() {
    let mut ctrl = DegradationController::with_thresholds(1, 1);
    // With threshold=1, one over-budget frame should trigger descent
    let changed = ctrl.report_frame_time(20.0, 10.0);
    assert!(changed);
    assert_eq!(ctrl.current_level(), DegradationLevel::L1);
}

#[test]
fn effect_params_high_degradation() {
    let params =
        EffectParams::for_profile(QualityProfile::Quality).apply_degradation(DegradationLevel::L7);
    assert_eq!(params.blur_radius, 0);
    assert_eq!(params.max_backdrop_blurs, 0);
    assert_eq!(params.shadow_blur_radius, 0);
    assert!(!params.parallax_enabled);
    assert_eq!(params.animation_scale, 0.0);
}

#[test]
fn effect_budget_remaining() {
    let budget = EffectBudget::for_profile(QualityProfile::Balanced);
    assert!(budget.remaining_ms(0.0) > 0.0);
    assert_eq!(budget.remaining_ms(100.0), 0.0); // exceeded
    assert!(budget.remaining_ms(6.0) > 0.0);
}

#[test]
fn quality_profile_default() {
    assert_eq!(QualityProfile::default(), QualityProfile::Balanced);
}
