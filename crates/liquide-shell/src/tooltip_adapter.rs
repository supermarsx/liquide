//! Tooltip integration adapter for the shell.
//!
//! Bridges the shell's hover state onto the canonical
//! [`liquide_tooltip::TooltipManager`] state machine, held in the dormant
//! `chrome_tooltip` field added by t51-e7. Before this adapter the shell drove
//! tooltips through ad-hoc `tooltip_text` / `tooltip_pos` / `tooltip_timer_us`
//! fields with a hand-rolled 400 ms dwell check in `dom_sync.rs`; this module
//! wires the real, tested tooltip lifecycle (show delay → fade-in → visible →
//! fade-out) and is the canonical replacement for that ad-hoc logic
//! (t49-e5-F12: `liquide-tooltip` had zero production consumers).
//!
//! ## Call order — the t49-e5-F07 ordering hazard
//!
//! t49-e5-F07 documented a tooltip controller whose queued show/hide *action*
//! was wiped by the next per-frame `update()` because `update()` reset the
//! pending action *before* the consumer read it — so a show queued by hover
//! input never fired (stale/stuck tooltip).
//!
//! [`TooltipManager`] expresses the same lifecycle as an explicit state machine
//! rather than a queued action, so the analogous hazard here is **call order**:
//! a hover transition queued this frame (`on_hover_begin` → `Pending`,
//! `on_hover_end` → `FadingOut`) must be applied to the manager *before*
//! [`TooltipManager::update`] advances the timers, and the visibility must be
//! read *after* `update`. If `update` ran first, a hover-begin applied later in
//! the same frame would sit un-advanced (its show never progresses) and a
//! hover-end could be advanced against the wrong prior state. This adapter
//! enforces the safe order inside [`Shell::sync_tooltip_manager`]:
//!
//!   1. refresh screen bounds,
//!   2. apply the pending hover transition (begin/end) — the "queued action",
//!   3. **then** `update(dt_ms)` — advances the just-applied transition,
//!   4. callers read visibility/opacity afterwards.
//!
//! Because the transition is applied before (never wiped by) `update`, a show
//! queued on the same frame survives to fire — the F07 failure mode cannot
//! occur here. The regressions below assert exactly that.
//!
//! The shell's hover input still lives in `shell/events.rs` (a peer-owned file)
//! and writes the `tooltip_text` / `tooltip_pos` fields; this adapter *reads*
//! those fields each frame and projects them onto the canonical manager. The
//! full removal of the ad-hoc fields (which are also read by the peer-owned,
//! in-flight `dom_sync.rs` render path and the `events.rs` hover path) is left
//! to the C3 retirement integrator (t51-e15) — see t51-e9 log.

use liquide_tooltip::TooltipConfig;
use liquide_tooltip::manager::ScreenBounds;
use liquide_ui_core::WidgetId;

use crate::shell::Shell;

/// Stable widget id for the shell's single hover slot.
///
/// The shell tracks at most one hovered chrome element at a time (currently the
/// dock item under the cursor). Using one constant id keeps `on_hover_begin` /
/// `on_hover_end` symmetric without needing to persist a per-target id: ending
/// always matches the begin, and re-entering after an end starts a fresh
/// lifecycle because the manager clears its hovered widget on `on_hover_end`.
const HOVER_SLOT: WidgetId = WidgetId::from_raw(0x5_4001_7000); // "shell tooltip"

impl Shell {
    /// Ensure the canonical [`TooltipManager`] exists, constructing it lazily on
    /// first use, and return a mutable reference to it.
    ///
    /// Kept lazy so the dormant `chrome_tooltip` field (t51-e7) stays `None`
    /// until a tooltip is actually exercised, avoiding any allocation/behavior
    /// change for shells that never hover.
    fn ensure_tooltip_manager(&mut self) -> &mut liquide_tooltip::TooltipManager {
        if self.chrome_tooltip.is_none() {
            self.chrome_tooltip = Some(liquide_tooltip::TooltipManager::new(
                TooltipConfig::default(),
            ));
        }
        self.chrome_tooltip
            .as_mut()
            .expect("tooltip manager just constructed")
    }

