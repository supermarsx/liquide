//! t65-s2 input-pipeline wiring tests.
//!
//! Covers the slice-S2 wired paths:
//! 1. Capture-phase DOM dispatch (`dispatch_dom_event_path` builds a root-first
//!    `event_path` and a capturing listener swallows a descendant event).
//! 2. Keyboard → DOM dispatch (`dispatch_dom_keyboard_event` reaches a focused
//!    DOM listener).
//! 3. `preventDefault` gating (a preventable listener suppresses the shell
//!    shortcut for the same key).
//! 4. The 14 previously-dead `execute_action` arms.
//! 5. (`set_context` is exercised indirectly; the resolver context tracks the
//!    live viewport — see `style_resolver_context_tracks_viewport`.)

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::shell::{ScreenshotRequest, Shell};
use crate::shortcuts::ShellAction;

use liquide_dom::NodeId;
use liquide_hit_test::event::{DomEvent, DomEventKind, Propagation};
use liquide_input::keyboard::{KeyCode, KeyEvent, KeyState, Modifiers};
use liquide_platform::event_loop::PlatformEvent;
use liquide_platform::window_host::NativeWindowHandle;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn key_press(key: KeyCode, modifiers: Modifiers) -> PlatformEvent {
    PlatformEvent::KeyInput {
        handle: NativeWindowHandle(0),
        event: KeyEvent {
            key,
            state: KeyState::Pressed,
            modifiers,
            scancode: 0,
            timestamp_us: 0,
        },
    }
}

/// Build two DOM nodes (parent + child) under the document root and return them.
fn build_parent_child(shell: &mut Shell) -> (NodeId, NodeId) {
    let doc = &mut shell.desktop_dom.doc;
    let root = doc.root();
    let parent = doc.create_element("div");
    let child = doc.create_element("span");
    doc.append_child(root, parent);
    doc.append_child(parent, child);
    (parent, child)
}

// ---------------------------------------------------------------------------
// 1. Capture-phase dispatch
// ---------------------------------------------------------------------------

#[test]
fn capture_phase_listener_swallows_child_event() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let (parent, child) = build_parent_child(&mut shell);

    let parent_hits = Arc::new(AtomicU32::new(0));
    let child_hits = Arc::new(AtomicU32::new(0));

    // Capturing listener on the PARENT that stops propagation before the child.
    let pc = parent_hits.clone();
    shell.add_capturing_event_handler(
        parent,
        None,
        true,
        Box::new(move |_e| {
            pc.fetch_add(1, Ordering::SeqCst);
            Propagation::StopPropagation
        }),
    );
    // Bubble listener on the child target.
    let cc = child_hits.clone();
    shell.add_capturing_event_handler(
        child,
        None,
        false,
        Box::new(move |_e| {
            cc.fetch_add(1, Ordering::SeqCst);
            Propagation::Continue
        }),
    );

    let event = DomEvent::new(
        child,
        DomEventKind::Click {
            button: liquide_hit_test::event::MouseButton::Left,
            x: 0.0,
            y: 0.0,
        },
    );
    shell.dispatch_dom_event_path(child, vec![event]);

    assert_eq!(
        parent_hits.load(Ordering::SeqCst),
        1,
        "capturing parent listener should fire"
    );
    assert_eq!(
        child_hits.load(Ordering::SeqCst),
        0,
        "child listener should be swallowed by the capturing parent (stopPropagation)"
    );
}

#[test]
fn dispatch_builds_root_first_event_path() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let (parent, child) = build_parent_child(&mut shell);

    // Capture order should be root → parent (front-to-back of event_path).
    let order = Arc::new(std::sync::Mutex::new(Vec::<NodeId>::new()));
    for node in [parent, child] {
        let ord = order.clone();
        shell.add_capturing_event_handler(
            node,
            None,
            true,
            Box::new(move |e| {
                ord.lock().unwrap().push(e.current_target);
                Propagation::Continue
            }),
        );
    }

    let event = DomEvent::new(
        child,
        DomEventKind::MouseDown {
            button: liquide_hit_test::event::MouseButton::Left,
            x: 0.0,
            y: 0.0,
        },
    );
    shell.dispatch_dom_event_path(child, vec![event]);

    let seen = order.lock().unwrap().clone();
    // Parent is captured before the at-target child.
    assert_eq!(
        seen,
        vec![parent, child],
        "capture should run parent (ancestor) before target"
    );
}

// ---------------------------------------------------------------------------
// 2. Keyboard → DOM dispatch
// ---------------------------------------------------------------------------

