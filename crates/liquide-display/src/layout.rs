//! Multi-output desktop layout.
//!
//! [`arrangement`](crate::arrangement) models the *hardware* placement of
//! outputs in the virtual desktop. This module adds the layer a desktop
//! environment actually consumes on top of that:
//!
//! - **Work-area reservations** ([`WorkAreaInsets`]): the chrome (panels, docks,
//!   status bars) a shell reserves along each edge of a monitor, so windows can
//!   be laid out / maximized into the *usable* area rather than over the panels.
//! - **A single layout object** ([`DesktopLayout`]) that ties an arrangement to
//!   its per-monitor reservations and exposes the queries the shell/session need:
//!   enumerate outputs, primary, point → monitor, monitor → work area, and
//!   move-rect-to-monitor.
//!
//! Everything here is pure data + geometry — no platform calls — so it is fully
//! testable and reusable across the live compositor, the visual-test harness,
//! and the settings UI.

use crate::arrangement::{DisplayArrangement, primary_monitor};
use crate::display::{DisplayId, DisplayInfo};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A rectangle in virtual-desktop (logical) coordinates: `(x, y, width, height)`.
///
/// This mirrors the `(i32, i32, u32, u32)` tuple used throughout
/// [`DisplayArrangement`] but as a named type for the layout APIs, where passing
/// rectangles around as anonymous tuples is error-prone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    /// Construct a rectangle.
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Right edge (`x + width`).
    pub fn right(&self) -> i32 {
        self.x + self.width as i32
    }

    /// Bottom edge (`y + height`).
    pub fn bottom(&self) -> i32 {
        self.y + self.height as i32
    }

    /// Center point.
    pub fn center(&self) -> (i32, i32) {
        (self.x + self.width as i32 / 2, self.y + self.height as i32 / 2)
    }

    /// Whether the point `(px, py)` lies inside this rectangle (half-open: the
    /// right/bottom edges are exclusive, matching [`DisplayArrangement::display_at_point`]).
    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x && px < self.right() && py >= self.y && py < self.bottom()
    }

    /// Construct from the `(x, y, w, h)` tuple used by [`DisplayArrangement`].
    pub fn from_tuple(t: (i32, i32, u32, u32)) -> Self {
        Self::new(t.0, t.1, t.2, t.3)
    }

    /// Convert back to the `(x, y, w, h)` tuple convention.
    pub fn as_tuple(&self) -> (i32, i32, u32, u32) {
        (self.x, self.y, self.width, self.height)
    }

    /// Clamp this rectangle so it fits entirely inside `bounds`.
    ///
    /// The rect is first shrunk to be no larger than `bounds`, then translated so
    /// it sits within the bounds. Used when relocating a window to a smaller
    /// monitor so it never spills past the work area.
    pub fn clamp_into(&self, bounds: Rect) -> Rect {
        let w = self.width.min(bounds.width);
        let h = self.height.min(bounds.height);
        let max_x = bounds.right() - w as i32;
        let max_y = bounds.bottom() - h as i32;
        let x = self.x.clamp(bounds.x, max_x.max(bounds.x));
        let y = self.y.clamp(bounds.y, max_y.max(bounds.y));
        Rect::new(x, y, w, h)
    }
}

/// Pixels reserved by the shell along each edge of a monitor for chrome
/// (panels / docks / status bars). The remaining region is the *work area*.
///
/// All values are in the monitor's logical (virtual-desktop) pixels.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkAreaInsets {
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
    pub left: u32,
}

