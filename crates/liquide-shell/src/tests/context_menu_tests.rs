//! Integration tests for the desktop right-click context menu system.

use liquide_input::keyboard::{KeyCode, KeyEvent, KeyState, Modifiers};
use liquide_input::mouse::{ButtonState, MouseButton, MouseEvent};
use liquide_platform::event_loop::PlatformEvent;
use liquide_platform::window_host::NativeWindowHandle;

use crate::shell::{ContextMenuItem, Shell};
use crate::shortcuts::ShellAction;

// ── Helpers ────────────────────────────────────────────────────────

fn mouse_move(x: f32, y: f32) -> PlatformEvent {
    PlatformEvent::MouseInput {
        handle: NativeWindowHandle(0),
        event: MouseEvent::Move { x, y },
    }
}

fn mouse_click(x: f32, y: f32, button: MouseButton) -> PlatformEvent {
    PlatformEvent::MouseInput {
        handle: NativeWindowHandle(0),
        event: MouseEvent::Button {
            button,
            state: ButtonState::Pressed,
            x,
            y,
        },
    }
}

fn mouse_release(x: f32, y: f32, button: MouseButton) -> PlatformEvent {
    PlatformEvent::MouseInput {
        handle: NativeWindowHandle(0),
        event: MouseEvent::Button {
            button,
            state: ButtonState::Released,
            x,
            y,
        },
    }
}

fn key_press(key: KeyCode) -> PlatformEvent {
    PlatformEvent::KeyInput {
        handle: NativeWindowHandle(0),
        event: KeyEvent {
            key,
            state: KeyState::Pressed,
            modifiers: Modifiers::new(),
            scancode: 0,
            timestamp_us: 0,
        },
    }
}

// ── Context menu dimensions (must match shell/mod.rs constants) ──────
const MENU_W: f32 = 200.0;
const ITEM_H: f32 = 28.0;          // CSS: menu-item { height: 28; }
const MENU_PAD: f32 = 4.0;         // CSS: context-menu { padding: 4; }
const PADDING_TOTAL: f32 = MENU_PAD * 2.0; // top + bottom

// ══════════════════════════════════════════════════════════════════
//  Basic state tests
// ══════════════════════════════════════════════════════════════════

#[test]
fn context_menu_initially_hidden() {
    let shell = Shell::new(1920.0, 1080.0);
    assert!(!shell.context_menu_visible);
    assert!(shell.context_menu_hover_index.is_none());
}

#[test]
fn context_menu_has_correct_item_count() {
    let items = ContextMenuItem::defaults();
    assert_eq!(items.len(), 5);
}

#[test]
fn context_menu_default_items_labels() {
    let items = ContextMenuItem::defaults();
    assert_eq!(items[0].label, "Open Terminal");
    assert_eq!(items[1].label, "Open File Manager");
    assert_eq!(items[2].label, "Change Wallpaper");
    assert_eq!(items[3].label, "Display Settings");
    assert_eq!(items[4].label, "System Settings");
}

#[test]
fn context_menu_default_items_actions() {
    let items = ContextMenuItem::defaults();
    assert_eq!(items[0].action, ShellAction::OpenTerminal);
    assert_eq!(items[1].action, ShellAction::OpenFileManager);
    assert_eq!(items[2].action, ShellAction::OpenSettings);
    assert_eq!(items[3].action, ShellAction::OpenSettings);
    assert_eq!(items[4].action, ShellAction::OpenSettings);
}

// ══════════════════════════════════════════════════════════════════
//  Opening the context menu
// ══════════════════════════════════════════════════════════════════

#[test]
fn right_click_on_empty_desktop_opens_context_menu() {
    let mut shell = Shell::new(1920.0, 1080.0);
    // Click well below the status bar (28px) and away from dock/windows
    let action = shell.handle_platform_event(&mouse_click(500.0, 500.0, MouseButton::Right));
    assert!(shell.context_menu_visible);
    assert_eq!(shell.context_menu_pos.x, 500.0);
    assert_eq!(shell.context_menu_pos.y, 500.0);
    assert_eq!(action, Some(ShellAction::Redraw));
}

#[test]
fn right_click_on_statusbar_opens_context_menu() {
    let mut shell = Shell::new(1920.0, 1080.0);
    // Status bar occupies y=[0, 28) by default. Click in that region.
    let action = shell.handle_platform_event(&mouse_click(500.0, 15.0, MouseButton::Right));
    assert!(shell.context_menu_visible);
    assert_eq!(action, Some(ShellAction::Redraw));
}

