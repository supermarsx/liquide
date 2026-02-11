//! Tests for animation primitives.

use crate::animation::{Animation, AnimationManager, AnimationState, Easing};

// ---------------------------------------------------------------------------
// Easing
// ---------------------------------------------------------------------------

#[test]
fn test_easing_linear() {
    let e = Easing::Linear;
    assert_eq!(e.apply(0.0), 0.0);
    assert_eq!(e.apply(0.5), 0.5);
    assert_eq!(e.apply(1.0), 1.0);
}

#[test]
fn test_easing_ease_in() {
    let e = Easing::EaseIn;
    assert_eq!(e.apply(0.0), 0.0);
    assert_eq!(e.apply(1.0), 1.0);
    // EaseIn should be below linear in the middle.
    assert!(e.apply(0.5) < 0.5);
}

#[test]
fn test_easing_ease_out() {
    let e = Easing::EaseOut;
    assert_eq!(e.apply(0.0), 0.0);
    assert_eq!(e.apply(1.0), 1.0);
    // EaseOut should be above linear in the middle.
    assert!(e.apply(0.5) > 0.5);
}

#[test]
fn test_easing_ease_in_out() {
    let e = Easing::EaseInOut;
    assert_eq!(e.apply(0.0), 0.0);
    assert_eq!(e.apply(1.0), 1.0);
    assert!((e.apply(0.5) - 0.5).abs() < 0.001);
}

#[test]
fn test_easing_cubic_bezier_endpoints() {
    let e = Easing::CubicBezier {
        x1: 0.25,
        y1: 0.1,
        x2: 0.25,
        y2: 1.0,
    };
    assert_eq!(e.apply(0.0), 0.0);
    assert!((e.apply(1.0) - 1.0).abs() < 0.001);
}

#[test]
fn test_easing_clamps_out_of_range() {
    let e = Easing::Linear;
    assert_eq!(e.apply(-0.5), 0.0);
    assert_eq!(e.apply(1.5), 1.0);
}

// ---------------------------------------------------------------------------
// Animation
// ---------------------------------------------------------------------------

#[test]
fn test_animation_new_starts_running() {
    let anim = Animation::new(0.0, 100.0, 1000, Easing::Linear);
    assert_eq!(anim.state, AnimationState::Running);
    assert_eq!(anim.elapsed_ms, 0);
    assert_eq!(anim.progress(), 0.0);
    assert_eq!(anim.current_value(), 0.0);
    assert!(!anim.is_complete());
}

#[test]
fn test_animation_tick_advances() {
    let mut anim = Animation::new(0.0, 100.0, 1000, Easing::Linear);
    anim.tick(500);
    assert_eq!(anim.elapsed_ms, 500);
    assert!((anim.progress() - 0.5).abs() < 0.001);
    assert!((anim.current_value() - 50.0).abs() < 0.1);
}

#[test]
fn test_animation_completes() {
    let mut anim = Animation::new(0.0, 100.0, 1000, Easing::Linear);
    anim.tick(1500);
    assert!(anim.is_complete());
    assert_eq!(anim.state, AnimationState::Completed);
    assert_eq!(anim.elapsed_ms, 1000); // capped at duration
    assert_eq!(anim.progress(), 1.0);
    assert!((anim.current_value() - 100.0).abs() < 0.1);
}

#[test]
fn test_animation_exact_completion() {
    let mut anim = Animation::new(0.0, 100.0, 500, Easing::Linear);
    anim.tick(500);
    assert!(anim.is_complete());
    assert_eq!(anim.progress(), 1.0);
}

#[test]
fn test_animation_zero_duration_completes_immediately() {
    let mut anim = Animation::new(0.0, 100.0, 0, Easing::Linear);
    // Zero duration means progress() returns 1.0 immediately.
    assert_eq!(anim.progress(), 1.0);
    anim.tick(0);
    assert!(anim.is_complete());
}

