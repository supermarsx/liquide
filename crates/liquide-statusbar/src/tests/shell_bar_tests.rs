use crate::items::*;
use crate::shell_bar::*;
use crate::slot::*;
use liquide_compositor::geometry::Rect;

// ========== ShellStatusBar::new ==========

#[test]
fn status_bar_new_default_config_creates_four_items() {
    let bar = ShellStatusBar::new(ShellBarConfig::default());
    assert_eq!(bar.item_count(), 5); // clock, notifications, connection, tray, session
    assert!(bar.find_item("clock").is_some());
    assert!(bar.find_item("notifications").is_some());
    assert!(bar.find_item("connection").is_some());
    assert!(bar.find_item("tray").is_some());
    assert!(bar.find_item("session").is_some());
}

#[test]
fn status_bar_new_all_disabled_creates_zero_items() {
    let config = ShellBarConfig {
        show_clock: false,
        show_notifications: false,
        show_connection_quality: false,
        show_tray: false,
        ..ShellBarConfig::default()
    };
    let bar = ShellStatusBar::new(config);
    assert_eq!(bar.item_count(), 1); // session button always present
}

// ========== add_item ==========

#[test]
fn status_bar_add_item() {
    let mut bar = ShellStatusBar::new(ShellBarConfig {
        show_clock: false,
        show_notifications: false,
        show_connection_quality: false,
        show_tray: false,
        ..ShellBarConfig::default()
    });
    assert_eq!(bar.item_count(), 1); // session button
    bar.add_item(StatusBarItem {
        id: "custom".into(),
        kind: StatusBarItemKind::Custom {
            plugin_id: "plugin1".into(),
            content: "hello".into(),
        },
        slot: StatusBarSlot::Left,
        visible: true,
        cached: false,
        last_update_us: 0,
    });
    assert_eq!(bar.item_count(), 2); // session + custom
}

// ========== remove_item ==========

#[test]
fn status_bar_remove_item() {
    let mut bar = ShellStatusBar::new(ShellBarConfig::default());
    assert_eq!(bar.item_count(), 5);
    let removed = bar.remove_item("clock");
    assert!(removed);
    assert_eq!(bar.item_count(), 4);
    assert!(bar.find_item("clock").is_none());
}

#[test]
fn status_bar_remove_item_nonexistent() {
    let mut bar = ShellStatusBar::new(ShellBarConfig::default());
    let removed = bar.remove_item("nonexistent");
    assert!(!removed);
    assert_eq!(bar.item_count(), 5);
}

// ========== update_clock ==========

#[test]
fn status_bar_update_clock_sets_dirty() {
    let mut bar = ShellStatusBar::new(ShellBarConfig::default());
    bar.mark_clean();
    assert!(!bar.is_dirty());
    bar.update_clock(12345);
    assert!(bar.is_dirty());
    let clock = bar.find_item("clock").unwrap();
    assert_eq!(clock.last_update_us, 12345);
    assert!(!clock.cached);
}

// ========== update_notification_count ==========

#[test]
fn status_bar_update_notification_count() {
    let mut bar = ShellStatusBar::new(ShellBarConfig::default());
    bar.mark_clean();
    bar.update_notification_count(42);
    assert!(bar.is_dirty());
    let notif = bar.find_item("notifications").unwrap();
    if let StatusBarItemKind::NotificationIndicator { unread_count, .. } = &notif.kind {
        assert_eq!(*unread_count, 42);
    } else {
        panic!("expected NotificationIndicator kind");
    }
}

// ========== update_connection_quality ==========

#[test]
fn status_bar_update_connection_quality() {
    let mut bar = ShellStatusBar::new(ShellBarConfig::default());
    bar.mark_clean();
    bar.update_connection_quality(85, 15);
    assert!(bar.is_dirty());
    let conn = bar.find_item("connection").unwrap();
    if let StatusBarItemKind::ConnectionQuality {
        quality_percent,
        latency_ms,
    } = &conn.kind
    {
        assert_eq!(*quality_percent, 85);
        assert_eq!(*latency_ms, 15);
    } else {
        panic!("expected ConnectionQuality kind");
    }
}

// ========== set_dnd ==========

#[test]
fn status_bar_set_dnd() {
    let mut bar = ShellStatusBar::new(ShellBarConfig::default());
    bar.mark_clean();
    bar.set_dnd(true);
    assert!(bar.is_dirty());
    let notif = bar.find_item("notifications").unwrap();
    if let StatusBarItemKind::NotificationIndicator { dnd_active, .. } = &notif.kind {
        assert!(*dnd_active);
    } else {
        panic!("expected NotificationIndicator kind");
    }
}

