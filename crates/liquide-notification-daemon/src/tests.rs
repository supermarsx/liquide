//! Tests for the notification daemon.

use crate::handler::NotificationHandler;
use crate::history::NotificationHistory;
use crate::queue::NotificationQueue;
use crate::rate_limiter::RateLimiter;
use crate::server::NotificationServer;
use crate::spec::*;
// ── Test handler ────────────────────────────────────────────────────────

/// A recording handler that tracks all calls for assertion.
struct RecordingHandler {
    notified: Vec<u32>,
    closed: Vec<(u32, CloseReason)>,
    actions: Vec<(u32, String)>,
}

impl RecordingHandler {
    fn new() -> Self {
        Self {
            notified: Vec::new(),
            closed: Vec::new(),
            actions: Vec::new(),
        }
    }
}

impl NotificationHandler for RecordingHandler {
    fn on_notify(&mut self, notification: &Notification) -> u32 {
        self.notified.push(notification.id);
        notification.id
    }

    fn on_close(&mut self, id: u32, reason: CloseReason) {
        self.closed.push((id, reason));
    }

    fn on_action_invoked(&mut self, id: u32, action_key: &str) {
        self.actions.push((id, action_key.to_string()));
    }
}

// We also need a Send-compatible handler for the server tests.
// RecordingHandler is already Send since it only contains owned data.

// ── Spec type tests ─────────────────────────────────────────────────────

#[test]
fn test_urgency_default() {
    assert_eq!(Urgency::default(), Urgency::Normal);
}

#[test]
fn test_urgency_priority_order() {
    assert!(Urgency::Low.priority() < Urgency::Normal.priority());
    assert!(Urgency::Normal.priority() < Urgency::Critical.priority());
}

#[test]
fn test_notification_builder() {
    let n = Notification::new("Test Summary")
        .with_app_name("test-app")
        .with_body("Test body text")
        .with_icon("dialog-information")
        .with_urgency(Urgency::Critical)
        .with_action("default", "OK")
        .with_action("cancel", "Cancel")
        .with_timeout(3000)
        .with_replaces_id(42);

    assert_eq!(n.summary, "Test Summary");
    assert_eq!(n.app_name, "test-app");
    assert_eq!(n.body, "Test body text");
    assert_eq!(n.icon, "dialog-information");
    assert_eq!(n.urgency(), Urgency::Critical);
    assert_eq!(n.actions.len(), 2);
    assert_eq!(n.actions[0], ("default".to_string(), "OK".to_string()));
    assert_eq!(n.expire_timeout, 3000);
    assert_eq!(n.replaces_id, 42);
}

#[test]
fn test_notification_default_urgency() {
    let n = Notification::new("Test");
    assert_eq!(n.urgency(), Urgency::Normal);
    assert_eq!(n.id, 0);
    assert_eq!(n.expire_timeout, -1);
}

#[test]
fn test_notification_hints_default() {
    let h = NotificationHints::default();
    assert!(h.urgency.is_none());
    assert!(h.category.is_none());
    assert!(!h.suppress_sound);
    assert!(!h.transient);
    assert!(!h.action_icons);
    assert!(!h.resident);
}

// ── Rate limiter tests ──────────────────────────────────────────────────

#[test]
fn test_rate_limiter_allows_under_limit() {
    let mut rl = RateLimiter::new(3);
    assert!(rl.check("app1", 1000));
    assert!(rl.check("app1", 1100));
    assert!(rl.check("app1", 1200));
    assert_eq!(rl.current_count("app1"), 3);
}

#[test]
fn test_rate_limiter_blocks_over_limit() {
    let mut rl = RateLimiter::new(2);
    assert!(rl.check("app1", 1000));
    assert!(rl.check("app1", 1100));
    assert!(!rl.check("app1", 1200)); // 3rd in <1s → blocked.
}

#[test]
fn test_rate_limiter_window_slides() {
    let mut rl = RateLimiter::new(2);
    assert!(rl.check("app1", 1000));
    assert!(rl.check("app1", 1500));
    assert!(!rl.check("app1", 1800)); // Still within window of first.
    // Advance past the first timestamp's window.
    assert!(rl.check("app1", 2001)); // 1000 is now >1s old, evicted.
}

#[test]
fn test_rate_limiter_per_app_isolation() {
    let mut rl = RateLimiter::new(1);
    assert!(rl.check("app1", 1000));
    assert!(!rl.check("app1", 1100)); // app1 blocked.
    assert!(rl.check("app2", 1100)); // app2 has its own window.
}

#[test]
fn test_rate_limiter_set_limit() {
    let mut rl = RateLimiter::new(1);
    assert!(rl.check("app1", 1000));
    assert!(!rl.check("app1", 1100));
    rl.set_limit(5);
    assert_eq!(rl.limit(), 5);
    assert!(rl.check("app1", 1200)); // Now allowed, higher limit.
}

#[test]
fn test_rate_limiter_reset() {
    let mut rl = RateLimiter::new(1);
    assert!(rl.check("app1", 1000));
    assert!(!rl.check("app1", 1100));
    rl.reset();
    assert!(rl.check("app1", 1200)); // Reset clears all state.
}

#[test]
fn test_rate_limiter_reset_app() {
    let mut rl = RateLimiter::new(1);
    assert!(rl.check("app1", 1000));
    assert!(rl.check("app2", 1000));
    rl.reset_app("app1");
    assert!(rl.check("app1", 1100)); // app1 reset.
    assert!(!rl.check("app2", 1100)); // app2 still limited.
}

// ── Queue tests ─────────────────────────────────────────────────────────

fn reset_ids() {
    crate::queue::reset_id_counter();
}

