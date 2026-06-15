//! STRICT, adversarial end-to-end NAVIGATION suite (t66-nav).
//!
//! PRIME DIRECTIVE: encode what CORRECT keyboard/pointer navigation MUST do at
//! the STATE (via `&mut Shell` readback) and PIXEL level, run it, and let the
//! failures stand as findings. "Navigation" = moving through the UI via the
//! keyboard / pointer. A nav path that merely *exists* must not pass — the
//! highlight has to actually advance/wrap, Enter has to activate the *highlighted*
//! item, focus has to actually move, the active workspace has to actually change
//! and round-trip.
//!
//! ## Driving model (mirrors `interaction_e2e.rs` / `e2e_context_menu.rs`)
//!
//! `capture_desktop_scripted_readback` runs the scripted [`PlatformEvent`]s, then
//! runs a readback closure against the live `&mut Shell` and returns its value
//! with the post-event [`Frame`]. Several nav flows here drive the activating key
//! sequence INSIDE the readback closure via the SAME `handle_platform_event` the
//! platform path uses (the closure is the only point where we hold `&mut Shell`
//! and can both open an overlay AND feed it nav keys, then read state back). Only
//! the *capture* of those keys is bypassed; key handling, key-nav, and action
//! dispatch all go through the real live handler.
//!
//! ## What state is publicly observable (a reported seam gap)
//!
//! The shell exposes public read accessors for: launcher selection
//! (`launcher().selected_index()` / `result_count()` / `activate_selected()`),
//! the focused window (`focus_manager().focused()`), the active workspace
//! (`workspace_manager().active().id.0`), `window_count()`, and
//! `session_menu_visible()` / `pending_session_request()`.
//!
//! It does NOT expose the highlighted index of the context / session / app menus
//! (`*_hover_index` are `pub(crate)` on `Shell` — see
//! `crates/liquide-shell/src/shell/mod.rs:316-318`). So for *menu keyboard
//! highlight* navigation we assert the highlight INDIRECTLY but RIGOROUSLY: after
//! N arrow presses, `Enter` must activate the item the highlight should be on, and
//! the resulting action (a recorded session request / an opened window) proves
//! which item was highlighted. Wrong highlight movement -> wrong action -> FAIL.
//! **SEAM REQUEST: add `Shell::context_menu_hover_index()` /
//! `session_menu_hover_index()` / `app_menu_hover_index()` read accessors so the
//! highlight index can be asserted directly.**

use liquide_input::keyboard::{KeyCode, KeyEvent, KeyState, Modifiers};
use liquide_platform::{NativeWindowHandle, PlatformEvent};
use liquide_shell::{SessionRequest, ShellAction};
use liquide_visual_test::scenarios::{
    ScriptedScenario, scenario_options, themed_desktop_capture,
};
use liquide_visual_test::{Frame, capture_desktop_scripted_readback};

const THEME: &str = "liquid-glass";

// ───────────────────────────────────────────────────────────────────────────
// Key-event helpers (drive the REAL live handler inside the readback closure)
// ───────────────────────────────────────────────────────────────────────────

/// A bare key press with no modifiers, as a `PlatformEvent` for the live handler.
fn key(code: KeyCode) -> PlatformEvent {
    PlatformEvent::KeyInput {
        handle: NativeWindowHandle(1),
        event: KeyEvent::new(code, KeyState::Pressed, Modifiers::new(), 0, 0),
    }
}

/// A modified key press as a `PlatformEvent`.
fn key_mod(code: KeyCode, modifiers: Modifiers) -> PlatformEvent {
    PlatformEvent::KeyInput {
        handle: NativeWindowHandle(1),
        event: KeyEvent::new(code, KeyState::Pressed, modifiers, 0, 0),
    }
}

/// A no-interaction base desktop for differential pixel probes.
fn base_desktop() -> Frame {
    themed_desktop_capture(THEME).expect("base desktop capture")
}