// ========== is_dirty / mark_clean ==========

#[test]
fn status_bar_dirty_lifecycle() {
    let mut bar = ShellStatusBar::new(ShellBarConfig::default());
    // Starts dirty
    assert!(bar.is_dirty());
    bar.mark_clean();
    assert!(!bar.is_dirty());
    // All items should be marked cached after mark_clean
    for item in bar.items() {
        assert!(item.cached);
    }
    // An update makes it dirty again
    bar.update_clock(100);
    assert!(bar.is_dirty());
}

#[test]
fn status_bar_dirty_starts_true_mark_clean_then_update() {
    let mut bar = ShellStatusBar::new(ShellBarConfig::default());
    assert!(bar.is_dirty());
    bar.mark_clean();
    assert!(!bar.is_dirty());
    bar.update_notification_count(1);
    assert!(bar.is_dirty());
}

// ========== items / item_count ==========

#[test]
fn status_bar_items_returns_all() {
    let bar = ShellStatusBar::new(ShellBarConfig::default());
    let items = bar.items();
    assert_eq!(items.len(), 5);
}

#[test]
fn status_bar_item_count() {
    let bar = ShellStatusBar::new(ShellBarConfig::default());
    assert_eq!(bar.item_count(), 5);
}

// ========== items_in_slot ==========

#[test]
fn status_bar_items_in_slot_left() {
    let bar = ShellStatusBar::new(ShellBarConfig::default());
    let left_items = bar.items_in_slot(StatusBarSlot::Left);
    assert!(left_items.is_empty());
}

#[test]
fn status_bar_items_in_slot_center() {
    let bar = ShellStatusBar::new(ShellBarConfig::default());
    let center_items = bar.items_in_slot(StatusBarSlot::Center);
    assert_eq!(center_items.len(), 1);
    assert_eq!(center_items[0].id, "clock");
}

#[test]
fn status_bar_items_in_slot_right() {
    let bar = ShellStatusBar::new(ShellBarConfig::default());
    let right_items = bar.items_in_slot(StatusBarSlot::Right);
    assert_eq!(right_items.len(), 4); // notifications, connection, tray, session
}

// ========== find_item ==========

#[test]
fn status_bar_find_item() {
    let bar = ShellStatusBar::new(ShellBarConfig::default());
    let clock = bar.find_item("clock");
    assert!(clock.is_some());
    assert_eq!(clock.unwrap().id, "clock");
}

#[test]
fn status_bar_find_item_nonexistent() {
    let bar = ShellStatusBar::new(ShellBarConfig::default());
    assert!(bar.find_item("nonexistent").is_none());
}

// ========== compute_bounds ==========

#[test]
fn status_bar_compute_bounds_top_position() {
    let bar = ShellStatusBar::new(ShellBarConfig {
        height: 34.0,
        ..ShellBarConfig::default()
    });
    let screen = Rect::new(0.0, 0.0, 1920.0, 1080.0);
    let bounds = bar.compute_bounds(screen);
    assert_eq!(bounds.x, 0.0);
    assert_eq!(bounds.y, 0.0);
    assert_eq!(bounds.width, 1920.0);
    assert_eq!(bounds.height, 34.0);
}

// ========== is_enabled ==========

#[test]
fn status_bar_is_enabled() {
    let bar = ShellStatusBar::new(ShellBarConfig::default());
    assert!(bar.is_enabled());

    let bar_disabled = ShellStatusBar::new(ShellBarConfig {
        enabled: false,
        ..ShellBarConfig::default()
    });
    assert!(!bar_disabled.is_enabled());
}

// ========== config accessor ==========

#[test]
fn status_bar_config_accessor() {
    let bar = ShellStatusBar::new(ShellBarConfig {
        height: 32.0,
        ..ShellBarConfig::default()
    });
    assert_eq!(bar.config().height, 32.0);
    assert!(bar.config().show_clock);
}

// ========== Display impls ==========

#[test]
fn status_bar_slot_display() {
    assert_eq!(format!("{}", StatusBarSlot::Left), "Left");
    assert_eq!(format!("{}", StatusBarSlot::Center), "Center");
    assert_eq!(format!("{}", StatusBarSlot::Right), "Right");
}

#[test]
fn status_bar_display() {
    let bar = ShellStatusBar::new(ShellBarConfig::default());
    let s = format!("{bar}");
    assert!(s.contains("5 items"));
    assert!(s.contains("dirty"));

    let mut bar2 = ShellStatusBar::new(ShellBarConfig::default());
    bar2.mark_clean();
    let s2 = format!("{bar2}");
    assert!(s2.contains("clean"));
}