#[test]
fn test_queue_enqueue_dequeue() {
    reset_ids();
    let mut q = NotificationQueue::new();
    let n = Notification::new("Hello");
    let id = q.enqueue_at(n, 1000).unwrap();
    assert!(id > 0);
    assert_eq!(q.pending_count(), 1);

    let dequeued = q.dequeue().unwrap();
    assert_eq!(dequeued.id, id);
    assert_eq!(q.pending_count(), 0);
}

#[test]
fn test_queue_priority_order() {
    reset_ids();
    let mut q = NotificationQueue::new();

    let low = Notification::new("Low").with_urgency(Urgency::Low);
    let normal = Notification::new("Normal"); // Default = Normal.
    let critical = Notification::new("Critical").with_urgency(Urgency::Critical);

    // Enqueue in arbitrary order.
    q.enqueue_at(low, 1000);
    q.enqueue_at(normal, 1001);
    q.enqueue_at(critical, 1002);

    assert_eq!(q.pending_count(), 3);

    // Should dequeue Critical, then Normal, then Low.
    assert_eq!(q.dequeue().unwrap().summary, "Critical");
    assert_eq!(q.dequeue().unwrap().summary, "Normal");
    assert_eq!(q.dequeue().unwrap().summary, "Low");
    assert!(q.dequeue().is_none());
}

#[test]
fn test_queue_peek() {
    reset_ids();
    let mut q = NotificationQueue::new();
    assert!(q.peek().is_none());

    q.enqueue_at(Notification::new("First"), 1000);
    assert_eq!(q.peek().unwrap().summary, "First");
    assert_eq!(q.pending_count(), 1); // Peek doesn't remove.
}

#[test]
fn test_queue_remove() {
    reset_ids();
    let mut q = NotificationQueue::new();
    let id = q.enqueue_at(Notification::new("Remove me"), 1000).unwrap();

    assert_eq!(q.pending_count(), 1);
    let removed = q.remove(id).unwrap();
    assert_eq!(removed.summary, "Remove me");
    assert_eq!(q.pending_count(), 0);
    assert!(q.remove(id).is_none()); // Already removed.
}

#[test]
fn test_queue_by_urgency() {
    reset_ids();
    let mut q = NotificationQueue::new();
    q.enqueue_at(
        Notification::new("N1"),
        1000,
    );
    q.enqueue_at(
        Notification::new("N2"),
        1001,
    );
    q.enqueue_at(
        Notification::new("C1").with_urgency(Urgency::Critical),
        1002,
    );
    q.enqueue_at(
        Notification::new("L1").with_urgency(Urgency::Low),
        1003,
    );

    let normals = q.by_urgency(Urgency::Normal);
    assert_eq!(normals.len(), 2);
    let criticals = q.by_urgency(Urgency::Critical);
    assert_eq!(criticals.len(), 1);
    let lows = q.by_urgency(Urgency::Low);
    assert_eq!(lows.len(), 1);
}

#[test]
fn test_queue_rate_limiting() {
    reset_ids();
    let mut q = NotificationQueue::with_rate_limit(2);
    let n1 = Notification::new("A").with_app_name("app1");
    let n2 = Notification::new("B").with_app_name("app1");
    let n3 = Notification::new("C").with_app_name("app1");

    assert!(q.enqueue_at(n1, 1000).is_some());
    assert!(q.enqueue_at(n2, 1100).is_some());
    assert!(q.enqueue_at(n3, 1200).is_none()); // Rate limited.
    assert_eq!(q.pending_count(), 2);
}

#[test]
fn test_queue_critical_bypasses_rate_limit() {
    reset_ids();
    let mut q = NotificationQueue::with_rate_limit(1);
    let n1 = Notification::new("A").with_app_name("app1");
    let n2 = Notification::new("B")
        .with_app_name("app1")
        .with_urgency(Urgency::Critical);

    assert!(q.enqueue_at(n1, 1000).is_some());
    // Critical should bypass the rate limit.
    assert!(q.enqueue_at(n2, 1100).is_some());
    assert_eq!(q.pending_count(), 2);
}

#[test]
fn test_queue_replaces_existing() {
    reset_ids();
    let mut q = NotificationQueue::new();
    let id1 = q.enqueue_at(Notification::new("Original"), 1000).unwrap();

    let replacement = Notification::new("Replacement").with_replaces_id(id1);
    let id2 = q.enqueue_at(replacement, 1001).unwrap();

    assert_ne!(id1, id2);
    assert_eq!(q.pending_count(), 1); // Original was removed.
    assert_eq!(q.dequeue().unwrap().summary, "Replacement");
}

// ── History tests ───────────────────────────────────────────────────────

#[test]
fn test_history_record_and_recent() {
    let mut h = NotificationHistory::new(100);
    let n1 = Notification::new("First").with_app_name("app1");
    let n2 = Notification::new("Second").with_app_name("app2");

    h.record(&n1, CloseReason::Expired, 1000, 2000);
    h.record(&n2, CloseReason::Dismissed, 2000, 3000);

    let recent = h.recent(10);
    assert_eq!(recent.len(), 2);
    assert_eq!(recent[0].notification.summary, "Second"); // Newest first.
    assert_eq!(recent[1].notification.summary, "First");
}

#[test]
fn test_history_recent_limit() {
    let mut h = NotificationHistory::new(100);
    for i in 0..10 {
        let n = Notification::new(format!("N{}", i));
        h.record(&n, CloseReason::Expired, i * 100, i * 100 + 50);
    }

    let recent = h.recent(3);
    assert_eq!(recent.len(), 3);
    assert_eq!(recent[0].notification.summary, "N9");
    assert_eq!(recent[1].notification.summary, "N8");
    assert_eq!(recent[2].notification.summary, "N7");
}

