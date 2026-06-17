//! Session-side multi-monitor wiring (t73-multimon §3.1).
//!
//! The session reads the platform monitor set via the
//! [`DisplayBackend`](liquide_platform::DisplayBackend) trait, builds a
//! [`liquide_display::DesktopLayout`] from it, and hands that to the shell
//! ([`liquide_shell::Shell::set_desktop_layout`]) so the shell can place chrome
//! per-monitor, reserve work areas, and make MoveToMonitor real.
//!
//! The `MonitorInfo → DisplayInfo` conversion is single-sourced here
//! ([`monitor_info_to_display_info`]) so it never drifts (the shell does not
//! duplicate it — t73-multimon §3.1).

use liquide_compositor::geometry::Rect;
use liquide_display::display::{DisplayInfo, Resolution, Rotation};
use liquide_display::DesktopLayout;
use liquide_platform::display::MonitorInfo;
use liquide_platform::PlatformBackend;
use liquide_shell::WindowId;

use super::DesktopCompositor;

/// Single-source conversion from a platform [`MonitorInfo`] to a display-model
/// [`DisplayInfo`] (t73-multimon §3.1). The platform reports *logical*
/// (already DPI-divided) geometry, so the display `scale` is left at `1.0` and
/// the geometry is used as the logical bounds directly — the per-monitor DPI is
/// applied to *input coordinates* on the platform side, not re-applied here.
pub(super) fn monitor_info_to_display_info(m: &MonitorInfo) -> DisplayInfo {
    let width = m.geometry.width.max(0.0).round() as u32;
    let height = m.geometry.height.max(0.0).round() as u32;
    DisplayInfo {
        id: m.id,
        name: m.name.clone(),
        connector: m.name.clone(),
        resolution: Resolution::new(width, height),
        available_resolutions: vec![Resolution::new(width, height)],
        refresh_rate: m.refresh_rate_hz as f32,
        available_refresh_rates: vec![m.refresh_rate_hz as f32],
        position: (m.geometry.x.round() as i32, m.geometry.y.round() as i32),
        rotation: Rotation::Normal,
        // Geometry is already logical; keep scale 1.0 so `bounds()` returns the
        // logical rect unchanged (re-dividing by the real DPI here would shrink
        // the already-logical bounds).
        scale: 1.0,
        primary: m.primary,
        enabled: true,
        physical_size_mm: None,
        connected: true,
    }
}

impl DesktopCompositor {
    /// Read the platform monitor set and install the resulting
    /// [`DesktopLayout`] on the shell (t73-multimon §3.1).
    ///
    /// When the platform reports zero or one monitor (Null/headless or a true
    /// single-monitor host) a single-monitor layout is built so the shell's
    /// MoveToMonitor behaves exactly as today (no adjacent monitor). When two or
    /// more monitors are reported, the real multi-output layout is installed and
    /// multi-monitor placement + real MoveToMonitor go live.
    ///
    /// Called once from `run()` after the platform window/display is ready, and
    /// safe to call again (e.g. on a live `DisplaysChanged` hotplug event) to
    /// rebuild.
    pub(super) fn install_desktop_layout(&mut self, platform: &dyn PlatformBackend) {
        let monitors = platform.display().monitors();
        let virtual_rect = platform.display().virtual_screen_rect();
        self.apply_desktop_layout(&monitors, virtual_rect);
    }

    /// Handle a live `PlatformEvent::DisplaysChanged` (monitor hotplug): a
    /// display was added, removed, or had its geometry changed at runtime
    /// (t93 gap #5c).
    ///
    /// Re-enumerates the platform monitor set, re-installs the desktop layout,
    /// migrates/clamps any window stranded on a removed monitor onto a surviving
    /// one, resizes the compositor when the primary geometry changed, and marks
    /// the frame dirty so the desktop re-lays-out immediately instead of only on
    /// the next periodic tick. Returns `true` if a redraw is needed.
    pub(super) fn handle_displays_changed(&mut self, platform: &dyn PlatformBackend) -> bool {
        tracing::info!("displays changed (hotplug): re-installing desktop layout");
        let monitors = platform.display().monitors();
        let virtual_rect = platform.display().virtual_screen_rect();
        self.apply_desktop_layout(&monitors, virtual_rect);

        // The primary geometry may have changed (e.g. the previous primary was
        // unplugged). Re-sync the compositor framebuffer / shell screen to the
        // new primary so the desktop covers the live display. In dev mode the
        // window keeps its requested size, so skip the fullscreen resize there.
        if !self.dt.dev_mode {
            self.resize_to_primary(&monitors, virtual_rect);
        }

        // Force a redraw so the re-laid-out chrome + migrated windows repaint
        // now, not only on the next ~1s periodic tick.
        self.dirty = true;
        self.dirty_damage = None;
        true
    }

