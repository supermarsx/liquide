//! Anchor configuration for positioned popups.
//!
//! An anchor attaches a popup to a reference element (button, widget, etc.)
//! and specifies which edge and alignment to use. The positioner uses
//! `flip` and `slide` to keep the popup on-screen.

use crate::Rect;

/// Which edge of the anchor element the popup attaches to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Edge {
    /// Popup appears above the anchor.
    Top,
    /// Popup appears below the anchor.
    Bottom,
    /// Popup appears to the left of the anchor.
    Left,
    /// Popup appears to the right of the anchor.
    Right,
}

impl Edge {
    /// Return the opposite edge (used for flipping).
    #[must_use]
    pub fn opposite(self) -> Self {
        match self {
            Self::Top => Self::Bottom,
            Self::Bottom => Self::Top,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }

    /// Whether this edge is horizontal (Top/Bottom).
    #[must_use]
    pub fn is_horizontal(self) -> bool {
        matches!(self, Self::Top | Self::Bottom)
    }
}

/// Alignment along the edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Alignment {
    /// Align to the start of the edge (left for Top/Bottom, top for Left/Right).
    Start,
    /// Center along the edge.
    Center,
    /// Align to the end of the edge.
    End,
}

/// Configuration for how a popup is anchored to a reference element.
#[derive(Debug, Clone)]
pub struct AnchorConfig {
    /// The bounding rectangle of the anchor element in screen-space.
    pub anchor_rect: Rect,
    /// Which edge of the anchor to attach the popup to.
    pub anchor_edge: Edge,
    /// Alignment along the attachment edge.
    pub alignment: Alignment,
    /// Additional offset from the computed anchor position (dx, dy).
    pub offset: (f32, f32),
    /// If true, flip to the opposite edge when there isn't enough space.
    pub flip: bool,
    /// If true, slide along the edge to keep the popup on-screen.
    pub slide: bool,
}

impl AnchorConfig {
    /// Create a new anchor config with sensible defaults.
    #[must_use]
    pub fn new(anchor_rect: Rect, edge: Edge) -> Self {
        Self {
            anchor_rect,
            anchor_edge: edge,
            alignment: Alignment::Start,
            offset: (0.0, 0.0),
            flip: true,
            slide: true,
        }
    }

    /// Builder: set alignment.
    #[must_use]
    pub fn with_alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }

    /// Builder: set offset.
    #[must_use]
    pub fn with_offset(mut self, dx: f32, dy: f32) -> Self {
        self.offset = (dx, dy);
        self
    }

    /// Builder: set flip behavior.
    #[must_use]
    pub fn with_flip(mut self, flip: bool) -> Self {
        self.flip = flip;
        self
    }

    /// Builder: set slide behavior.
    #[must_use]
    pub fn with_slide(mut self, slide: bool) -> Self {
        self.slide = slide;
        self
    }

    /// Compute the raw (un-clamped) position for a popup of the given size.
    ///
    /// Returns `(x, y)` of the popup's top-left corner before any flip/slide
    /// adjustments.
    #[must_use]
    pub fn compute_raw_position(&self, popup_width: f32, popup_height: f32) -> (f32, f32) {
        let ar = &self.anchor_rect;

        // Compute the primary axis position (perpendicular to edge).
        let (base_x, base_y) = match self.anchor_edge {
            Edge::Top => {
                let y = ar.y - popup_height;
                let x = self.align_along_horizontal(popup_width);
                (x, y)
            }
            Edge::Bottom => {
                let y = ar.bottom();
                let x = self.align_along_horizontal(popup_width);
                (x, y)
            }
            Edge::Left => {
                let x = ar.x - popup_width;
                let y = self.align_along_vertical(popup_height);
                (x, y)
            }
            Edge::Right => {
                let x = ar.right();
                let y = self.align_along_vertical(popup_height);
                (x, y)
            }
        };

        (base_x + self.offset.0, base_y + self.offset.1)
    }

    /// Align the popup horizontally relative to the anchor (for Top/Bottom edges).
    fn align_along_horizontal(&self, popup_width: f32) -> f32 {
        let ar = &self.anchor_rect;
        match self.alignment {
            Alignment::Start => ar.x,
            Alignment::Center => ar.x + (ar.width - popup_width) / 2.0,
            Alignment::End => ar.right() - popup_width,
        }
    }

    /// Align the popup vertically relative to the anchor (for Left/Right edges).
    fn align_along_vertical(&self, popup_height: f32) -> f32 {
        let ar = &self.anchor_rect;
        match self.alignment {
            Alignment::Start => ar.y,
            Alignment::Center => ar.y + (ar.height - popup_height) / 2.0,
            Alignment::End => ar.bottom() - popup_height,
        }
    }
}
