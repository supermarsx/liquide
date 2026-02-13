use crate::notification::*;
use liquide_interop::notification::{Notification, Urgency};

// ========== Helper ==========

fn make_notification(app: &str, summary: &str, urgency: Urgency, timeout_ms: i32) -> Notification {
    Notification {
        id: 0,
        app_name: app.to_string(),
        summary: summary.to_string(),
        body: String::new(),
        icon: None,
        urgency,
        timeout_ms,
        actions: Vec::new(),
    }
}

fn make_normal(app: &str, summary: &str) -> Notification {
    make_notification(app, summary, Urgency::Normal, 0)
}

fn make_critical(app: &str, summary: &str) -> Notification {
    make_notification(app, summary, Urgency::Critical, 0)
}

// ========== NotificationManager::new ==========

#[test]
fn notification_manager_new_defaults() {
    let mgr = NotificationManager::new(NotificationConfig::default());
    assert_eq!(mgr.active_count(), 0);
    assert_eq!(mgr.history_count(), 0);
    assert!(!mgr.is_dnd());
}

#[test]
fn notification_manager_new_dnd_from_config() {
    let config = NotificationConfig {
        dnd_enabled: true,
        ..NotificationConfig::default()
    };
    let mgr = NotificationManager::new(config);
    assert!(mgr.is_dnd());
}

// ========== notify ==========

#[test]
fn notification_notify_returns_some_id() {
    let mut mgr = NotificationManager::new(NotificationConfig::default());
    let id = mgr.notify(make_normal("app", "Hello"), 1000);
    assert!(id.is_some());
    assert_eq!(id.unwrap(), 1);
}

#[test]
fn notification_notify_adds_to_active() {
    let mut mgr = NotificationManager::new(NotificationConfig::default());
    mgr.notify(make_normal("app", "Hello"), 1000);
    assert_eq!(mgr.active_count(), 1);
    assert_eq!(mgr.active_notifications()[0].notification.summary, "Hello");
}

#[test]
fn notification_notify_sequential_ids() {
    let mut mgr = NotificationManager::new(NotificationConfig::default());
    let id1 = mgr.notify(make_normal("app", "First"), 1000).unwrap();
    let id2 = mgr.notify(make_normal("app", "Second"), 2000).unwrap();
    let id3 = mgr.notify(make_normal("app", "Third"), 3000).unwrap();
    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
    assert_eq!(id3, 3);
}

// ========== dismiss ==========

#[test]
fn notification_dismiss_moves_to_history() {
    let mut mgr = NotificationManager::new(NotificationConfig::default());
    let id = mgr.notify(make_normal("app", "Hello"), 1000).unwrap();
    mgr.dismiss(id);
    assert_eq!(mgr.active_count(), 0);
    assert_eq!(mgr.history_count(), 1);
    assert!(mgr.history().front().unwrap().dismissed);
}

#[test]
fn notification_dismiss_nonexistent_does_nothing() {
    let mut mgr = NotificationManager::new(NotificationConfig::default());
    mgr.notify(make_normal("app", "Hello"), 1000);
    mgr.dismiss(999);
    assert_eq!(mgr.active_count(), 1);
    assert_eq!(mgr.history_count(), 0);
}

// ========== dismiss_all ==========

#[test]
fn notification_dismiss_all() {
    let mut mgr = NotificationManager::new(NotificationConfig::default());
    mgr.notify(make_normal("app", "First"), 1000);
    mgr.notify(make_normal("app", "Second"), 2000);
    mgr.notify(make_normal("app", "Third"), 3000);
    mgr.dismiss_all();
    assert_eq!(mgr.active_count(), 0);
    assert_eq!(mgr.history_count(), 3);
    for n in mgr.history().iter() {
        assert!(n.dismissed);
    }
}

// ========== tick ==========

