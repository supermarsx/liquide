//! t75-shellact: multi-monitor MoveToMonitor, IME drive, and hotkey-fold tests.

use crate::shell::Shell;
use crate::shortcuts::{ShellAction, ShortcutManager};
use liquide_compositor::geometry::Rect;
use liquide_display::display::{DisplayInfo, Resolution, Rotation};
use liquide_display::DesktopLayout;
use liquide_input::{KeyCode, KeyEvent, KeyState, Modifiers};
use liquide_platform::{NativeWindowHandle, PlatformEvent};

// ── helpers ────────────────────────────────────────────────────────────────

fn make_display(id: u32, w: u32, h: u32, x: i32, y: i32, primary: bool) -> DisplayInfo {
    DisplayInfo {
        id,
        name: format!("M{id}"),
        connector: format!("DP-{id}"),
        resolution: Resolution::new(w, h),
        available_resolutions: vec![Resolution::new(w, h)],
        refresh_rate: 60.0,
        available_refresh_rates: vec![60.0],
        position: (x, y),
        rotation: Rotation::Normal,
        scale: 1.0,
        primary,
        enabled: true,
        physical_size_mm: None,
        connected: true,
    }
}

fn dual_layout() -> DesktopLayout {
    DesktopLayout::new(vec![
        make_display(1, 1920, 1080, 0, 0, true),
        make_display(2, 1920, 1080, 1920, 0, false),
    ])
}

fn single_layout() -> DesktopLayout {
    DesktopLayout::new(vec![make_display(1, 1920, 1080, 0, 0, true)])
}

fn key(k: KeyCode, mods: u8) -> PlatformEvent {
    PlatformEvent::KeyInput {
        handle: NativeWindowHandle(0),
        event: KeyEvent {
            key: k,
            state: KeyState::Pressed,
            modifiers: Modifiers::from_bits(mods),
            scancode: 0,
            timestamp_us: 0,
        },
    }
}

// ── multi-monitor: MoveToMonitor ─────────────────────────────────────────────

#[test]
fn move_to_monitor_moves_window_to_other_monitor_when_present() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.set_desktop_layout(dual_layout());

    // Open a window on monitor 1 (left half) and focus it.
    let id = shell.open_window("win", Rect::new(100.0, 100.0, 400.0, 300.0));
    let _ = shell.set_focus(id);
    assert_eq!(shell.window_monitor(id), Some(1), "spawned on monitor 1");

    // MoveToMonitorRight should relocate it onto monitor 2 (x >= 1920).
    assert!(shell.execute_action(&ShellAction::MoveToMonitorRight));
    assert_eq!(shell.window_monitor(id), Some(2), "now on monitor 2");
    let b = shell.window(id).unwrap().bounds;
    assert!(b.x >= 1920.0, "window x={} should be on monitor 2", b.x);

    // And back to monitor 1 with MoveToMonitorLeft.
    assert!(shell.execute_action(&ShellAction::MoveToMonitorLeft));
    assert_eq!(shell.window_monitor(id), Some(1), "back on monitor 1");
    let b = shell.window(id).unwrap().bounds;
    assert!(b.x < 1920.0, "window x={} should be back on monitor 1", b.x);
}

#[test]
fn move_to_monitor_is_noop_on_single_monitor() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.set_desktop_layout(single_layout());

    let id = shell.open_window("win", Rect::new(100.0, 100.0, 400.0, 300.0));
    let _ = shell.set_focus(id);
    let before = shell.window(id).unwrap().bounds;

    // Acknowledged but no movement (next_monitor → None on a single monitor).
    assert!(shell.execute_action(&ShellAction::MoveToMonitorRight));
    let after = shell.window(id).unwrap().bounds;
    assert_eq!(before.x, after.x, "single monitor must not move the window");
    assert_eq!(before.y, after.y);
    assert_eq!(shell.window_monitor(id), Some(1));
}

#[test]
fn move_to_monitor_falls_back_to_proxy_without_layout() {
    // No layout installed → legacy single-screen shift proxy (unchanged).
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("win", Rect::new(100.0, 100.0, 400.0, 300.0));
    let _ = shell.set_focus(id);
    assert_eq!(shell.window_monitor(id), None, "no layout → no assignment");

    let before = shell.window(id).unwrap().bounds.x;
    assert!(shell.execute_action(&ShellAction::MoveToMonitorRight));
    let after = shell.window(id).unwrap().bounds.x;
    assert!(
        (after - before - 1920.0).abs() < 1.0,
        "proxy shifts by one screen width: {before} -> {after}"
    );
}

#[test]
fn chrome_insets_reserved_into_primary_work_area() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.set_desktop_layout(dual_layout());
    // The primary monitor's work area should be smaller than its full bounds
    // because the shell reserved its top status bar (and dock).
    let layout = shell.desktop_layout().unwrap();
    let wa = layout.work_area_of(1).unwrap();
    assert!(wa.height < 1080, "top panel/dock must be reserved: {wa:?}");
    assert!(wa.y > 0, "work area should start below the status bar");
}

