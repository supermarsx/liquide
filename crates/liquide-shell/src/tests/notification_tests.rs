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

// ========================================================================
// New features: system tray, DND schedule, actions, grouping, persistent
// ========================================================================

use liquide_interop::notification::NotificationAction;

// ========== toggle_dnd ==========

#[test]
fn notification_toggle_dnd() {
    let mut mgr = NotificationManager::new(NotificationConfig::default());
    assert!(!mgr.is_dnd());
    let new_state = mgr.toggle_dnd();
    assert!(new_state);
    assert!(mgr.is_dnd());
    let new_state2 = mgr.toggle_dnd();
    assert!(!new_state2);
    assert!(!mgr.is_dnd());
}

// ========== mark_all_read ==========

#[test]
fn notification_mark_all_read() {
    let mut mgr = NotificationManager::new(NotificationConfig::default());
    mgr.notify(make_normal("app", "A"), 1000);
    mgr.notify(make_normal("app", "B"), 2000);
    let id3 = mgr.notify(make_normal("app", "C"), 3000).unwrap();
    mgr.dismiss(id3); // move to history
    assert_eq!(mgr.unread_count(), 3);
    mgr.mark_all_read();
    assert_eq!(mgr.unread_count(), 0);
    // Verify active notifications are read
    for n in mgr.active_notifications() {
        assert!(n.read);
    }
    // Verify history notifications are read
    for n in mgr.history().iter() {
        assert!(n.read);
    }
}

// ========== notify_ext with grouping ==========

#[test]
fn notification_group_key_replaces() {
    let mut mgr = NotificationManager::new(NotificationConfig::default());
    let opts = NotifyOptions {
        group_key: Some("email-count".into()),
        ..Default::default()
    };
    let id1 = mgr
        .notify_ext(make_normal("mail", "1 new email"), 1000, Some(opts.clone()))
        .unwrap();
    assert_eq!(mgr.active_count(), 1);
    assert_eq!(
        mgr.active_notifications()[0].notification.summary,
        "1 new email"
    );

    // Same group key: replaces existing
    let id2 = mgr
        .notify_ext(make_normal("mail", "3 new emails"), 2000, Some(opts))
        .unwrap();
    assert_ne!(id1, id2);
    assert_eq!(mgr.active_count(), 1);
    assert_eq!(
        mgr.active_notifications()[0].notification.summary,
        "3 new emails"
    );
    // Replaced notification went to history
    assert_eq!(mgr.history_count(), 1);
    assert_eq!(
        mgr.history().front().unwrap().notification.summary,
        "1 new email"
    );
}

#[test]
fn notification_different_group_keys_coexist() {
    let mut mgr = NotificationManager::new(NotificationConfig::default());
    let opts_a = NotifyOptions {
        group_key: Some("group-a".into()),
        ..Default::default()
    };
    let opts_b = NotifyOptions {
        group_key: Some("group-b".into()),
        ..Default::default()
    };
    mgr.notify_ext(make_normal("app", "A"), 1000, Some(opts_a));
    mgr.notify_ext(make_normal("app", "B"), 2000, Some(opts_b));
    assert_eq!(mgr.active_count(), 2);
}

// ========== notify_ext persistent ==========

#[test]
fn notification_persistent_no_auto_expire() {
    let mut mgr = NotificationManager::new(NotificationConfig {
        default_timeout_ms: 1000,
        ..NotificationConfig::default()
    });
    let opts = NotifyOptions {
        persistent: true,
        ..Default::default()
    };
    mgr.notify_ext(make_normal("app", "Sticky"), 0, Some(opts));
    assert_eq!(mgr.active_count(), 1);
    // Even after a long time, it should not expire
    let expired = mgr.tick(100_000_000_000);
    assert!(expired.is_empty());
    assert_eq!(mgr.active_count(), 1);
}

// ========== notify_ext progress ==========

