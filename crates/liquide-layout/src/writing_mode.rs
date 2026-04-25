//! Writing-mode support — maps logical properties to physical axes.
//!
//! CSS writing-mode changes which axis is inline (text flow) and which is block
//! (line stacking):
//!
//! | Writing-mode    | Inline axis | Block axis |
//! |-----------------|-------------|------------|
//! | horizontal-tb   | horizontal  | vertical   |
//! | vertical-rl     | vertical    | horizontal (R→L) |
//! | vertical-lr     | vertical    | horizontal (L→R) |
//! | sideways-rl     | vertical    | horizontal (R→L) |
//! | sideways-lr     | vertical    | horizontal (L→R) |

use crate::geometry::{Rect, Size};
use liquide_style_engine::computed::{Direction, WritingMode};

/// A size expressed in logical (inline/block) coordinates.
///
/// - **inline**: the dimension along the text flow direction.
/// - **block**: the dimension perpendicular to text flow (line stacking).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LogicalSize {
    pub inline: f32,
    pub block: f32,
}

impl LogicalSize {
    pub fn new(inline: f32, block: f32) -> Self {
        Self { inline, block }
    }

    pub fn zero() -> Self {
        Self {
            inline: 0.0,
            block: 0.0,
        }
    }
}

/// A rectangle expressed in logical (inline/block) coordinates.
///
/// - **inline_start / block_start**: origin in the logical coordinate system.
/// - **inline_size / block_size**: extent along each logical axis.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LogicalRect {
    pub inline_start: f32,
    pub block_start: f32,
    pub inline_size: f32,
    pub block_size: f32,
}

impl LogicalRect {
    pub fn new(inline_start: f32, block_start: f32, inline_size: f32, block_size: f32) -> Self {
        Self {
            inline_start,
            block_start,
            inline_size,
            block_size,
        }
    }

    pub fn zero() -> Self {
        Self {
            inline_start: 0.0,
            block_start: 0.0,
            inline_size: 0.0,
            block_size: 0.0,
        }
    }

    /// Logical size of this rect.
    pub fn size(&self) -> LogicalSize {
        LogicalSize::new(self.inline_size, self.block_size)
    }

    /// End of the inline extent.
    pub fn inline_end(&self) -> f32 {
        self.inline_start + self.inline_size
    }

    /// End of the block extent.
    pub fn block_end(&self) -> f32 {
        self.block_start + self.block_size
    }
}

// ── Free-standing conversion functions ──────────────────────────────

/// Convert a logical size to a physical `Size`.
///
/// - `horizontal-tb`: inline → width, block → height.
/// - `vertical-rl` / `vertical-lr` / `sideways-*`: inline → height, block → width.
#[must_use]
pub fn to_physical_size(logical: LogicalSize, mode: WritingMode) -> Size {
    if matches!(mode, WritingMode::HorizontalTb) {
        Size::new(logical.inline, logical.block)
    } else {
        Size::new(logical.block, logical.inline)
    }
}

/// Convert a physical `Size` to a `LogicalSize`.
#[must_use]
pub fn from_physical_size(physical: Size, mode: WritingMode) -> LogicalSize {
    if matches!(mode, WritingMode::HorizontalTb) {
        LogicalSize::new(physical.width, physical.height)
    } else {
        LogicalSize::new(physical.height, physical.width)
    }
}

/// Convert a logical rect to a physical `Rect`.
///
/// `container` is the physical size of the containing block — needed for
/// `vertical-rl` where block coordinates are measured from the right edge.
///
/// Mapping rules:
/// - `horizontal-tb`: inline_start → x, block_start → y, sizes map normally.
/// - `vertical-lr`: block_start → x, inline_start → y, sizes swapped.
/// - `vertical-rl` / `sideways-rl`: x = container.width − block_start − block_size
///   (blocks grow right-to-left), inline_start → y, sizes swapped.
/// - `sideways-lr`: same as `vertical-lr`.
#[must_use]
pub fn to_physical_rect(logical: LogicalRect, mode: WritingMode, container: Size) -> Rect {
    match mode {
        WritingMode::HorizontalTb => Rect::new(
            logical.inline_start,
            logical.block_start,
            logical.inline_size,
            logical.block_size,
        ),
        WritingMode::VerticalLr | WritingMode::SidewaysLr => Rect::new(
            logical.block_start,
            logical.inline_start,
            logical.block_size,
            logical.inline_size,
        ),
        WritingMode::VerticalRl | WritingMode::SidewaysRl => Rect::new(
            container.width - logical.block_start - logical.block_size,
            logical.inline_start,
            logical.block_size,
            logical.inline_size,
        ),
    }
}

