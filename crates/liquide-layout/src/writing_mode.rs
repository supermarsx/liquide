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

use liquide_style_engine::computed::WritingMode;

/// Context for mapping logical → physical coordinates under a given writing mode.
#[derive(Debug, Clone, Copy)]
pub struct WritingModeContext {
    pub mode: WritingMode,
}

impl WritingModeContext {
    pub fn new(mode: WritingMode) -> Self {
        Self { mode }
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

impl Default for WritingModeContext {
    fn default() -> Self {
        Self { mode: WritingMode::HorizontalTb }
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
}
