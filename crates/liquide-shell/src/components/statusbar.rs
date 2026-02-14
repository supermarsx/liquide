//! StatusBar component — renders proper tag names matching CSS selectors.
//!
//! **Critical fix**: the old `sync_dom` created ALL items as `<statusbar-item>`,
//! but the CSS themes target `status-indicator`, `notification-indicator`,
//! and `status-tray` tag names.  This component creates the correct tag
//! for each item kind so CSS selectors actually match.

use crate::desktop_dom::element_ids;
use crate::status_bar::{StatusBarItem, StatusBarItemKind, StatusBarSlot};
use crate::template::{Component, TemplateNode};

/// StatusBar component.
///
/// Produces a DOM tree like:
/// ```text
/// <statusbar id="shell-statusbar">
///   <statusbar-slot class="left">
///     (custom items)
///   </statusbar-slot>
///   <statusbar-slot class="center">
///     <statusbar-item id="clock">12:34</statusbar-item>
///   </statusbar-slot>
///   <statusbar-slot class="right">
///     <notification-indicator id="notifications" class="active">3</notification-indicator>
///     <status-indicator id="connection" class="connected">100% 5ms</status-indicator>
///     <status-tray id="tray" />
///     <session-button id="session">⏻</session-button>
///   </statusbar-slot>
/// </statusbar>
/// ```
///
/// Now the CSS selectors `status-indicator.connected`, `notification-indicator.active`,
/// and `status-tray` actually match the DOM elements.
pub struct StatusBarComponent<'a> {
    pub items: &'a [StatusBarItem],
}

impl StatusBarComponent<'_> {
    /// Determine the correct tag name for a status bar item.
    fn tag_for_kind(kind: &StatusBarItemKind) -> &'static str {
        match kind {
            StatusBarItemKind::Clock { .. } => "statusbar-item",
            StatusBarItemKind::NotificationIndicator { .. } => "notification-indicator",
            StatusBarItemKind::ConnectionQuality { .. } => "status-indicator",
            StatusBarItemKind::TrayArea => "status-tray",
            StatusBarItemKind::SessionButton => "session-button",
            StatusBarItemKind::Custom { .. } => "statusbar-item",
        }
    }

    /// Determine CSS classes for a status bar item based on its data.
    fn classes_for_item(item: &StatusBarItem) -> Vec<&'static str> {
        match &item.kind {
            StatusBarItemKind::NotificationIndicator {
                unread_count,
                dnd_active,
            } => {
                let mut classes = Vec::new();
                if *unread_count > 0 {
                    classes.push("active");
                }
                if *dnd_active {
                    classes.push("dnd");
                }
                classes
            }
            StatusBarItemKind::ConnectionQuality {
                quality_percent,
                latency_ms,
            } => {
                if *latency_ms > 200 || *quality_percent < 50 {
                    vec!["degraded"]
                } else {
                    vec!["connected"]
                }
            }
            _ => vec![],
        }
    }

    /// Render the display text for an item.
    fn text_for_item(item: &StatusBarItem) -> String {
        match &item.kind {
            StatusBarItemKind::Clock { .. } => {
                let seconds = item.last_update_us / 1_000_000;
                let hours = (seconds / 3600) % 24;
                let minutes = (seconds % 3600) / 60;
                format!("{hours:02}:{minutes:02}")
            }
            StatusBarItemKind::NotificationIndicator {
                unread_count,
                dnd_active,
            } => {
                if *dnd_active {
                    "DND".into()
                } else if *unread_count > 0 {
                    format!("{unread_count}")
                } else {
                    String::new()
                }
            }
            StatusBarItemKind::ConnectionQuality {
                quality_percent,
                latency_ms,
            } => {
                if *latency_ms > 0 {
                    format!("{quality_percent}% {latency_ms}ms")
                } else {
                    format!("{quality_percent}%")
                }
            }
            StatusBarItemKind::TrayArea => String::new(),
            StatusBarItemKind::SessionButton => "\u{23FB}".into(),
            StatusBarItemKind::Custom { content, .. } => content.clone(),
        }
    }

    /// Render a single item into a TemplateNode.
    fn render_item(item: &StatusBarItem) -> TemplateNode {
        let tag = Self::tag_for_kind(&item.kind);
        let classes = Self::classes_for_item(item);
        let text = Self::text_for_item(item);

        let mut node = TemplateNode::el(tag)
            .id(&item.id)
            .key(&item.id);

        for cls in classes {
            node = node.class(cls);
        }

        if !text.is_empty() {
            node = node.child(TemplateNode::text(&text));
        }

        node
    }

    /// Render items for a specific slot.
    fn render_slot(
        &self,
        slot_id: &str,
        slot_class: &str,
        target_slot: StatusBarSlot,
    ) -> TemplateNode {
        TemplateNode::el("statusbar-slot")
            .id(slot_id)
            .class(slot_class)
            .children(
                self.items
                    .iter()
                    .filter(|item| item.visible && item.slot == target_slot)
                    .map(Self::render_item),
            )
    }
}

