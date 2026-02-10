use crate::notification::*;

#[test]
fn test_create_notification() {
    let n = Notification::new("MyApp", "Hello World");
    assert_eq!(n.app_name, "MyApp");
    assert_eq!(n.summary, "Hello World");
    assert_eq!(n.urgency, Urgency::Normal);
    assert_eq!(n.timeout_ms, -1);
}

#[test]
fn test_urgency_levels() {
    let mut n = Notification::new("App", "Test");
    n.urgency = Urgency::Low;
    assert_eq!(n.urgency, Urgency::Low);
    n.urgency = Urgency::Critical;
    assert_eq!(n.urgency, Urgency::Critical);
}

#[test]
fn test_null_service_discards() {
    let mut svc = NullNotificationService;
    let id = svc.notify(Notification::new("App", "Test")).unwrap();
    assert_eq!(id, 0);
    assert!(svc.list().is_empty());
}

#[test]
fn test_memory_service_stores() {
    let mut svc = MemoryNotificationService::new();
    let id1 = svc.notify(Notification::new("App", "First")).unwrap();
    let id2 = svc.notify(Notification::new("App", "Second")).unwrap();
    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
    assert_eq!(svc.list().len(), 2);
}

#[test]
fn test_close() {
    let mut svc = MemoryNotificationService::new();
    let id = svc.notify(Notification::new("App", "Test")).unwrap();
    svc.close(id).unwrap();
    assert!(svc.list().is_empty());
}

#[test]
fn test_list_contents() {
    let mut svc = MemoryNotificationService::new();
    svc.notify(Notification::new("App1", "Hello")).unwrap();
    svc.notify(Notification::new("App2", "World")).unwrap();
    let list = svc.list();
    assert_eq!(list[0].app_name, "App1");
    assert_eq!(list[1].summary, "World");
}

#[test]
fn test_actions() {
    let mut n = Notification::new("App", "Test");
    n.actions.push(NotificationAction::new("reply", "Reply"));
    n.actions.push(NotificationAction::new("dismiss", "Dismiss"));
    assert_eq!(n.actions.len(), 2);
    assert_eq!(n.actions[0].key, "reply");
}

#[test]
fn test_display() {
    let n = Notification::new("MyApp", "Alert!");
    let s = format!("{n}");
    assert!(s.contains("MyApp"));
    assert!(s.contains("Alert!"));
}