    /// Re-enumerate + install the layout from a concrete monitor set, then
    /// migrate/clamp windows onto surviving monitors. Pure with respect to the
    /// platform (takes the already-read monitor set + virtual rect), so the
    /// hotplug behaviour is unit-testable without real display hardware.
    pub(super) fn apply_desktop_layout(&mut self, monitors: &[MonitorInfo], virtual_rect: Rect) {
        let displays: Vec<DisplayInfo> = if monitors.is_empty() {
            // Headless / Null backend: synthesize one logical monitor from the
            // virtual-screen rect so the shell still has a primary to anchor to.
            let r = virtual_rect;
            vec![DisplayInfo {
                id: 0,
                name: "virtual".to_string(),
                connector: "virtual".to_string(),
                resolution: Resolution::new(
                    r.width.max(1.0).round() as u32,
                    r.height.max(1.0).round() as u32,
                ),
                available_resolutions: vec![Resolution::new(
                    r.width.max(1.0).round() as u32,
                    r.height.max(1.0).round() as u32,
                )],
                refresh_rate: 60.0,
                available_refresh_rates: vec![60.0],
                position: (r.x.round() as i32, r.y.round() as i32),
                rotation: Rotation::Normal,
                scale: 1.0,
                primary: true,
                enabled: true,
                physical_size_mm: None,
                connected: true,
            }]
        } else {
            monitors.iter().map(monitor_info_to_display_info).collect()
        };

        let layout = DesktopLayout::new(displays);
        tracing::info!(
            monitors = layout.output_count(),
            single = layout.is_single_monitor(),
            "installed desktop layout"
        );
        // `set_desktop_layout` re-anchors the shell's `screen_rect` to the new
        // primary and republishes chrome insets.
        self.shell.set_desktop_layout(layout);

        // Migrate/clamp windows stranded on a removed monitor onto a surviving
        // one. We diff each window's center against the NEW monitor set: a window
        // whose center no longer lands on any monitor had its monitor removed
        // (or shrunk past it) and must be repatriated so it is not lost.
        self.migrate_windows_onto_surviving_monitors();
    }

    /// Move any window whose monitor was removed onto a surviving monitor,
    /// clamping its geometry into that monitor's work area (t93 gap #5c).
    ///
    /// A window is considered stranded when its center no longer lies on any
    /// monitor in the freshly-installed layout (its display was unplugged or
    /// resized out from under it). It is relocated onto the nearest surviving
    /// monitor's work area and clamped to fit, so removing a display can never
    /// orphan a window off the visible desktop. Windows already on a surviving
    /// monitor are left untouched (an added monitor never moves existing
    /// windows). Uses only public shell APIs so the shell crate is not touched.
    fn migrate_windows_onto_surviving_monitors(&mut self) {
        let Some(layout) = self.shell.desktop_layout() else {
            return;
        };
        if layout.output_count() == 0 {
            return;
        }

        // Collect the relocations first (immutable borrow of the shell), then
        // apply them (mutable borrow) — the borrow checker forbids holding both.
        let mut relocations: Vec<(WindowId, f32, f32)> = Vec::new();
        for window in self.shell.visible_windows() {
            let b = window.bounds;
            let (cx, cy) = (
                (b.x + b.width / 2.0).round() as i32,
                (b.y + b.height / 2.0).round() as i32,
            );
            // Still on a live monitor? Leave it where it is.
            if layout.monitor_at_point(cx, cy).is_some() {
                continue;
            }
            // Stranded: relocate onto the nearest surviving monitor's work area.
            let Some(target) = layout.monitor_at_point_or_nearest(cx, cy) else {
                continue;
            };
            let Some(wa) = layout.work_area_of(target) else {
                continue;
            };
            // Clamp the window's top-left so the whole window stays inside the
            // destination work area (shrinking the offset, not the size).
            let max_x = (wa.right() as f32 - b.width).max(wa.x as f32);
            let max_y = (wa.bottom() as f32 - b.height).max(wa.y as f32);
            let new_x = b.x.clamp(wa.x as f32, max_x);
            let new_y = b.y.clamp(wa.y as f32, max_y);
            relocations.push((window.id, new_x, new_y));
        }

        for (id, x, y) in relocations {
            let _ = self.shell.move_window(id, x, y);
        }
    }