#[test]
fn notification_progress() {
    let mut mgr = NotificationManager::new(NotificationConfig::default());
    let opts = NotifyOptions {
        progress: Some(0.0),
        persistent: true,
        ..Default::default()
    };
    let id = mgr
        .notify_ext(make_normal("app", "Downloading"), 0, Some(opts))
        .unwrap();
    assert_eq!(mgr.active_notifications()[0].progress, Some(0.0));

    mgr.update_progress(id, 0.5);
    assert_eq!(mgr.active_notifications()[0].progress, Some(0.5));

    mgr.update_progress(id, 1.5); // clamps to 1.0
    assert_eq!(mgr.active_notifications()[0].progress, Some(1.0));
}

// ========== notify_ext silent + category ==========

#[test]
fn notification_ext_fields_stored() {
    let mut mgr = NotificationManager::new(NotificationConfig::default());
    let opts = NotifyOptions {
        silent: true,
        category: Some("im".into()),
        ..Default::default()
    };
    mgr.notify_ext(make_normal("app", "Message"), 0, Some(opts));
    let n = &mgr.active_notifications()[0];
    assert!(n.silent);
    assert_eq!(n.category.as_deref(), Some("im"));
}

// ========== invoke_action ==========

#[test]
fn notification_invoke_action_auto_dismiss() {
    let mut mgr = NotificationManager::new(NotificationConfig::default());
    let mut n = make_normal("app", "PR Review");
    n.actions = vec![
        NotificationAction::new("approve", "Approve"),
        NotificationAction::new("reject", "Reject"),
    ];
    let id = mgr.notify(n, 0).unwrap();
    assert_eq!(mgr.active_count(), 1);

    mgr.invoke_action(id, "approve");
    // Non-persistent notification auto-dismissed after action
    assert_eq!(mgr.active_count(), 0);
    assert_eq!(mgr.history_count(), 1);

    // Action event emitted on next tick
    let (_, events) = mgr.tick_with_events(1000);
    assert!(events.iter().any(|e| matches!(
        e,
        NotificationEvent::ActionInvoked {
            notification_id,
            action_id,
        } if *notification_id == id && action_id == "approve"
    )));
}

#[test]
fn notification_invoke_action_persistent_stays() {
    let mut mgr = NotificationManager::new(NotificationConfig::default());
    let opts = NotifyOptions {
        persistent: true,
        ..Default::default()
    };
    let mut n = make_normal("app", "Download");
    n.actions = vec![NotificationAction::new("cancel", "Cancel")];
    let id = mgr.notify_ext(n, 0, Some(opts)).unwrap();

    mgr.invoke_action(id, "cancel");
    // Persistent notification stays after action
    assert_eq!(mgr.active_count(), 1);
}

// ========== tick_with_events ==========

#[test]
fn notification_tick_with_events_expired() {
    let mut mgr = NotificationManager::new(NotificationConfig {
        default_timeout_ms: 1000,
        ..NotificationConfig::default()
    });
    let id = mgr.notify(make_normal("app", "A"), 0).unwrap();
    let (expired_ids, events) = mgr.tick_with_events(2_000_000);
    assert_eq!(expired_ids, vec![id]);
    assert!(events.iter().any(|e| matches!(e, NotificationEvent::Expired(eid) if *eid == id)));
}

// ========== DND schedule ==========

#[test]
fn notification_dnd_schedule_same_day() {
    let schedule = DndSchedule::new(9, 0, 17, 0);
    assert!(!schedule.is_active(8, 59));
    assert!(schedule.is_active(9, 0));
    assert!(schedule.is_active(12, 30));
    assert!(schedule.is_active(16, 59));
    assert!(!schedule.is_active(17, 0));
}

#[test]
fn notification_dnd_schedule_overnight() {
    let schedule = DndSchedule::new(22, 0, 7, 0);
    assert!(schedule.is_active(22, 0));
    assert!(schedule.is_active(23, 30));
    assert!(schedule.is_active(0, 0));
    assert!(schedule.is_active(3, 0));
    assert!(schedule.is_active(6, 59));
    assert!(!schedule.is_active(7, 0));
    assert!(!schedule.is_active(12, 0));
    assert!(!schedule.is_active(21, 59));
}

