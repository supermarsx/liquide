use crate::tray::{NativeTray, NativeTrayParams, NullNativeTray, TrayUpdate};

fn make_tray_params() -> NativeTrayParams {
    NativeTrayParams {
        tooltip: "Test App".to_string(),
        icon_data: vec![0u8; 16],
        menu: vec!["Show".to_string(), "Quit".to_string()],
    }
}

#[test]
fn add_icon_returns_handle() {
    let mut tray = NullNativeTray::new();
    let handle = tray.add_icon(make_tray_params()).unwrap();
    assert_eq!(handle.0, 1);
}

#[test]
fn add_icons_returns_sequential_handles() {
    let mut tray = NullNativeTray::new();
    let h1 = tray.add_icon(make_tray_params()).unwrap();
    let h2 = tray.add_icon(make_tray_params()).unwrap();
    let h3 = tray.add_icon(make_tray_params()).unwrap();
    assert_eq!(h1.0, 1);
    assert_eq!(h2.0, 2);
    assert_eq!(h3.0, 3);
}

#[test]
fn update_icon_succeeds() {
    let mut tray = NullNativeTray::new();
    let handle = tray.add_icon(make_tray_params()).unwrap();
    let update = TrayUpdate {
        tooltip: Some("Updated".to_string()),
        icon_data: None,
    };
    assert!(tray.update_icon(handle, update).is_ok());
}

#[test]
fn update_icon_with_new_icon_data() {
    let mut tray = NullNativeTray::new();
    let handle = tray.add_icon(make_tray_params()).unwrap();
    let update = TrayUpdate {
        tooltip: None,
        icon_data: Some(vec![1u8; 32]),
    };
    assert!(tray.update_icon(handle, update).is_ok());
}

#[test]
fn remove_icon_succeeds() {
    let mut tray = NullNativeTray::new();
    let handle = tray.add_icon(make_tray_params()).unwrap();
    assert!(tray.remove_icon(handle).is_ok());
}

#[test]
fn remove_nonexistent_icon_is_ok() {
    let mut tray = NullNativeTray::new();
    let handle = crate::tray::NativeTrayHandle(999);
    assert!(tray.remove_icon(handle).is_ok());
}

#[test]
fn null_tray_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<NullNativeTray>();
}

#[test]
fn tray_update_default() {
    let update = TrayUpdate::default();
    assert!(update.tooltip.is_none());
    assert!(update.icon_data.is_none());
}
