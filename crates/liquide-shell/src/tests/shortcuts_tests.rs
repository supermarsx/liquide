use crate::shortcuts::*;
use liquide_input::{KeyCode, KeyEvent, KeyState, Modifiers};

// ---------------------------------------------------------------------------
// ShortcutManager::new() — default bindings
// ---------------------------------------------------------------------------

#[test]
fn shortcut_default_has_bindings() {
    let mgr = ShortcutManager::new();
    assert!(
        mgr.binding_count() > 0,
        "default manager should have bindings"
    );
}

// ---------------------------------------------------------------------------
// Verify specific default bindings
// ---------------------------------------------------------------------------

#[test]
fn shortcut_default_lock_session() {
    let mgr = ShortcutManager::new();
    let binding = KeyBinding::new(KeyCode::L, Modifiers::from_bits(Modifiers::SUPER));
    assert_eq!(
        mgr.lookup(&binding),
        Some(&ShellAction::LockSession),
        "Super+L should map to LockSession"
    );
}

#[test]
fn shortcut_default_close_window() {
    let mgr = ShortcutManager::new();
    let binding = KeyBinding::new(KeyCode::F4, Modifiers::from_bits(Modifiers::ALT));
    assert_eq!(
        mgr.lookup(&binding),
        Some(&ShellAction::CloseWindow),
        "Alt+F4 should map to CloseWindow"
    );
}

#[test]
fn shortcut_default_open_file_manager() {
    let mgr = ShortcutManager::new();
    let binding = KeyBinding::new(KeyCode::E, Modifiers::from_bits(Modifiers::SUPER));
    assert_eq!(
        mgr.lookup(&binding),
        Some(&ShellAction::OpenFileManager),
        "Super+E should map to OpenFileManager"
    );
}

#[test]
fn shortcut_default_open_terminal() {
    let mgr = ShortcutManager::new();
    let binding = KeyBinding::new(KeyCode::T, Modifiers::from_bits(Modifiers::SUPER));
    assert_eq!(
        mgr.lookup(&binding),
        Some(&ShellAction::OpenTerminal),
        "Super+T should map to OpenTerminal"
    );
}

#[test]
fn shortcut_default_open_task_manager() {
    let mgr = ShortcutManager::new();
    let binding = KeyBinding::new(
        KeyCode::Escape,
        Modifiers::from_bits(Modifiers::CTRL | Modifiers::SHIFT),
    );
    assert_eq!(
        mgr.lookup(&binding),
        Some(&ShellAction::OpenTaskManager),
        "Ctrl+Shift+Escape should map to OpenTaskManager"
    );
}

#[test]
fn shortcut_default_show_desktop() {
    let mgr = ShortcutManager::new();
    let binding = KeyBinding::new(KeyCode::D, Modifiers::from_bits(Modifiers::SUPER));
    assert_eq!(
        mgr.lookup(&binding),
        Some(&ShellAction::ShowDesktop),
        "Super+D should map to ShowDesktop"
    );
}

#[test]
fn shortcut_default_screenshot_full() {
    let mgr = ShortcutManager::new();
    let binding = KeyBinding::new(KeyCode::PrintScreen, Modifiers::from_bits(0));
    assert_eq!(
        mgr.lookup(&binding),
        Some(&ShellAction::ScreenshotFull),
        "PrintScreen should map to ScreenshotFull"
    );
}

// ---------------------------------------------------------------------------
// bind() — new key returns None
// ---------------------------------------------------------------------------

#[test]
fn shortcut_bind_new_key_returns_none() {
    let mut mgr = ShortcutManager::new();
    let binding = KeyBinding::new(KeyCode::F12, Modifiers::from_bits(Modifiers::CTRL));
    let prev = mgr.bind(binding.clone(), ShellAction::OpenLauncher);
    assert!(
        prev.is_none(),
        "binding to a previously unbound key should return None"
    );
    assert_eq!(mgr.lookup(&binding), Some(&ShellAction::OpenLauncher));
}

// ---------------------------------------------------------------------------
// bind() — over existing key returns displaced action
// ---------------------------------------------------------------------------

