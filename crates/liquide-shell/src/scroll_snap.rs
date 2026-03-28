//! Scroll snap enforcement — animates scroll containers to snap points
//! when a scroll gesture ends.
//!
//! The display list emits `ScrollContainerHints` with `scroll_snap_type`,
//! `scroll_snap_align`, and `scroll_snap_stop` but the shell never enforced
//! these.  This module collects snap points from the layout tree and animates
//! the scroll offset toward the nearest snap target.

use std::collections::HashMap;

use liquide_layout::tree::LayoutBoxId;
use liquide_style_engine::computed::{
    ScrollSnapAlign, ScrollSnapStrictness, ScrollSnapStop, ScrollSnapType,
};

/// Proximity threshold in logical pixels — if the current scroll position is
/// within this distance of a snap point, `proximity` mode will snap to it.
const PROXIMITY_THRESHOLD: f32 = 40.0;

/// Lerp factor per tick (0..1).  Higher = snappier animation.
const SNAP_LERP_FACTOR: f32 = 0.25;

/// When the remaining distance is below this threshold the animation finishes.
const SNAP_EPSILON: f32 = 0.5;

/// A single snap point derived from a child element.
#[derive(Debug, Clone, Copy)]
pub struct SnapPoint {
    /// Snap-aligned X scroll offset that would bring this child into alignment.
    pub x: f32,
    /// Snap-aligned Y scroll offset that would bring this child into alignment.
    pub y: f32,
    /// The alignment for this child.
    pub align: ScrollSnapAlign,
    /// Whether this point is a mandatory stop (scroll-snap-stop: always).
    pub stop: ScrollSnapStop,
}

/// Active snap animation state for a single scroll container.
#[derive(Debug, Clone, Copy)]
struct SnapAnimation {
    target_x: f32,
    target_y: f32,
}

/// Engine that manages scroll-snap behaviour for all scroll containers.
pub struct ScrollSnapEngine {
    /// Active snap animations: layout box id -> target scroll position.
    active_snaps: HashMap<LayoutBoxId, SnapAnimation>,
}

impl ScrollSnapEngine {
    /// Create a new (empty) snap engine.
    pub fn new() -> Self {
        Self {
            active_snaps: HashMap::new(),
        }
    }

    /// Called when a scroll gesture ends on a container that has snap type set.
    ///
    /// `current_x` / `current_y` — current scroll offset of the container.
    /// `viewport_w` / `viewport_h` — visible viewport size of the container.
    /// `snap_points` — snap positions computed from children.
    pub fn on_scroll_end(
        &mut self,
        container: LayoutBoxId,
        current_x: f32,
        current_y: f32,
        _viewport_w: f32,
        _viewport_h: f32,
        snap_type: ScrollSnapType,
        snap_points: &[SnapPoint],
    ) {
        if snap_points.is_empty() {
            return;
        }

        let (snap_x, snap_y) = match snap_type {
            ScrollSnapType::None => return,
            ScrollSnapType::X(strictness) => {
                let target_x = find_nearest_1d(current_x, snap_points, true, strictness);
                (target_x, None)
            }
            ScrollSnapType::Y(strictness) | ScrollSnapType::Block(strictness) => {
                let target_y = find_nearest_1d(current_y, snap_points, false, strictness);
                (None, target_y)
            }
            ScrollSnapType::Inline(strictness) => {
                let target_x = find_nearest_1d(current_x, snap_points, true, strictness);
                (target_x, None)
            }
            ScrollSnapType::Both(strictness) => {
                let target_x = find_nearest_1d(current_x, snap_points, true, strictness);
                let target_y = find_nearest_1d(current_y, snap_points, false, strictness);
                (target_x, target_y)
            }
        };

        let tx = snap_x.unwrap_or(current_x);
        let ty = snap_y.unwrap_or(current_y);

        // Only start animation if the target is different from current
        if (tx - current_x).abs() > SNAP_EPSILON || (ty - current_y).abs() > SNAP_EPSILON {
            self.active_snaps.insert(
                container,
                SnapAnimation {
                    target_x: tx,
                    target_y: ty,
                },
            );
        }
    }

