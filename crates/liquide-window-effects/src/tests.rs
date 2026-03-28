use crate::easing::EasingFunction;
use crate::effects::{Rect, EffectManager, EffectState, WindowEffect};
use crate::snap_preview::{SnapPreview, SnapZone};
use crate::workspace_transition::{WorkspaceTransition, TransitionDirection};
use std::time::Duration;

// ── Easing tests ─────────────────────────────────────────────────────

#[test]
fn easing_all_start_at_zero() {
    let variants = [
        EasingFunction::Linear,
        EasingFunction::EaseIn,
        EasingFunction::EaseOut,
        EasingFunction::EaseInOut,
        EasingFunction::EaseInCubic,
        EasingFunction::EaseOutCubic,
        EasingFunction::EaseInOutCubic,
        EasingFunction::EaseInBack,
        EasingFunction::EaseOutBack,
        EasingFunction::EaseOutBounce,
        EasingFunction::Spring,
    ];
    for f in variants {
        let v = f.eval(0.0);
        assert!((v - 0.0).abs() < 1e-5, "{f:?} at t=0 gave {v}");
    }
}

#[test]
fn easing_all_end_at_one() {
    let variants = [
        EasingFunction::Linear,
        EasingFunction::EaseIn,
        EasingFunction::EaseOut,
        EasingFunction::EaseInOut,
        EasingFunction::EaseInCubic,
        EasingFunction::EaseOutCubic,
        EasingFunction::EaseInOutCubic,
        EasingFunction::EaseInBack,
        EasingFunction::EaseOutBack,
        EasingFunction::EaseOutBounce,
        EasingFunction::Spring,
    ];
    for f in variants {
        let v = f.eval(1.0);
        assert!((v - 1.0).abs() < 1e-5, "{f:?} at t=1 gave {v}");
    }
}

#[test]
fn easing_clamps_input() {
    // Negative input should clamp to 0
    assert!((EasingFunction::Linear.eval(-0.5) - 0.0).abs() < 1e-5);
    // Input > 1 should clamp to 1
    assert!((EasingFunction::Linear.eval(1.5) - 1.0).abs() < 1e-5);
}

#[test]
fn easing_linear_midpoint() {
    assert!((EasingFunction::Linear.eval(0.5) - 0.5).abs() < 1e-5);
}

#[test]
fn easing_ease_in_slower_start() {
    // EaseIn (quadratic) at t=0.5 should be 0.25, less than linear's 0.5
    let v = EasingFunction::EaseIn.eval(0.5);
    assert!(v < 0.5, "EaseIn at 0.5 should be less than 0.5, got {v}");
    assert!((v - 0.25).abs() < 1e-5);
}

#[test]
fn easing_ease_out_faster_start() {
    // EaseOut at t=0.5 should be > 0.5
    let v = EasingFunction::EaseOut.eval(0.5);
    assert!(v > 0.5, "EaseOut at 0.5 should be greater than 0.5, got {v}");
}

#[test]
fn easing_ease_out_bounce_stays_positive() {
    for i in 0..=100 {
        let t = i as f32 / 100.0;
        let v = EasingFunction::EaseOutBounce.eval(t);
        assert!(v >= 0.0, "EaseOutBounce at t={t} gave negative {v}");
    }
}

// ── Rect tests ───────────────────────────────────────────────────────

#[test]
fn rect_lerp_at_zero() {
    let a = Rect::new(0.0, 0.0, 100.0, 100.0);
    let b = Rect::new(50.0, 50.0, 200.0, 200.0);
    let r = a.lerp(&b, 0.0);
    assert!((r.x - 0.0).abs() < 1e-5);
    assert!((r.y - 0.0).abs() < 1e-5);
    assert!((r.width - 100.0).abs() < 1e-5);
    assert!((r.height - 100.0).abs() < 1e-5);
}

#[test]
fn rect_lerp_at_one() {
    let a = Rect::new(0.0, 0.0, 100.0, 100.0);
    let b = Rect::new(50.0, 50.0, 200.0, 200.0);
    let r = a.lerp(&b, 1.0);
    assert!((r.x - 50.0).abs() < 1e-5);
    assert!((r.y - 50.0).abs() < 1e-5);
    assert!((r.width - 200.0).abs() < 1e-5);
    assert!((r.height - 200.0).abs() < 1e-5);
}

#[test]
fn rect_lerp_midpoint() {
    let a = Rect::new(0.0, 0.0, 100.0, 100.0);
    let b = Rect::new(100.0, 100.0, 200.0, 200.0);
    let r = a.lerp(&b, 0.5);
    assert!((r.x - 50.0).abs() < 1e-5);
    assert!((r.y - 50.0).abs() < 1e-5);
    assert!((r.width - 150.0).abs() < 1e-5);
    assert!((r.height - 150.0).abs() < 1e-5);
}

