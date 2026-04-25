//! Z-order management for the popup stack.
//!
//! Popups always render above regular windows. Within the popup layer:
//! - Tooltips are the lowest of the popup layers (they should never obscure
//!   interactive popups like context menus or dialogs).
//! - Non-modal popups sit above tooltips.
//! - Modal dialogs sit above everything.
//! - Within the same type, the most recently opened popup is on top.

use crate::popup::{Popup, PopupType};

/// Base z-order for tooltips (lowest popup tier — informational overlay).
const BASE_Z_TOOLTIP: i32 = 8_000;
/// Base z-order for non-modal popups (above tooltips and regular windows).
const BASE_Z_NONMODAL: i32 = 10_000;
/// Base z-order for modal dialogs (above everything else).
const BASE_Z_MODAL: i32 = 20_000;

/// Manages z-order assignment for the popup layer.
pub struct PopupStack {
    /// Monotonically increasing counter for z-order within a category.
    next_nonmodal: i32,
    next_modal: i32,
    next_tooltip: i32,
}

impl PopupStack {
    /// Create a new popup stack.
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_nonmodal: 0,
            next_modal: 0,
            next_tooltip: 0,
        }
    }

    /// Compute the z-order for a new popup of the given type.
    pub fn z_order_for_popup(&mut self, popup_type: PopupType, modal: bool) -> i32 {
        if modal {
            let z = BASE_Z_MODAL + self.next_modal;
            self.next_modal += 1;
            z
        } else if popup_type == PopupType::Tooltip {
            let z = BASE_Z_TOOLTIP + self.next_tooltip;
            self.next_tooltip += 1;
            z
        } else {
            let z = BASE_Z_NONMODAL + self.next_nonmodal;
            self.next_nonmodal += 1;
            z
        }
    }

    /// Sort a slice of popups by z-order (ascending, lowest first).
    pub fn sort_by_z_order(popups: &mut [&Popup]) {
        popups.sort_by_key(|p| p.z_order);
    }

    /// Reset the counters (e.g. when all popups are closed).
    pub fn reset(&mut self) {
        self.next_nonmodal = 0;
        self.next_modal = 0;
        self.next_tooltip = 0;
    }

    /// Get the z-order base for non-modal popups.
    #[must_use]
    pub fn base_nonmodal() -> i32 {
        BASE_Z_NONMODAL
    }

    /// Get the z-order base for modal dialogs.
    #[must_use]
    pub fn base_modal() -> i32 {
        BASE_Z_MODAL
    }

    /// Get the z-order base for tooltips.
    #[must_use]
    pub fn base_tooltip() -> i32 {
        BASE_Z_TOOLTIP
    }
}

impl Default for PopupStack {
    fn default() -> Self {
        Self::new()
    }
}
