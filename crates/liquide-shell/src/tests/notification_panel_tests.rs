//! Tests for notification panel toggle and status bar auto-hide with top-edge hover.

use liquide_compositor::geometry::Rect;
use liquide_input::mouse::{ButtonState, MouseButton, MouseEvent};
use liquide_platform::event_loop::PlatformEvent;
use liquide_platform::window_host::NativeWindowHandle;

use crate::shell::Shell;
use crate::shortcuts::ShellAction;
use crate::window::WindowState;

fn mouse_click(x: f32, y: f32) -> PlatformEvent {
    PlatformEvent::MouseInput {
        handle: NativeWindowHandle(0),
        event: MouseEvent::Button {
            button: MouseButton::Left,
            state: ButtonState::Pressed,
            x,
            y,
        },
    }
}

fn mouse_move(x: f32, y: f32) -> PlatformEvent {
    PlatformEvent::MouseInput {
        handle: NativeWindowHandle(0),
        event: MouseEvent::Move { x, y },
    }
}

/// Clicking the notification indicator region in the status bar should toggle
/// `notification_panel_visible` and return `OpenNotificationCenter`.
#[test]
fn notification_indicator_click_toggles_panel() {
    let mut shell = Shell::new(1920.0, 1080.0);
    assert!(!shell.notification_panel_visible);

    // The notification indicator sits 36-80 px from the right edge of the
    // screen, inside the status bar (y ≈ 0 for a top bar).
    let click_x = 1920.0 - 58.0; // middle of the 36..80 region
    let click_y = 15.0; // inside the status bar

    let action = shell.handle_platform_event(&mouse_click(click_x, click_y));
    assert!(matches!(action, Some(ShellAction::OpenNotificationCenter)));
    assert!(shell.notification_panel_visible);

    // Second click should toggle it off.
    let action2 = shell.handle_platform_event(&mouse_click(click_x, click_y));
    assert!(matches!(action2, Some(ShellAction::OpenNotificationCenter)));
    assert!(!shell.notification_panel_visible);
}

/// When a window is maximized and auto-hide is enabled, the status bar should
/// reveal itself when the cursor hovers at the very top edge (y ≤ 2 px) and
/// hide again when the cursor moves away.
#[test]
fn top_edge_hover_reveals_status_bar() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.config.status_bar.auto_hide_on_maximize = true;

    let id = shell.open_window("Max", Rect::new(0.0, 0.0, 800.0, 600.0));
    shell.maximize(id).unwrap();
    assert_eq!(shell.window(id).unwrap().state, WindowState::Maximized);

    // Move cursor away from top edge first.
    shell.handle_platform_event(&mouse_move(500.0, 500.0));
    let changed = shell.tick(1_000_000);
    // Status bar should be hidden (maximized + cursor not at top).
    assert!(!shell.status_bar_visible);

    // Now hover at the very top edge.
    shell.handle_platform_event(&mouse_move(500.0, 0.0));
    let changed = shell.tick(2_000_000);
    assert!(changed);
    assert!(shell.status_bar_visible);

    // Move cursor away again.
    shell.handle_platform_event(&mouse_move(500.0, 300.0));
    let changed = shell.tick(3_000_000);
    assert!(changed);
    assert!(!shell.status_bar_visible);
}
