//! `sync_dom()` — push current shell state into the desktop DOM tree
//! using the HTML template engine.
//!
//! Each shell element (statusbar, dock, notifications, launcher, menus) is
//! rendered via `TemplateRegistry::render()` with a `TemplateContext` built
//! from live shell state.  The rendered HTML replaces the element's children
//! in the DOM.  A per-template cache skips redundant DOM rebuilds when the
//! rendered HTML hasn't changed.

use crate::desktop_dom::DockItemInfo;
use crate::launcher::SearchResultKind;
use liquide_dom::html_parser::parse_html_into;
use liquide_dom::template_registry::TemplateContext;
use liquide_interop::notification::Urgency;
use liquide_statusbar::{StatusBarItem, StatusBarItemKind, StatusBarSlot};

use super::{CONTEXT_MENU_WIDTH, MENU_ITEM_HEIGHT, MENU_PADDING, Shell};

impl Shell {
    /// Push current shell state into the desktop DOM tree.
    ///
    /// Called once per frame just before the CSS pipeline runs.
    pub(crate) fn sync_dom(&mut self) {
        self.sync_statusbar_template();
        self.sync_dock_template();
        self.sync_notifications_template();
        self.sync_launcher_template();
        self.sync_session_menu_template();
        self.sync_context_menu_template();
        self.sync_app_menu_template();
        self.sync_tooltip_template();

        // Keep the DOM viewport in sync with the screen rect.
        self.css_pipeline
            .set_viewport(self.screen_rect.width, self.screen_rect.height);

        // ── Thread coordinator fallback (remote rendering) ───
        self.sync_thread_coordinator();

        self.dom_dirty = false;
    }

    fn status_bar_item_text(&self, item: &StatusBarItem) -> String {
        match &item.kind {
            StatusBarItemKind::Clock { format } => self
                .status_bar
                .format_clock_timestamp(item.last_update_us, format),
            StatusBarItemKind::NotificationIndicator { unread_count, .. } => {
                if *unread_count > 0 {
                    unread_count.to_string()
                } else {
                    String::new()
                }
            }
            StatusBarItemKind::ConnectionQuality {
                quality_percent, ..
            } => format!("{quality_percent}%"),
            StatusBarItemKind::TrayArea => String::new(),
            StatusBarItemKind::SessionButton => "User".to_string(),
            StatusBarItemKind::Custom { content, .. } => content.clone(),
        }
    }

    fn focused_dock_app_id(&self) -> Option<String> {
        self.dock.focused_app().map(str::to_string).or_else(|| {
            self.focus.focused().and_then(|window_id| {
                self.windows.get(&window_id).and_then(|window| {
                    if window.app_id.is_empty() {
                        None
                    } else {
                        Some(window.app_id.clone())
                    }
                })
            })
        })
    }

