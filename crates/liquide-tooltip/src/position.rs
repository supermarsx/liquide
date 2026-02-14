//! Tooltip positioning logic.
//!
//! Ensures tooltips don't get clipped by screen edges. Prefers positioning
//! below the anchor, but flips above if insufficient space.

/// Preferred tooltip placement relative to the anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TooltipPosition {
    /// Below the anchor (default).
    Below,
    /// Above the anchor.
    Above,
    /// To the right of the anchor.
    Right,
    /// To the left of the anchor.
    Left,
}

/// Computed tooltip rectangle.
#[derive(Debug, Clone, Copy)]
pub struct TooltipRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Compute the final tooltip position, flipping if needed to stay on screen.
pub fn compute_tooltip_position(
    // Anchor widget rect
    anchor_x: f32,
    anchor_y: f32,
    anchor_w: f32,
    anchor_h: f32,
    // Tooltip size
    tooltip_w: f32,
    tooltip_h: f32,
    // Offset from anchor
    offset_x: f32,
    offset_y: f32,
    // Screen bounds
    screen_w: f32,
    screen_h: f32,
    // Preferred position
    preferred: TooltipPosition,
) -> TooltipRect {
    // Center the tooltip horizontally over the anchor
    let mut x = anchor_x + (anchor_w - tooltip_w) / 2.0 + offset_x;
    let mut y;

    match preferred {
        TooltipPosition::Below => {
            y = anchor_y + anchor_h + offset_y;
            // Flip above if clipped at bottom
            if y + tooltip_h > screen_h {
                y = anchor_y - tooltip_h - offset_y;
            }
        }
        TooltipPosition::Above => {
            y = anchor_y - tooltip_h - offset_y;
            // Flip below if clipped at top
            if y < 0.0 {
                y = anchor_y + anchor_h + offset_y;
            }
        }
        TooltipPosition::Right => {
            x = anchor_x + anchor_w + offset_x;
            y = anchor_y + (anchor_h - tooltip_h) / 2.0;
            if x + tooltip_w > screen_w {
                x = anchor_x - tooltip_w - offset_x;
            }
        }
        TooltipPosition::Left => {
            x = anchor_x - tooltip_w - offset_x;
            y = anchor_y + (anchor_h - tooltip_h) / 2.0;
            if x < 0.0 {
                x = anchor_x + anchor_w + offset_x;
            }
        }
    }

    // Clamp to screen edges (always keep visible)
    x = x.clamp(4.0, (screen_w - tooltip_w - 4.0).max(4.0));
    y = y.clamp(4.0, (screen_h - tooltip_h - 4.0).max(4.0));

    TooltipRect { x, y, width: tooltip_w, height: tooltip_h }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tooltip_below() {
        let r = compute_tooltip_position(
            100.0, 100.0, 80.0, 30.0,  // anchor
            120.0, 24.0,                // tooltip size
            0.0, 8.0,                   // offset
            1920.0, 1080.0,             // screen
            TooltipPosition::Below,
        );
        assert!(r.y > 130.0); // Below anchor
        assert!(r.x >= 4.0);
    }

    #[test]
    fn test_tooltip_flips_when_clipped() {
        let r = compute_tooltip_position(
            100.0, 1060.0, 80.0, 20.0,  // anchor near bottom
            120.0, 24.0,
            0.0, 8.0,
            1920.0, 1080.0,
            TooltipPosition::Below,
        );
        // Should flip to above since bottom is clipped
        assert!(r.y < 1060.0);
    }
}
