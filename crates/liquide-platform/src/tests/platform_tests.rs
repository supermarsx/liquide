use crate::{NullPlatform, PlatformBackend, PlatformError};
use crate::event_loop::PlatformEvent;
use crate::window_host::{NativeWindowHandle, NativeWindowParams};
use liquide_compositor::geometry::Rect;

// ── NullPlatform tests ─────────────────────────────────────────────

#[test]
fn null_platform_name() {
    let platform = NullPlatform::new();
    assert_eq!(platform.platform_name(), "null");
}

#[test]
fn null_platform_display_empty() {
    let platform = NullPlatform::new();
    assert!(platform.display().monitors().is_empty());
    assert!(platform.display().primary_monitor().is_none());
}

#[test]
fn null_platform_poll_event_none() {
    let mut platform = NullPlatform::new();
    assert!(platform.poll_event().is_none());
}

#[test]
fn null_platform_wait_event_quit() {
    let mut platform = NullPlatform::new();
    let event = platform.wait_event();
    assert!(matches!(event, PlatformEvent::Quit));
}

#[test]
fn null_platform_present_frame_ok() {
    use liquide_compositor::pixel::PixelFormat;
    let mut platform = NullPlatform::new();
    let handle = NativeWindowHandle(1);
    let pixels = [0u8; 32];
    let result = platform.present_frame(handle, &pixels, 2, 2, 8, PixelFormat::Bgra8);
    assert!(result.is_ok());
}

#[test]
fn null_platform_keymap_is_null() {
    let platform = NullPlatform::new();
    assert_eq!(platform.keymap().platform_name(), "null");
}

#[test]
fn null_platform_window_host_create_destroy() {
    let mut platform = NullPlatform::new();
    let params = NativeWindowParams {
        title: "test".to_string(),
        geometry: Rect::new(0.0, 0.0, 100.0, 100.0),
        window_type: "normal".to_string(),
        parent: None,
        app_id: "test".to_string(),
    };
    let handle = platform.window_host().create_window(params).unwrap();
    assert!(platform.window_host().destroy_window(handle).is_ok());
}

#[test]
fn null_platform_set_cursor_shape_returns_false() {
    let mut platform = NullPlatform::new();
    let handle = NativeWindowHandle(1);
    assert!(!platform.set_cursor_shape(handle, "pointer"));
}

#[test]
fn null_platform_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<NullPlatform>();
}

#[test]
fn null_platform_debug() {
    let platform = NullPlatform::new();
    let debug = format!("{platform:?}");
    assert!(debug.contains("NullPlatform"));
}

// ── PlatformError tests ────────────────────────────────────────────

#[test]
fn platform_error_display_variant() {
    let err = PlatformError::Display("monitor not found".into());
    assert_eq!(format!("{err}"), "display error: monitor not found");
}

#[test]
fn platform_error_window_variant() {
    let err = PlatformError::Window("creation failed".into());
    assert_eq!(format!("{err}"), "window error: creation failed");
}

#[test]
fn platform_error_taskbar_variant() {
    let err = PlatformError::Taskbar("not supported".into());
    assert_eq!(format!("{err}"), "taskbar error: not supported");
}

#[test]
fn platform_error_tray_variant() {
    let err = PlatformError::Tray("icon failed".into());
    assert_eq!(format!("{err}"), "tray error: icon failed");
}

#[test]
fn platform_error_notification_variant() {
    let err = PlatformError::Notification("permission denied".into());
    assert_eq!(format!("{err}"), "notification error: permission denied");
}

#[test]
fn platform_error_dragdrop_variant() {
    let err = PlatformError::DragDrop("not available".into());
    assert_eq!(format!("{err}"), "drag-drop error: not available");
}

#[test]
fn platform_error_keymap_variant() {
    let err = PlatformError::Keymap("unknown layout".into());
    assert_eq!(format!("{err}"), "keymap error: unknown layout");
}

#[test]
fn platform_error_event_loop_variant() {
    let err = PlatformError::EventLoop("timeout".into());
    assert_eq!(format!("{err}"), "event loop error: timeout");
}

#[test]
fn platform_error_presentation_variant() {
    let err = PlatformError::Presentation("swap chain lost".into());
    assert_eq!(format!("{err}"), "presentation error: swap chain lost");
}

#[test]
fn platform_error_other_variant() {
    let err = PlatformError::Other("something".into());
    assert_eq!(format!("{err}"), "something");
}

#[test]
fn platform_error_debug_impl() {
    let err = PlatformError::Display("test".into());
    let debug = format!("{err:?}");
    assert!(debug.contains("Display"));
}

// ── Data type serde tests ──────────────────────────────────────────

