//! Overview / exposé hit-test adapter for the shell (t101-p5 full-CSS
//! migration).
//!
//! The overview surface is a DOM/CSS overlay synced through
//! [`Shell::sync_overview_template`](crate::shell::Shell) and laid out by the
//! CSS pipeline (the `overview*` rules). Each `overview-tile`
//! (`#overview-tile-<window_id>`) carries `data-window-id`; clicking a tile
//! focuses/raises that window. The click hit-test reads the tile's **laid-out
//! CSS box** from the live layout tree, NEVER hardcoded grid math — the
//! recurring hit-test-from-CSS-geometry contract (t86). A theme change that
//! moves the tiles therefore moves the click-zones with them.

use liquide_layout::geometry::Point;

use crate::shell::Shell;
use crate::shortcuts::ShellAction;
use crate::window::WindowId;

impl Shell {
    /// Resolve the window whose overview tile contains `(x, y)`, reading the
    /// tile boxes from the **CSS layout** (t101-p5 / t86 hit-test-from-CSS
    /// contract).
    ///
    /// Walks the visible windows and, for each, reads its tile element
    /// (`#overview-tile-<id>`) box from the live hit-test engine's layout tree.
    /// Returns the first window whose laid-out tile box contains the point. The
    /// click-zone is the painted CSS box, NOT a recomputed grid cell — so a
    /// stylesheet change that moves a tile moves its click-zone. Returns `None`
    /// when the overview is closed, not yet laid out, or the point hits no tile.
    #[must_use]
    pub(crate) fn overview_tile_window_at(&self, x: f32, y: f32) -> Option<WindowId> {
        if !self.overview_visible {
            return None;
        }
        let hit_test = self.hit_test_engine.as_ref()?;
        let pt = Point::new(x, y);
        for window in self.visible_windows() {
            let tile_el_id = format!("overview-tile-{}", window.id.0);
            let Some(node) = self.desktop_dom.doc.get_element_by_id(&tile_el_id) else {
                continue;
            };
            if let Some(rect) = hit_test.bounds_for_node(node) {
                if rect.contains(pt) {
                    return Some(window.id);
                }
            }
        }
        None
    }

    /// Handle a primary press on the open overview at `(x, y)`.
    ///
    /// The overview is modal/topmost, so while it is open EVERY press is
    /// consumed here (it must not leak to windows/chrome behind the scrim).
    /// When the press lands inside a tile's CSS-laid-out box
    /// ([`Self::overview_tile_window_at`]) the corresponding window is focused
    /// and raised and the overview closes — exactly like clicking a window in a
    /// real exposé. A press on the empty scrim simply dismisses the overview.
    ///
    /// Returns the resulting [`ShellAction`] (always a redraw) so the caller can
    /// repaint after the overview state change.
    pub(crate) fn overview_press(&mut self, x: f32, y: f32) -> ShellAction {
        if let Some(window_id) = self.overview_tile_window_at(x, y) {
            // Focus + raise the picked window, then dismiss the overview. Errors
            // (window vanished between layout and press) are non-fatal: the
            // overview still closes.
            let _ = self.set_focus(window_id);
            let _ = self.raise_window(window_id);
            self.close_overview();
        } else {
            // Empty-scrim press dismisses the overview.
            self.close_overview();
        }
        ShellAction::Redraw
    }

    /// Close the overview and drop its captured thumbnails, invalidating the
    /// scene so the next frame rebuilds without the overlay.
    fn close_overview(&mut self) {
        self.overview_visible = false;
        self.clear_overview_thumbnails();
        self.mark_window_scene_dirty();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_compositor::geometry::Rect;

    fn shell_with_overview() -> Shell {
        let mut shell = Shell::new(1280.0, 720.0);
        shell.cursor_blink_on = true;
        shell.cursor_blink_time_us = u64::MAX;
        shell.open_window("Alpha", Rect::new(100.0, 80.0, 400.0, 300.0));
        shell.open_window("Beta", Rect::new(560.0, 120.0, 360.0, 260.0));
        shell.overview_visible = true;
        // Build once so the DOM is synced and the pipeline lays out the tiles,
        // populating the hit-test engine.
        let _ = shell.build_scene();
        shell
    }

    /// Clicking a tile resolves to its window via the laid-out CSS box (the
    /// overview must be laid out with non-degenerate, distinct tile boxes).
    #[test]
    fn tile_click_resolves_to_window_from_layout() {
        let shell = shell_with_overview();
        let ids: Vec<WindowId> = shell.visible_windows().iter().map(|w| w.id).collect();

        let mut hit_count = 0;
        for id in &ids {
            let tile_el_id = format!("overview-tile-{}", id.0);
            let node = shell
                .desktop_dom
                .doc
                .get_element_by_id(&tile_el_id)
                .expect("tile element exists");
            let rect = shell
                .hit_test_engine
                .as_ref()
                .unwrap()
                .bounds_for_node(node)
                .expect("tile has a laid-out CSS box");
            let cx = rect.x + rect.width / 2.0;
            let cy = rect.y + rect.height / 2.0;
            assert_eq!(
                shell.overview_tile_window_at(cx, cy),
                Some(*id),
                "center of tile {id:?}'s CSS box must resolve to it"
            );
            hit_count += 1;
        }
        assert_eq!(hit_count, 2, "both window tiles must be hit-testable");
    }

    /// Clicking a tile focuses + raises its window and closes the overview.
    #[test]
    fn tile_press_focuses_and_closes() {
        let mut shell = shell_with_overview();
        let ids: Vec<WindowId> = shell.visible_windows().iter().map(|w| w.id).collect();
        let target = ids[0];

        let tile_el_id = format!("overview-tile-{}", target.0);
        let node = shell
            .desktop_dom
            .doc
            .get_element_by_id(&tile_el_id)
            .unwrap();
        let rect = shell
            .hit_test_engine
            .as_ref()
            .unwrap()
            .bounds_for_node(node)
            .unwrap();
        let cx = rect.x + rect.width / 2.0;
        let cy = rect.y + rect.height / 2.0;

        let _ = shell.overview_press(cx, cy);

        assert!(!shell.overview_visible, "press on a tile closes the overview");
        assert_eq!(
            shell.focus.focused(),
            Some(target),
            "press on a tile focuses its window"
        );
    }
}
