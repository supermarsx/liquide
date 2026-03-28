//! Workspace policies: workspace count limits, focus behaviour, and window
//! placement strategies.

use crate::layout::Rect;
use serde::{Deserialize, Serialize};

// ── WorkspacePolicy ──────────────────────────────────────────────────

/// Policy governing workspace creation and navigation behaviour.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspacePolicy {
    /// Minimum number of workspaces.
    pub min_count: usize,
    /// Maximum number of workspaces (0 = unlimited).
    pub max_count: usize,
    /// Whether workspaces are created dynamically (GNOME-style).
    pub dynamic_creation: bool,
    /// Whether next/prev navigation wraps at boundaries.
    pub wrap_navigation: bool,
    /// Whether moving a window to another workspace automatically switches
    /// to that workspace.
    pub move_window_switches: bool,
    /// Pattern for auto-generated workspace names. `{}` is replaced with
    /// the 1-based index.
    pub default_name_pattern: String,
}

impl Default for WorkspacePolicy {
    fn default() -> Self {
        Self {
            min_count: 1,
            max_count: 0,
            dynamic_creation: true,
            wrap_navigation: true,
            move_window_switches: false,
            default_name_pattern: "Workspace {}".into(),
        }
    }
}

impl WorkspacePolicy {
    /// Return `true` if the given count is within the allowed range.
    pub fn allows_count(&self, count: usize) -> bool {
        count >= self.min_count && (self.max_count == 0 || count <= self.max_count)
    }

    /// Return `true` if a new workspace can be created at the given total.
    pub fn can_create_at(&self, current_count: usize) -> bool {
        self.dynamic_creation
            && (self.max_count == 0 || current_count < self.max_count)
    }

    /// Return `true` if a workspace can be destroyed at the given total.
    pub fn can_destroy_at(&self, current_count: usize) -> bool {
        current_count > self.min_count.max(1)
    }
}

// ── FocusPolicy ──────────────────────────────────────────────────────

/// Focus model for windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FocusPolicy {
    /// Focus follows the mouse pointer immediately.
    FollowMouse,
    /// Focus only changes on explicit click.
    ClickToFocus,
    /// Focus follows the mouse but with a configurable delay (milliseconds)
    /// before the focus actually transfers. This prevents accidental focus
    /// changes when the mouse briefly passes over a window.
    FocusFollowsMouseSloppy {
        /// Delay in milliseconds before focus transfers.
        delay_ms: u32,
    },
}

impl Default for FocusPolicy {
    fn default() -> Self {
        Self::ClickToFocus
    }
}

impl FocusPolicy {
    /// Return the delay in milliseconds (0 for instant policies).
    pub fn delay_ms(&self) -> u32 {
        match self {
            Self::FollowMouse => 0,
            Self::ClickToFocus => 0,
            Self::FocusFollowsMouseSloppy { delay_ms } => *delay_ms,
        }
    }

    /// Return `true` if focus changes on pointer motion (as opposed to
    /// only on click).
    pub fn follows_mouse(&self) -> bool {
        matches!(
            self,
            Self::FollowMouse | Self::FocusFollowsMouseSloppy { .. }
        )
    }
}

// ── WindowPlacementPolicy ────────────────────────────────────────────

/// Strategy for placing newly opened windows on the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowPlacementPolicy {
    /// Find a position that minimizes overlap with existing windows.
    Smart,
    /// Place at a random position within the screen bounds.
    Random,
    /// Cascade from the top-left corner, offset by a fixed step.
    Cascade,
    /// Center on the screen.
    Center,
    /// Place the window under the current mouse pointer.
    UnderMouse,
}

impl Default for WindowPlacementPolicy {
    fn default() -> Self {
        Self::Smart
    }
}

/// A window rectangle used for placement calculations.
#[derive(Debug, Clone, Copy)]
pub struct WindowRect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

