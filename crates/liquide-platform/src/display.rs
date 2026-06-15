//! Display and monitor enumeration.
//!
//! Provides the [`DisplayBackend`] trait for querying connected monitors
//! and a [`NullDisplayBackend`] that returns empty results for testing.

use liquide_compositor::geometry::{Point, Rect};
use serde::{Deserialize, Serialize};

/// Information about a connected monitor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorInfo {
    /// Unique identifier for this monitor.
    pub id: u32,
    /// Human-readable name (e.g. "HDMI-1").
    pub name: String,
    /// Full geometry of the monitor in virtual screen coordinates.
    pub geometry: Rect,
    /// Usable work area excluding taskbars / panels.
    pub work_area: Rect,
    /// DPI scaling factor (1.0 = 96 DPI).
    pub dpi_scale: f32,
    /// Whether this is the primary monitor.
    pub primary: bool,
    /// Refresh rate in hertz.
    pub refresh_rate_hz: u32,
}

impl MonitorInfo {
    /// Whether the virtual-screen point lies within this monitor's geometry.
    #[must_use]
    pub fn contains_point(&self, point: Point) -> bool {
        self.geometry.contains(point)
    }
}

/// Backend for querying display / monitor information.
pub trait DisplayBackend: Send {
    /// Return information about all connected monitors.
    fn monitors(&self) -> Vec<MonitorInfo>;

    /// Return information about the primary monitor, if any.
    fn primary_monitor(&self) -> Option<MonitorInfo>;

    /// Return the bounding rectangle of the entire virtual screen
    /// (the union of all monitors).
    fn virtual_screen_rect(&self) -> Rect;

    // ── consumer helpers (default-implemented over `monitors()`) ─────────
    //
    // These give the session/shell everything needed to place windows and
    // chrome across monitors without reaching into backend internals. They are
    // pure geometry over the `MonitorInfo` set, so every backend (Win32, X11,
    // Wayland, macOS, DRM, Null) gets correct multi-monitor behaviour for free.

    /// Find the monitor that contains the virtual-screen point, if any.
    fn monitor_at_point(&self, point: Point) -> Option<MonitorInfo> {
        self.monitors().into_iter().find(|m| m.contains_point(point))
    }

    /// Find the monitor containing the point, falling back to the nearest
    /// monitor (by squared distance to centre) when the point lies in a gap or
    /// off every monitor. Returns `None` only when there are no monitors.
    ///
    /// Window placement should use this so a window dragged into a gap or
    /// off-screen still resolves to a real monitor instead of disappearing.
    fn monitor_at_point_or_nearest(&self, point: Point) -> Option<MonitorInfo> {
        let monitors = self.monitors();
        if let Some(hit) = monitors.iter().find(|m| m.contains_point(point)).cloned() {
            return Some(hit);
        }
        monitors.into_iter().min_by(|a, b| {
            let da = center_dist_sq(&a.geometry, point);
            let db = center_dist_sq(&b.geometry, point);
            da.total_cmp(&db)
        })
    }

    /// The usable work area of a monitor by id (geometry minus reserved chrome
    /// such as the taskbar). Returns `None` if no monitor has that id.
    fn work_area_of(&self, id: u32) -> Option<Rect> {
        self.monitors()
            .into_iter()
            .find(|m| m.id == id)
            .map(|m| m.work_area)
    }