/// Count pixels in `frame`'s rect that DIFFER from the same rect in `base`
/// (max-channel delta > `tol`).
fn changed_vs_base(frame: &Frame, base: &Frame, x: u32, y: u32, w: u32, h: u32, tol: u8) -> usize {
    let mut n = 0usize;
    let x1 = (x + w).min(frame.width).min(base.width);
    let y1 = (y + h).min(frame.height).min(base.height);
    for py in y..y1 {
        for px in x..x1 {
            let a = frame.pixel(px, py).unwrap();
            let b = base.pixel(px, py).unwrap();
            let d = a
                .iter()
                .zip(b.iter())
                .map(|(&p, &q)| p.abs_diff(q))
                .max()
                .unwrap_or(0);
            if d > tol {
                n += 1;
            }
        }
    }
    n
}

// ===========================================================================
// 1. MENU KEYBOARD NAV — SESSION MENU
//    Down/Up advance the highlight (wrapping at ends), Enter activates the
//    HIGHLIGHTED item, Esc closes. Highlight state is asserted INDIRECTLY but
//    strictly: a given number of arrow presses must land Enter on a specific
//    item, proven by the recorded session request. Wrong movement -> wrong
//    request -> FAIL.
//
//    Session menu items (SessionMenuItem::defaults), top to bottom:
//      0: Lock    -> LockSession (no SessionRequest recorded)
//      1: Log Out -> LogOut
//      2: Restart -> Restart
//      3: Shut Down -> Shutdown
//    First ArrowDown moves the (initially None) highlight to index 0
//    (cycle_menu_index(None, len, +1) == 0).
// ===========================================================================

/// Drive `arrows` ArrowDown presses then Enter on a freshly-opened session menu;
/// return the recorded `pending_session_request()` and whether the menu closed.
fn session_menu_down_then_enter(arrows: usize) -> (Option<SessionRequest>, bool) {
    capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |_handle| Vec::new(),
        move |shell| {
            shell.toggle_session_menu();
            assert!(shell.session_menu_visible(), "precondition: session menu open");
            for _ in 0..arrows {
                shell.handle_platform_event(&key(KeyCode::ArrowDown));
            }
            let action = shell.handle_platform_event(&key(KeyCode::Enter));
            if let Some(a) = action {
                shell.execute_action(&a);
            }
            (shell.pending_session_request(), shell.session_menu_visible())
        },
    )
    .expect("session-menu key-nav capture")
    .1
}

#[test]
fn session_menu_arrow_down_advances_highlight_and_enter_activates() {
    // 1 ArrowDown -> highlight on item 0 (Lock); Enter -> LockSession.
    // LockSession does NOT record a SessionRequest (it locks the session), so we
    // assert the menu CLOSED (dismiss-on-activate) and that it did NOT mis-fire a
    // LogOut/Restart/Shutdown request.
    let (req1, open1) = session_menu_down_then_enter(1);
    assert!(!open1, "session menu must dismiss after Enter activates an item");
    assert_eq!(
        req1, None,
        "1 ArrowDown should highlight item 0 (Lock); Enter fired a session request \
         {req1:?} — the highlight advanced past item 0 (Lock records no request). Wrong \
         arrow-step distance."
    );

    // 2 ArrowDown -> highlight on item 1 (Log Out); Enter -> LogOut recorded.
    let (req2, _open2) = session_menu_down_then_enter(2);
    assert_eq!(
        req2,
        Some(SessionRequest::LogOut),
        "2 ArrowDown should land the highlight on item 1 (Log Out) and Enter must activate \
         the HIGHLIGHTED item (LogOut). Got {req2:?}. Either ArrowDown does not advance the \
         session-menu highlight or Enter ignores it. Check events.rs session-menu ArrowDown / \
         Enter arms."
    );

    // 3 ArrowDown -> item 2 (Restart).
    let (req3, _) = session_menu_down_then_enter(3);
    assert_eq!(
        req3,
        Some(SessionRequest::Restart),
        "3 ArrowDown should highlight item 2 (Restart); Enter must fire Restart, got {req3:?}."
    );
}

