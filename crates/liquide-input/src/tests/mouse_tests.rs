use crate::mouse::*;

#[test]
fn mouse_button_variants() {
    assert_ne!(MouseButton::Left, MouseButton::Right);
    assert_ne!(MouseButton::Middle, MouseButton::Back);
    assert_ne!(MouseButton::Forward, MouseButton::Left);
}

#[test]
fn button_other() {
    let b = MouseButton::Other(42);
    assert_eq!(b, MouseButton::Other(42));
    assert_ne!(b, MouseButton::Other(43));
    assert_ne!(b, MouseButton::Left);
}

#[test]
fn mouse_move_event() {
    let evt = MouseEvent::Move { x: 100.0, y: 200.0 };
    if let MouseEvent::Move { x, y } = evt {
        assert_eq!(x, 100.0);
        assert_eq!(y, 200.0);
    } else {
        panic!("expected Move");
    }
}

#[test]
fn mouse_button_event() {
    let evt = MouseEvent::Button {
        button: MouseButton::Left,
        state: ButtonState::Pressed,
        x: 50.0,
        y: 60.0,
    };
    if let MouseEvent::Button { button, state, .. } = evt {
        assert_eq!(button, MouseButton::Left);
        assert_eq!(state, ButtonState::Pressed);
    } else {
        panic!("expected Button");
    }
}

#[test]
fn mouse_scroll_vertical() {
    let evt = MouseEvent::Scroll {
        axis: ScrollAxis::Vertical,
        delta: -3.0,
        x: 0.0,
        y: 0.0,
    };
    if let MouseEvent::Scroll { axis, delta, .. } = evt {
        assert_eq!(axis, ScrollAxis::Vertical);
        assert_eq!(delta, -3.0);
    } else {
        panic!("expected Scroll");
    }
}

#[test]
fn mouse_scroll_horizontal() {
    let evt = MouseEvent::Scroll {
        axis: ScrollAxis::Horizontal,
        delta: 1.5,
        x: 10.0,
        y: 20.0,
    };
    if let MouseEvent::Scroll { axis, delta, .. } = evt {
        assert_eq!(axis, ScrollAxis::Horizontal);
        assert_eq!(delta, 1.5);
    } else {
        panic!("expected Scroll");
    }
}

#[test]
fn mouse_enter_leave() {
    let enter = MouseEvent::Enter { x: 5.0, y: 10.0 };
    let leave = MouseEvent::Leave;
    assert_ne!(enter, leave);
}

#[test]
fn button_state_variants() {
    assert_ne!(ButtonState::Pressed, ButtonState::Released);
}
