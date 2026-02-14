//! Dirty tracking flags for incremental style/layout/paint.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::NodeId;

/// Per-node dirty flags.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirtyFlags {
    bits: u8,
}

impl DirtyFlags {
    const STYLE: u8 = 0x01;
    const LAYOUT: u8 = 0x02;
    const PAINT: u8 = 0x04;
    const SUBTREE_STYLE: u8 = 0x08;
    const SUBTREE_LAYOUT: u8 = 0x10;

    /// No flags set.
    pub fn clean() -> Self {
        Self { bits: 0 }
    }

    /// All dirty.
    pub fn all_dirty() -> Self {
        Self { bits: 0x1F }
    }

    pub fn needs_style(&self) -> bool {
        self.bits & Self::STYLE != 0
    }
    pub fn needs_layout(&self) -> bool {
        self.bits & Self::LAYOUT != 0
    }
    pub fn needs_paint(&self) -> bool {
        self.bits & Self::PAINT != 0
    }
    pub fn subtree_needs_style(&self) -> bool {
        self.bits & Self::SUBTREE_STYLE != 0
    }
    pub fn subtree_needs_layout(&self) -> bool {
        self.bits & Self::SUBTREE_LAYOUT != 0
    }

    pub fn mark_style_dirty(&mut self) {
        self.bits |= Self::STYLE | Self::LAYOUT | Self::PAINT;
    }
    pub fn mark_layout_dirty(&mut self) {
        self.bits |= Self::LAYOUT | Self::PAINT;
    }
    pub fn mark_paint_dirty(&mut self) {
        self.bits |= Self::PAINT;
    }
    pub fn mark_subtree_style_dirty(&mut self) {
        self.bits |= Self::SUBTREE_STYLE;
    }
    pub fn mark_subtree_layout_dirty(&mut self) {
        self.bits |= Self::SUBTREE_LAYOUT;
    }

    pub fn clear_style(&mut self) {
        self.bits &= !Self::STYLE;
    }
    pub fn clear_layout(&mut self) {
        self.bits &= !Self::LAYOUT;
    }
    pub fn clear_paint(&mut self) {
        self.bits &= !Self::PAINT;
    }
    pub fn clear_subtree_style(&mut self) {
        self.bits &= !Self::SUBTREE_STYLE;
    }
    pub fn clear_subtree_layout(&mut self) {
        self.bits &= !Self::SUBTREE_LAYOUT;
    }
    pub fn clear_all(&mut self) {
        self.bits = 0;
    }

    pub fn is_clean(&self) -> bool {
        self.bits == 0
    }
    pub fn any_dirty(&self) -> bool {
        self.bits != 0
    }
}

/// Document-level set of dirty nodes for batch processing.
#[derive(Debug, Clone, Default)]
pub struct DirtySet {
    /// Nodes needing style recalculation.
    pub style: HashSet<NodeId>,
    /// Nodes needing layout.
    pub layout: HashSet<NodeId>,
    /// Nodes needing repaint.
    pub paint: HashSet<NodeId>,
}

impl DirtySet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mark_style(&mut self, node: NodeId) {
        self.style.insert(node);
        self.layout.insert(node);
        self.paint.insert(node);
    }

    pub fn mark_layout(&mut self, node: NodeId) {
        self.layout.insert(node);
        self.paint.insert(node);
    }

    pub fn mark_paint(&mut self, node: NodeId) {
        self.paint.insert(node);
    }

    pub fn clear_style(&mut self) {
        self.style.clear();
    }

    pub fn clear_layout(&mut self) {
        self.layout.clear();
    }

    pub fn clear_paint(&mut self) {
        self.paint.clear();
    }

    pub fn clear_all(&mut self) {
        self.style.clear();
        self.layout.clear();
        self.paint.clear();
    }

    pub fn has_work(&self) -> bool {
        !self.style.is_empty() || !self.layout.is_empty() || !self.paint.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_cascade() {
        let mut f = DirtyFlags::clean();
        assert!(f.is_clean());
        f.mark_style_dirty();
        assert!(f.needs_style());
        assert!(f.needs_layout());
        assert!(f.needs_paint());
    }

    #[test]
    fn set_tracking() {
        let mut set = DirtySet::new();
        set.mark_style(1);
        assert!(set.has_work());
        set.clear_all();
        assert!(!set.has_work());
    }
}
