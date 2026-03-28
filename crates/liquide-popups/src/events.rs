//! Event routing logic for popups.
//!
//! Determines when clicks, focus changes, and key presses should dismiss
//! popups or be blocked by modal dialogs.

use crate::popup::{Popup, PopupId, WindowId};

/// Event routing helper that inspects the current popup state.
pub struct EventRouter;

impl EventRouter {
    /// Returns `true` if a modal popup is blocking events to `target_window`.
    ///
    /// A modal popup blocks a window if:
    /// - The modal's owner is `target_window`, OR
    /// - The modal has no owner (blocks everything).
    #[must_use]
    pub fn should_block_event(popups: &[Popup], target_window: WindowId) -> bool {
        popups.iter().any(|p| {
            p.modal && match p.owner {
                Some(owner) => owner == target_window,
                None => true,
            }
        })
    }

    /// Returns the IDs of popups that should be dismissed because the user
    /// clicked at `(x, y)` which is outside their bounds.
    ///
    /// Modal popups are never dismissed by click-outside (they need explicit
    /// close or cancel). Only popups with `dismiss_on_click_outside == true`
    /// are returned.
    #[must_use]
    pub fn handle_click_outside(popups: &[Popup], x: f32, y: f32) -> Vec<PopupId> {
        let mut to_dismiss = Vec::new();
        for popup in popups {
            if popup.dismiss_on_click_outside && !popup.contains_point(x, y) {
                to_dismiss.push(popup.id);
            }
        }
        to_dismiss
    }

    /// Returns the topmost popup that should be dismissed on Escape, if any.
    ///
    /// Iterates from highest z-order to lowest and returns the first popup
    /// with `dismiss_on_escape == true`.
    #[must_use]
    pub fn handle_escape(popups: &[Popup]) -> Option<PopupId> {
        // Popups are typically stored in insertion order; find the one with
        // the highest z-order that is escape-dismissable.
        popups
            .iter()
            .filter(|p| p.dismiss_on_escape)
            .max_by_key(|p| p.z_order)
            .map(|p| p.id)
    }

    /// Returns popups that should close when the focus moves to `new_focus`.
    ///
    /// A popup closes on focus change if:
    /// - It has an owner and the owner is not `new_focus`, AND
    /// - It is not a modal dialog (modal stays until explicitly closed).
    /// - Notifications and splash screens are exempt (they aren't focus-linked).
    #[must_use]
    pub fn handle_focus_change(popups: &[Popup], new_focus: WindowId) -> Vec<PopupId> {
        use crate::popup::PopupType;

        let mut to_dismiss = Vec::new();
        for popup in popups {
            // Skip types that don't care about focus.
            if matches!(
                popup.popup_type,
                PopupType::Notification | PopupType::Splash
            ) {
                continue;
            }
            // Modal dialogs are not dismissed by focus change.
            if popup.modal {
                continue;
            }
            // If owned and the focus is not on the owner, dismiss.
            if let Some(owner) = popup.owner {
                if owner != new_focus {
                    to_dismiss.push(popup.id);
                }
            }
        }
        to_dismiss
    }

    /// Hit-test: find the topmost popup at the given point.
    #[must_use]
    pub fn popup_at_point(popups: &[Popup], x: f32, y: f32) -> Option<PopupId> {
        popups
            .iter()
            .filter(|p| p.contains_point(x, y))
            .max_by_key(|p| p.z_order)
            .map(|p| p.id)
    }
}
