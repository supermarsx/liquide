//! Notifications component — incremental rendering of notification toasts.
//!
//! **Fix**: the old `sync_dom` destroyed and recreated ALL notification DOM nodes
//! every frame.  This component uses keyed reconciliation so only changed
//! notifications are touched.

use crate::template::{Component, TemplateNode};
use crate::types::element_ids;

/// Minimal notification info for the component.
///
/// Decoupled from `ShellNotification` to avoid circular dependencies.
#[derive(Debug, Clone)]
pub struct NotificationInfo {
    pub id: u32,
    pub summary: String,
    pub body: String,
    pub urgency: NotificationUrgency,
    pub icon: String,
    pub actions: Vec<NotificationAction>,
}

/// Urgency level for notification styling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationUrgency {
    Low,
    Normal,
    Critical,
}

/// An action button on a notification.
#[derive(Debug, Clone)]
pub struct NotificationAction {
    pub id: String,
    pub label: String,
}

/// Notifications component that renders the notification area.
///
/// Produces a DOM tree like:
/// ```text
/// <notification-area id="notification-area">
///   <notification data-key="notif-1" class="urgency-normal">
///     <notification-icon data-icon="mail" />
///     <notification-content>
///       <notification-title>New Mail</notification-title>
///       <notification-body>You have 3 new messages</notification-body>
///     </notification-content>
///     <notification-actions>
///       <notification-action data-action-id="read">Read</notification-action>
///     </notification-actions>
///   </notification>
///   …
/// </notification-area>
/// ```
///
/// **Improvements over old sync_dom**:
/// - Keyed by notification ID → incremental updates, no full rebuild
/// - Urgency classes (`.urgency-low`, `.urgency-normal`, `.urgency-critical`)
/// - Icon sub-element
/// - Action buttons rendered
pub struct NotificationsComponent<'a> {
    pub notifications: &'a [NotificationInfo],
}

impl Component for NotificationsComponent<'_> {
    fn render(&self) -> TemplateNode {
        TemplateNode::el("notification-area")
            .id(element_ids::NOTIFICATION_AREA)
            .children(self.notifications.iter().map(|notif| {
                let key = format!("notif-{}", notif.id);
                let urgency_class = match notif.urgency {
                    NotificationUrgency::Low => "urgency-low",
                    NotificationUrgency::Normal => "urgency-normal",
                    NotificationUrgency::Critical => "urgency-critical",
                };

                let mut node = TemplateNode::el("notification")
                    .key(&key)
                    .class(urgency_class);

                // Icon (if present)
                if !notif.icon.is_empty() {
                    node = node.child(
                        TemplateNode::el("notification-icon").attr("data-icon", &notif.icon),
                    );
                }

                // Content (title + body)
                node = node.child(
                    TemplateNode::el("notification-content")
                        .child(
                            TemplateNode::el("notification-title")
                                .child(TemplateNode::text(&notif.summary)),
                        )
                        .child(
                            TemplateNode::el("notification-body")
                                .child(TemplateNode::text(&notif.body)),
                        ),
                );

                // Action buttons (if any)
                if !notif.actions.is_empty() {
                    node = node.child(TemplateNode::el("notification-actions").children(
                        notif.actions.iter().map(|action| {
                            TemplateNode::el("notification-action")
                                .key(&action.id)
                                .attr("data-action-id", &action.id)
                                .child(TemplateNode::text(&action.label))
                        }),
                    ));
                }

                node
            }))
    }

    fn mount_point(&self) -> &str {
        element_ids::NOTIFICATION_AREA
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_notifications() -> Vec<NotificationInfo> {
        vec![
            NotificationInfo {
                id: 1,
                summary: "New Mail".into(),
                body: "You have 3 new messages".into(),
                urgency: NotificationUrgency::Normal,
                icon: "mail".into(),
                actions: vec![
                    NotificationAction {
                        id: "read".into(),
                        label: "Read".into(),
                    },
                    NotificationAction {
                        id: "dismiss".into(),
                        label: "Dismiss".into(),
                    },
                ],
            },
            NotificationInfo {
                id: 2,
                summary: "Low Battery".into(),
                body: "10% remaining".into(),
                urgency: NotificationUrgency::Critical,
                icon: "battery-low".into(),
                actions: vec![],
            },
        ]
    }

    #[test]
    fn notifications_renders_all() {
        let notifs = make_notifications();
        let comp = NotificationsComponent {
            notifications: &notifs,
        };
        let tree = comp.render();

        assert_eq!(tree.tag, "notification-area");
        assert_eq!(tree.children.len(), 2);
    }

    #[test]
    fn notification_has_correct_structure() {
        let notifs = make_notifications();
        let comp = NotificationsComponent {
            notifications: &notifs,
        };
        let tree = comp.render();

        let first = &tree.children[0];
        assert_eq!(first.tag, "notification");
        assert_eq!(first.key.as_deref(), Some("notif-1"));
        assert!(first.classes.contains(&"urgency-normal".to_string()));

        // Should have icon + content + actions
        assert_eq!(first.children.len(), 3);
        assert_eq!(first.children[0].tag, "notification-icon");
        assert_eq!(first.children[1].tag, "notification-content");
        assert_eq!(first.children[2].tag, "notification-actions");
    }

    #[test]
    fn notification_urgency_classes() {
        let notifs = make_notifications();
        let comp = NotificationsComponent {
            notifications: &notifs,
        };
        let tree = comp.render();

        assert!(tree.children[0]
            .classes
            .contains(&"urgency-normal".to_string()));
        assert!(tree.children[1]
            .classes
            .contains(&"urgency-critical".to_string()));
    }

    #[test]
    fn notification_actions() {
        let notifs = make_notifications();
        let comp = NotificationsComponent {
            notifications: &notifs,
        };
        let tree = comp.render();

        let actions = &tree.children[0].children[2];
        assert_eq!(actions.children.len(), 2);
        assert_eq!(actions.children[0].tag, "notification-action");
        assert!(actions.children[0]
            .attrs
            .iter()
            .any(|(k, v)| k == "data-action-id" && v == "read"));
    }

    #[test]
    fn notification_no_icon_when_empty() {
        let notifs = vec![NotificationInfo {
            id: 1,
            summary: "Test".into(),
            body: "Body".into(),
            urgency: NotificationUrgency::Low,
            icon: String::new(),
            actions: vec![],
        }];
        let comp = NotificationsComponent {
            notifications: &notifs,
        };
        let tree = comp.render();

        // No icon, just content
        assert_eq!(tree.children[0].children.len(), 1);
        assert_eq!(tree.children[0].children[0].tag, "notification-content");
    }

    #[test]
    fn notification_no_actions_when_empty() {
        let notifs = make_notifications();
        let comp = NotificationsComponent {
            notifications: &notifs,
        };
        let tree = comp.render();

        // Second notification (battery) has no actions
        let battery = &tree.children[1];
        // Should have icon + content (no actions)
        assert_eq!(battery.children.len(), 2);
    }

    #[test]
    fn empty_notifications() {
        let comp = NotificationsComponent { notifications: &[] };
        let tree = comp.render();

        assert_eq!(tree.tag, "notification-area");
        assert_eq!(tree.children.len(), 0);
    }
}