    fn live_tray_items(&self) -> Vec<TemplateContext> {
        let mut tray_items = Vec::new();

        let mut notification_items = self.notifications.visible_tray_icons();
        notification_items.sort_by(|left, right| {
            left.app_name
                .cmp(&right.app_name)
                .then_with(|| left.id.0.cmp(&right.id.0))
        });
        for icon in notification_items {
            let mut ctx = TemplateContext::new();
            ctx.set("id", &format!("tray-icon-{}", icon.id.0));
            ctx.set("source", "notification");
            ctx.set("label", &icon.app_name);
            ctx.set(
                "tooltip",
                if icon.tooltip.is_empty() {
                    icon.app_name.as_str()
                } else {
                    icon.tooltip.as_str()
                },
            );
            ctx.set("icon", &icon.icon);
            ctx.set("has_icon", !icon.icon.is_empty());
            ctx.set("has_badge", icon.badge.is_some());
            ctx.set("badge", icon.badge.as_deref().unwrap_or(""));
            ctx.set("has_menu", !icon.menu_items.is_empty());
            ctx.set("has_icon_data", false);
            ctx.set("classes", if icon.badge.is_some() { "has-badge" } else { "" });
            tray_items.push(ctx);
        }

        let mut seamless_items: Vec<_> = self.seamless.tray_icons().values().collect();
        seamless_items.sort_by(|left, right| {
            left.app_id
                .cmp(&right.app_id)
                .then_with(|| left.item_id.cmp(&right.item_id))
        });
        for icon in seamless_items {
            let mut ctx = TemplateContext::new();
            let label = if icon.app_id.is_empty() {
                icon.item_id.as_str()
            } else {
                icon.app_id.as_str()
            };
            ctx.set("id", &format!("tray-item-{}", icon.item_id));
            ctx.set("source", "seamless");
            ctx.set("label", label);
            ctx.set(
                "tooltip",
                if icon.tooltip.is_empty() {
                    label
                } else {
                    icon.tooltip.as_str()
                },
            );
            ctx.set("icon", "");
            ctx.set("has_icon", false);
            ctx.set("has_badge", false);
            ctx.set("badge", "");
            ctx.set("has_menu", !icon.menu_items.is_empty());
            ctx.set("has_icon_data", !icon.icon_data.is_empty());
            ctx.set(
                "classes",
                if icon.icon_data.is_empty() {
                    "seamless"
                } else {
                    "seamless has-icon-data"
                },
            );
            tray_items.push(ctx);
        }

        tray_items
    }

    // ══════════════════════════════════════════════════════════
    // Status bar
    // ══════════════════════════════════════════════════════════

    fn sync_statusbar_template(&mut self) {
        let mut ctx = TemplateContext::new();
        ctx.set("show_branding", self.status_bar.config().show_app_menu);
        ctx.set("branding_text", "LiquiDE");

        let left_items: Vec<TemplateContext> = self
            .status_bar
            .items_in_slot(StatusBarSlot::Left)
            .into_iter()
            .filter(|item| item.visible)
            .map(|item| {
                let mut item_ctx = TemplateContext::new();
                item_ctx.set("id", &item.id);
                item_ctx.set("classes", "");
                item_ctx.set("text", &self.status_bar_item_text(item));
                item_ctx
            })
            .collect();
        ctx.set("left_items", left_items);

        let mut center_items = Vec::new();
        for item in self.status_bar.items_in_slot(StatusBarSlot::Center) {
            if !item.visible {
                continue;
            }
            if matches!(&item.kind, StatusBarItemKind::Clock { .. } | StatusBarItemKind::Custom { .. }) {
                let mut item_ctx = TemplateContext::new();
                item_ctx.set("id", &item.id);
                item_ctx.set("classes", "");
                item_ctx.set("text", &self.status_bar_item_text(item));
                center_items.push(item_ctx);
            }
        }
        ctx.set("center_items", center_items);

        let mut right_items = Vec::new();
        let tray_items = self.live_tray_items();
        for item in self.status_bar.items_in_slot(StatusBarSlot::Right) {
            if !item.visible {
                continue;
            }
            match &item.kind {
                StatusBarItemKind::NotificationIndicator {
                    unread_count,
                    dnd_active,
                } => {
                    let mut ic = TemplateContext::new();
                    ic.set("id", &item.id);
                    ic.set("type_notification", true);
                    let cls = if *dnd_active {
                        "dnd"
                    } else if *unread_count > 0 {
                        "active"
                    } else {
                        ""
                    };
                    ic.set("classes", cls);
                    ic.set("text", &unread_count.to_string());
                    right_items.push(ic);
                }
                StatusBarItemKind::ConnectionQuality {
                    quality_percent, ..
                } => {
                    let mut ic = TemplateContext::new();
                    ic.set("id", &item.id);
                    ic.set("type_status", true);
                    let cls = if *quality_percent == 0 {
                        "disconnected"
                    } else if *quality_percent < 80 {
                        "degraded"
                    } else {
                        "connected"
                    };
                    ic.set("classes", cls);
                    right_items.push(ic);
                }
                StatusBarItemKind::TrayArea => {
                    let mut ic = TemplateContext::new();
                    ic.set("id", &item.id);
                    ic.set("type_tray", true);
                    ic.set("tray_items", self.live_tray_items());
                    ic.set("tray_item_count", tray_items.len().to_string());
                    right_items.push(ic);
                }
                StatusBarItemKind::SessionButton => {
                    let mut ic = TemplateContext::new();
                    ic.set("id", &item.id);
                    ic.set("type_session", true);
                    ic.set("text", &self.status_bar_item_text(item));
                    right_items.push(ic);
                }
                StatusBarItemKind::Custom { .. } => {
                    let mut ic = TemplateContext::new();
                    ic.set("id", &item.id);
                    ic.set("classes", "");
                    ic.set("text", &self.status_bar_item_text(item));
                    right_items.push(ic);
                }
                StatusBarItemKind::Clock { .. } => {}
            }
        }
        ctx.set("right_items", right_items);

        self.apply_template("statusbar", "shell-statusbar", &ctx);
    }