#[test]
fn session_menu_highlight_wraps_at_bottom() {
    // 4 items. From None, ArrowDown x4 = item3 (Shut Down). A 5th ArrowDown MUST
    // WRAP to item 0 (Lock) — not clamp at item 3. So 5 downs + Enter activates
    // item 0 (Lock -> no SessionRequest, menu closes). If wrapping is broken
    // (clamped at the bottom), the 5th down stays on item 3 and Enter would fire
    // Shutdown — caught here.
    let (req, open) = session_menu_down_then_enter(5);
    assert!(!open, "menu must close after Enter");
    assert_eq!(
        req, None,
        "ArrowDown past the last session-menu item must WRAP to item 0 (Lock, no request); \
         got {req:?}. A Shutdown/Restart/LogOut here means the highlight CLAMPED at the bottom \
         instead of wrapping. Check cycle_menu_index wrap (events.rs)."
    );
}

#[test]
fn session_menu_arrow_up_from_top_wraps_to_bottom() {
    // A single ArrowUp from a freshly-opened menu (highlight None) must select the
    // LAST item (cycle_menu_index(None, len, -1) == len-1 == item 3 Shut Down).
    // Enter then fires Shutdown. If ArrowUp is unwired or mis-wraps, this fails.
    let (req, open) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |_handle| Vec::new(),
        |shell| {
            shell.toggle_session_menu();
            assert!(shell.session_menu_visible(), "precondition: menu open");
            shell.handle_platform_event(&key(KeyCode::ArrowUp));
            let action = shell.handle_platform_event(&key(KeyCode::Enter));
            if let Some(a) = action {
                shell.execute_action(&a);
            }
            (shell.pending_session_request(), shell.session_menu_visible())
        },
    )
    .expect("session-menu arrow-up capture")
    .1;

    assert!(!open, "menu must close after Enter");
    assert_eq!(
        req,
        Some(SessionRequest::Shutdown),
        "ArrowUp from the top of the session menu must wrap to the LAST item (Shut Down -> \
         Shutdown). Got {req:?}. ArrowUp is unwired or wraps wrongly."
    );
}

#[test]
fn session_menu_escape_closes_without_activating() {
    // Esc must close the menu and fire NO action.
    let (req, open) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |_handle| Vec::new(),
        |shell| {
            shell.toggle_session_menu();
            assert!(shell.session_menu_visible(), "precondition: menu open");
            shell.handle_platform_event(&key(KeyCode::ArrowDown)); // highlight item 0
            let esc_action = shell.handle_platform_event(&key(KeyCode::Escape));
            if let Some(a) = esc_action {
                shell.execute_action(&a);
            }
            (shell.pending_session_request(), shell.session_menu_visible())
        },
    )
    .expect("session-menu escape capture")
    .1;

    assert!(
        !open,
        "Escape must close the session menu (session_menu_visible() should be false)."
    );
    assert_eq!(
        req, None,
        "Escape must NOT activate any item; a session request {req:?} was recorded — Escape \
         leaked an activation."
    );
}

// ===========================================================================
// 2. MENU KEYBOARD NAV — CONTEXT MENU (desktop right-click menu)
//    The desktop context menu MUST be keyboard-navigable: ArrowDown/Up move the
//    highlight, Enter activates the highlighted item. This is the SAME contract
//    the session menu satisfies. The desktop context menu's first item is
//    "Open Terminal" (ShellAction::OpenTerminal -> opens a window).
//
//    ADVERSARIAL EXPECTATION: a correctly-navigable context menu, opened then
//    driven ArrowDown -> Enter, activates an item and opens a window
//    (window_count == 1). If the context menu has NO keyboard nav handler (only
//    Escape is wired in events.rs), ArrowDown/Enter are swallowed/ignored, no
//    window opens, and this FAILS — exposing the missing nav.
// ===========================================================================

