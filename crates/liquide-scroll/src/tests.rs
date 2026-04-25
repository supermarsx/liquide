#[cfg(test)]
mod tests {
    use crate::manager::ScrollManager;
    use crate::momentum::MomentumScroller;
    use crate::overscroll::{OverscrollEffect, SpringAnimation};
    use crate::scrollbar::{
        self, AutoHideController, Orientation, Rect, ScrollbarHit, ScrollbarStyle,
    };
    use crate::smooth::SmoothScroller;
    use crate::snap::{SnapAlignment, SnapConfig, SnapType, find_snap_target};
    use crate::state::ScrollState;
    use crate::wheel::WheelConfig;

    // ── ScrollState ──────────────────────────────────────────────────

    #[test]
    fn state_new_defaults() {
        let s = ScrollState::new((800.0, 2000.0), (400.0, 600.0));
        assert_eq!(s.offset, (0.0, 0.0));
        assert_eq!(s.max_scroll(), (400.0, 1400.0));
    }

    #[test]
    fn state_max_scroll_no_overflow() {
        let s = ScrollState::new((200.0, 300.0), (400.0, 600.0));
        assert_eq!(s.max_scroll(), (0.0, 0.0));
    }

    #[test]
    fn state_is_at_start() {
        let s = ScrollState::new((800.0, 2000.0), (400.0, 600.0));
        assert_eq!(s.is_at_start(), (true, true));
    }

    #[test]
    fn state_is_at_end() {
        let mut s = ScrollState::new((800.0, 2000.0), (400.0, 600.0));
        s.set_offset(400.0, 1400.0);
        assert_eq!(s.is_at_end(), (true, true));
    }

    #[test]
    fn state_scroll_percent() {
        let mut s = ScrollState::new((800.0, 2000.0), (400.0, 600.0));
        assert_eq!(s.scroll_percent(), (0.0, 0.0));
        s.set_offset(200.0, 700.0);
        assert!((s.scroll_percent().0 - 0.5).abs() < 0.01);
        assert!((s.scroll_percent().1 - 0.5).abs() < 0.01);
    }

    #[test]
    fn state_scroll_by_clamps() {
        let mut s = ScrollState::new((800.0, 2000.0), (400.0, 600.0));
        s.scroll_by(-100.0, -100.0);
        assert_eq!(s.offset, (0.0, 0.0));
        s.scroll_by(9999.0, 9999.0);
        assert_eq!(s.offset, (400.0, 1400.0));
    }

    #[test]
    fn state_set_content_size_reclamps() {
        let mut s = ScrollState::new((800.0, 2000.0), (400.0, 600.0));
        s.set_offset(400.0, 1400.0);
        s.set_content_size(500.0, 800.0);
        assert_eq!(s.offset, (100.0, 200.0));
    }

    #[test]
    fn state_can_scroll() {
        let s = ScrollState::new((800.0, 2000.0), (400.0, 600.0));
        assert!(s.can_scroll_x());
        assert!(s.can_scroll_y());
        let s2 = ScrollState::new((200.0, 300.0), (400.0, 600.0));
        assert!(!s2.can_scroll_x());
        assert!(!s2.can_scroll_y());
    }

    // ── SmoothScroller ───────────────────────────────────────────────

    #[test]
    fn smooth_not_animating_by_default() {
        let s = SmoothScroller::new();
        assert!(!s.is_animating());
    }

    #[test]
    fn smooth_scroll_to_completes() {
        let mut s = SmoothScroller::new();
        s.scroll_to((0.0, 0.0), (100.0, 200.0), 300);
        assert!(s.is_animating());

        let pos = s.tick(300);
        assert!(!s.is_animating());
        assert!((pos.0 - 100.0).abs() < 0.01);
        assert!((pos.1 - 200.0).abs() < 0.01);
    }

