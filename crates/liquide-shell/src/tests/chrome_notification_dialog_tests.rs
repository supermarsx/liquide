//! Regressions for t51-e14: the canonical `liquide-notification-daemon` and
//! `liquide-dialogs` crates wired into the running shell.
//!
//! These assert real behavior (not field presence):
//!   * a posted notification is driven through the canonical daemon and drained
//!     / mirrored so the center can render it (fixes t49-e5-F03);
//!   * `OpenNotificationCenter` shows a LIVE center whose rendered DOM contains
//!     the current notifications — not a dead stub;
//!   * a dialog request routes through `liquide-dialogs`.

use liquide_input::mouse::{ButtonState, MouseButton, MouseEvent};
use liquide_interop::notification::{Notification, NotificationAction, Urgency};
use liquide_platform::event_loop::PlatformEvent;
use liquide_platform::window_host::NativeWindowHandle;

use crate::notification::ShellDialogKind;
use crate::shell::Shell;
use crate::shortcuts::ShellAction;

fn notif(app: &str, summary: &str) -> Notification {
    let mut n = Notification::new(app, summary);
    n.body = format!("{summary} body");
    n
}

fn mouse_click(x: f32, y: f32) -> PlatformEvent {
    PlatformEvent::MouseInput {
        handle: NativeWindowHandle(0),
        event: MouseEvent::Button {
            button: MouseButton::Left,
            state: ButtonState::Pressed,
            x,
            y,
        },
    }
}

/// A posted notification is routed through the canonical daemon (id assigned,
/// active set populated) and mirrored into the renderable manager.
#[test]
fn posted_notification_is_driven_through_the_daemon() {
    let mut shell = Shell::new(1920.0, 1080.0);
    assert_eq!(shell.daemon_active_count(), 0);
    assert_eq!(shell.notifications().active_count(), 0);

    let id = shell
        .post_notification(notif("mail", "New message"), 1_000_000)
        .expect("daemon should accept the notification");

    // Canonical daemon assigned a real id and tracks it active.
    assert!(id > 0, "daemon assigned a non-zero id");
    assert_eq!(shell.daemon_active_count(), 1);
    // Mirror is populated for rendering.
    assert_eq!(shell.notifications().active_count(), 1);
    assert_eq!(
        shell.notifications().active_notifications()[0]
            .notification
            .summary,
        "New message"
    );
}

/// Lifecycle events that the old `tick_detailed` discarded (F03) are now
/// drained and returned, and an action invoked through the daemon surfaces an
/// `ActionInvoked` event instead of vanishing.
#[test]
fn notification_action_events_are_drained_not_dropped() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let mut n = notif("chat", "Friend request");
    n.actions.push(NotificationAction::new("accept", "Accept"));
    let id = shell.post_notification(n, 1_000_000).expect("posted");

    // Invoking an action returns the event (previously dropped on tick).
    let events = shell.invoke_notification_action(id, "accept", 2_000_000);
    let saw_action = events.iter().any(|e| {
        matches!(
            e,
            crate::notification::NotificationEvent::ActionInvoked { action_id, .. }
                if action_id == "accept"
        )
    });
    assert!(
        saw_action,
        "action invocation surfaced an event: {events:?}"
    );
}

/// Opening the notification center renders a LIVE panel into the DOM whose
/// content is the current notifications — not a dead, empty stub. This is the
/// direct F03 fix.
#[test]
fn open_notification_center_shows_live_notifications() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell
        .post_notification(notif("mail", "Quarterly report"), 1_000_000)
        .expect("posted");
    shell
        .post_notification(notif("calendar", "Standup at 10"), 1_100_000)
        .expect("posted");

    // Center starts closed: no panel in the DOM.
    shell.sync_dom();
    assert!(
        shell
            .desktop_dom
            .doc
            .get_element_by_id("notification-center")
            .is_none(),
        "center must be absent while closed"
    );

    // Open it (the OpenNotificationCenter target) and re-sync.
    assert!(shell.open_notification_center());
    assert!(shell.notification_center_open());
    shell.sync_dom();

    let center = shell
        .desktop_dom
        .doc
        .get_element_by_id("notification-center")
        .expect("live notification center rendered into the DOM");
    // The panel has children (header + list), not an empty dead stub.
    assert!(
        !shell.desktop_dom.doc.children(center).is_empty(),
        "center is populated, not a dead stub"
    );
    // Each posted notification appears as a center item.
    assert!(
        shell
            .desktop_dom
            .doc
            .get_element_by_id("notif-center-1")
            .is_some(),
        "first notification rendered in the center"
    );
    assert!(
        shell
            .desktop_dom
            .doc
            .get_element_by_id("notif-center-2")
            .is_some(),
        "second notification rendered in the center"
    );
}