#[test]
fn test_history_by_app() {
    let mut h = NotificationHistory::new(100);
    h.record(
        &Notification::new("A1").with_app_name("app-a"),
        CloseReason::Expired,
        100,
        200,
    );
    h.record(
        &Notification::new("B1").with_app_name("app-b"),
        CloseReason::Dismissed,
        200,
        300,
    );
    h.record(
        &Notification::new("A2").with_app_name("app-a"),
        CloseReason::Closed,
        300,
        400,
    );

    let app_a = h.by_app("app-a");
    assert_eq!(app_a.len(), 2);
    assert_eq!(app_a[0].notification.summary, "A2"); // Newest first.
    assert_eq!(app_a[1].notification.summary, "A1");

    let app_b = h.by_app("app-b");
    assert_eq!(app_b.len(), 1);
}

#[test]
fn test_history_capacity_eviction() {
    let mut h = NotificationHistory::new(3);
    for i in 0..5 {
        let n = Notification::new(format!("N{}", i));
        h.record(&n, CloseReason::Expired, i * 100, i * 100 + 50);
    }

    assert_eq!(h.len(), 3);
    let entries = h.recent(10);
    assert_eq!(entries[0].notification.summary, "N4");
    assert_eq!(entries[2].notification.summary, "N2");
}

#[test]
fn test_history_transient_skipped() {
    let mut h = NotificationHistory::new(100);
    let mut n = Notification::new("Transient");
    n.hints.transient = true;

    h.record(&n, CloseReason::Expired, 100, 200);
    assert!(h.is_empty());
}

#[test]
fn test_history_clear() {
    let mut h = NotificationHistory::new(100);
    h.record(
        &Notification::new("Test"),
        CloseReason::Expired,
        100,
        200,
    );
    assert_eq!(h.len(), 1);
    h.clear();
    assert!(h.is_empty());
}

// ── Server tests ────────────────────────────────────────────────────────

#[test]
fn test_server_register_handler() {
    let mut server = NotificationServer::new();
    assert!(!server.has_handler());
    server.register_handler(Box::new(RecordingHandler::new()));
    assert!(server.has_handler());
}

#[test]
fn test_server_notify_dispatches() {
    reset_ids();
    let mut server = NotificationServer::new();
    server.register_handler(Box::new(RecordingHandler::new()));

    let id = server.notify_at(Notification::new("Hello"), 1000);
    assert!(id > 0);
    assert_eq!(server.active_count(), 1);
    assert_eq!(server.pending_count(), 0);
}

#[test]
fn test_server_notify_without_handler_queues() {
    reset_ids();
    let mut server = NotificationServer::new();

    let id = server.notify_at(Notification::new("Queued"), 1000);
    assert!(id > 0);
    assert_eq!(server.active_count(), 0);
    assert_eq!(server.pending_count(), 1);

    // Register handler — pending should drain on next notify or tick.
    server.register_handler(Box::new(RecordingHandler::new()));
    server.tick(1001);
    assert_eq!(server.active_count(), 1);
    assert_eq!(server.pending_count(), 0);
}

#[test]
fn test_server_close_notification() {
    reset_ids();
    let mut server = NotificationServer::new();
    server.register_handler(Box::new(RecordingHandler::new()));

    let id = server.notify_at(Notification::new("Will close"), 1000);
    assert_eq!(server.active_count(), 1);

    server.close_notification_at(id, CloseReason::Dismissed, 2000);
    assert_eq!(server.active_count(), 0);
    assert_eq!(server.history().len(), 1);
}

#[test]
fn test_server_tick_expires_notifications() {
    reset_ids();
    let mut server = NotificationServer::new();
    server.register_handler(Box::new(RecordingHandler::new()));
    server.set_default_timeout(1000);

    server.notify_at(Notification::new("Expires soon"), 1000);
    assert_eq!(server.active_count(), 1);

    // Not yet expired at 1500ms.
    server.tick(1500);
    assert_eq!(server.active_count(), 1);

    // Expired at 2001ms (1000ms timeout elapsed).
    server.tick(2001);
    assert_eq!(server.active_count(), 0);
    assert_eq!(server.history().len(), 1);
    assert_eq!(
        server.history().entries()[0].close_reason,
        CloseReason::Expired
    );
}

#[test]
fn test_server_critical_never_auto_expires() {
    reset_ids();
    let mut server = NotificationServer::new();
    server.register_handler(Box::new(RecordingHandler::new()));
    server.set_default_timeout(100);

    server.notify_at(
        Notification::new("Critical").with_urgency(Urgency::Critical),
        1000,
    );
    assert_eq!(server.active_count(), 1);

    // Way past timeout — should still be active.
    server.tick(100_000);
    assert_eq!(server.active_count(), 1);
}

#[test]
fn test_server_never_expire_timeout_zero() {
    reset_ids();
    let mut server = NotificationServer::new();
    server.register_handler(Box::new(RecordingHandler::new()));
    server.set_default_timeout(100);

    server.notify_at(
        Notification::new("Persistent").with_timeout(0),
        1000,
    );

    server.tick(100_000);
    assert_eq!(server.active_count(), 1); // Never expires.
}

#[test]
fn test_server_invoke_action() {
    reset_ids();
    let mut server = NotificationServer::new();
    server.register_handler(Box::new(RecordingHandler::new()));

    let id = server.notify_at(
        Notification::new("With action")
            .with_action("open", "Open")
            .with_action("dismiss", "Dismiss"),
        1000,
    );

    // Invoking a valid action should close the notification (non-resident).
    server.invoke_action(id, "open");
    assert_eq!(server.active_count(), 0);
}

