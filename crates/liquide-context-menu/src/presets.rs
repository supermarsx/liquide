//! Predefined context menu definitions for common shell surfaces.
//!
//! Each function returns a `Vec<MenuItem>` that can be passed to a
//! [`ContextMenu`] for rendering and interaction.

use crate::{MenuAction, MenuItem};

// ---------------------------------------------------------------------------
// Action tag constants — the shell maps these to ShellAction variants.
// ---------------------------------------------------------------------------

// Desktop context menu actions (100–199)
pub const ACTION_CONFIGURE_DESKTOP: MenuAction = MenuAction(100);
pub const ACTION_DISPLAY_SETTINGS: MenuAction = MenuAction(101);
pub const ACTION_CHANGE_WALLPAPER: MenuAction = MenuAction(102);
pub const ACTION_OPEN_TERMINAL_HERE: MenuAction = MenuAction(103);
pub const ACTION_OPEN_FILE_MANAGER: MenuAction = MenuAction(104);
pub const ACTION_REFRESH_DESKTOP: MenuAction = MenuAction(105);

// Window title bar context menu actions (200–299)
pub const ACTION_CLOSE_WINDOW: MenuAction = MenuAction(200);
pub const ACTION_MAXIMIZE_WINDOW: MenuAction = MenuAction(201);
pub const ACTION_MINIMIZE_WINDOW: MenuAction = MenuAction(202);
pub const ACTION_RESTORE_WINDOW: MenuAction = MenuAction(203);
pub const ACTION_TILE_LEFT: MenuAction = MenuAction(204);
pub const ACTION_TILE_RIGHT: MenuAction = MenuAction(205);
pub const ACTION_FULLSCREEN_TOGGLE: MenuAction = MenuAction(206);
pub const ACTION_ALWAYS_ON_TOP: MenuAction = MenuAction(207);
pub const ACTION_MOVE_TO_WORKSPACE: MenuAction = MenuAction(208);

// Dock item context menu actions (300–399)
pub const ACTION_LAUNCH_APP: MenuAction = MenuAction(300);
pub const ACTION_NEW_INSTANCE: MenuAction = MenuAction(301);
pub const ACTION_PIN_TO_DOCK: MenuAction = MenuAction(302);
pub const ACTION_UNPIN_FROM_DOCK: MenuAction = MenuAction(303);
pub const ACTION_QUIT_APP: MenuAction = MenuAction(304);
pub const ACTION_APP_INFO: MenuAction = MenuAction(305);

// Status bar / top bar context menu actions (400–499)
pub const ACTION_OPEN_SETTINGS: MenuAction = MenuAction(400);
pub const ACTION_OPEN_NOTIFICATION_CENTER: MenuAction = MenuAction(401);
pub const ACTION_OPEN_QUICK_SETTINGS: MenuAction = MenuAction(402);
pub const ACTION_LOCK_SESSION: MenuAction = MenuAction(403);
pub const ACTION_LOG_OUT: MenuAction = MenuAction(404);
pub const ACTION_RESTART: MenuAction = MenuAction(405);
pub const ACTION_SHUT_DOWN: MenuAction = MenuAction(406);

// ---------------------------------------------------------------------------
// Menu builders
// ---------------------------------------------------------------------------

/// Desktop right-click context menu items.
pub fn desktop_menu() -> Vec<MenuItem> {
    vec![
        MenuItem::action_with_icon(
            "Configure Desktop & Wallpaper",
            "preferences-system",
            ACTION_CONFIGURE_DESKTOP,
        ),
        MenuItem::action_with_icon(
            "Display Settings",
            "preferences-system",
            ACTION_DISPLAY_SETTINGS,
        ),
        MenuItem::action_with_icon(
            "Change Wallpaper",
            "camera",
            ACTION_CHANGE_WALLPAPER,
        ),
        MenuItem::action_with_icon(
            "Open Terminal Here",
            "terminal",
            ACTION_OPEN_TERMINAL_HERE,
        ),
        MenuItem::action_with_icon(
            "Open File Manager",
            "folder",
            ACTION_OPEN_FILE_MANAGER,
        ),
        MenuItem::action("Refresh Desktop", ACTION_REFRESH_DESKTOP),
    ]
}

/// Window title bar right-click context menu.
pub fn window_menu() -> Vec<MenuItem> {
    vec![
        MenuItem::action("Minimize", ACTION_MINIMIZE_WINDOW)
            .with_shortcut("Super+H"),
        MenuItem::action("Maximize", ACTION_MAXIMIZE_WINDOW)
            .with_shortcut("Super+Up"),
        MenuItem::action("Restore", ACTION_RESTORE_WINDOW),
        MenuItem::action("Tile Left", ACTION_TILE_LEFT)
            .with_shortcut("Super+Left"),
        MenuItem::action("Tile Right", ACTION_TILE_RIGHT)
            .with_shortcut("Super+Right"),
        MenuItem::action("Fullscreen", ACTION_FULLSCREEN_TOGGLE)
            .with_shortcut("F11"),
        MenuItem::action("Always on Top", ACTION_ALWAYS_ON_TOP),
        MenuItem::action("Move to Workspace…", ACTION_MOVE_TO_WORKSPACE),
        MenuItem::action("Close", ACTION_CLOSE_WINDOW)
            .with_shortcut("Alt+F4"),
    ]
}

/// Dock item right-click context menu for a pinned app.
pub fn dock_pinned_menu() -> Vec<MenuItem> {
    vec![
        MenuItem::action_with_icon("Launch", "web-browser", ACTION_LAUNCH_APP),
        MenuItem::action("New Instance", ACTION_NEW_INSTANCE),
        MenuItem::action("Unpin from Dock", ACTION_UNPIN_FROM_DOCK),
        MenuItem::action("App Info", ACTION_APP_INFO),
    ]
}

/// Dock item right-click context menu for a running (non-pinned) app.
pub fn dock_running_menu() -> Vec<MenuItem> {
    vec![
        MenuItem::action("New Instance", ACTION_NEW_INSTANCE),
        MenuItem::action("Pin to Dock", ACTION_PIN_TO_DOCK),
        MenuItem::action("App Info", ACTION_APP_INFO),
        MenuItem::action("Quit", ACTION_QUIT_APP),
    ]
}

/// Status bar (top bar) right-click context menu.
pub fn status_bar_menu() -> Vec<MenuItem> {
    vec![
        MenuItem::action_with_icon("Settings", "preferences-system", ACTION_OPEN_SETTINGS),
        MenuItem::action_with_icon(
            "Notifications",
            "notification",
            ACTION_OPEN_NOTIFICATION_CENTER,
        ),
        MenuItem::action("Quick Settings", ACTION_OPEN_QUICK_SETTINGS),
    ]
}
