//! `sync_dom()` — push current shell state into the desktop DOM tree.

use crate::desktop_dom::DockItemInfo;
use crate::launcher::SearchResultKind;

use super::Shell;

impl Shell {
    /// Push current shell state into the desktop DOM tree.
    ///
    /// Called once per frame just before the CSS pipeline runs.
    pub(crate) fn sync_dom(&mut self) {
        use crate::components_dock::DockComponent;
        use crate::components_launcher::LauncherComponent;
        use crate::components_menus::{
            AppMenuComponent, ContextMenuComponent, SessionMenuComponent,
        };
        use crate::components_notifications::{
            NotificationAction as NotificationActionInfo, NotificationInfo, NotificationUrgency,
            NotificationsComponent,
        };
        use crate::components_statusbar::StatusBarComponent;
        use crate::{Component, TemplateRenderer};
        use liquide_components::element_ids;

        // ── Dock (template-driven) ──────────────────────────
        let dock_infos: Vec<liquide_components::DockItemInfo> = self
            .dock
            .items()
            .iter()
            .map(|item| liquide_components::DockItemInfo {
                app_id: item.app_id.clone(),
                label: item.label.clone(),
                icon: item.icon.clone(),
                is_running: item.running_window_count > 0,
                is_pinned: item.pinned_position.is_some(),
            })
            .collect();
        let hover_idx = self.dock.hover_index();
        let dock_comp = DockComponent {
            items: &dock_infos,
            hover_index: hover_idx,
        };
        TemplateRenderer::apply(&mut self.desktop_dom.doc, &dock_comp);

        // ── Status bar (template-driven, correct tag names) ──
        use liquide_components::{StatusBarItemData, StatusBarSlot};

        // Map status bar items to component types
        let mut left_items = Vec::new();
        let mut center_items = Vec::new();
        let mut right_items = Vec::new();

        // Add logo to left slot
        left_items.push(StatusBarItemData::Logo {
            name: "LiquiDE".into(),
        });

        for item in self.status_bar.items() {
            use liquide_statusbar::StatusBarItemKind;
            let component_item = match &item.kind {
                StatusBarItemKind::Clock { .. } => {
                    // Format current time as HH:MM
                    let now = std::time::SystemTime::now();
                    let secs = now.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
                    let hours = (secs / 3600) % 24;
                    let minutes = (secs / 60) % 60;
                    let time_str = format!("{:02}:{:02}", hours, minutes);
                    StatusBarItemData::Clock { time: time_str }
                }
                StatusBarItemKind::NotificationIndicator {
                    unread_count,
                    dnd_active,
                } => StatusBarItemData::NotificationIndicator {
                    unread_count: *unread_count as usize,
                    dnd: *dnd_active,
                },
                StatusBarItemKind::ConnectionQuality {
                    quality_percent, ..
                } => StatusBarItemData::ConnectionQuality {
                    connected: *quality_percent > 0,
                    degraded: *quality_percent < 80,
                },
                StatusBarItemKind::TrayArea => StatusBarItemData::TrayArea,
                StatusBarItemKind::SessionButton => StatusBarItemData::SessionButton {
                    username: "User".into(),
                },
                StatusBarItemKind::Custom { .. } => continue,
            };

            // Distribute to slots
            if matches!(item.kind, StatusBarItemKind::Clock { .. }) {
                center_items.push(component_item);
            } else {
                right_items.push(component_item);
            }
        }

        let slots = [
            StatusBarSlot { items: left_items },
            StatusBarSlot {
                items: center_items,
            },
            StatusBarSlot { items: right_items },
        ];

        let statusbar_comp = StatusBarComponent { slots: &slots };
        TemplateRenderer::apply(&mut self.desktop_dom.doc, &statusbar_comp);

        // ── Notifications (template-driven, incremental) ─────
        let notif_infos: Vec<NotificationInfo> = self
            .notifications
            .active_notifications()
            .iter()
            .map(|sn| NotificationInfo {
                id: sn.id,
                summary: sn.notification.summary.clone(),
                body: sn.notification.body.clone(),
                urgency: match sn.notification.urgency {
                    liquide_interop::notification::Urgency::Low => NotificationUrgency::Low,
                    liquide_interop::notification::Urgency::Normal => NotificationUrgency::Normal,
                    liquide_interop::notification::Urgency::Critical => {
                        NotificationUrgency::Critical
                    }
                },
                icon: sn.notification.icon.clone().unwrap_or_default(),
                actions: sn
                    .notification
                    .actions
                    .iter()
                    .map(|action| NotificationActionInfo {
                        id: action.key.clone(),
                        label: action.label.clone(),
                    })
                    .collect(),
            })
            .collect();
        let notif_comp = NotificationsComponent {
            notifications: &notif_infos,
        };
        TemplateRenderer::apply(&mut self.desktop_dom.doc, &notif_comp);

        // ── Launcher (template-driven) ──────────────────────
        if self.launcher.is_visible() {
            let items: Vec<liquide_components::LauncherItemInfo> = self
                .launcher
                .results()
                .iter()
                .map(|r| {
                    let app_id = match &r.kind {
                        SearchResultKind::Application { app_id } => app_id.clone(),
                        _ => String::new(),
                    };
                    liquide_components::LauncherItemInfo {
                        app_id,
                        name: r.title.clone(),
                        description: String::new(),
                        icon: r.icon.clone().unwrap_or_default(),
                    }
                })
                .collect();
            let launcher_comp = LauncherComponent {
                items: &items,
                selected_index: self.launcher.selected_index(),
                search_query: self.launcher.query(),
                visible: true,
            };
            let root = self.desktop_dom.doc.root();
            let template = launcher_comp.render();
            TemplateRenderer::apply_or_create(
                &mut self.desktop_dom.doc,
                root,
                element_ids::LAUNCHER_OVERLAY,
                &template,
            );
        } else {
            TemplateRenderer::unmount(&mut self.desktop_dom.doc, element_ids::LAUNCHER_OVERLAY);
        }

        // ── Session menu (template-driven) ──────────────────
        if self.session_menu_visible {
            let items: Vec<liquide_components::MenuItemInfo> = self
                .session_menu_items
                .iter()
                .map(|si| liquide_components::MenuItemInfo {
                    label: si.label.clone(),
                    action: si.label.to_lowercase().replace(' ', "-"),
                    icon: if si.icon.is_empty() {
                        None
                    } else {
                        Some(si.icon.clone())
                    },
                    disabled: false,
                })
                .collect();
            let session_comp = SessionMenuComponent {
                items: &items,
                hover_index: self.session_menu_hover_index,
            };
            let root = self.desktop_dom.doc.root();
            let template = session_comp.render();
            TemplateRenderer::apply_or_create(
                &mut self.desktop_dom.doc,
                root,
                element_ids::SESSION_MENU,
                &template,
            );
        } else {
            TemplateRenderer::unmount(&mut self.desktop_dom.doc, element_ids::SESSION_MENU);
        }

        // ── Context menu (template-driven) ──────────────────
        if self.context_menu_visible {
            use super::ContextMenuItem;
            let ctx_items = ContextMenuItem::defaults();
            let infos: Vec<liquide_components::ContextMenuItemInfo> = ctx_items
                .iter()
                .map(|ci| {
                    liquide_components::ContextMenuItemInfo::Item(
                        liquide_components::MenuItemInfo {
                            label: ci.label.clone(),
                            action: ci.label.to_lowercase().replace(' ', "-"),
                            icon: None,
                            disabled: false,
                        },
                    )
                })
                .collect();
            let ctx_comp = ContextMenuComponent {
                menu_id: "ctx-shell",
                items: &infos,
                hover_index: self.context_menu_hover_index,
                position: Some((self.context_menu_pos.x, self.context_menu_pos.y)),
            };
            let root = self.desktop_dom.doc.root();
            let template = ctx_comp.render();
            TemplateRenderer::apply_or_create(
                &mut self.desktop_dom.doc,
                root,
                "ctx-shell",
                &template,
            );
        } else {
            TemplateRenderer::unmount(&mut self.desktop_dom.doc, "ctx-shell");
        }

        // ── App menu (template-driven) ──────────────────────
        if self.app_menu_open.is_some() {
            let items = vec![
                liquide_components::MenuItemInfo {
                    label: "Minimize".into(),
                    action: "minimize".into(),
                    icon: None,
                    disabled: false,
                },
                liquide_components::MenuItemInfo {
                    label: "Maximize".into(),
                    action: "maximize".into(),
                    icon: None,
                    disabled: false,
                },
                liquide_components::MenuItemInfo {
                    label: "Close".into(),
                    action: "close".into(),
                    icon: None,
                    disabled: false,
                },
                liquide_components::MenuItemInfo {
                    label: "System Settings".into(),
                    action: "settings".into(),
                    icon: None,
                    disabled: false,
                },
                liquide_components::MenuItemInfo {
                    label: "About Liquide".into(),
                    action: "about".into(),
                    icon: None,
                    disabled: false,
                },
            ];
            let app_comp = AppMenuComponent {
                items: &items,
                hover_index: None,
            };
            let root = self.desktop_dom.doc.root();
            let template = app_comp.render();
            TemplateRenderer::apply_or_create(
                &mut self.desktop_dom.doc,
                root,
                element_ids::APP_MENU,
                &template,
            );
        } else {
            TemplateRenderer::unmount(&mut self.desktop_dom.doc, element_ids::APP_MENU);
        }

        // Keep the DOM viewport in sync with the screen rect.
        self.css_pipeline
            .set_viewport(self.screen_rect.width, self.screen_rect.height);

        if let Some(coordinator) = &self.thread_coordinator {
            let thread_dock_items: Vec<DockItemInfo> = self
                .dock
                .items()
                .iter()
                .map(|item| DockItemInfo {
                    app_id: item.app_id.clone(),
                    label: item.label.clone(),
                    icon: item.icon.clone(),
                    is_running: item.running_window_count > 0,
                    is_pinned: item.pinned_position.is_some(),
                })
                .collect();
            coordinator.update_dock(thread_dock_items, self.dock.hover_index());

            let statusbar_items: Vec<crate::threading::StatusBarItemUpdate> = self
                .status_bar
                .items()
                .iter()
                .map(|item| {
                    use liquide_statusbar::StatusBarItemKind;
                    let content = match &item.kind {
                        StatusBarItemKind::Clock { .. } => {
                            let now = std::time::SystemTime::now();
                            let secs = now
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs();
                            format!("{:02}:{:02}", (secs / 3600) % 24, (secs / 60) % 60)
                        }
                        StatusBarItemKind::NotificationIndicator { unread_count, .. } => {
                            unread_count.to_string()
                        }
                        StatusBarItemKind::ConnectionQuality { quality_percent, .. } => {
                            format!("{}%", quality_percent)
                        }
                        StatusBarItemKind::TrayArea => "tray".to_string(),
                        StatusBarItemKind::SessionButton => "session".to_string(),
                        StatusBarItemKind::Custom { content, .. } => content.clone(),
                    };
                    let slot = match item.slot {
                        liquide_statusbar::StatusBarSlot::Left => {
                            crate::desktop_dom::StatusBarSlotKind::Left
                        }
                        liquide_statusbar::StatusBarSlot::Center => {
                            crate::desktop_dom::StatusBarSlotKind::Center
                        }
                        liquide_statusbar::StatusBarSlot::Right => {
                            crate::desktop_dom::StatusBarSlotKind::Right
                        }
                    };

                    crate::threading::StatusBarItemUpdate {
                        slot,
                        item_id: item.id.clone(),
                        content,
                        visible: item.visible,
                    }
                })
                .collect();
            coordinator.update_statusbar(statusbar_items);

            let launcher_items: Vec<crate::desktop_dom::LauncherItemInfo> = self
                .launcher
                .results()
                .iter()
                .map(|r| {
                    let app_id = match &r.kind {
                        SearchResultKind::Application { app_id } => app_id.clone(),
                        _ => String::new(),
                    };
                    crate::desktop_dom::LauncherItemInfo {
                        app_id,
                        label: r.title.clone(),
                        icon: r.icon.clone().unwrap_or_default(),
                    }
                })
                .collect();
            coordinator.update_launcher(
                self.launcher.is_visible(),
                self.launcher.query().to_string(),
                launcher_items,
                if self.launcher.result_count() > 0 {
                    Some(self.launcher.selected_index())
                } else {
                    None
                },
            );

            let notifications: Vec<crate::threading::NotificationData> = self
                .notifications
                .active_notifications()
                .iter()
                .map(|sn| crate::threading::NotificationData {
                    id: sn.id.to_string(),
                    title: sn.notification.summary.clone(),
                    body: sn.notification.body.clone(),
                    urgency: format!("{:?}", sn.notification.urgency).to_lowercase(),
                })
                .collect();
            coordinator.update_notifications(notifications);
        }

        self.dom_dirty = false;
    }
}
