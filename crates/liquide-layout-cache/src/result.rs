//! Layout result — the cached output of a single node's layout pass.
//!
//! `LayoutResult` stores everything that downstream consumers (paint,
//! hit-testing, parent layout) need from a completed layout, and
//! `IntrinsicSizes` stores the intrinsic sizing information.

/// Cached intrinsic sizing for a node.
///
/// Intrinsic sizes only change when the *content* of the node changes
/// (text edit, child insertion, image load), not when the parent merely
/// offers a different available width.  They can therefore be cached
/// separately and more aggressively than full layout results.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct IntrinsicSizes {
    /// The narrowest the content can be without overflow.
    pub min_content_width: f32,
    /// The widest the content wants to be (no wrapping).
    pub max_content_width: f32,
    /// Optional min-content height (only meaningful for some elements).
    pub min_content_height: Option<f32>,
    /// Optional max-content height.
    pub max_content_height: Option<f32>,
}

impl IntrinsicSizes {
    pub fn new(min_content_width: f32, max_content_width: f32) -> Self {
        Self {
            min_content_width,
            max_content_width,
            min_content_height: None,
            max_content_height: None,
        }
    }

    pub fn with_height(
        mut self,
        min_content_height: f32,
        max_content_height: f32,
    ) -> Self {
        self.min_content_height = Some(min_content_height);
        self.max_content_height = Some(max_content_height);
        self
    }
}

/// The complete output of layout for a single node.
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutResult {
    /// Final size of the node's border box (width, height).
    pub size: (f32, f32),
    /// First baseline position (distance from the top of the border box).
    pub baseline: Option<f32>,
    /// Resolved margins (top, right, bottom, left).
    pub margins: (f32, f32, f32, f32),
    /// Position of each child's margin box relative to this node's content
    /// box origin.  Index corresponds to child order.
    pub child_offsets: Vec<(f32, f32)>,
    /// Scrollable overflow extent (width, height).
    pub overflow: (f32, f32),
    /// Cached intrinsic sizes computed during this layout pass.
    pub intrinsic_sizes: IntrinsicSizes,
}

impl Default for LayoutResult {
    fn default() -> Self {
        Self {
            size: (0.0, 0.0),
            baseline: None,
            margins: (0.0, 0.0, 0.0, 0.0),
            child_offsets: Vec::new(),
            overflow: (0.0, 0.0),
            intrinsic_sizes: IntrinsicSizes::default(),
        }
    }
}

impl LayoutResult {
    /// Create a result with the given border-box size.
    pub fn with_size(width: f32, height: f32) -> Self {
        Self {
            size: (width, height),
            overflow: (width, height),
            ..Default::default()
        }
    }

    /// The border-box width.
    pub fn width(&self) -> f32 {
        self.size.0
    }

    /// The border-box height.
    pub fn height(&self) -> f32 {
        self.size.1
    }

    /// Total margin-box width (border width + left + right margin).
    pub fn margin_box_width(&self) -> f32 {
        self.size.0 + self.margins.1 + self.margins.3
    }

    /// Total margin-box height (border height + top + bottom margin).
    pub fn margin_box_height(&self) -> f32 {
        self.size.1 + self.margins.0 + self.margins.2
    }
}