#[test]
fn monitor_info_serde_roundtrip() {
    use crate::display::MonitorInfo;
    let info = MonitorInfo {
        id: 2,
        name: "DP-1".to_string(),
        geometry: Rect::new(0.0, 0.0, 2560.0, 1440.0),
        work_area: Rect::new(0.0, 48.0, 2560.0, 1392.0),
        dpi_scale: 1.5,
        primary: false,
        refresh_rate_hz: 144,
    };
    let json = serde_json::to_string(&info).unwrap();
    let deserialized: MonitorInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.id, 2);
    assert_eq!(deserialized.name, "DP-1");
    assert!(!deserialized.primary);
    assert_eq!(deserialized.refresh_rate_hz, 144);
    assert!((deserialized.dpi_scale - 1.5).abs() < f32::EPSILON);
}

#[test]
fn native_window_handle_serde_roundtrip() {
    let handle = NativeWindowHandle(12345);
    let json = serde_json::to_string(&handle).unwrap();
    let deserialized: NativeWindowHandle = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, handle);
}

#[test]
fn native_window_params_serde_roundtrip() {
    let params = NativeWindowParams {
        title: "My Window".to_string(),
        geometry: Rect::new(100.0, 200.0, 800.0, 600.0),
        window_type: "dialog".to_string(),
        parent: Some(NativeWindowHandle(1)),
        app_id: "com.example.app".to_string(),
    };
    let json = serde_json::to_string(&params).unwrap();
    let deserialized: NativeWindowParams = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.title, "My Window");
    assert_eq!(deserialized.window_type, "dialog");
    assert_eq!(deserialized.parent.unwrap().0, 1);
}

#[test]
fn jump_list_item_serde_roundtrip() {
    use crate::taskbar::JumpListItem;
    let item = JumpListItem {
        title: "Recent".to_string(),
        description: "Open recent files".to_string(),
        icon: "folder".to_string(),
        action: "open_recent".to_string(),
    };
    let json = serde_json::to_string(&item).unwrap();
    let deserialized: JumpListItem = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.title, "Recent");
    assert_eq!(deserialized.action, "open_recent");
}

#[test]
fn notification_params_serde_roundtrip() {
    use crate::notifications::NativeNotificationParams;
    let params = NativeNotificationParams {
        title: "Alert".to_string(),
        body: "Something happened".to_string(),
        icon: Some("warning".to_string()),
        urgency: "critical".to_string(),
        timeout_ms: 10000,
        actions: vec!["Dismiss".to_string(), "View".to_string()],
        sound: false,
    };
    let json = serde_json::to_string(&params).unwrap();
    let deserialized: NativeNotificationParams = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.title, "Alert");
    assert_eq!(deserialized.urgency, "critical");
    assert_eq!(deserialized.actions.len(), 2);
    assert!(!deserialized.sound);
}

#[test]
fn tray_params_serde_roundtrip() {
    use crate::tray::NativeTrayParams;
    let params = NativeTrayParams {
        tooltip: "My App".to_string(),
        icon_data: vec![1, 2, 3, 4],
        menu: vec!["Open".to_string(), "Exit".to_string()],
    };
    let json = serde_json::to_string(&params).unwrap();
    let deserialized: NativeTrayParams = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.tooltip, "My App");
    assert_eq!(deserialized.icon_data, vec![1, 2, 3, 4]);
    assert_eq!(deserialized.menu.len(), 2);
}

#[test]
fn tray_handle_serde_roundtrip() {
    use crate::tray::NativeTrayHandle;
    let handle = NativeTrayHandle(99);
    let json = serde_json::to_string(&handle).unwrap();
    let deserialized: NativeTrayHandle = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, handle);
}

// ── Handle equality / hash tests ───────────────────────────────────

#[test]
fn native_window_handle_equality() {
    let h1 = NativeWindowHandle(10);
    let h2 = NativeWindowHandle(10);
    let h3 = NativeWindowHandle(20);
    assert_eq!(h1, h2);
    assert_ne!(h1, h3);
}

#[test]
fn native_window_handle_hash_consistent() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(NativeWindowHandle(1));
    set.insert(NativeWindowHandle(2));
    set.insert(NativeWindowHandle(1)); // duplicate
    assert_eq!(set.len(), 2);
}

#[test]
fn native_tray_handle_equality() {
    use crate::tray::NativeTrayHandle;
    let h1 = NativeTrayHandle(5);
    let h2 = NativeTrayHandle(5);
    let h3 = NativeTrayHandle(6);
    assert_eq!(h1, h2);
    assert_ne!(h1, h3);
}

#[test]
fn native_tray_handle_hash_consistent() {
    use std::collections::HashSet;
    use crate::tray::NativeTrayHandle;
    let mut set = HashSet::new();
    set.insert(NativeTrayHandle(10));
    set.insert(NativeTrayHandle(20));
    set.insert(NativeTrayHandle(10)); // duplicate
    assert_eq!(set.len(), 2);
}