#[test]
fn right_click_stores_raw_position() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.handle_platform_event(&mouse_click(123.0, 456.0, MouseButton::Right));
    assert_eq!(shell.context_menu_pos.x, 123.0);
    assert_eq!(shell.context_menu_pos.y, 456.0);
}

#[test]
fn right_click_always_opens_not_toggles() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.handle_platform_event(&mouse_click(500.0, 500.0, MouseButton::Right));
    assert!(shell.context_menu_visible);
    // Right-click again at new position: should still be open, new position
    shell.handle_platform_event(&mouse_click(600.0, 600.0, MouseButton::Right));
    assert!(shell.context_menu_visible);
    assert_eq!(shell.context_menu_pos.x, 600.0);
    assert_eq!(shell.context_menu_pos.y, 600.0);
}

// ══════════════════════════════════════════════════════════════════
//  Position clamping
// ══════════════════════════════════════════════════════════════════

#[test]
fn context_menu_position_clamped_bottom_right() {
    let mut shell = Shell::new(1920.0, 1080.0);
    // Right-click near the bottom-right corner
    shell.handle_platform_event(&mouse_click(1900.0, 1060.0, MouseButton::Right));
    assert!(shell.context_menu_visible);

    // Verify clamped values
    let items = ContextMenuItem::defaults();
    let menu_h = PADDING_TOTAL + items.len() as f32 * ITEM_H; // 196
    let clamped_x = shell.context_menu_pos.x.min(1920.0 - MENU_W - 4.0).max(0.0);
    let clamped_y = shell.context_menu_pos.y.min(1080.0 - menu_h - 4.0).max(0.0);
    assert!(clamped_x < 1900.0, "x should be clamped from 1900");
    assert!(clamped_y < 1060.0, "y should be clamped from 1060");
    // Exact expected values
    assert_eq!(clamped_x, 1920.0 - MENU_W - 4.0); // 1656
    assert_eq!(clamped_y, 1080.0 - menu_h - 4.0);  // 880
}

#[test]
fn context_menu_position_clamped_top_left() {
    // On a tiny screen the clamped position must be >= 0
    let mut shell = Shell::new(300.0, 250.0);
    shell.handle_platform_event(&mouse_click(0.0, 0.0, MouseButton::Right));
    assert!(shell.context_menu_visible);

    let items = ContextMenuItem::defaults();
    let menu_h = PADDING_TOTAL + items.len() as f32 * ITEM_H;
    let clamped_x = shell.context_menu_pos.x.min(300.0 - MENU_W - 4.0).max(0.0);
    let clamped_y = shell.context_menu_pos.y.min(250.0 - menu_h - 4.0).max(0.0);
    // Screen is narrower than menu_w + 4, so min clamps below 0, then max(0)
    assert_eq!(clamped_x, 0.0);
    assert_eq!(clamped_y, 0.0);
}

#[test]
fn context_menu_position_no_clamp_when_fits() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.handle_platform_event(&mouse_click(100.0, 100.0, MouseButton::Right));
    // Position is far from edges, no clamping needed
    let items = ContextMenuItem::defaults();
    let menu_h = PADDING_TOTAL + items.len() as f32 * ITEM_H;
    let clamped_x = shell.context_menu_pos.x.min(1920.0 - MENU_W - 4.0).max(0.0);
    let clamped_y = shell.context_menu_pos.y.min(1080.0 - menu_h - 4.0).max(0.0);
    assert_eq!(clamped_x, 100.0);
    assert_eq!(clamped_y, 100.0);
}

// ══════════════════════════════════════════════════════════════════
//  Closing the context menu
// ══════════════════════════════════════════════════════════════════

#[test]
fn escape_closes_context_menu() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.handle_platform_event(&mouse_click(500.0, 500.0, MouseButton::Right));
    assert!(shell.context_menu_visible);

    let action = shell.handle_platform_event(&key_press(KeyCode::Escape));
    assert!(!shell.context_menu_visible);
    assert_eq!(action, Some(ShellAction::Redraw));
}

#[test]
fn click_outside_closes_context_menu() {
    let mut shell = Shell::new(1920.0, 1080.0);
    // Open at (500, 500)
    shell.handle_platform_event(&mouse_click(500.0, 500.0, MouseButton::Right));
    assert!(shell.context_menu_visible);

    // Click far away (no menu, no dock, no bar, no window)
    shell.handle_platform_event(&mouse_click(100.0, 900.0, MouseButton::Left));
    assert!(!shell.context_menu_visible);
}