impl Component for StatusBarComponent<'_> {
    fn render(&self) -> TemplateNode {
        TemplateNode::el("statusbar")
            .id(element_ids::STATUSBAR)
            .child(self.render_slot(
                element_ids::STATUSBAR_SLOT_LEFT,
                "left",
                StatusBarSlot::Left,
            ))
            .child(self.render_slot(
                element_ids::STATUSBAR_SLOT_CENTER,
                "center",
                StatusBarSlot::Center,
            ))
            .child(self.render_slot(
                element_ids::STATUSBAR_SLOT_RIGHT,
                "right",
                StatusBarSlot::Right,
            ))
    }

    fn mount_point(&self) -> &str {
        element_ids::STATUSBAR
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::status_bar::StatusBarItem;

    fn make_default_items() -> Vec<StatusBarItem> {
        vec![
            StatusBarItem {
                id: "clock".into(),
                kind: StatusBarItemKind::Clock {
                    format: "%H:%M".into(),
                },
                slot: StatusBarSlot::Center,
                visible: true,
                cached: false,
                last_update_us: 43200_000_000, // 12:00
            },
            StatusBarItem {
                id: "notifications".into(),
                kind: StatusBarItemKind::NotificationIndicator {
                    unread_count: 3,
                    dnd_active: false,
                },
                slot: StatusBarSlot::Right,
                visible: true,
                cached: false,
                last_update_us: 0,
            },
            StatusBarItem {
                id: "connection".into(),
                kind: StatusBarItemKind::ConnectionQuality {
                    quality_percent: 95,
                    latency_ms: 12,
                },
                slot: StatusBarSlot::Right,
                visible: true,
                cached: false,
                last_update_us: 0,
            },
            StatusBarItem {
                id: "tray".into(),
                kind: StatusBarItemKind::TrayArea,
                slot: StatusBarSlot::Right,
                visible: true,
                cached: false,
                last_update_us: 0,
            },
            StatusBarItem {
                id: "session".into(),
                kind: StatusBarItemKind::SessionButton,
                slot: StatusBarSlot::Right,
                visible: true,
                cached: false,
                last_update_us: 0,
            },
        ]
    }

    #[test]
    fn statusbar_renders_correct_structure() {
        let items = make_default_items();
        let comp = StatusBarComponent { items: &items };
        let tree = comp.render();

        assert_eq!(tree.tag, "statusbar");
        assert_eq!(tree.children.len(), 3); // left, center, right slots
    }

    #[test]
    fn statusbar_uses_correct_tag_names() {
        let items = make_default_items();
        let comp = StatusBarComponent { items: &items };
        let tree = comp.render();

        // Center slot → clock as statusbar-item
        let center = &tree.children[1];
        assert_eq!(center.children.len(), 1); // clock only
        assert_eq!(center.children[0].tag, "statusbar-item");

        // Right slot → notification-indicator, status-indicator, status-tray, session-button
        let right = &tree.children[2];
        assert_eq!(right.children.len(), 4);
        assert_eq!(right.children[0].tag, "notification-indicator");
        assert_eq!(right.children[1].tag, "status-indicator");
        assert_eq!(right.children[2].tag, "status-tray");
        assert_eq!(right.children[3].tag, "session-button");
    }

    #[test]
    fn statusbar_notification_active_class() {
        let items = make_default_items();
        let comp = StatusBarComponent { items: &items };
        let tree = comp.render();

        let right = &tree.children[2];
        let notif = &right.children[0];
        assert!(notif.classes.contains(&"active".to_string()));
    }

    #[test]
    fn statusbar_connection_connected_class() {
        let items = make_default_items();
        let comp = StatusBarComponent { items: &items };
        let tree = comp.render();

        let right = &tree.children[2];
        let conn = &right.children[1];
        assert!(conn.classes.contains(&"connected".to_string()));
    }

    #[test]
    fn statusbar_connection_degraded_class() {
        let mut items = make_default_items();
        // Make connection degraded
        items[2].kind = StatusBarItemKind::ConnectionQuality {
            quality_percent: 30,
            latency_ms: 300,
        };

        let comp = StatusBarComponent { items: &items };
        let tree = comp.render();

        let right = &tree.children[2];
        let conn = &right.children[1];
        assert!(conn.classes.contains(&"degraded".to_string()));
        assert!(!conn.classes.contains(&"connected".to_string()));
    }

    #[test]
    fn statusbar_clock_text() {
        let items = make_default_items();
        let comp = StatusBarComponent { items: &items };
        let tree = comp.render();

        let clock = &tree.children[1].children[0];
        assert_eq!(clock.children.len(), 1);
        assert_eq!(clock.children[0].text.as_deref(), Some("12:00"));
    }

    #[test]
    fn statusbar_hidden_items_excluded() {
        let mut items = make_default_items();
        items[1].visible = false; // Hide notifications

        let comp = StatusBarComponent { items: &items };
        let tree = comp.render();

        let right = &tree.children[2];
        // Should have 3 items instead of 4
        assert_eq!(right.children.len(), 3);
        // First one should be connection, not notifications
        assert_eq!(right.children[0].tag, "status-indicator");
    }

    #[test]
    fn statusbar_mount_point() {
        let items = make_default_items();
        let comp = StatusBarComponent { items: &items };
        assert_eq!(comp.mount_point(), element_ids::STATUSBAR);
    }
}