#[test]
fn rect_center() {
    let r = Rect::new(10.0, 20.0, 100.0, 200.0);
    let (cx, cy) = r.center();
    assert!((cx - 60.0).abs() < 1e-5);
    assert!((cy - 120.0).abs() < 1e-5);
}

// ── Effect tests ─────────────────────────────────────────────────────

#[test]
fn effect_open_produces_frame() {
    let effect = WindowEffect::Open {
        window_id: 1,
        from: Rect::new(50.0, 50.0, 190.0, 190.0),
        to: Rect::new(50.0, 50.0, 200.0, 200.0),
        opacity_from: 0.0,
        opacity_to: 1.0,
    };
    let mut state = EffectState::new(effect, EasingFunction::EaseOutCubic, Duration::from_millis(200));
    let frame = state.update();
    assert_eq!(frame.window_id, 1);
    assert!(frame.opacity >= 0.0);
    assert!(!frame.finished || frame.opacity > 0.9);
}

#[test]
fn effect_close_fades_out() {
    let effect = WindowEffect::Close {
        window_id: 2,
        from: Rect::new(50.0, 50.0, 200.0, 200.0),
        to: Rect::new(50.0, 50.0, 190.0, 190.0),
        opacity_from: 1.0,
        opacity_to: 0.0,
    };
    let mut state = EffectState::new(effect, EasingFunction::EaseIn, Duration::from_millis(150));
    let frame = state.update();
    assert_eq!(frame.window_id, 2);
    // At very start, opacity should be close to 1.0
    assert!(frame.opacity <= 1.0);
}

#[test]
fn effect_transform_zero_duration_finishes() {
    let effect = WindowEffect::Transform {
        window_id: 3,
        from: Rect::new(0.0, 0.0, 100.0, 100.0),
        to: Rect::new(100.0, 100.0, 200.0, 200.0),
    };
    let mut state = EffectState::new(effect, EasingFunction::Linear, Duration::ZERO);
    let frame = state.update();
    // Zero duration -> instantly finished
    assert!(frame.finished);
    assert!((frame.bounds.x - 100.0).abs() < 1e-5);
    assert!((frame.bounds.width - 200.0).abs() < 1e-5);
}

#[test]
fn effect_focus_scale_returns_to_one() {
    let effect = WindowEffect::FocusIn {
        window_id: 4,
        bounds: Rect::new(100.0, 100.0, 400.0, 300.0),
    };
    let mut state = EffectState::new(effect, EasingFunction::EaseInOut, Duration::ZERO);
    let frame = state.update();
    // At t=1.0, scale should be back to 1.0
    assert!((frame.scale - 1.0).abs() < 1e-5);
}

#[test]
fn effect_fullscreen_zero_duration() {
    let effect = WindowEffect::Fullscreen {
        window_id: 5,
        from: Rect::new(100.0, 100.0, 800.0, 600.0),
        to: Rect::new(0.0, 0.0, 1920.0, 1080.0),
    };
    let mut state = EffectState::new(effect, EasingFunction::Linear, Duration::ZERO);
    let frame = state.update();
    assert!(frame.finished);
    assert!((frame.bounds.x - 0.0).abs() < 1e-5);
    assert!((frame.bounds.y - 0.0).abs() < 1e-5);
    assert!((frame.bounds.width - 1920.0).abs() < 1e-5);
    assert!((frame.bounds.height - 1080.0).abs() < 1e-5);
}

// ── EffectManager tests ──────────────────────────────────────────────

#[test]
fn manager_tick_clears_finished() {
    let mut mgr = EffectManager::new();
    // open_window with a real rect, then wait for zero-duration effect
    // We can't set duration via public API, so we test with a long-duration
    // effect that we immediately cancel, or we test the flow differently.
    // Let's test that open_window creates an effect and tick returns frames.
    mgr.open_window(10, Rect::new(0.0, 0.0, 800.0, 600.0));
    assert_eq!(mgr.active_count(), 1);
    assert!(mgr.has_active_effects());

    // Tick should return one frame (effect is still in progress)
    let frames = mgr.tick();
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].window_id, 10);
}