#[test]
fn test_server_invoke_action_resident() {
    reset_ids();
    let mut server = NotificationServer::new();
    server.register_handler(Box::new(RecordingHandler::new()));

    let mut n = Notification::new("Resident action")
        .with_action("play", "Play");
    n.hints.resident = true;

    let id = server.notify_at(n, 1000);

    // Resident notification stays after action.
    server.invoke_action(id, "play");
    assert_eq!(server.active_count(), 1);
}

#[test]
fn test_server_invoke_invalid_action() {
    reset_ids();
    let mut server = NotificationServer::new();
    server.register_handler(Box::new(RecordingHandler::new()));

    let id = server.notify_at(
        Notification::new("Test").with_action("ok", "OK"),
        1000,
    );

    // Invoking a non-existent action should do nothing.
    server.invoke_action(id, "nonexistent");
    assert_eq!(server.active_count(), 1);
}

#[test]
fn test_server_capabilities() {
    let server = NotificationServer::new();
    let caps = server.get_capabilities();
    assert!(caps.contains(&"body".to_string()));
    assert!(caps.contains(&"actions".to_string()));
    assert!(caps.contains(&"icon-static".to_string()));
    assert!(caps.contains(&"persistence".to_string()));
}

#[test]
fn test_server_info() {
    let server = NotificationServer::new();
    let info = server.get_server_info();
    assert_eq!(info.name, "LiquiDE Notification Daemon");
    assert_eq!(info.vendor, "LiquiDE");
    assert_eq!(info.spec_version, "1.2");
}

#[test]
fn test_server_rate_limits() {
    reset_ids();
    let mut server = NotificationServer::new();
    server.register_handler(Box::new(RecordingHandler::new()));
    server.queue_mut().rate_limiter_mut().set_limit(1);

    let id1 = server.notify_at(
        Notification::new("First").with_app_name("app1"),
        1000,
    );
    let id2 = server.notify_at(
        Notification::new("Second").with_app_name("app1"),
        1100,
    );

    assert!(id1 > 0);
    assert_eq!(id2, 0); // Rate limited.
    assert_eq!(server.active_count(), 1);
}

#[test]
fn test_server_get_active() {
    reset_ids();
    let mut server = NotificationServer::new();
    server.register_handler(Box::new(RecordingHandler::new()));

    let id = server.notify_at(Notification::new("Lookup test"), 1000);
    let active = server.get_active(id).unwrap();
    assert_eq!(active.summary, "Lookup test");
    assert!(server.get_active(99999).is_none());
}

// ── Platform tests ──────────────────────────────────────────────────────

#[test]
fn test_platform_error_display() {
    use crate::platform::PlatformError;

    let e = PlatformError::ToolNotFound("gdbus".to_string());
    assert_eq!(format!("{}", e), "platform tool not found: gdbus");

    let e = PlatformError::CommandFailed {
        tool: "powershell".to_string(),
        stderr: "access denied".to_string(),
    };
    assert_eq!(format!("{}", e), "powershell failed: access denied");

    let e = PlatformError::Unsupported;
    assert_eq!(format!("{}", e), "platform not supported");
}

// ── Grouping tests ──────────────────────────────────────────────────────

use crate::grouping::{
    collapse_group, expand_group, group_notifications, GroupableNotification, NotificationGroup,
};

#[test]
fn test_group_by_app() {
    let notifs = vec![
        GroupableNotification { id: 1, app_id: "chat".into() },
        GroupableNotification { id: 2, app_id: "email".into() },
        GroupableNotification { id: 3, app_id: "chat".into() },
        GroupableNotification { id: 4, app_id: "chat".into() },
        GroupableNotification { id: 5, app_id: "email".into() },
    ];
    let groups = group_notifications(&notifs);

    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].app_id, "chat");
    assert_eq!(groups[0].notifications, vec![1, 3, 4]);
    assert_eq!(groups[1].app_id, "email");
    assert_eq!(groups[1].notifications, vec![2, 5]);
}

#[test]
fn test_group_single_app() {
    let notifs = vec![
        GroupableNotification { id: 10, app_id: "browser".into() },
        GroupableNotification { id: 20, app_id: "browser".into() },
    ];
    let groups = group_notifications(&notifs);

    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].app_id, "browser");
    assert_eq!(groups[0].len(), 2);
}

#[test]
fn test_group_empty() {
    let groups = group_notifications(&[]);
    assert!(groups.is_empty());
}

#[test]
fn test_collapse_expand_group() {
    let notifs = vec![
        GroupableNotification { id: 1, app_id: "chat".into() },
        GroupableNotification { id: 2, app_id: "chat".into() },
        GroupableNotification { id: 3, app_id: "chat".into() },
    ];
    let mut groups = group_notifications(&notifs);
    let group = &mut groups[0];

    assert!(!group.collapsed);
    collapse_group(group);
    assert!(group.collapsed);
    assert_eq!(group.summary_count, 3);

    expand_group(group);
    assert!(!group.collapsed);
    // All notifications still accessible after expand.
    assert_eq!(group.notifications.len(), 3);
}

#[test]
fn test_group_latest() {
    let mut group = NotificationGroup::new("app");
    assert!(group.latest().is_none());

    group.add(100);
    group.add(200);
    group.add(300);
    assert_eq!(group.latest(), Some(300));
}

#[test]
fn test_group_add_remove() {
    let mut group = NotificationGroup::new("test-app");
    assert!(group.is_empty());

    group.add(1);
    group.add(2);
    group.add(3);
    assert_eq!(group.len(), 3);
    assert_eq!(group.summary_count, 3);

    assert!(group.remove(2));
    assert_eq!(group.len(), 2);
    assert_eq!(group.notifications, vec![1, 3]);
    assert_eq!(group.summary_count, 2);

    // Removing non-existent ID returns false.
    assert!(!group.remove(999));
    assert_eq!(group.len(), 2);
}