/// Convert a physical `Rect` to a `LogicalRect`.
///
/// `container` is the physical size of the containing block.
#[must_use]
pub fn from_physical_rect(physical: Rect, mode: WritingMode, container: Size) -> LogicalRect {
    match mode {
        WritingMode::HorizontalTb => {
            LogicalRect::new(physical.x, physical.y, physical.width, physical.height)
        }
        WritingMode::VerticalLr | WritingMode::SidewaysLr => {
            LogicalRect::new(physical.y, physical.x, physical.height, physical.width)
        }
        WritingMode::VerticalRl | WritingMode::SidewaysRl => LogicalRect::new(
            physical.y,
            container.width - physical.x - physical.width,
            physical.height,
            physical.width,
        ),
    }
}

/// Context for mapping logical → physical coordinates under a given writing mode.
#[derive(Debug, Clone, Copy)]
pub struct WritingModeContext {
    pub mode: WritingMode,
    pub direction: Direction,
}

impl WritingModeContext {
    pub fn new(mode: WritingMode) -> Self {
        Self {
            mode,
            direction: Direction::Ltr,
        }
    }

    pub fn with_direction(mode: WritingMode, direction: Direction) -> Self {
        Self { mode, direction }
    }

    /// Is the inline direction right-to-left?
    #[must_use]
    pub fn is_rtl(&self) -> bool {
        matches!(self.direction, Direction::Rtl)
    }

    /// Get the container inline size from physical (width, height).
    #[must_use]
    pub fn inline_size(&self, width: f32, height: f32) -> f32 {
        if self.is_vertical() { height } else { width }
    }

    /// Get the container block size from physical (width, height).
    #[must_use]
    pub fn block_size(&self, width: f32, height: f32) -> f32 {
        if self.is_vertical() { width } else { height }
    }

    /// Resolve (margin-block-start, margin-block-end) to physical
    /// (margin in block-start direction, margin in block-end direction).
    /// For horizontal-tb these map to (top, bottom).
    /// For vertical modes these map to (left, right) or (right, left).
    #[must_use]
    pub fn block_start_end_physical(&self) -> BlockAxis {
        if self.is_vertical() {
            if self.block_flow_is_rtl() {
                BlockAxis::Horizontal {
                    start_is_right: true,
                }
            } else {
                BlockAxis::Horizontal {
                    start_is_right: false,
                }
            }
        } else {
            BlockAxis::Vertical
        }
    }

    /// Is the inline axis vertical? (text flows top-to-bottom)
    #[must_use]
    pub fn is_vertical(&self) -> bool {
        !matches!(self.mode, WritingMode::HorizontalTb)
    }

    /// Is the inline axis horizontal? (text flows left-to-right or right-to-left)
    #[must_use]
    pub fn is_horizontal(&self) -> bool {
        matches!(self.mode, WritingMode::HorizontalTb)
    }

    /// Block flow direction: does block progression go right-to-left?
    #[must_use]
    pub fn block_flow_is_rtl(&self) -> bool {
        matches!(self.mode, WritingMode::VerticalRl | WritingMode::SidewaysRl)
    }

    /// Map (inline-size, block-size) to (width, height).
    #[must_use]
    pub fn to_physical(&self, inline_size: f32, block_size: f32) -> (f32, f32) {
        if self.is_vertical() {
            (block_size, inline_size)
        } else {
            (inline_size, block_size)
        }
    }

    /// Map (width, height) to (inline-size, block-size).
    #[must_use]
    pub fn to_logical(&self, width: f32, height: f32) -> (f32, f32) {
        if self.is_vertical() {
            (height, width)
        } else {
            (width, height)
        }
    }

    /// Position a block child at offset `block_pos` within container of
    /// physical `container_inline × container_block`.
    ///
    /// Returns (x, y) physical coordinates.
    #[must_use]
    pub fn position_block_child(
        &self,
        block_pos: f32,
        inline_pos: f32,
        _container_width: f32,
    ) -> (f32, f32) {
        if self.is_vertical() {
            if self.block_flow_is_rtl() {
                // vertical-rl: blocks stack right-to-left; inline flows top-to-bottom
                (_container_width - block_pos, inline_pos)
            } else {
                // vertical-lr: blocks stack left-to-right; inline flows top-to-bottom
                (block_pos, inline_pos)
            }
        } else {
            // horizontal-tb: blocks stack top-to-bottom; inline flows left-to-right
            (inline_pos, block_pos)
        }
    }

