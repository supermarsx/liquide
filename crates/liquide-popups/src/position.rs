//! Popup positioning with screen-edge avoidance, anchor flipping, and overlap
//! prevention.

use crate::Rect;
use crate::anchor::AnchorConfig;
use crate::popup::{Popup, PopupConfig};

/// Minimum number of pixels a popup must remain visible on-screen.
const MIN_VISIBLE_PX: f32 = 32.0;

/// Default gap between the cursor and a tooltip.
const TOOLTIP_CURSOR_GAP: f32 = 12.0;

/// Default gap between the click point and a context menu.
const CONTEXT_MENU_GAP: f32 = 2.0;

/// Computes optimal popup positions with full edge-avoidance logic.
pub struct PopupPositioner;

impl PopupPositioner {
    /// Compute the final screen-space [`Rect`] for a popup described by
    /// `config`, given a `screen` bounding rectangle and the list of
    /// currently open popups (for overlap avoidance).
    #[must_use]
    pub fn position(config: &PopupConfig, screen: Rect, existing: &[Popup]) -> Rect {
        let pw = config.width;
        let ph = config.height;

        let (mut x, mut y) = if let Some(ref anchor) = config.anchor {
            Self::position_anchored(anchor, pw, ph, screen)
        } else {
            (config.preferred_x, config.preferred_y)
        };

        // Clamp to screen.
        (x, y) = Self::clamp_to_screen(x, y, pw, ph, screen);

        // Avoid overlapping other popups if possible.
        let candidate = Rect::new(x, y, pw, ph);
        let nudged = Self::avoid_overlaps(candidate, existing, screen);

        nudged
    }

    /// Quick tooltip positioning: place below the cursor, avoid screen edges.
    #[must_use]
    pub fn position_tooltip(anchor: (f32, f32), size: (f32, f32), screen: Rect) -> (f32, f32) {
        let (ax, ay) = anchor;
        let (sw, sh) = (size.0, size.1);

        // Default: below and slightly to the right of the cursor.
        let mut x = ax;
        let mut y = ay + TOOLTIP_CURSOR_GAP;

        // If it would go off the bottom, place above.
        if y + sh > screen.bottom() {
            y = ay - sh - TOOLTIP_CURSOR_GAP;
        }

        // If it would go off the right, shift left.
        if x + sw > screen.right() {
            x = screen.right() - sw;
        }

        // Clamp minimum.
        if x < screen.x {
            x = screen.x;
        }
        if y < screen.y {
            y = screen.y;
        }

        (x, y)
    }

    /// Context menu positioning: at click point, avoid screen edges.
    #[must_use]
    pub fn position_context_menu(click: (f32, f32), size: (f32, f32), screen: Rect) -> (f32, f32) {
        let (cx, cy) = click;
        let (mw, mh) = (size.0, size.1);

        let mut x = cx + CONTEXT_MENU_GAP;
        let mut y = cy + CONTEXT_MENU_GAP;

        // If the menu goes off the right edge, open to the left of the cursor.
        if x + mw > screen.right() {
            x = cx - mw - CONTEXT_MENU_GAP;
        }
        // If the menu goes off the bottom, open upward.
        if y + mh > screen.bottom() {
            y = cy - mh - CONTEXT_MENU_GAP;
        }

        // Final clamp.
        if x < screen.x {
            x = screen.x;
        }
        if y < screen.y {
            y = screen.y;
        }

        (x, y)
    }

    // ----- internal helpers -----

    /// Position an anchored popup, applying flip and slide as needed.
    fn position_anchored(
        anchor: &AnchorConfig,
        popup_w: f32,
        popup_h: f32,
        screen: Rect,
    ) -> (f32, f32) {
        let (mut x, mut y) = anchor.compute_raw_position(popup_w, popup_h);

        // Check if flip is needed.
        if anchor.flip {
            let fits = Self::fits_on_screen(x, y, popup_w, popup_h, screen);
            if !fits {
                let flipped_edge = anchor.anchor_edge.opposite();
                let flipped_anchor = AnchorConfig {
                    anchor_rect: anchor.anchor_rect,
                    anchor_edge: flipped_edge,
                    alignment: anchor.alignment,
                    offset: (anchor.offset.0, anchor.offset.1),
                    flip: false,
                    slide: anchor.slide,
                };
                let (fx, fy) = flipped_anchor.compute_raw_position(popup_w, popup_h);
                if Self::fits_on_screen(fx, fy, popup_w, popup_h, screen) {
                    x = fx;
                    y = fy;
                }
                // If neither side fits, keep the original and let slide/clamp handle it.
            }
        }

        // Slide along the edge to stay on screen.
        if anchor.slide {
            if anchor.anchor_edge.is_horizontal() {
                // Slide horizontally.
                if x + popup_w > screen.right() {
                    x = screen.right() - popup_w;
                }
                if x < screen.x {
                    x = screen.x;
                }
            } else {
                // Slide vertically.
                if y + popup_h > screen.bottom() {
                    y = screen.bottom() - popup_h;
                }
                if y < screen.y {
                    y = screen.y;
                }
            }
        }

        (x, y)
    }

    /// Whether a rect at (x,y) of size (w,h) fits entirely within the screen.
    fn fits_on_screen(x: f32, y: f32, w: f32, h: f32, screen: Rect) -> bool {
        x >= screen.x && y >= screen.y && x + w <= screen.right() && y + h <= screen.bottom()
    }

    /// Clamp a popup position so at least `MIN_VISIBLE_PX` remains on-screen
    /// in each dimension.
    fn clamp_to_screen(mut x: f32, mut y: f32, w: f32, h: f32, screen: Rect) -> (f32, f32) {
        // Don't let the popup go too far off-screen.
        let max_x = screen.right() - MIN_VISIBLE_PX.min(w);
        let max_y = screen.bottom() - MIN_VISIBLE_PX.min(h);

        if x > max_x {
            x = max_x;
        }
        if y > max_y {
            y = max_y;
        }
        if x < screen.x - w + MIN_VISIBLE_PX.min(w) {
            x = screen.x;
        }
        if y < screen.y - h + MIN_VISIBLE_PX.min(h) {
            y = screen.y;
        }

        (x, y)
    }

    /// Try to nudge a popup rectangle so it doesn't overlap any existing
    /// popups. If no non-overlapping position can be found with small
    /// adjustments, returns the original position.
    fn avoid_overlaps(candidate: Rect, existing: &[Popup], screen: Rect) -> Rect {
        if existing.is_empty() {
            return candidate;
        }

        // Check if there's any overlap.
        let has_overlap = existing.iter().any(|p| candidate.intersects(&p.bounds));
        if !has_overlap {
            return candidate;
        }

        // Try nudging in 4 directions by increasing amounts.
        let directions: [(f32, f32); 4] = [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)];
        let nudge_amounts = [8.0, 16.0, 32.0, 64.0];

        for &amount in &nudge_amounts {
            for &(dx, dy) in &directions {
                let nx = candidate.x + dx * amount;
                let ny = candidate.y + dy * amount;
                let nudged = Rect::new(nx, ny, candidate.width, candidate.height);

                // Must be on screen.
                if nx < screen.x
                    || ny < screen.y
                    || nx + candidate.width > screen.right()
                    || ny + candidate.height > screen.bottom()
                {
                    continue;
                }

                let still_overlaps = existing.iter().any(|p| nudged.intersects(&p.bounds));
                if !still_overlaps {
                    return nudged;
                }
            }
        }

        // Could not avoid overlap — return original clamped position.
        candidate
    }
}