    // ══════════════════════════════════════════════════════════
    // Dock
    // ══════════════════════════════════════════════════════════

    fn sync_dock_template(&mut self) {
        let mut ctx = TemplateContext::new();
        let hover_idx = self.dock.hover_index();
        let focused_app_id = self.focused_dock_app_id();

        let dock_items: Vec<TemplateContext> = self
            .dock
            .items()
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let mut ic = TemplateContext::new();
                ic.set("index", &i.to_string());
                ic.set("app_id", &item.app_id);
                ic.set("label", &item.label);
                ic.set("icon", &item.icon);
                let is_active = item.running_window_count > 0;
                let is_focused = focused_app_id.as_deref() == Some(item.app_id.as_str());
                let has_badge = item.badge_count > 0;
                let mut classes = Vec::new();
                if is_active {
                    classes.push("active");
                }
                if item.pinned_position.is_some() {
                    classes.push("pinned");
                }
                if hover_idx == Some(i) {
                    classes.push("hovered");
                }
                if is_focused {
                    classes.push("focused");
                }
                if item.needs_attention {
                    classes.push("needs-attention");
                }
                ic.set("is_running", is_active);
                ic.set("is_pinned", item.pinned_position.is_some());
                ic.set("is_hovered", hover_idx == Some(i));
                ic.set("is_focused", is_focused);
                ic.set("needs_attention", item.needs_attention);
                ic.set("has_badge", has_badge);
                ic.set("badge_count", item.badge_count.to_string());
                ic.set("classes", classes.join(" "));
                ic
            })
            .collect();
        ctx.set("dock_items", dock_items);

        self.apply_template("dock", "shell-dock", &ctx);
    }

    // ══════════════════════════════════════════════════════════
    // Notifications
    // ══════════════════════════════════════════════════════════

    fn sync_notifications_template(&mut self) {
        let active = self.notifications.active_notifications();
        if active.is_empty() {
            // Clear notification area children
            if let Some(area) = self.desktop_dom.doc.get_element_by_id("notification-area") {
                let children: Vec<_> = self.desktop_dom.doc.children(area).to_vec();
                for child in children {
                    self.desktop_dom.doc.remove_child(area, child);
                    self.desktop_dom.doc.destroy_node(child);
                }
            }
            self.template_cache.remove("notifications");
            return;
        }

        // Render each notification individually using the "notification" template
        let area = match self.desktop_dom.doc.get_element_by_id("notification-area") {
            Some(id) => id,
            None => return,
        };

        let mut html = String::new();
        for sn in active {
            let mut nc = TemplateContext::new();
            nc.set("id", &format!("notif-{}", sn.id));
            nc.set("title", &sn.notification.summary);
            nc.set("body", &sn.notification.body);

            // Urgency class for CSS styling
            let urgency_class = match sn.notification.urgency {
                Urgency::Low => "urgency-low",
                Urgency::Normal => "urgency-normal",
                Urgency::Critical => "urgency-critical",
            };
            nc.set("urgency_class", urgency_class);

            // Optional icon
            if let Some(ref icon) = sn.notification.icon {
                nc.set("icon", icon);
            }

            // Actions list
            let has_actions = !sn.notification.actions.is_empty();
            nc.set("has_actions", has_actions);
            if has_actions {
                let action_ctxs: Vec<TemplateContext> = sn
                    .notification
                    .actions
                    .iter()
                    .map(|a| {
                        let mut ac = TemplateContext::new();
                        ac.set("action_id", &a.key);
                        ac.set("label", &a.label);
                        ac
                    })
                    .collect();
                nc.set("actions", action_ctxs);
            }

            if let Some(rendered) = self.template_registry.render("notification", &nc) {
                html.push_str(&rendered);
            }
        }

        if let Some(cached) = self.template_cache.get("notifications") {
            if *cached == html {
                return;
            }
        }

        // Clear and rebuild
        let children: Vec<_> = self.desktop_dom.doc.children(area).to_vec();
        for child in children {
            self.desktop_dom.doc.remove_child(area, child);
            self.desktop_dom.doc.destroy_node(child);
        }
        parse_html_into(&mut self.desktop_dom.doc, area, &html);
        self.template_cache.insert("notifications".into(), html);
    }

    // ══════════════════════════════════════════════════════════
    // Launcher
    // ══════════════════════════════════════════════════════════

    fn sync_launcher_template(&mut self) {
        if self.launcher.is_visible() {
            let mut ctx = TemplateContext::new();
            ctx.set("query", self.launcher.query());

            let items: Vec<TemplateContext> = self
                .launcher
                .results()
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    let app_id = match &r.kind {
                        SearchResultKind::Application { app_id } => app_id.as_str(),
                        _ => "",
                    };
                    let mut ic = TemplateContext::new();
                    ic.set("index", &i.to_string());
                    ic.set("app_id", app_id);
                    ic.set("label", &r.title);
                    ic.set("icon", r.icon.as_deref().unwrap_or(""));
                    ic
                })
                .collect();
            ctx.set("results", items);

            if let Some(html) = self.template_registry.render("launcher", &ctx) {
                if let Some(cached) = self.template_cache.get("launcher") {
                    if *cached == html {
                        return;
                    }
                }
                let root = self.desktop_dom.doc.root();
                // Remove existing launcher overlay
                if let Some(existing) = self.desktop_dom.doc.get_element_by_id("launcher-overlay") {
                    if let Some(parent) = self.desktop_dom.doc.parent(existing) {
                        self.desktop_dom.doc.remove_child(parent, existing);
                    }
                    self.desktop_dom.doc.destroy_node(existing);
                }
                parse_html_into(&mut self.desktop_dom.doc, root, &html);
                self.template_cache.insert("launcher".into(), html);
            }
        } else {
            // Remove launcher overlay if present
            if let Some(existing) = self.desktop_dom.doc.get_element_by_id("launcher-overlay") {
                if let Some(parent) = self.desktop_dom.doc.parent(existing) {
                    self.desktop_dom.doc.remove_child(parent, existing);
                }
                self.desktop_dom.doc.destroy_node(existing);
            }
            self.template_cache.remove("launcher");
        }
    }

    // ══════════════════════════════════════════════════════════
    // Session menu
    // ══════════════════════════════════════════════════════════

    fn sync_session_menu_template(&mut self) {
        if self.session_menu_visible {
            let items: Vec<TemplateContext> = self
                .session_menu_items
                .iter()
                .enumerate()
                .map(|(i, si)| {
                    let mut ic = TemplateContext::new();
                    ic.set("index", &i.to_string());
                    ic.set("label", &si.label);
                    ic.set("action", &si.label.to_lowercase().replace(' ', "-"));
                    if !si.icon.is_empty() {
                        ic.set("icon", &si.icon);
                    }
                    ic
                })
                .collect();

            let mut ctx = TemplateContext::new();
            ctx.set("id", "session-menu");
            ctx.set("items", items);

            let menu_bounds = self.session_menu_bounds();
            ctx.set("pos_left", &format!("{}px", menu_bounds.x.round() as i32));
            ctx.set("pos_top", &format!("{}px", menu_bounds.y.round() as i32));

            self.apply_overlay_template("session-menu", "session-menu", &ctx);
        } else {
            self.remove_overlay("session-menu");
            self.template_cache.remove("session-menu");
        }
    }

    // ══════════════════════════════════════════════════════════
    // Context menu
    // ══════════════════════════════════════════════════════════

    fn sync_context_menu_template(&mut self) {
        if self.context_menu_visible {
            use super::ContextMenuItem;
            let ctx_items = ContextMenuItem::defaults();
            let items: Vec<TemplateContext> = ctx_items
                .iter()
                .enumerate()
                .map(|(i, ci)| {
                    let mut ic = TemplateContext::new();
                    ic.set("index", &i.to_string());
                    ic.set("label", &ci.label);
                    ic.set("action", &ci.label.to_lowercase().replace(' ', "-"));
                    if !ci.icon.is_empty() {
                        ic.set("icon", &ci.icon);
                    }
                    ic
                })
                .collect();

            let mut ctx = TemplateContext::new();
            ctx.set("id", "ctx-shell");
            ctx.set("items", items);

            // Position the context menu at the right-click location, clamped to screen.
            let ctx_x = self.context_menu_pos.x;
            let ctx_y = self.context_menu_pos.y;
            let menu_h = MENU_PADDING * 2.0 + ctx_items.len() as f32 * MENU_ITEM_HEIGHT;
            let clamped_x = ctx_x
                .min(self.screen_rect.width - CONTEXT_MENU_WIDTH - 4.0)
                .max(0.0);
            let clamped_y = ctx_y.min(self.screen_rect.height - menu_h - 4.0).max(0.0);
            ctx.set("pos_left", &format!("{}px", clamped_x.round() as i32));
            ctx.set("pos_top", &format!("{}px", clamped_y.round() as i32));

            self.apply_overlay_template("context-menu", "ctx-shell", &ctx);
        } else {
            self.remove_overlay("ctx-shell");
            self.template_cache.remove("context-menu");
        }
    }

    // ══════════════════════════════════════════════════════════
    // App menu
    // ══════════════════════════════════════════════════════════

    fn sync_app_menu_template(&mut self) {
        if self.app_menu_open.is_some() {
            let menu_items = [
                ("Minimize", "minimize"),
                ("Maximize", "maximize"),
                ("Close", "close"),
                ("System Settings", "settings"),
                ("About Liquide", "about"),
            ];
            let target_window = self
                .app_menu_target_window_id()
                .map(|window_id| window_id.0.to_string())
                .unwrap_or_default();
            let items: Vec<TemplateContext> = menu_items
                .iter()
                .enumerate()
                .map(|(i, (label, action))| {
                    let mut ic = TemplateContext::new();
                    ic.set("index", &i.to_string());
                    ic.set("label", *label);
                    ic.set("action", *action);
                    ic.set(
                        "selected_class",
                        if self.app_menu_hover_index == Some(i) {
                            "selected"
                        } else {
                            ""
                        },
                    );
                    ic.set(
                        "aria_selected",
                        if self.app_menu_hover_index == Some(i) {
                            "true"
                        } else {
                            "false"
                        },
                    );
                    ic.set(
                        "tab_index",
                        if self.app_menu_hover_index == Some(i) {
                            "0"
                        } else {
                            "-1"
                        },
                    );
                    ic
                })
                .collect();

            let mut ctx = TemplateContext::new();
            ctx.set("id", "app-menu");
            ctx.set("items", items);
            ctx.set("window_id", &target_window);

            if let Some(menu_bounds) = self.app_menu_bounds(menu_items.len()) {
                ctx.set("pos_left", &format!("{}px", menu_bounds.x.round() as i32));
                ctx.set("pos_top", &format!("{}px", menu_bounds.y.round() as i32));
                self.apply_overlay_template("app-menu", "app-menu", &ctx);
            } else {
                self.remove_overlay("app-menu");
                self.template_cache.remove("app-menu");
            }
        } else {
            self.remove_overlay("app-menu");
            self.template_cache.remove("app-menu");
        }
    }

    // ══════════════════════════════════════════════════════════
    // Tooltip
    // ══════════════════════════════════════════════════════════

    fn sync_tooltip_template(&mut self) {
        if let Some(ref text) = self.tooltip_text {
            // Enforce 400ms hover delay before showing tooltip
            let now_us = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_micros() as u64;
            if now_us.saturating_sub(self.tooltip_timer_us) < 400_000 {
                // Not enough time elapsed — don't show yet, but remove stale overlay
                self.remove_overlay("shell-tooltip");
                self.template_cache.remove("tooltip");
                return;
            }

            let mut ctx = TemplateContext::new();
            ctx.set("id", "shell-tooltip");
            ctx.set("text", text.as_str());
            ctx.set("position", "top"); // tooltip appears above the dock
            ctx.set(
                "pos_left",
                &format!("{}px", self.tooltip_pos.x.round() as i32),
            );
            ctx.set(
                "pos_top",
                &format!("{}px", self.tooltip_pos.y.round() as i32),
            );

            self.apply_overlay_template("tooltip", "shell-tooltip", &ctx);
        } else {
            self.remove_overlay("shell-tooltip");
            self.template_cache.remove("tooltip");
        }
    }

    // ══════════════════════════════════════════════════════════
    // Helpers
    // ══════════════════════════════════════════════════════════

    /// Render a template and replace the children of the element with the given
    /// DOM id.  Skips the DOM rebuild if the rendered HTML hasn't changed.
    fn apply_template(&mut self, template_name: &str, element_id: &str, ctx: &TemplateContext) {
        let html = match self.template_registry.render(template_name, ctx) {
            Some(h) => h,
            None => return,
        };

        // Cache check — skip DOM rebuild if output is identical.
        if let Some(cached) = self.template_cache.get(template_name) {
            if *cached == html {
                return;
            }
        }

        // Find the target element and replace its children.
        if let Some(node_id) = self.desktop_dom.doc.get_element_by_id(element_id) {
            let children: Vec<_> = self.desktop_dom.doc.children(node_id).to_vec();
            for child in children {
                self.desktop_dom.doc.remove_child(node_id, child);
                self.desktop_dom.doc.destroy_node(child);
            }
            parse_html_into(&mut self.desktop_dom.doc, node_id, &html);
        }

        self.template_cache.insert(template_name.to_string(), html);
    }

    /// Render a template as a top-level overlay (appended to root), creating
    /// or replacing as needed.
    fn apply_overlay_template(
        &mut self,
        template_name: &str,
        element_id: &str,
        ctx: &TemplateContext,
    ) {
        let html = match self.template_registry.render(template_name, ctx) {
            Some(h) => h,
            None => return,
        };

        if let Some(cached) = self.template_cache.get(template_name) {
            if *cached == html {
                return;
            }
        }

        // Remove existing overlay
        self.remove_overlay(element_id);

        // Append new overlay to root
        let root = self.desktop_dom.doc.root();
        parse_html_into(&mut self.desktop_dom.doc, root, &html);

        self.template_cache.insert(template_name.to_string(), html);
    }

    /// Remove an overlay element from the DOM.
    fn remove_overlay(&mut self, element_id: &str) {
        if let Some(existing) = self.desktop_dom.doc.get_element_by_id(element_id) {
            if let Some(parent) = self.desktop_dom.doc.parent(existing) {
                self.desktop_dom.doc.remove_child(parent, existing);
            }
            self.desktop_dom.doc.destroy_node(existing);
        }
        // Invalidate cache so it re-renders if shown again.
        // (Use element_id as cache key isn't 1:1 with template name, but the
        // apply_overlay caller passes matching names.)
    }

    /// Push state to the threaded fallback coordinator (for remote rendering).
    fn sync_thread_coordinator(&self) {
        let Some(coordinator) = &self.thread_coordinator else {
            return;
        };

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
                let content = self.status_bar_item_text(item);
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
}