    #[test]
    fn smooth_scroll_eases_out() {
        let mut s = SmoothScroller::new();
        s.scroll_to((0.0, 0.0), (100.0, 0.0), 100);

        // At 50% time with ease-out cubic, should be past halfway.
        let pos = s.tick(50);
        assert!(
            pos.0 > 50.0,
            "ease-out should be past 50% at t=0.5, got {}",
            pos.0
        );
    }

    #[test]
    fn smooth_cancel_freezes() {
        let mut s = SmoothScroller::new();
        s.scroll_to((0.0, 0.0), (100.0, 200.0), 300);
        s.tick(100);
        let frozen = s.cancel();
        assert!(!s.is_animating());
        // Should be somewhere between start and target.
        assert!(frozen.0 > 0.0 && frozen.0 < 100.0);
    }

    #[test]
    fn smooth_scroll_by() {
        let mut s = SmoothScroller::new();
        s.scroll_by((50.0, 100.0), (25.0, 50.0), 200);
        assert!(s.is_animating());
        assert_eq!(s.target(), (75.0, 150.0));
    }

    #[test]
    fn smooth_zero_duration() {
        let mut s = SmoothScroller::new();
        s.scroll_to((0.0, 0.0), (100.0, 100.0), 0);
        assert!(!s.is_animating());
    }

    // ── MomentumScroller ─────────────────────────────────────────────

    #[test]
    fn momentum_inactive_by_default() {
        let m = MomentumScroller::new();
        assert!(!m.is_active());
    }

    #[test]
    fn momentum_no_velocity_no_animation() {
        let mut m = MomentumScroller::new();
        m.begin_touch((100.0, 100.0));
        // Single sample — no velocity.
        let started = m.end_touch();
        assert!(!started);
        assert!(!m.is_active());
    }

    #[test]
    fn momentum_velocity_starts_animation() {
        let mut m = MomentumScroller::new();
        m.begin_touch((0.0, 0.0));
        m.move_touch((0.0, 0.0), 0);
        m.move_touch((0.0, -50.0), 10);
        m.move_touch((0.0, -120.0), 20);
        let started = m.end_touch();
        assert!(started);
        assert!(m.is_active());
    }

    #[test]
    fn momentum_decelerates_to_stop() {
        let mut m = MomentumScroller::new();
        m.begin_touch((0.0, 0.0));
        m.move_touch((0.0, 0.0), 0);
        m.move_touch((0.0, -100.0), 10);
        m.move_touch((0.0, -200.0), 20);
        m.end_touch();

        let mut total_dy = 0.0f32;
        for _ in 0..10_000 {
            if !m.is_active() {
                break;
            }
            let delta = m.tick(16);
            total_dy += delta.1;
        }
        assert!(!m.is_active());
        // Should have scrolled in the negative direction.
        assert!(total_dy < 0.0);
    }

    #[test]
    fn momentum_cancel() {
        let mut m = MomentumScroller::new();
        m.begin_touch((0.0, 0.0));
        m.move_touch((0.0, 0.0), 0);
        m.move_touch((0.0, -100.0), 10);
        m.end_touch();
        assert!(m.is_active());
        m.cancel();
        assert!(!m.is_active());
    }

    // ── OverscrollEffect ─────────────────────────────────────────────

    #[test]
    fn overscroll_no_excess() {
        let o = OverscrollEffect::new();
        let result = o.apply(50.0, 100.0, 0.0);
        assert_eq!(result, 50.0);
    }

    #[test]
    fn overscroll_past_end() {
        let o = OverscrollEffect::new();
        let result = o.apply(100.0, 100.0, 50.0);
        // Should be past max, but dampened.
        assert!(result > 100.0);
        assert!(result < 200.0);
    }

    #[test]
    fn overscroll_before_start() {
        let o = OverscrollEffect::new();
        let result = o.apply(0.0, 100.0, -50.0);
        // Should be negative, but dampened.
        assert!(result < 0.0);
        assert!(result > -100.0);
    }

    #[test]
    fn overscroll_disabled() {
        let mut o = OverscrollEffect::new();
        o.enabled = false;
        let result = o.apply(0.0, 100.0, -50.0);
        assert_eq!(result, 0.0);
    }