    /// Drive the canonical tooltip manager for one frame.
    ///
    /// `dt_ms` is the elapsed time since the previous call. Reads the shell's
    /// current hover state (`tooltip_text` / `tooltip_pos`, written by the
    /// hover input path in `shell/events.rs`) and projects it onto the
    /// canonical [`TooltipManager`] in the **F07-safe order** documented at the
    /// module level: apply the hover transition first, then advance the timers.
    ///
    /// Returns `true` if a tooltip is currently visible after the update (so a
    /// caller can decide to redraw the tooltip overlay).
    pub(crate) fn sync_tooltip_manager(&mut self, dt_ms: f32) -> bool {
        let screen = self.screen_rect;
        // Snapshot the hover state set by the (peer-owned) input path before we
        // touch the manager, so the borrow of `self` for the manager is clean.
        let hover = self.tooltip_text.clone();
        let pos = self.tooltip_pos;

        let mgr = self.ensure_tooltip_manager();

        // (1) Keep edge-clamping bounds current (per-monitor origin aware).
        mgr.set_screen_bounds(ScreenBounds::new(
            screen.x,
            screen.y,
            screen.width,
            screen.height,
        ));

        // (2) Apply the queued hover transition FIRST — never after `update`.
        match hover {
            Some(text) => {
                // Anchor: the shell positions the tooltip at `tooltip_pos`
                // (top-left of the tooltip box, already clamped above the dock
                // item). Use a zero-size anchor at that point so the manager's
                // `Below` placement keeps it at the requested position.
                mgr.on_hover_begin(HOVER_SLOT, &text, pos.x, pos.y, 0.0, 0.0);
            }
            None => {
                mgr.on_hover_end(HOVER_SLOT);
            }
        }

        // (3) THEN advance the timers, so the transition queued in (2) is
        // progressed this frame rather than wiped. Reading visibility happens
        // AFTER the update (step 4, below / at the call site).
        mgr.update(dt_ms);

        mgr.is_visible()
    }

    /// Whether the canonical tooltip manager currently shows a tooltip
    /// (including fade animations). `false` when the manager has never been
    /// constructed.
    #[must_use]
    pub(crate) fn tooltip_manager_visible(&self) -> bool {
        self.chrome_tooltip
            .as_ref()
            .is_some_and(liquide_tooltip::TooltipManager::is_visible)
    }

