//! Window frame decoration hit-test adapter (t103-p6 full-CSS migration).
//!
//! The window titlebar + close/maximize/minimize/pin buttons are now laid out
//! by the CSS pipeline as a `window-frame` DOM subtree (`#window-deco-<id>`,
//! synced by [`Shell::sync_window_decorations`](crate::shell::Shell)). The
//! titlebar drag region and each button's click zone are read from those
//! **laid-out CSS boxes** via the live hit-test engine — NEVER from hardcoded
//! `DecorationStyle` button-stride math. A theme change that moves/resizes the
//! buttons therefore moves the click zones with them (the recurring
//! hit-test-from-CSS-geometry contract, t86).
//!
//! Only the in-DOM zones (titlebar drag + the four buttons) come from this
//! adapter. The RESIZE-edge zones extend *outside* the window box and have no
//! CSS element, so they remain served by the rect-based
//! [`hit_test_decoration`](crate::decoration::hit_test_decoration); callers use
//! this adapter for buttons/titlebar and fall back to the rect math for resize
//! edges and for the first frame before the decoration has been laid out.

use liquide_layout::geometry::Point;

use crate::decoration::HitZone;
use crate::shell::Shell;
use crate::window::WindowId;

/// Per-button decoration element id suffixes mapped to their hit zone. Walked in
/// declaration order; the first laid-out box containing the point wins.
const BUTTON_ZONES: &[(&str, HitZone)] = &[
    ("close", HitZone::CloseButton),
    ("max", HitZone::MaximizeButton),
    ("min", HitZone::MinimizeButton),
    ("pin", HitZone::AlwaysOnTopButton),
];

impl Shell {
    /// Absolute laid-out box of a window's titlebar drag region, read from the
    /// CSS layout (`#window-deco-<id>-titlebar`), or `None` when the decoration
    /// has not been laid out yet.
    ///
    /// This is the painted titlebar box; a click inside it (and not inside a
    /// button) is a titlebar drag — exactly tracking the CSS, so a theme change
    /// that changes the titlebar height moves the drag region.
    #[must_use]
    pub(crate) fn window_titlebar_bounds_from_css(
        &self,
        window_id: WindowId,
    ) -> Option<liquide_layout::geometry::Rect> {
        let el_id = format!("window-deco-{}-titlebar", window_id.0);
        let node = self.desktop_dom.doc.get_element_by_id(&el_id)?;
        self.hit_test_engine.as_ref()?.bounds_for_node(node)
    }

    /// Absolute laid-out box of one of a window's decoration buttons, read from
    /// the CSS layout, or `None` when the button is not laid out.
    ///
    /// `suffix` is one of `close`/`max`/`min`/`pin` (the template element id
    /// suffixes).
    #[must_use]
    pub(crate) fn window_button_bounds_from_css(
        &self,
        window_id: WindowId,
        suffix: &str,
    ) -> Option<liquide_layout::geometry::Rect> {
        let el_id = format!("window-deco-{}-{suffix}", window_id.0);
        let node = self.desktop_dom.doc.get_element_by_id(&el_id)?;
        self.hit_test_engine.as_ref()?.bounds_for_node(node)
    }

    /// Resolve which decoration button (if any) of `window_id` contains
    /// `(x, y)`, reading each button's **laid-out CSS box**.
    ///
    /// Returns the matching [`HitZone`] (CloseButton/MaximizeButton/
    /// MinimizeButton/AlwaysOnTopButton). Returns `None` when no button box
    /// contains the point, or when the decoration is not laid out (first frame)
    /// — the caller then falls back to the rect-based hit-test.
    #[must_use]
    pub(crate) fn window_button_zone_from_css(
        &self,
        window_id: WindowId,
        x: f32,
        y: f32,
    ) -> Option<HitZone> {
        let pt = Point::new(x, y);
        for (suffix, zone) in BUTTON_ZONES {
            if let Some(rect) = self.window_button_bounds_from_css(window_id, suffix) {
                if rect.contains(pt) {
                    return Some(*zone);
                }
            }
        }
        None
    }