    #[test]
    fn overscroll_asymptotic() {
        let o = OverscrollEffect::new();
        // Large excess should asymptotically approach max_overscroll.
        let r1 = o.apply(0.0, 100.0, -100.0);
        let r2 = o.apply(0.0, 100.0, -10000.0);
        assert!(r2.abs() > r1.abs());
        assert!(r2.abs() < o.max_overscroll + 1.0);
    }

    // ── SpringAnimation ──────────────────────────────────────────────

    #[test]
    fn spring_settles() {
        let mut spring = SpringAnimation::new(50.0, 0.0, 0.0, 300.0, 25.0);
        for _ in 0..1000 {
            spring.tick(1.0 / 60.0);
        }
        assert!(spring.is_settled());
        assert!((spring.value - 0.0).abs() < 1.0);
    }

    #[test]
    fn spring_release() {
        let o = OverscrollEffect::new();
        let mut spring = o.release(30.0);
        assert!(!spring.is_settled());
        for _ in 0..1000 {
            spring.tick(1.0 / 60.0);
        }
        assert!(spring.is_settled());
    }

    // ── ScrollSnap ───────────────────────────────────────────────────

    #[test]
    fn snap_mandatory_finds_nearest() {
        let mut cfg = SnapConfig::new(SnapType::Mandatory, 50.0);
        cfg.add_point(0.0, SnapAlignment::Start);
        cfg.add_point(200.0, SnapAlignment::Start);
        cfg.add_point(400.0, SnapAlignment::Start);

        let target = find_snap_target(180.0, 0.0, 300.0, &cfg);
        assert_eq!(target, Some(200.0));
    }

    #[test]
    fn snap_proximity_within_threshold() {
        let mut cfg = SnapConfig::new(SnapType::Proximity, 30.0);
        cfg.add_point(100.0, SnapAlignment::Start);
        cfg.add_point(200.0, SnapAlignment::Start);

        // Within threshold.
        let target = find_snap_target(115.0, 0.0, 300.0, &cfg);
        assert_eq!(target, Some(100.0));

        // Outside threshold.
        let target = find_snap_target(155.0, 0.0, 300.0, &cfg);
        assert_eq!(target, None);
    }

    #[test]
    fn snap_respects_velocity() {
        let mut cfg = SnapConfig::new(SnapType::Mandatory, 50.0);
        cfg.add_point(0.0, SnapAlignment::Start);
        cfg.add_point(200.0, SnapAlignment::Start);
        cfg.add_point(400.0, SnapAlignment::Start);

        // Positive velocity (scrolling forward) from position 190 — should prefer 200.
        let target = find_snap_target(190.0, 0.1, 300.0, &cfg);
        assert_eq!(target, Some(200.0));

        // Negative velocity (scrolling backward) from position 210 — should prefer 200.
        let target = find_snap_target(210.0, -0.1, 300.0, &cfg);
        assert_eq!(target, Some(200.0));
    }

    #[test]
    fn snap_center_alignment() {
        let mut cfg = SnapConfig::new(SnapType::Mandatory, 50.0);
        // Snap point at content position 300, viewport 200.
        // With Center alignment, effective scroll offset = 300 - 200/2 = 200.
        cfg.add_point(300.0, SnapAlignment::Center);

        let target = find_snap_target(190.0, 0.0, 200.0, &cfg);
        assert_eq!(target, Some(200.0));
    }

    #[test]
    fn snap_empty_points() {
        let cfg = SnapConfig::new(SnapType::Mandatory, 50.0);
        let target = find_snap_target(100.0, 0.0, 300.0, &cfg);
        assert_eq!(target, None);
    }

    // ── Scrollbar ────────────────────────────────────────────────────

    #[test]
    fn scrollbar_not_visible_when_content_fits() {
        let state = ScrollState::new((400.0, 300.0), (400.0, 600.0));
        let sb = scrollbar::compute(&state, 600.0, Orientation::Vertical);
        assert!(!sb.visible);
    }

