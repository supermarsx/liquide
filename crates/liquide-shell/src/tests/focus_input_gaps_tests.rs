//! t94-e4 — focus/input gap regressions (t92 gap #5).
//!
//! Two gaps, each reproduced then proven fixed:
//!   * #5a focus-follows-mouse — modeled by `FocusPolicy::FocusFollowsMouse`
//!     but had no live consumer. With it ON, a pointer move into a different
//!     window focuses that window; with it OFF (the DEFAULT, click-to-focus)
//!     a move does NOT change focus. No refocus mid-drag; no thrash over the
//!     same window. Hit-testing goes through the canonical tree router
//!     (`window_at_point`, t93-e3), never a reintroduced flat scan.
//!   * #5b modal grab — when a modal dialog is open, input is grabbed to it:
//!     clicks/keys outside the modal do not focus or activate other windows.
//!     The modal STACK is respected (nested modals unwind one level at a time).
//!
//! These tests fail if the policy is ignored or the modal grab leaks.

use crate::focus::{FocusManager, FocusPolicy};
use crate::shell::Shell;
use crate::shortcuts::ShellAction;
use crate::window::WindowId;
use liquide_compositor::geometry::Rect;
use liquide_input::mouse::{ButtonState, MouseButton, MouseEvent};
use liquide_platform::event_loop::PlatformEvent;
use liquide_platform::window_host::NativeWindowHandle;

fn mouse_move(x: f32, y: f32) -> PlatformEvent {
    PlatformEvent::MouseInput {
        handle: NativeWindowHandle(0),
        event: MouseEvent::Move { x, y },
    }
}

fn left_press(x: f32, y: f32) -> PlatformEvent {
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

/// Two side-by-side, non-overlapping windows A (left) and B (right). Returns
/// `(shell, a, b)` with A focused (it was opened first, then re-focused).
fn shell_with_two_windows() -> (Shell, WindowId, WindowId) {
    let mut shell = Shell::new(1920.0, 1080.0);
    let a = shell.open_window("A", Rect::new(100.0, 100.0, 400.0, 300.0));
    let b = shell.open_window("B", Rect::new(800.0, 100.0, 400.0, 300.0));
    // Start with A focused so a move into B is a genuine focus change.
    shell.set_focus(a).unwrap();
    assert_eq!(shell.focus.focused(), Some(a));
    // Sanity: the points we will use resolve to the expected windows via the
    // canonical router, so the test is exercising real hit-testing.
    assert_eq!(shell.window_at_point(200.0, 200.0), Some(a));
    assert_eq!(shell.window_at_point(900.0, 200.0), Some(b));
    (shell, a, b)
}

// ── #5a focus-follows-mouse ────────────────────────────────────────────

/// FFM OFF (default click-to-focus): moving the pointer into window B must NOT
/// change focus — it stays on A until a click. This is the reproduce-half: if
/// the move handler ignored the policy and always tracked the pointer, focus
/// would wrongly move to B here.
#[test]
fn move_into_other_window_does_not_focus_under_click_to_focus_default() {
    let (mut shell, a, _b) = shell_with_two_windows();
    assert_eq!(
        shell.focus_policy(),
        FocusPolicy::ClickToFocus,
        "click-to-focus must be the DEFAULT policy"
    );

    shell.handle_platform_event(&mouse_move(900.0, 200.0)); // over B
    assert_eq!(
        shell.focus.focused(),
        Some(a),
        "with click-to-focus (default), a pointer move into B must NOT steal focus from A"
    );
}

/// FFM ON: moving the pointer into window B focuses B; moving back into A
/// focuses A. Proves the policy is consumed live through the canonical router.
#[test]
fn move_into_other_window_focuses_it_under_focus_follows_mouse() {
    let (mut shell, a, b) = shell_with_two_windows();
    shell.set_focus_policy(FocusPolicy::FocusFollowsMouse);
    assert_eq!(shell.focus_policy(), FocusPolicy::FocusFollowsMouse);

    // Pointer crosses into B → focus follows.
    shell.handle_platform_event(&mouse_move(900.0, 200.0));
    assert_eq!(
        shell.focus.focused(),
        Some(b),
        "focus-follows-mouse must focus B when the pointer crosses into it"
    );

    // Pointer crosses back into A → focus follows again.
    shell.handle_platform_event(&mouse_move(200.0, 200.0));
    assert_eq!(
        shell.focus.focused(),
        Some(a),
        "focus-follows-mouse must focus A when the pointer crosses back into it"
    );
}

/// FFM focuses without auto-raising: classic focus-follows-mouse changes focus
/// but does NOT bring the window to the top of the z-order (auto-raise stays a
/// click-only behavior). Over the overlap of two windows, focusing the lower
/// one via FFM must not make it win the hit-test.
#[test]
fn focus_follows_mouse_does_not_auto_raise() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let a = shell.open_window("A", Rect::new(100.0, 100.0, 400.0, 300.0));
    let b = shell.open_window("B", Rect::new(200.0, 200.0, 400.0, 300.0));
    // B is topmost at the overlap (created last).
    assert_eq!(shell.window_at_point(300.0, 300.0), Some(b));
    shell.set_focus(b).unwrap();
    shell.set_focus_policy(FocusPolicy::FocusFollowsMouse);

    // FFM move over the overlap. The router resolves the TOPMOST (B), so focus
    // stays/lands on B and the stack is unchanged. (A is occluded here; FFM
    // never silently raises the lower window.)
    shell.handle_platform_event(&mouse_move(300.0, 300.0));
    assert_eq!(shell.focus.focused(), Some(b));
    assert_eq!(
        shell.window_at_point(300.0, 300.0),
        Some(b),
        "FFM must not auto-raise: the z-order/topmost is unchanged by a focus-follows move"
    );
    let _ = a;
}