    /// Resolve the decoration zone at `(x, y)` for `window_id` from the CSS
    /// layout: a button zone if a button box contains the point, else
    /// [`HitZone::TitleBar`] if the titlebar box contains it. Returns `None`
    /// when neither is laid out / hit — the caller falls back to the rect-based
    /// [`hit_test_decoration`](crate::decoration::hit_test_decoration) (which
    /// also owns the resize-edge zones outside the DOM box).
    #[must_use]
    pub(crate) fn window_decoration_zone_from_css(
        &self,
        window_id: WindowId,
        x: f32,
        y: f32,
    ) -> Option<HitZone> {
        if let Some(zone) = self.window_button_zone_from_css(window_id, x, y) {
            return Some(zone);
        }
        let pt = Point::new(x, y);
        if let Some(tb) = self.window_titlebar_bounds_from_css(window_id) {
            if tb.contains(pt) {
                return Some(HitZone::TitleBar);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_compositor::geometry::Rect;

    /// The REAL shipped stylesheets (variables + components) — the production
    /// source of the `window-frame`/button rules + their custom-property tokens.
    /// Driving them through the pipeline gives the geometry tests teeth (a
    /// regressed dimension on disk moves the laid-out box and the assertions
    /// move with it).
    const VARIABLES_CSS: &str = include_str!("../../../assets/themes/variables.css");
    const COMPONENTS_CSS: &str = include_str!("../../../assets/themes/components.css");

    fn windowed_shell() -> Shell {
        let mut shell = Shell::new(1280.0, 720.0);
        shell.cursor_blink_on = true;
        shell.cursor_blink_time_us = u64::MAX;
        shell.add_stylesheet(VARIABLES_CSS);
        shell.add_stylesheet(COMPONENTS_CSS);
        shell.open_window("Alpha", Rect::new(200.0, 120.0, 640.0, 420.0));
        // Build once so the DOM is synced and the pipeline lays out the
        // decoration, populating the hit-test engine.
        let _ = shell.build_scene();
        shell
    }

    /// The titlebar + each button resolve to a non-degenerate laid-out CSS box.
    #[test]
    fn decoration_boxes_come_from_css_layout() {
        let shell = windowed_shell();
        let wid = shell.visible_windows()[0].id;

        let tb = shell
            .window_titlebar_bounds_from_css(wid)
            .expect("titlebar must have a laid-out CSS box");
        assert!(
            tb.width > 1.0 && tb.height > 1.0,
            "titlebar box must be non-degenerate, got {tb:?}"
        );

        for suffix in ["close", "max", "min", "pin"] {
            let b = shell
                .window_button_bounds_from_css(wid, suffix)
                .unwrap_or_else(|| panic!("{suffix} button must have a laid-out CSS box"));
            assert!(
                b.width > 1.0 && b.height > 1.0,
                "{suffix} button box must be non-degenerate, got {b:?}"
            );
        }
    }

    /// Clicking the center of each button's CSS box resolves to that button's
    /// zone — the hit-test reads the laid-out geometry, not a constant stride.
    #[test]
    fn button_center_resolves_to_its_zone() {
        let shell = windowed_shell();
        let wid = shell.visible_windows()[0].id;

        let cases = [
            ("close", HitZone::CloseButton),
            ("max", HitZone::MaximizeButton),
            ("min", HitZone::MinimizeButton),
            ("pin", HitZone::AlwaysOnTopButton),
        ];
        for (suffix, expected) in cases {
            let b = shell
                .window_button_bounds_from_css(wid, suffix)
                .expect("button box");
            let cx = b.x + b.width / 2.0;
            let cy = b.y + b.height / 2.0;
            assert_eq!(
                shell.window_button_zone_from_css(wid, cx, cy),
                Some(expected),
                "center of {suffix} box must resolve to {expected:?}"
            );
        }
    }

    /// A point on the titlebar but outside every button resolves to the drag
    /// region, while a point well outside the decoration resolves to nothing.
    #[test]
    fn titlebar_gap_is_drag_outside_is_none() {
        let shell = windowed_shell();
        let wid = shell.visible_windows()[0].id;
        let tb = shell.window_titlebar_bounds_from_css(wid).unwrap();

        // Left edge of the titlebar (the title text region) is a drag zone.
        let drag = shell.window_decoration_zone_from_css(wid, tb.x + 8.0, tb.y + tb.height / 2.0);
        assert_eq!(drag, Some(HitZone::TitleBar), "titlebar gap must be a drag");

        // Far below the decoration: not a decoration zone.
        assert_eq!(
            shell.window_decoration_zone_from_css(wid, tb.x + 8.0, tb.y + 5000.0),
            None
        );
    }
}