impl WorkAreaInsets {
    /// Construct insets for all four edges.
    pub const fn new(top: u32, right: u32, bottom: u32, left: u32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    /// Insets reserving only a top panel of `height` pixels.
    pub const fn top_panel(height: u32) -> Self {
        Self::new(height, 0, 0, 0)
    }

    /// Insets reserving only a bottom dock/taskbar of `height` pixels.
    pub const fn bottom_dock(height: u32) -> Self {
        Self::new(0, 0, height, 0)
    }

    /// Apply these insets to a monitor bounds rectangle, returning the usable
    /// work area. Insets larger than the monitor collapse to a zero-area rect
    /// pinned at the inset origin rather than producing a negative size.
    pub fn apply(&self, bounds: Rect) -> Rect {
        let horiz = self.left.saturating_add(self.right);
        let vert = self.top.saturating_add(self.bottom);
        let width = bounds.width.saturating_sub(horiz);
        let height = bounds.height.saturating_sub(vert);
        Rect::new(
            bounds.x + self.left as i32,
            bounds.y + self.top as i32,
            width,
            height,
        )
    }
}

/// A complete multi-output desktop layout: the hardware arrangement plus the
/// per-monitor chrome reservations the shell has applied.
///
/// This is the object the session hands to the shell so the shell can place
/// chrome and windows correctly across every monitor.
#[derive(Debug, Clone)]
pub struct DesktopLayout {
    arrangement: DisplayArrangement,
    /// Per-monitor reserved chrome. Missing entries default to no reservation.
    insets: HashMap<DisplayId, WorkAreaInsets>,
}

impl DesktopLayout {
    /// Build a layout from a set of detected outputs (no chrome reserved yet).
    pub fn new(displays: Vec<DisplayInfo>) -> Self {
        Self {
            arrangement: DisplayArrangement::new(displays),
            insets: HashMap::new(),
        }
    }

    /// Build a layout from an existing [`DisplayArrangement`].
    pub fn from_arrangement(arrangement: DisplayArrangement) -> Self {
        Self {
            arrangement,
            insets: HashMap::new(),
        }
    }

    /// Borrow the underlying hardware arrangement.
    pub fn arrangement(&self) -> &DisplayArrangement {
        &self.arrangement
    }

    /// Mutably borrow the underlying hardware arrangement (e.g. to re-arrange
    /// monitors). Insets are keyed by `DisplayId` and survive re-positioning.
    pub fn arrangement_mut(&mut self) -> &mut DisplayArrangement {
        &mut self.arrangement
    }

    // ── enumerate outputs ────────────────────────────────────────────────

    /// All enabled outputs, in arrangement order.
    pub fn outputs(&self) -> Vec<&DisplayInfo> {
        self.arrangement
            .displays
            .iter()
            .filter(|d| d.enabled)
            .collect()
    }

    /// Number of enabled outputs.
    pub fn output_count(&self) -> usize {
        self.arrangement.displays.iter().filter(|d| d.enabled).count()
    }

    /// Look up an output by id.
    pub fn output(&self, id: DisplayId) -> Option<&DisplayInfo> {
        self.arrangement.get(id)
    }

    /// `true` when exactly one output is enabled (single-monitor host).
    pub fn is_single_monitor(&self) -> bool {
        self.output_count() == 1
    }

    // ── primary ──────────────────────────────────────────────────────────

    /// The primary output's id, if any. Defers to
    /// [`primary_monitor`](crate::arrangement::primary_monitor) for the
    /// "marked-primary, else largest" policy.
    pub fn primary(&self) -> Option<DisplayId> {
        primary_monitor(&self.arrangement.displays)
    }

    /// The primary output, if any.
    pub fn primary_output(&self) -> Option<&DisplayInfo> {
        self.primary().and_then(|id| self.arrangement.get(id))
    }

    // ── point → monitor ──────────────────────────────────────────────────

    /// Which monitor contains the virtual-desktop point `(x, y)`.
    pub fn monitor_at_point(&self, x: i32, y: i32) -> Option<DisplayId> {
        self.arrangement.display_at_point(x, y)
    }

    /// Which monitor a point belongs to, falling back to the nearest monitor
    /// (by squared distance to monitor center) when the point lies in a gap or
    /// outside every monitor. Returns `None` only when there are no outputs.
    ///
    /// This is what window placement should use: a window dragged into a gap or
    /// off-screen still resolves to a real monitor instead of vanishing.
    pub fn monitor_at_point_or_nearest(&self, x: i32, y: i32) -> Option<DisplayId> {
        if let Some(id) = self.monitor_at_point(x, y) {
            return Some(id);
        }
        self.arrangement
            .displays
            .iter()
            .filter(|d| d.enabled)
            .min_by_key(|d| {
                let b = Rect::from_tuple(d.bounds());
                let (cx, cy) = b.center();
                let dx = (cx - x) as i64;
                let dy = (cy - y) as i64;
                dx * dx + dy * dy
            })
            .map(|d| d.id)
    }

