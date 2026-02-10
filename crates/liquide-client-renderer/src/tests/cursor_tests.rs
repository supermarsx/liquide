use crate::cursor::{CursorShape, CursorState, ResizeDirection};

#[test]
fn test_default_cursor() {
    let c = CursorState::new();
    assert_eq!(c.x, 0);
    assert_eq!(c.y, 0);
    assert_eq!(c.shape, CursorShape::Arrow);
    assert!(c.visible);
    assert!(!c.has_custom_image());
}

#[test]
fn test_set_position() {
    let mut c = CursorState::new();
    c.set_position(100, 200);
    assert_eq!(c.x, 100);
    assert_eq!(c.y, 200);
}

#[test]
fn test_set_shape() {
    let mut c = CursorState::new();
    c.set_shape(CursorShape::Hand);
    assert_eq!(c.shape, CursorShape::Hand);
}

#[test]
fn test_hide_show() {
    let mut c = CursorState::new();
    assert!(c.visible);

    c.hide();
    assert!(!c.visible);
    assert_eq!(c.shape, CursorShape::Hidden);

    c.show();
    assert!(c.visible);
}

#[test]
fn test_custom_image() {
    let mut c = CursorState::new();
    let image = vec![0xFF; 32 * 32 * 4];
    c.set_custom_image(image.clone(), 32, 32, 0, 0);
    assert!(c.has_custom_image());
    assert_eq!(c.shape, CursorShape::Custom);
    assert_eq!(c.custom_width, 32);
    assert_eq!(c.custom_height, 32);
    assert_eq!(c.custom_image.as_ref().unwrap().len(), 32 * 32 * 4);
}

#[test]
fn test_custom_image_hotspot() {
    let mut c = CursorState::new();
    c.set_custom_image(vec![0; 64], 4, 4, 2, 3);
    assert_eq!(c.hotspot_x, 2);
    assert_eq!(c.hotspot_y, 3);
}

#[test]
fn test_cursor_shape_display() {
    assert_eq!(format!("{}", CursorShape::Arrow), "arrow");
    assert_eq!(format!("{}", CursorShape::Hand), "hand");
    assert_eq!(format!("{}", CursorShape::Text), "text");
    assert_eq!(format!("{}", CursorShape::Crosshair), "crosshair");
    assert_eq!(format!("{}", CursorShape::Wait), "wait");
    assert_eq!(format!("{}", CursorShape::Help), "help");
    assert_eq!(format!("{}", CursorShape::NotAllowed), "not-allowed");
    assert_eq!(format!("{}", CursorShape::Custom), "custom");
    assert_eq!(format!("{}", CursorShape::Hidden), "hidden");
}

#[test]
fn test_resize_direction_display() {
    assert_eq!(
        format!("{}", CursorShape::Resize(ResizeDirection::North)),
        "resize-north"
    );
    assert_eq!(
        format!("{}", CursorShape::Resize(ResizeDirection::SouthWest)),
        "resize-south-west"
    );
}

#[test]
fn test_cursor_state_display() {
    let mut c = CursorState::new();
    c.set_position(50, 75);
    let display = format!("{c}");
    assert!(display.contains("50"));
    assert!(display.contains("75"));
    assert!(display.contains("arrow"));
}

#[test]
fn test_cursor_shape_default() {
    let shape = CursorShape::default();
    assert_eq!(shape, CursorShape::Arrow);
}

#[test]
fn test_cursor_state_default() {
    let c = CursorState::default();
    assert_eq!(c.x, 0);
    assert_eq!(c.y, 0);
    assert!(c.visible);
}

#[test]
fn test_cursor_shape_serde() {
    let shape = CursorShape::Resize(ResizeDirection::NorthEast);
    let json = serde_json::to_string(&shape).unwrap();
    let deserialized: CursorShape = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, shape);
}

#[test]
fn test_cursor_state_serde() {
    let mut c = CursorState::new();
    c.set_position(100, 200);
    c.set_shape(CursorShape::Hand);
    let json = serde_json::to_string(&c).unwrap();
    let deserialized: CursorState = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.x, 100);
    assert_eq!(deserialized.y, 200);
    assert_eq!(deserialized.shape, CursorShape::Hand);
}
