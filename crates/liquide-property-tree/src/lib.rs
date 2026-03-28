//! Property trees for efficient compositing, hit testing, and damage tracking.
//!
//! This crate provides four independent hierarchical trees that decouple visual
//! effects from the layout tree:
//!
//! - **TransformTree** — hierarchical 2D affine transforms
//! - **ClipTree** — hierarchical clip regions (rect, rounded rect, circle, polygon)
//! - **EffectTree** — hierarchical opacity, blend modes, and filter effects
//! - **ScrollTree** — hierarchical scroll offsets (compositor-mutatable)
//!
//! Each tree supports:
//! - O(1) node lookup by ID
//! - Dirty tracking with top-down recomputation of cached accumulated values
//! - Per-node invalidation that propagates to descendants
//!
//! The [`PropertyTreeSet`] provides unified access to all four trees, plus
//! element-to-tree mappings for operations like hit testing and damage computation.

pub mod clip_tree;
pub mod effect_tree;
pub mod property_set;
pub mod scroll_tree;
pub mod transform;
pub mod transform_tree;

#[cfg(test)]
mod tests;

// Re-export primary types at crate root for convenience.
pub use clip_tree::{ClipChain, ClipChainEntry, ClipNode, ClipTree, ClipType};
pub use effect_tree::{BlendMode, EffectNode, EffectTree, FilterOp};
pub use property_set::{ElementId, NodeMapping, PropertyTreeSet};
pub use scroll_tree::{ScrollNode, ScrollTree};
pub use transform::Transform2D;
pub use transform_tree::{NodeId, TransformNode, TransformTree, ROOT_ID};

/// A rectangle used throughout the property tree system.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    /// Create a new rectangle.
    #[must_use]
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { x, y, width, height }
    }

    /// The zero rectangle.
    pub const ZERO: Self = Self { x: 0.0, y: 0.0, width: 0.0, height: 0.0 };

    /// Right edge.
    #[must_use]
    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    /// Bottom edge.
    #[must_use]
    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    /// Whether this rect contains a point.
    #[must_use]
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }

    /// Whether this rect intersects another.
    #[must_use]
    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.right()
            && self.right() > other.x
            && self.y < other.bottom()
            && self.bottom() > other.y
    }

    /// Intersection of two rects.
    #[must_use]
    pub fn intersection(&self, other: &Rect) -> Option<Rect> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        if right > x && bottom > y {
            Some(Rect::new(x, y, right - x, bottom - y))
        } else {
            None
        }
    }

    /// Union of two rects.
    #[must_use]
    pub fn union(&self, other: &Rect) -> Rect {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let right = self.right().max(other.right());
        let bottom = self.bottom().max(other.bottom());
        Rect::new(x, y, right - x, bottom - y)
    }

    /// Area.
    #[must_use]
    pub fn area(&self) -> f32 {
        self.width * self.height
    }

    /// Expand by a uniform margin.
    #[must_use]
    pub fn expand(&self, margin: f32) -> Self {
        Self {
            x: self.x - margin,
            y: self.y - margin,
            width: self.width + margin * 2.0,
            height: self.height + margin * 2.0,
        }
    }
}