#[test]
fn context_menu_keyboard_arrow_enter_activates_item() {
    let (rx, ry) = (300.0_f32, 250.0_f32);

    let (_frame, window_count) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        // Open the context menu via a real right-click on the empty desktop.
        |handle| ScriptedScenario::new(handle).right_click(rx, ry).into_events(),
        |shell| {
            // The right-click already opened the context menu. Navigate it with
            // the keyboard: ArrowDown to highlight the first item, then Enter to
            // activate it.
            shell.handle_platform_event(&key(KeyCode::ArrowDown));
            let action = shell.handle_platform_event(&key(KeyCode::Enter));
            if let Some(a) = action {
                shell.execute_action(&a);
            }
            shell.window_count()
        },
    )
    .expect("context-menu key-nav capture");

    assert_eq!(
        window_count, 1,
        "context menu is NOT keyboard-navigable: after a right-click opened it, ArrowDown+Enter \
         did not activate any item (window_count={window_count}, expected the highlighted item to \
         fire and open a window). events.rs handles ONLY Escape for the context menu \
         (crates/liquide-shell/src/shell/events.rs:482-486) — no ArrowDown/ArrowUp/Enter arms — so \
         the desktop context menu cannot be operated by keyboard. FIX in liquide-shell \
         (events.rs handle_platform_event: add context-menu ArrowDown/ArrowUp/Enter arms mirroring \
         the session-menu arms at events.rs:487-521)."
    );
}

#[test]
fn context_menu_keyboard_escape_closes() {
    // Escape IS wired for the context menu; this is the PASSING control that
    // proves the menu was actually open and the harness drives it. The menu rect
    // top-left == click point (no clamping at this position).
    let (rx, ry) = (300.0_f32, 250.0_f32);
    let base = base_desktop();

    let (frame, ()) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |handle| {
            ScriptedScenario::new(handle)
                .right_click(rx, ry)
                .hotkey(KeyCode::Escape, Modifiers::new())
                .into_events()
        },
        |_shell| (),
    )
    .expect("context-menu escape capture");

    // Menu geometry: 200 wide, 5 items * 28 + 2*4 padding = 148 tall.
    let still_painted = changed_vs_base(&frame, &base, rx as u32, ry as u32, 200, 148, 24);
    let area = 200 * 148usize;
    assert!(
        still_painted < area / 6,
        "context menu was NOT dismissed by Escape: {still_painted}/{area} pixels still differ \
         from the bare desktop (expected the menu gone, < 1/6)."
    );
}

// ===========================================================================
// 3. MENU KEYBOARD NAV — APP MENU (title-bar window menu)
//    Right-clicking a window title bar opens the app menu (Minimize/Maximize/
//    Close/Settings/About). It MUST be keyboard navigable: it opens with the
//    highlight on item 0, ArrowDown moves it, Enter activates the highlighted
//    item. Item 1 is Maximize -> the focused window becomes Maximized.
//
//    Opens with hover index Some(0) (events.rs:1019). So from open:
//      Enter            -> item 0 (Minimize)
//      ArrowDown, Enter -> item 1 (Maximize)
// ===========================================================================

#[test]
fn app_menu_keyboard_arrow_enter_activates_maximize() {
    use liquide_input::mouse::MouseButton;
    use liquide_shell::WindowState;

    // Open a window via the first dock item, learn its title-bar location, then in
    // a second capture open the app menu on its title bar and drive it by keyboard.
    let (dock_cx, dock_cy) = {
        let (_f, c) = capture_desktop_scripted_readback(
            &scenario_options(THEME),
            |_h| Vec::new(),
            |shell| {
                let screen = shell.screen_rect();
                let rects = shell.dock().compute_item_rects(screen);
                let (_, rect) = rects.first().copied().expect("dock has an item");
                (rect.x + rect.width / 2.0, rect.y + rect.height / 2.0)
            },
        )
        .expect("dock geometry");
        c
    };

    let initial = {
        let (_f, b) = capture_desktop_scripted_readback(
            &scenario_options(THEME),
            |handle| ScriptedScenario::new(handle).left_click(dock_cx, dock_cy).into_events(),
            |shell| {
                shell
                    .visible_windows()
                    .first()
                    .map(|w| w.bounds)
                    .expect("a window is open after the dock click")
            },
        )
        .expect("open-window probe");
        b
    };

    // Title-bar point: inside the top edge, left of the decoration buttons.
    let tb_x = initial.x + 40.0;
    let tb_y = initial.y + 8.0;

    // The control test `app_menu_keyboard_arrow_enter_baseline_minimize` (below)
    // proves the title-bar right-click DOES open the app menu by activating item 0
    // (Minimize) with a bare Enter — so a failure of THIS test isolates to the
    // ArrowDown highlight-advance, not to the menu failing to open. (app_menu_open
    // is pub(crate) with no public accessor, so the open state can only be observed
    // through the activated item's effect.)
    let _ = MouseButton::Left; // mouse-button import kept meaningful for the scenario

    let (_frame, state) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |handle| {
            // Open the window, then RIGHT-click its title bar to open the app menu.
            ScriptedScenario::new(handle)
                .left_click(dock_cx, dock_cy)
                .right_click(tb_x, tb_y)
                .into_events()
        },
        |shell| {
            // Navigate: ArrowDown moves highlight 0 -> 1 (Maximize), Enter activates.
            shell.handle_platform_event(&key(KeyCode::ArrowDown));
            let action = shell.handle_platform_event(&key(KeyCode::Enter));
            if let Some(a) = action {
                shell.execute_action(&a);
            }
            shell
                .visible_windows()
                .first()
                .map(|w| w.state)
                .expect("window still present")
        },
    )
    .expect("app-menu key-nav capture");

    assert_eq!(
        state,
        WindowState::Maximized,
        "app-menu keyboard nav broken: from the open app menu (highlight on item 0), ArrowDown \
         must move the highlight to item 1 (Maximize) and Enter must activate it, maximizing the \
         focused window. The window state is {state:?} instead of Maximized — ArrowDown did not \
         advance the app-menu highlight, or Enter did not activate the highlighted item. Check the \
         app-menu ArrowDown/Enter arms (events.rs:522-545) and activate_app_menu_index."
    );
}

