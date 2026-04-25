//! Layout engine — flexbox-like layout computation.

use serde::{Deserialize, Serialize};

/// Layout direction (row or column).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Direction {
    Horizontal,
    Vertical,
}

impl Default for Direction {
    fn default() -> Self {
        Self::Vertical
    }
}

/// Alignment along the cross axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Alignment {
    Start,
    Center,
    End,
    Stretch,
}

impl Default for Alignment {
    fn default() -> Self {
        Self::Start
    }
}

/// Spacing between items.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Spacing(pub f32);

impl Default for Spacing {
    fn default() -> Self {
        Self(0.0)
    }
}

/// Padding on all four sides.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Padding {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Padding {
    pub const ZERO: Self = Self {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    };

    pub const fn uniform(v: f32) -> Self {
        Self {
            top: v,
            right: v,
            bottom: v,
            left: v,
        }
    }

    pub const fn symmetric(h: f32, v: f32) -> Self {
        Self {
            top: v,
            right: h,
            bottom: v,
            left: h,
        }
    }

    pub fn horizontal(&self) -> f32 {
        self.left + self.right
    }

    pub fn vertical(&self) -> f32 {
        self.top + self.bottom
    }
}

impl Default for Padding {
    fn default() -> Self {
        Self::ZERO
    }
}

/// A layout node representing a measured widget.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutNode {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl LayoutNode {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn zero() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        }
    }
}

/// Result of measuring a widget.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutResult {
    pub width: f32,
    pub height: f32,
    /// Baseline position from top (for text alignment).
    pub baseline: Option<f32>,
}

impl LayoutResult {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            width,
            height,
            baseline: None,
        }
    }

    pub fn with_baseline(mut self, baseline: f32) -> Self {
        self.baseline = Some(baseline);
        self
    }
}

/// A flexbox-like layout computation.
///
/// Given a set of measured children, compute their positions within
/// a container of known size.
pub fn flex_layout(
    container_width: f32,
    container_height: f32,
    direction: Direction,
    alignment: Alignment,
    spacing: f32,
    padding: &Padding,
    children: &[LayoutResult],
) -> Vec<LayoutNode> {
    let inner_w = container_width - padding.horizontal();
    let inner_h = container_height - padding.vertical();

    let _total_main: f32 = children
        .iter()
        .map(|c| match direction {
            Direction::Horizontal => c.width,
            Direction::Vertical => c.height,
        })
        .sum::<f32>()
        + spacing * (children.len().saturating_sub(1)) as f32;

    let mut pos = match direction {
        Direction::Horizontal => padding.left,
        Direction::Vertical => padding.top,
    };

    children
        .iter()
        .map(|child| {
            let (x, y, w, h) = match direction {
                Direction::Horizontal => {
                    let cross = match alignment {
                        Alignment::Start => padding.top,
                        Alignment::Center => padding.top + (inner_h - child.height) / 2.0,
                        Alignment::End => padding.top + inner_h - child.height,
                        Alignment::Stretch => padding.top,
                    };
                    let cross_h = if alignment == Alignment::Stretch {
                        inner_h
                    } else {
                        child.height
                    };
                    let node = (pos, cross, child.width, cross_h);
                    pos += child.width + spacing;
                    node
                }
                Direction::Vertical => {
                    let cross = match alignment {
                        Alignment::Start => padding.left,
                        Alignment::Center => padding.left + (inner_w - child.width) / 2.0,
                        Alignment::End => padding.left + inner_w - child.width,
                        Alignment::Stretch => padding.left,
                    };
                    let cross_w = if alignment == Alignment::Stretch {
                        inner_w
                    } else {
                        child.width
                    };
                    let node = (cross, pos, cross_w, child.height);
                    pos += child.height + spacing;
                    node
                }
            };
            LayoutNode::new(x, y, w, h)
        })
        .collect()
}
