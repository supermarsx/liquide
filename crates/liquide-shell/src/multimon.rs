//! Multi-monitor wiring for the shell (t73-multimon §3.2–§3.4).
//!
//! The shell owns an optional [`liquide_display::DesktopLayout`] handed to it by
//! the session (which reads the real platform monitor set — t73-multimon §3.1).
//! With a layout present the shell:
//!
//! - places chrome (status bar / dock) on the primary monitor and reserves the
//!   matching [`WorkAreaInsets`] back into the layout so `work_area_of()` is
//!   correct for window tiling/maximize,
//! - assigns each window to a monitor (by spawn / drag-end center), and
//! - makes `MoveToMonitor{Left,Right}` REAL — `next/prev_monitor` +
//!   `move_rect_to_monitor` — instead of the single-screen shift proxy.
//!
//! With **no** layout (the default for headless/single-window hosts and every
//! existing test) the shell behaves exactly as before: chrome paints over
//! `screen_rect`, and `MoveToMonitor` falls back to the same-screen proxy.
//! A single-monitor layout is likewise a no-op for `MoveToMonitor`
//! (`next_monitor` → `None`), so a single-monitor host behaves as today.

use liquide_compositor::geometry::Rect as ShellRect;
use liquide_display::layout::Rect as LayoutRect;
use liquide_display::{DesktopLayout, DisplayId};

use crate::shell::Shell;
use crate::window::{WindowId, WindowState};

/// Convert a shell (f32) rect into a layout (i32/u32) rect. Negative sizes are
/// clamped to zero; fractional coordinates are rounded.
pub(crate) fn shell_rect_to_layout(r: ShellRect) -> LayoutRect {
    LayoutRect::new(
        r.x.round() as i32,
        r.y.round() as i32,
        r.width.max(0.0).round() as u32,
        r.height.max(0.0).round() as u32,
    )
}

/// Convert a layout (i32/u32) rect back into a shell (f32) rect.
pub(crate) fn layout_rect_to_shell(r: LayoutRect) -> ShellRect {
    ShellRect::new(r.x as f32, r.y as f32, r.width as f32, r.height as f32)
}

impl Shell {
    /// Install the multi-monitor [`DesktopLayout`] the session built from the
    /// real platform monitor set (t73-multimon §3.1). Passing the layout makes
    /// multi-monitor placement + real MoveToMonitor live; the shell also
    /// republishes its chrome reservations into the layout immediately so
    /// `work_area_of()` reflects the panel/dock insets.
    ///
    /// A single-monitor layout is accepted and behaves exactly as the legacy
    /// single-screen path. Passing a layout whose primary differs from the
    /// shell's current `screen_rect` also realigns `screen_rect` to the primary
    /// monitor's bounds so existing single-screen code keeps working against the
    /// primary.
    pub fn set_desktop_layout(&mut self, layout: DesktopLayout) {
        // Anchor screen_rect to the primary monitor's bounds for back-compat with
        // the (large) body of shell code that still assumes one screen.
        if let Some(primary) = layout.primary_output() {
            let b = primary.bounds();
            let primary_rect = ShellRect::new(b.0 as f32, b.1 as f32, b.2 as f32, b.3 as f32);
            if (self.screen_rect.width - primary_rect.width).abs() > 0.5
                || (self.screen_rect.height - primary_rect.height).abs() > 0.5
                || (self.screen_rect.x - primary_rect.x).abs() > 0.5
                || (self.screen_rect.y - primary_rect.y).abs() > 0.5
            {
                self.screen_rect = primary_rect;
                self.update_style_resolver_context();
            }
        }
        self.desktop_layout = Some(layout);
        self.publish_chrome_insets();
        self.mark_window_scene_dirty();
    }

    /// Read-only access to the installed desktop layout, if any.
    #[must_use]
    pub fn desktop_layout(&self) -> Option<&DesktopLayout> {
        self.desktop_layout.as_ref()
    }

    /// Number of monitors the shell currently knows about (1 when no real layout
    /// is installed — the implicit single screen).
    #[must_use]
    pub fn monitor_count(&self) -> usize {
        self.desktop_layout
            .as_ref()
            .map_or(1, DesktopLayout::output_count)
    }

    /// The monitor a window is currently assigned to, if a layout is installed
    /// and the window has been placed. `None` with no layout.
    #[must_use]
    pub fn window_monitor(&self, wid: WindowId) -> Option<DisplayId> {
        self.window_monitors.get(&wid).copied()
    }

    /// Assign a window to the monitor under its center point (t73-multimon §3.3).
    /// No-op when no layout is installed. Called on open and on drag-end.
    pub(crate) fn assign_window_to_monitor(&mut self, wid: WindowId) {
        let Some(layout) = self.desktop_layout.as_ref() else {
            return;
        };
        let Some(window) = self.windows.get(&wid) else {
            return;
        };
        let (cx, cy) = (
            (window.bounds.x + window.bounds.width / 2.0).round() as i32,
            (window.bounds.y + window.bounds.height / 2.0).round() as i32,
        );
        let target = layout
            .monitor_at_point_or_nearest(cx, cy)
            .or_else(|| layout.primary());
        if let Some(id) = target {
            self.window_monitors.insert(wid, id);
        }
    }