    /// Relocate a window rectangle onto the target monitor's work area,
    /// preserving its position relative to its current monitor's work area where
    /// possible, then clamping so it stays fully inside the destination.
    ///
    /// This is the real "move window to monitor" primitive (replacing the
    /// single-screen proxy): a window 10% from the left of a 1080p monitor lands
    /// 10% from the left of a 4K monitor. Returns `None` if `target_id` is
    /// unknown.
    fn move_rect_to_monitor(&self, rect: Rect, target_id: u32) -> Option<Rect> {
        let monitors = self.monitors();
        let dst = monitors.iter().find(|m| m.id == target_id)?.work_area;

        // Resolve the source monitor from the rect's centre.
        let center = rect.center();
        let src = monitors
            .iter()
            .find(|m| m.contains_point(center))
            .filter(|m| m.id != target_id)
            .map(|m| m.work_area);

        let moved = match src {
            Some(src) if src.width > 0.0 && src.height > 0.0 => {
                let rel_x = (rect.x - src.x) / src.width;
                let rel_y = (rect.y - src.y) / src.height;
                Rect::new(
                    dst.x + rel_x * dst.width,
                    dst.y + rel_y * dst.height,
                    rect.width,
                    rect.height,
                )
            }
            _ => {
                // No usable source — centre in the destination work area.
                Rect::new(
                    dst.x + (dst.width - rect.width.min(dst.width)) / 2.0,
                    dst.y + (dst.height - rect.height.min(dst.height)) / 2.0,
                    rect.width.min(dst.width),
                    rect.height.min(dst.height),
                )
            }
        };
        Some(clamp_rect_into(moved, dst))
    }

    /// `true` when exactly one monitor is connected (single-monitor host).
    fn is_single_monitor(&self) -> bool {
        self.monitors().len() == 1
    }
}

/// Squared distance from a rectangle's centre to a point.
fn center_dist_sq(rect: &Rect, point: Point) -> f32 {
    let c = rect.center();
    let dx = c.x - point.x;
    let dy = c.y - point.y;
    dx * dx + dy * dy
}

/// Clamp `rect` so it fits entirely inside `bounds`, shrinking it first if it is
/// larger than the bounds, then translating it within them.
fn clamp_rect_into(rect: Rect, bounds: Rect) -> Rect {
    let w = rect.width.min(bounds.width);
    let h = rect.height.min(bounds.height);
    let max_x = (bounds.right() - w).max(bounds.x);
    let max_y = (bounds.bottom() - h).max(bounds.y);
    let x = rect.x.clamp(bounds.x, max_x);
    let y = rect.y.clamp(bounds.y, max_y);
    Rect::new(x, y, w, h)
}

/// A [`DisplayBackend`] that reports no monitors.
#[derive(Debug, Default)]
pub struct NullDisplayBackend;

impl DisplayBackend for NullDisplayBackend {
    fn monitors(&self) -> Vec<MonitorInfo> {
        Vec::new()
    }

    fn primary_monitor(&self) -> Option<MonitorInfo> {
        None
    }

    fn virtual_screen_rect(&self) -> Rect {
        Rect::ZERO
    }
}

#[cfg(test)]
mod helper_tests {
    use super::*;

    /// In-memory backend so the geometry helpers can be exercised on every
    /// platform (CI runs on Linux; the live Win32 backend cannot be enumerated
    /// in a headless test).
    struct MockDisplayBackend {
        monitors: Vec<MonitorInfo>,
    }

    impl DisplayBackend for MockDisplayBackend {
        fn monitors(&self) -> Vec<MonitorInfo> {
            self.monitors.clone()
        }
        fn primary_monitor(&self) -> Option<MonitorInfo> {
            self.monitors.iter().find(|m| m.primary).cloned()
        }
        fn virtual_screen_rect(&self) -> Rect {
            self.monitors
                .iter()
                .map(|m| m.geometry)
                .reduce(|a, b| a.union(&b))
                .unwrap_or(Rect::ZERO)
        }
    }

    fn mon(id: u32, x: f32, w: f32, primary: bool) -> MonitorInfo {
        MonitorInfo {
            id,
            name: format!("MON-{id}"),
            geometry: Rect::new(x, 0.0, w, 1080.0),
            // Reserve a 40px top panel in the work area.
            work_area: Rect::new(x, 40.0, w, 1040.0),
            dpi_scale: 1.0,
            primary,
            refresh_rate_hz: 60,
        }
    }

    /// A 720p monitor (smaller target for the clamp test).
    fn mon_720(id: u32, x: f32, w: f32) -> MonitorInfo {
        MonitorInfo {
            id,
            name: format!("MON-{id}"),
            geometry: Rect::new(x, 0.0, w, 720.0),
            work_area: Rect::new(x, 0.0, w, 720.0),
            dpi_scale: 1.0,
            primary: false,
            refresh_rate_hz: 60,
        }
    }

