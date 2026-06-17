//! `sync_dom()` — push current shell state into the desktop DOM tree
//! using the HTML template engine.
//!
//! Each shell element (statusbar, dock, notifications, launcher, menus) is
//! rendered via `TemplateRegistry::render()` with a `TemplateContext` built
//! from live shell state.  The rendered HTML replaces the element's children
//! in the DOM.  A per-template cache skips redundant DOM rebuilds when the
//! rendered HTML hasn't changed.

use std::collections::HashSet;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::launcher::SearchResultKind;
use liquide_dom::NodeId;
use liquide_dom::escape_html;
use liquide_dom::html_parser::parse_html_into;
use liquide_dom::template_registry::TemplateContext;
use liquide_interop::notification::Urgency;
use liquide_statusbar::{StatusBarItem, StatusBarItemKind, StatusBarSlot};

use super::Shell;

const NOTIFICATION_ITEM_CACHE_PREFIX: &str = "notifications:";

fn template_state_hash<T: Hash>(state: &T) -> String {
    let mut hasher = DefaultHasher::new();
    state.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Render a single `<status-tray-item>` element from a tray context into `out`.
///
/// The status-bar tray HTML is assembled by hand in Rust (the flat template
/// engine cannot express the nested per-item structure), so this function is
/// the manual attribute-building site that bypasses the template registry's
/// escaping. Every value here originates from untrusted notification/seamless
/// sources (app titles, tooltips, ids), so each is routed through
/// [`escape_html`] before being embedded into a raw attribute value — otherwise
/// an embedded `"` or `<` could break out of an attribute and inject elements
/// into the shell chrome DOM (T49-e5-F06).
fn render_tray_item(tray: &TemplateContext, out: &mut String) {
    let source = escape_html(tray.get_str("source"));
    let label = escape_html(tray.get_str("label"));
    let tooltip = escape_html(tray.get_str("tooltip"));
    let icon = escape_html(tray.get_str("icon"));
    let badge = escape_html(tray.get_str("badge"));
    let classes = escape_html(tray.get_str("classes"));
    let has_icon = tray.is_truthy("has_icon");
    let has_badge = tray.is_truthy("has_badge");
    let has_menu = tray.is_truthy("has_menu");
    let has_icon_data = tray.is_truthy("has_icon_data");
    let inner_id = escape_html(tray.get_str("id"));
    let mut attrs = format!(
        " id=\"{inner_id}\" data-source=\"{source}\" data-label=\"{label}\" data-tooltip=\"{tooltip}\""
    );
    if has_icon {
        attrs.push_str(&format!(" data-icon=\"{icon}\""));
    }
    if has_menu {
        attrs.push_str(" data-has-menu=\"true\"");
    }
    if has_icon_data {
        attrs.push_str(" data-has-icon-data=\"true\"");
    }
    if !classes.is_empty() {
        attrs.push_str(&format!(" class=\"{classes}\""));
    }
    out.push_str(&format!("<status-tray-item{attrs}>"));
    if has_badge {
        out.push_str(&format!("<status-tray-badge>{badge}</status-tray-badge>"));
    }
    out.push_str("</status-tray-item>");
}

fn element_inner_html<'a>(html: &'a str, tag: &str) -> Option<&'a str> {
    let open_start = html.find(&format!("<{tag}"))?;
    let open_end = html[open_start..].find('>')? + open_start;
    let inner_start = open_end + 1;
    let close = format!("</{tag}>");
    let close_start = html[inner_start..].rfind(&close)? + inner_start;
    Some(&html[inner_start..close_start])
}

impl Shell {
    /// Push current shell state into the desktop DOM tree.
    ///
    /// Called once per frame just before the CSS pipeline runs.
    ///
    /// Returns `true` if the DOM has any pending dirty nodes after sync
    /// (i.e. chrome content changed this frame and must be re-rendered).
    ///
    /// The "chrome changed?" signal is simply "is the DOM dirty set non-empty?"
    /// (t82-incremental). This is reliable because `build_scene` now CONSUMES the
    /// dirty set every frame — on a cache miss after the pipeline reads it, and
    /// on a cache hit right before returning — so at the start of any frame the
    /// set is empty and only THIS frame's mutations remain. Those mutations come
    /// from two places, both of which must count:
    ///   * event-time DOM mutations (e.g. `dispatch_mouse_move` setting a
    ///     `:hover` pseudo-state on the item under the cursor), which happen
    ///     between builds, and
    ///   * sync-time mutations from the `sync_*_template` calls below.
    /// The previous before/after-LENGTH watch missed BOTH a repeat-dirtying of an
    /// already-dirty node (HashSet length unchanged) and event-time hover
    /// dirtying, so a moving menu-item hover returned a STALE cached scene. A
    /// plain non-empty check fixes that. Each `sync_*_template` still early-
    /// returns when its HTML cache matches, so an idle frame leaves the set
    /// empty and this returns `false` (the idle full-scene cache may reuse).
    pub(crate) fn sync_dom(&mut self) -> bool {
        self.sync_statusbar_template();
        self.sync_dock_template();
        self.sync_notifications_template();
        self.sync_notification_center_template();
        self.sync_launcher_template();
        self.sync_session_menu_template();
        self.sync_context_menu_template();
        self.sync_app_menu_template();
        self.sync_dialog_template();
        self.sync_lockscreen_template();
        self.sync_overview_template();
        self.sync_window_decorations();
        // App window content via CSS widgets (t108-p8). Drive the per-window
        // widget hosts (action → model → re-render loop) FIRST so this frame's
        // pipeline sees any model-driven re-render, then mount/position the
        // content hosts (initial mount + structural remount + position sync).
        let _app_widgets_changed = self.drive_app_widget_hosts();
        self.sync_app_widget_content();
        self.sync_tooltip_template();

        // Keep the DOM viewport in sync with the screen rect.
        self.css_pipeline
            .set_viewport(self.screen_rect.width, self.screen_rect.height);

        let changed = self.dom_dirty_len() != 0 || self.dom_dirty;
        self.dom_dirty = false;
        changed
    }