/// CONTROL for the app-menu nav test: a bare Enter on the freshly-opened app
/// menu activates item 0 (Minimize). This proves the title-bar right-click DOES
/// open the app menu (highlight starts at 0) and that Enter activates the
/// highlighted item — so if `app_menu_keyboard_arrow_enter_activates_maximize`
/// fails while THIS passes, the defect is isolated to the ArrowDown
/// highlight-advance, not to the menu opening or Enter dispatch.
#[test]
fn app_menu_keyboard_enter_baseline_minimizes() {
    let (dock_cx, dock_cy) = {
        let (_f, c) = capture_desktop_scripted_readback(
            &scenario_options(THEME),
            |_h| Vec::new(),
            |shell| {
                let screen = shell.screen_rect();
                let rects = shell.dock().compute_item_rects(screen);
                let (_, rect) = rects.first().copied().expect("dock item");
                (rect.x + rect.width / 2.0, rect.y + rect.height / 2.0)
            },
        )
        .expect("dock geometry");
        c
    };

    let initial = {
        let (_f, b) = capture_desktop_scripted_readback(
            &scenario_options(THEME),
            |handle| ScriptedScenario::new(handle).left_click(dock_cx, dock_cy).into_events(),
            |shell| {
                shell
                    .visible_windows()
                    .first()
                    .map(|w| w.bounds)
                    .expect("window open")
            },
        )
        .expect("open-window probe");
        b
    };
    let tb_x = initial.x + 40.0;
    let tb_y = initial.y + 8.0;

    let (_frame, visible_after) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |handle| {
            ScriptedScenario::new(handle)
                .left_click(dock_cx, dock_cy)
                .right_click(tb_x, tb_y)
                .into_events()
        },
        |shell| {
            // Bare Enter on the open app menu -> item 0 (Minimize) -> window hidden.
            let action = shell.handle_platform_event(&key(KeyCode::Enter));
            if let Some(a) = action {
                shell.execute_action(&a);
            }
            shell.visible_windows().len()
        },
    )
    .expect("app-menu enter baseline capture");

    assert_eq!(
        visible_after, 0,
        "app-menu baseline broke: right-clicking the title bar then pressing Enter (which should \
         activate item 0, Minimize) left {visible_after} window(s) visible (expected 0). Either the \
         title-bar right-click did not open the app menu or Enter did not activate its first item. \
         Check events.rs right-click title-bar arm (1015-1023) and the app-menu Enter arm (539-542)."
    );
}