#[test]
fn notification_check_dnd_schedule_auto_enable() {
    let mut mgr = NotificationManager::new(NotificationConfig::default());
    mgr.set_dnd_schedule(Some(DndSchedule::new(22, 0, 7, 0)));
    assert!(!mgr.is_dnd());

    mgr.check_dnd_schedule(23, 0);
    assert!(mgr.is_dnd());

    mgr.check_dnd_schedule(8, 0);
    assert!(!mgr.is_dnd());
}

#[test]
fn notification_dnd_schedule_no_schedule_noop() {
    let mut mgr = NotificationManager::new(NotificationConfig::default());
    assert!(!mgr.is_dnd());
    mgr.check_dnd_schedule(12, 0);
    assert!(!mgr.is_dnd()); // no schedule, no change
}

// ========== Tray icon: add / remove ==========

#[test]
fn notification_tray_add_icon() {
    let mut mgr = NotificationManager::new(NotificationConfig::default());
    let id = mgr.add_tray_icon("TestApp", "Test tooltip", "test-icon", 0);
    assert_eq!(mgr.tray_icon_count(), 1);
    let icons = mgr.tray_icons();
    let icon = icons.get(&id).unwrap();
    assert_eq!(icon.app_name, "TestApp");
    assert_eq!(icon.tooltip, "Test tooltip");
    assert_eq!(icon.icon, "test-icon");
    assert!(icon.visible);
    assert!(!icon.auto_demoted);
    assert!(icon.badge.is_none());
    assert!(icon.menu_items.is_empty());
}

#[test]
fn notification_tray_add_multiple_icons() {
    let mut mgr = NotificationManager::new(NotificationConfig::default());
    let id1 = mgr.add_tray_icon("App1", "Tooltip1", "icon1", 0);
    let id2 = mgr.add_tray_icon("App2", "Tooltip2", "icon2", 0);
    assert_ne!(id1, id2);
    assert_eq!(mgr.tray_icon_count(), 2);
}

#[test]
fn notification_tray_remove_icon() {
    let mut mgr = NotificationManager::new(NotificationConfig::default());
    let id = mgr.add_tray_icon("App", "tip", "icon", 0);
    assert_eq!(mgr.tray_icon_count(), 1);
    mgr.remove_tray_icon(id);
    assert_eq!(mgr.tray_icon_count(), 0);
}

#[test]
fn notification_tray_remove_nonexistent() {
    let mut mgr = NotificationManager::new(NotificationConfig::default());
    mgr.remove_tray_icon(TrayIconId(999));
    assert_eq!(mgr.tray_icon_count(), 0);
}

// ========== Tray icon: update ==========

#[test]
fn notification_tray_update_icon() {
    let mut mgr = NotificationManager::new(NotificationConfig::default());
    let id = mgr.add_tray_icon("App", "old tip", "old-icon", 0);

    mgr.update_tray_icon(id, Some("new tip"), Some("new-icon"), Some(Some("3")), 1000);

    let icon = mgr.tray_icons().get(&id).unwrap();
    assert_eq!(icon.tooltip, "new tip");
    assert_eq!(icon.icon, "new-icon");
    assert_eq!(icon.badge.as_deref(), Some("3"));
    assert_eq!(icon.last_interaction_us, 1000);
}

#[test]
fn notification_tray_update_partial() {
    let mut mgr = NotificationManager::new(NotificationConfig::default());
    let id = mgr.add_tray_icon("App", "tip", "icon", 0);

    // Only update tooltip, leave icon and badge unchanged
    mgr.update_tray_icon(id, Some("new tip"), None, None, 500);

    let icon = mgr.tray_icons().get(&id).unwrap();
    assert_eq!(icon.tooltip, "new tip");
    assert_eq!(icon.icon, "icon"); // unchanged
    assert!(icon.badge.is_none()); // unchanged
}

