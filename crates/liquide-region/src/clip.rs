//! Stack-based clip region for nested painting contexts.
//!
//! Mirrors the GDI clip region stack used by SaveDC/RestoreDC and
//! the nested BeginPaint/EndPaint pattern in NT compositing.

use crate::rect::Rect;
use crate::region::Region;

/// A clip region with a push/pop stack for nested clipping.
///
/// Each `push_clip` intersects a new rectangle with the current clip,
/// and `pop_clip` restores the previous state. This models the way
/// painting code narrows the clip as it descends into child windows.
#[derive(Debug, Clone)]
pub struct ClipRegion {
    /// Stack of clip regions. The last entry is the current effective clip.
    /// The stack always has at least one entry (the initial clip).
    stack: Vec<Region>,
}

impl ClipRegion {
    /// Create a clip region that starts with no clipping (FULL region).
    #[inline]
    pub fn new() -> Self {
        Self {
            stack: vec![Region::FULL],
        }
    }

    /// Create a clip region initialized to a specific rectangle.
    pub fn from_rect(rect: Rect) -> Self {
        Self {
            stack: vec![Region::from_rect(rect)],
        }
    }

    /// Create a clip region initialized to an existing region.
    pub fn from_region(region: Region) -> Self {
        Self {
            stack: vec![region],
        }
    }

    /// Push a rectangular clip, intersecting with the current clip.
    /// Returns the new effective clip region.
    pub fn push_clip(&mut self, rect: Rect) -> &Region {
        let current = self.stack.last().unwrap();
        let new_clip = current.intersect(&Region::from_rect(rect));
        self.stack.push(new_clip);
        self.stack.last().unwrap()
    }

    /// Push an arbitrary region clip, intersecting with the current clip.
    pub fn push_clip_region(&mut self, region: &Region) -> &Region {
        let current = self.stack.last().unwrap();
        let new_clip = current.intersect(region);
        self.stack.push(new_clip);
        self.stack.last().unwrap()
    }

    /// Pop the most recent clip, restoring the previous state.
    /// Panics if this would pop the initial clip (stack underflow).
    pub fn pop_clip(&mut self) {
        assert!(
            self.stack.len() > 1,
            "ClipRegion: cannot pop the initial clip"
        );
        self.stack.pop();
    }

    /// The current effective clip region.
    #[inline]
    pub fn current(&self) -> &Region {
        self.stack.last().unwrap()
    }

    /// True if `rect` would be at least partially visible through the
    /// current clip.
    pub fn is_visible(&self, rect: &Rect) -> bool {
        let clip = self.current();
        if clip.is_full() {
            return !rect.is_empty();
        }
        clip.intersects_rect(rect)
    }

    /// Current depth of the clip stack (1 = initial clip only).
    #[inline]
    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    /// Reset the clip region to its initial state (FULL, depth 1).
    pub fn reset(&mut self) {
        self.stack.clear();
        self.stack.push(Region::FULL);
    }
}

impl Default for ClipRegion {
    fn default() -> Self {
        Self::new()
    }
}