    /// Advance all active snap animations by one tick.
    ///
    /// `dt_ms` is the elapsed time in milliseconds (used for future
    /// time-based interpolation; currently ignored in favour of a fixed
    /// lerp factor).
    ///
    /// Returns a list of `(container_box_id, new_scroll_x, new_scroll_y)`
    /// positions that the caller should apply.
    pub fn tick(
        &mut self,
        _dt_ms: f32,
        current_offsets: &HashMap<LayoutBoxId, (f32, f32)>,
    ) -> Vec<(LayoutBoxId, f32, f32)> {
        let mut updates = Vec::new();
        let mut finished = Vec::new();

        for (&box_id, anim) in &self.active_snaps {
            let (cx, cy) = current_offsets.get(&box_id).copied().unwrap_or((0.0, 0.0));

            let new_x = cx + (anim.target_x - cx) * SNAP_LERP_FACTOR;
            let new_y = cy + (anim.target_y - cy) * SNAP_LERP_FACTOR;

            let dx = (anim.target_x - new_x).abs();
            let dy = (anim.target_y - new_y).abs();

            if dx < SNAP_EPSILON && dy < SNAP_EPSILON {
                // Snap exactly to target and finish
                updates.push((box_id, anim.target_x, anim.target_y));
                finished.push(box_id);
            } else {
                updates.push((box_id, new_x, new_y));
            }
        }

        for id in finished {
            self.active_snaps.remove(&id);
        }

        updates
    }

    /// Whether there are active snap animations that need ticking.
    pub fn has_active_snaps(&self) -> bool {
        !self.active_snaps.is_empty()
    }

    /// Cancel any active snap animation for the given container (e.g. when
    /// the user starts a new scroll gesture).
    pub fn cancel(&mut self, container: LayoutBoxId) {
        self.active_snaps.remove(&container);
    }
}

/// Compute snap points for children of a scroll container.
///
/// `container_offset_x/y` is the content-area origin of the container.
/// `viewport_w/h` is the visible viewport size.
/// Children's border rects are relative to the container's content area.
pub fn compute_snap_points(
    children_rects: &[(f32, f32, f32, f32)], // (x, y, width, height) relative to container content
    children_aligns: &[ScrollSnapAlign],
    children_stops: &[ScrollSnapStop],
    viewport_w: f32,
    viewport_h: f32,
) -> Vec<SnapPoint> {
    let mut points = Vec::with_capacity(children_rects.len());
    for (i, &(cx, cy, cw, ch)) in children_rects.iter().enumerate() {
        let align = children_aligns.get(i).copied().unwrap_or(ScrollSnapAlign::None);
        let stop = children_stops.get(i).copied().unwrap_or(ScrollSnapStop::Normal);

        if align == ScrollSnapAlign::None {
            continue;
        }

        // Compute the scroll offset that would bring this child into alignment
        let snap_x = match align {
            ScrollSnapAlign::Start => cx,
            ScrollSnapAlign::Center => cx + cw / 2.0 - viewport_w / 2.0,
            ScrollSnapAlign::End => cx + cw - viewport_w,
            ScrollSnapAlign::None => 0.0,
        };
        let snap_y = match align {
            ScrollSnapAlign::Start => cy,
            ScrollSnapAlign::Center => cy + ch / 2.0 - viewport_h / 2.0,
            ScrollSnapAlign::End => cy + ch - viewport_h,
            ScrollSnapAlign::None => 0.0,
        };

        points.push(SnapPoint {
            x: snap_x.max(0.0),
            y: snap_y.max(0.0),
            align,
            stop,
        });
    }
    points
}

/// Find the nearest snap coordinate along one axis.
fn find_nearest_1d(
    current: f32,
    points: &[SnapPoint],
    is_x: bool,
    strictness: ScrollSnapStrictness,
) -> Option<f32> {
    let mut best: Option<(f32, f32)> = None; // (distance, value)

    for pt in points {
        if pt.align == ScrollSnapAlign::None {
            continue;
        }
        let val = if is_x { pt.x } else { pt.y };
        let dist = (val - current).abs();
        if best.is_none() || dist < best.unwrap().0 {
            best = Some((dist, val));
        }
    }

    match (strictness, best) {
        (ScrollSnapStrictness::Mandatory, Some((_, val))) => Some(val),
        (ScrollSnapStrictness::Proximity, Some((dist, val))) if dist <= PROXIMITY_THRESHOLD => {
            Some(val)
        }
        _ => None,
    }
}

