//! Direction-based focus and swap navigation for tiled windows.

use liquide_compositor::geometry::Rect;

use crate::layout::Direction;

/// Unique window identifier.
pub type WindowId = u64;

/// Find the window in `candidates` that is closest to `origin_rect` in the
/// given direction. Returns the window ID if one is found.
///
/// The algorithm:
/// 1. Filter candidates to those that are strictly in the requested direction
///    from the origin rect's center.
/// 2. Among those, pick the one whose center is closest (Euclidean distance).
#[must_use]
pub fn find_in_direction(
    direction: Direction,
    origin_rect: Rect,
    candidates: &[(WindowId, Rect)],
) -> Option<WindowId> {
    let oc = origin_rect.center();

    let mut best: Option<(WindowId, f32)> = None;

    for &(wid, ref rect) in candidates {
        let cc = rect.center();

        let in_direction = match direction {
            Direction::Up => cc.y < oc.y,
            Direction::Down => cc.y > oc.y,
            Direction::Left => cc.x < oc.x,
            Direction::Right => cc.x > oc.x,
        };

        if !in_direction {
            continue;
        }

        let dx = cc.x - oc.x;
        let dy = cc.y - oc.y;
        let dist = dx * dx + dy * dy;

        match best {
            Some((_, best_dist)) if dist < best_dist => {
                best = Some((wid, dist));
            }
            None => {
                best = Some((wid, dist));
            }
            _ => {}
        }
    }

    best.map(|(wid, _)| wid)
}

/// Find the index in a list closest to a given direction from a reference rect.
/// Used internally by the engine to map window IDs to positions.
#[must_use]
pub fn find_index_in_direction(
    direction: Direction,
    origin_idx: usize,
    positions: &[Rect],
) -> Option<usize> {
    if positions.is_empty() || origin_idx >= positions.len() {
        return None;
    }

    let origin = positions[origin_idx];
    let candidates: Vec<(WindowId, Rect)> = positions
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != origin_idx)
        .map(|(i, &r)| (i as WindowId, r))
        .collect();

    find_in_direction(direction, origin, &candidates).map(|id| id as usize)
}

/// Compute the next index in a circular list (wrapping forward).
#[must_use]
pub fn next_index(current: usize, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    (current + 1) % len
}

/// Compute the previous index in a circular list (wrapping backward).
#[must_use]
pub fn prev_index(current: usize, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    if current == 0 {
        len - 1
    } else {
        current - 1
    }
}
