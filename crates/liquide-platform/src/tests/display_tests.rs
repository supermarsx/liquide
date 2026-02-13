use crate::display::{DisplayBackend, NullDisplayBackend};
use liquide_compositor::geometry::Rect;

#[test]
fn null_display_returns_empty_monitors() {
    let backend = NullDisplayBackend;
    let monitors = backend.monitors();
    assert!(monitors.is_empty());
}

#[test]
fn null_display_primary_monitor_is_none() {
    let backend = NullDisplayBackend;
    assert!(backend.primary_monitor().is_none());
}

#[test]
fn null_display_virtual_screen_rect_is_zero() {
    let backend = NullDisplayBackend;
    let rect = backend.virtual_screen_rect();
    assert_eq!(rect, Rect::ZERO);
}

#[test]
fn null_display_monitors_returns_vec() {
    let backend = NullDisplayBackend;
    let monitors = backend.monitors();
    assert_eq!(monitors.len(), 0);
}

#[test]
fn null_display_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<NullDisplayBackend>();
}

#[test]
fn null_display_debug() {
    let backend = NullDisplayBackend;
    let debug = format!("{backend:?}");
    assert!(debug.contains("NullDisplayBackend"));
}

#[test]
fn null_display_default() {
    let backend = NullDisplayBackend::default();
    assert!(backend.monitors().is_empty());
}

#[test]
fn monitor_info_clone() {
    use crate::display::MonitorInfo;
    let info = MonitorInfo {
        id: 1,
        name: "Test".to_string(),
        geometry: Rect::new(0.0, 0.0, 1920.0, 1080.0),
        work_area: Rect::new(0.0, 0.0, 1920.0, 1040.0),
        dpi_scale: 1.0,
        primary: true,
        refresh_rate_hz: 60,
    };
    let cloned = info.clone();
    assert_eq!(cloned.id, 1);
    assert_eq!(cloned.name, "Test");
    assert!(cloned.primary);
    assert_eq!(cloned.refresh_rate_hz, 60);
}