#[test]
fn test_animation_pause_and_resume() {
    let mut anim = Animation::new(0.0, 100.0, 1000, Easing::Linear);
    anim.tick(200);
    anim.pause();
    assert_eq!(anim.state, AnimationState::Paused);

    // Ticking while paused should not advance.
    anim.tick(500);
    assert_eq!(anim.elapsed_ms, 200);

    anim.resume();
    assert_eq!(anim.state, AnimationState::Running);
    anim.tick(300);
    assert_eq!(anim.elapsed_ms, 500);
}

#[test]
fn test_animation_reset() {
    let mut anim = Animation::new(0.0, 100.0, 1000, Easing::Linear);
    anim.tick(500);
    anim.reset();
    assert_eq!(anim.state, AnimationState::Idle);
    assert_eq!(anim.elapsed_ms, 0);
    assert_eq!(anim.progress(), 0.0);
}

#[test]
fn test_animation_value_interpolation() {
    let mut anim = Animation::new(50.0, 150.0, 100, Easing::Linear);
    anim.tick(50); // 50% progress
    assert!((anim.current_value() - 100.0).abs() < 0.1);
}

#[test]
fn test_animation_reverse_interpolation() {
    let mut anim = Animation::new(100.0, 0.0, 100, Easing::Linear);
    anim.tick(50); // 50% progress
    assert!((anim.current_value() - 50.0).abs() < 0.1);
}

// ---------------------------------------------------------------------------
// AnimationManager
// ---------------------------------------------------------------------------

#[test]
fn test_manager_start() {
    let mut mgr = AnimationManager::new();
    let id = mgr.start(0.0, 100.0, 1000, Easing::Linear);
    assert!(id > 0);
    assert_eq!(mgr.total_count(), 1);
    assert_eq!(mgr.active_count(), 1);
}

#[test]
fn test_manager_tick_all() {
    let mut mgr = AnimationManager::new();
    mgr.start(0.0, 100.0, 500, Easing::Linear);
    mgr.start(0.0, 100.0, 1000, Easing::Linear);

    let completed = mgr.tick_all(500);
    // First animation should complete at 500ms.
    assert_eq!(completed.len(), 1);
    assert_eq!(mgr.active_count(), 1);
}

#[test]
fn test_manager_tick_all_both_complete() {
    let mut mgr = AnimationManager::new();
    mgr.start(0.0, 100.0, 500, Easing::Linear);
    mgr.start(0.0, 100.0, 800, Easing::Linear);

    let completed = mgr.tick_all(1000);
    assert_eq!(completed.len(), 2);
    assert_eq!(mgr.active_count(), 0);
}

#[test]
fn test_manager_cancel() {
    let mut mgr = AnimationManager::new();
    let id = mgr.start(0.0, 100.0, 1000, Easing::Linear);
    mgr.cancel(id);
    assert_eq!(mgr.total_count(), 0);
    assert!(mgr.get(id).is_none());
}

#[test]
fn test_manager_get() {
    let mut mgr = AnimationManager::new();
    let id = mgr.start(0.0, 100.0, 1000, Easing::Linear);
    let anim = mgr.get(id).unwrap();
    assert_eq!(anim.from_value, 0.0);
    assert_eq!(anim.to_value, 100.0);
    assert_eq!(anim.duration_ms, 1000);
}

#[test]
fn test_manager_get_nonexistent() {
    let mgr = AnimationManager::new();
    assert!(mgr.get(999).is_none());
}

#[test]
fn test_manager_unique_ids() {
    let mut mgr = AnimationManager::new();
    let id1 = mgr.start(0.0, 1.0, 100, Easing::Linear);
    let id2 = mgr.start(0.0, 1.0, 100, Easing::Linear);
    assert_ne!(id1, id2);
}

#[test]
fn test_manager_active_count_excludes_completed() {
    let mut mgr = AnimationManager::new();
    mgr.start(0.0, 100.0, 100, Easing::Linear);
    mgr.start(0.0, 100.0, 200, Easing::Linear);
    mgr.tick_all(150);
    // First completed, second still running.
    assert_eq!(mgr.active_count(), 1);
    assert_eq!(mgr.total_count(), 2);
}
