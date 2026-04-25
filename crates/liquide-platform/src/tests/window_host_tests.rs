use crate::window_host::{
    NativeWindowHandle, NativeWindowHost, NativeWindowParams, NullWindowHost,
};
use liquide_compositor::geometry::Rect;

fn make_params(title: &str) -> NativeWindowParams {
    NativeWindowParams {
        title: title.to_string(),
        geometry: Rect::new(0.0, 0.0, 800.0, 600.0),
        window_type: "normal".to_string(),
        parent: None,
        app_id: "com.test.app".to_string(),
    }
}

#[test]
fn create_window_returns_handle() {
    let mut host = NullWindowHost::new();
    let handle = host.create_window(make_params("win1")).unwrap();
    assert_eq!(handle.0, 1);
}

#[test]
fn create_windows_returns_sequential_handles() {
    let mut host = NullWindowHost::new();
    let h1 = host.create_window(make_params("win1")).unwrap();
    let h2 = host.create_window(make_params("win2")).unwrap();
    let h3 = host.create_window(make_params("win3")).unwrap();
    assert_eq!(h1.0, 1);
    assert_eq!(h2.0, 2);
    assert_eq!(h3.0, 3);
}

#[test]
fn destroy_window_succeeds() {
    let mut host = NullWindowHost::new();
    let handle = host.create_window(make_params("win")).unwrap();
    assert!(host.destroy_window(handle).is_ok());
}

#[test]
fn destroy_nonexistent_window_is_ok() {
    let mut host = NullWindowHost::new();
    let handle = NativeWindowHandle(999);
    assert!(host.destroy_window(handle).is_ok());
}

#[test]
fn set_geometry_succeeds() {
    let mut host = NullWindowHost::new();
    let handle = host.create_window(make_params("win")).unwrap();
    let result = host.set_geometry(handle, Rect::new(10.0, 20.0, 640.0, 480.0));
    assert!(result.is_ok());
}

#[test]
fn set_title_succeeds() {
    let mut host = NullWindowHost::new();
    let handle = host.create_window(make_params("win")).unwrap();
    assert!(host.set_title(handle, "New Title").is_ok());
}

#[test]
fn set_icon_succeeds() {
    let mut host = NullWindowHost::new();
    let handle = host.create_window(make_params("win")).unwrap();
    assert!(host.set_icon(handle, &[0u8; 16]).is_ok());
}

#[test]
fn set_state_succeeds() {
    let mut host = NullWindowHost::new();
    let handle = host.create_window(make_params("win")).unwrap();
    assert!(host.set_state(handle, "maximized").is_ok());
}

#[test]
fn set_z_order_succeeds() {
    let mut host = NullWindowHost::new();
    let handle = host.create_window(make_params("win")).unwrap();
    assert!(host.set_z_order(handle, 10).is_ok());
}

#[test]
fn set_focus_succeeds() {
    let mut host = NullWindowHost::new();
    let handle = host.create_window(make_params("win")).unwrap();
    assert!(host.set_focus(handle).is_ok());
}