#[test]
fn notification_tray_update_clears_badge() {
    let mut mgr = NotificationManager::new(NotificationConfig::default());
    let id = mgr.add_tray_icon("App", "tip", "icon", 0);
    mgr.update_tray_icon(id, None, None, Some(Some("5")), 100);
    assert_eq!(
        mgr.tray_icons().get(&id).unwrap().badge.as_deref(),
        Some("5")
    );
    // Clear badge
    mgr.update_tray_icon(id, None, None, Some(None), 200);
    assert!(mgr.tray_icons().get(&id).unwrap().badge.is_none());
}

// ========== Tray icon: auto-demote ==========

#[test]
fn notification_tray_auto_demote_on_tick() {
    let mut mgr = NotificationManager::new(NotificationConfig {
        tray_auto_demote_secs: 10, // 10 seconds for testing
        ..NotificationConfig::default()
    });
    let id = mgr.add_tray_icon("App", "tip", "icon", 0);

    // Before timeout: should still be visible
    let (_, events) = mgr.tick_with_events(5_000_000); // 5 seconds
    assert!(mgr.visible_tray_icons().iter().any(|i| i.id == id));
    assert!(mgr.overflow_tray_icons().is_empty());
    assert!(!events.iter().any(|e| matches!(e, NotificationEvent::TrayIconDemoted(_))));

    // After timeout: should be demoted
    let (_, events) = mgr.tick_with_events(11_000_000); // 11 seconds
    assert!(mgr.visible_tray_icons().is_empty());
    assert!(mgr.overflow_tray_icons().iter().any(|i| i.id == id));
    assert!(events.iter().any(|e| matches!(
        e,
        NotificationEvent::TrayIconDemoted(tid) if *tid == id
    )));
}

#[test]
fn notification_tray_auto_demote_not_repeated() {
    let mut mgr = NotificationManager::new(NotificationConfig {
        tray_auto_demote_secs: 10,
        ..NotificationConfig::default()
    });
    let id = mgr.add_tray_icon("App", "tip", "icon", 0);

    // First tick after timeout: demote event
    let (_, events1) = mgr.tick_with_events(11_000_000);
    assert_eq!(
        events1
            .iter()
            .filter(|e| matches!(e, NotificationEvent::TrayIconDemoted(_)))
            .count(),
        1
    );

    // Second tick: no duplicate event (already demoted)
    let (_, events2) = mgr.tick_with_events(20_000_000);
    assert!(
        !events2
            .iter()
            .any(|e| matches!(e, NotificationEvent::TrayIconDemoted(_)))
    );
}

#[test]
fn notification_tray_update_promotes() {
    let mut mgr = NotificationManager::new(NotificationConfig {
        tray_auto_demote_secs: 10,
        ..NotificationConfig::default()
    });
    let id = mgr.add_tray_icon("App", "tip", "icon", 0);

    // Demote
    mgr.tick_with_events(11_000_000);
    assert!(mgr.tray_icons().get(&id).unwrap().auto_demoted);

    // Update promotes
    mgr.update_tray_icon(id, Some("new tip"), None, None, 12_000_000);
    assert!(!mgr.tray_icons().get(&id).unwrap().auto_demoted);
    assert!(mgr.visible_tray_icons().iter().any(|i| i.id == id));
}

#[test]
fn notification_tray_touch_promotes() {
    let mut mgr = NotificationManager::new(NotificationConfig {
        tray_auto_demote_secs: 10,
        ..NotificationConfig::default()
    });
    let id = mgr.add_tray_icon("App", "tip", "icon", 0);

    // Demote
    mgr.tick_with_events(11_000_000);
    assert!(mgr.tray_icons().get(&id).unwrap().auto_demoted);

    // Touch (user click) promotes
    mgr.touch_tray_icon(id, 12_000_000);
    assert!(!mgr.tray_icons().get(&id).unwrap().auto_demoted);
}

// ========== Tray icon: menu ==========

