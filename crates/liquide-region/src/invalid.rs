//! Per-window invalid region tracker.
//!
//! Tracks the per-window update region managed by invalidate / validate /
//! begin-paint / end-paint operations.

use crate::rect::Rect;
use crate::region::Region;

/// Tracks the invalid (needs-repaint) region of a single window.
///
/// # Usage pattern
///
/// ```ignore
/// // Something changes — mark a rectangle dirty:
/// invalid.invalidate(Some(dirty_rect));
///
/// // On paint:
/// if invalid.is_dirty() {
///     let region = invalid.take(); // atomically grabs & clears
///     // paint using region as clip...
/// }
/// ```
#[derive(Debug, Clone)]
pub struct InvalidRegion {
    region: Region,
}

impl InvalidRegion {
    /// Create a new, clean (no invalid area) tracker.
    #[inline]
    pub fn new() -> Self {
        Self {
            region: Region::empty(),
        }
    }

    /// Create a tracker that starts fully invalid (entire window needs paint).
    #[inline]
    pub fn new_full() -> Self {
        Self {
            region: Region::FULL,
        }
    }

    /// Add a rectangle to the invalid region. Pass `None` to invalidate the
    /// entire window.
    pub fn invalidate(&mut self, rect: Option<Rect>) {
        match rect {
            None => {
                // Invalidate everything.
                self.region = Region::FULL;
            }
            Some(r) => {
                if r.is_empty() {
                    return;
                }
                if self.region.is_full() {
                    return; // already fully invalid
                }
                self.region = self.region.union(&Region::from_rect(r));
            }
        }
    }

    /// Remove a rectangle from the invalid region, marking it as valid
    /// (already painted). Pass `None` to validate the entire window.
    pub fn validate(&mut self, rect: Option<Rect>) {
        match rect {
            None => {
                self.region = Region::empty();
            }
            Some(r) => {
                if r.is_empty() {
                    return;
                }
                if self.region.is_empty() {
                    return;
                }
                // FULL minus a finite rect is still FULL in our model,
                // since FULL represents infinite area. Callers should
                // resolve FULL to window bounds first if they need
                // precise subtraction.
                if self.region.is_full() {
                    return;
                }
                self.region = self.region.subtract(&Region::from_rect(r));
            }
        }
    }

    /// True if there is any invalid area that needs painting.
    #[inline]
    pub fn is_dirty(&self) -> bool {
        !self.region.is_empty()
    }

    /// Take the current invalid region and clear it, returning ownership.
    /// This is the "BeginPaint" side — the caller gets the region to use
    /// as a paint clip, and the tracker is reset to clean.
    #[inline]
    pub fn take(&mut self) -> Region {
        std::mem::replace(&mut self.region, Region::empty())
    }

    /// Peek at the current invalid region without clearing it.
    #[inline]
    pub fn region(&self) -> &Region {
        &self.region
    }
}

impl Default for InvalidRegion {
    fn default() -> Self {
        Self::new()
    }
}