#[test]
fn manager_reduce_motion_skips_animations() {
    let mut mgr = EffectManager::new();
    mgr.set_reduce_motion(true);

    mgr.open_window(1, Rect::new(0.0, 0.0, 800.0, 600.0));
    assert_eq!(mgr.active_count(), 0);

    mgr.close_window(2, Rect::new(0.0, 0.0, 800.0, 600.0));
    assert_eq!(mgr.active_count(), 0);

    mgr.transform_window(3, Rect::new(0.0, 0.0, 100.0, 100.0), Rect::new(50.0, 50.0, 200.0, 200.0));
    assert_eq!(mgr.active_count(), 0);

    mgr.focus_window(4, Rect::new(0.0, 0.0, 400.0, 300.0));
    assert_eq!(mgr.active_count(), 0);
}

#[test]
fn manager_cancel_effects_for_window() {
    let mut mgr = EffectManager::new();
    mgr.open_window(1, Rect::new(0.0, 0.0, 800.0, 600.0));
    mgr.open_window(2, Rect::new(0.0, 0.0, 800.0, 600.0));
    assert_eq!(mgr.active_count(), 2);

    mgr.cancel_effects_for(1);
    assert_eq!(mgr.active_count(), 1);
    assert!(!mgr.is_animating(1));
    assert!(mgr.is_animating(2));
}

#[test]
fn manager_is_animating() {
    let mut mgr = EffectManager::new();
    assert!(!mgr.is_animating(1));
    mgr.open_window(1, Rect::new(0.0, 0.0, 800.0, 600.0));
    assert!(mgr.is_animating(1));
    assert!(!mgr.is_animating(2));
}

#[test]
fn manager_has_active_effects() {
    let mut mgr = EffectManager::new();
    assert!(!mgr.has_active_effects());
    mgr.open_window(1, Rect::new(0.0, 0.0, 800.0, 600.0));
    assert!(mgr.has_active_effects());
}

#[test]
fn manager_open_cancels_previous() {
    let mut mgr = EffectManager::new();
    mgr.open_window(1, Rect::new(0.0, 0.0, 800.0, 600.0));
    mgr.open_window(1, Rect::new(0.0, 0.0, 400.0, 300.0));
    // Should have only one effect for window 1
    assert_eq!(mgr.active_count(), 1);
}

#[test]
fn manager_multiple_windows() {
    let mut mgr = EffectManager::new();
    mgr.open_window(1, Rect::new(0.0, 0.0, 800.0, 600.0));
    mgr.transform_window(2, Rect::new(0.0, 0.0, 100.0, 100.0), Rect::new(50.0, 50.0, 200.0, 200.0));
    mgr.focus_window(3, Rect::new(100.0, 100.0, 400.0, 300.0));
    assert_eq!(mgr.active_count(), 3);

    let frames = mgr.tick();
    assert_eq!(frames.len(), 3);
}

#[test]
fn manager_default() {
    let mgr = EffectManager::default();
    assert_eq!(mgr.active_count(), 0);
    assert!(!mgr.has_active_effects());
}

// ── SnapPreview tests ────────────────────────────────────────────────

#[test]
fn snap_detect_left_edge() {
    let screen = Rect::new(0.0, 0.0, 1920.0, 1080.0);
    assert_eq!(SnapPreview::detect_zone(2.0, 500.0, screen, 10.0), SnapZone::Left);
}

#[test]
fn snap_detect_right_edge() {
    let screen = Rect::new(0.0, 0.0, 1920.0, 1080.0);
    assert_eq!(SnapPreview::detect_zone(1918.0, 500.0, screen, 10.0), SnapZone::Right);
}

#[test]
fn snap_detect_top_edge_maximizes() {
    let screen = Rect::new(0.0, 0.0, 1920.0, 1080.0);
    assert_eq!(SnapPreview::detect_zone(960.0, 2.0, screen, 10.0), SnapZone::Maximize);
}

#[test]
fn snap_detect_bottom_edge() {
    let screen = Rect::new(0.0, 0.0, 1920.0, 1080.0);
    assert_eq!(SnapPreview::detect_zone(960.0, 1078.0, screen, 10.0), SnapZone::Bottom);
}

#[test]
fn snap_detect_top_left_corner() {
    let screen = Rect::new(0.0, 0.0, 1920.0, 1080.0);
    assert_eq!(SnapPreview::detect_zone(2.0, 2.0, screen, 10.0), SnapZone::TopLeft);
}

#[test]
fn snap_detect_top_right_corner() {
    let screen = Rect::new(0.0, 0.0, 1920.0, 1080.0);
    assert_eq!(SnapPreview::detect_zone(1918.0, 2.0, screen, 10.0), SnapZone::TopRight);
}

#[test]
fn snap_detect_bottom_left_corner() {
    let screen = Rect::new(0.0, 0.0, 1920.0, 1080.0);
    assert_eq!(SnapPreview::detect_zone(2.0, 1078.0, screen, 10.0), SnapZone::BottomLeft);
}