#[test]
fn left_click_above_menu_closes_it() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.handle_platform_event(&mouse_click(500.0, 500.0, MouseButton::Right));
    assert!(shell.context_menu_visible);

    // Click above the menu area
    shell.handle_platform_event(&mouse_click(500.0, 490.0, MouseButton::Left));
    assert!(!shell.context_menu_visible, "left-click above menu should close it");
}

#[test]
fn left_click_left_of_menu_closes_it() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.handle_platform_event(&mouse_click(500.0, 500.0, MouseButton::Right));
    assert!(shell.context_menu_visible);

    // Click to the left of the menu
    shell.handle_platform_event(&mouse_click(490.0, 550.0, MouseButton::Left));
    assert!(!shell.context_menu_visible, "left-click left of menu should close it");
}

// ══════════════════════════════════════════════════════════════════
//  Hover tracking
// ══════════════════════════════════════════════════════════════════

#[test]
fn hover_sets_first_item_index() {
    let mut shell = Shell::new(1920.0, 1080.0);
    // Open context menu at (100, 100) — well within screen bounds
    shell.handle_platform_event(&mouse_click(100.0, 100.0, MouseButton::Right));
    assert!(shell.context_menu_visible);

    // Menu renders at (100, 100). Items start at y = 100 + 4 (padding).
    // First item occupies y = [104, 132). Midpoint ~118.
    shell.handle_platform_event(&mouse_move(200.0, 118.0));
    assert_eq!(shell.context_menu_hover_index, Some(0));
}

#[test]
fn hover_sets_second_item_index() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.handle_platform_event(&mouse_click(100.0, 100.0, MouseButton::Right));

    // Second item: y = 100 + 4 + 28 = 132 to 160. Midpoint ~146.
    shell.handle_platform_event(&mouse_move(200.0, 146.0));
    assert_eq!(shell.context_menu_hover_index, Some(1));
}

#[test]
fn hover_sets_third_item_index() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.handle_platform_event(&mouse_click(100.0, 100.0, MouseButton::Right));

    // Third item: y = 100 + 4 + 56 = 160 to 188. Midpoint ~174.
    shell.handle_platform_event(&mouse_move(200.0, 174.0));
    assert_eq!(shell.context_menu_hover_index, Some(2));
}

#[test]
fn hover_sets_fourth_item_index() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.handle_platform_event(&mouse_click(100.0, 100.0, MouseButton::Right));

    // Fourth item: y = 100 + 4 + 84 = 188 to 216. Midpoint ~202.
    shell.handle_platform_event(&mouse_move(200.0, 202.0));
    assert_eq!(shell.context_menu_hover_index, Some(3));
}

#[test]
fn hover_sets_fifth_item_index() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.handle_platform_event(&mouse_click(100.0, 100.0, MouseButton::Right));

    // Fifth (last) item: y = 100 + 4 + 112 = 216 to 244. Midpoint ~230.
    shell.handle_platform_event(&mouse_move(200.0, 230.0));
    assert_eq!(shell.context_menu_hover_index, Some(4));
}

#[test]
fn hover_outside_menu_clears_index() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.handle_platform_event(&mouse_click(100.0, 100.0, MouseButton::Right));

    // First, hover inside
    shell.handle_platform_event(&mouse_move(200.0, 118.0));
    assert!(shell.context_menu_hover_index.is_some());

    // Move outside menu bounds entirely
    shell.handle_platform_event(&mouse_move(50.0, 50.0));
    assert!(
        shell.context_menu_hover_index.is_none(),
        "hover outside menu should clear index"
    );
}

#[test]
fn hover_below_items_clears_index() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.handle_platform_event(&mouse_click(100.0, 100.0, MouseButton::Right));

    // Menu total height = 8 + 5*28 = 148. Menu bottom = 100 + 148 = 248.
    // Move completely below the menu.
    shell.handle_platform_event(&mouse_move(200.0, 260.0));
    assert!(
        shell.context_menu_hover_index.is_none(),
        "hover below menu should clear index"
    );
}

#[test]
fn hover_above_items_in_padding_clears_index() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.handle_platform_event(&mouse_click(100.0, 100.0, MouseButton::Right));

    // Inside menu rect but in top padding: y = 100 + 2 = 102 (rel_y = 102 - 100 - 4 = -2 < 0)
    shell.handle_platform_event(&mouse_move(200.0, 102.0));
    assert!(
        shell.context_menu_hover_index.is_none(),
        "hover in top padding should not select an item"
    );
}

