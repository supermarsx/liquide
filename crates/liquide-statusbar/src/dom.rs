//! DOM-based rendering for the status bar.
//!
//! Builds a DOM subtree representing the status bar layout (app menu,
//! menu bar items on the left; system indicators + theme toggle on
//! the right). The subtree integrates with `liquide-style-engine` to
//! resolve styles via CSS instead of the legacy `UiTheme` immediate-mode
//! painter.

use liquide_dom::{Document, NodeId};

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
pub fn build_statusbar_dom(doc: &mut Document, parent: NodeId, bar: &StatusBar) -> StatusBarNodes {
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
pub fn sync_statusbar_indicators(doc: &mut Document, slot_right: NodeId, bar: &StatusBar) {
    // Find all indicator children and update text
    let children: Vec<NodeId> = doc.children(slot_right).to_vec();
    let mut indicator_idx = 0;

    for child in children {
        let is_indicator = doc.get(child).map_or(false, |n| n.has_class("indicator"));
        if !is_indicator {
            continue;
        }
        if let Some(indicator) = bar.indicators.get(indicator_idx) {
            let label = indicator_label(indicator);
            // Only the clock is provably size-stable: `indicator_label` formats it
            // as a fixed-width `HH:MM` (zero-padded, always 5 chars), so a per-frame
            // tick can never change the box's intrinsic width and we can demote the
            // update from LAYOUT to PAINT (t136/t142). Battery/Wi-Fi/volume render
            // `{}%` and the notification count are variable-width, so they stay on
            // the conservative LAYOUT path (a wider value CAN reflow the slot).
            let size_stable = matches!(indicator.kind, IndicatorKind::Clock { .. });
            let text_kids: Vec<NodeId> = doc.children(child).to_vec();
            if let Some(&first_txt) = text_kids.first() {
                set_text_content_sized(doc, first_txt, &label, size_stable);
            }
            indicator_idx += 1;
        }
    }
}

/// Apply a text-content change to an existing text node, optionally demoting the
/// dirty scope to **paint-only** when the author guarantees the box is size-stable.
///
/// This mirrors `liquide_components::TemplateRenderer::patch_text` (t136) but
/// operates directly on `liquide_dom` so the status-bar's hand-built DOM path can
/// use the fast path without depending on the template engine. `set_text_content`
/// always marks the node LAYOUT-dirty (the safe default, since a text change can
/// alter intrinsic width). When `size_stable` is set, the box provably cannot
/// move, so we remove the node from the LAYOUT scope (document + node flag) and
/// leave it PAINT-dirty: the pipeline skips layout and just repaints, and mutation
/// observers (scene/hit-test) still fire because we go through `set_text_content`.
fn set_text_content_sized(doc: &mut Document, node_id: NodeId, text: &str, size_stable: bool) {
    // No-op guard: re-setting identical content must not dirty anything.
    if doc.get(node_id).and_then(|n| n.text_content()) == Some(text) {
        return;
    }

    // Canonical mutator: updates node data, notifies observers, marks LAYOUT.
    doc.set_text_content(node_id, text);

    if !size_stable {
        return;
    }

    // Size-stable opt-in: demote LAYOUT → PAINT (document scope + node flags).
    doc.dirty.layout.remove(&node_id);
    doc.dirty.paint.insert(node_id);
    if let Some(node) = doc.get_mut(node_id) {
        node.dirty.clear_layout();
        node.dirty.mark_paint_dirty();
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
        IndicatorKind::Clock {
            timestamp_us,
            format: _,
        } => {
            let total_secs = *timestamp_us / 1_000_000;
            let hours = (total_secs / 3600) % 24;
            let minutes = (total_secs / 60) % 60;
            format!("{hours:02}:{minutes:02}")
        }
        IndicatorKind::Battery { percent, .. } => format!("{}%", percent),
        IndicatorKind::Wifi {
            quality_percent, ..
        } => format!("{}%", quality_percent),
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

    // ── t142: clock indicator text update is paint-only ──────────────

    use crate::indicator::SystemIndicator;

    /// Locate the first text-node child of the indicator with class `cls`.
    fn indicator_text_node(doc: &Document, slot_right: NodeId, cls: &str) -> NodeId {
        for &child in doc.children(slot_right) {
            if doc.get(child).map_or(false, |n| n.has_class(cls)) {
                return *doc
                    .children(child)
                    .first()
                    .expect("indicator must have a text child");
            }
        }
        panic!("indicator with class `{cls}` not found");
    }

    /// Build a status bar with both a fixed-width clock and a variable-width
    /// battery so the test can prove selectivity.
    fn bar_with_clock_and_battery() -> StatusBar {
        let mut bar = StatusBar::default();
        // Replace indicators with a deterministic clock + battery pair.
        bar.indicators = vec![SystemIndicator::clock(), SystemIndicator::battery(85)];
        bar
    }

    #[test]
    fn clock_text_update_is_paint_only_not_layout() {
        let mut bar = bar_with_clock_and_battery();
        let mut doc = Document::new();
        let root = doc.root();
        let nodes = build_statusbar_dom(&mut doc, root, &bar);

        let clock_txt = indicator_text_node(&doc, nodes.slot_right, "clock");

        // Clear dirty from construction.
        doc.dirty.clear_all();
        if let Some(n) = doc.get_mut(clock_txt) {
            n.dirty.clear_all();
        }

        // Tick the clock (00:00 → 00:01): same fixed-width "HH:MM" shape.
        bar.update_clock(60 * 1_000_000);
        sync_statusbar_indicators(&mut doc, nodes.slot_right, &bar);

        // Content updated (still rendered).
        assert_eq!(
            doc.get(clock_txt).and_then(|n| n.text_content()),
            Some("00:01")
        );

        // The clock text update must be PAINT, NOT LAYOUT.
        assert!(
            !doc.dirty.layout.contains(&clock_txt),
            "size-stable clock tick must not mark LAYOUT-dirty"
        );
        assert!(
            !doc.get(clock_txt).unwrap().dirty.needs_layout(),
            "clock text node must not carry the LAYOUT flag"
        );
        assert!(
            doc.dirty.paint.contains(&clock_txt),
            "clock tick must still mark PAINT-dirty so it repaints"
        );
        assert!(
            doc.get(clock_txt).unwrap().dirty.needs_paint(),
            "clock text node must carry the PAINT flag"
        );
    }

    #[test]
    fn variable_width_indicator_still_marks_layout() {
        // SELECTIVITY / teeth: a battery `{}%` value can change width (85% → 100%)
        // and reflow the slot, so it MUST stay on the conservative LAYOUT path.
        // RED if someone blanket-marks every indicator paint-only.
        let mut bar = bar_with_clock_and_battery();
        let mut doc = Document::new();
        let root = doc.root();
        let nodes = build_statusbar_dom(&mut doc, root, &bar);

        let battery_txt = indicator_text_node(&doc, nodes.slot_right, "battery");
        doc.dirty.clear_all();
        if let Some(n) = doc.get_mut(battery_txt) {
            n.dirty.clear_all();
        }

        // Battery goes 85% → 100% (3 chars → 4 chars: a real width change).
        if let crate::indicator::IndicatorKind::Battery { ref mut percent, .. } =
            bar.indicators[1].kind
        {
            *percent = 100;
        }
        sync_statusbar_indicators(&mut doc, nodes.slot_right, &bar);

        assert_eq!(
            doc.get(battery_txt).and_then(|n| n.text_content()),
            Some("100%")
        );
        assert!(
            doc.dirty.layout.contains(&battery_txt),
            "a variable-width indicator must mark LAYOUT-dirty"
        );
        assert!(
            doc.get(battery_txt).unwrap().dirty.needs_layout(),
            "a variable-width indicator must carry the LAYOUT flag"
        );
    }

    #[test]
    fn identical_clock_text_does_not_dirty_anything() {
        // Re-syncing the SAME clock value must be a no-op (no dirty churn).
        let bar = bar_with_clock_and_battery();
        let mut doc = Document::new();
        let root = doc.root();
        let nodes = build_statusbar_dom(&mut doc, root, &bar);

        let clock_txt = indicator_text_node(&doc, nodes.slot_right, "clock");
        doc.dirty.clear_all();
        if let Some(n) = doc.get_mut(clock_txt) {
            n.dirty.clear_all();
        }

        // No timestamp change → same "00:00" text.
        sync_statusbar_indicators(&mut doc, nodes.slot_right, &bar);

        assert!(!doc.dirty.layout.contains(&clock_txt));
        assert!(!doc.dirty.paint.contains(&clock_txt));
    }
}