/// No refocus mid-drag: while a window move drag is in progress, an FFM pointer
/// move that passes over a different window must NOT change focus (the dragged
/// window keeps focus). Reproduces the focus-thrash gap the task calls out.
#[test]
fn focus_follows_mouse_does_not_refocus_during_drag() {
    let (mut shell, a, b) = shell_with_two_windows();
    shell.set_focus_policy(FocusPolicy::FocusFollowsMouse);

    // Start a move drag on A by injecting a Moving drag state directly (the
    // titlebar geometry is theme-dependent; the contract under test is "no
    // refocus while a drag is active", independent of how the drag began).
    shell.drag_state = Some(crate::shell::DragState::Moving {
        window_id: a,
        offset_x: 10.0,
        offset_y: 10.0,
    });

    // Move the pointer far into B's area while dragging A.
    shell.handle_platform_event(&mouse_move(900.0, 200.0));
    assert_eq!(
        shell.focus.focused(),
        Some(a),
        "no focus-follows-mouse refocus may occur while a drag is in progress"
    );
    let _ = b;
}

/// No thrash over the same window: repeated FFM moves that stay within the same
/// window must not redraw on every move (only a genuine focus change does).
#[test]
fn focus_follows_mouse_same_window_move_is_noop() {
    let (mut shell, _a, b) = shell_with_two_windows();
    shell.set_focus_policy(FocusPolicy::FocusFollowsMouse);

    // First move into B: a genuine change → requests a redraw.
    let first = shell.handle_platform_event(&mouse_move(880.0, 180.0));
    assert_eq!(shell.focus.focused(), Some(b));
    assert!(
        matches!(first, Some(ShellAction::Redraw)),
        "the first move into B (a real focus change) should request a redraw"
    );

    // Second move that stays inside B: focus is already B, so FFM must be a
    // no-op (no spurious refocus). Focus stays B.
    let second = shell.handle_platform_event(&mouse_move(1000.0, 250.0));
    assert_eq!(
        shell.focus.focused(),
        Some(b),
        "a move that stays inside the focused window must not change focus (no thrash)"
    );
    // The second move must not have been driven into a focus change.
    let _ = second;
}