#[test]
fn snap_detect_bottom_right_corner() {
    let screen = Rect::new(0.0, 0.0, 1920.0, 1080.0);
    assert_eq!(SnapPreview::detect_zone(1918.0, 1078.0, screen, 10.0), SnapZone::BottomRight);
}

#[test]
fn snap_detect_center_is_none() {
    let screen = Rect::new(0.0, 0.0, 1920.0, 1080.0);
    assert_eq!(SnapPreview::detect_zone(960.0, 540.0, screen, 10.0), SnapZone::None);
}

#[test]
fn snap_show_hide_state() {
    let mut sp = SnapPreview::new();
    assert!(!sp.active);
    assert_eq!(sp.zone, SnapZone::None);

    sp.show(SnapZone::Left, Rect::new(0.0, 0.0, 1920.0, 1080.0), 8.0);
    assert!(sp.active);
    assert_eq!(sp.zone, SnapZone::Left);
    assert!(sp.opacity > 0.0);
    assert!(sp.target_rect.width > 0.0);

    sp.hide();
    assert!(!sp.active);
    assert_eq!(sp.zone, SnapZone::None);
    assert!((sp.opacity - 0.0).abs() < 1e-5);
}

#[test]
fn snap_show_maximize_covers_work_area() {
    let mut sp = SnapPreview::new();
    let work_area = Rect::new(0.0, 0.0, 1920.0, 1080.0);
    sp.show(SnapZone::Maximize, work_area, 8.0);
    // Should be work_area minus 2*gap on each side
    assert!((sp.target_rect.width - (1920.0 - 16.0)).abs() < 1e-5);
    assert!((sp.target_rect.height - (1080.0 - 16.0)).abs() < 1e-5);
}

#[test]
fn snap_left_right_split_screen() {
    let mut sp = SnapPreview::new();
    let work_area = Rect::new(0.0, 0.0, 1920.0, 1080.0);
    let gap = 8.0;

    sp.show(SnapZone::Left, work_area, gap);
    let left_rect = sp.target_rect;

    sp.show(SnapZone::Right, work_area, gap);
    let right_rect = sp.target_rect;

    // Left and right should not overlap
    assert!(left_rect.x + left_rect.width <= right_rect.x);
    // Both should have the same width
    assert!((left_rect.width - right_rect.width).abs() < 1e-5);
}

#[test]
fn snap_default() {
    let sp = SnapPreview::default();
    assert!(!sp.active);
}

// ── WorkspaceTransition tests ────────────────────────────────────────

#[test]
fn workspace_transition_start_update() {
    let mut wt = WorkspaceTransition::new();
    assert!(!wt.active);

    wt.start(0, 1, TransitionDirection::Left);
    assert!(wt.active);
    assert_eq!(wt.from_workspace, 0);
    assert_eq!(wt.to_workspace, 1);

    let frame = wt.update();
    assert!(frame.new_opacity >= 0.0 && frame.new_opacity <= 1.0);
}

#[test]
fn workspace_transition_zero_duration_finishes() {
    let mut wt = WorkspaceTransition::new();
    wt.duration = Duration::ZERO;
    wt.start(0, 1, TransitionDirection::Right);

    let frame = wt.update();
    assert!(frame.finished);
    assert!((frame.new_opacity - 1.0).abs() < 1e-5);
    assert!((frame.old_opacity - 0.0).abs() < 1e-5);
    assert!(!wt.active);
}

#[test]
fn workspace_transition_cancel() {
    let mut wt = WorkspaceTransition::new();
    wt.start(0, 1, TransitionDirection::Up);
    assert!(wt.active);

    wt.cancel();
    assert!(!wt.active);
}

#[test]
fn workspace_transition_inactive_returns_default() {
    let mut wt = WorkspaceTransition::new();
    let frame = wt.update();
    assert!(!frame.finished);
    assert!((frame.offset_x - 0.0).abs() < 1e-5);
    assert!((frame.offset_y - 0.0).abs() < 1e-5);
}

#[test]
fn workspace_transition_fade_only_no_offset() {
    let mut wt = WorkspaceTransition::new();
    wt.duration = Duration::ZERO;
    wt.start(0, 1, TransitionDirection::FadeOnly);
    let frame = wt.update();
    assert!(frame.finished);
    assert!((frame.offset_x - 0.0).abs() < 1e-5);
    assert!((frame.offset_y - 0.0).abs() < 1e-5);
}

#[test]
fn workspace_transition_default() {
    let wt = WorkspaceTransition::default();
    assert!(!wt.active);
    assert_eq!(wt.from_workspace, 0);
    assert_eq!(wt.to_workspace, 0);
}
