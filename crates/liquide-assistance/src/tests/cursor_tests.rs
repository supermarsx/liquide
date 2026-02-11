use crate::cursor::cursor_appearance_for_mode;
use crate::mode::AssistanceMode;

#[test]
fn test_view_only_cursor() {
    let app = cursor_appearance_for_mode(AssistanceMode::ViewOnly, "Alice");
    assert_eq!(app.opacity, 0.5);
    assert!(app.visible);
    assert_eq!(app.ring_color, Some([100, 149, 237, 255]));
    assert_eq!(app.label.as_deref(), Some("Alice"));
}

#[test]
fn test_interactive_cursor() {
    let app = cursor_appearance_for_mode(AssistanceMode::Interactive, "Bob");
    assert_eq!(app.opacity, 1.0);
    assert!(app.visible);
    assert_eq!(app.ring_color, Some([50, 205, 50, 255]));
    assert_eq!(app.label.as_deref(), Some("Bob"));
}

#[test]
fn test_exclusive_cursor() {
    let app = cursor_appearance_for_mode(AssistanceMode::Exclusive, "Carol");
    assert_eq!(app.opacity, 1.0);
    assert!(app.visible);
    assert!(app.ring_color.is_none());
    assert_eq!(app.label.as_deref(), Some("Remote Control"));
}

#[test]
fn test_stealth_cursor_invisible() {
    let app = cursor_appearance_for_mode(AssistanceMode::Stealth, "Hidden");
    assert_eq!(app.opacity, 0.0);
    assert!(!app.visible);
    assert!(app.ring_color.is_none());
    assert!(app.label.is_none());
}
