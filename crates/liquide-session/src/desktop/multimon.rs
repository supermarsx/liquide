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

use liquide_display::display::{DisplayInfo, Resolution, Rotation};
use liquide_display::DesktopLayout;
use liquide_platform::display::MonitorInfo;
use liquide_platform::PlatformBackend;

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
    /// safe to call again (e.g. on a future `DisplaysChanged` event) to rebuild.
    pub(super) fn install_desktop_layout(&mut self, platform: &dyn PlatformBackend) {
        let monitors = platform.display().monitors();

        let displays: Vec<DisplayInfo> = if monitors.is_empty() {
            // Headless / Null backend: synthesize one logical monitor from the
            // virtual-screen rect so the shell still has a primary to anchor to.
            let r = platform.display().virtual_screen_rect();
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
        self.shell.set_desktop_layout(layout);
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
}