// ===========================================================================
// 4. FOCUS TRAVERSAL — Tab / Shift-Tab between focusable elements.
//    A desktop environment must let the keyboard move focus between focusable
//    UI elements in order with plain Tab / Shift-Tab. We assert the shell binds
//    Tab (and Shift-Tab) to a focus-traversal action.
//
//    ADVERSARIAL EXPECTATION: plain Tab should be a bound, focus-moving shortcut.
//    The shell's shortcut table binds ONLY Alt+Tab / Alt+Shift+Tab / Super+Tab —
//    there is NO plain-Tab focus-traversal binding, and `FocusManager` only cycles
//    *window* focus via history, not focusable-element traversal. So a bare Tab
//    resolves to no action and moves nothing. This FAILS, exposing the missing
//    focus-traversal navigation.
// ===========================================================================

#[test]
fn plain_tab_is_bound_to_a_focus_traversal_action() {
    let (_frame, bound_action) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |_handle| Vec::new(),
        |shell| {
            // Resolve a bare Tab (no modifiers) through the shortcut table the way
            // the live key path does.
            let ke = KeyEvent::new(KeyCode::Tab, KeyState::Pressed, Modifiers::new(), 0, 0);
            shell.shortcuts().handle_key_event(&ke).cloned()
        },
    )
    .expect("plain-tab binding probe");

    assert!(
        bound_action.is_some(),
        "plain Tab is not bound to ANY shell action: keyboard FOCUS TRAVERSAL between focusable \
         elements does not exist. The shortcut table (crates/liquide-shell/src/shortcuts.rs) binds \
         only Alt+Tab (SwitchWindowForward), Alt+Shift+Tab (SwitchWindowBackward) and Super+Tab \
         (TaskOverview) — there is no plain Tab / Shift-Tab focus-traversal binding, and \
         FocusManager (crates/liquide-shell/src/focus.rs) only cycles WINDOW focus via history, \
         not focusable-element traversal. FIX in liquide-shell: add Tab/Shift-Tab focus-traversal \
         (a focusable-element ring + bindings) so Tab moves focus."
    );
}

// ===========================================================================
// 5. WINDOW NAV — Alt-Tab (SwitchWindowForward) cycles window focus.
//    Open two windows, then Alt-Tab and assert the focused window changes, and a
//    second Alt-Tab round-trips back. STATE via focus_manager().focused().
// ===========================================================================

#[test]
fn alt_tab_cycles_window_focus_and_round_trips() {
    use liquide_input::keyboard::Modifiers;

    // Two dock items -> two distinct windows. Find the first two dock centres.
    let centres = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |_h| Vec::new(),
        |shell| {
            let screen = shell.screen_rect();
            let rects = shell.dock().compute_item_rects(screen);
            let cs: Vec<(f32, f32)> = rects
                .iter()
                .take(2)
                .map(|(_, r)| (r.x + r.width / 2.0, r.y + r.height / 2.0))
                .collect();
            cs
        },
    )
    .expect("dock geometry")
    .1;
    assert!(centres.len() >= 2, "need at least 2 dock items for two windows");

    let alt = Modifiers::from_bits(Modifiers::ALT);

    let (_frame, (f0, f1, f2, nwin)) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |handle| {
            ScriptedScenario::new(handle)
                .left_click(centres[0].0, centres[0].1)
                .left_click(centres[1].0, centres[1].1)
                .into_events()
        },
        move |shell| {
            let nwin = shell.window_count();
            let f0 = shell.focus_manager().focused();
            // Alt-Tab forward: focus must move to another window.
            let a1 = shell.handle_platform_event(&key_mod(KeyCode::Tab, alt));
            if let Some(a) = a1 {
                shell.execute_action(&a);
            }
            let f1 = shell.focus_manager().focused();
            // Alt-Tab again: with two windows it must round-trip back to f0.
            let a2 = shell.handle_platform_event(&key_mod(KeyCode::Tab, alt));
            if let Some(a) = a2 {
                shell.execute_action(&a);
            }
            let f2 = shell.focus_manager().focused();
            (f0, f1, f2, nwin)
        },
    )
    .expect("alt-tab capture");

    assert_eq!(nwin, 2, "two windows must be open for the Alt-Tab cycle test");
    assert!(f0.is_some(), "a window must be focused before Alt-Tab");
    assert_ne!(
        f0, f1,
        "Alt-Tab (SwitchWindowForward) did not change the focused window ({f0:?} -> {f1:?}). \
         Window navigation is broken: check the Alt+Tab binding (shortcuts.rs) and \
         FocusManager::focus_next (focus.rs) / execute_action(SwitchWindowForward) (tick.rs)."
    );
    assert_eq!(
        f0, f2,
        "a second Alt-Tab did not round-trip focus back ({f1:?} -> {f2:?}, expected {f0:?}). With \
         exactly two windows, two forward cycles must return to the starting window. \
         FocusManager::focus_next history cycling is wrong."
    );
}