// ── IME drive ────────────────────────────────────────────────────────────────

#[test]
fn ime_sequence_commits_text() {
    let mut shell = Shell::new(1280.0, 720.0);
    let id = shell.open_window("win", Rect::new(50.0, 50.0, 400.0, 300.0));
    let _ = shell.set_focus(id);

    // ASCII default: IME inactive.
    assert!(!shell.ime_active());

    // Activate the IME (Ctrl+Space via the real keyboard path) and switch it to
    // Hiragana mode (a host/settings action). Then type the romaji "ka" — the
    // engine converts it to the preedit か — and commit with Enter. Everything
    // after activation flows through the real keyboard path, proving the shell
    // drives the IME and routes its committed text into the focused window.
    shell.handle_platform_event(&key(KeyCode::Space, Modifiers::CTRL));
    assert!(shell.ime_active(), "Ctrl+Space activates the IME");
    shell
        .input_method_mut()
        .set_mode(liquide_input_method::InputMode::Hiragana);

    shell.handle_platform_event(&key(KeyCode::K, 0));
    shell.handle_platform_event(&key(KeyCode::A, 0));
    // Mid-composition: a preedit exists and nothing is committed to the window.
    assert!(!shell.ime_preedit().is_empty(), "preedit should show か");
    assert_eq!(shell.window_text_input(id), None, "not committed yet");

    // Enter commits the composed kana into the focused window.
    shell.handle_platform_event(&key(KeyCode::Enter, 0));
    let typed = shell.window_text_input(id).unwrap_or("");
    assert_eq!(
        typed, "\u{304b}",
        "IME romaji sequence must commit 'か', got {typed:?}"
    );
    assert!(shell.ime_preedit().is_empty(), "preedit cleared after commit");
}

#[test]
fn ascii_input_unaffected_when_ime_inactive() {
    // Regression guard: with the IME inactive (default), a plain key still routes
    // to the focused window's text buffer exactly as before the IME wire.
    let mut shell = Shell::new(1280.0, 720.0);
    let id = shell.open_window("win", Rect::new(50.0, 50.0, 400.0, 300.0));
    let _ = shell.set_focus(id);

    shell.handle_platform_event(&key(KeyCode::H, 0));
    shell.handle_platform_event(&key(KeyCode::I, 0));
    assert_eq!(shell.window_text_input(id), Some("hi"));
}

// ── hotkeys fold ─────────────────────────────────────────────────────────────

#[test]
fn custom_hotkey_binding_fires() {
    let mut mgr = ShortcutManager::new();
    // Bind a custom combination via the folded-in hotkeys grammar.
    let displaced = mgr.bind_from_str("Ctrl+Alt+G", ShellAction::OpenSettings);
    assert!(displaced.is_none(), "fresh binding displaces nothing");

    // The custom binding fires for the matching key event.
    let ev = KeyEvent {
        key: KeyCode::G,
        state: KeyState::Pressed,
        modifiers: Modifiers::from_bits(Modifiers::CTRL | Modifiers::ALT),
        scancode: 0,
        timestamp_us: 0,
    };
    assert_eq!(
        mgr.handle_key_event(&ev),
        Some(&ShellAction::OpenSettings),
        "custom Ctrl+Alt+G must fire OpenSettings"
    );
}

#[test]
fn hotkey_defaults_import_maps_actions() {
    let mut mgr = ShortcutManager::new();
    let imported = mgr.import_hotkey_defaults();
    assert!(imported > 0, "should import several hotkey defaults");

    // Super+Space → ShowLauncher → OpenLauncher (hotkeys default differs from the
    // shell's built-in Super-only launcher chord, proving the fold took effect).
    let ev = KeyEvent {
        key: KeyCode::Space,
        state: KeyState::Pressed,
        modifiers: Modifiers::from_bits(Modifiers::SUPER),
        scancode: 0,
        timestamp_us: 0,
    };
    assert_eq!(
        mgr.handle_key_event(&ev),
        Some(&ShellAction::OpenLauncher),
        "imported Super+Space must map to OpenLauncher"
    );
}

#[test]
fn hotkey_action_map_is_complete_for_supported_actions() {
    use liquide_hotkeys::HotkeyAction as HA;
    // Sanity: the mapped actions resolve; unmapped (media/volume) are skipped.
    let mut mgr = ShortcutManager::new();
    let n = mgr.import_hotkey_bindings(vec![
        (
            liquide_hotkeys::KeyBinding::new(liquide_hotkeys::Modifiers::SUPER, liquide_hotkeys::Key::D),
            HA::ShowDesktop,
        ),
        (
            liquide_hotkeys::KeyBinding::new(liquide_hotkeys::Modifiers::NONE, liquide_hotkeys::Key::VolumeUp),
            HA::VolumeUp, // unmapped → skipped
        ),
    ]);
    assert_eq!(n, 1, "only the mappable ShowDesktop binding is imported");
}
