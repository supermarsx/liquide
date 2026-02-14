//! DOM-based rendering for the status bar.
//!
//! Builds a DOM subtree representing the status bar layout (app menu,
//! menu bar items on the left; system indicators + theme toggle on
//! the right). The subtree integrates with `liquide-style-engine` to
//! resolve styles via CSS instead of the legacy `UiTheme` immediate-mode
//! painter.

use liquide_dom::{Document, NodeId, PseudoStateFlags};

use crate::indicator::{IndicatorKind, SystemIndicator};
use crate::status_bar::StatusBar;

/// Build a DOM subtree for the status bar and return the root node id.
///
/// ```text
/// <statusbar id="css-statusbar">
///   <statusbar-slot class="left">
///     <statusbar-item class="app-menu"> AppName </statusbar-item>
///     <statusbar-item class="menu-bar-item"> File </statusbar-item>
///     …
///   </statusbar-slot>
///   <statusbar-slot class="center" />
///   <statusbar-slot class="right">
///     <statusbar-item class="indicator clock"> 14:30 </statusbar-item>
///     <statusbar-item class="indicator wifi"> … </statusbar-item>
///     <statusbar-item class="theme-toggle"> 🌙 </statusbar-item>
///   </statusbar-slot>
/// </statusbar>
/// ```
pub fn build_statusbar_dom(
    doc: &mut Document,
    parent: NodeId,
    bar: &StatusBar,
) -> StatusBarNodes {
    let statusbar = doc.create_element("statusbar");
    doc.set_id(statusbar, "css-statusbar");
    doc.append_child(parent, statusbar);

    // Left slot: app name + menu bar
    let slot_left = doc.create_element("statusbar-slot");
    doc.add_class(slot_left, "left");
    doc.append_child(statusbar, slot_left);

    // App menu
    if bar.config.show_app_menu {
        let app_el = doc.create_element("statusbar-item");
        doc.add_class(app_el, "app-menu");
        let txt = doc.create_text(&bar.app_menu.app_name);
        doc.append_child(app_el, txt);
        doc.append_child(slot_left, app_el);
    }

    // Menu bar items (File, Edit, View, …)
    for item in &bar.menu_bar.items {
        let el = doc.create_element("statusbar-item");
        doc.add_class(el, "menu-bar-item");
        let txt = doc.create_text(&item.menu.label);
        doc.append_child(el, txt);
        doc.append_child(slot_left, el);
    }

    // Center slot (empty by default — can be used for window title)
    let slot_center = doc.create_element("statusbar-slot");
    doc.add_class(slot_center, "center");
    doc.append_child(statusbar, slot_center);

    // Right slot: indicators + theme toggle
    let slot_right = doc.create_element("statusbar-slot");
    doc.add_class(slot_right, "right");
    doc.append_child(statusbar, slot_right);

    // Theme toggle
    if bar.config.show_theme_toggle {
        let toggle = doc.create_element("statusbar-item");
        doc.add_class(toggle, "theme-toggle");
        let icon = match bar.theme_toggle.current_mode {
            liquide_ui_core::theme::ThemeMode::Dark => "🌙",
            liquide_ui_core::theme::ThemeMode::Light => "☀️",
            liquide_ui_core::theme::ThemeMode::System => "🖥️",
        };
        let txt = doc.create_text(icon);
        doc.append_child(toggle, txt);
        doc.append_child(slot_right, toggle);
    }

    // System indicators (clock, battery, wifi, notifications, volume)
    for indicator in &bar.indicators {
        let el = doc.create_element("statusbar-item");
        doc.add_class(el, "indicator");

        let class = match &indicator.kind {
            IndicatorKind::Clock { .. } => "clock",
            IndicatorKind::Battery { .. } => "battery",
            IndicatorKind::Wifi { .. } => "wifi",
            IndicatorKind::Notification { .. } => "notification",
            IndicatorKind::Volume { .. } => "volume",
        };
        doc.add_class(el, class);

        let label = indicator_label(indicator);
        let txt = doc.create_text(&label);
        doc.append_child(el, txt);
        doc.append_child(slot_right, el);
    }

    StatusBarNodes {
        root: statusbar,
        slot_left,
        slot_center,
        slot_right,
    }
}

/// Sync indicator text content to match the current state.
pub fn sync_statusbar_indicators(
    doc: &mut Document,
    slot_right: NodeId,
    bar: &StatusBar,
) {
    // Find all indicator children and update text
    let children: Vec<NodeId> = doc.children(slot_right).to_vec();
    let mut indicator_idx = 0;

    for child in children {
        let is_indicator = doc
            .get(child)
            .map_or(false, |n| n.has_class("indicator"));
        if !is_indicator {
            continue;
        }
        if let Some(indicator) = bar.indicators.get(indicator_idx) {
            let label = indicator_label(indicator);
            let text_kids: Vec<NodeId> = doc.children(child).to_vec();
            if let Some(&first_txt) = text_kids.first() {
                doc.set_text_content(first_txt, &label);
            }
            indicator_idx += 1;
        }
    }
}

/// References to the key status bar DOM nodes.
pub struct StatusBarNodes {
    pub root: NodeId,
    pub slot_left: NodeId,
    pub slot_center: NodeId,
    pub slot_right: NodeId,
}

fn indicator_label(ind: &SystemIndicator) -> String {
    match &ind.kind {
        IndicatorKind::Clock { timestamp_us, format } => {
            let total_secs = *timestamp_us / 1_000_000;
            let hours = (total_secs / 3600) % 24;
            let minutes = (total_secs / 60) % 60;
            format!("{hours:02}:{minutes:02}")
        }
        IndicatorKind::Battery { percent, .. } => format!("{}%", percent),
        IndicatorKind::Wifi { quality_percent, .. } => format!("{}%", quality_percent),
        IndicatorKind::Notification { unread_count, .. } => {
            if *unread_count > 0 {
                format!("{}", unread_count)
            } else {
                String::new()
            }
        }
        IndicatorKind::Volume { level, .. } => format!("{}%", level),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_statusbar_dom() {
        let bar = StatusBar::default();
        let mut doc = Document::new();
        let root = doc.root();

        let nodes = build_statusbar_dom(&mut doc, root, &bar);

        // Root is a statusbar element
        assert!(doc.get(nodes.root).is_some());

        // Has three slot children
        assert_eq!(doc.children(nodes.root).len(), 3);

        // Left slot has app-menu + menu bar items
        let left_count = doc.children(nodes.slot_left).len();
        assert!(left_count >= 1); // at least app-menu

        // Right slot has indicators
        let right_count = doc.children(nodes.slot_right).len();
        assert!(right_count >= 1); // at least one indicator
    }
}