#[test]
fn notification_tick_expires_past_notifications() {
    let mut mgr = NotificationManager::new(NotificationConfig {
        default_timeout_ms: 5000,
        ..NotificationConfig::default()
    });
    let id = mgr.notify(make_normal("app", "Hello"), 1000).unwrap();
    // expires_at_us = 1000 + 5000*1000 = 5_001_000
    let expired = mgr.tick(5_001_000);
    assert_eq!(expired, vec![id]);
    assert_eq!(mgr.active_count(), 0);
    assert_eq!(mgr.history_count(), 1);
}

#[test]
fn notification_tick_returns_expired_ids() {
    let mut mgr = NotificationManager::new(NotificationConfig {
        default_timeout_ms: 1000,
        ..NotificationConfig::default()
    });
    let id1 = mgr.notify(make_normal("app", "A"), 0).unwrap();
    let id2 = mgr.notify(make_normal("app", "B"), 500_000).unwrap();
    // id1 expires at 0 + 1000*1000 = 1_000_000
    // id2 expires at 500_000 + 1_000_000 = 1_500_000
    let expired = mgr.tick(1_200_000);
    assert_eq!(expired, vec![id1]);
    assert_eq!(mgr.active_count(), 1);
    // Advance further to expire id2
    let expired2 = mgr.tick(2_000_000);
    assert_eq!(expired2, vec![id2]);
    assert_eq!(mgr.active_count(), 0);
}

#[test]
fn notification_tick_not_yet_expired() {
    let mut mgr = NotificationManager::new(NotificationConfig {
        default_timeout_ms: 5000,
        ..NotificationConfig::default()
    });
    mgr.notify(make_normal("app", "Hello"), 1000);
    let expired = mgr.tick(2000);
    assert!(expired.is_empty());
    assert_eq!(mgr.active_count(), 1);
}

// ========== DND mode ==========

#[test]
fn notification_dnd_blocks_normal() {
    let mut mgr = NotificationManager::new(NotificationConfig {
        dnd_enabled: true,
        dnd_allow_critical: true,
        ..NotificationConfig::default()
    });
    let id = mgr.notify(make_normal("app", "Hello"), 1000);
    assert!(id.is_none());
    assert_eq!(mgr.active_count(), 0);
}

#[test]
fn notification_dnd_blocks_low() {
    let mut mgr = NotificationManager::new(NotificationConfig {
        dnd_enabled: true,
        dnd_allow_critical: true,
        ..NotificationConfig::default()
    });
    let n = make_notification("app", "Low priority", Urgency::Low, 0);
    let id = mgr.notify(n, 1000);
    assert!(id.is_none());
    assert_eq!(mgr.active_count(), 0);
}

#[test]
fn notification_dnd_allows_critical_when_configured() {
    let mut mgr = NotificationManager::new(NotificationConfig {
        dnd_enabled: true,
        dnd_allow_critical: true,
        ..NotificationConfig::default()
    });
    let id = mgr.notify(make_critical("app", "Critical!"), 1000);
    assert!(id.is_some());
    assert_eq!(mgr.active_count(), 1);
}

#[test]
fn notification_dnd_blocks_critical_when_not_allowed() {
    let mut mgr = NotificationManager::new(NotificationConfig {
        dnd_enabled: true,
        dnd_allow_critical: false,
        ..NotificationConfig::default()
    });
    let id = mgr.notify(make_critical("app", "Critical!"), 1000);
    assert!(id.is_none());
    assert_eq!(mgr.active_count(), 0);
}

// ========== should_show ==========

#[test]
fn notification_should_show_no_dnd() {
    let mgr = NotificationManager::new(NotificationConfig::default());
    assert!(mgr.should_show(Urgency::Normal));
    assert!(mgr.should_show(Urgency::Low));
    assert!(mgr.should_show(Urgency::Critical));
}

#[test]
fn notification_should_show_dnd_on() {
    let mgr = NotificationManager::new(NotificationConfig {
        dnd_enabled: true,
        dnd_allow_critical: true,
        ..NotificationConfig::default()
    });
    assert!(!mgr.should_show(Urgency::Normal));
    assert!(!mgr.should_show(Urgency::Low));
    assert!(mgr.should_show(Urgency::Critical));
}