    /// Total number of DOM nodes currently flagged dirty (style+layout+paint).
    /// Used by [`Shell::sync_dom`] to detect whether a template mutation
    /// occurred this frame (the set only grows in the shell flow until cleared).
    pub(crate) fn dom_dirty_len(&self) -> usize {
        let d = &self.desktop_dom.doc.dirty;
        d.style.len() + d.layout.len() + d.paint.len()
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
            ctx.set(
                "classes",
                if icon.badge.is_some() {
                    "has-badge"
                } else {
                    ""
                },
            );
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
        // The template engine in `liquide-dom` does not support nested
        // `{{#if}}` / `{{#each}}` blocks, so we build the per-slot HTML
        // ourselves in Rust and feed it into a flat template via three
        // raw-string substitutions (`*_items_html`).  This keeps the
        // dispatch on `StatusBarItemKind` (notification, status, tray,
        // session, …) entirely in Rust where nesting is natural.
        let cfg = self.status_bar.config().clone();

        let mut left_html = String::new();
        for item in self.status_bar.items_in_slot(StatusBarSlot::Left) {
            if !item.visible {
                continue;
            }
            let text = self.status_bar_item_text(item);
            left_html.push_str(&format!(
                "<statusbar-item id=\"{id}\" class=\"\">{text}</statusbar-item>",
                id = escape_html(&item.id),
                text = escape_html(&text),
            ));
        }

        let mut center_html = String::new();
        for item in self.status_bar.items_in_slot(StatusBarSlot::Center) {
            if !item.visible {
                continue;
            }
            if !matches!(
                &item.kind,
                StatusBarItemKind::Clock { .. } | StatusBarItemKind::Custom { .. }
            ) {
                continue;
            }
            let text = self.status_bar_item_text(item);
            center_html.push_str(&format!(
                "<statusbar-item id=\"{id}\" class=\"\">{text}</statusbar-item>",
                id = escape_html(&item.id),
                text = escape_html(&text),
            ));
        }

        let live_tray = self.live_tray_items();
        let mut right_html = String::new();
        for item in self.status_bar.items_in_slot(StatusBarSlot::Right) {
            if !item.visible {
                continue;
            }
            match &item.kind {
                StatusBarItemKind::NotificationIndicator {
                    unread_count,
                    dnd_active,
                } => {
                    let cls = if *dnd_active {
                        "dnd"
                    } else if *unread_count > 0 {
                        "active"
                    } else {
                        ""
                    };
                    right_html.push_str(&format!(
                        "<notification-indicator id=\"{id}\" class=\"{cls}\">{count}</notification-indicator>",
                        id = escape_html(&item.id),
                        cls = cls,
                        count = unread_count,
                    ));
                }
                StatusBarItemKind::ConnectionQuality {
                    quality_percent, ..
                } => {
                    let cls = if *quality_percent == 0 {
                        "disconnected"
                    } else if *quality_percent < 80 {
                        "degraded"
                    } else {
                        "connected"
                    };
                    right_html.push_str(&format!(
                        "<status-indicator id=\"{id}\" class=\"{cls}\"></status-indicator>",
                        id = escape_html(&item.id),
                        cls = cls,
                    ));
                }
                StatusBarItemKind::TrayArea => {
                    right_html.push_str(&format!(
                        "<status-tray id=\"{id}\" data-count=\"{count}\">",
                        id = escape_html(&item.id),
                        count = live_tray.len(),
                    ));
                    for tray in &live_tray {
                        render_tray_item(tray, &mut right_html);
                    }
                    right_html.push_str("</status-tray>");
                }
                StatusBarItemKind::SessionButton => {
                    right_html.push_str(&format!(
                        "<session-button id=\"{id}\">{text}</session-button>",
                        id = escape_html(&item.id),
                        text = escape_html(&self.status_bar_item_text(item)),
                    ));
                }
                StatusBarItemKind::Custom { .. } => {
                    right_html.push_str(&format!(
                        "<statusbar-item id=\"{id}\" class=\"\">{text}</statusbar-item>",
                        id = escape_html(&item.id),
                        text = escape_html(&self.status_bar_item_text(item)),
                    ));
                }
                StatusBarItemKind::Clock { .. } => {}
            }
        }

        let mut ctx = TemplateContext::new();
        ctx.set("show_branding", cfg.show_app_menu);
        ctx.set("branding_text", "LiquiDE");
        // These are pre-built HTML fragments (each per-item value already
        // escaped via `escape_html` at the attribute/text build sites above),
        // so they must be substituted VERBATIM — `set_raw_html`, not `set`,
        // otherwise the flat template engine would HTML-escape the structural
        // `<statusbar-item>` markup and the whole status bar would render as
        // visible escaped text (T49-e5-F06: escape dynamic values, never the
        // structural markup).
        ctx.set_raw_html("left_items_html", left_html);
        ctx.set_raw_html("center_items_html", center_html);
        ctx.set_raw_html("right_items_html", right_html);

        self.apply_template("statusbar", "shell-statusbar", &ctx);
        self.mark_wired(crate::shell::WiringBit::StatusBar);
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
        // Apply position/size/alignment/auto-hide as data-attrs + CSS custom
        // properties on `#shell-dock` so the theme CSS can react (row vs column
        // flex, which edge to anchor, justify-content, slide-out on hide) and
        // so the scene/geometry authority (compute_bounds/compute_item_rects)
        // and the DOM stay in agreement (t72-dock shell follow-up §2/§4).
        self.sync_dock_attributes();
        // Set the `:hover` pseudo-state on the hovered dock item so the themed
        // `dock-item:hover` rule paints (the template only injects a `.hovered`
        // class, which the theme does not style). Applied after the template so
        // it targets the freshly-rendered item children (t65-s3).
        self.desktop_dom.set_dock_hover(hover_idx);
        self.mark_wired(crate::shell::WiringBit::Dock);
    }

    /// Push the resolved [`DockConfig`] onto the `#shell-dock` element as
    /// data-attributes + CSS custom properties, and reflect auto-hide
    /// visibility as `data-hidden`. The theme CSS keys off these to pick the
    /// dock's flex direction, anchored edge, item distribution, label
    /// visibility, sizing, and slide-out transform (t72-dock follow-up §2/§4).
    fn sync_dock_attributes(&mut self) {
        use liquide_dock::{DockAlignment, DockPosition};

        let cfg = self.dock.config();
        let position = match cfg.position {
            DockPosition::Bottom => "bottom",
            DockPosition::Top => "top",
            DockPosition::Left => "left",
            DockPosition::Right => "right",
        };
        let alignment = match cfg.alignment {
            DockAlignment::Centered => "centered",
            DockAlignment::Justified => "justified",
        };
        let show_labels = if cfg.show_labels { "true" } else { "false" };
        let icon_size = cfg.icon_size;
        let thickness = cfg.effective_thickness();
        let padding = cfg.padding;
        let spacing = cfg.spacing;
        // `data-hidden` reflects the live auto-hide visibility so CSS can
        // animate the dock off-screen; always-visible (mode Off) ⇒ shown.
        let hidden = !self.dock.is_visible();

        let id = crate::desktop_dom::element_ids::DOCK;
        if let Some(dock) = self.desktop_dom.doc.get_element_by_id(id) {
            // Only write attributes that actually changed: these are the SAME
            // values on every idle frame, and an unconditional `set_attribute`
            // re-dirties the dock node each frame, which (now that the shell
            // consumes the DOM dirty set per-frame) would defeat the full-scene
            // idle cache (t82-incremental).
            self.desktop_dom
                .set_attr_if_changed(dock, "data-position", position);
            self.desktop_dom
                .set_attr_if_changed(dock, "data-alignment", alignment);
            self.desktop_dom
                .set_attr_if_changed(dock, "data-show-labels", show_labels);
            self.desktop_dom.set_attr_if_changed(
                dock,
                "data-hidden",
                if hidden { "true" } else { "false" },
            );
            // CSS custom properties for sizing — the theme reads these via
            // `var(--dock-*)`; the scene path remains the geometry authority.
            self.desktop_dom.set_attr_if_changed(
                dock,
                "style",
                &format!(
                    "--dock-icon-size:{icon_size}px;--dock-thickness:{thickness}px;\
                     --dock-padding:{padding}px;--dock-gap:{spacing}px;"
                ),
            );
        }
    }

    // ══════════════════════════════════════════════════════════
    // Notifications
    // ══════════════════════════════════════════════════════════

    fn sync_notifications_template(&mut self) {
        let active = self.notifications.active_notifications().to_vec();
        if active.is_empty() {
            // Clear notification area children
            if let Some(area) = self.desktop_dom.doc.get_element_by_id("notification-area") {
                let children: Vec<_> = self.desktop_dom.doc.children(area).to_vec();
                for child in children {
                    self.desktop_dom.doc.remove_child(area, child);
                    self.desktop_dom.doc.destroy_node(child);
                }
            }
            self.clear_notification_template_cache();
            return;
        }

        // Render each notification individually using the "notification" template
        let area = match self.desktop_dom.doc.get_element_by_id("notification-area") {
            Some(id) => id,
            None => return,
        };

        let mut rendered_notifications = Vec::with_capacity(active.len());
        let mut html = String::new();
        for sn in &active {
            let mut nc = TemplateContext::new();
            let element_id = format!("notif-{}", sn.id);
            nc.set("id", &element_id);
            nc.set("title", &sn.notification.summary);
            nc.set("body", &sn.notification.body);

            // Urgency class for CSS styling
            let urgency_class = match sn.notification.urgency {
                Urgency::Low => "urgency-low",
                Urgency::Normal => "urgency-normal",
                Urgency::Critical => "urgency-critical",
            };
            nc.set("urgency_class", urgency_class);

            let action_state: Vec<_> = sn
                .notification
                .actions
                .iter()
                .map(|action| (action.key.as_str(), action.label.as_str()))
                .collect();
            let state_hash = template_state_hash(&(
                element_id.as_str(),
                sn.notification.summary.as_str(),
                sn.notification.body.as_str(),
                urgency_class,
                sn.notification.icon.as_deref().unwrap_or(""),
                &action_state,
            ));
            nc.set("state_hash", &state_hash);

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
                let cache_key = format!("{NOTIFICATION_ITEM_CACHE_PREFIX}{element_id}");
                rendered_notifications.push((element_id, cache_key, rendered));
            }
        }

        if let Some(cached) = self.template_cache.get("notifications") {
            if *cached == html {
                return;
            }
        }

        let mut desired_nodes = Vec::with_capacity(rendered_notifications.len());
        let mut live_cache_keys = HashSet::with_capacity(rendered_notifications.len());

