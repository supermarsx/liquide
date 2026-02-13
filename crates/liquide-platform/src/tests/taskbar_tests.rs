use crate::taskbar::{JumpListItem, NullTaskbar, TaskbarIntegration};

#[test]
fn set_progress_returns_ok() {
    let mut taskbar = NullTaskbar;
    assert!(taskbar.set_progress(1, 0.5).is_ok());
}

#[test]
fn set_progress_zero_returns_ok() {
    let mut taskbar = NullTaskbar;
    assert!(taskbar.set_progress(1, 0.0).is_ok());
}

#[test]
fn set_overlay_icon_returns_ok() {
    let mut taskbar = NullTaskbar;
    assert!(taskbar.set_overlay_icon(1, &[0u8; 8]).is_ok());
}

#[test]
fn set_badge_count_returns_ok() {
    let mut taskbar = NullTaskbar;
    assert!(taskbar.set_badge_count(42).is_ok());
}

#[test]
fn add_jump_list_item_returns_ok() {
    let mut taskbar = NullTaskbar;
    let item = JumpListItem {
        title: "Open".to_string(),
        description: "Open the application".to_string(),
        icon: "app-icon".to_string(),
        action: "open".to_string(),
    };
    assert!(taskbar.add_jump_list_item(item).is_ok());
}

#[test]
fn null_taskbar_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<NullTaskbar>();
}