/// Closing the center removes the panel from the DOM (no stale overlay).
#[test]
fn closing_notification_center_removes_the_panel() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell
        .post_notification(notif("mail", "Hello"), 1_000_000)
        .expect("posted");

    shell.open_notification_center();
    shell.sync_dom();
    assert!(
        shell
            .desktop_dom
            .doc
            .get_element_by_id("notification-center")
            .is_some()
    );

    // Toggle closed.
    assert!(!shell.toggle_notification_center());
    shell.sync_dom();
    assert!(
        shell
            .desktop_dom
            .doc
            .get_element_by_id("notification-center")
            .is_none(),
        "center panel removed from the DOM when closed"
    );
}

/// The status-bar notification indicator click opens the live center end to
/// end: it toggles the canonical center and the rendered DOM then shows the
/// posted notification (the full F03 path).
#[test]
fn indicator_click_opens_live_center_end_to_end() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell
        .post_notification(notif("mail", "Receipt"), 1_000_000)
        .expect("posted");
    assert!(!shell.notification_center_open());

    // Click the notification indicator hit-region (36..80 px from the right).
    let action = shell.handle_platform_event(&mouse_click(1920.0 - 58.0, 15.0));
    assert!(matches!(action, Some(ShellAction::OpenNotificationCenter)));
    assert!(shell.notification_center_open());

    shell.sync_dom();
    assert!(
        shell
            .desktop_dom
            .doc
            .get_element_by_id("notification-center")
            .is_some(),
        "indicator click rendered the live center"
    );
    assert!(
        shell
            .desktop_dom
            .doc
            .get_element_by_id("notif-center-1")
            .is_some(),
        "the posted notification shows in the clicked-open center"
    );
}

/// A critical notification rendered in the center carries the urgency class
/// derived from the daemon-mirrored notification (real content, not a stub).
#[test]
fn center_item_reflects_urgency_class() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let mut n = notif("system", "Disk almost full");
    n.urgency = Urgency::Critical;
    shell.post_notification(n, 1_000_000).expect("posted");

    shell.open_notification_center();
    shell.sync_dom();

    let item = shell
        .desktop_dom
        .doc
        .get_element_by_id("notif-center-1")
        .expect("center item");
    let node = shell.desktop_dom.doc.get(item).expect("item node");
    assert!(
        node.has_class("urgency-critical"),
        "critical urgency class propagated to the rendered center item"
    );
}

/// A shell dialog request routes through the canonical `liquide-dialogs` crate
/// and the open dialog is tracked in the e7 `chrome_active_dialog` field.
#[test]
fn dialog_request_routes_through_liquide_dialogs() {
    let mut shell = Shell::new(1920.0, 1080.0);
    assert!(!shell.has_active_dialog());

    let id = shell.request_message_dialog(ShellDialogKind::Confirm, "Quit?", "Discard changes?");
    // The canonical crate minted a real DialogId, tracked as the active dialog.
    assert!(shell.has_active_dialog());
    assert_eq!(shell.active_dialog(), Some(id));

    // An input dialog likewise routes through the canonical crate.
    let input_id = shell.request_input_dialog("Rename", "New name");
    assert_eq!(shell.active_dialog(), Some(input_id));
    assert_ne!(input_id, id, "distinct canonical dialog ids");

    // Dismissing clears the tracked dialog.
    shell.dismiss_active_dialog();
    assert!(!shell.has_active_dialog());
}

/// Rendered notification content is HTML-escaped (preserves the t50-e5 escaping
/// discipline through the new center builder; no injection via summary/body).
#[test]
fn center_escapes_untrusted_notification_text() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let mut n = Notification::new("evil", "<script>alert(1)</script>");
    n.body = "a & b < c".to_string();
    shell.post_notification(n, 1_000_000).expect("posted");

    shell.open_notification_center();
    shell.sync_dom();

    let item = shell
        .desktop_dom
        .doc
        .get_element_by_id("notif-center-1")
        .expect("item rendered despite hostile text");

    // Walk the item subtree: the hostile summary must round-trip as a literal
    // text node (the `&lt;script&gt;` we emitted decodes back to text), and
    // crucially NO element named `script` may exist anywhere under the item —
    // that would mean the escaping was bypassed and markup was injected.
    let mut stack = vec![item];
    let mut saw_script_element = false;
    let mut saw_literal_summary = false;
    while let Some(node_id) = stack.pop() {
        if let Some(node) = shell.desktop_dom.doc.get(node_id) {
            if node.tag_name() == "script" {
                saw_script_element = true;
            }
            if node
                .text_content()
                .is_some_and(|t| t.contains("<script>alert(1)</script>"))
            {
                saw_literal_summary = true;
            }
        }
        for &child in shell.desktop_dom.doc.children(node_id) {
            stack.push(child);
        }
    }

    assert!(
        !saw_script_element,
        "hostile summary must not inject a <script> element into the chrome DOM"
    );
    assert!(
        saw_literal_summary,
        "hostile summary rendered as escaped literal text"
    );
}