    #[test]
    fn scrollbar_thumb_size_proportional() {
        let state = ScrollState::new((400.0, 1200.0), (400.0, 600.0));
        let sb = scrollbar::compute(&state, 600.0, Orientation::Vertical);
        assert!(sb.visible);
        // ratio = 600/1200 = 0.5, thumb = 600 * 0.5 = 300.
        assert!((sb.thumb_size - 300.0).abs() < 1.0);
    }

    #[test]
    fn scrollbar_thumb_min_size() {
        let state = ScrollState::new((400.0, 100_000.0), (400.0, 600.0));
        let sb = scrollbar::compute(&state, 600.0, Orientation::Vertical);
        assert!(sb.thumb_size >= 30.0);
    }

    #[test]
    fn scrollbar_thumb_position_at_end() {
        let mut state = ScrollState::new((400.0, 1200.0), (400.0, 600.0));
        state.set_offset(0.0, 600.0);
        let sb = scrollbar::compute(&state, 600.0, Orientation::Vertical);
        // At max scroll, thumb should be at end.
        let expected_pos = 600.0 - sb.thumb_size;
        assert!((sb.thumb_position - expected_pos).abs() < 1.0);
    }

    #[test]
    fn scrollbar_hit_test_thumb() {
        let state = ScrollState::new((400.0, 1200.0), (400.0, 600.0));
        let sb = scrollbar::compute(&state, 580.0, Orientation::Vertical);
        // Scrollbar rect: 10px wide on right side, arrows 10px each.
        let rect = Rect::new(390.0, 0.0, 10.0, 600.0);
        // Thumb is at position 0 in track (scroll is at start).
        // Arrow zone = 10px (width). Track starts at y=10.
        // Thumb starts at track_start + thumb_position = 10 + 0 = 10.
        let hit = scrollbar::hit_test((395.0, 15.0), rect, &sb);
        assert_eq!(hit, ScrollbarHit::Thumb);
    }

    #[test]
    fn scrollbar_hit_test_up_arrow() {
        let state = ScrollState::new((400.0, 1200.0), (400.0, 600.0));
        let sb = scrollbar::compute(&state, 580.0, Orientation::Vertical);
        let rect = Rect::new(390.0, 0.0, 10.0, 600.0);
        let hit = scrollbar::hit_test((395.0, 5.0), rect, &sb);
        assert_eq!(hit, ScrollbarHit::UpArrow);
    }

    #[test]
    fn scrollbar_hit_test_down_arrow() {
        let state = ScrollState::new((400.0, 1200.0), (400.0, 600.0));
        let sb = scrollbar::compute(&state, 580.0, Orientation::Vertical);
        let rect = Rect::new(390.0, 0.0, 10.0, 600.0);
        let hit = scrollbar::hit_test((395.0, 595.0), rect, &sb);
        assert_eq!(hit, ScrollbarHit::DownArrow);
    }

    #[test]
    fn scrollbar_hit_test_track() {
        let mut state = ScrollState::new((400.0, 3000.0), (400.0, 600.0));
        state.set_offset(0.0, 0.0);
        let sb = scrollbar::compute(&state, 580.0, Orientation::Vertical);
        let rect = Rect::new(390.0, 0.0, 10.0, 600.0);
        // Click well below thumb (thumb is near top since offset=0).
        let hit = scrollbar::hit_test((395.0, 400.0), rect, &sb);
        assert!(matches!(
            hit,
            ScrollbarHit::Track {
                before_thumb: false
            }
        ));
    }

    #[test]
    fn scrollbar_hit_test_none() {
        let state = ScrollState::new((400.0, 1200.0), (400.0, 600.0));
        let sb = scrollbar::compute(&state, 580.0, Orientation::Vertical);
        let rect = Rect::new(390.0, 0.0, 10.0, 600.0);
        let hit = scrollbar::hit_test((100.0, 300.0), rect, &sb);
        assert_eq!(hit, ScrollbarHit::None);
    }