    /// For a vertical writing mode, should the text be rotated sideways?
    #[must_use]
    pub fn sideways(&self) -> bool {
        matches!(self.mode, WritingMode::SidewaysRl | WritingMode::SidewaysLr)
    }

    /// Rotation angle in degrees for text in this writing mode.
    /// - horizontal-tb: 0°
    /// - vertical-rl / vertical-lr: 90° CW (upright)
    /// - sideways-rl: 90° CW (all glyphs rotated)
    /// - sideways-lr: 270° (counter-clockwise)
    #[must_use]
    pub fn text_rotation_degrees(&self) -> f32 {
        match self.mode {
            WritingMode::HorizontalTb => 0.0,
            WritingMode::VerticalRl | WritingMode::VerticalLr => 90.0,
            WritingMode::SidewaysRl => 90.0,
            WritingMode::SidewaysLr => 270.0,
        }
    }
}

/// Describes which physical axis the block direction maps to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BlockAxis {
    /// Block direction is vertical (horizontal-tb): start=top, end=bottom.
    Vertical,
    /// Block direction is horizontal (vertical writing modes).
    /// `start_is_right`: true for vertical-rl (blocks go right-to-left).
    Horizontal { start_is_right: bool },
}

impl Default for WritingModeContext {
    fn default() -> Self {
        Self {
            mode: WritingMode::HorizontalTb,
            direction: Direction::Ltr,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horizontal_tb_is_horizontal() {
        let ctx = WritingModeContext::new(WritingMode::HorizontalTb);
        assert!(ctx.is_horizontal());
        assert!(!ctx.is_vertical());
    }

    #[test]
    fn vertical_rl_is_vertical() {
        let ctx = WritingModeContext::new(WritingMode::VerticalRl);
        assert!(ctx.is_vertical());
        assert!(!ctx.is_horizontal());
        assert!(ctx.block_flow_is_rtl());
    }

    #[test]
    fn vertical_lr_is_vertical() {
        let ctx = WritingModeContext::new(WritingMode::VerticalLr);
        assert!(ctx.is_vertical());
        assert!(!ctx.block_flow_is_rtl());
    }

    #[test]
    fn to_physical_horizontal() {
        let ctx = WritingModeContext::new(WritingMode::HorizontalTb);
        assert_eq!(ctx.to_physical(100.0, 50.0), (100.0, 50.0));
    }

    #[test]
    fn to_physical_vertical() {
        let ctx = WritingModeContext::new(WritingMode::VerticalRl);
        // inline=100 → height, block=50 → width
        assert_eq!(ctx.to_physical(100.0, 50.0), (50.0, 100.0));
    }

    #[test]
    fn to_logical_roundtrip() {
        let ctx = WritingModeContext::new(WritingMode::VerticalLr);
        let (inline_s, block_s) = ctx.to_logical(200.0, 300.0);
        assert_eq!(ctx.to_physical(inline_s, block_s), (200.0, 300.0));
    }

    // ── LogicalSize tests ──

    #[test]
    fn logical_size_to_physical_horizontal() {
        let ls = LogicalSize::new(100.0, 50.0);
        let ps = to_physical_size(ls, WritingMode::HorizontalTb);
        assert_eq!(ps, Size::new(100.0, 50.0));
    }

    #[test]
    fn logical_size_to_physical_vertical_rl() {
        let ls = LogicalSize::new(100.0, 50.0);
        let ps = to_physical_size(ls, WritingMode::VerticalRl);
        // inline=100 → height, block=50 → width
        assert_eq!(ps, Size::new(50.0, 100.0));
    }

    #[test]
    fn logical_size_roundtrip_horizontal() {
        let original = Size::new(320.0, 240.0);
        let logical = from_physical_size(original, WritingMode::HorizontalTb);
        let back = to_physical_size(logical, WritingMode::HorizontalTb);
        assert_eq!(back, original);
    }

    #[test]
    fn logical_size_roundtrip_vertical_lr() {
        let original = Size::new(320.0, 240.0);
        let logical = from_physical_size(original, WritingMode::VerticalLr);
        assert_eq!(logical, LogicalSize::new(240.0, 320.0));
        let back = to_physical_size(logical, WritingMode::VerticalLr);
        assert_eq!(back, original);
    }

    // ── LogicalRect tests ──

    #[test]
    fn logical_rect_to_physical_horizontal() {
        let lr = LogicalRect::new(10.0, 20.0, 100.0, 50.0);
        let container = Size::new(800.0, 600.0);
        let pr = to_physical_rect(lr, WritingMode::HorizontalTb, container);
        assert_eq!(pr, Rect::new(10.0, 20.0, 100.0, 50.0));
    }

    #[test]
    fn logical_rect_to_physical_vertical_lr() {
        let lr = LogicalRect::new(10.0, 20.0, 100.0, 50.0);
        let container = Size::new(800.0, 600.0);
        let pr = to_physical_rect(lr, WritingMode::VerticalLr, container);
        // block_start=20 → x, inline_start=10 → y, block_size=50 → width, inline_size=100 → height
        assert_eq!(pr, Rect::new(20.0, 10.0, 50.0, 100.0));
    }

    #[test]
    fn logical_rect_to_physical_vertical_rl() {
        let lr = LogicalRect::new(10.0, 20.0, 100.0, 50.0);
        let container = Size::new(800.0, 600.0);
        let pr = to_physical_rect(lr, WritingMode::VerticalRl, container);
        // x = 800 - 20 - 50 = 730
        assert_eq!(pr, Rect::new(730.0, 10.0, 50.0, 100.0));
    }

    #[test]
    fn logical_rect_roundtrip_horizontal() {
        let original = Rect::new(10.0, 20.0, 100.0, 50.0);
        let container = Size::new(800.0, 600.0);
        let logical = from_physical_rect(original, WritingMode::HorizontalTb, container);
        let back = to_physical_rect(logical, WritingMode::HorizontalTb, container);
        assert_eq!(back, original);
    }

    #[test]
    fn logical_rect_roundtrip_vertical_lr() {
        let original = Rect::new(30.0, 40.0, 200.0, 150.0);
        let container = Size::new(800.0, 600.0);
        let logical = from_physical_rect(original, WritingMode::VerticalLr, container);
        let back = to_physical_rect(logical, WritingMode::VerticalLr, container);
        assert_eq!(back, original);
    }

    #[test]
    fn logical_rect_roundtrip_vertical_rl() {
        let original = Rect::new(30.0, 40.0, 200.0, 150.0);
        let container = Size::new(800.0, 600.0);
        let logical = from_physical_rect(original, WritingMode::VerticalRl, container);
        let back = to_physical_rect(logical, WritingMode::VerticalRl, container);
        assert_eq!(back, original);
    }

    #[test]
    fn logical_rect_roundtrip_sideways_rl() {
        let original = Rect::new(100.0, 200.0, 50.0, 80.0);
        let container = Size::new(1024.0, 768.0);
        let logical = from_physical_rect(original, WritingMode::SidewaysRl, container);
        let back = to_physical_rect(logical, WritingMode::SidewaysRl, container);
        assert_eq!(back, original);
    }

    #[test]
    fn logical_rect_methods() {
        let lr = LogicalRect::new(10.0, 20.0, 100.0, 50.0);
        assert_eq!(lr.inline_end(), 110.0);
        assert_eq!(lr.block_end(), 70.0);
        assert_eq!(lr.size(), LogicalSize::new(100.0, 50.0));
    }

    #[test]
    fn rtl_direction() {
        let ctx = WritingModeContext::with_direction(WritingMode::HorizontalTb, Direction::Rtl);
        assert!(ctx.is_rtl());
        assert!(ctx.is_horizontal());
    }

    #[test]
    fn inline_block_sizes() {
        let h = WritingModeContext::new(WritingMode::HorizontalTb);
        assert_eq!(h.inline_size(800.0, 600.0), 800.0);
        assert_eq!(h.block_size(800.0, 600.0), 600.0);

        let v = WritingModeContext::new(WritingMode::VerticalRl);
        assert_eq!(v.inline_size(800.0, 600.0), 600.0);
        assert_eq!(v.block_size(800.0, 600.0), 800.0);
    }

    #[test]
    fn block_axis_mapping() {
        let h = WritingModeContext::new(WritingMode::HorizontalTb);
        assert_eq!(h.block_start_end_physical(), BlockAxis::Vertical);

        let vrl = WritingModeContext::new(WritingMode::VerticalRl);
        assert_eq!(
            vrl.block_start_end_physical(),
            BlockAxis::Horizontal {
                start_is_right: true
            }
        );

        let vlr = WritingModeContext::new(WritingMode::VerticalLr);
        assert_eq!(
            vlr.block_start_end_physical(),
            BlockAxis::Horizontal {
                start_is_right: false
            }
        );
    }
}