    /// Current tooltip opacity (0.0–1.0) from the canonical manager, or `0.0`
    /// when no manager exists.
    #[must_use]
    pub(crate) fn tooltip_manager_opacity(&self) -> f32 {
        self.chrome_tooltip
            .as_ref()
            .map_or(0.0, liquide_tooltip::TooltipManager::opacity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_compositor::geometry::Point;

    /// Default `TooltipConfig` show delay (ms) — the dwell before a hovered
    /// tooltip begins fading in.
    fn show_delay_ms() -> f32 {
        TooltipConfig::default().show_delay_ms as f32
    }

    /// A hover that dwells past the show delay must make the canonical
    /// `TooltipManager` report the tooltip visible — i.e. the manager (not the
    /// ad-hoc fields) drives show.
    #[test]
    fn hover_dwell_shows_tooltip_via_manager() {
        let mut shell = Shell::new(1920.0, 1080.0);

        // Simulate the input path having set a hover label + position.
        shell.tooltip_text = Some("Files".to_string());
        shell.tooltip_pos = Point::new(100.0, 200.0);

        // Dormant until first driven.
        assert!(shell.chrome_tooltip.is_none());

        // First frame: hover applied, but not yet past the show delay.
        let visible_now = shell.sync_tooltip_manager(0.0);
        assert!(shell.chrome_tooltip.is_some(), "manager constructed lazily");
        assert!(
            !visible_now,
            "tooltip should still be pending before the delay"
        );
        assert!(!shell.tooltip_manager_visible());

        // Dwell past the show delay — the manager must now show the tooltip.
        let visible_after = shell.sync_tooltip_manager(show_delay_ms() + 1.0);
        assert!(
            visible_after,
            "tooltip must become visible via the canonical manager after dwell"
        );
        assert!(shell.tooltip_manager_visible());

        // Opacity ramps during fade-in; after a further frame it is positive and
        // bounded in [0, 1].
        shell.sync_tooltip_manager(8.0);
        let opacity = shell.tooltip_manager_opacity();
        assert!(
            opacity > 0.0 && opacity <= 1.0,
            "fading-in tooltip opacity must ramp into (0, 1], got {opacity}"
        );
    }

    /// Moving away (hover cleared) must hide the tooltip through the manager's
    /// fade-out → hidden lifecycle.
    #[test]
    fn moving_away_hides_tooltip_via_manager() {
        let mut shell = Shell::new(1920.0, 1080.0);

        // Show a tooltip first.
        shell.tooltip_text = Some("Terminal".to_string());
        shell.tooltip_pos = Point::new(50.0, 60.0);
        shell.sync_tooltip_manager(0.0);
        shell.sync_tooltip_manager(show_delay_ms() + 200.0);
        assert!(shell.tooltip_manager_visible(), "tooltip should be visible");

        // Mouse leaves: the input path clears the hover label.
        shell.tooltip_text = None;

        // The frame the hover ends, the manager begins fading out (still
        // visible), then a long frame completes the fade-out → hidden.
        shell.sync_tooltip_manager(1.0);
        let hidden = !shell.sync_tooltip_manager(10_000.0);
        assert!(
            hidden,
            "tooltip must hide via the manager after the hover ends"
        );
        assert!(!shell.tooltip_manager_visible());
    }

    /// F07 regression — call-order: a show queued in the SAME frame as the
    /// per-frame update must NOT be wiped before it can fire.
    ///
    /// `sync_tooltip_manager` applies the hover transition *before* `update`,
    /// so a single dwell-length frame (hover begin + dwell in one call)
    /// progresses straight to visible. If the order were inverted (update
    /// first, transition second — the t49-e5-F07 failure shape), the freshly
    /// queued show would be left un-advanced and the tooltip would never appear
    /// that frame.
    #[test]
    fn f07_queued_show_not_wiped_before_it_fires() {
        let mut shell = Shell::new(1920.0, 1080.0);

        shell.tooltip_text = Some("Browser".to_string());
        shell.tooltip_pos = Point::new(10.0, 20.0);

        // A single frame whose dt already exceeds the show delay: the hover
        // BEGIN queued this frame must be advanced by THIS frame's update.
        let visible = shell.sync_tooltip_manager(show_delay_ms() + 1.0);
        assert!(
            visible,
            "a show queued the same frame as the update must fire, not be wiped \
             (t49-e5-F07 ordering)"
        );
        assert!(shell.tooltip_manager_visible());
    }

    /// F07 regression — the symmetric hide case: a hover-end queued the same
    /// frame as the update must also be honored (the manager must enter
    /// fade-out), not be wiped so the tooltip stays stuck visible.
    #[test]
    fn f07_queued_hide_not_wiped_leaves_tooltip_unstuck() {
        let mut shell = Shell::new(1920.0, 1080.0);

        // Become fully visible.
        shell.tooltip_text = Some("Settings".to_string());
        shell.tooltip_pos = Point::new(70.0, 80.0);
        shell.sync_tooltip_manager(0.0);
        shell.sync_tooltip_manager(show_delay_ms() + 1000.0);
        assert!(shell.tooltip_manager_visible());

        // Clear hover and run a long frame: the hover-END applied before this
        // frame's update must drive the fade-out to completion rather than be
        // wiped (which would leave the tooltip stuck visible — the F07 symptom).
        shell.tooltip_text = None;
        let still_visible = shell.sync_tooltip_manager(10_000.0);
        assert!(
            !still_visible,
            "a hide queued the same frame as the update must fire, not be wiped \
             (t49-e5-F07 ordering — tooltip must not stay stuck visible)"
        );
        assert!(!shell.tooltip_manager_visible());
    }

    /// Re-entering after a leave starts a fresh dwell (the manager clears its
    /// hovered slot on end, so the next begin restarts the lifecycle).
    #[test]
    fn re_enter_after_leave_restarts_dwell() {
        let mut shell = Shell::new(1920.0, 1080.0);

        // Show, then hide fully.
        shell.tooltip_text = Some("Files".to_string());
        shell.tooltip_pos = Point::new(100.0, 200.0);
        shell.sync_tooltip_manager(0.0);
        shell.sync_tooltip_manager(show_delay_ms() + 1.0);
        assert!(shell.tooltip_manager_visible());
        shell.tooltip_text = None;
        shell.sync_tooltip_manager(10_000.0);
        assert!(!shell.tooltip_manager_visible());

        // Re-enter: a single zero-length frame must NOT immediately show — the
        // dwell restarts from scratch.
        shell.tooltip_text = Some("Files".to_string());
        let visible_immediately = shell.sync_tooltip_manager(0.0);
        assert!(
            !visible_immediately,
            "re-entering must restart the show delay, not flash instantly"
        );
        // Dwelling again shows it once more.
        let visible_after = shell.sync_tooltip_manager(show_delay_ms() + 1.0);
        assert!(visible_after);
    }
}