    // ── AutoHideController ───────────────────────────────────────────

    #[test]
    fn auto_hide_overlay_fades() {
        let mut ctrl = AutoHideController::new(ScrollbarStyle::Overlay);
        ctrl.on_activity();
        assert_eq!(ctrl.tick(0), 1.0);

        // After delay + fade.
        let opacity = ctrl.tick(1300);
        assert!(opacity < 1.0);
    }

    #[test]
    fn auto_hide_classic_always_visible() {
        let mut ctrl = AutoHideController::new(ScrollbarStyle::Classic);
        assert_eq!(ctrl.tick(5000), 1.0);
    }

    #[test]
    fn auto_hide_hidden_always_zero() {
        let mut ctrl = AutoHideController::new(ScrollbarStyle::Hidden);
        ctrl.on_activity();
        assert_eq!(ctrl.tick(0), 0.0);
    }

    // ── WheelConfig ──────────────────────────────────────────────────

    #[test]
    fn wheel_default_delta() {
        let w = WheelConfig::new();
        let delta = w.compute_delta(1.0, false, 600.0);
        // 1 tick * 3 lines * 20px = 60px.
        assert!((delta - 60.0).abs() < 0.01);
    }

    #[test]
    fn wheel_page_scroll() {
        let w = WheelConfig::new();
        let delta = w.compute_delta(1.0, true, 600.0);
        // Page scroll: 600 - 40 = 560px.
        assert!((delta - 560.0).abs() < 0.01);
    }

    #[test]
    fn wheel_natural_scrolling() {
        let mut w = WheelConfig::new();
        w.natural_scrolling = true;
        let delta = w.compute_delta(1.0, false, 600.0);
        assert!((delta - (-60.0)).abs() < 0.01);
    }

    // ── ScrollManager ────────────────────────────────────────────────

    #[test]
    fn manager_register_unregister() {
        let mut mgr = ScrollManager::new();
        let state = mgr.register(1, (800.0, 2000.0), (400.0, 600.0));
        assert_eq!(state.offset, (0.0, 0.0));
        assert_eq!(mgr.container_count(), 1);
        mgr.unregister(1);
        assert_eq!(mgr.container_count(), 0);
    }

    #[test]
    fn manager_wheel_smooth() {
        let mut mgr = ScrollManager::new();
        mgr.wheel_config.smooth_wheel = true;
        mgr.register(1, (800.0, 2000.0), (400.0, 600.0));
        mgr.handle_wheel(1, (0.0, 1.0), false);

        // Tick should produce animation.
        let updates = mgr.tick(200);
        assert!(!updates.is_empty());
        let (_, offset) = updates[0];
        assert!(offset.1 > 0.0);
    }

    #[test]
    fn manager_wheel_instant() {
        let mut mgr = ScrollManager::new();
        mgr.wheel_config.smooth_wheel = false;
        mgr.register(1, (800.0, 2000.0), (400.0, 600.0));
        mgr.handle_wheel(1, (0.0, 1.0), false);

        // With instant scroll, offset should be updated immediately (no animations).
        let state = mgr.state(1).unwrap();
        assert!(state.offset.1 > 0.0);
    }

    #[test]
    fn manager_scroll_to_element() {
        let mut mgr = ScrollManager::new();
        mgr.register(1, (800.0, 2000.0), (400.0, 600.0));

        // Element at y=1500, height=100 — currently not visible.
        mgr.scroll_to_element(1, (0.0, 1500.0, 100.0, 100.0));

        // Should have started smooth scroll.
        assert!(mgr.has_active_animations());

        // Tick enough to complete.
        for _ in 0..100 {
            mgr.tick(16);
        }

        let state = mgr.state(1).unwrap();
        // Element bottom (1600) should be visible (offset + viewport >= 1600).
        assert!(state.offset.1 + state.viewport_size.1 >= 1500.0);
    }