    // ── monitor → work area ──────────────────────────────────────────────

    /// The full (chrome-inclusive) bounds of a monitor.
    pub fn monitor_bounds(&self, id: DisplayId) -> Option<Rect> {
        self.arrangement.get(id).map(|d| Rect::from_tuple(d.bounds()))
    }

    /// The usable work area of a monitor (full bounds minus reserved chrome).
    /// Returns `None` when the monitor id is unknown.
    pub fn work_area_of(&self, id: DisplayId) -> Option<Rect> {
        let bounds = self.monitor_bounds(id)?;
        let insets = self.insets.get(&id).copied().unwrap_or_default();
        Some(insets.apply(bounds))
    }

    /// Reserve chrome along the edges of a monitor. Returns `false` if the
    /// monitor id is unknown.
    pub fn set_insets(&mut self, id: DisplayId, insets: WorkAreaInsets) -> bool {
        if self.arrangement.get(id).is_none() {
            return false;
        }
        self.insets.insert(id, insets);
        true
    }

    /// The chrome reservation currently applied to a monitor (default = none).
    pub fn insets_of(&self, id: DisplayId) -> WorkAreaInsets {
        self.insets.get(&id).copied().unwrap_or_default()
    }

    /// Clear all chrome reservations (e.g. before re-applying the shell layout).
    pub fn clear_insets(&mut self) {
        self.insets.clear();
    }

    // ── move rect to monitor ─────────────────────────────────────────────

    /// Relocate a window rectangle onto the target monitor, preserving its
    /// position *relative to the work area* of its current monitor where
    /// possible, and clamping so it stays fully inside the destination work area.
    ///
    /// This is the real implementation behind a "move window to next monitor"
    /// command: it maps the rect from its source monitor's work area into the
    /// destination monitor's work area (so a window 10% from the left edge of a
    /// 1080p monitor lands 10% from the left edge of a 4K monitor), then clamps.
    ///
    /// Returns `None` if `target` is unknown. If the rect's current monitor can't
    /// be resolved, the rect is centered in the destination work area.
    pub fn move_rect_to_monitor(&self, rect: Rect, target: DisplayId) -> Option<Rect> {
        let dst = self.work_area_of(target)?;

        // Resolve the source monitor from the rect's center.
        let (cx, cy) = rect.center();
        let src = self
            .monitor_at_point_or_nearest(cx, cy)
            .filter(|&id| id != target)
            .and_then(|id| self.work_area_of(id));

        match src {
            Some(src) if src.width > 0 && src.height > 0 => {
                // Proportional remap of the rect's top-left within the work area.
                let rel_x = (rect.x - src.x) as f64 / src.width as f64;
                let rel_y = (rect.y - src.y) as f64 / src.height as f64;
                let new_x = dst.x + (rel_x * dst.width as f64).round() as i32;
                let new_y = dst.y + (rel_y * dst.height as f64).round() as i32;
                Some(Rect::new(new_x, new_y, rect.width, rect.height).clamp_into(dst))
            }
            _ => {
                // No usable source — center the window in the destination.
                let w = rect.width.min(dst.width);
                let h = rect.height.min(dst.height);
                let x = dst.x + (dst.width.saturating_sub(w) / 2) as i32;
                let y = dst.y + (dst.height.saturating_sub(h) / 2) as i32;
                Some(Rect::new(x, y, w, h))
            }
        }
    }

    /// The id of the monitor logically "next" after `from` (wrapping), ordered by
    /// arrangement position (left-to-right, then top-to-bottom). Used to implement
    /// "move window to next monitor". Returns `None` with fewer than two outputs.
    pub fn next_monitor(&self, from: DisplayId) -> Option<DisplayId> {
        self.ordered_monitor_step(from, 1)
    }

    /// The id of the monitor logically "previous" before `from` (wrapping).
    pub fn prev_monitor(&self, from: DisplayId) -> Option<DisplayId> {
        self.ordered_monitor_step(from, -1)
    }

