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
            "preferences-desktop-wallpaper",
            ACTION_CHANGE_WALLPAPER,
        ),
        MenuItem::action_with_icon("Open Terminal Here", "terminal", ACTION_OPEN_TERMINAL_HERE),
        MenuItem::action_with_icon("Open File Manager", "folder", ACTION_OPEN_FILE_MANAGER),
        MenuItem::action("Refresh Desktop", ACTION_REFRESH_DESKTOP),
    ]
}

/// Window title bar right-click context menu.
pub fn window_menu() -> Vec<MenuItem> {
    vec![
        MenuItem::action("Minimize", ACTION_MINIMIZE_WINDOW).with_shortcut("Super+H"),
        MenuItem::action("Maximize", ACTION_MAXIMIZE_WINDOW).with_shortcut("Super+Up"),
        MenuItem::action("Restore", ACTION_RESTORE_WINDOW),
        MenuItem::action("Tile Left", ACTION_TILE_LEFT).with_shortcut("Super+Left"),
        MenuItem::action("Tile Right", ACTION_TILE_RIGHT).with_shortcut("Super+Right"),
        MenuItem::action("Fullscreen", ACTION_FULLSCREEN_TOGGLE).with_shortcut("F11"),
        MenuItem::action("Always on Top", ACTION_ALWAYS_ON_TOP),
        MenuItem::action("Move to Workspace…", ACTION_MOVE_TO_WORKSPACE),
        MenuItem::action("Close", ACTION_CLOSE_WINDOW).with_shortcut("Alt+F4"),
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

// ---------------------------------------------------------------------------
// Extended action tag constants
// ---------------------------------------------------------------------------

// File context menu actions (500–599)
pub const ACTION_OPEN_FILE: MenuAction = MenuAction(500);
pub const ACTION_OPEN_WITH: MenuAction = MenuAction(501);
pub const ACTION_CUT_FILE: MenuAction = MenuAction(502);
pub const ACTION_COPY_FILE: MenuAction = MenuAction(503);
pub const ACTION_PASTE_FILE: MenuAction = MenuAction(504);
pub const ACTION_RENAME_FILE: MenuAction = MenuAction(505);
pub const ACTION_MOVE_TO_TRASH: MenuAction = MenuAction(506);
pub const ACTION_FILE_PROPERTIES: MenuAction = MenuAction(507);
pub const ACTION_COMPRESS: MenuAction = MenuAction(508);
pub const ACTION_OPEN_IN_TERMINAL: MenuAction = MenuAction(509);
pub const ACTION_COPY_PATH: MenuAction = MenuAction(510);

// Text editing context menu actions (600–699)
pub const ACTION_UNDO: MenuAction = MenuAction(600);
pub const ACTION_REDO: MenuAction = MenuAction(601);
pub const ACTION_CUT_TEXT: MenuAction = MenuAction(602);
pub const ACTION_COPY_TEXT: MenuAction = MenuAction(603);
pub const ACTION_PASTE_TEXT: MenuAction = MenuAction(604);
pub const ACTION_SELECT_ALL: MenuAction = MenuAction(605);
pub const ACTION_DELETE_TEXT: MenuAction = MenuAction(606);

// Desktop extended actions (150–199)
pub const ACTION_NEW_FOLDER: MenuAction = MenuAction(150);
pub const ACTION_PASTE_ON_DESKTOP: MenuAction = MenuAction(151);
pub const ACTION_SORT_BY_NAME: MenuAction = MenuAction(152);
pub const ACTION_SORT_BY_DATE: MenuAction = MenuAction(153);
pub const ACTION_SORT_BY_SIZE: MenuAction = MenuAction(154);
pub const ACTION_SORT_BY_TYPE: MenuAction = MenuAction(155);

// Window titlebar extended (210–249)
pub const ACTION_MOVE_WINDOW: MenuAction = MenuAction(210);
pub const ACTION_RESIZE_WINDOW: MenuAction = MenuAction(211);

// Open With submenu entries (520–539)
pub const ACTION_OPEN_WITH_TEXT_EDITOR: MenuAction = MenuAction(520);
pub const ACTION_OPEN_WITH_IMAGE_VIEWER: MenuAction = MenuAction(521);
pub const ACTION_OPEN_WITH_BROWSER: MenuAction = MenuAction(522);
pub const ACTION_OPEN_WITH_OTHER: MenuAction = MenuAction(523);

// ---------------------------------------------------------------------------
// Extended preset menus
// ---------------------------------------------------------------------------

/// Enhanced desktop right-click context menu with submenus and separators.
///
/// Includes: Change Wallpaper, Display Settings, Sort By submenu,
/// Paste, New Folder, Open Terminal, Settings.
pub fn desktop_context_menu() -> Vec<MenuItem> {
    vec![
        MenuItem::action_with_icon(
            "Change Wallpaper",
            "preferences-desktop-wallpaper",
            ACTION_CHANGE_WALLPAPER,
        ),
        MenuItem::action_with_icon(
            "Display Settings",
            "preferences-system",
            ACTION_DISPLAY_SETTINGS,
        ),
        MenuItem::separator(),
        MenuItem::submenu(
            "Sort By",
            vec![
                MenuItem::radio("Name", ACTION_SORT_BY_NAME, 1, true),
                MenuItem::radio("Date Modified", ACTION_SORT_BY_DATE, 1, false),
                MenuItem::radio("Size", ACTION_SORT_BY_SIZE, 1, false),
                MenuItem::radio("Type", ACTION_SORT_BY_TYPE, 1, false),
            ],
        ),
        MenuItem::separator(),
        MenuItem::action_with_icon("Paste", "edit-paste", ACTION_PASTE_ON_DESKTOP)
            .with_shortcut("Ctrl+V"),
        MenuItem::action_with_icon("New Folder", "folder-new", ACTION_NEW_FOLDER)
            .with_shortcut("Ctrl+Shift+N"),
        MenuItem::separator(),
        MenuItem::action_with_icon("Open Terminal Here", "terminal", ACTION_OPEN_TERMINAL_HERE),
        MenuItem::action_with_icon("Settings", "preferences-system", ACTION_OPEN_SETTINGS),
    ]
}

/// File / folder right-click context menu.
///
/// # Parameters
/// - `is_dir`: if true, "Open" label changes to "Open Folder"
/// - `selection_count`: number of selected items (affects plural labels)
pub fn file_context_menu(is_dir: bool, selection_count: u32) -> Vec<MenuItem> {
    let open_label = if is_dir { "Open Folder" } else { "Open" };
    let delete_label = if selection_count > 1 {
        format!("Move {} Items to Trash", selection_count)
    } else {
        "Move to Trash".to_string()
    };

    let mut items = vec![MenuItem::action_with_icon(
        open_label,
        "document-open",
        ACTION_OPEN_FILE,
    )];

    if !is_dir {
        items.push(MenuItem::submenu(
            "Open With",
            vec![
                MenuItem::action_with_icon(
                    "Text Editor",
                    "text-editor",
                    ACTION_OPEN_WITH_TEXT_EDITOR,
                ),
                MenuItem::action_with_icon(
                    "Image Viewer",
                    "image-viewer",
                    ACTION_OPEN_WITH_IMAGE_VIEWER,
                ),
                MenuItem::action_with_icon("Web Browser", "web-browser", ACTION_OPEN_WITH_BROWSER),
                MenuItem::separator(),
                MenuItem::action("Other Application...", ACTION_OPEN_WITH_OTHER),
            ],
        ));
    }

    items.extend([
        MenuItem::separator(),
        MenuItem::action_with_icon("Cut", "edit-cut", ACTION_CUT_FILE).with_shortcut("Ctrl+X"),
        MenuItem::action_with_icon("Copy", "edit-copy", ACTION_COPY_FILE).with_shortcut("Ctrl+C"),
        MenuItem::action_with_icon("Paste", "edit-paste", ACTION_PASTE_FILE)
            .with_shortcut("Ctrl+V"),
        MenuItem::separator(),
        MenuItem::action("Rename", ACTION_RENAME_FILE).with_shortcut("F2"),
        MenuItem::action(&delete_label, ACTION_MOVE_TO_TRASH)
            .with_shortcut("Del")
            .with_danger(true),
        MenuItem::separator(),
        MenuItem::action("Copy Path", ACTION_COPY_PATH),
        MenuItem::action_with_icon("Compress", "package-x-generic", ACTION_COMPRESS),
    ]);

    if is_dir {
        items.push(MenuItem::action_with_icon(
            "Open in Terminal",
            "terminal",
            ACTION_OPEN_IN_TERMINAL,
        ));
    }

    items.push(MenuItem::separator());
    items.push(
        MenuItem::action_with_icon("Properties", "document-properties", ACTION_FILE_PROPERTIES)
            .with_shortcut("Alt+Enter"),
    );

    items
}

/// Text editing context menu.
///
/// # Parameters
/// - `has_selection`: whether text is currently selected (enables Cut/Copy/Delete)
/// - `is_editable`: whether the text field is editable (enables Cut/Paste/Undo/Redo)
pub fn text_context_menu(has_selection: bool, is_editable: bool) -> Vec<MenuItem> {
    let mut items = Vec::new();

    if is_editable {
        items.push(
            MenuItem::action_with_icon("Undo", "edit-undo", ACTION_UNDO).with_shortcut("Ctrl+Z"),
        );
        items.push(
            MenuItem::action_with_icon("Redo", "edit-redo", ACTION_REDO).with_shortcut("Ctrl+Y"),
        );
        items.push(MenuItem::separator());
    }

    if is_editable {
        items.push(
            MenuItem::action_with_icon("Cut", "edit-cut", ACTION_CUT_TEXT)
                .with_shortcut("Ctrl+X")
                .with_disabled(!has_selection),
        );
    }

    items.push(
        MenuItem::action_with_icon("Copy", "edit-copy", ACTION_COPY_TEXT)
            .with_shortcut("Ctrl+C")
            .with_disabled(!has_selection),
    );

    if is_editable {
        items.push(
            MenuItem::action_with_icon("Paste", "edit-paste", ACTION_PASTE_TEXT)
                .with_shortcut("Ctrl+V"),
        );
    }

    if has_selection && is_editable {
        items.push(
            MenuItem::action("Delete", ACTION_DELETE_TEXT)
                .with_shortcut("Del")
                .with_danger(true),
        );
    }

    items.push(MenuItem::separator());
    items.push(MenuItem::action("Select All", ACTION_SELECT_ALL).with_shortcut("Ctrl+A"));

    items
}

/// Enhanced window titlebar context menu.
///
/// Includes: Minimize, Maximize, Move, Resize, Always on Top (toggle), Close.
pub fn window_titlebar_menu() -> Vec<MenuItem> {
    vec![
        MenuItem::action_with_icon("Minimize", "window-minimize", ACTION_MINIMIZE_WINDOW)
            .with_shortcut("Super+H"),
        MenuItem::action_with_icon("Maximize", "window-maximize", ACTION_MAXIMIZE_WINDOW)
            .with_shortcut("Super+Up"),
        MenuItem::action("Restore", ACTION_RESTORE_WINDOW),
        MenuItem::separator(),
        MenuItem::action("Move", ACTION_MOVE_WINDOW),
        MenuItem::action("Resize", ACTION_RESIZE_WINDOW),
        MenuItem::separator(),
        MenuItem::action("Tile Left", ACTION_TILE_LEFT).with_shortcut("Super+Left"),
        MenuItem::action("Tile Right", ACTION_TILE_RIGHT).with_shortcut("Super+Right"),
        MenuItem::action("Fullscreen", ACTION_FULLSCREEN_TOGGLE).with_shortcut("F11"),
        MenuItem::separator(),
        MenuItem::checkbox("Always on Top", ACTION_ALWAYS_ON_TOP, false),
        MenuItem::separator(),
        MenuItem::action("Close", ACTION_CLOSE_WINDOW)
            .with_shortcut("Alt+F4")
            .with_danger(true),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MenuItemKind;
    use liquide_paint::icons::icon_id_for_name;

    /// Depth-first collect of every `(label, icon)` pair carried by a preset,
    /// descending into submenus so nested items are audited too.
    fn collect_icons(items: &[MenuItem], out: &mut Vec<(String, String)>) {
        for item in items {
            if let Some(icon) = &item.icon {
                out.push((item.label.clone(), icon.clone()));
            }
            if let MenuItemKind::Submenu(children) = &item.kind {
                collect_icons(children, out);
            }
        }
    }

    /// The icon id the placeholder box renders as. Any producer name that maps
    /// here is a wrong/missing icon (renders as a debuggable box, not a glyph).
    const PLACEHOLDER_ID: u32 = 0;

    /// Every icon name emitted by every shipping preset menu must resolve to a
    /// real (non-zero) glyph through the shared paint name-map. This guards the
    /// whole family against a producer emitting an unmapped name (which would
    /// render as the placeholder box).
    #[test]
    fn every_preset_icon_name_resolves_to_a_real_glyph() {
        let mut icons = Vec::new();
        collect_icons(&desktop_menu(), &mut icons);
        collect_icons(&window_menu(), &mut icons);
        collect_icons(&dock_pinned_menu(), &mut icons);
        collect_icons(&dock_running_menu(), &mut icons);
        collect_icons(&status_bar_menu(), &mut icons);
        collect_icons(&desktop_context_menu(), &mut icons);
        collect_icons(&file_context_menu(false, 1), &mut icons);
        collect_icons(&file_context_menu(true, 3), &mut icons);
        collect_icons(&text_context_menu(true, true), &mut icons);
        collect_icons(&window_titlebar_menu(), &mut icons);

        assert!(!icons.is_empty(), "presets should carry icons to audit");
        for (label, icon) in &icons {
            assert_ne!(
                icon_id_for_name(icon),
                PLACEHOLDER_ID,
                "preset item {label:?} uses icon {icon:?} which does not resolve \
                 to a real glyph (renders as the placeholder box)",
            );
        }
    }

    /// The "Change Wallpaper" action must carry the WALLPAPER glyph, not the
    /// camera glyph (the t-icon-producers semantic fix). This is RED before the
    /// fix (`camera` → id 8) and GREEN after (`preferences-desktop-wallpaper`
    /// → id 30), in BOTH desktop menu builders that diverged.
    #[test]
    fn change_wallpaper_uses_the_wallpaper_glyph_not_camera() {
        let wallpaper_id = icon_id_for_name("preferences-desktop-wallpaper");
        let camera_id = icon_id_for_name("camera");
        assert_ne!(wallpaper_id, PLACEHOLDER_ID, "wallpaper name must resolve");
        assert_ne!(
            wallpaper_id, camera_id,
            "wallpaper and camera must be distinct glyphs for the test to bite"
        );

        for items in [desktop_menu(), desktop_context_menu()] {
            let mut icons = Vec::new();
            collect_icons(&items, &mut icons);
            let (_, icon) = icons
                .iter()
                .find(|(label, _)| label == "Change Wallpaper")
                .expect("a Change Wallpaper item is present");
            assert_eq!(
                icon, "preferences-desktop-wallpaper",
                "Change Wallpaper must use the wallpaper icon name"
            );
            assert_eq!(
                icon_id_for_name(icon),
                wallpaper_id,
                "Change Wallpaper icon must resolve to the wallpaper glyph"
            );
        }
    }
}
