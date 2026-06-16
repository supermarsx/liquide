//! Inescapable per-thread framebuffer write-scissor (t84 / formerly t80).
//!
//! This is the SINGLE source of truth for damage-only confinement. On a
//! partial-damage frame the renderer installs the damage bounding box here once
//! and clears it (`None`) afterwards.
//!
//! Unlike the per-node `clip` arguments — which several node-paint paths simply
//! forgot to honour (the t79/t83-R1 stale-pixel regression class) — the scissor
//! is consulted by [`FrameBuffer::set_pixel`] **itself** (and the
//! window-clamp/per-pixel helpers below). Therefore ANY pixel write, regardless
//! of which crate or codepath issued it (renderer-cpu raster helpers, raw
//! get-modify-set node loops, blur write-back, future node kinds), is physically
//! dropped if it falls outside the active scissor. The invariant "no node
//! escapes the damage clip" is enforced by the type system / write primitive,
//! not by convention.
//!
//! On a full-damage frame the scissor is `None` and every write path is a true
//! no-op branch (one predictable, almost-always-not-taken compare) — byte
//! identical to the unclipped path.
//!
//! The scissor is stored as an inclusive-exclusive **integer pixel window**
//! `[x0, x1) × [y0, y1)` so the hot-path per-pixel test in `set_pixel` is a
//! plain `u32` comparison with no float conversion.

use crate::geometry::Rect;

thread_local! {
    /// Per-thread integer write-scissor window `[x0, x1) × [y0, y1)`.
    ///
    /// `None` means unconstrained (full-damage frame). Stored as integers so the
    /// per-write check is branch-predictable and conversion-free.
    static WRITE_SCISSOR: std::cell::Cell<Option<(u32, u32, u32, u32)>> =
        const { std::cell::Cell::new(None) };
}

/// Convert a float [`Rect`] damage bound to the inclusive-exclusive integer
/// pixel window used by the scissor. `NaN`/negative saturate to 0; the right/
/// bottom edges are `ceil`-ed so a partially-covered edge pixel is still inside
/// the clip (conservative — never narrows past truth).
#[inline]
#[must_use]
fn rect_to_window(s: Rect) -> (u32, u32, u32, u32) {
    let sx0 = s.x.max(0.0) as u32;
    let sy0 = s.y.max(0.0) as u32;
    let sx1 = s.right().ceil().max(0.0) as u32;
    let sy1 = s.bottom().ceil().max(0.0) as u32;
    (sx0, sy0, sx1, sy1)
}

/// Install the per-thread framebuffer write-scissor. Returns the previous value
/// so the caller can restore it. Passing `None` removes the scissor.
///
/// While set, every [`FrameBuffer::set_pixel`](crate::framebuffer::FrameBuffer::set_pixel)
/// drops writes outside the rect, and the [`scissor_clamp_window`] /
/// [`scissor_allows`] helpers confine loop bounds / per-pixel writes.
pub fn set_write_scissor(scissor: Option<Rect>) -> Option<Rect> {
    let prev = WRITE_SCISSOR.with(|c| c.replace(scissor.map(rect_to_window)));
    prev.map(|(x0, y0, x1, y1)| {
        Rect::new(x0 as f32, y0 as f32, (x1 - x0) as f32, (y1 - y0) as f32)
    })
}

/// The currently-installed write-scissor for this thread as an integer window,
/// if any. Used by the hot-path pixel write in `set_pixel`.
#[inline]
#[must_use]
pub fn write_scissor_window() -> Option<(u32, u32, u32, u32)> {
    WRITE_SCISSOR.with(std::cell::Cell::get)
}

/// The currently-installed write-scissor for this thread as a [`Rect`], if any.
#[inline]
#[must_use]
pub fn write_scissor() -> Option<Rect> {
    write_scissor_window()
        .map(|(x0, y0, x1, y1)| Rect::new(x0 as f32, y0 as f32, (x1 - x0) as f32, (y1 - y0) as f32))
}

/// Clamp an integer pixel write-window `[x0,x1) × [y0,y1)` to the active
/// write-scissor. Returns the (possibly empty) intersected window; unchanged
/// when no scissor is set. Empty windows have `x1 <= x0` or `y1 <= y0`; callers
/// must guard against drawing into them.
#[inline]
#[must_use]
pub fn scissor_clamp_window(x0: u32, y0: u32, x1: u32, y1: u32) -> (u32, u32, u32, u32) {
    match write_scissor_window() {
        None => (x0, y0, x1, y1),
        Some((sx0, sy0, sx1, sy1)) => (x0.max(sx0), y0.max(sy0), x1.min(sx1), y1.min(sy1)),
    }
}

/// Whether a single pixel `(x, y)` is permitted by the active write-scissor.
/// Always `true` when no scissor is set.
#[inline]
#[must_use]
pub fn scissor_allows(x: u32, y: u32) -> bool {
    match write_scissor_window() {
        None => true,
        Some((sx0, sy0, sx1, sy1)) => x >= sx0 && x < sx1 && y >= sy0 && y < sy1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_is_passthrough() {
        let _ = set_write_scissor(None);
        assert_eq!(scissor_clamp_window(2, 3, 10, 20), (2, 3, 10, 20));
        assert!(scissor_allows(0, 0));
        assert!(scissor_allows(1000, 1000));
        assert!(write_scissor_window().is_none());
    }

    #[test]
    fn window_clamps_and_restores() {
        let prev = set_write_scissor(Some(Rect::new(5.0, 5.0, 10.0, 10.0)));
        assert!(prev.is_none());
        // [5,15) x [5,15)
        assert_eq!(scissor_clamp_window(0, 0, 100, 100), (5, 5, 15, 15));
        assert!(!scissor_allows(4, 10));
        assert!(scissor_allows(5, 5));
        assert!(scissor_allows(14, 14));
        assert!(!scissor_allows(15, 15));
        // restore
        let prev2 = set_write_scissor(None);
        assert_eq!(prev2, Some(Rect::new(5.0, 5.0, 10.0, 10.0)));
    }

    #[test]
    fn nan_and_negative_saturate() {
        let _ = set_write_scissor(Some(Rect::new(f32::NAN, -3.0, 10.0, 10.0)));
        // NaN.max(0.0) == 0.0, negative saturates to 0
        let (x0, y0, _, _) = scissor_clamp_window(0, 0, 100, 100);
        assert_eq!((x0, y0), (0, 0));
        let _ = set_write_scissor(None);
    }
}