#[test]
fn test_group_preserves_order() {
    let notifs = vec![
        GroupableNotification { id: 5, app_id: "a".into() },
        GroupableNotification { id: 3, app_id: "b".into() },
        GroupableNotification { id: 1, app_id: "a".into() },
        GroupableNotification { id: 4, app_id: "c".into() },
        GroupableNotification { id: 2, app_id: "b".into() },
    ];
    let groups = group_notifications(&notifs);

    // Groups should appear in order of first occurrence.
    assert_eq!(groups[0].app_id, "a");
    assert_eq!(groups[1].app_id, "b");
    assert_eq!(groups[2].app_id, "c");

    // Within each group, insertion order preserved.
    assert_eq!(groups[0].notifications, vec![5, 1]);
    assert_eq!(groups[1].notifications, vec![3, 2]);
}

// ── Notification log tests ──────────────────────────────────────────────

use crate::log::{LogAction, NotificationLog};

#[test]
fn test_log_record_and_query() {
    let mut log = NotificationLog::new(100);
    log.record_event(1, "chat", "Message", "Hello", 1000, LogAction::Shown);
    log.record_event(2, "email", "New mail", "From Bob", 2000, LogAction::Shown);
    log.record_event(1, "chat", "Message", "Hello", 3000, LogAction::Clicked);

    assert_eq!(log.len(), 3);
    assert!(!log.is_empty());
}

#[test]
fn test_log_entries_for_app() {
    let mut log = NotificationLog::new(100);
    log.record_event(1, "chat", "Msg1", "", 1000, LogAction::Shown);
    log.record_event(2, "email", "Mail1", "", 2000, LogAction::Shown);
    log.record_event(3, "chat", "Msg2", "", 3000, LogAction::Dismissed);

    let chat_entries = log.entries_for_app("chat");
    assert_eq!(chat_entries.len(), 2);
    assert_eq!(chat_entries[0].notification_id, 1);
    assert_eq!(chat_entries[1].notification_id, 3);

    let email_entries = log.entries_for_app("email");
    assert_eq!(email_entries.len(), 1);
}

#[test]
fn test_log_entries_since() {
    let mut log = NotificationLog::new(100);
    log.record_event(1, "a", "S1", "", 1000, LogAction::Shown);
    log.record_event(2, "a", "S2", "", 2000, LogAction::Shown);
    log.record_event(3, "a", "S3", "", 3000, LogAction::Expired);
    log.record_event(4, "a", "S4", "", 4000, LogAction::Dismissed);

    let since_2500 = log.entries_since(2500);
    assert_eq!(since_2500.len(), 2);
    assert_eq!(since_2500[0].notification_id, 3);
    assert_eq!(since_2500[1].notification_id, 4);

    let since_5000 = log.entries_since(5000);
    assert!(since_5000.is_empty());
}

#[test]
fn test_log_clear() {
    let mut log = NotificationLog::new(100);
    log.record_event(1, "a", "S", "", 1000, LogAction::Shown);
    log.record_event(2, "a", "S", "", 2000, LogAction::Shown);
    assert_eq!(log.len(), 2);

    log.clear();
    assert!(log.is_empty());
}

#[test]
fn test_log_clear_for_app() {
    let mut log = NotificationLog::new(100);
    log.record_event(1, "chat", "M1", "", 1000, LogAction::Shown);
    log.record_event(2, "email", "M2", "", 2000, LogAction::Shown);
    log.record_event(3, "chat", "M3", "", 3000, LogAction::Dismissed);

    log.clear_for_app("chat");
    assert_eq!(log.len(), 1);
    assert_eq!(log.all_entries()[0].app_id, "email");
}

#[test]
fn test_log_capacity_eviction() {
    let mut log = NotificationLog::new(3);
    for i in 0..5 {
        log.record_event(i, "app", &format!("S{}", i), "", i * 100, LogAction::Shown);
    }

    assert_eq!(log.len(), 3);
    assert_eq!(log.all_entries()[0].notification_id, 2);
    assert_eq!(log.all_entries()[2].notification_id, 4);
}

#[test]
fn test_log_entries_by_action() {
    let mut log = NotificationLog::new(100);
    log.record_event(1, "a", "S", "", 1000, LogAction::Shown);
    log.record_event(2, "a", "S", "", 2000, LogAction::Clicked);
    log.record_event(3, "a", "S", "", 3000, LogAction::Shown);
    log.record_event(4, "a", "S", "", 4000, LogAction::ActionInvoked("open".into()));

    let shown = log.entries_by_action(&LogAction::Shown);
    assert_eq!(shown.len(), 2);

    let clicked = log.entries_by_action(&LogAction::Clicked);
    assert_eq!(clicked.len(), 1);

    let actions = log.entries_by_action(&LogAction::ActionInvoked("open".into()));
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].notification_id, 4);
}

#[test]
fn test_log_action_invoked_distinct() {
    // ActionInvoked with different keys should not match each other.
    let mut log = NotificationLog::new(100);
    log.record_event(1, "a", "S", "", 1000, LogAction::ActionInvoked("open".into()));
    log.record_event(2, "a", "S", "", 2000, LogAction::ActionInvoked("close".into()));

    let open = log.entries_by_action(&LogAction::ActionInvoked("open".into()));
    assert_eq!(open.len(), 1);
    let close = log.entries_by_action(&LogAction::ActionInvoked("close".into()));
    assert_eq!(close.len(), 1);
}