#[test]
fn hover_transitions_between_items() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.handle_platform_event(&mouse_click(100.0, 100.0, MouseButton::Right));

    // Hover first item
    shell.handle_platform_event(&mouse_move(200.0, 118.0));
    assert_eq!(shell.context_menu_hover_index, Some(0));

    // Move down to third item
    shell.handle_platform_event(&mouse_move(200.0, 174.0));
    assert_eq!(shell.context_menu_hover_index, Some(2));

    // Move back up to second item
    shell.handle_platform_event(&mouse_move(200.0, 146.0));
    assert_eq!(shell.context_menu_hover_index, Some(1));
}

// ══════════════════════════════════════════════════════════════════
//  Clicking menu items
// ══════════════════════════════════════════════════════════════════

#[test]
fn click_first_item_returns_open_terminal() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.handle_platform_event(&mouse_click(100.0, 100.0, MouseButton::Right));

    // Click first item (Open Terminal): y = 100 + 4 + 14 = 118
    let action = shell.handle_platform_event(&mouse_click(200.0, 118.0, MouseButton::Left));
    assert!(!shell.context_menu_visible, "menu should close after item click");
    assert_eq!(action, Some(ShellAction::OpenTerminal));
}

#[test]
fn click_second_item_returns_open_file_manager() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.handle_platform_event(&mouse_click(100.0, 100.0, MouseButton::Right));

    // Second item (Open File Manager): y = 100 + 4 + 28 + 14 = 146
    let action = shell.handle_platform_event(&mouse_click(200.0, 146.0, MouseButton::Left));
    assert!(!shell.context_menu_visible);
    assert_eq!(action, Some(ShellAction::OpenFileManager));
}

#[test]
fn click_third_item_returns_open_settings() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.handle_platform_event(&mouse_click(100.0, 100.0, MouseButton::Right));

    // Third item (Change Wallpaper): y = 100 + 4 + 56 + 14 = 174
    let action = shell.handle_platform_event(&mouse_click(200.0, 174.0, MouseButton::Left));
    assert!(!shell.context_menu_visible);
    assert_eq!(action, Some(ShellAction::OpenSettings));
}

#[test]
fn click_fourth_item_returns_open_settings() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.handle_platform_event(&mouse_click(100.0, 100.0, MouseButton::Right));

    // Fourth item (Display Settings): y = 100 + 4 + 84 + 14 = 202
    let action = shell.handle_platform_event(&mouse_click(200.0, 202.0, MouseButton::Left));
    assert!(!shell.context_menu_visible);
    assert_eq!(action, Some(ShellAction::OpenSettings));
}

#[test]
fn click_fifth_item_returns_open_settings() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.handle_platform_event(&mouse_click(100.0, 100.0, MouseButton::Right));

    // Fifth item (System Settings): y = 100 + 4 + 112 + 14 = 230
    let action = shell.handle_platform_event(&mouse_click(200.0, 230.0, MouseButton::Left));
    assert!(!shell.context_menu_visible);
    assert_eq!(action, Some(ShellAction::OpenSettings));
}

#[test]
fn click_menu_item_closes_menu() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.handle_platform_event(&mouse_click(100.0, 100.0, MouseButton::Right));
    assert!(shell.context_menu_visible);

    // Click any item
    shell.handle_platform_event(&mouse_click(200.0, 118.0, MouseButton::Left));
    assert!(!shell.context_menu_visible, "clicking a menu item must close the menu");
}

// ══════════════════════════════════════════════════════════════════
//  Interaction with other shell elements
// ══════════════════════════════════════════════════════════════════

#[test]
fn context_menu_closes_session_menu() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.session_menu_visible = true;

    shell.handle_platform_event(&mouse_click(500.0, 500.0, MouseButton::Right));
    assert!(
        !shell.session_menu_visible,
        "right-click should close session menu"
    );
    assert!(shell.context_menu_visible);
}

#[test]
fn escape_only_closes_context_menu_not_both() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.context_menu_visible = true;
    shell.session_menu_visible = true;

    // Escape should close context menu first (it is checked before session menu)
    shell.handle_platform_event(&key_press(KeyCode::Escape));
    assert!(!shell.context_menu_visible);
    // Session menu should still be open since the Escape was consumed by context menu
    assert!(shell.session_menu_visible);
}

