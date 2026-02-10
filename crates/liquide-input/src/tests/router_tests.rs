use liquide_compositor::geometry::Rect;

use crate::event::InputEvent;
use crate::keyboard::*;
use crate::mouse::*;
use crate::touch::*;
use crate::router::*;

struct TestSurface {
    id: u64,
    bounds: Rect,
}

impl InputTarget for TestSurface {
    fn id(&self) -> u64 { self.id }
    fn bounds(&self) -> Rect { self.bounds }
}

fn make_key_event() -> InputEvent {
    InputEvent::Keyboard(KeyEvent::new(KeyCode::A, KeyState::Pressed, Modifiers::new(), 30, 0))
}

fn make_mouse_event(x: f32, y: f32) -> InputEvent {
    InputEvent::Mouse(MouseEvent::Move { x, y })
}

#[test]
fn router_no_focus_initially() {
    let router = InputRouter::new();
    assert_eq!(router.focused(), None);
}

#[test]
fn router_set_focus() {
    let mut router = InputRouter::new();
    router.set_focus(42);
    assert_eq!(router.focused(), Some(42));
}

#[test]
fn router_route_to_focused() {
    let mut router = InputRouter::new();
    router.set_focus(1);
    let s1 = TestSurface { id: 1, bounds: Rect::new(0.0, 0.0, 100.0, 100.0) };
    let surfaces: Vec<&dyn InputTarget> = vec![&s1];
    let result = router.route(&make_key_event(), &surfaces);
    assert!(result.is_some());
    assert_eq!(result.unwrap().0, 1);
}

#[test]
fn router_grab_keyboard() {
    let mut router = InputRouter::new();
    router.set_focus(1);
    router.set_grab(GrabMode::Keyboard { surface_id: 2 });
    let s1 = TestSurface { id: 1, bounds: Rect::new(0.0, 0.0, 100.0, 100.0) };
    let s2 = TestSurface { id: 2, bounds: Rect::new(100.0, 0.0, 100.0, 100.0) };
    let surfaces: Vec<&dyn InputTarget> = vec![&s1, &s2];
    let result = router.route(&make_key_event(), &surfaces);
    assert_eq!(result.unwrap().0, 2); // Goes to grab target, not focus
}

#[test]
fn router_grab_pointer() {
    let mut router = InputRouter::new();
    router.set_grab(GrabMode::Pointer { surface_id: 3 });
    let s1 = TestSurface { id: 1, bounds: Rect::new(0.0, 0.0, 100.0, 100.0) };
    let surfaces: Vec<&dyn InputTarget> = vec![&s1];
    let result = router.route(&make_mouse_event(50.0, 50.0), &surfaces);
    assert_eq!(result.unwrap().0, 3); // Goes to grab target even though point is in s1
}

#[test]
fn router_release_grab() {
    let mut router = InputRouter::new();
    router.set_grab(GrabMode::Full { surface_id: 99 });
    router.release_grab();
    let s1 = TestSurface { id: 1, bounds: Rect::new(0.0, 0.0, 100.0, 100.0) };
    let surfaces: Vec<&dyn InputTarget> = vec![&s1];
    let result = router.route(&make_mouse_event(50.0, 50.0), &surfaces);
    assert_eq!(result.unwrap().0, 1); // Hit-test succeeds after grab released
}

#[test]
fn router_route_no_surfaces_none() {
    let router = InputRouter::new();
    let surfaces: Vec<&dyn InputTarget> = vec![];
    let result = router.route(&make_key_event(), &surfaces);
    assert!(result.is_none());
}

#[test]
fn router_hit_test_routes_to_surface() {
    let router = InputRouter::new();
    let s1 = TestSurface { id: 1, bounds: Rect::new(0.0, 0.0, 100.0, 100.0) };
    let s2 = TestSurface { id: 2, bounds: Rect::new(200.0, 200.0, 100.0, 100.0) };
    let surfaces: Vec<&dyn InputTarget> = vec![&s1, &s2];

    let result = router.route(&make_mouse_event(50.0, 50.0), &surfaces);
    assert_eq!(result.unwrap().0, 1);

    let result2 = router.route(&make_mouse_event(250.0, 250.0), &surfaces);
    assert_eq!(result2.unwrap().0, 2);
}

#[test]
fn router_grab_overrides_hit_test() {
    let mut router = InputRouter::new();
    router.set_grab(GrabMode::Full { surface_id: 5 });
    let s1 = TestSurface { id: 1, bounds: Rect::new(0.0, 0.0, 100.0, 100.0) };
    let surfaces: Vec<&dyn InputTarget> = vec![&s1];
    let result = router.route(&make_mouse_event(50.0, 50.0), &surfaces);
    assert_eq!(result.unwrap().0, 5);
}

#[test]
fn router_full_grab() {
    let mut router = InputRouter::new();
    router.set_focus(1);
    router.set_grab(GrabMode::Full { surface_id: 10 });
    let s1 = TestSurface { id: 1, bounds: Rect::new(0.0, 0.0, 100.0, 100.0) };
    let surfaces: Vec<&dyn InputTarget> = vec![&s1];

    // Keyboard goes to grab
    let kb = router.route(&make_key_event(), &surfaces);
    assert_eq!(kb.unwrap().0, 10);

    // Mouse goes to grab
    let ms = router.route(&make_mouse_event(50.0, 50.0), &surfaces);
    assert_eq!(ms.unwrap().0, 10);

    // Touch goes to grab
    let pt = TouchPoint::new(1, 50.0, 50.0, 1.0);
    let te = TouchEvent::new(TouchPhase::Begin, pt, 0);
    let touch = router.route(&InputEvent::Touch(te), &surfaces);
    assert_eq!(touch.unwrap().0, 10);
}