#[test]
fn test_log_default_capacity() {
    let log = NotificationLog::default();
    // Default should have a reasonable capacity (5000).
    assert!(log.is_empty());
    assert_eq!(log.max_entries, 5000);
}

// ── DND schedule tests ──────────────────────────────────────────────────

use crate::dnd::{crosses_midnight, DndSchedule, DndTimeRange};

#[test]
fn test_dnd_disabled_by_default() {
    let schedule = DndSchedule::new();
    assert!(!schedule.is_active(22, 0, 1)); // Monday 22:00 — not active.
}

#[test]
fn test_dnd_basic_range() {
    let mut schedule = DndSchedule::new();
    schedule.enable();
    schedule.add_schedule(DndTimeRange::new(22, 0, 7, 0)); // 22:00–07:00

    // Inside evening portion.
    assert!(schedule.is_active(23, 30, 3)); // Wednesday 23:30
    assert!(schedule.is_active(22, 0, 0));  // Sunday 22:00 (start is inclusive)

    // Inside morning portion (next day).
    assert!(schedule.is_active(3, 0, 4));  // Thursday 03:00 (started Wed evening)

    // Outside range.
    assert!(!schedule.is_active(12, 0, 3)); // Wednesday noon
    assert!(!schedule.is_active(8, 0, 3));  // Wednesday 8am
}

#[test]
fn test_dnd_non_crossing_range() {
    let mut schedule = DndSchedule::new();
    schedule.enable();
    schedule.add_schedule(DndTimeRange::new(9, 0, 17, 0)); // 9:00–17:00

    assert!(schedule.is_active(10, 0, 1)); // Monday 10am
    assert!(schedule.is_active(9, 0, 5));  // Friday 9am (start is inclusive)
    assert!(!schedule.is_active(17, 0, 1)); // Monday 5pm (end is exclusive)
    assert!(!schedule.is_active(8, 59, 1)); // Monday 8:59am
    assert!(!schedule.is_active(20, 0, 1)); // Monday 8pm
}

#[test]
fn test_dnd_crosses_midnight() {
    let range = DndTimeRange::new(22, 0, 7, 0);
    assert!(crosses_midnight(&range));

    let range2 = DndTimeRange::new(9, 0, 17, 0);
    assert!(!crosses_midnight(&range2));

    // Same start and end = crosses midnight (treated as "full day" in duration sense).
    let range3 = DndTimeRange::new(10, 0, 10, 0);
    assert!(crosses_midnight(&range3));
}

#[test]
fn test_dnd_weekday_filter() {
    let mut schedule = DndSchedule::new();
    schedule.enable();
    // Only on weekdays (Mon=1 through Fri=5).
    schedule.add_schedule(
        DndTimeRange::new(22, 0, 7, 0).with_days(vec![1, 2, 3, 4, 5]),
    );

    // Monday evening → should be active.
    assert!(schedule.is_active(23, 0, 1));
    // Tuesday morning (started Monday night) → check "yesterday" = Monday = in days list.
    assert!(schedule.is_active(3, 0, 2));
    // Saturday evening → not in schedule.
    assert!(!schedule.is_active(23, 0, 6));
    // Sunday morning (started Saturday night) → yesterday=Saturday=6, not in list.
    assert!(!schedule.is_active(3, 0, 0));
}

#[test]
fn test_dnd_manual_override() {
    let mut schedule = DndSchedule::new();
    // No schedule, not enabled.
    assert!(!schedule.is_active(12, 0, 3));

    // Manual override forces DND on regardless.
    schedule.set_manual_override(true);
    assert!(schedule.is_active(12, 0, 3));
    assert!(schedule.is_active(0, 0, 0)); // Any time.

    schedule.set_manual_override(false);
    assert!(!schedule.is_active(12, 0, 3));
}

#[test]
fn test_dnd_add_remove_schedule() {
    let mut schedule = DndSchedule::new();
    schedule.enable();

    schedule.add_schedule(DndTimeRange::new(22, 0, 7, 0));
    schedule.add_schedule(DndTimeRange::new(12, 0, 13, 0));
    assert_eq!(schedule.schedule_count(), 2);

    let removed = schedule.remove_schedule(0);
    assert!(removed.is_some());
    assert_eq!(removed.unwrap().start_hour, 22);
    assert_eq!(schedule.schedule_count(), 1);

    // Out of bounds returns None.
    assert!(schedule.remove_schedule(99).is_none());
}

#[test]
fn test_dnd_multiple_ranges() {
    let mut schedule = DndSchedule::new();
    schedule.enable();
    schedule.add_schedule(DndTimeRange::new(22, 0, 7, 0));  // Night
    schedule.add_schedule(DndTimeRange::new(12, 0, 13, 0)); // Lunch

    assert!(schedule.is_active(23, 0, 1));  // Night range
    assert!(schedule.is_active(12, 30, 1)); // Lunch range
    assert!(!schedule.is_active(10, 0, 1)); // Neither
}

#[test]
fn test_dnd_enable_disable() {
    let mut schedule = DndSchedule::new();
    schedule.add_schedule(DndTimeRange::new(22, 0, 7, 0));

    // Disabled by default — should not be active.
    assert!(!schedule.is_active(23, 0, 1));

    schedule.enable();
    assert!(schedule.is_active(23, 0, 1));

    schedule.disable();
    assert!(!schedule.is_active(23, 0, 1));
}

#[test]
fn test_dnd_weekend_only() {
    let mut schedule = DndSchedule::new();
    schedule.enable();
    // Weekend nights: Saturday(6) and Sunday(0) evening.
    schedule.add_schedule(
        DndTimeRange::new(23, 0, 10, 0).with_days(vec![0, 6]),
    );

    // Saturday 23:30 → active (day=6 is in list).
    assert!(schedule.is_active(23, 30, 6));
    // Sunday 8:00 → morning portion, yesterday=Saturday=6, in list.
    assert!(schedule.is_active(8, 0, 0));
    // Monday 8:00 → morning portion, yesterday=Sunday=0, in list.
    assert!(schedule.is_active(8, 0, 1));
    // Wednesday 23:30 → day=3, not in list.
    assert!(!schedule.is_active(23, 30, 3));
}