// ===========================================================================
// 6. WORKSPACE NAV — WorkspaceNext / WorkspacePrev switch and round-trip, and
//    each workspace's window membership is correct at every step.
// ===========================================================================

#[test]
fn workspace_next_prev_switches_and_round_trips_with_correct_windows() {
    // Open a window on workspace 0, add a 2nd workspace, then step forward and
    // back, checking the active id and the per-step visible-window membership.
    let (dock_cx, dock_cy) = {
        let (_f, c) = capture_desktop_scripted_readback(
            &scenario_options(THEME),
            |_h| Vec::new(),
            |shell| {
                let screen = shell.screen_rect();
                let rects = shell.dock().compute_item_rects(screen);
                let (_, rect) = rects.first().copied().expect("dock item");
                (rect.x + rect.width / 2.0, rect.y + rect.height / 2.0)
            },
        )
        .expect("dock geometry");
        c
    };

    let (_frame, steps) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |handle| ScriptedScenario::new(handle).left_click(dock_cx, dock_cy).into_events(),
        |shell| {
            // Window is on workspace 0.
            let ws0 = shell.workspace_manager().active().id.0;
            let vis0 = shell.visible_windows().len();

            // Add a second workspace, switch forward to it.
            shell.execute_action(&ShellAction::WorkspaceAdd);
            shell.execute_action(&ShellAction::WorkspaceNext);
            let ws1 = shell.workspace_manager().active().id.0;
            let vis1 = shell.visible_windows().len();

            // Switch back (previous).
            shell.execute_action(&ShellAction::WorkspacePrev);
            let ws2 = shell.workspace_manager().active().id.0;
            let vis2 = shell.visible_windows().len();

            (ws0, vis0, ws1, vis1, ws2, vis2)
        },
    )
    .expect("workspace-nav capture");

    let (ws0, vis0, ws1, vis1, ws2, vis2) = steps;

    assert_eq!(vis0, 1, "the opened window must be visible on its origin workspace 0");
    assert_ne!(
        ws0, ws1,
        "WorkspaceNext did not change the active workspace ({ws0} -> {ws1}). Check \
         execute_action(WorkspaceNext) / WorkspaceManager::switch_next (workspace.rs)."
    );
    assert_eq!(
        vis1, 0,
        "after switching to the new (empty) workspace, the origin-workspace window must NOT \
         render (visible_windows filters by active-workspace membership), but {vis1} window(s) \
         are visible — workspace navigation is not actually changing which windows show."
    );
    assert_eq!(
        ws2, ws0,
        "WorkspacePrev did not round-trip back to the origin workspace ({ws1} -> {ws2}, expected \
         {ws0}). Forward/back workspace navigation must round-trip."
    );
    assert_eq!(
        vis2, 1,
        "after switching back to workspace 0, its window must render again ({vis2} visible, \
         expected 1). Workspace membership was lost across the round-trip."
    );
}

// ===========================================================================
// 7. LAUNCHER NAV — open the launcher, arrow / type to navigate, Enter launches.
//    Launcher selection is publicly readable (selected_index / result_count /
//    activate_selected), so this is a full STATE-asserted nav test.
// ===========================================================================

