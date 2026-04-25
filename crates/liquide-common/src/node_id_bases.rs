//! Centralized base offsets for scene-graph node IDs.
//!
//! Individual shell components (dock, taskbar, tray, context menus, tooltips,
//! notifications, etc.) assign `u64` IDs to their scene nodes. To prevent
//! collisions between components these bases are defined in one place so any
//! crate can reference them without inventing its own magic numbers.
//!
//! Each base is allocated a range of 1_000 IDs. Components may subdivide their
//! range however they like; e.g. the dock uses `DOCK_BASE + 0` for the panel
//! root and `DOCK_BASE + 100..` for individual item nodes.
//!
//! ### Guidelines
//! - Never hand out IDs outside a component's reserved range.
//! - Keep the bases sorted and contiguous to aid debugging.
//! - New components should pick the next unused slot (add below, don't reorder).
//!
//! ### Range table
//! | Base       | Component             |
//! |------------|-----------------------|
//! | `1_000`    | Taskbar / status bar  |
//! | `2_000`    | Dock                  |
//! | `3_000`    | System tray           |
//! | `4_000`    | Notification daemon   |
//! | `5_000`    | Context menus         |
//! | `6_000`    | Tooltips / popups     |
//! | `7_000`    | Overview / Exposé     |
//! | `8_000`    | Lock screen           |
//! | `9_000`    | Dialogs               |
//! | `10_000`+  | Application content   |

/// Root ID of the top-level status bar / taskbar panel.
pub const TASKBAR_BASE: u64 = 1_000;

/// Root ID of the application dock.
pub const DOCK_BASE: u64 = 2_000;
/// Base ID for per-dock-item nodes (dock items are `DOCK_ITEM_BASE + index`).
pub const DOCK_ITEM_BASE: u64 = 2_100;

/// Root ID of the system tray / status-notifier area.
pub const TRAY_BASE: u64 = 3_000;
/// Base ID for per-tray-item nodes.
pub const TRAY_ITEM_BASE: u64 = 3_100;

/// Root ID of the notification daemon overlay.
pub const NOTIFICATION_BASE: u64 = 4_000;

/// Root ID of a context menu panel.
pub const CONTEXT_MENU_BASE: u64 = 5_000;
/// Base ID for per-menu-item nodes.
pub const CONTEXT_MENU_ITEM_BASE: u64 = 5_100;

/// Root ID for tooltips / popups.
pub const TOOLTIP_BASE: u64 = 6_000;

/// Root ID for the overview / Exposé.
pub const OVERVIEW_BASE: u64 = 7_000;
/// Base ID for per-window overview slots.
pub const OVERVIEW_SLOT_BASE: u64 = 7_100;

/// Root ID for the lock screen overlay.
pub const LOCKSCREEN_BASE: u64 = 8_000;

/// Root ID for dialogs.
pub const DIALOG_BASE: u64 = 9_000;

/// Starting ID for application-owned scene nodes.
pub const APPLICATION_BASE: u64 = 10_000;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bases_are_sorted_and_non_overlapping() {
        let bases = [
            TASKBAR_BASE,
            DOCK_BASE,
            TRAY_BASE,
            NOTIFICATION_BASE,
            CONTEXT_MENU_BASE,
            TOOLTIP_BASE,
            OVERVIEW_BASE,
            LOCKSCREEN_BASE,
            DIALOG_BASE,
            APPLICATION_BASE,
        ];
        for w in bases.windows(2) {
            assert!(w[0] + 1_000 <= w[1], "ranges overlap: {} → {}", w[0], w[1]);
        }
    }

    #[test]
    fn item_bases_lie_within_component_range() {
        assert!(DOCK_ITEM_BASE >= DOCK_BASE && DOCK_ITEM_BASE < DOCK_BASE + 1_000);
        assert!(TRAY_ITEM_BASE >= TRAY_BASE && TRAY_ITEM_BASE < TRAY_BASE + 1_000);
        assert!(
            CONTEXT_MENU_ITEM_BASE >= CONTEXT_MENU_BASE
                && CONTEXT_MENU_ITEM_BASE < CONTEXT_MENU_BASE + 1_000
        );
        assert!(OVERVIEW_SLOT_BASE >= OVERVIEW_BASE && OVERVIEW_SLOT_BASE < OVERVIEW_BASE + 1_000);
    }
}