impl Default for ScrollSnapEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mandatory_snaps_to_nearest() {
        let mut engine = ScrollSnapEngine::new();
        let points = vec![
            SnapPoint { x: 0.0, y: 0.0, align: ScrollSnapAlign::Start, stop: ScrollSnapStop::Normal },
            SnapPoint { x: 0.0, y: 300.0, align: ScrollSnapAlign::Start, stop: ScrollSnapStop::Normal },
            SnapPoint { x: 0.0, y: 600.0, align: ScrollSnapAlign::Start, stop: ScrollSnapStop::Normal },
        ];

        engine.on_scroll_end(
            0, 0.0, 170.0, 400.0, 400.0,
            ScrollSnapType::Y(ScrollSnapStrictness::Mandatory),
            &points,
        );

        assert!(engine.has_active_snaps());

        // Simulate ticking until settled
        let mut offsets = HashMap::new();
        offsets.insert(0usize, (0.0f32, 170.0f32));
        for _ in 0..50 {
            let updates = engine.tick(16.0, &offsets);
            for (id, x, y) in &updates {
                offsets.insert(*id, (*x, *y));
            }
            if !engine.has_active_snaps() {
                break;
            }
        }

        let (_, final_y) = offsets[&0usize];
        // Should snap to 300.0 (nearest to 170.0: distance to 0 is 170, distance to 300 is 130)
        assert!((final_y - 300.0).abs() < 1.0, "Expected snap to 300, got {}", final_y);
    }

    #[test]
    fn proximity_ignores_distant_points() {
        let mut engine = ScrollSnapEngine::new();
        let points = vec![
            SnapPoint { x: 0.0, y: 0.0, align: ScrollSnapAlign::Start, stop: ScrollSnapStop::Normal },
            SnapPoint { x: 0.0, y: 500.0, align: ScrollSnapAlign::Start, stop: ScrollSnapStop::Normal },
        ];

        // Current position is 250 — equidistant from both, but well beyond threshold
        engine.on_scroll_end(
            0, 0.0, 250.0, 400.0, 400.0,
            ScrollSnapType::Y(ScrollSnapStrictness::Proximity),
            &points,
        );

        // Should NOT snap because nearest is 250px away (> 40px threshold)
        assert!(!engine.has_active_snaps());
    }

    #[test]
    fn proximity_snaps_when_close() {
        let mut engine = ScrollSnapEngine::new();
        let points = vec![
            SnapPoint { x: 0.0, y: 300.0, align: ScrollSnapAlign::Start, stop: ScrollSnapStop::Normal },
        ];

        // Current position is 320 — 20px away from snap point (within threshold)
        engine.on_scroll_end(
            0, 0.0, 320.0, 400.0, 400.0,
            ScrollSnapType::Y(ScrollSnapStrictness::Proximity),
            &points,
        );

        assert!(engine.has_active_snaps());
    }

    #[test]
    fn compute_snap_points_center_align() {
        let rects = vec![(0.0, 0.0, 200.0, 400.0), (0.0, 400.0, 200.0, 400.0)];
        let aligns = vec![ScrollSnapAlign::Center, ScrollSnapAlign::Center];
        let stops = vec![ScrollSnapStop::Normal, ScrollSnapStop::Normal];
        let points = compute_snap_points(&rects, &aligns, &stops, 200.0, 400.0);

        assert_eq!(points.len(), 2);
        // First child center: 200 - snap_y = 0 + 200 - 200 = 0
        assert!((points[0].y - 0.0).abs() < 0.01);
        // Second child center: 400 + 200 - 200 = 400
        assert!((points[1].y - 400.0).abs() < 0.01);
    }
}