#[test]
fn launcher_arrow_keys_advance_selection_and_wrap() {
    let (_frame, (count, idx_after_down, idx_after_up_from_top)) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |_handle| Vec::new(),
        |shell| {
            // Open the launcher (seeds the default app grid).
            shell.launcher_mut().open();
            assert!(shell.launcher().is_visible(), "precondition: launcher open");
            let count = shell.launcher().result_count();
            assert!(count >= 2, "launcher must have >=2 default apps to navigate");

            // Fresh open: selected_index == 0. ArrowDown -> 1.
            assert_eq!(shell.launcher().selected_index(), 0, "fresh launcher selects index 0");
            shell.handle_platform_event(&key(KeyCode::ArrowDown));
            let idx_after_down = shell.launcher().selected_index();

            // Drive selection back to 0, then ArrowUp must WRAP to the last item.
            shell.launcher_mut().select_index(0);
            shell.handle_platform_event(&key(KeyCode::ArrowUp));
            let idx_after_up_from_top = shell.launcher().selected_index();

            (count, idx_after_down, idx_after_up_from_top)
        },
    )
    .expect("launcher arrow-nav capture");

    assert_eq!(
        idx_after_down, 1,
        "ArrowDown in the launcher did not advance the selection 0 -> 1 (got {idx_after_down}). \
         Check the launcher ArrowDown arm (events.rs:442-445) and Launcher::select_next."
    );
    assert_eq!(
        idx_after_up_from_top,
        count - 1,
        "ArrowUp from the first launcher item must WRAP to the last (index {}), got {}. \
         Launcher::select_prev wrap is broken.",
        count - 1,
        idx_after_up_from_top
    );
}

#[test]
fn launcher_type_to_filter_then_enter_launches() {
    // Type "term" to filter to the Terminal app, then Enter must launch it
    // (open a window) and close the launcher.
    let (_frame, (results_nonempty, top_is_terminal, window_count, launcher_closed)) =
        capture_desktop_scripted_readback(
            &scenario_options(THEME),
            |_handle| Vec::new(),
            |shell| {
                shell.launcher_mut().open();
                // Type the query through the live key path (each printable key is
                // appended to the launcher query by events.rs).
                for code in [KeyCode::T, KeyCode::E, KeyCode::R, KeyCode::M] {
                    shell.handle_platform_event(&key(code));
                }
                let results_nonempty = shell.launcher().result_count() > 0;
                let top_is_terminal = shell
                    .launcher()
                    .results()
                    .first()
                    .map(|r| r.title.to_lowercase().contains("terminal"))
                    .unwrap_or(false);

                // Enter activates the selected (top) result -> opens a window.
                let action = shell.handle_platform_event(&key(KeyCode::Enter));
                if let Some(a) = action {
                    shell.execute_action(&a);
                }
                (
                    results_nonempty,
                    top_is_terminal,
                    shell.window_count(),
                    !shell.launcher().is_visible(),
                )
            },
        )
        .expect("launcher type-enter capture");

    assert!(
        results_nonempty,
        "typing 'term' into the launcher produced NO results — type-to-filter navigation is \
         broken (keys not reaching the launcher query). Check keycode_to_char fall-through in the \
         launcher key arm (events.rs:471-479)."
    );
    assert!(
        top_is_terminal,
        "typing 'term' did not rank the Terminal app first — the filtered launcher grid is wrong, \
         so Enter would launch the wrong app."
    );
    assert_eq!(
        window_count, 1,
        "Enter on the filtered launcher result did not launch the app (window_count={window_count}, \
         expected 1). Check the launcher Enter arm -> activate_selected -> open_app_window \
         (events.rs:446-458)."
    );
    assert!(
        launcher_closed,
        "the launcher did not close after Enter launched an app — launcher navigation must dismiss \
         on activate."
    );
}

#[test]
fn launcher_escape_closes() {
    let closed = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |_handle| Vec::new(),
        |shell| {
            shell.launcher_mut().open();
            assert!(shell.launcher().is_visible(), "precondition: launcher open");
            let action = shell.handle_platform_event(&key(KeyCode::Escape));
            if let Some(a) = action {
                shell.execute_action(&a);
            }
            !shell.launcher().is_visible()
        },
    )
    .expect("launcher escape capture")
    .1;

    assert!(
        closed,
        "Escape did not close the launcher. Check the launcher Escape arm (events.rs:433-437)."
    );
}