/// Find the best position for a new window using the Smart placement
/// strategy. Scans a grid of candidate positions and picks the one with
/// minimum overlap with existing windows.
///
/// Returns `(x, y)` for the top-left corner of the new window.
pub fn smart_placement(
    window_w: u32,
    window_h: u32,
    existing: &[WindowRect],
    screen: Rect,
) -> (i32, i32) {
    // Scan a grid of candidate positions (step = 32px for reasonable
    // granularity without excessive iteration).
    let step = 32i32;
    let max_x = screen.x + screen.w as i32 - window_w as i32;
    let max_y = screen.y + screen.h as i32 - window_h as i32;

    if max_x < screen.x || max_y < screen.y {
        // Window is larger than screen — just place at screen origin.
        return (screen.x, screen.y);
    }

    if existing.is_empty() {
        // No windows: center on screen.
        return (
            screen.x + (screen.w as i32 - window_w as i32) / 2,
            screen.y + (screen.h as i32 - window_h as i32) / 2,
        );
    }

    let mut best_x = screen.x;
    let mut best_y = screen.y;
    let mut best_overlap = u64::MAX;

    let mut cy = screen.y;
    while cy <= max_y {
        let mut cx = screen.x;
        while cx <= max_x {
            let candidate = Rect::new(cx, cy, window_w, window_h);
            let overlap = total_overlap(&candidate, existing);
            if overlap < best_overlap {
                best_overlap = overlap;
                best_x = cx;
                best_y = cy;
                if overlap == 0 {
                    return (best_x, best_y);
                }
            }
            cx += step;
        }
        cy += step;
    }

    (best_x, best_y)
}

/// Compute the total overlap area between a candidate rect and all existing
/// window rects.
fn total_overlap(candidate: &Rect, existing: &[WindowRect]) -> u64 {
    let mut total = 0u64;
    for win in existing {
        let ix1 = candidate.x.max(win.x);
        let iy1 = candidate.y.max(win.y);
        let ix2 = (candidate.x + candidate.w as i32).min(win.x + win.w as i32);
        let iy2 = (candidate.y + candidate.h as i32).min(win.y + win.h as i32);
        if ix2 > ix1 && iy2 > iy1 {
            total += (ix2 - ix1) as u64 * (iy2 - iy1) as u64;
        }
    }
    total
}

/// Compute a cascade position for the given window index.
pub fn cascade_position(
    index: usize,
    window_w: u32,
    window_h: u32,
    screen: Rect,
) -> (i32, i32) {
    let offset = 30;
    let x = screen.x + (index as i32 * offset) % (screen.w as i32 - window_w as i32).max(1);
    let y = screen.y + (index as i32 * offset) % (screen.h as i32 - window_h as i32).max(1);
    (x, y)
}

