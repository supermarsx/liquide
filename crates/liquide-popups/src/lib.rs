//! Comprehensive popup and modal window management for the LiquiDE desktop shell.
//!
//! This crate provides a unified system for managing all transient overlay
//! surfaces: tooltips, context menus, dropdowns, dialogs, notifications,
//! popovers, and splash screens.
//!
//! ## Architecture
//!
//! - [`PopupManager`] is the central orchestrator that owns all open popups
//!   and manages their lifecycle.
//! - [`PopupPositioner`] computes optimal placement with screen-edge avoidance,
//!   anchor flipping, and overlap prevention.
//! - [`PopupStack`] manages z-order so popups always render above regular windows,
//!   with modal dialogs above non-modal popups.
//! - [`TooltipController`] handles tooltip-specific delay/cancel semantics.
//! - [`DropdownController`] handles dropdown item selection and keyboard navigation.
//! - Event routing helpers determine when clicks, focus changes, and key presses
//!   should dismiss popups or be blocked by modal dialogs.

pub mod anchor;
pub mod dialog_info;
pub mod dropdown;
pub mod events;
pub mod manager;
pub mod popup;
pub mod position;
pub mod stack;
pub mod tooltip;

#[cfg(test)]
mod tests;

pub use anchor::{Alignment, AnchorConfig, Edge};
pub use dialog_info::DialogInfo;
pub use dropdown::{DropdownController, DropdownItem, DropdownKey};
pub use events::EventRouter;
pub use manager::PopupManager;
pub use popup::{Popup, PopupConfig, PopupId, PopupType, WindowId};
pub use position::PopupPositioner;
pub use stack::PopupStack;
pub use tooltip::TooltipController;

/// A rectangle in screen-space pixels.
///
/// Kept local to avoid coupling to a specific compositor crate while remaining
/// compatible with the f32-based geometry used throughout LiquiDE.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    /// Create a new rectangle.
    #[must_use]
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Right edge.
    #[must_use]
    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    /// Bottom edge.
    #[must_use]
    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    /// Whether the point (px, py) lies inside the rectangle.
    #[must_use]
    pub fn contains_point(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.right() && py >= self.y && py < self.bottom()
    }

    /// Whether two rectangles overlap.
    #[must_use]
    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.right()
            && self.right() > other.x
            && self.y < other.bottom()
            && self.bottom() > other.y
    }

    /// Area in square pixels.
    #[must_use]
    pub fn area(&self) -> f32 {
        self.width * self.height
    }

    /// A zero-size rectangle at the origin.
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    };
}