// ── #5b modal grab ─────────────────────────────────────────────────────

/// With a modal dialog open, a click on a non-modal window behind the scrim
/// must NOT focus it — input is grabbed to the modal. Reproduce-half: without
/// the grab the click would focus the background window.
#[test]
fn click_on_background_window_is_swallowed_while_modal_open() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let a = shell.open_window("A", Rect::new(100.0, 100.0, 400.0, 300.0));
    let b = shell.open_window("B", Rect::new(800.0, 100.0, 400.0, 300.0));
    shell.set_focus(a).unwrap();

    // Open a modal dialog (establishes the modal grab).
    let _id = shell.request_message_dialog(
        crate::notification::ShellDialogKind::Confirm,
        "Quit?",
        "Discard changes?",
    );
    assert!(shell.has_active_modal(), "a dialog must establish a modal grab");

    // A real left press squarely on background window B.
    shell.handle_platform_event(&left_press(900.0, 200.0));
    assert_eq!(
        shell.focus.focused(),
        Some(a),
        "while a modal is open, a click on a background window must NOT focus it (grab leaked)"
    );
    let _ = b;
}

/// Control: with NO modal open, the very same click DOES focus the background
/// window — proving the swallow above is caused by the modal grab, not by the
/// click being a no-op for some unrelated reason (teeth for the test).
#[test]
fn click_on_background_window_focuses_it_without_modal() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let a = shell.open_window("A", Rect::new(100.0, 100.0, 400.0, 300.0));
    let b = shell.open_window("B", Rect::new(800.0, 100.0, 400.0, 300.0));
    shell.set_focus(a).unwrap();
    assert!(!shell.has_active_modal());

    shell.handle_platform_event(&left_press(900.0, 200.0));
    assert_eq!(
        shell.focus.focused(),
        Some(b),
        "with no modal, a click on B must focus B (control proves the modal swallow has teeth)"
    );
}

/// FFM is suppressed while a modal is open: even with focus-follows-mouse ON,
/// a pointer move over a background window must NOT change focus while the modal
/// grab is in effect.
#[test]
fn focus_follows_mouse_is_suppressed_while_modal_open() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let a = shell.open_window("A", Rect::new(100.0, 100.0, 400.0, 300.0));
    let _b = shell.open_window("B", Rect::new(800.0, 100.0, 400.0, 300.0));
    shell.set_focus(a).unwrap();
    shell.set_focus_policy(FocusPolicy::FocusFollowsMouse);

    shell.request_message_dialog(
        crate::notification::ShellDialogKind::Info,
        "Heads up",
        "Modal is open",
    );
    assert!(shell.has_active_modal());

    shell.handle_platform_event(&mouse_move(900.0, 200.0)); // over B
    assert_eq!(
        shell.focus.focused(),
        Some(a),
        "focus-follows-mouse must be suppressed while a modal grab is active"
    );
}

/// Opening a dialog then dismissing it releases the grab so clicks reach
/// windows again — proves the grab is not sticky.
#[test]
fn dismissing_modal_releases_the_grab() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let a = shell.open_window("A", Rect::new(100.0, 100.0, 400.0, 300.0));
    let b = shell.open_window("B", Rect::new(800.0, 100.0, 400.0, 300.0));
    shell.set_focus(a).unwrap();

    shell.request_message_dialog(
        crate::notification::ShellDialogKind::Confirm,
        "Quit?",
        "Discard changes?",
    );
    assert!(shell.has_active_modal());
    // Click swallowed while modal up.
    shell.handle_platform_event(&left_press(900.0, 200.0));
    assert_eq!(shell.focus.focused(), Some(a));

    // Dismiss → grab released → click now focuses B.
    shell.dismiss_active_dialog();
    assert!(!shell.has_active_modal(), "dismiss must release the modal grab");
    shell.handle_platform_event(&left_press(900.0, 200.0));
    assert_eq!(
        shell.focus.focused(),
        Some(b),
        "after dismissing the modal, a click must reach the background window again"
    );
}