        for (element_id, cache_key, item_html) in rendered_notifications {
            live_cache_keys.insert(cache_key.clone());
            let existing = self
                .desktop_dom
                .doc
                .get_element_by_id(&element_id)
                .filter(|&node| self.desktop_dom.doc.parent(node) == Some(area));
            let unchanged = self
                .template_cache
                .get(&cache_key)
                .is_some_and(|cached| cached == &item_html);

            let node = if unchanged {
                existing
            } else {
                if let Some(existing) = existing {
                    self.desktop_dom.doc.remove_child(area, existing);
                    self.desktop_dom.doc.destroy_node(existing);
                }
                parse_html_into(&mut self.desktop_dom.doc, area, &item_html);
                self.desktop_dom.doc.get_element_by_id(&element_id)
            };

            if let Some(node) = node {
                desired_nodes.push(node);
            }
            self.template_cache.insert(cache_key, item_html);
        }

        self.remove_stale_template_children(area, &desired_nodes);
        self.order_template_children(area, &desired_nodes);
        self.template_cache.retain(|key, _| {
            !key.starts_with(NOTIFICATION_ITEM_CACHE_PREFIX) || live_cache_keys.contains(key)
        });
        self.template_cache.insert("notifications".into(), html);
    }

    // ══════════════════════════════════════════════════════════
    // Notification center (live panel — t51-e14, fixes t49-e5-F03)
    // ══════════════════════════════════════════════════════════

    /// Render the notification center panel that the `OpenNotificationCenter`
    /// action / status-bar indicator toggles.
    ///
    /// Before t51-e14 the toggle flipped `notification_panel_visible` but
    /// nothing rendered it and the panel read no data — a dead end (F03). This
    /// builds a real `<notification-center>` overlay from the live notification
    /// set (active + history, kept canonical via the daemon-backed
    /// `post_notification` path) when the panel is open, and removes it when
    /// closed. The HTML is assembled by hand (the flat template registry has no
    /// nested-list template for this surface), so every dynamic value is routed
    /// through [`escape_html`] before embedding, matching the t50-e5 escaping
    /// discipline used by the status-bar tray builder above (T49-e5-F06).
    fn sync_notification_center_template(&mut self) {
        const CENTER_ID: &str = "notification-center";

        if !self.notification_center_open() {
            if let Some(existing) = self.desktop_dom.doc.get_element_by_id(CENTER_ID) {
                if let Some(parent) = self.desktop_dom.doc.parent(existing) {
                    self.desktop_dom.doc.remove_child(parent, existing);
                }
                self.desktop_dom.doc.destroy_node(existing);
            }
            self.template_cache.remove("notification-center");
            return;
        }

        let items = self.notification_center_items();

        let mut html = String::from("<notification-center id=\"notification-center\">");
        html.push_str(&format!(
            "<notification-center-header data-count=\"{count}\">Notifications</notification-center-header>",
            count = items.len()
        ));
        if items.is_empty() {
            html.push_str(
                "<notification-center-empty>No notifications</notification-center-empty>",
            );
        } else {
            html.push_str("<notification-center-list>");
            for sn in &items {
                let urgency_class = match sn.notification.urgency {
                    Urgency::Low => "urgency-low",
                    Urgency::Normal => "urgency-normal",
                    Urgency::Critical => "urgency-critical",
                };
                let title = escape_html(&sn.notification.summary);
                let body = escape_html(&sn.notification.body);
                let app = escape_html(&sn.notification.app_name);
                html.push_str(&format!(
                    "<notification-center-item id=\"notif-center-{id}\" \
                     class=\"{urgency_class}\" data-notif-id=\"{id}\" data-app=\"{app}\" \
                     data-read=\"{read}\">",
                    id = sn.id,
                    read = sn.read,
                ));
                html.push_str(&format!(
                    "<notification-center-title>{title}</notification-center-title>"
                ));
                if !sn.notification.body.is_empty() {
                    html.push_str(&format!(
                        "<notification-center-body>{body}</notification-center-body>"
                    ));
                }
                for action in &sn.notification.actions {
                    let action_id = escape_html(&action.key);
                    let label = escape_html(&action.label);
                    html.push_str(&format!(
                        "<notification-action data-notif-id=\"{id}\" \
                         data-action-id=\"{action_id}\">{label}</notification-action>",
                        id = sn.id,
                    ));
                }
                html.push_str("</notification-center-item>");
            }
            html.push_str("</notification-center-list>");
        }
        html.push_str("</notification-center>");

        if let Some(cached) = self.template_cache.get("notification-center") {
            if *cached == html {
                return;
            }
        }

        let root = self.desktop_dom.doc.root();
        if let Some(existing) = self.desktop_dom.doc.get_element_by_id(CENTER_ID) {
            if let Some(parent) = self.desktop_dom.doc.parent(existing) {
                self.desktop_dom.doc.remove_child(parent, existing);
            }
            self.desktop_dom.doc.destroy_node(existing);
        }
        parse_html_into(&mut self.desktop_dom.doc, root, &html);
        self.template_cache
            .insert("notification-center".into(), html);
    }

    // ══════════════════════════════════════════════════════════
    // Launcher
    // ══════════════════════════════════════════════════════════

    fn sync_launcher_template(&mut self) {
        if self.launcher.is_visible() {
            self.mark_wired(crate::shell::WiringBit::Launcher);
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
                    let key = if app_id.is_empty() {
                        format!("result-{i}")
                    } else {
                        app_id.to_string()
                    };
                    ic.set("key", &key);
                    ic.set("app_id", app_id);
                    ic.set("label", &r.title);
                    ic.set("icon", r.icon.as_deref().unwrap_or(""));
                    ic
                })
                .collect();
            ctx.set("results", items);

            let result_state: Vec<_> = self
                .launcher
                .results()
                .iter()
                .enumerate()
                .map(|(i, result)| {
                    let app_id = match &result.kind {
                        SearchResultKind::Application { app_id } => app_id.as_str(),
                        _ => "",
                    };
                    (
                        i,
                        app_id,
                        result.title.as_str(),
                        result.icon.as_deref().unwrap_or(""),
                    )
                })
                .collect();
            let state_hash = template_state_hash(&(self.launcher.query(), &result_state));
            ctx.set("state_hash", &state_hash);

            if let Some(html) = self.template_registry.render("launcher", &ctx) {
                if let Some(cached) = self.template_cache.get("launcher") {
                    if *cached == html {
                        return;
                    }
                }
                let root = self.desktop_dom.doc.root();
                if let Some(existing) = self.desktop_dom.doc.get_element_by_id("launcher-overlay") {
                    self.desktop_dom
                        .doc
                        .set_attribute(existing, "data-state-hash", &state_hash);
                    if let Some(inner_html) = element_inner_html(&html, "launcher-overlay") {
                        self.replace_template_children(existing, inner_html);
                    } else {
                        if let Some(parent) = self.desktop_dom.doc.parent(existing) {
                            self.desktop_dom.doc.remove_child(parent, existing);
                        }
                        self.desktop_dom.doc.destroy_node(existing);
                        parse_html_into(&mut self.desktop_dom.doc, root, &html);
                    }
                } else {
                    parse_html_into(&mut self.desktop_dom.doc, root, &html);
                }
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
            // Set the `:hover` pseudo-state on the highlighted item so the themed
            // `menu-item:hover` rule paints the keyboard-nav highlight (the
            // template carries no `.selected` rule the theme styles) (t65-s3).
            self.desktop_dom
                .set_menu_hover("session-menu", self.session_menu_hover_index);
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
            self.mark_wired(crate::shell::WiringBit::ContextMenu);
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
            ctx.set("id", "context-menu");
            ctx.set("items", items);

            // Position the context menu at the right-click location, clamped to screen.
            let ctx_x = self.context_menu_pos.x;
            let ctx_y = self.context_menu_pos.y;
            let menu_h = self.menu_padding() * 2.0 + ctx_items.len() as f32 * self.menu_item_height();
            let clamped_x = ctx_x
                .min(self.screen_rect.width - self.context_menu_width() - 4.0)
                .max(0.0);
            let clamped_y = ctx_y.min(self.screen_rect.height - menu_h - 4.0).max(0.0);
            ctx.set("pos_left", &format!("{}px", clamped_x.round() as i32));
            ctx.set("pos_top", &format!("{}px", clamped_y.round() as i32));

            self.apply_overlay_template("context-menu", "context-menu", &ctx);
            // Set the `:hover` pseudo-state on the highlighted item so the themed
            // `menu-item:hover` rule paints the keyboard-nav highlight, mirroring
            // the session-menu highlight (t66-navfix).
            self.desktop_dom
                .set_menu_hover("context-menu", self.context_menu_hover_index);
        } else {
            self.remove_overlay("context-menu");
            self.template_cache.remove("context-menu");
        }
    }

    // ══════════════════════════════════════════════════════════
    // Modal dialog (message box / input)
    // ══════════════════════════════════════════════════════════

    /// Sync the modal dialog overlay (t65-s3). When a canonical dialog is open
    /// (`chrome_dialog_content` set by `request_message_dialog` /
    /// `request_input_dialog`), render the `dialog` template so the title,
    /// message, and button LABELS paint as real text through the CSS pipeline.
    /// Replaces the prior imperative blank-rect dialog in `scene.rs`.
    fn sync_dialog_template(&mut self) {
        if let Some(content) = self.chrome_dialog_content.clone() {
            // Fall back to a single "OK" button when no labels were supplied.
            let labels: Vec<String> = if content.buttons.is_empty() {
                vec!["OK".to_string()]
            } else {
                content.buttons.clone()
            };
            // The default/primary button gets the `primary` accent. Clamp the
            // canonical default index into range (falls back to the last button).
            let primary_idx = content
                .default_button
                .min(labels.len().saturating_sub(1));
            let buttons: Vec<TemplateContext> = labels
                .iter()
                .enumerate()
                .map(|(i, label)| {
                    let mut bc = TemplateContext::new();
                    bc.set("index", &i.to_string());
                    bc.set("label", label);
                    bc.set("is_primary", i == primary_idx);
                    bc
                })
                .collect();

            let mut ctx = TemplateContext::new();
            ctx.set("id", "dialog-overlay");
            ctx.set("title", &content.title);
            ctx.set("message", &content.message);
            ctx.set("buttons", buttons);

            self.apply_overlay_template("dialog", "dialog-overlay", &ctx);
        } else {
            self.remove_overlay("dialog-overlay");
            self.template_cache.remove("dialog");
        }
    }

    // ══════════════════════════════════════════════════════════
    // Lock screen (t95-p4 full-CSS migration)
    // ══════════════════════════════════════════════════════════

    /// Sync the lock-screen overlay (t95-p4). When the canonical lock screen is
    /// engaged (`chrome_lockscreen` locked), render the `lockscreen` template so
    /// the clock, date, user name, and password field paint as real DOM/CSS
    /// elements through the pipeline — replacing the prior imperative
    /// filled-rect overlay (`scene.rs::add_lockscreen_overlay`).
    ///
    /// The password field (`#lockscreen-password`) is a real DOM box laid out by
    /// the `lockscreen-prompt` CSS rule; its click/focus hit-test reads that
    /// laid-out box (see `events.rs` lock-screen press handling +
    /// `lockscreen_password_field_bounds`), NOT a hardcoded geometry constant.
    fn sync_lockscreen_template(&mut self) {
        use liquide_lockscreen::screen::ScreenPhase;

        if let Some(layout) = self
            .chrome_lockscreen
            .as_ref()
            .filter(|s| s.is_locked())
            .map(|s| s.layout_info())
        {
            self.mark_wired(crate::shell::WiringBit::LockScreen);

            // The field is "focused" once the screen leaves the bare clock
            // phase (any password-entry / auth phase). Drives the `.focused`
            // CSS so the click-to-focus is visible.
            let focused = layout.phase != ScreenPhase::Clock;
            let error = layout.error_message.clone().unwrap_or_default();

            let mut ctx = TemplateContext::new();
            ctx.set("id", "lockscreen-overlay");
            ctx.set("clock", &layout.clock_text);
            ctx.set("date", &layout.date_text);
            ctx.set("display_name", &layout.display_name);
            ctx.set("dots", &layout.password_dots);
            ctx.set("focused_class", if focused { "focused" } else { "" });
            ctx.set("has_error", !error.is_empty());
            ctx.set("error", &error);

            self.apply_overlay_template("lockscreen", "lockscreen-overlay", &ctx);
        } else {
            self.remove_overlay("lockscreen-overlay");
            self.template_cache.remove("lockscreen");
        }
    }

    // ══════════════════════════════════════════════════════════
    // Overview / exposé (t101-p5 full-CSS migration)
    // ══════════════════════════════════════════════════════════

    /// Sync the overview / exposé overlay (t101-p5). When the overview is
    /// toggled (`overview_visible`), render the `overview` template so the grid
    /// of window tiles paints as real DOM/CSS elements laid out by CSS grid —
    /// replacing the prior imperative grid painter
    /// (`scene.rs::add_overview_overlay`'s `cols=sqrt(count)` math).
    ///
    /// Each `overview-tile` (`#overview-tile-<id>`) carries `data-window-id`;
    /// its click hit-test reads that laid-out CSS box (see
    /// `overview_adapter::overview_tile_window_at`), NOT hardcoded grid
    /// geometry — the recurring hit-test-from-CSS-geometry contract (t86). The
    /// captured window thumbnail (or glass placeholder) is painted onto each
    /// tile's laid-out box by `scene.rs::paint_overview_thumbnails`.
    fn sync_overview_template(&mut self) {
        if self.overview_visible {
            let focused = self.focus.focused();
            let tiles: Vec<TemplateContext> = self
                .visible_windows()
                .iter()
                .map(|w| {
                    let mut tc = TemplateContext::new();
                    tc.set("window_id", &w.id.0.to_string());
                    tc.set("title", &w.title);
                    tc.set(
                        "focused_class",
                        if focused == Some(w.id) { "focused" } else { "" },
                    );
                    tc
                })
                .collect();

            let mut ctx = TemplateContext::new();
            ctx.set("id", "overview-overlay");
            ctx.set("count", &tiles.len().to_string());
            ctx.set("tiles", tiles);

            self.apply_overlay_template("overview", "overview-overlay", &ctx);
        } else {
            self.remove_overlay("overview-overlay");
            self.template_cache.remove("overview");
        }
    }

    // ══════════════════════════════════════════════════════════
    // Window frame decorations (t103-p6 full-CSS migration)
    // ══════════════════════════════════════════════════════════

    /// Sync the per-window frame decoration DOM (t103-p6). For every visible
    /// **decorated** window, mount/maintain a `window-frame` element
    /// (`#window-deco-<id>`) inside `workspace-container`, absolutely positioned
    /// (inline `style`) over that window's titlebar screen rect, so the CSS
    /// pipeline lays out the title + close/maximize/minimize/pin buttons.
    ///
    /// The laid-out boxes are the single source of truth for BOTH the painted
    /// decoration geometry (`scene.rs::build_uncached_window_workspace_node`
    /// anchors the `Decoration` node's button rects to them) and the
    /// titlebar-drag / button hit-test (`window_decoration_adapter`), so a theme
    /// change that moves/resizes the buttons moves the painted glyphs AND the
    /// click zones together — the recurring hit-test-from-CSS contract (t86).
    ///
    /// To preserve the idle full-scene cache (t76) and frame-to-frame
    /// determinism (e2e_temporal), the per-frame update mutates the DOM ONLY
    /// when something actually changed: positions are integer-rounded and
    /// written through `set_attr_if_changed`, classes through the already-guarded
    /// `add_class`/`remove_class`, and the title text only when it differs. On a
    /// steady-state frame nothing is written, so `doc.dirty` stays empty and the
    /// idle cache holds.
    fn sync_window_decorations(&mut self) {
        use liquide_dom::PseudoStateFlags;

        let focused = self.focus.focused();
        let title_h = self.decoration_style.title_bar_height.round() as i32;

        // The set of window ids that SHOULD have a live decoration this frame.
        let mut wanted: HashSet<u64> = HashSet::new();

        // Snapshot the per-window decoration state first (immutable borrow of
        // `self.visible_windows`) so the mutation pass below can borrow the DOM
        // mutably without overlapping the window borrow.
        struct DecoState {
            id: u64,
            x: i32,
            y: i32,
            w: i32,
            title: String,
            focused: bool,
            topmost: bool,
        }
        let decos: Vec<DecoState> = self
            .visible_windows()
            .iter()
            .filter(|w| w.flags.contains(crate::window::WindowFlags::DECORATED))
            .map(|w| DecoState {
                id: w.id.0,
                x: w.bounds.x.round() as i32,
                y: w.bounds.y.round() as i32,
                w: w.bounds.width.round() as i32,
                title: w.title.clone(),
                focused: focused == Some(w.id),
                topmost: w.flags.contains(crate::window::WindowFlags::ALWAYS_ON_TOP),
            })
            .collect();

        for deco in &decos {
            wanted.insert(deco.id);
            let frame_id = format!("window-deco-{}", deco.id);

            // Create the subtree once if it does not exist yet.
            if self.desktop_dom.doc.get_element_by_id(&frame_id).is_none() {
                let mut ctx = TemplateContext::new();
                ctx.set("window_id", &deco.id.to_string());
                ctx.set("title", &deco.title);
                // The template's `style="..."` provides the initial position;
                // the layout engine takes unitless lengths, and the per-frame
                // pass below keeps it in sync via `set_inline_style`.
                ctx.set("x", &deco.x.to_string());
                ctx.set("y", &deco.y.to_string());
                ctx.set("w", &deco.w.to_string());
                ctx.set("h", &title_h.to_string());
                ctx.set("focused_class", if deco.focused { "focused" } else { "" });
                ctx.set("pin_class", if deco.topmost { "active" } else { "" });

                if let Some(html) = self.template_registry.render("window-frame", &ctx) {
                    let workspace = self.desktop_dom.workspace;
                    parse_html_into(&mut self.desktop_dom.doc, workspace, &html);
                }
            }

            // Patch position/size in place via change-guarded INLINE STYLES so an
            // idle frame leaves the DOM clean (the HTML parser consumes the
            // `style` attribute into inline styles at parse time, so per-frame
            // updates must go through `set_inline_style`, not the attribute).
            if let Some(frame) = self.desktop_dom.doc.get_element_by_id(&frame_id) {
                Self::set_inline_style_if_changed(
                    &mut self.desktop_dom.doc,
                    frame,
                    "left",
                    &deco.x.to_string(),
                );
                Self::set_inline_style_if_changed(
                    &mut self.desktop_dom.doc,
                    frame,
                    "top",
                    &deco.y.to_string(),
                );
                Self::set_inline_style_if_changed(
                    &mut self.desktop_dom.doc,
                    frame,
                    "width",
                    &deco.w.to_string(),
                );
                Self::set_inline_style_if_changed(
                    &mut self.desktop_dom.doc,
                    frame,
                    "height",
                    &title_h.to_string(),
                );

                // Focus class + pseudo-state (the `.focused` rule + `:focus`).
                let has_focused = self
                    .desktop_dom
                    .doc
                    .get(frame)
                    .map(|n| n.has_class("focused"))
                    .unwrap_or(false);
                if deco.focused && !has_focused {
                    self.desktop_dom.doc.add_class(frame, "focused");
                } else if !deco.focused && has_focused {
                    self.desktop_dom.doc.remove_class(frame, "focused");
                }
                self.desktop_dom
                    .doc
                    .set_pseudo_state(frame, PseudoStateFlags::FOCUS, deco.focused);

                // Pin (always-on-top) active class.
                let pin_id = format!("window-deco-{}-pin", deco.id);
                if let Some(pin) = self.desktop_dom.doc.get_element_by_id(&pin_id) {
                    let has_active = self
                        .desktop_dom
                        .doc
                        .get(pin)
                        .map(|n| n.has_class("active"))
                        .unwrap_or(false);
                    if deco.topmost && !has_active {
                        self.desktop_dom.doc.add_class(pin, "active");
                    } else if !deco.topmost && has_active {
                        self.desktop_dom.doc.remove_class(pin, "active");
                    }
                }

                // Title text — only update when it differs.
                let title_id = format!("window-deco-{}-title", deco.id);
                if let Some(title_el) = self.desktop_dom.doc.get_element_by_id(&title_id) {
                    let current = self
                        .desktop_dom
                        .doc
                        .children(title_el)
                        .first()
                        .and_then(|&c| self.desktop_dom.doc.get(c))
                        .and_then(|n| n.text_content().map(str::to_string));
                    match current {
                        Some(t) if t == deco.title => {}
                        Some(_) => {
                            let child = self.desktop_dom.doc.children(title_el)[0];
                            self.desktop_dom.doc.set_text_content(child, &deco.title);
                        }
                        None => {
                            let txt = self.desktop_dom.doc.create_text(&deco.title);
                            self.desktop_dom.doc.append_child(title_el, txt);
                        }
                    }
                }
            }
        }

        // Reconcile: tear down decoration frames for windows that are no longer
        // visible/decorated (closed, minimized, undecorated). Without this a
        // stale frame would leak (and keep a hit-test box) after the window goes
        // away.
        let stale: Vec<u64> = self
            .live_decoration_ids()
            .into_iter()
            .filter(|id| !wanted.contains(id))
            .collect();
        for id in stale {
            let frame_id = format!("window-deco-{id}");
            if let Some(node) = self.desktop_dom.doc.get_element_by_id(&frame_id) {
                if let Some(parent) = self.desktop_dom.doc.parent(node) {
                    self.desktop_dom.doc.remove_child(parent, node);
                }
                self.desktop_dom.doc.destroy_node(node);
            }
        }
    }

    // ══════════════════════════════════════════════════════════
    // App window content — CSS widgets (t108-p8 full-CSS migration)
    // ══════════════════════════════════════════════════════════

    /// Sync the per-window CSS widget content (t108-p8). For every visible window
    /// whose installed `AppView::widget_model()` is `Some`, maintain an
    /// `app-content-host` element (`#app-content-<id>`) inside `workspace-container`
    /// positioned (inline `style`, `position:fixed`) over the window's CONTENT
    /// rect, and mount the model's widgets as a per-window
    /// [`liquide_widgets::WidgetHost`] under it (mirroring the P6 `window-frame`
    /// scaffold). Windows whose `widget_model()` is `None` (terminal /
    /// un-migrated apps) get no host and keep the legacy `AppContentView` scene
    /// path untouched.
    ///
    /// The widget DOM flows through the same CSS pipeline that paints all chrome,
    /// so the laid-out boxes are the single source of truth for paint AND
    /// hit-test (the t86 contract). To preserve the idle full-scene cache (t76)
    /// and frame determinism (e2e_temporal), this writes the DOM ONLY when
    /// something actually changed: a host is (re)mounted only when the model's
    /// STRUCTURE signature changes, and the content-host position is written
    /// through `set_inline_style_if_changed`. A steady-state frame writes
    /// nothing, so `doc.dirty` stays empty and the idle cache holds. This NEVER
    /// calls `mark_full_scene_dirty`; an app-content change bumps the per-window
    /// `app_content_rev` instead (via `mark_app_content_dirty`).
    pub(crate) fn sync_app_widget_content(&mut self) {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let title_h = self.decoration_style.title_bar_height.round() as i32;

        // Snapshot per-window content geometry + the widget model first (immutable
        // borrows), so the mount/patch pass can borrow the DOM + host mutably
        // without overlapping the window/app-view borrows.
        struct ContentState {
            id: WindowIdLocal,
            x: i32,
            y: i32,
            w: i32,
            h: i32,
            model: liquide_interop::AppWidgetModel,
            sig: u64,
        }
        // Local alias to avoid importing the type at module scope.
        type WindowIdLocal = crate::window::WindowId;

        let states: Vec<ContentState> = self
            .visible_windows()
            .iter()
            .filter_map(|w| {
                let view = self.app_views.get(&w.id)?;
                let model = view.widget_model()?;
                let decorated = w.flags.contains(crate::window::WindowFlags::DECORATED);
                let t = if decorated { title_h } else { 0 };
                let mut hasher = DefaultHasher::new();
                // Structure-only signature: the variant shape + keys, NOT the
                // mutable per-widget values (those re-render in place after an
                // action). Hashing the full model is acceptable here because the
                // remount path is gated on a CHANGE; a stable model hashes the
                // same every frame and never remounts.
                crate::app_widgets::model_structure(&model).hash(&mut hasher);
                Some(ContentState {
                    id: w.id,
                    x: w.bounds.x.round() as i32,
                    y: (w.bounds.y.round() as i32) + t,
                    w: w.bounds.width.round() as i32,
                    h: (w.bounds.height.round() as i32 - t).max(0),
                    model,
                    sig: hasher.finish(),
                })
            })
            .collect();

        let mut wanted: HashSet<u64> = HashSet::new();
        for st in &states {
            wanted.insert(st.id.0);
            let host_id = format!("app-content-{}", st.id.0);

            // Create the content-host element once if it does not exist yet.
            let host_node = match self.desktop_dom.doc.get_element_by_id(&host_id) {
                Some(n) => n,
                None => {
                    // Mount under the DOM ROOT (not `workspace-container`). A
                    // `position:fixed` element's layout box is resolved against the
                    // viewport, but the hit-test point-query accumulates each
                    // ancestor's content offset as it descends — so a fixed element
                    // mounted under the offset `workspace-container` would have its
                    // HIT box shifted by that container's origin while its LAYOUT /
                    // paint box (and `bounds_for_node`) stay at true screen coords,
                    // making box-query and dispatcher-hit DIVERGE (the same engine
                    // quirk the P6 deco frame sidesteps by reading `bounds_for_node`
                    // directly instead of point-hit-testing through the subtree).
                    // The chrome overlays that DO rely on point-hit (launcher /
                    // menus) are all mounted under root, where the accumulated
                    // offset is (0,0) and fixed coords agree. Mounting here keeps
                    // box-query == dispatcher-hit for the widgets.
                    let root = self.desktop_dom.doc.root();
                    let el = self.desktop_dom.doc.create_element("app-content-host");
                    self.desktop_dom.doc.set_id(el, &host_id);
                    self.desktop_dom
                        .doc
                        .set_attribute(el, "data-window-id", &st.id.0.to_string());
                    // Structural positioning is set INLINE (not via a theme rule)
                    // so the content host lays out over the window content rect in
                    // SCREEN coordinates (fixed, like the P6 deco frame) and clips
                    // widget overflow to the content rect (composing with the
                    // inescapable scissor) regardless of which theme is loaded.
                    self.desktop_dom
                        .doc
                        .set_inline_style(el, "position", "fixed");
                    self.desktop_dom
                        .doc
                        .set_inline_style(el, "overflow", "hidden");
                    self.desktop_dom.doc.append_child(root, el);
                    el
                }
            };

            // Keep the host positioned over the window content rect (fixed →
            // screen coordinates, like the P6 decoration frame). Change-guarded.
            Self::set_inline_style_if_changed(
                &mut self.desktop_dom.doc,
                host_node,
                "left",
                &st.x.to_string(),
            );
            Self::set_inline_style_if_changed(
                &mut self.desktop_dom.doc,
                host_node,
                "top",
                &st.y.to_string(),
            );
            Self::set_inline_style_if_changed(
                &mut self.desktop_dom.doc,
                host_node,
                "width",
                &st.w.to_string(),
            );
            Self::set_inline_style_if_changed(
                &mut self.desktop_dom.doc,
                host_node,
                "height",
                &st.h.to_string(),
            );

            // (Re)mount the widgets only when the model STRUCTURE changed (first
            // mount, or an external structural change). A stable model hashes the
            // same every frame → no DOM write → idle cache holds.
            let needs_mount = self.app_widget_sigs.get(&st.id) != Some(&st.sig)
                || !self.app_widget_hosts.contains_key(&st.id);
            if needs_mount {
                // Tear down any previous host subtree + host state for this window.
                let children: Vec<NodeId> =
                    self.desktop_dom.doc.children(host_node).to_vec();
                for child in children {
                    self.desktop_dom.doc.remove_child(host_node, child);
                    self.desktop_dom.doc.destroy_node(child);
                }
                let mut host = liquide_widgets::WidgetHost::new();
                crate::app_widgets::mount_model_into(
                    &st.model,
                    st.id.0,
                    host_node,
                    &mut host,
                    &mut self.desktop_dom.doc,
                    &mut self.event_dispatcher,
                );
                self.app_widget_hosts.insert(st.id, host);
                self.app_widget_sigs.insert(st.id, st.sig);
            }
        }

        // Reconcile: tear down content hosts for windows that are no longer
        // visible / widget-backed (closed, minimized, switched to text path).
        let stale: Vec<u64> = self
            .live_app_content_ids()
            .into_iter()
            .filter(|id| !wanted.contains(id))
            .collect();
        for id in stale {
            let host_id = format!("app-content-{id}");
            if let Some(node) = self.desktop_dom.doc.get_element_by_id(&host_id) {
                if let Some(parent) = self.desktop_dom.doc.parent(node) {
                    self.desktop_dom.doc.remove_child(parent, node);
                }
                self.desktop_dom.doc.destroy_node(node);
            }
            let wid = crate::window::WindowId(id);
            self.app_widget_hosts.remove(&wid);
            self.app_widget_sigs.remove(&wid);
        }
    }

    /// Drive every widget-backed window's [`liquide_widgets::WidgetHost`] for one
    /// frame (t108-p8): drain the events the real `EventDispatcher` queued into
    /// the host (clicks/scroll on a widget) plus any focused-window keyboard,
    /// translate each emitted [`liquide_widgets::WidgetAction`] into an
    /// [`liquide_interop::AppWidgetAction`], feed it to the window's
    /// `AppView::apply_action`, and — when the model changed — re-render the acted
    /// widget so its DOM reflects the new state.
    ///
    /// This is the action → model → re-render loop. It uses the live
    /// `hit_test_engine` (the laid-out tree the user actually interacted with) so
    /// all widget hit-geometry is layout-derived (never a constant). A steady
    /// frame with no queued events drains nothing, applies nothing, and writes no
    /// DOM, so the idle cache holds.
    ///
    /// Returns `true` if any model changed (so the caller can bump the relevant
    /// per-window app-content revision / mark the window scene dirty).
    pub(crate) fn drive_app_widget_hosts(&mut self) -> bool {
        let Some(hit_test) = self.hit_test_engine.take() else {
            return false;
        };
        let mut any_changed = false;
        let window_ids: Vec<crate::window::WindowId> =
            self.app_widget_hosts.keys().copied().collect();

        for wid in window_ids {
            // Take the host out so we can borrow it mutably alongside the doc /
            // app-view / dispatcher (all disjoint Shell fields).
            let Some(mut host) = self.app_widget_hosts.remove(&wid) else {
                continue;
            };

            // 1. Drain queued pointer events against the behaviors (this also
            //    re-renders any widget whose own interaction state changed, e.g. a
            //    checkbox flipping `:checked`).
            let mut actions =
                host.process_pending(&mut self.desktop_dom.doc, &hit_test);

            // 2. Route the focused widget's keyboard, when this is the focused
            //    window and a widget owns DOM focus.
            if self.focus.focused() == Some(wid) {
                for key in std::mem::take(&mut self.pending_widget_keys) {
                    let mut k =
                        host.on_keyboard(key, &mut self.desktop_dom.doc, &hit_test);
                    actions.append(&mut k);
                }
            }

            // 3. Translate + apply each action to the app model, re-rendering the
            //    acted widget on a real change so the DOM reflects the new state.
            let mut this_window_changed = false;
            if !actions.is_empty() {
                if let Some(view) = self.app_views.get_mut(&wid) {
                    // Re-fetch the model once so translate_action can pick the verb
                    // by the target widget's family.
                    let mut model = view.widget_model();
                    for action in &actions {
                        let app_key =
                            crate::app_widgets::strip_widget_id(wid.0, &action.widget);
                        let model_widget = model
                            .as_mut()
                            .and_then(|m| m.find_mut(&app_key))
                            .map(|w| &*w);
                        let translated = crate::app_widgets::translate_action(
                            &app_key,
                            model_widget,
                            action,
                        );
                        if view.apply_action(&translated) {
                            this_window_changed = true;
                            // Reconcile: re-render the acted widget from its host
                            // state (which already mutated) so the DOM is current.
                            // Host-owned state survives because the widget id is
                            // stable across reconciliation.
                            host.rerender(&action.widget, &mut self.desktop_dom.doc);
                        }
                    }
                }
            }

            self.app_widget_hosts.insert(wid, host);

            if this_window_changed {
                self.bump_app_content_rev(wid);
                any_changed = true;
            }
        }

        self.hit_test_engine = Some(hit_test);
        // Drop any keys that were queued for a window whose host did not drain
        // them this frame (e.g. focus moved away between the keypress and the
        // drive), so stale keys never replay into the wrong widget next frame.
        self.pending_widget_keys.clear();
        any_changed
    }

    /// Window ids that currently have a live `app-content-host` element mounted
    /// under the DOM root.
    fn live_app_content_ids(&self) -> Vec<u64> {
        let root = self.desktop_dom.doc.root();
        self.desktop_dom
            .doc
            .children(root)
            .iter()
            .filter_map(|&child| {
                let node = self.desktop_dom.doc.get(child)?;
                if node.tag_name() != "app-content-host" {
                    return None;
                }
                node.element_id
                    .as_deref()?
                    .strip_prefix("app-content-")?
                    .parse::<u64>()
                    .ok()
            })
            .collect()
    }

    /// Set an inline style property ONLY when its value actually differs from
    /// the current one, so an idle frame writing the same geometry leaves the
    /// node clean (preserving the idle full-scene cache, t76). Mirrors
    /// `DesktopDocument::set_attr_if_changed` for inline styles.
    fn set_inline_style_if_changed(
        doc: &mut liquide_dom::Document,
        node: liquide_dom::NodeId,
        property: &str,
        value: &str,
    ) {
        if doc.get_inline_style(node, property).as_deref() == Some(value) {
            return;
        }
        doc.set_inline_style(node, property, value);
    }

    /// Window ids that currently have a live `window-frame` decoration element
    /// mounted in the workspace container.
    fn live_decoration_ids(&self) -> Vec<u64> {
        let workspace = self.desktop_dom.workspace;
        self.desktop_dom
            .doc
            .children(workspace)
            .iter()
            .filter_map(|&child| {
                let node = self.desktop_dom.doc.get(child)?;
                if node.tag_name() != "window-frame" {
                    return None;
                }
                node.element_id
                    .as_deref()?
                    .strip_prefix("window-deco-")?
                    .parse::<u64>()
                    .ok()
            })
            .collect()
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
        // The show-delay / fade lifecycle is owned by the canonical
        // `liquide-tooltip` TooltipManager (t51-e9). t51-e15 retired the former
        // hand-rolled 400 ms `tooltip_timer_us` dwell: the render path is now
        // the authoritative per-frame driver of the manager (so the tooltip
        // resolves correctly regardless of tick↔render ordering across the
        // render-thread boundary), and the render gate is the manager's
        // visibility. `tooltip_text` remains the rendered content and
        // `tooltip_pos` the anchor; only the *when-to-show* decision moved to
        // the manager. `sync_tooltip_manager` applies the hover transition
        // before advancing the timers — the F07-safe order (see
        // `tooltip_adapter`).
        self.sync_tooltip_manager(self.frame_delta_ms);
        if self.tooltip_manager_visible() {
            self.mark_wired(crate::shell::WiringBit::Tooltip);
            // Cannot fail while visible: the manager only reports visible while
            // a hover label is present, but guard defensively rather than panic.
            let Some(text) = self.tooltip_text.clone() else {
                self.remove_overlay("shell-tooltip");
                self.template_cache.remove("tooltip");
                return;
            };

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

    fn clear_notification_template_cache(&mut self) {
        self.template_cache.remove("notifications");
        self.template_cache
            .retain(|key, _| !key.starts_with(NOTIFICATION_ITEM_CACHE_PREFIX));
    }

    fn remove_stale_template_children(&mut self, parent: NodeId, desired_nodes: &[NodeId]) {
        let desired: HashSet<NodeId> = desired_nodes.iter().copied().collect();
        let children: Vec<_> = self.desktop_dom.doc.children(parent).to_vec();
        for child in children {
            if !desired.contains(&child) {
                self.desktop_dom.doc.remove_child(parent, child);
                self.desktop_dom.doc.destroy_node(child);
            }
        }
    }

    fn order_template_children(&mut self, parent: NodeId, desired_nodes: &[NodeId]) {
        for (index, &node) in desired_nodes.iter().enumerate() {
            let children: Vec<_> = self.desktop_dom.doc.children(parent).to_vec();
            if children.get(index).copied() == Some(node) {
                continue;
            }
            if let Some(before) = children.get(index).copied() {
                if before != node {
                    self.desktop_dom.doc.insert_before(parent, node, before);
                }
            } else {
                self.desktop_dom.doc.append_child(parent, node);
            }
        }
    }

    fn replace_template_children(&mut self, parent: NodeId, html: &str) {
        let children: Vec<_> = self.desktop_dom.doc.children(parent).to_vec();
        for child in children {
            self.desktop_dom.doc.remove_child(parent, child);
            self.desktop_dom.doc.destroy_node(child);
        }
        parse_html_into(&mut self.desktop_dom.doc, parent, html);
    }

    /// Render a template and replace the children of the element with the given
    /// DOM id.  Skips the DOM rebuild if the rendered HTML hasn't changed.
    ///
    /// When the rendered HTML differs only in text content and/or a small set
    /// of attributes (e.g. the once-per-minute clock tick, a changing unread
    /// count), the changed nodes are patched **in place** instead of tearing
    /// down and reparsing the whole subtree.  This keeps the styling/layout of
    /// unchanged siblings intact (so the pipeline fast path can survive a clock
    /// tick) and avoids re-requesting glyphs for text that did not change.
    /// A structural change (added/removed/reordered/retagged nodes) still falls
    /// back to the full teardown + reparse for correctness.
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

        // Find the target element and update its children.
        if let Some(node_id) = self.desktop_dom.doc.get_element_by_id(element_id) {
            // Try an in-place patch first — only tears down + reparses when the
            // structure actually changed.
            if !self.patch_template_children(node_id, &html) {
                let children: Vec<_> = self.desktop_dom.doc.children(node_id).to_vec();
                for child in children {
                    self.desktop_dom.doc.remove_child(node_id, child);
                    self.desktop_dom.doc.destroy_node(child);
                }
                parse_html_into(&mut self.desktop_dom.doc, node_id, &html);
            }
        }

        self.template_cache.insert(template_name.to_string(), html);
    }

    /// Attempt to patch the children of `parent` to match `html` **in place**.
    ///
    /// Parses `html` into a scratch document and structurally diffs it against
    /// the live subtree rooted at `parent`.  If (and only if) the two trees
    /// have identical structure — same tags, same `id`/class sets, same child
    /// counts at every level — the differing text content and attribute values
    /// are applied directly to the existing live nodes (via `set_text_content`
    /// / `set_attribute` / `remove_attribute`), preserving node identity and
    /// the cached style/layout of everything that did not change.
    ///
    /// Returns `true` when the subtree was patched in place, `false` when a
    /// structural difference means the caller must fall back to a full rebuild.
    fn patch_template_children(&mut self, parent: NodeId, html: &str) -> bool {
        use liquide_dom::html_parser::parse_html;

        let new_doc = parse_html(html);
        let new_root = new_doc.root();

        let live_children: Vec<NodeId> = self.desktop_dom.doc.children(parent).to_vec();
        let new_children: Vec<NodeId> = new_doc.children(new_root).to_vec();

        if live_children.len() != new_children.len() {
            return false;
        }

        // First pass: verify the entire pairing is structurally compatible
        // before mutating anything, so a deep structural mismatch never leaves
        // the live tree half-patched.
        for (&live, &new) in live_children.iter().zip(new_children.iter()) {
            if !Self::subtrees_structurally_match(&self.desktop_dom.doc, live, &new_doc, new) {
                return false;
            }
        }

        // Second pass: apply text/attribute differences in place.
        for (&live, &new) in live_children.iter().zip(new_children.iter()) {
            self.patch_node_in_place(live, &new_doc, new);
        }

        true
    }

    /// Structural-only comparison: do `live` (in the live doc) and `new` (in
    /// `new_doc`) describe the same tree shape — same kind, same tag, same
    /// `id`, same class set, same child count, recursively?  Text content and
    /// attribute *values* are intentionally ignored here (those are patchable).
    fn subtrees_structurally_match(
        live_doc: &liquide_dom::Document,
        live: NodeId,
        new_doc: &liquide_dom::Document,
        new: NodeId,
    ) -> bool {
        use liquide_dom::node::NodeData;

        let (Some(ln), Some(nn)) = (live_doc.get(live), new_doc.get(new)) else {
            return false;
        };

        // Node kind must match (Element vs Text vs Image vs …). We only patch
        // Element and Text nodes in place; any other kind must structurally
        // match by discriminant and is otherwise left alone.
        match (&ln.data, &nn.data) {
            (NodeData::Text(_), NodeData::Text(_)) => return true,
            (NodeData::Element, NodeData::Element) => {}
            // Same non-element/non-text kind: require an exact value match,
            // because we have no in-place patch for these (Image src, etc.).
            (a, b) => {
                return std::mem::discriminant(a) == std::mem::discriminant(b)
                    && Self::node_data_equal(a, b);
            }
        }

        // Element: tag, id and class set must be identical (those drive
        // selector matching, so a change there is a real structural/style
        // change that warrants a rebuild path).
        if ln.tag != nn.tag {
            return false;
        }
        if ln.element_id != nn.element_id {
            return false;
        }
        if ln.classes != nn.classes {
            return false;
        }

        let live_kids = live_doc.children(live);
        let new_kids = new_doc.children(new);
        if live_kids.len() != new_kids.len() {
            return false;
        }
        for (&lk, &nk) in live_kids.iter().zip(new_kids.iter()) {
            if !Self::subtrees_structurally_match(live_doc, lk, new_doc, nk) {
                return false;
            }
        }
        true
    }

    fn node_data_equal(a: &liquide_dom::node::NodeData, b: &liquide_dom::node::NodeData) -> bool {
        use liquide_dom::node::NodeData;
        match (a, b) {
            (NodeData::Text(x), NodeData::Text(y)) => x == y,
            (NodeData::Comment(x), NodeData::Comment(y)) => x == y,
            (
                NodeData::Image { src: s1, alt: a1, .. },
                NodeData::Image { src: s2, alt: a2, .. },
            ) => s1 == s2 && a1 == a2,
            (NodeData::Surface { surface_id: x }, NodeData::Surface { surface_id: y }) => x == y,
            _ => std::mem::discriminant(a) == std::mem::discriminant(b),
        }
    }

    /// Apply text-content and attribute differences from `new` (in `new_doc`)
    /// onto `live` (in the live doc).  Assumes the two subtrees have already
    /// been verified structurally compatible by
    /// [`Self::subtrees_structurally_match`].
    fn patch_node_in_place(&mut self, live: NodeId, new_doc: &liquide_dom::Document, new: NodeId) {
        use liquide_dom::node::NodeData;

        // Snapshot the new node's data we need, then drop the borrow before we
        // mutate the live doc.
        let (new_text, new_attrs): (Option<String>, Vec<(String, String)>) = {
            let Some(nn) = new_doc.get(new) else {
                return;
            };
            let text = match &nn.data {
                NodeData::Text(s) => Some(s.clone()),
                _ => None,
            };
            let attrs: Vec<(String, String)> = nn
                .attrs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            (text, attrs)
        };

        // Text node: update content only when it actually changed (so an
        // unchanged text node stays clean and does not re-request glyphs).
        if let Some(new_text) = new_text {
            let changed = self
                .desktop_dom
                .doc
                .get(live)
                .and_then(|n| match &n.data {
                    NodeData::Text(s) => Some(s.as_str() != new_text),
                    _ => Some(true),
                })
                .unwrap_or(true);
            if changed {
                self.desktop_dom.doc.set_text_content(live, &new_text);
            }
            return;
        }

        // Element: diff attributes (set changed/added, remove deleted).
        let live_attrs: Vec<(String, Option<String>)> = self
            .desktop_dom
            .doc
            .get(live)
            .map(|n| {
                n.attrs
                    .iter()
                    .map(|(k, v)| (k.to_string(), Some(v.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        for (key, new_val) in &new_attrs {
            let current = self.desktop_dom.doc.get_attribute(live, key);
            if current.as_deref() != Some(new_val.as_str()) {
                self.desktop_dom.doc.set_attribute(live, key, new_val);
            }
        }
        // Remove attributes that no longer exist in the new node.
        for (key, _) in &live_attrs {
            if !new_attrs.iter().any(|(k, _)| k == key) {
                self.desktop_dom.doc.remove_attribute(live, key);
            }
        }

        // Recurse into children (counts already verified equal).
        let live_kids: Vec<NodeId> = self.desktop_dom.doc.children(live).to_vec();
        let new_kids: Vec<NodeId> = new_doc.children(new).to_vec();
        for (lk, nk) in live_kids.into_iter().zip(new_kids.into_iter()) {
            self.patch_node_in_place(lk, new_doc, nk);
        }
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

        // Try to patch the existing overlay in place first (e.g. the tooltip's
        // per-frame position attribute / text changes during a fade). Only when
        // there is no existing overlay, or its structure changed, do we tear it
        // down and reparse.
        if !self.patch_overlay_in_place(element_id, &html) {
            // Remove existing overlay
            self.remove_overlay(element_id);

            // Append new overlay to root
            let root = self.desktop_dom.doc.root();
            parse_html_into(&mut self.desktop_dom.doc, root, &html);
        }

        self.template_cache.insert(template_name.to_string(), html);
    }

    /// Attempt to patch an existing overlay element (identified by
    /// `element_id`) to match `html` in place. The overlay's root element in
    /// `html` is matched against the live overlay element of the same id; if
    /// they are structurally compatible, only text/attribute differences are
    /// applied. Returns `false` (caller rebuilds) when the overlay does not yet
    /// exist or its structure changed.
    fn patch_overlay_in_place(&mut self, element_id: &str, html: &str) -> bool {
        use liquide_dom::html_parser::parse_html;

        let Some(live) = self.desktop_dom.doc.get_element_by_id(element_id) else {
            return false;
        };

        let new_doc = parse_html(html);
        let Some(new) = new_doc.get_element_by_id(element_id) else {
            return false;
        };

        if !Self::subtrees_structurally_match(&self.desktop_dom.doc, live, &new_doc, new) {
            return false;
        }
        self.patch_node_in_place(live, &new_doc, new);
        true
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

}

#[cfg(test)]
mod dom_sync_escape_tests {
    use super::{TemplateContext, render_tray_item};

    /// Regression: T49-e5-F06 — the hand-built tray-item attribute HTML must
    /// HTML-escape every untrusted value so a malicious app title / tooltip / id
    /// cannot break out of an attribute and inject elements into the shell DOM.
    #[test]
    fn tray_item_attributes_are_html_escaped() {
        let mut tray = TemplateContext::new();
        // An app title that tries to close the attribute and inject an element.
        tray.set("id", "evil\"><script>alert(1)</script>");
        tray.set("source", "seamless");
        tray.set("label", "Title & \"co\" <b>");
        tray.set("tooltip", "tip > here & \"there\"");
        tray.set("classes", "evil\" onclick=\"x");
        tray.set("badge", "<img src=x>");
        tray.set("has_badge", true);

        let mut out = String::new();
        render_tray_item(&tray, &mut out);

        // No raw injection survives anywhere in the built markup.
        assert!(
            !out.contains("<script>"),
            "raw <script> leaked into tray HTML: {out}"
        );
        assert!(
            !out.contains("<img src=x>"),
            "raw <img> leaked into tray badge: {out}"
        );
        // The malicious id no longer closes the attribute.
        assert!(!out.contains("id=\"evil\">"), "attribute break-out: {out}");
        // The dangerous characters are entity-encoded.
        assert!(out.contains("&lt;script&gt;"));
        assert!(out.contains("Title &amp; &quot;co&quot; &lt;b&gt;"));
        assert!(out.contains("onclick=&quot;x"));
        assert!(out.contains("&lt;img src=x&gt;"));
    }

    /// A plain tray item passes through unchanged (no double-escaping/mangling).
    #[test]
    fn plain_tray_item_passes_through_unchanged() {
        let mut tray = TemplateContext::new();
        tray.set("id", "tray-item-clock");
        tray.set("source", "seamless");
        tray.set("label", "Clock");
        tray.set("tooltip", "12:00 PM");
        tray.set("classes", "seamless");

        let mut out = String::new();
        render_tray_item(&tray, &mut out);

        assert_eq!(
            out,
            "<status-tray-item id=\"tray-item-clock\" data-source=\"seamless\" \
             data-label=\"Clock\" data-tooltip=\"12:00 PM\" class=\"seamless\">\
             </status-tray-item>"
        );
    }
}