// ── Layout tests ────────────────────────────────────────────────────────

use crate::layout::{
    compute_positions, LayoutAnchor, NotificationInfo, NotificationLayout,
    Priority, Rect,
};

fn screen() -> Rect {
    Rect::new(0.0, 0.0, 1920.0, 1080.0)
}

fn make_infos(count: usize) -> Vec<NotificationInfo> {
    (0..count)
        .map(|i| NotificationInfo {
            id: i as u64 + 1,
            width: 300.0,
            height: 80.0,
            priority: Priority::Normal,
        })
        .collect()
}

#[test]
fn test_layout_top_right() {
    let infos = make_infos(3);
    let positions = compute_positions(&infos, screen(), LayoutAnchor::TopRight);

    assert_eq!(positions.len(), 3);
    // First notification at top-right corner.
    let p = &positions[0];
    assert!((p.x - (1920.0 - 12.0 - 300.0)).abs() < 0.01);
    assert!((p.y - 12.0).abs() < 0.01);

    // Second notification below the first with gap.
    let p2 = &positions[1];
    assert!((p2.y - (12.0 + 80.0 + 8.0)).abs() < 0.01);
}

#[test]
fn test_layout_top_left() {
    let infos = make_infos(2);
    let positions = compute_positions(&infos, screen(), LayoutAnchor::TopLeft);

    assert_eq!(positions.len(), 2);
    let p = &positions[0];
    assert!((p.x - 12.0).abs() < 0.01); // Left margin.
    assert!((p.y - 12.0).abs() < 0.01);
}

#[test]
fn test_layout_bottom_right() {
    let infos = make_infos(2);
    let positions = compute_positions(&infos, screen(), LayoutAnchor::BottomRight);

    assert_eq!(positions.len(), 2);
    // First notification at bottom-right.
    let p = &positions[0];
    assert!((p.x - (1920.0 - 12.0 - 300.0)).abs() < 0.01);
    assert!((p.y - (1080.0 - 12.0 - 80.0)).abs() < 0.01);

    // Second notification above the first.
    let p2 = &positions[1];
    assert!((p2.y - (1080.0 - 12.0 - 80.0 - 8.0 - 80.0)).abs() < 0.01);
}

#[test]
fn test_layout_bottom_left() {
    let infos = make_infos(1);
    let positions = compute_positions(&infos, screen(), LayoutAnchor::BottomLeft);

    assert_eq!(positions.len(), 1);
    let p = &positions[0];
    assert!((p.x - 12.0).abs() < 0.01);
    assert!((p.y - (1080.0 - 12.0 - 80.0)).abs() < 0.01);
}

#[test]
fn test_layout_top_center() {
    let infos = make_infos(1);
    let positions = compute_positions(&infos, screen(), LayoutAnchor::TopCenter);

    assert_eq!(positions.len(), 1);
    let p = &positions[0];
    // Centered horizontally.
    let expected_x = (1920.0 - 300.0) / 2.0;
    assert!((p.x - expected_x).abs() < 0.01);
    assert!((p.y - 12.0).abs() < 0.01);
}

#[test]
fn test_layout_priority_ordering() {
    let infos = vec![
        NotificationInfo { id: 1, width: 300.0, height: 80.0, priority: Priority::Low },
        NotificationInfo { id: 2, width: 300.0, height: 80.0, priority: Priority::Urgent },
        NotificationInfo { id: 3, width: 300.0, height: 80.0, priority: Priority::Normal },
        NotificationInfo { id: 4, width: 300.0, height: 80.0, priority: Priority::High },
    ];

    let positions = compute_positions(&infos, screen(), LayoutAnchor::TopRight);
    assert_eq!(positions.len(), 4);

    // Urgent first (closest to anchor), then High, Normal, Low.
    assert_eq!(positions[0].id, 2); // Urgent
    assert_eq!(positions[1].id, 4); // High
    assert_eq!(positions[2].id, 3); // Normal
    assert_eq!(positions[3].id, 1); // Low
}

#[test]
fn test_layout_overflow_handling() {
    // Screen is 200px tall with 12px margin top/bottom → 176px usable.
    // Each notification is 80px + 8px gap = 88px. Fits 2 (80+8+80 = 168).
    let small_screen = Rect::new(0.0, 0.0, 400.0, 200.0);
    let infos = make_infos(5);

    let positions = compute_positions(&infos, small_screen, LayoutAnchor::TopRight);
    // Only 2 should fit: 12 + 80 = 92, 92 + 8 + 80 = 180 < 200-12=188. Third: 180+8+80=268 > 188.
    assert_eq!(positions.len(), 2);
}

#[test]
fn test_layout_empty() {
    let positions = compute_positions(&[], screen(), LayoutAnchor::TopRight);
    assert!(positions.is_empty());
}

#[test]
fn test_layout_custom_gap_margin() {
    let layout = NotificationLayout::new(16.0, 24.0);
    let infos = make_infos(2);
    let positions = layout.compute_positions(&infos, screen(), LayoutAnchor::TopRight);

    assert_eq!(positions.len(), 2);
    // First at margin 24.
    assert!((positions[0].y - 24.0).abs() < 0.01);
    // Second at 24 + 80 + 16 = 120.
    assert!((positions[1].y - 120.0).abs() < 0.01);
}