#[test]
fn keyboard_event_reaches_focused_dom_listener() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let (_parent, child) = build_parent_child(&mut shell);

    let key_hits = Arc::new(AtomicU32::new(0));
    let kc = key_hits.clone();
    shell.add_capturing_event_handler(
        child,
        Some(DomEventKind::KeyDown {
            key: 0,
            modifiers: 0,
        }),
        false,
        Box::new(move |_e| {
            kc.fetch_add(1, Ordering::SeqCst);
            Propagation::Continue
        }),
    );

    // Focus the DOM node so keyboard dispatch targets it.
    shell.set_dom_focus(Some(child));

    let ke = KeyEvent {
        key: KeyCode::A,
        state: KeyState::Pressed,
        modifiers: Modifiers::default(),
        scancode: 0,
        timestamp_us: 0,
    };
    let prevented = shell.dispatch_dom_keyboard_event(&ke);

    assert_eq!(
        key_hits.load(Ordering::SeqCst),
        1,
        "focused DOM KeyDown listener should fire"
    );
    assert!(!prevented, "no listener called preventDefault");
}

#[test]
fn keyboard_dispatch_noop_without_dom_focus() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let ke = KeyEvent {
        key: KeyCode::A,
        state: KeyState::Pressed,
        modifiers: Modifiers::default(),
        scancode: 0,
        timestamp_us: 0,
    };
    // No DOM focus set → returns false, no panic.
    assert!(!shell.dispatch_dom_keyboard_event(&ke));
}

// ---------------------------------------------------------------------------
// 3. preventDefault gating
// ---------------------------------------------------------------------------

#[test]
fn prevent_default_listener_sets_flag() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let (_parent, child) = build_parent_child(&mut shell);

    // A preventable listener that always preventDefaults KeyDown.
    shell.add_preventable_event_handler(
        child,
        Some(DomEventKind::KeyDown {
            key: 0,
            modifiers: 0,
        }),
        false,
        |_e| true,
    );
    shell.set_dom_focus(Some(child));

    let ke = KeyEvent {
        key: KeyCode::A,
        state: KeyState::Pressed,
        modifiers: Modifiers::default(),
        scancode: 0,
        timestamp_us: 0,
    };
    assert!(
        shell.dispatch_dom_keyboard_event(&ke),
        "preventDefault listener should flip the shared flag"
    );
}

#[test]
fn prevent_default_suppresses_shell_shortcut() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let (_parent, child) = build_parent_child(&mut shell);

    // Super+L is bound to LockSession. A focused DOM listener that
    // preventDefaults must suppress the shortcut from the platform event path.
    shell.add_preventable_event_handler(
        child,
        Some(DomEventKind::KeyDown {
            key: 0,
            modifiers: 0,
        }),
        false,
        |_e| true,
    );
    shell.set_dom_focus(Some(child));

    let action = shell.handle_platform_event(&key_press(
        KeyCode::L,
        Modifiers::from_bits(Modifiers::SUPER),
    ));
    assert_eq!(
        action,
        Some(ShellAction::Redraw),
        "preventDefault should suppress LockSession and yield a plain Redraw"
    );
}

#[test]
fn no_prevent_default_lets_shortcut_through() {
    let mut shell = Shell::new(1920.0, 1080.0);
    // No DOM focus / no preventing listener → Super+L still locks.
    let action = shell.handle_platform_event(&key_press(
        KeyCode::L,
        Modifiers::from_bits(Modifiers::SUPER),
    ));
    assert_eq!(action, Some(ShellAction::LockSession));
}

// ---------------------------------------------------------------------------
// 4. The 14 previously-dead execute_action arms
// ---------------------------------------------------------------------------

#[test]
fn clipboard_history_toggles() {
    let mut shell = Shell::new(1920.0, 1080.0);
    assert!(!shell.clipboard_history_visible());
    assert!(shell.execute_action(&ShellAction::OpenClipboardHistory));
    assert!(shell.clipboard_history_visible());
    assert!(shell.execute_action(&ShellAction::OpenClipboardHistory));
    assert!(!shell.clipboard_history_visible());
}

#[test]
fn quick_settings_toggles() {
    let mut shell = Shell::new(1920.0, 1080.0);
    assert!(shell.execute_action(&ShellAction::OpenQuickSettings));
    assert!(shell.quick_settings_visible());
}

#[test]
fn screen_reader_toggles() {
    let mut shell = Shell::new(1920.0, 1080.0);
    assert!(shell.execute_action(&ShellAction::ToggleScreenReader));
    assert!(shell.screen_reader_enabled());
    assert!(shell.execute_action(&ShellAction::ToggleScreenReader));
    assert!(!shell.screen_reader_enabled());
}

