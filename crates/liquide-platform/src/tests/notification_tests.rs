use crate::notifications::{
    NativeNotificationParams, NativeNotifications, NullNativeNotifications,
};

fn make_notification_params() -> NativeNotificationParams {
    NativeNotificationParams {
        title: "Test".to_string(),
        body: "Hello, world!".to_string(),
        icon: None,
        urgency: "normal".to_string(),
        timeout_ms: 5000,
        actions: vec!["OK".to_string()],
        sound: true,
    }
}

#[test]
fn show_returns_id() {
    let mut notif = NullNativeNotifications::new();
    let id = notif.show(make_notification_params()).unwrap();
    assert_eq!(id, 1);
}

#[test]
fn show_returns_sequential_ids() {
    let mut notif = NullNativeNotifications::new();
    let id1 = notif.show(make_notification_params()).unwrap();
    let id2 = notif.show(make_notification_params()).unwrap();
    let id3 = notif.show(make_notification_params()).unwrap();
    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
    assert_eq!(id3, 3);
}

#[test]
fn dismiss_succeeds() {
    let mut notif = NullNativeNotifications::new();
    let id = notif.show(make_notification_params()).unwrap();
    assert!(notif.dismiss(id).is_ok());
}

#[test]
fn dismiss_nonexistent_is_ok() {
    let mut notif = NullNativeNotifications::new();
    assert!(notif.dismiss(999).is_ok());
}

#[test]
fn null_notifications_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<NullNativeNotifications>();
}

#[test]
fn notification_params_clone() {
    let params = make_notification_params();
    let cloned = params.clone();
    assert_eq!(cloned.title, "Test");
    assert_eq!(cloned.body, "Hello, world!");
    assert!(cloned.sound);
}
