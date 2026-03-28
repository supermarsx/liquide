//! Notification stacking layout.
//!
//! Computes screen positions for active notifications, stacking them from a
//! chosen anchor corner/edge with configurable gaps and priority ordering.

use serde::{Deserialize, Serialize};

/// Priority level for layout ordering. Higher priority notifications are
/// placed closer to the anchor (more visible).
///
/// This is distinct from [`crate::spec::Urgency`] which controls daemon
/// behavior (rate-limit bypass, auto-expire). `Priority` is purely visual.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Priority {
    /// Lowest visual priority — stacked furthest from anchor.
    Low = 0,
    /// Default visual priority.
    Normal = 1,
    /// Elevated visual priority — closer to anchor.
    High = 2,
    /// Highest visual priority — always at the anchor.
    Urgent = 3,
}

impl Default for Priority {
    fn default() -> Self {
        Priority::Normal
    }
}

/// Screen rectangle.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// Which corner/edge of the screen notifications stack from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayoutAnchor {
    TopRight,
    TopLeft,
    BottomRight,
    BottomLeft,
    TopCenter,
}

impl Default for LayoutAnchor {
    fn default() -> Self {
        LayoutAnchor::TopRight
    }
}

/// Input information about a notification for layout computation.
#[derive(Debug, Clone)]
pub struct NotificationInfo {
    /// Unique notification ID.
    pub id: u64,
    /// Desired width.
    pub width: f32,
    /// Desired height.
    pub height: f32,
    /// Visual priority (determines stacking order).
    pub priority: Priority,
}

/// Computed screen position for a notification.
#[derive(Debug, Clone)]
pub struct NotificationPosition {
    /// Notification ID.
    pub id: u64,
    /// X coordinate (left edge).
    pub x: f32,
    /// Y coordinate (top edge).
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

/// Default gap between stacked notifications in pixels.
const DEFAULT_GAP: f32 = 8.0;

/// Default margin from the screen edge in pixels.
const DEFAULT_MARGIN: f32 = 12.0;

/// Notification layout engine.
///
/// Computes positions for a set of notifications on screen, stacking them
/// from a chosen anchor with configurable gap and margin.
pub struct NotificationLayout {
    /// Gap between adjacent notifications.
    pub gap: f32,
    /// Margin from the screen edge.
    pub margin: f32,
}

impl NotificationLayout {
    /// Creates a layout engine with the given gap and margin.
    pub fn new(gap: f32, margin: f32) -> Self {
        Self { gap, margin }
    }

    /// Computes screen positions for the given notifications.
    ///
    /// Notifications are sorted by priority (highest first = closest to anchor),
    /// then stacked vertically from the anchor. Notifications that would extend
    /// beyond the screen bounds are omitted from the result.
    pub fn compute_positions(
        &self,
        notifications: &[NotificationInfo],
        screen: Rect,
        anchor: LayoutAnchor,
    ) -> Vec<NotificationPosition> {
        if notifications.is_empty() {
            return Vec::new();
        }

        // Sort by priority descending (Urgent first), stable to preserve
        // insertion order among equal priorities.
        let mut sorted: Vec<&NotificationInfo> = notifications.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));

        let mut positions = Vec::new();
        let mut cursor_y: f32;
        let grows_down: bool;

        match anchor {
            LayoutAnchor::TopRight | LayoutAnchor::TopLeft | LayoutAnchor::TopCenter => {
                cursor_y = screen.y + self.margin;
                grows_down = true;
            }
            LayoutAnchor::BottomRight | LayoutAnchor::BottomLeft => {
                cursor_y = screen.y + screen.height - self.margin;
                grows_down = false;
            }
        }

        for info in &sorted {
            let x = match anchor {
                LayoutAnchor::TopRight | LayoutAnchor::BottomRight => {
                    screen.x + screen.width - self.margin - info.width
                }
                LayoutAnchor::TopLeft | LayoutAnchor::BottomLeft => screen.x + self.margin,
                LayoutAnchor::TopCenter => {
                    screen.x + (screen.width - info.width) / 2.0
                }
            };

            let y = if grows_down {
                cursor_y
            } else {
                cursor_y - info.height
            };

            // Overflow check: skip notifications that don't fit on screen.
            if grows_down {
                let bottom = y + info.height;
                if bottom > screen.y + screen.height - self.margin {
                    continue; // Would overflow below screen.
                }
            } else {
                if y < screen.y + self.margin {
                    continue; // Would overflow above screen.
                }
            }

            positions.push(NotificationPosition {
                id: info.id,
                x,
                y,
                width: info.width,
                height: info.height,
            });

            if grows_down {
                cursor_y = y + info.height + self.gap;
            } else {
                cursor_y = y - self.gap;
            }
        }

        positions
    }
}

impl Default for NotificationLayout {
    fn default() -> Self {
        Self::new(DEFAULT_GAP, DEFAULT_MARGIN)
    }
}

/// Convenience function: compute positions with default gap and margin.
pub fn compute_positions(
    notifications: &[NotificationInfo],
    screen: Rect,
    anchor: LayoutAnchor,
) -> Vec<NotificationPosition> {
    NotificationLayout::default().compute_positions(notifications, screen, anchor)
}