#[test]
fn shortcut_bind_existing_key_returns_displaced() {
    let mut mgr = ShortcutManager::new();
    let binding = KeyBinding::new(KeyCode::L, Modifiers::from_bits(Modifiers::SUPER));
    // Super+L is already bound to LockSession
    let prev = mgr.bind(binding.clone(), ShellAction::OpenSettings);
    assert_eq!(
        prev,
        Some(ShellAction::LockSession),
        "displaced action should be LockSession"
    );
    assert_eq!(mgr.lookup(&binding), Some(&ShellAction::OpenSettings));
}

// ---------------------------------------------------------------------------
// unbind()
// ---------------------------------------------------------------------------

#[test]
fn shortcut_unbind_removes_binding() {
    let mut mgr = ShortcutManager::new();
    let binding = KeyBinding::new(KeyCode::L, Modifiers::from_bits(Modifiers::SUPER));
    let removed = mgr.unbind(&binding);
    assert_eq!(
        removed,
        Some(ShellAction::LockSession),
        "unbind should return the removed action"
    );
    assert!(
        mgr.lookup(&binding).is_none(),
        "lookup after unbind should return None"
    );
}

// ---------------------------------------------------------------------------
// binding_for() — reverse lookup
// ---------------------------------------------------------------------------

#[test]
fn shortcut_binding_for_reverse_lookup() {
    let mgr = ShortcutManager::new();
    let kb = mgr.binding_for(&ShellAction::CloseWindow);
    assert!(kb.is_some(), "CloseWindow should have a binding");
    let kb = kb.unwrap();
    assert_eq!(kb.key, KeyCode::F4);
    assert!(kb.modifiers.alt());
}

#[test]
fn shortcut_binding_for_unknown_action() {
    let mgr = ShortcutManager::new();
    // SwitchToWorkspace(99) is not bound by default
    let kb = mgr.binding_for(&ShellAction::SwitchToWorkspace(99));
    assert!(
        kb.is_none(),
        "SwitchToWorkspace(99) should not have a binding"
    );
}

// ---------------------------------------------------------------------------
// conflicts()
// ---------------------------------------------------------------------------

#[test]
fn shortcut_conflicts_bound_key() {
    let mgr = ShortcutManager::new();
    let binding = KeyBinding::new(KeyCode::L, Modifiers::from_bits(Modifiers::SUPER));
    assert!(
        mgr.conflicts(&binding),
        "Super+L should conflict (already bound)"
    );
}

#[test]
fn shortcut_conflicts_unbound_key() {
    let mgr = ShortcutManager::new();
    let binding = KeyBinding::new(KeyCode::F12, Modifiers::from_bits(Modifiers::CTRL));
    assert!(
        !mgr.conflicts(&binding),
        "Ctrl+F12 should not conflict (unbound)"
    );
}

// ---------------------------------------------------------------------------
// handle_key_event()
// ---------------------------------------------------------------------------

#[test]
fn shortcut_handle_key_event_pressed_match() {
    let mgr = ShortcutManager::new();
    let event = KeyEvent::new(
        KeyCode::L,
        KeyState::Pressed,
        Modifiers::from_bits(Modifiers::SUPER),
        0,
        0,
    );
    assert_eq!(
        mgr.handle_key_event(&event),
        Some(&ShellAction::LockSession),
        "pressed Super+L should return LockSession"
    );
}

#[test]
fn shortcut_handle_key_event_released_returns_none() {
    let mgr = ShortcutManager::new();
    let event = KeyEvent::new(
        KeyCode::L,
        KeyState::Released,
        Modifiers::from_bits(Modifiers::SUPER),
        0,
        0,
    );
    assert!(
        mgr.handle_key_event(&event).is_none(),
        "released events should return None"
    );
}

#[test]
fn shortcut_handle_key_event_unbound_returns_none() {
    let mgr = ShortcutManager::new();
    let event = KeyEvent::new(
        KeyCode::F12,
        KeyState::Pressed,
        Modifiers::from_bits(Modifiers::CTRL),
        0,
        0,
    );
    assert!(
        mgr.handle_key_event(&event).is_none(),
        "unbound key press should return None"
    );
}

#[test]
fn shortcut_handle_key_event_repeat_returns_none() {
    let mgr = ShortcutManager::new();
    let event = KeyEvent::new(
        KeyCode::L,
        KeyState::Repeat,
        Modifiers::from_bits(Modifiers::SUPER),
        0,
        0,
    );
    assert!(
        mgr.handle_key_event(&event).is_none(),
        "repeat events should return None"
    );
}

