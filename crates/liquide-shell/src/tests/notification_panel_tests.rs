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

/// Clicking the notification indicator region in the status bar must toggle the
/// notification center EXACTLY ONCE over the full integrated input path.
///
/// Single-owner toggle contract (t59-shell): the click handler returns
/// `OpenNotificationCenter` WITHOUT mutating state; `execute_action` is the sole
/// owner of the toggle. Driving handler + execute_action (exactly as
/// `DesktopCompositor::handle_event` does) must flip the panel open, then a
/// second full click must flip it closed. The previous version of this test
/// drove only `handle_platform_event` and asserted the handler ALONE mutated —
/// which encoded the double-toggle bug (handler mutated AND execute_action
/// toggled again, cancelling the click). This version drives the real path so a
/// regression to double-toggle is caught.
#[test]
fn notification_indicator_click_toggles_panel() {
    let mut shell = Shell::new(1920.0, 1080.0);
    assert!(!shell.notification_panel_visible);

    // The notification indicator sits 36-80 px from the right edge of the
    // screen, inside the status bar (y ≈ 0 for a top bar).
    let click_x = 1920.0 - 58.0; // middle of the 36..80 region
    let click_y = 15.0; // inside the status bar

    // Full integrated path: handler returns the action, execute_action toggles.
    let action = shell.handle_platform_event(&mouse_click(click_x, click_y));
    assert!(matches!(action, Some(ShellAction::OpenNotificationCenter)));
    assert!(shell.execute_action(&action.unwrap()));
    assert!(
        shell.notification_panel_visible,
        "one full click must OPEN the center (single toggle, not double)"
    );

    // Second full click should toggle it off.
    let action2 = shell.handle_platform_event(&mouse_click(click_x, click_y));
    assert!(matches!(action2, Some(ShellAction::OpenNotificationCenter)));
    assert!(shell.execute_action(&action2.unwrap()));
    assert!(
        !shell.notification_panel_visible,
        "a second full click must CLOSE the center"
    );
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
    let _changed = shell.tick(1_000_000);
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

#[test]
fn configured_reveal_distance_controls_auto_hide() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.config.status_bar.auto_hide_on_maximize = true;
    shell.config.status_bar.auto_hide_reveal_distance = 6.0;
    shell.status_bar = liquide_statusbar::ShellStatusBar::new(shell.config.status_bar.clone());

    let id = shell.open_window("Max", Rect::new(0.0, 0.0, 800.0, 600.0));
    shell.maximize(id).unwrap();

    shell.status_bar_visible = false;
    shell.handle_platform_event(&mouse_move(500.0, 5.0));
    shell.tick(1_000_000);
    assert!(shell.status_bar_visible);

    shell.handle_platform_event(&mouse_move(500.0, 7.0));
    shell.tick(2_000_000);
    assert!(!shell.status_bar_visible);
}

#[test]
fn launcher_click_respects_show_app_menu_setting() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.config.status_bar.show_app_menu = false;
    shell.status_bar = liquide_statusbar::ShellStatusBar::new(shell.config.status_bar.clone());

    let action = shell.handle_platform_event(&mouse_click(12.0, 15.0));
    assert!(action.is_none());
    assert!(!shell.launcher.is_visible());
}

// ── t187 teeth: E (one styled, positioned notification center; no TL leak) ────

const VARIABLES_CSS: &str = include_str!("../../../../assets/themes/variables.css");
const COMPONENTS_NOTIFICATIONS_CSS: &str =
    include_str!("../../../../assets/themes/components/notifications.css");

/// E (notification center is ONE styled, correctly-positioned panel — no
/// top-left unstyled leak): with the notification-center CSS loaded, an open
/// center with one item lays out as a SINGLE positioned panel that is NOT in the
/// top-left menu-bar corner.
///
/// RED before t187: the `notification-center` DOM subtree had ZERO matching CSS,
/// so it laid out in normal flow at the document origin (x≈0, y≈0) — bare text
/// leaking over the menu bar. GREEN after: `components/notifications.css` styles
/// `notification-center*` as a fixed top-RIGHT panel.
#[test]
fn notification_center_is_one_styled_positioned_panel() {
    let mut shell = Shell::new(1280.0, 720.0);
    shell.cursor_blink_on = true;
    shell.cursor_blink_time_us = u64::MAX;
    // Load the design tokens + the notification component rules (the same base
    // layers the live DE loads). Without the center CSS the panel would leak to
    // the top-left.
    shell.add_stylesheet(VARIABLES_CSS);
    shell.add_stylesheet(COMPONENTS_NOTIFICATIONS_CSS);

    let mut notif = liquide_interop::notification::Notification::new(
        "Visual Test",
        "Notification center entry",
    );
    notif.body = "An item to populate the notification center.".to_string();
    let _ = shell.post_notification(notif, 0);
    assert!(shell.open_notification_center());
    let _ = shell.build_scene();

    // Exactly ONE notification-center element exists (no duplicate track).
    let mut centers = 0usize;
    fn count_centers(doc: &liquide_dom::Document, n: liquide_dom::NodeId, acc: &mut usize) {
        if let Some(node) = doc.get(n) {
            if node.tag_name() == "notification-center" {
                *acc += 1;
            }
        }
        for &c in doc.children(n) {
            count_centers(doc, c, acc);
        }
    }
    count_centers(
        &shell.desktop_dom.doc,
        shell.desktop_dom.doc.root(),
        &mut centers,
    );
    assert_eq!(
        centers, 1,
        "there must be exactly ONE notification-center track, got {centers}"
    );

    // The center is laid out as a POSITIONED panel — NOT parked in the top-left
    // menu-bar corner (the unstyled-leak signature is x≈0 AND y≈0).
    let center = shell
        .desktop_dom
        .doc
        .get_element_by_id("notification-center")
        .expect("notification-center element");
    let b = shell
        .hit_test_engine
        .as_ref()
        .expect("hit-test engine")
        .bounds_for_node(center)
        .expect("notification-center laid-out box");
    let in_top_left_menu_bar = b.x < 40.0 && b.y < 40.0;
    assert!(
        !in_top_left_menu_bar,
        "notification-center must NOT render in the top-left menu-bar region \
         (the unstyled-leak signature); got box {b:?}"
    );
    // It is anchored to the RIGHT half of the screen (top-right panel).
    assert!(
        b.x > 1280.0 / 2.0,
        "the styled center panel must be anchored on the right, got x={}",
        b.x
    );
    // And it has real extent (a styled panel, not a zero-box).
    assert!(
        b.width > 100.0 && b.height > 20.0,
        "the center panel must have panel-sized extent, got {b:?}"
    );
}
