//! Tests for touch input, gesture translation, and the input translator.

use crate::gesture::GestureKind;
use crate::input::{GestureEvent, InputTranslator, MouseAction, TouchEvent, TouchMode, TouchPhase};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn tap_event(id: u32, x: f32, y: f32, phase: TouchPhase, timestamp: u64) -> TouchEvent {
    TouchEvent {
        id,
        x,
        y,
        phase,
        pressure: 1.0,
        timestamp,
    }
}

fn gesture(kind: GestureKind, x: f32, y: f32) -> GestureEvent {
    GestureEvent {
        kind,
        position_x: x,
        position_y: y,
        scale: None,
        delta_x: None,
        delta_y: None,
        timestamp: 0,
    }
}

// ===========================================================================
// TouchMode display
// ===========================================================================

#[test]
fn test_touch_mode_display() {
    assert_eq!(TouchMode::Direct.to_string(), "direct");
    assert_eq!(TouchMode::Trackpad.to_string(), "trackpad");
    assert_eq!(TouchMode::Hybrid.to_string(), "hybrid");
}

// ===========================================================================
// TouchPhase display
// ===========================================================================

#[test]
fn test_touch_phase_display() {
    assert_eq!(TouchPhase::Began.to_string(), "began");
    assert_eq!(TouchPhase::Moved.to_string(), "moved");
    assert_eq!(TouchPhase::Ended.to_string(), "ended");
    assert_eq!(TouchPhase::Cancelled.to_string(), "cancelled");
}

// ===========================================================================
// Direct mode translation
// ===========================================================================

#[test]
fn test_direct_touch_began_produces_move() {
    let mut translator = InputTranslator::new(TouchMode::Direct);
    let event = tap_event(1, 100.0, 200.0, TouchPhase::Began, 0);
    let action = translator.translate_touch(&event);
    assert_eq!(action, Some(MouseAction::Move { x: 100.0, y: 200.0 }));
}

#[test]
fn test_direct_touch_moved_produces_move() {
    let mut translator = InputTranslator::new(TouchMode::Direct);
    let event = tap_event(1, 150.0, 250.0, TouchPhase::Moved, 10);
    let action = translator.translate_touch(&event);
    assert_eq!(action, Some(MouseAction::Move { x: 150.0, y: 250.0 }));
}

#[test]
fn test_direct_touch_ended_no_drag_produces_none() {
    let mut translator = InputTranslator::new(TouchMode::Direct);
    let event = tap_event(1, 100.0, 200.0, TouchPhase::Ended, 20);
    let action = translator.translate_touch(&event);
    assert!(action.is_none());
}

#[test]
fn test_direct_touch_ended_during_drag_produces_drag_end() {
    let mut translator = InputTranslator::new(TouchMode::Direct);
    // Start a drag via gesture.
    let g = gesture(GestureKind::LongPress, 50.0, 60.0);
    let _ = translator.translate_gesture(&g);
    assert!(translator.is_dragging());

    let event = tap_event(1, 70.0, 80.0, TouchPhase::Ended, 100);
    let action = translator.translate_touch(&event);
    assert_eq!(action, Some(MouseAction::DragEnd { x: 70.0, y: 80.0 }));
    assert!(!translator.is_dragging());
}

// ===========================================================================
// Gesture translation
// ===========================================================================

#[test]
fn test_single_tap_produces_left_click() {
    let mut translator = InputTranslator::new(TouchMode::Direct);
    let g = gesture(GestureKind::SingleTap, 100.0, 200.0);
    let action = translator.translate_gesture(&g);
    assert_eq!(action, Some(MouseAction::LeftClick { x: 100.0, y: 200.0 }));
}

#[test]
fn test_two_finger_tap_produces_right_click() {
    let mut translator = InputTranslator::new(TouchMode::Direct);
    let g = gesture(GestureKind::TwoFingerTap, 100.0, 200.0);
    let action = translator.translate_gesture(&g);
    assert_eq!(action, Some(MouseAction::RightClick { x: 100.0, y: 200.0 }));
}

#[test]
fn test_long_press_starts_drag() {
    let mut translator = InputTranslator::new(TouchMode::Direct);
    let g = gesture(GestureKind::LongPress, 50.0, 60.0);
    let action = translator.translate_gesture(&g);
    assert_eq!(action, Some(MouseAction::DragStart { x: 50.0, y: 60.0 }));
    assert!(translator.is_dragging());
}

#[test]
fn test_long_press_drag_continues_drag() {
    let mut translator = InputTranslator::new(TouchMode::Direct);
    // First start the drag.
    let _ = translator.translate_gesture(&gesture(GestureKind::LongPress, 10.0, 20.0));
    // Now drag move.
    let g = gesture(GestureKind::LongPressDrag, 30.0, 40.0);
    let action = translator.translate_gesture(&g);
    assert_eq!(action, Some(MouseAction::DragMove { x: 30.0, y: 40.0 }));
}

#[test]
fn test_three_finger_swipe_produces_middle_click() {
    let mut translator = InputTranslator::new(TouchMode::Direct);
    let g = gesture(GestureKind::ThreeFingerSwipe, 100.0, 200.0);
    let action = translator.translate_gesture(&g);
    assert_eq!(
        action,
        Some(MouseAction::MiddleClick { x: 100.0, y: 200.0 })
    );
}

#[test]
fn test_edge_swipe_produces_none() {
    let mut translator = InputTranslator::new(TouchMode::Direct);
    let g = gesture(GestureKind::EdgeSwipeLeft, 0.0, 100.0);
    assert!(translator.translate_gesture(&g).is_none());
    let g = gesture(GestureKind::EdgeSwipeRight, 400.0, 100.0);
    assert!(translator.translate_gesture(&g).is_none());
}

#[test]
fn test_pan_in_direct_mode_scrolls() {
    let mut translator = InputTranslator::new(TouchMode::Direct);
    let g = GestureEvent {
        kind: GestureKind::Pan,
        position_x: 100.0,
        position_y: 200.0,
        scale: None,
        delta_x: Some(5.0),
        delta_y: Some(-3.0),
        timestamp: 0,
    };
    let action = translator.translate_gesture(&g);
    assert_eq!(
        action,
        Some(MouseAction::Scroll {
            x: 100.0,
            y: 200.0,
            dx: 5.0,
            dy: -3.0,
        })
    );
}

#[test]
fn test_pan_in_trackpad_mode_moves_cursor() {
    let mut translator = InputTranslator::new(TouchMode::Trackpad);
    let g = GestureEvent {
        kind: GestureKind::Pan,
        position_x: 0.0,
        position_y: 0.0,
        scale: None,
        delta_x: Some(10.0),
        delta_y: Some(20.0),
        timestamp: 0,
    };
    let action = translator.translate_gesture(&g);
    assert_eq!(action, Some(MouseAction::Move { x: 10.0, y: 20.0 }));
}

// ===========================================================================
// End drag helper
// ===========================================================================

#[test]
fn test_end_drag_when_dragging() {
    let mut translator = InputTranslator::new(TouchMode::Direct);
    let _ = translator.translate_gesture(&gesture(GestureKind::LongPress, 10.0, 20.0));
    let action = translator.end_drag(30.0, 40.0);
    assert_eq!(action, Some(MouseAction::DragEnd { x: 30.0, y: 40.0 }));
    assert!(!translator.is_dragging());
}

#[test]
fn test_end_drag_when_not_dragging() {
    let mut translator = InputTranslator::new(TouchMode::Direct);
    let action = translator.end_drag(30.0, 40.0);
    assert!(action.is_none());
}