    #[test]
    fn manager_touch_momentum() {
        let mut mgr = ScrollManager::new();
        mgr.register(1, (800.0, 5000.0), (400.0, 600.0));
        // Start at mid-scroll so momentum has room to move in either direction.
        mgr.state_mut(1).unwrap().set_offset(0.0, 2000.0);

        mgr.handle_touch_start(1, (200.0, 300.0));
        mgr.handle_touch_move(1, (200.0, 300.0), 0);
        mgr.handle_touch_move(1, (200.0, 200.0), 10);
        mgr.handle_touch_move(1, (200.0, 50.0), 20);
        mgr.handle_touch_end(1);

        assert!(mgr.has_active_animations());

        // Tick until animations settle.
        let mut total_updates = 0;
        for _ in 0..2000 {
            let updates = mgr.tick(16);
            total_updates += updates.len();
            if !mgr.has_active_animations() {
                break;
            }
        }
        assert!(total_updates > 0);
    }

    #[test]
    fn manager_scrollbar_state() {
        let mut mgr = ScrollManager::new();
        mgr.register(1, (400.0, 2000.0), (400.0, 600.0));

        let sb = mgr.scrollbar_v(1, 600.0).unwrap();
        assert!(sb.visible);
        assert!(sb.thumb_size > 0.0);
        assert_eq!(sb.thumb_position, 0.0);
    }

    #[test]
    fn manager_scrollbar_click_page_down() {
        let mut mgr = ScrollManager::new();
        mgr.register(1, (400.0, 2000.0), (400.0, 600.0));

        mgr.handle_scrollbar_click(
            1,
            ScrollbarHit::Track {
                before_thumb: false,
            },
            Orientation::Vertical,
        );

        let state = mgr.state(1).unwrap();
        assert!((state.offset.1 - 600.0).abs() < 1.0);
    }

    #[test]
    fn manager_scrollbar_click_page_up() {
        let mut mgr = ScrollManager::new();
        mgr.register(1, (400.0, 2000.0), (400.0, 600.0));
        mgr.state_mut(1).unwrap().set_offset(0.0, 800.0);

        mgr.handle_scrollbar_click(
            1,
            ScrollbarHit::Track { before_thumb: true },
            Orientation::Vertical,
        );

        let state = mgr.state(1).unwrap();
        assert!((state.offset.1 - 200.0).abs() < 1.0);
    }

    #[test]
    fn manager_no_active_animations_initially() {
        let mut mgr = ScrollManager::new();
        mgr.register(1, (800.0, 2000.0), (400.0, 600.0));
        assert!(!mgr.has_active_animations());
    }

    #[test]
    fn manager_snap_after_momentum() {
        let mut mgr = ScrollManager::new();
        mgr.register(1, (400.0, 2000.0), (400.0, 600.0));

        let mut snap_cfg = SnapConfig::new(SnapType::Mandatory, 50.0);
        snap_cfg.add_point(0.0, SnapAlignment::Start);
        snap_cfg.add_point(600.0, SnapAlignment::Start);
        snap_cfg.add_point(1200.0, SnapAlignment::Start);
        mgr.set_snap_config(1, None, Some(snap_cfg));

        // Touch gesture that scrolls a bit, then releases.
        mgr.handle_touch_start(1, (200.0, 300.0));
        mgr.handle_touch_move(1, (200.0, 300.0), 0);
        mgr.handle_touch_move(1, (200.0, 250.0), 10);
        mgr.handle_touch_move(1, (200.0, 200.0), 20);
        mgr.handle_touch_end(1);

        // Tick until settled.
        for _ in 0..5000 {
            mgr.tick(16);
            if !mgr.has_active_animations() {
                break;
            }
        }

        let state = mgr.state(1).unwrap();
        // Should snap to nearest snap point (0 or 600).
        let snapped = (state.offset.1 - 0.0).abs() < 2.0 || (state.offset.1 - 600.0).abs() < 2.0;
        assert!(snapped, "Expected snap to 0 or 600, got {}", state.offset.1);
    }
}