    /// Compute the shell's chrome reservation (top status bar + dock) and publish
    /// it into the layout's primary monitor so `work_area_of()` is correct for
    /// tiling/maximize (t73-multimon §3.2). Per-monitor chrome beyond the primary
    /// is deferred (start: primary only, per the spec).
    pub(crate) fn publish_chrome_insets(&mut self) {
        use liquide_dock::DockPosition;
        use liquide_display::WorkAreaInsets;

        let Some(primary) = self.desktop_layout.as_ref().and_then(DesktopLayout::primary) else {
            return;
        };

        let bar_h = if self.status_bar_visible {
            self.status_bar.config().height.round() as u32
        } else {
            0
        };
        let dock_bounds = self.dock.compute_bounds(self.screen_rect);
        let (mut top, mut right, mut bottom, mut left) = (bar_h, 0u32, 0u32, 0u32);
        if self.dock.is_visible() {
            match self.dock.config().position {
                DockPosition::Bottom => bottom = dock_bounds.height.max(0.0).round() as u32,
                DockPosition::Top => top = top.saturating_add(dock_bounds.height.max(0.0).round() as u32),
                DockPosition::Left => left = dock_bounds.width.max(0.0).round() as u32,
                DockPosition::Right => right = dock_bounds.width.max(0.0).round() as u32,
            }
        }

        if let Some(layout) = self.desktop_layout.as_mut() {
            layout.set_insets(primary, WorkAreaInsets::new(top, right, bottom, left));
        }
    }

    /// The usable work area of a window's monitor (full monitor minus reserved
    /// chrome), or `None` when no layout is installed. Window tiling/maximize
    /// should prefer this over [`Shell::work_area`] when a layout is present so a
    /// window on a non-primary monitor maximizes into the right monitor
    /// (t73-multimon §3.2).
    #[must_use]
    pub fn window_work_area(&self, wid: WindowId) -> Option<ShellRect> {
        let layout = self.desktop_layout.as_ref()?;
        let monitor = self
            .window_monitors
            .get(&wid)
            .copied()
            .or_else(|| layout.primary())?;
        layout.work_area_of(monitor).map(layout_rect_to_shell)
    }

    /// Move the focused window to the next (`+1`) or previous (`-1`) monitor —
    /// the REAL implementation behind `MoveToMonitor{Right,Left}` (t73-multimon
    /// §3.4), replacing the single-screen shift proxy.
    ///
    /// With a multi-monitor layout: resolve the window's current monitor, step to
    /// next/prev (wrapping), proportionally remap the window rect into the target
    /// monitor's work area, and re-assign the window to that monitor. With a
    /// single-monitor layout (`next_monitor` → `None`) this is a no-op move. With
    /// NO layout installed it falls back to the legacy same-screen proxy so the
    /// headless/test path is byte-for-byte unchanged.
    ///
    /// Returns `true` (the gesture is always acknowledged, even with no focused
    /// window) to preserve the proxy's contract.
    pub(crate) fn move_focused_window_to_adjacent_monitor(&mut self, step: i32) -> bool {
        // No real layout → legacy single-screen proxy (unchanged behavior).
        let Some(layout) = self.desktop_layout.as_ref() else {
            return self.move_focused_window_by_monitor(step as f32);
        };

        let Some(wid) = self.focus.focused() else {
            return true;
        };

        // Resolve the source monitor (assigned, else by center, else primary).
        let from = self.window_monitors.get(&wid).copied().or_else(|| {
            self.windows.get(&wid).and_then(|w| {
                let cx = (w.bounds.x + w.bounds.width / 2.0).round() as i32;
                let cy = (w.bounds.y + w.bounds.height / 2.0).round() as i32;
                layout.monitor_at_point_or_nearest(cx, cy)
            })
        });
        let Some(from) = from.or_else(|| layout.primary()) else {
            return true;
        };

        // Single-monitor layout: no adjacent monitor → acknowledged no-op.
        let target = if step >= 0 {
            layout.next_monitor(from)
        } else {
            layout.prev_monitor(from)
        };
        let Some(target) = target else {
            return true;
        };

        if let Some(window) = self.windows.get(&wid) {
            let rect = shell_rect_to_layout(window.bounds);
            if let Some(moved) = layout.move_rect_to_monitor(rect, target) {
                let new_bounds = layout_rect_to_shell(moved);
                if let Some(window) = self.windows.get_mut(&wid) {
                    window.bounds = new_bounds;
                    if window.state == WindowState::Maximized {
                        window.state = WindowState::Normal;
                    }
                    window.tiled = false;
                    window.tile_zone = None;
                }
                self.window_monitors.insert(wid, target);
            }
        }
        self.mark_window_scene_dirty();
        true
    }
}
