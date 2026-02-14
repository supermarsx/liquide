//! StatusBar component — renders proper tag names matching CSS selectors.
//!
//! **Critical fix**: the old `sync_dom` created ALL items as `<statusbar-item>`,
//! but the CSS themes target `status-indicator`, `notification-indicator`,
//! and `status-tray` tag names.  This component creates the correct tag
//! for each item kind so CSS selectors actually match.

use crate::types::{element_ids, StatusBarItemData, StatusBarSlot};
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
///     <status-indicator id="connection" class="connected">Connected</status-indicator>
///     <status-tray id="tray" />
///     <session-button id="session">username</session-button>
///   </statusbar-slot>
/// </statusbar>
/// ```
///
/// Now the CSS selectors `status-indicator.connected`, `notification-indicator.active`,
/// and `status-tray` actually match the DOM elements.
pub struct StatusBarComponent<'a> {
    pub slots: &'a [StatusBarSlot; 3], // left, center, right
}

impl StatusBarComponent<'_> {
    /// Determine the correct tag name for a status bar item.
    fn tag_for_kind(item: &StatusBarItemData) -> &'static str {
        match item {
            StatusBarItemData::Clock { .. } => "statusbar-item",
            StatusBarItemData::NotificationIndicator { .. } => "notification-indicator",
            StatusBarItemData::ConnectionQuality { .. } => "status-indicator",
            StatusBarItemData::TrayArea => "status-tray",
            StatusBarItemData::SessionButton { .. } => "session-button",
        }
    }

    /// Generate an ID for an item based on its variant.
    fn id_for_item(item: &StatusBarItemData, _index: usize) -> String {
        match item {
            StatusBarItemData::Clock { .. } => "clock".to_string(),
            StatusBarItemData::NotificationIndicator { .. } => "notifications".to_string(),
            StatusBarItemData::ConnectionQuality { .. } => "connection".to_string(),
            StatusBarItemData::TrayArea => "tray".to_string(),
            StatusBarItemData::SessionButton { .. } => "session".to_string(),
        }
    }

    /// Determine CSS classes for a status bar item based on its data.
    fn classes_for_item(item: &StatusBarItemData) -> Vec<&'static str> {
        match item {
            StatusBarItemData::NotificationIndicator { unread_count, dnd } => {
                let mut classes = Vec::new();
                if *unread_count > 0 {
                    classes.push("active");
                }
                if *dnd {
                    classes.push("dnd");
                }
                classes
            }
            StatusBarItemData::ConnectionQuality { connected, degraded } => {
                if *degraded {
                    vec!["degraded"]
                } else if *connected {
                    vec!["connected"]
                } else {
                    vec!["disconnected"]
                }
            }
            _ => vec![],
        }
    }

    /// Render the display text for an item.
    fn text_for_item(item: &StatusBarItemData) -> String {
        match item {
            StatusBarItemData::Clock { time } => time.clone(),
            StatusBarItemData::NotificationIndicator { unread_count, dnd } => {
                if *dnd {
                    "DND".into()
                } else if *unread_count > 0 {
                    format!("{unread_count}")
                } else {
                    String::new()
                }
            }
            StatusBarItemData::ConnectionQuality { connected, degraded } => {
                if *connected && !degraded {
                    "Connected".into()
                } else if *connected && *degraded {
                    "Degraded".into()
                } else {
                    "Disconnected".into()
                }
            }
            StatusBarItemData::TrayArea => String::new(),
            StatusBarItemData::SessionButton { username } => username.clone(),
        }
    }

    /// Render a single item into a TemplateNode.
    fn render_item(item: &StatusBarItemData, index: usize) -> TemplateNode {
        let tag = Self::tag_for_kind(item);
        let id = Self::id_for_item(item, index);
        let classes = Self::classes_for_item(item);
        let text = Self::text_for_item(item);

        let mut node = TemplateNode::el(tag)
            .id(&id)
            .key(&id);

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
        slot_index: usize,
    ) -> TemplateNode {
        TemplateNode::el("statusbar-slot")
            .id(slot_id)
            .class(slot_class)
            .children(
                self.slots[slot_index]
                    .items
                    .iter()
                    .enumerate()
                    .map(|(i, item)| Self::render_item(item, i)),
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
                0,
            ))
            .child(self.render_slot(
                element_ids::STATUSBAR_SLOT_CENTER,
                "center",
                1,
            ))
            .child(self.render_slot(
                element_ids::STATUSBAR_SLOT_RIGHT,
                "right",
                2,
            ))
    }

    fn mount_point(&self) -> &str {
        element_ids::STATUSBAR
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_default_slots() -> [StatusBarSlot; 3] {
        [
            // Left slot (empty)
            StatusBarSlot { items: vec![] },
            // Center slot (clock)
            StatusBarSlot {
                items: vec![StatusBarItemData::Clock {
                    time: "12:00".to_string(),
                }],
            },
            // Right slot (notifications, connection, tray, session)
            StatusBarSlot {
                items: vec![
                    StatusBarItemData::NotificationIndicator {
                        unread_count: 3,
                        dnd: false,
                    },
                    StatusBarItemData::ConnectionQuality {
                        connected: true,
                        degraded: false,
                    },
                    StatusBarItemData::TrayArea,
                    StatusBarItemData::SessionButton {
                        username: "user".to_string(),
                    },
                ],
            },
        ]
    }

    #[test]
    fn statusbar_renders_correct_structure() {
        let slots = make_default_slots();
        let comp = StatusBarComponent { slots: &slots };
        let tree = comp.render();

        assert_eq!(tree.tag, "statusbar");
        assert_eq!(tree.children.len(), 3); // left, center, right slots
    }

    #[test]
    fn statusbar_uses_correct_tag_names() {
        let slots = make_default_slots();
        let comp = StatusBarComponent { slots: &slots };
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
        let slots = make_default_slots();
        let comp = StatusBarComponent { slots: &slots };
        let tree = comp.render();

        let right = &tree.children[2];
        let notif = &right.children[0];
        assert!(notif.classes.contains(&"active".to_string()));
    }

    #[test]
    fn statusbar_connection_connected_class() {
        let slots = make_default_slots();
        let comp = StatusBarComponent { slots: &slots };
        let tree = comp.render();

        let right = &tree.children[2];
        let conn = &right.children[1];
        assert!(conn.classes.contains(&"connected".to_string()));
    }

    #[test]
    fn statusbar_connection_degraded_class() {
        let mut slots = make_default_slots();
        // Make connection degraded
        slots[2].items[1] = StatusBarItemData::ConnectionQuality {
            connected: true,
            degraded: true,
        };

        let comp = StatusBarComponent { slots: &slots };
        let tree = comp.render();

        let right = &tree.children[2];
        let conn = &right.children[1];
        assert!(conn.classes.contains(&"degraded".to_string()));
        assert!(!conn.classes.contains(&"connected".to_string()));
    }

    #[test]
    fn statusbar_clock_text() {
        let slots = make_default_slots();
        let comp = StatusBarComponent { slots: &slots };
        let tree = comp.render();

        let clock = &tree.children[1].children[0];
        assert_eq!(clock.children.len(), 1);
        assert_eq!(clock.children[0].text.as_deref(), Some("12:00"));
    }

    #[test]
    fn statusbar_empty_slot() {
        let slots = make_default_slots();
        let comp = StatusBarComponent { slots: &slots };
        let tree = comp.render();

        let left = &tree.children[0];
        // Left slot is empty
        assert_eq!(left.children.len(), 0);
    }

    #[test]
    fn statusbar_mount_point() {
        let slots = make_default_slots();
        let comp = StatusBarComponent { slots: &slots };
        assert_eq!(comp.mount_point(), element_ids::STATUSBAR);
    }
}