// ========== set_dnd / is_dnd ==========

#[test]
fn notification_set_dnd_is_dnd() {
    let mut mgr = NotificationManager::new(NotificationConfig::default());
    assert!(!mgr.is_dnd());
    mgr.set_dnd(true);
    assert!(mgr.is_dnd());
    mgr.set_dnd(false);
    assert!(!mgr.is_dnd());
}

// ========== active_notifications ==========

#[test]
fn notification_active_notifications_slice() {
    let mut mgr = NotificationManager::new(NotificationConfig::default());
    mgr.notify(make_normal("app", "A"), 1000);
    mgr.notify(make_normal("app", "B"), 2000);
    let active = mgr.active_notifications();
    assert_eq!(active.len(), 2);
    assert_eq!(active[0].notification.summary, "A");
    assert_eq!(active[1].notification.summary, "B");
}

// ========== active_count ==========

#[test]
fn notification_active_count() {
    let mut mgr = NotificationManager::new(NotificationConfig::default());
    assert_eq!(mgr.active_count(), 0);
    mgr.notify(make_normal("app", "A"), 1000);
    assert_eq!(mgr.active_count(), 1);
    mgr.notify(make_normal("app", "B"), 2000);
    assert_eq!(mgr.active_count(), 2);
}

// ========== history / history_count ==========

#[test]
fn notification_history_accessor() {
    let mut mgr = NotificationManager::new(NotificationConfig::default());
    let id = mgr.notify(make_normal("app", "A"), 1000).unwrap();
    mgr.dismiss(id);
    let history = mgr.history();
    assert_eq!(history.len(), 1);
    assert_eq!(history.front().unwrap().notification.summary, "A");
}

#[test]
fn notification_history_count() {
    let mut mgr = NotificationManager::new(NotificationConfig::default());
    assert_eq!(mgr.history_count(), 0);
    let id = mgr.notify(make_normal("app", "A"), 1000).unwrap();
    mgr.dismiss(id);
    assert_eq!(mgr.history_count(), 1);
}

// ========== unread_count ==========

#[test]
fn notification_unread_count() {
    let mut mgr = NotificationManager::new(NotificationConfig::default());
    mgr.notify(make_normal("app", "A"), 1000);
    mgr.notify(make_normal("app", "B"), 2000);
    assert_eq!(mgr.unread_count(), 2);
    // Dismiss one to history (still unread)
    mgr.dismiss(1);
    assert_eq!(mgr.unread_count(), 2); // one active + one in history, both unread
}

// ========== mark_read ==========

#[test]
fn notification_mark_read() {
    let mut mgr = NotificationManager::new(NotificationConfig::default());
    let id = mgr.notify(make_normal("app", "A"), 1000).unwrap();
    assert_eq!(mgr.unread_count(), 1);
    mgr.mark_read(id);
    assert_eq!(mgr.unread_count(), 0);
    assert!(mgr.active_notifications()[0].read);
}

#[test]
fn notification_mark_read_in_history() {
    let mut mgr = NotificationManager::new(NotificationConfig::default());
    let id = mgr.notify(make_normal("app", "A"), 1000).unwrap();
    mgr.dismiss(id);
    assert_eq!(mgr.unread_count(), 1);
    mgr.mark_read(id);
    assert_eq!(mgr.unread_count(), 0);
    assert!(mgr.history().front().unwrap().read);
}

// ========== clear_history ==========

#[test]
fn notification_clear_history() {
    let mut mgr = NotificationManager::new(NotificationConfig::default());
    let id1 = mgr.notify(make_normal("app", "A"), 1000).unwrap();
    let id2 = mgr.notify(make_normal("app", "B"), 2000).unwrap();
    mgr.dismiss(id1);
    mgr.dismiss(id2);
    assert_eq!(mgr.history_count(), 2);
    mgr.clear_history();
    assert_eq!(mgr.history_count(), 0);
}