// ---------------------------------------------------------------------------
// all_bindings()
// ---------------------------------------------------------------------------

#[test]
fn shortcut_all_bindings_returns_hashmap() {
    let mgr = ShortcutManager::new();
    let bindings = mgr.all_bindings();
    assert_eq!(
        bindings.len(),
        mgr.binding_count(),
        "all_bindings length should match binding_count"
    );
    assert!(bindings.len() > 0);
}

// ---------------------------------------------------------------------------
// Display impls
// ---------------------------------------------------------------------------

#[test]
fn shortcut_key_binding_display() {
    let kb = KeyBinding::new(KeyCode::L, Modifiers::from_bits(Modifiers::SUPER));
    let display = format!("{kb}");
    assert!(
        display.contains("Super"),
        "display should contain Super: {display}"
    );
    assert!(display.contains("L"), "display should contain L: {display}");
}

#[test]
fn shortcut_key_binding_display_multi_modifiers() {
    let kb = KeyBinding::new(
        KeyCode::Delete,
        Modifiers::from_bits(Modifiers::CTRL | Modifiers::ALT),
    );
    let display = format!("{kb}");
    assert!(
        display.contains("Ctrl"),
        "display should contain Ctrl: {display}"
    );
    assert!(
        display.contains("Alt"),
        "display should contain Alt: {display}"
    );
}

#[test]
fn shortcut_shell_action_display_lock_session() {
    let action = ShellAction::LockSession;
    assert_eq!(format!("{action}"), "Lock Session");
}

#[test]
fn shortcut_shell_action_display_close_window() {
    let action = ShellAction::CloseWindow;
    assert_eq!(format!("{action}"), "Close Window");
}

#[test]
fn shortcut_shell_action_display_launch_dock_app() {
    let action = ShellAction::LaunchDockApp(3);
    assert_eq!(format!("{action}"), "Launch Dock App 3");
}

#[test]
fn shortcut_shell_action_display_switch_to_workspace() {
    let action = ShellAction::SwitchToWorkspace(2);
    assert_eq!(format!("{action}"), "Switch to Workspace 2");
}

#[test]
fn shortcut_direction_display() {
    assert_eq!(format!("{}", Direction::Left), "Left");
    assert_eq!(format!("{}", Direction::Right), "Right");
    assert_eq!(format!("{}", Direction::Up), "Up");
    assert_eq!(format!("{}", Direction::Down), "Down");
}

// ---------------------------------------------------------------------------
// Dock app shortcuts — Super+1 through Super+9
// ---------------------------------------------------------------------------

#[test]
fn shortcut_dock_app_super_1() {
    let mgr = ShortcutManager::new();
    let binding = KeyBinding::new(KeyCode::Digit1, Modifiers::from_bits(Modifiers::SUPER));
    assert_eq!(
        mgr.lookup(&binding),
        Some(&ShellAction::LaunchDockApp(1)),
        "Super+1 should map to LaunchDockApp(1)"
    );
}

#[test]
fn shortcut_dock_app_super_5() {
    let mgr = ShortcutManager::new();
    let binding = KeyBinding::new(KeyCode::Digit5, Modifiers::from_bits(Modifiers::SUPER));
    assert_eq!(
        mgr.lookup(&binding),
        Some(&ShellAction::LaunchDockApp(5)),
        "Super+5 should map to LaunchDockApp(5)"
    );
}

#[test]
fn shortcut_dock_app_super_9() {
    let mgr = ShortcutManager::new();
    let binding = KeyBinding::new(KeyCode::Digit9, Modifiers::from_bits(Modifiers::SUPER));
    assert_eq!(
        mgr.lookup(&binding),
        Some(&ShellAction::LaunchDockApp(9)),
        "Super+9 should map to LaunchDockApp(9)"
    );
}

#[test]
fn shortcut_dock_app_all_digits() {
    let mgr = ShortcutManager::new();
    let digit_keys = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
    ];
    for (i, key) in digit_keys.iter().enumerate() {
        let slot = (i + 1) as u32;
        let binding = KeyBinding::new(*key, Modifiers::from_bits(Modifiers::SUPER));
        assert_eq!(
            mgr.lookup(&binding),
            Some(&ShellAction::LaunchDockApp(slot)),
            "Super+{slot} should map to LaunchDockApp({slot})"
        );
    }
}