    fn ordered_monitor_step(&self, from: DisplayId, step: i32) -> Option<DisplayId> {
        let mut ordered: Vec<&DisplayInfo> =
            self.arrangement.displays.iter().filter(|d| d.enabled).collect();
        if ordered.len() < 2 {
            return None;
        }
        ordered.sort_by_key(|d| {
            let (x, y, _, _) = d.bounds();
            (x, y)
        });
        let idx = ordered.iter().position(|d| d.id == from)?;
        let len = ordered.len() as i32;
        let next = (idx as i32 + step).rem_euclid(len) as usize;
        Some(ordered[next].id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::{Resolution, Rotation};

    fn make_display(id: DisplayId, connector: &str, w: u32, h: u32, x: i32, y: i32) -> DisplayInfo {
        DisplayInfo {
            id,
            name: format!("Display {id}"),
            connector: connector.to_string(),
            resolution: Resolution::new(w, h),
            available_resolutions: vec![Resolution::new(w, h)],
            refresh_rate: 60.0,
            available_refresh_rates: vec![60.0],
            position: (x, y),
            rotation: Rotation::Normal,
            scale: 1.0,
            primary: false,
            enabled: true,
            physical_size_mm: Some((600, 340)),
            connected: true,
        }
    }

    fn dual() -> DesktopLayout {
        DesktopLayout::new(vec![
            make_display(1, "DP-1", 1920, 1080, 0, 0),
            make_display(2, "HDMI-0", 1920, 1080, 1920, 0),
        ])
    }

    #[test]
    fn rect_geometry() {
        let r = Rect::new(10, 20, 100, 50);
        assert_eq!(r.right(), 110);
        assert_eq!(r.bottom(), 70);
        assert_eq!(r.center(), (60, 45));
        assert!(r.contains(10, 20));
        assert!(!r.contains(110, 20)); // right edge exclusive
    }

    #[test]
    fn rect_clamp_into_translates_and_shrinks() {
        let bounds = Rect::new(0, 0, 1920, 1080);
        // Off the right/bottom edge — should be pulled fully inside.
        let r = Rect::new(1900, 1000, 400, 300);
        let c = r.clamp_into(bounds);
        assert!(c.right() <= bounds.right());
        assert!(c.bottom() <= bounds.bottom());
        assert!(c.x >= 0 && c.y >= 0);
    }

    #[test]
    fn insets_apply_carves_work_area() {
        let bounds = Rect::new(0, 0, 1920, 1080);
        let insets = WorkAreaInsets::new(30, 0, 60, 0); // 30 top panel, 60 dock
        let wa = insets.apply(bounds);
        assert_eq!(wa, Rect::new(0, 30, 1920, 1080 - 90));
    }

    #[test]
    fn insets_oversized_do_not_underflow() {
        let bounds = Rect::new(0, 0, 100, 100);
        let insets = WorkAreaInsets::new(200, 200, 200, 200);
        let wa = insets.apply(bounds);
        assert_eq!(wa.width, 0);
        assert_eq!(wa.height, 0);
    }

    #[test]
    fn enumerate_outputs() {
        let layout = dual();
        assert_eq!(layout.output_count(), 2);
        assert!(!layout.is_single_monitor());
        let ids: Vec<DisplayId> = layout.outputs().iter().map(|d| d.id).collect();
        assert_eq!(ids, vec![1, 2]);
    }

    #[test]
    fn single_monitor_fallback() {
        let layout = DesktopLayout::new(vec![make_display(1, "eDP-1", 1920, 1080, 0, 0)]);
        assert!(layout.is_single_monitor());
        assert_eq!(layout.primary(), Some(1));
        // A point off the only screen still resolves to it via nearest.
        assert_eq!(layout.monitor_at_point(5000, 5000), None);
        assert_eq!(layout.monitor_at_point_or_nearest(5000, 5000), Some(1));
        // Moving to the same single monitor centers within its work area.
        let moved = layout
            .move_rect_to_monitor(Rect::new(10, 10, 400, 300), 1)
            .unwrap();
        assert!(Rect::new(0, 0, 1920, 1080).clamp_into(Rect::new(0, 0, 1920, 1080)).contains(moved.x, moved.y) || moved.x >= 0);
        // No "next" monitor exists.
        assert_eq!(layout.next_monitor(1), None);
    }

    #[test]
    fn primary_defaults_to_largest_then_marked() {
        let mut layout = dual();
        // Neither marked → largest pixel count; both equal → lowest id wins.
        assert_eq!(layout.primary(), Some(1));
        layout.arrangement_mut().set_primary(2);
        assert_eq!(layout.primary(), Some(2));
    }

    #[test]
    fn point_to_monitor() {
        let layout = dual();
        assert_eq!(layout.monitor_at_point(100, 100), Some(1));
        assert_eq!(layout.monitor_at_point(2000, 500), Some(2));
        assert_eq!(layout.monitor_at_point(9999, 0), None);
        // Off the right edge resolves to the nearest (monitor 2).
        assert_eq!(layout.monitor_at_point_or_nearest(9999, 500), Some(2));
    }

    #[test]
    fn work_area_reflects_insets() {
        let mut layout = dual();
        assert_eq!(
            layout.work_area_of(1),
            Some(Rect::new(0, 0, 1920, 1080))
        );
        assert!(layout.set_insets(1, WorkAreaInsets::top_panel(40)));
        assert_eq!(
            layout.work_area_of(1),
            Some(Rect::new(0, 40, 1920, 1040))
        );
        // Monitor 2 untouched.
        assert_eq!(layout.work_area_of(2), Some(Rect::new(1920, 0, 1920, 1080)));
        // Unknown monitor.
        assert!(!layout.set_insets(99, WorkAreaInsets::default()));
        assert_eq!(layout.work_area_of(99), None);
    }

    #[test]
    fn move_rect_to_monitor_remaps_proportionally() {
        let mut layout = dual();
        // Reserve a 40px top panel on both so work areas start at y=40.
        layout.set_insets(1, WorkAreaInsets::top_panel(40));
        layout.set_insets(2, WorkAreaInsets::top_panel(40));

        // A window near the top-left of monitor 1's work area.
        let rect = Rect::new(0, 40, 400, 300);
        let moved = layout.move_rect_to_monitor(rect, 2).unwrap();
        // It should land near the top-left of monitor 2's work area.
        assert_eq!(moved.width, 400);
        assert_eq!(moved.height, 300);
        assert!(moved.x >= 1920, "x={} should be on monitor 2", moved.x);
        assert!(moved.y >= 40, "y={} should respect the panel", moved.y);
        // And stay inside monitor 2's work area.
        let wa2 = layout.work_area_of(2).unwrap();
        assert!(moved.right() <= wa2.right());
        assert!(moved.bottom() <= wa2.bottom());
    }

    #[test]
    fn move_rect_to_monitor_clamps_to_smaller_target() {
        let mut layout = DesktopLayout::new(vec![
            make_display(1, "DP-1", 3840, 2160, 0, 0),
            make_display(2, "HDMI-0", 1280, 720, 3840, 0),
        ]);
        layout.arrangement_mut().get_mut(1).unwrap().primary = true;
        // A large window on the 4K monitor.
        let rect = Rect::new(2000, 1500, 1600, 1000);
        let moved = layout.move_rect_to_monitor(rect, 2).unwrap();
        let wa2 = layout.work_area_of(2).unwrap();
        // Must fit inside the smaller 720p monitor.
        assert!(moved.width <= wa2.width);
        assert!(moved.height <= wa2.height);
        assert!(moved.x >= wa2.x && moved.right() <= wa2.right());
        assert!(moved.y >= wa2.y && moved.bottom() <= wa2.bottom());
    }

    #[test]
    fn move_rect_to_unknown_monitor_is_none() {
        let layout = dual();
        assert!(layout.move_rect_to_monitor(Rect::new(0, 0, 100, 100), 99).is_none());
    }

    #[test]
    fn next_prev_monitor_wraps() {
        let layout = dual();
        assert_eq!(layout.next_monitor(1), Some(2));
        assert_eq!(layout.next_monitor(2), Some(1)); // wrap
        assert_eq!(layout.prev_monitor(1), Some(2)); // wrap
        assert_eq!(layout.prev_monitor(2), Some(1));
    }

    #[test]
    fn next_monitor_orders_by_position() {
        // Insertion order reversed relative to spatial order.
        let layout = DesktopLayout::new(vec![
            make_display(2, "HDMI-0", 1920, 1080, 1920, 0),
            make_display(1, "DP-1", 1920, 1080, 0, 0),
        ]);
        // Leftmost (id 1, x=0) → next is id 2 (x=1920).
        assert_eq!(layout.next_monitor(1), Some(2));
    }
}