#[test]
fn notification_tray_set_menu() {
    let mut mgr = NotificationManager::new(NotificationConfig::default());
    let id = mgr.add_tray_icon("App", "tip", "icon", 0);

    let items = vec![
        TrayMenuItem::new("show", "Show Window"),
        TrayMenuItem::separator(),
        TrayMenuItem::new("quit", "Quit"),
    ];
    mgr.set_tray_menu(id, items);

    let icon = mgr.tray_icons().get(&id).unwrap();
    assert_eq!(icon.menu_items.len(), 3);
    assert_eq!(icon.menu_items[0].label, "Show Window");
    assert!(!icon.menu_items[0].separator);
    assert!(icon.menu_items[1].separator);
    assert_eq!(icon.menu_items[2].id, "quit");
}

// ========== Tray icon: visible vs overflow ==========

#[test]
fn notification_tray_visible_overflow_split() {
    let mut mgr = NotificationManager::new(NotificationConfig {
        tray_auto_demote_secs: 10,
        ..NotificationConfig::default()
    });
    let id_old = mgr.add_tray_icon("OldApp", "old", "icon", 0);
    let id_new = mgr.add_tray_icon("NewApp", "new", "icon", 5_000_000);

    // Advance past demote threshold for old, but not new
    mgr.tick_with_events(11_000_000);

    let visible = mgr.visible_tray_icons();
    let overflow = mgr.overflow_tray_icons();

    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].id, id_new);
    assert_eq!(overflow.len(), 1);
    assert_eq!(overflow[0].id, id_old);
}

// ========== Display with tray ==========

#[test]
fn notification_manager_display_with_tray() {
    let mut mgr = NotificationManager::new(NotificationConfig::default());
    mgr.add_tray_icon("App", "tip", "icon", 0);
    let s = format!("{mgr}");
    assert!(s.contains("tray=1"));
}

// ========== TrayIconId display ==========

#[test]
fn notification_tray_icon_id_display() {
    let id = TrayIconId(42);
    assert_eq!(format!("{id}"), "TrayIcon(42)");
}

// ========== Max visible with persistent prefers non-persistent eviction ==========

#[test]
fn notification_max_visible_prefers_non_persistent_eviction() {
    let mut mgr = NotificationManager::new(NotificationConfig {
        max_visible: 2,
        ..NotificationConfig::default()
    });

    // Add one persistent and one normal
    let persistent_opts = NotifyOptions {
        persistent: true,
        ..Default::default()
    };
    mgr.notify_ext(make_normal("app", "Persistent"), 1000, Some(persistent_opts));
    mgr.notify(make_normal("app", "Normal"), 2000);

    // Add a third: should evict the normal one, not the persistent one
    mgr.notify(make_normal("app", "NewNormal"), 3000);
    assert_eq!(mgr.active_count(), 2);

    let summaries: Vec<&str> = mgr
        .active_notifications()
        .iter()
        .map(|n| n.notification.summary.as_str())
        .collect();
    assert!(summaries.contains(&"Persistent"));
    assert!(summaries.contains(&"NewNormal"));
    // The evicted "Normal" went to history
    assert_eq!(mgr.history_count(), 1);
    assert_eq!(
        mgr.history().front().unwrap().notification.summary,
        "Normal"
    );
}

// ========== DND schedule edge cases ==========

#[test]
fn notification_dnd_schedule_boundary_values() {
    let schedule = DndSchedule::new(0, 0, 0, 0);
    // start == end: same-day path, range is empty
    assert!(!schedule.is_active(0, 0));
    assert!(!schedule.is_active(12, 0));
}

#[test]
fn notification_dnd_schedule_full_day() {
    // 0:00 to 23:59 covers the full day
    let schedule = DndSchedule::new(0, 0, 23, 59);
    assert!(schedule.is_active(0, 0));
    assert!(schedule.is_active(12, 0));
    assert!(schedule.is_active(23, 58));
    assert!(!schedule.is_active(23, 59)); // end is exclusive
}

#[test]
fn notification_dnd_schedule_clamps_invalid() {
    let schedule = DndSchedule::new(25, 70, 30, 80);
    assert_eq!(schedule.start_hour, 23);
    assert_eq!(schedule.start_minute, 59);
    assert_eq!(schedule.end_hour, 23);
    assert_eq!(schedule.end_minute, 59);
}