// ========== max_visible pushes oldest to history ==========

#[test]
fn notification_max_visible_pushes_oldest_to_history() {
    let mut mgr = NotificationManager::new(NotificationConfig {
        max_visible: 2,
        ..NotificationConfig::default()
    });
    mgr.notify(make_normal("app", "First"), 1000);
    mgr.notify(make_normal("app", "Second"), 2000);
    // Adding a third should push "First" to history
    mgr.notify(make_normal("app", "Third"), 3000);
    assert_eq!(mgr.active_count(), 2);
    assert_eq!(mgr.history_count(), 1);
    assert_eq!(
        mgr.history().front().unwrap().notification.summary,
        "First"
    );
    // Active should contain Second and Third
    assert_eq!(mgr.active_notifications()[0].notification.summary, "Second");
    assert_eq!(mgr.active_notifications()[1].notification.summary, "Third");
}

// ========== history capacity ring ==========

#[test]
fn notification_history_capacity_ring() {
    let mut mgr = NotificationManager::new(NotificationConfig {
        max_visible: 10,
        history_capacity: 3,
        ..NotificationConfig::default()
    });
    // Add and dismiss 5 notifications
    for i in 0..5 {
        let id = mgr
            .notify(make_normal("app", &format!("N{i}")), (i as u64) * 1000)
            .unwrap();
        mgr.dismiss(id);
    }
    // History should be capped at 3 (most recent first)
    assert_eq!(mgr.history_count(), 3);
    assert_eq!(
        mgr.history().front().unwrap().notification.summary,
        "N4"
    );
    assert_eq!(
        mgr.history().back().unwrap().notification.summary,
        "N2"
    );
}

// ========== default timeout used when timeout_ms is 0 ==========

#[test]
fn notification_default_timeout_when_zero() {
    let mut mgr = NotificationManager::new(NotificationConfig {
        default_timeout_ms: 3000,
        ..NotificationConfig::default()
    });
    let n = make_notification("app", "Hello", Urgency::Normal, 0);
    mgr.notify(n, 1000);
    // expires_at_us should be 1000 + 3000 * 1000 = 3_001_000
    let shell_notif = &mgr.active_notifications()[0];
    assert_eq!(shell_notif.expires_at_us, 1000 + 3_000_000);
}

#[test]
fn notification_custom_timeout_used_when_positive() {
    let mut mgr = NotificationManager::new(NotificationConfig {
        default_timeout_ms: 3000,
        ..NotificationConfig::default()
    });
    let n = make_notification("app", "Hello", Urgency::Normal, 2000);
    mgr.notify(n, 1000);
    // timeout_ms=2000, timeout_us = 2000*1000 = 2_000_000
    // expires_at_us = 1000 + 2_000_000 = 2_001_000
    let shell_notif = &mgr.active_notifications()[0];
    assert_eq!(shell_notif.expires_at_us, 1000 + 2_000_000);
}

// ========== Display impls ==========

#[test]
fn notification_position_display() {
    assert_eq!(format!("{}", NotificationPosition::TopRight), "TopRight");
    assert_eq!(format!("{}", NotificationPosition::TopLeft), "TopLeft");
    assert_eq!(
        format!("{}", NotificationPosition::BottomRight),
        "BottomRight"
    );
    assert_eq!(
        format!("{}", NotificationPosition::BottomLeft),
        "BottomLeft"
    );
}

#[test]
fn notification_manager_display() {
    let mut mgr = NotificationManager::new(NotificationConfig::default());
    mgr.notify(make_normal("app", "A"), 1000);
    let s = format!("{mgr}");
    assert!(s.contains("active=1"));
    assert!(s.contains("history=0"));
    assert!(s.contains("dnd=false"));
}

#[test]
fn notification_manager_display_with_dnd() {
    let mgr = NotificationManager::new(NotificationConfig {
        dnd_enabled: true,
        ..NotificationConfig::default()
    });
    let s = format!("{mgr}");
    assert!(s.contains("dnd=true"));
}