#[test]
fn test_layout_variable_heights() {
    let infos = vec![
        NotificationInfo { id: 1, width: 300.0, height: 60.0, priority: Priority::Normal },
        NotificationInfo { id: 2, width: 300.0, height: 100.0, priority: Priority::Normal },
        NotificationInfo { id: 3, width: 300.0, height: 40.0, priority: Priority::Normal },
    ];

    let positions = compute_positions(&infos, screen(), LayoutAnchor::TopRight);
    assert_eq!(positions.len(), 3);

    // y-positions: 12, 12+60+8=80, 80+100+8=188
    assert!((positions[0].y - 12.0).abs() < 0.01);
    assert!((positions[1].y - 80.0).abs() < 0.01);
    assert!((positions[2].y - 188.0).abs() < 0.01);

    assert!((positions[0].height - 60.0).abs() < 0.01);
    assert!((positions[1].height - 100.0).abs() < 0.01);
    assert!((positions[2].height - 40.0).abs() < 0.01);
}

#[test]
fn test_priority_ordering() {
    assert!(Priority::Low < Priority::Normal);
    assert!(Priority::Normal < Priority::High);
    assert!(Priority::High < Priority::Urgent);
    assert_eq!(Priority::default(), Priority::Normal);
}

// ── App settings tests ──────────────────────────────────────────────────

use crate::app_settings::{AppNotificationSettings, AppSettings};

#[test]
fn test_app_settings_defaults() {
    let settings = AppSettings::default();
    assert!(settings.enabled);
    assert!(settings.sound_enabled);
    assert!(settings.priority_override.is_none());
    assert!(!settings.bypass_dnd);
    assert!(settings.timeout_override.is_none());
}

#[test]
fn test_app_settings_get_default() {
    let registry = AppNotificationSettings::new();
    let s = registry.get("unknown-app");
    assert!(s.enabled);
    assert!(s.sound_enabled);
}

#[test]
fn test_app_settings_set_get() {
    let mut registry = AppNotificationSettings::new();
    let mut settings = AppSettings::default();
    settings.enabled = false;
    settings.priority_override = Some(Priority::High);

    registry.set("chat", settings);

    let s = registry.get("chat");
    assert!(!s.enabled);
    assert_eq!(s.priority_override, Some(Priority::High));
}

#[test]
fn test_app_settings_set_enabled() {
    let mut registry = AppNotificationSettings::new();
    registry.set_enabled("noisy-app", false);

    let s = registry.get("noisy-app");
    assert!(!s.enabled);
    // Other fields should still be default.
    assert!(s.sound_enabled);
}

#[test]
fn test_app_settings_set_sound() {
    let mut registry = AppNotificationSettings::new();
    registry.set_sound_enabled("music", false);

    assert!(!registry.get("music").sound_enabled);
}

#[test]
fn test_app_settings_bypass_dnd() {
    let mut registry = AppNotificationSettings::new();
    registry.set_bypass_dnd("alarm", true);

    assert!(registry.get("alarm").bypass_dnd);
    assert!(!registry.get("chat").bypass_dnd); // Default.
}

#[test]
fn test_app_settings_should_deliver() {
    let mut registry = AppNotificationSettings::new();
    registry.set_enabled("disabled-app", false);
    registry.set_bypass_dnd("alarm", true);

    // Normal app, no DND → deliver.
    assert!(registry.should_deliver("chat", false));
    // Normal app, DND active → don't deliver.
    assert!(!registry.should_deliver("chat", true));
    // Disabled app → never deliver.
    assert!(!registry.should_deliver("disabled-app", false));
    assert!(!registry.should_deliver("disabled-app", true));
    // Alarm bypasses DND.
    assert!(registry.should_deliver("alarm", true));
    assert!(registry.should_deliver("alarm", false));
}

#[test]
fn test_app_settings_remove() {
    let mut registry = AppNotificationSettings::new();
    registry.set_enabled("app", false);
    assert!(!registry.get("app").enabled);

    let removed = registry.remove("app");
    assert!(removed);
    assert!(registry.get("app").enabled); // Back to default.

    assert!(!registry.remove("nonexistent"));
}

#[test]
fn test_app_settings_configured_apps() {
    let mut registry = AppNotificationSettings::new();
    assert_eq!(registry.app_count(), 0);

    registry.set_enabled("a", false);
    registry.set_sound_enabled("b", false);
    registry.set_bypass_dnd("a", true);

    assert_eq!(registry.app_count(), 2);
    let apps = registry.configured_apps();
    assert!(apps.contains(&"a"));
    assert!(apps.contains(&"b"));
}

#[test]
fn test_app_settings_timeout_override() {
    let mut registry = AppNotificationSettings::new();
    registry.set_timeout_override("quick-app", Some(2000));

    assert_eq!(registry.get("quick-app").timeout_override, Some(2000));
    assert_eq!(registry.get("other").timeout_override, None);

    registry.set_timeout_override("quick-app", None);
    assert_eq!(registry.get("quick-app").timeout_override, None);
}

#[test]
fn test_app_settings_priority_override() {
    let mut registry = AppNotificationSettings::new();
    registry.set_priority_override("vip", Some(Priority::Urgent));
    assert_eq!(registry.get("vip").priority_override, Some(Priority::Urgent));

    registry.set_priority_override("vip", None);
    assert_eq!(registry.get("vip").priority_override, None);
}

#[test]
fn test_app_settings_explicit() {
    let mut registry = AppNotificationSettings::new();
    assert!(registry.get_explicit("unconfigured").is_none());

    registry.set_enabled("configured", false);
    assert!(registry.get_explicit("configured").is_some());
    assert!(!registry.get_explicit("configured").unwrap().enabled);
}