    fn dual() -> MockDisplayBackend {
        MockDisplayBackend {
            monitors: vec![mon(1, 0.0, 1920.0, true), mon(2, 1920.0, 1920.0, false)],
        }
    }

    #[test]
    fn enumerate_monitors() {
        let b = dual();
        assert_eq!(b.monitors().len(), 2);
        assert!(!b.is_single_monitor());
        assert_eq!(b.primary_monitor().unwrap().id, 1);
    }

    #[test]
    fn single_monitor_host() {
        let b = MockDisplayBackend {
            monitors: vec![mon(1, 0.0, 1920.0, true)],
        };
        assert!(b.is_single_monitor());
        // Off-screen point still resolves to the only monitor.
        assert!(b.monitor_at_point(Point::new(5000.0, 5000.0)).is_none());
        assert_eq!(
            b.monitor_at_point_or_nearest(Point::new(5000.0, 5000.0))
                .unwrap()
                .id,
            1
        );
    }

    #[test]
    fn point_to_monitor() {
        let b = dual();
        assert_eq!(b.monitor_at_point(Point::new(100.0, 100.0)).unwrap().id, 1);
        assert_eq!(b.monitor_at_point(Point::new(2000.0, 500.0)).unwrap().id, 2);
        assert!(b.monitor_at_point(Point::new(9999.0, 0.0)).is_none());
        // Off the right edge → nearest is monitor 2.
        assert_eq!(
            b.monitor_at_point_or_nearest(Point::new(9999.0, 500.0))
                .unwrap()
                .id,
            2
        );
    }

    #[test]
    fn work_area_lookup() {
        let b = dual();
        assert_eq!(b.work_area_of(1), Some(Rect::new(0.0, 40.0, 1920.0, 1040.0)));
        assert_eq!(
            b.work_area_of(2),
            Some(Rect::new(1920.0, 40.0, 1920.0, 1040.0))
        );
        assert!(b.work_area_of(99).is_none());
    }

    #[test]
    fn move_rect_remaps_into_target_work_area() {
        let b = dual();
        // Near the top-left of monitor 1's work area.
        let rect = Rect::new(0.0, 40.0, 400.0, 300.0);
        let moved = b.move_rect_to_monitor(rect, 2).unwrap();
        assert_eq!(moved.width, 400.0);
        assert_eq!(moved.height, 300.0);
        // Lands on monitor 2, respecting the panel.
        assert!(moved.x >= 1920.0, "x={}", moved.x);
        assert!(moved.y >= 40.0, "y={}", moved.y);
        let wa2 = b.work_area_of(2).unwrap();
        assert!(moved.right() <= wa2.right());
        assert!(moved.bottom() <= wa2.bottom());
    }

    #[test]
    fn move_rect_clamps_to_smaller_target() {
        let b = MockDisplayBackend {
            monitors: vec![
                MonitorInfo {
                    id: 1,
                    name: "4K".into(),
                    geometry: Rect::new(0.0, 0.0, 3840.0, 2160.0),
                    work_area: Rect::new(0.0, 0.0, 3840.0, 2160.0),
                    dpi_scale: 2.0,
                    primary: true,
                    refresh_rate_hz: 60,
                },
                mon_720(2, 3840.0, 1280.0),
            ],
        };
        let rect = Rect::new(2000.0, 1500.0, 1600.0, 1000.0);
        let moved = b.move_rect_to_monitor(rect, 2).unwrap();
        let wa2 = b.work_area_of(2).unwrap();
        assert!(moved.width <= wa2.width);
        assert!(moved.height <= wa2.height);
        assert!(moved.x >= wa2.x && moved.right() <= wa2.right());
        assert!(moved.y >= wa2.y && moved.bottom() <= wa2.bottom());
    }

    #[test]
    fn move_rect_to_unknown_monitor_is_none() {
        let b = dual();
        assert!(b.move_rect_to_monitor(Rect::new(0.0, 0.0, 10.0, 10.0), 99).is_none());
    }
}