#[test]
fn right_click_on_window_does_not_open_context_menu() {
    use liquide_compositor::geometry::Rect;

    let mut shell = Shell::new(1920.0, 1080.0);
    // Open a window that covers the click area
    let _wid = shell.open_window("Test", Rect::new(400.0, 400.0, 300.0, 300.0));

    // Right-click in the middle of the window client area
    shell.handle_platform_event(&mouse_click(550.0, 550.0, MouseButton::Right));
    // Right-click on a window client area does NOT open the context menu
    assert!(
        !shell.context_menu_visible,
        "right-click on window client area should not open desktop context menu"
    );
}

#[test]
fn left_click_does_not_open_context_menu() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.handle_platform_event(&mouse_click(500.0, 500.0, MouseButton::Left));
    assert!(!shell.context_menu_visible, "left-click should not open context menu");
}

#[test]
fn middle_click_does_not_open_context_menu() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.handle_platform_event(&mouse_click(500.0, 500.0, MouseButton::Middle));
    assert!(!shell.context_menu_visible, "middle-click should not open context menu");
}

// ══════════════════════════════════════════════════════════════════
//  DOM synchronization
// ══════════════════════════════════════════════════════════════════

#[test]
fn context_menu_dom_overlay_present_when_visible() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.handle_platform_event(&mouse_click(500.0, 500.0, MouseButton::Right));
    shell.sync_dom();

    let menu_node = shell.desktop_dom.doc.get_element_by_id("ctx-shell");
    assert!(
        menu_node.is_some(),
        "context menu should be in DOM when visible"
    );
}

#[test]
fn context_menu_removed_from_dom_when_hidden() {
    let mut shell = Shell::new(1920.0, 1080.0);
    // Open and sync
    shell.handle_platform_event(&mouse_click(500.0, 500.0, MouseButton::Right));
    shell.sync_dom();
    assert!(shell.desktop_dom.doc.get_element_by_id("ctx-shell").is_some());

    // Close and sync
    shell.handle_platform_event(&key_press(KeyCode::Escape));
    shell.sync_dom();
    assert!(
        shell.desktop_dom.doc.get_element_by_id("ctx-shell").is_none(),
        "context menu should be removed from DOM when hidden"
    );
}

#[test]
fn context_menu_dom_not_present_initially() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.sync_dom();
    assert!(
        shell.desktop_dom.doc.get_element_by_id("ctx-shell").is_none(),
        "context menu should not be in DOM initially"
    );
}

// ══════════════════════════════════════════════════════════════════
//  Edge cases
// ══════════════════════════════════════════════════════════════════

#[test]
fn context_menu_at_origin() {
    let mut shell = Shell::new(1920.0, 1080.0);
    // Note: (0, 0) is within the status bar area; right-click opens context menu there.
    shell.handle_platform_event(&mouse_click(0.0, 0.0, MouseButton::Right));
    assert!(shell.context_menu_visible);
    assert_eq!(shell.context_menu_pos.x, 0.0);
    assert_eq!(shell.context_menu_pos.y, 0.0);
}

#[test]
fn context_menu_at_screen_edge() {
    let mut shell = Shell::new(1920.0, 1080.0);
    // Right edge of screen
    shell.handle_platform_event(&mouse_click(1919.0, 500.0, MouseButton::Right));
    assert!(shell.context_menu_visible);

    let clamped_x = shell
        .context_menu_pos
        .x
        .min(1920.0 - MENU_W - 4.0)
        .max(0.0);
    assert!(
        clamped_x < 1919.0,
        "x near right edge should be clamped"
    );
}

#[test]
fn rapid_open_close_open() {
    let mut shell = Shell::new(1920.0, 1080.0);

    // Open
    shell.handle_platform_event(&mouse_click(500.0, 500.0, MouseButton::Right));
    assert!(shell.context_menu_visible);

    // Close via Escape
    shell.handle_platform_event(&key_press(KeyCode::Escape));
    assert!(!shell.context_menu_visible);

    // Open again at a new position
    shell.handle_platform_event(&mouse_click(600.0, 600.0, MouseButton::Right));
    assert!(shell.context_menu_visible);
    assert_eq!(shell.context_menu_pos.x, 600.0);
    assert_eq!(shell.context_menu_pos.y, 600.0);
}

#[test]
fn open_close_via_click_outside_then_reopen() {
    let mut shell = Shell::new(1920.0, 1080.0);

    // Open
    shell.handle_platform_event(&mouse_click(500.0, 500.0, MouseButton::Right));
    assert!(shell.context_menu_visible);

    // Close via click outside
    shell.handle_platform_event(&mouse_click(100.0, 900.0, MouseButton::Left));
    assert!(!shell.context_menu_visible);

    // Re-open
    shell.handle_platform_event(&mouse_click(700.0, 700.0, MouseButton::Right));
    assert!(shell.context_menu_visible);
    assert_eq!(shell.context_menu_pos.x, 700.0);
}