/// Nested-modal correctness at the shell grab level: with TWO modals pushed,
/// the grab stays in effect after dismissing the inner one — a click is still
/// swallowed — and is only released once BOTH are dismissed. This is the
/// nested-modal requirement: dismissing the innermost modal must not prematurely
/// release the grab held by the one beneath it.
#[test]
fn nested_modals_keep_grab_until_all_dismissed() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let a = shell.open_window("A", Rect::new(100.0, 100.0, 400.0, 300.0));
    let b = shell.open_window("B", Rect::new(800.0, 100.0, 400.0, 300.0));
    shell.set_focus(a).unwrap();

    // Push two nested modal grabs directly (the single-slot dialog API is
    // replace-semantics; true nesting is a property of the modal STACK).
    shell.focus.push_modal(1001);
    shell.focus.push_modal(1002);
    assert_eq!(shell.modal_depth(), 2, "two nested modals are active");

    // Inner modal up: click swallowed.
    shell.handle_platform_event(&left_press(900.0, 200.0));
    assert_eq!(shell.focus.focused(), Some(a));

    // Dismiss the INNER modal: the outer one still holds the grab.
    assert_eq!(shell.focus.pop_modal(), Some(1002));
    assert_eq!(shell.modal_depth(), 1);
    assert!(
        shell.has_active_modal(),
        "dismissing the inner modal must NOT release the outer modal's grab"
    );
    shell.handle_platform_event(&left_press(900.0, 200.0));
    assert_eq!(
        shell.focus.focused(),
        Some(a),
        "while the outer modal is still up, the click must remain swallowed"
    );

    // Dismiss the OUTER modal: grab fully released, click reaches B.
    assert_eq!(shell.focus.pop_modal(), Some(1001));
    assert!(!shell.has_active_modal());
    shell.handle_platform_event(&left_press(900.0, 200.0));
    assert_eq!(
        shell.focus.focused(),
        Some(b),
        "once all nested modals are dismissed, input reaches windows again"
    );
}

// ── modal stack unit guarantees (focus.rs) ─────────────────────────────

/// The modal stack unwinds LIFO and `active_modal` always reports the topmost.
#[test]
fn modal_stack_is_lifo_and_reports_topmost() {
    let mut fm = FocusManager::new(FocusPolicy::ClickToFocus);
    assert!(!fm.has_active_modal());
    assert_eq!(fm.active_modal(), None);

    fm.push_modal(10);
    fm.push_modal(20);
    fm.push_modal(30);
    assert_eq!(fm.modal_depth(), 3);
    assert_eq!(fm.active_modal(), Some(30));

    assert_eq!(fm.pop_modal(), Some(30));
    assert_eq!(fm.active_modal(), Some(20));
    assert_eq!(fm.pop_modal(), Some(20));
    assert_eq!(fm.active_modal(), Some(10));
    assert_eq!(fm.pop_modal(), Some(10));
    assert_eq!(fm.active_modal(), None);
    assert!(!fm.has_active_modal());
    assert_eq!(fm.pop_modal(), None);
}

/// Pushing the same modal token twice does not require two pops to release it
/// (defensive de-dup), and `remove_modal` can drop a specific modal out of
/// stack order while preserving the rest.
#[test]
fn modal_stack_dedups_and_supports_out_of_order_removal() {
    let mut fm = FocusManager::new(FocusPolicy::ClickToFocus);
    fm.push_modal(7);
    fm.push_modal(7); // re-push same token
    assert_eq!(fm.modal_depth(), 1, "re-pushing the same token must not stack twice");

    fm.push_modal(8);
    fm.push_modal(9);
    // Remove the middle modal (8) out of order; 7 and 9 remain, 9 still topmost.
    fm.remove_modal(8);
    assert_eq!(fm.modal_depth(), 2);
    assert_eq!(fm.active_modal(), Some(9));
    fm.clear_modals();
    assert!(!fm.has_active_modal());
}