#[test]
fn magnifier_and_zoom() {
    let mut shell = Shell::new(1920.0, 1080.0);
    assert!(shell.execute_action(&ShellAction::ToggleMagnifier));
    assert!(shell.magnifier_enabled());
    assert!(
        shell.zoom_level() > 1.0,
        "enabling magnifier seeds zoom > 1.0"
    );

    let before = shell.zoom_level();
    assert!(shell.execute_action(&ShellAction::ZoomIn));
    assert!(shell.zoom_level() > before, "ZoomIn increases zoom");

    // Zoom all the way out disengages the magnifier.
    for _ in 0..40 {
        shell.execute_action(&ShellAction::ZoomOut);
    }
    assert_eq!(shell.zoom_level(), 1.0);
    assert!(!shell.magnifier_enabled());
}

#[test]
fn title_bar_menu_opens_for_focused_window() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let _id = shell.open_app_window("com.liquide.terminal");
    assert!(shell.execute_action(&ShellAction::TitleBarMenu));
    assert!(
        shell.app_menu_open.is_some(),
        "TitleBarMenu should open the app menu for the focused window"
    );
}

#[test]
fn move_to_monitor_shifts_focused_window() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_app_window("com.liquide.terminal");
    let x0 = shell.window(id).unwrap().bounds.x;

    assert!(shell.execute_action(&ShellAction::MoveToMonitorRight));
    let x1 = shell.window(id).unwrap().bounds.x;
    assert!(
        (x1 - x0 - 1920.0).abs() < 0.5,
        "right move shifts +1 screen width"
    );

    assert!(shell.execute_action(&ShellAction::MoveToMonitorLeft));
    let x2 = shell.window(id).unwrap().bounds.x;
    assert!(
        (x2 - x0).abs() < 0.5,
        "left move returns to the original column"
    );
}

#[test]
fn move_to_monitor_handled_with_no_focus() {
    let mut shell = Shell::new(1920.0, 1080.0);
    // No window/focus: still acknowledged (never silently false).
    assert!(shell.execute_action(&ShellAction::MoveToMonitorRight));
}

#[test]
fn screenshot_arms_record_intent() {
    let cases = [
        (ShellAction::ScreenshotFull, ScreenshotRequest::Full),
        (ShellAction::ScreenshotWindow, ScreenshotRequest::Window),
        (ShellAction::ScreenshotRegion, ScreenshotRequest::Region),
        (
            ShellAction::ScreenshotToClipboard,
            ScreenshotRequest::ToClipboard,
        ),
    ];
    for (action, expected) in cases {
        let mut shell = Shell::new(1920.0, 1080.0);
        assert!(shell.execute_action(&action));
        assert_eq!(shell.pending_screenshot(), Some(expected));
        assert_eq!(shell.take_screenshot_request(), Some(expected));
        assert_eq!(shell.pending_screenshot(), None, "request is taken once");
    }
}

#[test]
fn screen_record_toggles_and_records_intent() {
    let mut shell = Shell::new(1920.0, 1080.0);
    assert!(shell.execute_action(&ShellAction::ScreenRecord));
    assert!(shell.screen_recording());
    assert_eq!(shell.pending_screenshot(), Some(ScreenshotRequest::Record));
    assert!(shell.execute_action(&ShellAction::ScreenRecord));
    assert!(
        !shell.screen_recording(),
        "second ScreenRecord stops recording"
    );
}

#[test]
fn no_action_arm_is_silently_dead() {
    // Spot-check that all 14 formerly-dead arms return true (handled).
    let actions = [
        ShellAction::TitleBarMenu,
        ShellAction::OpenClipboardHistory,
        ShellAction::OpenQuickSettings,
        ShellAction::ToggleScreenReader,
        ShellAction::ToggleMagnifier,
        ShellAction::ZoomIn,
        ShellAction::ZoomOut,
        ShellAction::MoveToMonitorLeft,
        ShellAction::MoveToMonitorRight,
        ShellAction::ScreenshotFull,
        ShellAction::ScreenshotWindow,
        ShellAction::ScreenshotRegion,
        ShellAction::ScreenshotToClipboard,
        ShellAction::ScreenRecord,
    ];
    for action in actions {
        let mut shell = Shell::new(1920.0, 1080.0);
        assert!(
            shell.execute_action(&action),
            "{action:?} must be handled (return true), not silently dead"
        );
    }
}

// ---------------------------------------------------------------------------
// 5. StyleResolver responsive-unit context
// ---------------------------------------------------------------------------

#[test]
fn style_resolver_context_tracks_viewport() {
    let mut shell = Shell::new(1280.0, 720.0);
    let ctx = shell.style_resolver().unwrap().context();
    assert_eq!(ctx.viewport_width, 1280.0);
    assert_eq!(ctx.viewport_height, 720.0);

    shell.resize_screen(800.0, 600.0);
    let ctx = shell.style_resolver().unwrap().context();
    assert_eq!(ctx.viewport_width, 800.0);
    assert_eq!(ctx.viewport_height, 600.0);
}
