//! PaintContext — BeginPaint/EndPaint equivalent.
//!
//! This mirrors the Win32 painting model where BeginPaint takes the
//! invalid region from the window, provides it as a clip for painting,
//! and EndPaint validates the painted area.

use crate::invalid::InvalidRegion;
use crate::rect::Rect;
use crate::region::Region;

/// Opaque identifier for a window. The actual type is determined by
/// the windowing layer; we use u64 to avoid coupling to any specific
/// window manager.
pub type WindowId = u64;

/// Context returned by `begin_paint`, consumed by `end_paint`.
///
/// Holds the invalid region snapshot so that EndPaint can validate
/// exactly what was claimed for painting.
#[derive(Debug)]
pub struct PaintContext {
    /// The window being painted.
    pub window_id: WindowId,
    /// Bounding rectangle of the invalid region at the time BeginPaint
    /// was called. Callers can use this as a fast dirty-rect check.
    pub update_rect: Rect,
    /// The actual invalid region, usable as a clip mask for painting.
    pub clip: Region,
    /// True if the background needs erasing (the region was newly
    /// invalidated and no erase has been done yet).
    pub erase_background: bool,
}

/// Begin a paint operation on a window.
///
/// This atomically takes the window's invalid region (clearing it),
/// computes the bounding update rect, and packages them into a
/// `PaintContext` that the painting code can use.
///
/// Equivalent to Win32 `BeginPaint`:
/// - Takes hrgnUpdate from the window
/// - Sets the DC clip region to the update region
/// - Returns the PAINTSTRUCT with rcPaint = bounding rect
///
/// # Arguments
/// - `window_id`: Identifier of the window being painted.
/// - `invalid`: The window's invalid region tracker (will be cleared).
///
/// Returns `None` if there is nothing to paint.
pub fn begin_paint(window_id: WindowId, invalid: &mut InvalidRegion) -> Option<PaintContext> {
    if !invalid.is_dirty() {
        return None;
    }

    let region = invalid.take();

    // Compute the bounding update rect.
    let update_rect = if region.is_full() {
        // FULL region: the caller will need to paint everything. We
        // return a maximal rect. The caller should intersect with the
        // actual window bounds.
        Rect {
            left: i32::MIN / 2,
            top: i32::MIN / 2,
            right: i32::MAX / 2,
            bottom: i32::MAX / 2,
        }
    } else {
        match region.bounding_rect() {
            Some(r) => r,
            None => return None, // empty region after take (shouldn't happen)
        }
    };

    Some(PaintContext {
        window_id,
        update_rect,
        clip: region,
        erase_background: true,
    })
}

/// Finish a paint operation.
///
/// If additional invalidation occurred during painting (e.g., another
/// thread called `invalidate`), those new regions are NOT affected —
/// they remain in the `InvalidRegion` for the next paint cycle.
///
/// Equivalent to Win32 `EndPaint`.
///
/// # Arguments
/// - `ctx`: The paint context from `begin_paint` (consumed).
/// - `_invalid`: The window's invalid region tracker (currently unused,
///   but present for future use if partial validation is needed).
pub fn end_paint(ctx: PaintContext, _invalid: &mut InvalidRegion) {
    // In the current model, begin_paint already cleared the invalid
    // region via take(). EndPaint is here for API symmetry and to
    // consume the PaintContext so it can't be reused.
    //
    // If we wanted Win32-exact semantics where EndPaint validates
    // only the update region (allowing intermediate invalidations
    // to survive), we would do:
    //   invalid.validate_region(&ctx.clip);
    // But since take() already cleared it, any new invalidations
    // that arrived between begin/end are already separate.
    let _ = ctx;
}

/// Begin a paint with a known window size, resolving FULL to actual bounds.
///
/// This is a convenience wrapper that converts a FULL invalid region to
/// the actual window rectangle before returning.
pub fn begin_paint_bounded(
    window_id: WindowId,
    invalid: &mut InvalidRegion,
    window_rect: Rect,
) -> Option<PaintContext> {
    if !invalid.is_dirty() {
        return None;
    }

    let region = invalid.take();

    let (clip, update_rect) = if region.is_full() {
        let clip = Region::from_rect(window_rect);
        (clip, window_rect)
    } else {
        let update_rect = match region.bounding_rect() {
            Some(r) => r,
            None => return None,
        };
        (region, update_rect)
    };

    Some(PaintContext {
        window_id,
        update_rect,
        clip,
        erase_background: true,
    })
}