    /// Re-sync the compositor framebuffer + shell screen size to the new primary
    /// monitor geometry after a hotplug, mirroring the initial-resize logic in
    /// `run()` (event_loop.rs). No-op when the primary geometry is unchanged.
    fn resize_to_primary(&mut self, monitors: &[MonitorInfo], virtual_rect: Rect) {
        let primary_rect = monitors
            .iter()
            .find(|m| m.primary)
            .map(|m| m.geometry)
            .or_else(|| monitors.first().map(|m| m.geometry))
            .unwrap_or(virtual_rect);
        let new_w = primary_rect.width as u32;
        let new_h = primary_rect.height as u32;
        if new_w == 0 || new_h == 0 || (new_w == self.width && new_h == self.height) {
            return;
        }
        tracing::info!(
            old_w = self.width,
            old_h = self.height,
            new_w,
            new_h,
            "resizing compositor to new primary after display change"
        );
        self.width = new_w;
        self.height = new_h;
        if let Some(ref mut compositor) = self.compositor {
            let _ = compositor.resize(new_w, new_h);
        } else if let Some(ref tx) = self.render_tx {
            let _ = tx.send(super::RenderMsg::Resize {
                width: new_w,
                height: new_h,
            });
        }
        self.shell.resize_screen(new_w as f32, new_h as f32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_compositor::geometry::Rect;

    fn monitor(id: u32, x: f32, y: f32, w: f32, h: f32, primary: bool) -> MonitorInfo {
        MonitorInfo {
            id,
            name: format!("M{id}"),
            geometry: Rect::new(x, y, w, h),
            work_area: Rect::new(x, y, w, h),
            dpi_scale: 1.0,
            primary,
            refresh_rate_hz: 60,
        }
    }

    #[test]
    fn conversion_preserves_geometry_and_primary() {
        let m = monitor(2, 1920.0, 0.0, 2560.0, 1440.0, true);
        let d = monitor_info_to_display_info(&m);
        assert_eq!(d.id, 2);
        assert_eq!(d.position, (1920, 0));
        assert_eq!(d.resolution, Resolution::new(2560, 1440));
        assert!(d.primary);
        // Logical geometry → scale 1.0 so bounds() round-trips the logical rect.
        assert_eq!(d.bounds(), (1920, 0, 2560, 1440));
    }

    #[test]
    fn dual_monitor_layout_is_not_single() {
        let displays = vec![
            monitor_info_to_display_info(&monitor(1, 0.0, 0.0, 1920.0, 1080.0, true)),
            monitor_info_to_display_info(&monitor(2, 1920.0, 0.0, 1920.0, 1080.0, false)),
        ];
        let layout = DesktopLayout::new(displays);
        assert_eq!(layout.output_count(), 2);
        assert!(!layout.is_single_monitor());
        assert_eq!(layout.primary(), Some(1));
        assert_eq!(layout.next_monitor(1), Some(2));
    }

    // ── Monitor hotplug (t93 gap #5c) ────────────────────────────────────────
    //
    // These drive `apply_desktop_layout` — the pure, hardware-free half of the
    // live `DisplaysChanged` path (the win32 `WM_DISPLAYCHANGE` decode is the
    // backend half and is exercised by `liquide-platform`). They simulate a
    // monitor REMOVAL (a window on the removed output must migrate onto a
    // surviving one and be clamped) and an ADDITION (the layout expands). Each
    // is written to FAIL if hotplug is ignored — i.e. if the layout is not
    // re-installed or the stranded window is not migrated.

    /// Two side-by-side 1080p monitors; monitor 1 is primary.
    fn dual_monitors() -> Vec<MonitorInfo> {
        vec![
            monitor(1, 0.0, 0.0, 1920.0, 1080.0, true),
            monitor(2, 1920.0, 0.0, 1920.0, 1080.0, false),
        ]
    }

    fn single_monitor() -> Vec<MonitorInfo> {
        vec![monitor(1, 0.0, 0.0, 1920.0, 1080.0, true)]
    }

    #[test]
    fn hotplug_removal_migrates_stranded_window_onto_surviving_monitor() {
        let mut desktop = DesktopCompositor::new(1920, 1080);
        // Start dual-monitor.
        desktop.apply_desktop_layout(&dual_monitors(), Rect::new(0.0, 0.0, 3840.0, 1080.0));
        assert_eq!(desktop.shell.monitor_count(), 2);

        // A window living on monitor 2 (center at x≈2600, well right of 1920).
        let wid = desktop
            .shell
            .open_window("on-mon-2", Rect::new(2400.0, 200.0, 400.0, 300.0));
        // Sanity: its center is on monitor 2 in the dual layout.
        let layout = desktop.shell.desktop_layout().unwrap();
        assert_eq!(layout.monitor_at_point(2600, 350), Some(2));

        // HOTPLUG: monitor 2 is unplugged -> single-monitor layout.
        desktop.apply_desktop_layout(&single_monitor(), Rect::new(0.0, 0.0, 1920.0, 1080.0));
        assert_eq!(
            desktop.shell.monitor_count(),
            1,
            "layout must collapse to the single surviving monitor"
        );

        // The window must have MIGRATED onto monitor 1 and been CLAMPED fully
        // inside its bounds (not left stranded at x=2400 off the visible desktop).
        let b = desktop.shell.window(wid).unwrap().bounds;
        assert!(
            b.x >= 0.0 && b.x + b.width <= 1920.0,
            "stranded window must be migrated+clamped onto monitor 1; got x={} w={}",
            b.x,
            b.width
        );
        assert!(
            b.y >= 0.0 && b.y + b.height <= 1080.0,
            "migrated window must stay within the surviving monitor vertically; got y={} h={}",
            b.y,
            b.height
        );
        // Its center must now resolve to a live monitor.
        let layout = desktop.shell.desktop_layout().unwrap();
        let (cx, cy) = (
            (b.x + b.width / 2.0).round() as i32,
            (b.y + b.height / 2.0).round() as i32,
        );
        assert_eq!(
            layout.monitor_at_point(cx, cy),
            Some(1),
            "migrated window center must land on the surviving monitor"
        );
    }

    #[test]
    fn hotplug_removal_leaves_window_on_surviving_monitor_untouched() {
        let mut desktop = DesktopCompositor::new(1920, 1080);
        desktop.apply_desktop_layout(&dual_monitors(), Rect::new(0.0, 0.0, 3840.0, 1080.0));

        // A window safely on monitor 1.
        let wid = desktop
            .shell
            .open_window("on-mon-1", Rect::new(100.0, 100.0, 500.0, 400.0));
        let before = desktop.shell.window(wid).unwrap().bounds;

        // Unplug monitor 2; the monitor-1 window must NOT move.
        desktop.apply_desktop_layout(&single_monitor(), Rect::new(0.0, 0.0, 1920.0, 1080.0));
        let after = desktop.shell.window(wid).unwrap().bounds;
        assert_eq!(
            (after.x, after.y),
            (before.x, before.y),
            "a window already on a surviving monitor must not be relocated by hotplug"
        );
    }

    #[test]
    fn hotplug_addition_expands_layout_without_moving_windows() {
        let mut desktop = DesktopCompositor::new(1920, 1080);
        // Start single-monitor.
        desktop.apply_desktop_layout(&single_monitor(), Rect::new(0.0, 0.0, 1920.0, 1080.0));
        assert_eq!(desktop.shell.monitor_count(), 1);

        let wid = desktop
            .shell
            .open_window("w", Rect::new(200.0, 200.0, 400.0, 300.0));
        let before = desktop.shell.window(wid).unwrap().bounds;

        // HOTPLUG: a second monitor is added on the right.
        desktop.apply_desktop_layout(&dual_monitors(), Rect::new(0.0, 0.0, 3840.0, 1080.0));
        assert_eq!(
            desktop.shell.monitor_count(),
            2,
            "added monitor must expand the layout"
        );
        // The newly-added monitor must be addressable.
        let layout = desktop.shell.desktop_layout().unwrap();
        assert_eq!(layout.monitor_at_point(2600, 350), Some(2));
        assert_eq!(layout.next_monitor(1), Some(2));

        // An addition must never relocate existing windows.
        let after = desktop.shell.window(wid).unwrap().bounds;
        assert_eq!(
            (after.x, after.y),
            (before.x, before.y),
            "adding a monitor must not move existing windows"
        );
    }

    #[test]
    fn hotplug_geometry_change_remigrates_window_off_shrunken_monitor() {
        // A monitor that SHRINKS (resolution change) can strand a window that was
        // valid at the larger size. The window must re-migrate to stay visible.
        let mut desktop = DesktopCompositor::new(1920, 1080);
        // Single 1920x1080 monitor.
        desktop.apply_desktop_layout(&single_monitor(), Rect::new(0.0, 0.0, 1920.0, 1080.0));
        let wid = desktop
            .shell
            .open_window("w", Rect::new(1500.0, 800.0, 380.0, 260.0));

        // The monitor's resolution drops to 1280x720; the window's center
        // (≈1690, 930) is now off the live area and must be clamped back in.
        let shrunk = vec![monitor(1, 0.0, 0.0, 1280.0, 720.0, true)];
        desktop.apply_desktop_layout(&shrunk, Rect::new(0.0, 0.0, 1280.0, 720.0));

        let b = desktop.shell.window(wid).unwrap().bounds;
        assert!(
            b.x + b.width <= 1280.0 && b.y + b.height <= 720.0,
            "window must be clamped into the shrunken monitor; got x={} y={} w={} h={}",
            b.x,
            b.y,
            b.width,
            b.height
        );
    }
}