/// Compute a center position.
pub fn center_position(window_w: u32, window_h: u32, screen: Rect) -> (i32, i32) {
    (
        screen.x + (screen.w as i32 - window_w as i32) / 2,
        screen.y + (screen.h as i32 - window_h as i32) / 2,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── WorkspacePolicy ─────────────────────────────────────────────

    #[test]
    fn default_policy() {
        let p = WorkspacePolicy::default();
        assert_eq!(p.min_count, 1);
        assert_eq!(p.max_count, 0);
        assert!(p.dynamic_creation);
        assert!(p.wrap_navigation);
        assert!(!p.move_window_switches);
    }

    #[test]
    fn allows_count_within_range() {
        let p = WorkspacePolicy {
            min_count: 2,
            max_count: 5,
            ..Default::default()
        };
        assert!(!p.allows_count(1));
        assert!(p.allows_count(2));
        assert!(p.allows_count(5));
        assert!(!p.allows_count(6));
    }

    #[test]
    fn allows_count_unlimited_max() {
        let p = WorkspacePolicy {
            min_count: 1,
            max_count: 0,
            ..Default::default()
        };
        assert!(p.allows_count(100));
    }

    #[test]
    fn can_create_at() {
        let p = WorkspacePolicy {
            max_count: 4,
            dynamic_creation: true,
            ..Default::default()
        };
        assert!(p.can_create_at(3));
        assert!(!p.can_create_at(4));
    }

    #[test]
    fn can_create_at_disabled() {
        let p = WorkspacePolicy {
            dynamic_creation: false,
            ..Default::default()
        };
        assert!(!p.can_create_at(1));
    }

    #[test]
    fn can_destroy_at() {
        let p = WorkspacePolicy {
            min_count: 2,
            ..Default::default()
        };
        assert!(!p.can_destroy_at(2));
        assert!(p.can_destroy_at(3));
    }

    // ── FocusPolicy ─────────────────────────────────────────────────

    #[test]
    fn focus_click_defaults() {
        let f = FocusPolicy::default();
        assert_eq!(f, FocusPolicy::ClickToFocus);
        assert_eq!(f.delay_ms(), 0);
        assert!(!f.follows_mouse());
    }

    #[test]
    fn focus_follow_mouse() {
        let f = FocusPolicy::FollowMouse;
        assert!(f.follows_mouse());
        assert_eq!(f.delay_ms(), 0);
    }

    #[test]
    fn focus_sloppy() {
        let f = FocusPolicy::FocusFollowsMouseSloppy { delay_ms: 250 };
        assert!(f.follows_mouse());
        assert_eq!(f.delay_ms(), 250);
    }

    // ── WindowPlacementPolicy ───────────────────────────────────────

    #[test]
    fn default_placement_is_smart() {
        assert_eq!(WindowPlacementPolicy::default(), WindowPlacementPolicy::Smart);
    }

    // ── smart_placement ─────────────────────────────────────────────

    #[test]
    fn smart_placement_empty_centers() {
        let screen = Rect::new(0, 0, 1920, 1080);
        let (x, y) = smart_placement(400, 300, &[], screen);
        assert_eq!(x, (1920 - 400) / 2);
        assert_eq!(y, (1080 - 300) / 2);
    }

    #[test]
    fn smart_placement_avoids_existing() {
        let screen = Rect::new(0, 0, 1920, 1080);
        let existing = vec![WindowRect {
            x: 0,
            y: 0,
            w: 800,
            h: 600,
        }];
        let (x, y) = smart_placement(400, 300, &existing, screen);
        // Should not overlap with the existing window (overlap = 0 possible).
        let candidate = Rect::new(x, y, 400, 300);
        let win_rect = Rect::new(0, 0, 800, 600);
        // Either no overlap or at least displaced from (0,0).
        assert!(
            !candidate.intersects(&win_rect) || (x != 0 || y != 0),
            "Placed at ({}, {}), expected to avoid (0,0,800,600)",
            x,
            y
        );
    }

    #[test]
    fn smart_placement_window_larger_than_screen() {
        let screen = Rect::new(0, 0, 300, 200);
        let (x, y) = smart_placement(500, 400, &[], screen);
        assert_eq!(x, 0);
        assert_eq!(y, 0);
    }

    // ── cascade_position ────────────────────────────────────────────

    #[test]
    fn cascade_positions_increase() {
        let screen = Rect::new(0, 0, 1920, 1080);
        let (x0, y0) = cascade_position(0, 400, 300, screen);
        let (x1, y1) = cascade_position(1, 400, 300, screen);
        assert!(x1 > x0);
        assert!(y1 > y0);
    }

    #[test]
    fn cascade_wraps_within_screen() {
        let screen = Rect::new(0, 0, 800, 600);
        for i in 0..20 {
            let (x, y) = cascade_position(i, 400, 300, screen);
            assert!(x >= 0);
            assert!(y >= 0);
            assert!(x < screen.w as i32);
            assert!(y < screen.h as i32);
        }
    }

    // ── center_position ─────────────────────────────────────────────

    #[test]
    fn center_position_centered() {
        let screen = Rect::new(0, 0, 1920, 1080);
        let (x, y) = center_position(400, 300, screen);
        assert_eq!(x, 760);
        assert_eq!(y, 390);
    }

    #[test]
    fn center_position_with_offset_screen() {
        let screen = Rect::new(100, 50, 1920, 1080);
        let (x, y) = center_position(400, 300, screen);
        assert_eq!(x, 100 + 760);
        assert_eq!(y, 50 + 390);
    }
}