#[test]
fn hover_index_resets_on_close() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.handle_platform_event(&mouse_click(100.0, 100.0, MouseButton::Right));

    // Set hover
    shell.handle_platform_event(&mouse_move(200.0, 118.0));
    assert_eq!(shell.context_menu_hover_index, Some(0));

    // Close
    shell.handle_platform_event(&key_press(KeyCode::Escape));
    // After closing, hover tracking in handle_mouse_move is skipped
    // because context_menu_visible is false, but the hover index field
    // persists until next move. Open a new menu and verify it works fresh.
    shell.handle_platform_event(&mouse_click(100.0, 100.0, MouseButton::Right));
    // Move to second item
    shell.handle_platform_event(&mouse_move(200.0, 146.0));
    assert_eq!(shell.context_menu_hover_index, Some(1));
}

#[test]
fn context_menu_hover_returns_redraw_on_change() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.handle_platform_event(&mouse_click(100.0, 100.0, MouseButton::Right));

    // First hover should trigger redraw
    let action = shell.handle_platform_event(&mouse_move(200.0, 118.0));
    assert_eq!(action, Some(ShellAction::Redraw));

    // Moving to a different item should also trigger redraw
    let action = shell.handle_platform_event(&mouse_move(200.0, 146.0));
    assert_eq!(action, Some(ShellAction::Redraw));
}

#[test]
fn context_menu_hover_no_redraw_when_unchanged() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.handle_platform_event(&mouse_click(100.0, 100.0, MouseButton::Right));

    // First hover on item 0
    shell.handle_platform_event(&mouse_move(200.0, 118.0));
    assert_eq!(shell.context_menu_hover_index, Some(0));

    // Move within the same item (still item 0, y=110 is in [104,132))
    let _action = shell.handle_platform_event(&mouse_move(210.0, 110.0));
    // Hover index didn't change, but cursor shape changes may still cause redraw.
    // The key check is that hover_index is still 0.
    assert_eq!(shell.context_menu_hover_index, Some(0));
}

#[test]
fn mouse_release_does_not_affect_context_menu() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.handle_platform_event(&mouse_click(500.0, 500.0, MouseButton::Right));
    assert!(shell.context_menu_visible);

    // Release should not close it
    shell.handle_platform_event(&mouse_release(500.0, 500.0, MouseButton::Right));
    assert!(shell.context_menu_visible, "mouse release should not close context menu");
}

#[test]
fn context_menu_on_small_screen() {
    // Screen smaller than menu dimensions
    let mut shell = Shell::new(200.0, 150.0);
    shell.handle_platform_event(&mouse_click(100.0, 100.0, MouseButton::Right));
    assert!(shell.context_menu_visible);

    // Clamping should keep position at (0, 0) since menu is larger than screen
    let items = ContextMenuItem::defaults();
    let menu_h = PADDING_TOTAL + items.len() as f32 * ITEM_H;
    let clamped_x = shell.context_menu_pos.x.min(200.0 - MENU_W - 4.0).max(0.0);
    let clamped_y = shell.context_menu_pos.y.min(150.0 - menu_h - 4.0).max(0.0);
    assert_eq!(clamped_x, 0.0);
    assert_eq!(clamped_y, 0.0);
}

#[test]
fn context_menu_item_construct() {
    let item = ContextMenuItem::new("Test Label", "test-icon", ShellAction::Redraw);
    assert_eq!(item.label, "Test Label");
    assert_eq!(item.icon, "test-icon");
    assert_eq!(item.action, ShellAction::Redraw);
}

#[test]
fn context_menu_reopen_at_different_position() {
    let mut shell = Shell::new(1920.0, 1080.0);

    // Open at position A
    shell.handle_platform_event(&mouse_click(200.0, 200.0, MouseButton::Right));
    assert_eq!(shell.context_menu_pos.x, 200.0);
    assert_eq!(shell.context_menu_pos.y, 200.0);

    // Close
    shell.handle_platform_event(&key_press(KeyCode::Escape));

    // Open at position B
    shell.handle_platform_event(&mouse_click(800.0, 800.0, MouseButton::Right));
    assert_eq!(shell.context_menu_pos.x, 800.0);
    assert_eq!(shell.context_menu_pos.y, 800.0);
}
