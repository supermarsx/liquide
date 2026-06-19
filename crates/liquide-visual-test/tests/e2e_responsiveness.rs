//! E2E RESPONSIVENESS-LATENCY suite (task t77, executor t77-RESP-TESTS).
//!
//! ## What this suite measures — and what it deliberately does NOT
//!
//! The user's complaint is jank: "test e2e responsiveness and hover delays etc
//! they should be near-ms responses at minimum." These tests put a *budget* on
//! the latency between a logical input event and the resulting STATE / SCENE
//! change, and FAIL when that budget is exceeded. A regression that re-introduces
//! a long hover dwell, a multi-frame click lag, or a key-stroke that needs more
//! than one event-dispatch to land turns one of these tests red.
//!
//! ### HONESTY BOUNDARY (the load-bearing limitation — read this first)
//!
//! These tests measure **LOGICAL latency**: how quickly the shell's *state* and
//! the *rendered scene* react to an input, expressed in event-dispatch cycles
//! and in the shell's per-frame animation budget (`frame_delta_ms`). They run on
//! the deterministic, single-threaded headless capture path
//! (`capture_desktop_scripted_*`) and read back shell state + the CPU
//! framebuffer.
//!
//! They CANNOT, and do not claim to, measure **wall-clock present latency** —
//! the time from "the OS delivered the input" to "the pixels were actually shown
//! on the user's screen." On the live build that interval also includes Win32 /
//! DXGI / GDI present, DWM composition, and (over RDP) the remote present layer.
//! None of that is observable offscreen. So:
//!   * "within one frame" here means "within one logical event-dispatch /
//!     scene-build", NOT "within one 16.67 ms vsync interval on the wire."
//!   * "the tooltip shows within the budget" means the canonical TooltipManager
//!     reaches a *painted* state once the shell has been advanced by the
//!     configured dwell budget — NOT that a human saw it N ms after hovering.
//! These are the strongest signals the offscreen harness can give; the present-
//! path latency needs a live on-screen probe and is explicitly OUT OF SCOPE
//! (matches the t58-flicker / t58 honesty note).
//!
//! ### Why a state/scene-cycle budget is a meaningful responsiveness metric
//!
//! On the real event loop each input is handled and the scene rebuilt before the
//! next present. So a logical reaction that needs only ONE event-dispatch lands
//! in the very next frame the loop draws (near-instant on a 60 Hz loop), whereas
//! a reaction that needs the animation clock to advance by a dwell budget B is
//! delayed by ~B regardless of how fast frames present. Counting dispatch cycles
//! and budgeting the dwell is therefore the offscreen-faithful proxy for "does
//! the DE react promptly."
//!
//! ### No fake-green (per the prime directive)
//!   * Every budget test has a paired TEETH check proving the probe can see the
//!     real change, so a passing budget is never vacuous.
//!   * The tooltip dwell budget is computed from `TooltipConfig::default()`, so
//!     it tracks the live config (t77-A1 set it to 100 ms + 50 ms fade-in). The
//!     "would have failed at the OLD 650 ms dwell" tooth is asserted directly: a
//!     capture advanced by only the NEW budget must already paint the tooltip,
//!     which is impossible under the old 500 + 150 ms dwell.
//!   * Where a target behavior is not yet in the tree, the specific test is
//!     `#[ignore]`d with a comment — never weakened to pass.

use liquide_input::keyboard::{KeyCode, KeyEvent, KeyState, Modifiers};
use liquide_platform::{NativeWindowHandle, PlatformEvent};
use liquide_tooltip::TooltipConfig;
use liquide_visual_test::diff::{DiffOptions, diff_frames};
use liquide_visual_test::scenarios::{ScriptedScenario, scenario_options, themed_desktop_capture};
use liquide_visual_test::{Frame, capture_desktop_scripted_readback, capture_desktop_scripted_with};

const THEME: &str = "liquid-glass";

// ───────────────────────────────────────────────────────────────────────────
// Tooltip dwell budget — read LIVE from `liquide_tooltip::TooltipConfig::
// default()`.
//
// SINGLE-SOURCING: the plan requires the budget to track `TooltipConfig::
// default()` automatically and NOT hardcode 150. We read `show_delay_ms` and
// `fade_in_ms` straight off the default config (via the `liquide-tooltip`
// dev-dependency), so when a peer tunes the config this suite follows it with no
// edit here. A1 reduced these to 100 ms / 50 ms (budget 150 ms); if they
// regressed back toward the old 500 ms / 150 ms (budget 650 ms) the budget
// helpers below move with them and the regression teeth fire.
// ───────────────────────────────────────────────────────────────────────────

/// `TooltipConfig::default().show_delay_ms`, read live (was 500 ms pre-A1).
fn tt_show_delay_ms() -> f32 {
    TooltipConfig::default().show_delay_ms as f32
}

/// `TooltipConfig::default().fade_in_ms`, read live (was 150 ms pre-A1).
fn tt_fade_in_ms() -> f32 {
    TooltipConfig::default().fade_in_ms as f32
}

/// The hover→fully-visible budget computed from the live default config:
/// `show_delay_ms + fade_in_ms`. With A1 this is 100 + 50 = 150 ms.
fn tooltip_budget_ms() -> f32 {
    tt_show_delay_ms() + tt_fade_in_ms()
}

/// The pre-A1 laggy dwell (500 ms show-delay + 150 ms fade-in = 650 ms). Used
/// only to express the "the new budget must be meaningfully faster than the old
/// jank" tooth.
const OLD_LAGGY_DWELL_MS: f32 = 650.0;

// ───────────────────────────────────────────────────────────────────────────
// Geometry helpers (read off the live layout, not hard-coded)
// ───────────────────────────────────────────────────────────────────────────

fn base_desktop() -> Frame {
    themed_desktop_capture(THEME).expect("base desktop capture")
}

/// Centre + top-left rect of the first dock item, read off the live dock layout.
fn first_dock_item() -> ((f32, f32), (u32, u32, u32, u32)) {
    let (_f, out) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |_h| Vec::new(),
        |shell| {
            let screen = shell.screen_rect();
            let rects = shell.dock().compute_item_rects(screen);
            let (_, r) = rects.first().copied().expect("dock has at least one item");
            (
                (r.x + r.width / 2.0, r.y + r.height / 2.0),
                (
                    r.x.max(0.0) as u32,
                    r.y.max(0.0) as u32,
                    r.width as u32,
                    r.height as u32,
                ),
            )
        },
    )
    .expect("dock-geometry probe");
    out
}

/// The tooltip paints in a band ABOVE the dock icon (anchored at
/// `item.y - offset`).
///
/// GEOMETRY NOTE (t172-e5 dock magnification): the band *immediately* above the
/// icon top is NO LONGER bare wallpaper on hover — macOS-style cursor-proximity
/// magnification (`apply_dock_magnification`, paint-only `transform: scale` with
/// `transform-origin: bottom`) grows the hovered glyph UPWARD out of its box the
/// instant the cursor is over the dock, independent of the tooltip dwell. With
/// the live dock (48px icons, ~1.5x peak factor) the magnified glyph reaches
/// roughly `iy - 28`, so that lower sub-band shows hundreds of changed px at any
/// dwell — even 0 ms — which is the magnification, not the tooltip. Measuring it
/// would let the dwell "teeth" probe see paint while the tooltip is still
/// Pending (the t172-e9 toothless failure).
///
/// We therefore probe ONLY the tooltip-only sub-band `[iy-44, iy-28)` (16 px),
/// which sits ABOVE where the magnified icon reaches but WITHIN the tooltip's
/// painted extent. Empirically (liquid-glass, this layout): 0 px changed while
/// the tooltip is Pending (< show-delay), ~236 px once it has surfaced — so any
/// paint here is the tooltip, and the dwell teeth stay sharp.
fn tooltip_region(item: (u32, u32, u32, u32)) -> (u32, u32, u32, u32) {
    let (ix, iy, iw, _ih) = item;
    let top = iy.saturating_sub(44);
    // Exclude the magnification zone (the ~28 px just above the icon top): the
    // tooltip-only band ends 28 px above the icon, not at the icon top.
    let bottom = iy.saturating_sub(28).max(top + 1);
    let h = bottom.saturating_sub(top).max(1);
    let x = ix.saturating_sub(40);
    let w = iw + 80;
    (x, top, w, h)
}

/// Count pixels in `frame`'s `rect` whose max-channel delta from the same rect in
/// `base` exceeds `tol`.
fn changed_vs(frame: &Frame, base: &Frame, rect: (u32, u32, u32, u32), tol: u8) -> usize {
    let (x, y, w, h) = rect;
    let mut n = 0;
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

/// Render a steady dock hover over the first dock item, advancing the shell's
/// per-frame animation clock (`frame_delta_ms`) by `delta_ms`. The pointer is
/// moved ONTO the item and left there; the canonical TooltipManager is advanced
/// by `delta_ms` in the captured render pass, so `delta_ms` is the dwell time the
/// hover has been held.
fn dock_hover_frame(delta_ms: f32) -> Frame {
    let ((cx, cy), _item) = first_dock_item();
    capture_desktop_scripted_with(
        &scenario_options(THEME),
        |handle| ScriptedScenario::new(handle).pointer_move(cx, cy).into_events(),
        move |shell| shell.set_frame_delta_ms(delta_ms),
    )
    .expect("dock-hover capture")
}

/// A single `KeyInput` PlatformEvent (press) for keyboard nav.
fn key_press(code: KeyCode) -> PlatformEvent {
    PlatformEvent::KeyInput {
        handle: NativeWindowHandle(1),
        event: KeyEvent::new(code, KeyState::Pressed, Modifiers::new(), 0, 0),
    }
}

// ===========================================================================
// 1. HOVER → TOOLTIP VISIBLE WITHIN THE CONFIGURED DWELL BUDGET.
//
//    A steady dock hover advanced by ONLY the new dwell budget
//    (show_delay + fade_in, computed from TooltipConfig::default()) must paint
//    the tooltip. This is the headline "hover delay" responsiveness assertion.
//
//    LOGICAL-LATENCY MEANING: the tooltip surfaces once the shell has been
//    advanced by `budget` ms of dwell; on the live loop that is ~budget ms after
//    the cursor stops, since the loop advances `frame_delta_ms` every frame.
// ===========================================================================

#[test]
fn hover_tooltip_paints_within_dwell_budget() {
    let (_centre, item) = first_dock_item();
    let tip = tooltip_region(item);
    let base = base_desktop();

    let budget = tooltip_budget_ms();

    // Advance the hover by EXACTLY the configured budget. A small +1ms nudge
    // keeps us robustly inside the fully-faded-in plateau against float rounding,
    // not a slack that would hide a regression (the old dwell is 650ms — 4x+
    // larger — so this margin cannot mask the pre-A1 behavior).
    let at_budget = dock_hover_frame(budget + 1.0);
    let painted = changed_vs(&at_budget, &base, tip, 10);

    assert!(
        painted > 30,
        "HOVER LATENCY REGRESSION: after advancing a steady dock hover by the \
         configured dwell budget ({budget} ms = show_delay + fade_in from \
         TooltipConfig::default()), the tooltip region {tip:?} changed only \
         {painted} px vs the bare desktop — the tooltip did NOT surface within \
         its own budget. If the tooltip renders at all this should be tens-to-\
         hundreds of px. (If A1's 100ms/50ms config was reverted to 500ms/150ms, \
         {budget}ms is below the dwell and this fails — which is the point.)"
    );
}

/// TEETH for test 1: the NEW budget is meaningfully faster than the OLD 650 ms
/// jank. A hover advanced by only the new budget must already paint the tooltip,
/// whereas the same paint is impossible under the old 500+150 ms dwell because
/// the new budget (150 ms) is far below it. We assert the new-budget frame paints
/// the tooltip AND that a frame advanced by *less than the old dwell but at least
/// the new budget* also paints — proving the responsiveness win is real, not a
/// threshold fudge.
#[test]
fn hover_tooltip_budget_beats_the_old_laggy_dwell() {
    let (_centre, item) = first_dock_item();
    let tip = tooltip_region(item);
    let base = base_desktop();

    let budget = tooltip_budget_ms();

    // Sanity: the regression we are guarding against only makes sense if the new
    // budget is actually faster than the old one. If they ever converge, this
    // tooth has lost its meaning and should fail loudly.
    assert!(
        budget < OLD_LAGGY_DWELL_MS,
        "the configured dwell budget ({budget} ms) is not faster than the old \
         laggy dwell ({OLD_LAGGY_DWELL_MS} ms) — the hover responsiveness win has \
         been lost; A1's TooltipConfig defaults regressed."
    );

    // A hover advanced by the NEW budget paints the tooltip; under the OLD 650 ms
    // dwell the very same advance would still be Pending (invisible). So this
    // single observation distinguishes the fixed config from the broken one.
    let at_new_budget = dock_hover_frame(budget + 1.0);
    let painted = changed_vs(&at_new_budget, &base, tip, 10);
    assert!(
        painted > 30,
        "the tooltip does not paint at the new {budget}ms budget — either the \
         dwell regressed past it or the tooltip render path broke ({painted} px)."
    );

    // TEETH (probe can see absence): well BELOW the show-delay the tooltip must
    // be Pending (not painted), so the budget test is not trivially-always-true.
    let show_delay = tt_show_delay_ms();
    let below_delay = (show_delay * 0.25).max(1.0);
    let early = dock_hover_frame(below_delay);
    let early_paint = changed_vs(&early, &base, tip, 10);
    assert!(
        early_paint <= painted / 2,
        "TOOTHLESS: the tooltip region already shows {early_paint} px after only \
         {below_delay}ms of dwell (well under the {show_delay}ms show-delay), \
         nearly as much as the {painted} px at full budget — the probe cannot tell \
         'shown' from 'pending', so the budget test would pass vacuously."
    );
}

// ===========================================================================
// 2. HOVER → CURSOR/STATE REACTS IN A SINGLE EVENT-DISPATCH (≈0 latency).
//
//    Moving the pointer onto a dock item must set the hover state + pointer
//    cursor in the SAME event-dispatch that delivered the move — no extra frame
//    of lag. (The tooltip has a dwell by design; the hover *state* must not.)
// ===========================================================================

#[test]
fn hover_state_reacts_in_one_dispatch() {
    let ((cx, cy), _item) = first_dock_item();

    let (_frame, (hover_idx, cursor)) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        // A SINGLE pointer move — exactly one input event.
        |handle| ScriptedScenario::new(handle).pointer_move(cx, cy).into_events(),
        |shell| (shell.dock().hover_index(), shell.cursor_shape()),
    )
    .expect("hover-state capture");

    assert_eq!(
        hover_idx,
        Some(0),
        "HOVER STATE LAG: after a single pointer-move onto dock item 0 the hover \
         index is {hover_idx:?}, expected Some(0). The hover state must latch in \
         the same dispatch as the move, not a frame later."
    );
    assert_eq!(
        format!("{cursor:?}"),
        "Pointer",
        "CURSOR LAG: cursor over a hovered dock item is {cursor:?}, expected \
         Pointer — the cursor must update in the same dispatch as the move."
    );
}

/// TEETH for test 2: with the pointer over BARE wallpaper (no hoverable), the
/// hover index is None — so `Some(0)` above is a real reaction to the move, not a
/// latched-on default.
#[test]
fn hover_state_teeth_clear_off_target() {
    let (_frame, hover_idx) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |handle| ScriptedScenario::new(handle).pointer_move(20.0, 360.0).into_events(),
        |shell| shell.dock().hover_index(),
    )
    .expect("off-target hover capture");
    assert_eq!(
        hover_idx, None,
        "TOOTHLESS: hover_index is {hover_idx:?} with the pointer on empty \
         wallpaper — it should be None. If it latches Some regardless of target, \
         test 2's Some(0) proves nothing."
    );
}

// ===========================================================================
// 3. RIGHT-CLICK → CONTEXT MENU PAINTED IN THE NEXT SCENE BUILD (one frame).
//
//    A right-click dispatched through the real input path must produce a context
//    menu that is PAINTED in the very next captured frame (the single render that
//    follows the click) — not after an extra tick. The capture renders exactly
//    one frame after the scripted events, so a menu visible in it = one-frame
//    latency.
// ===========================================================================

#[test]
fn right_click_context_menu_opens_in_one_frame() {
    // Click over the dark side of the gradient wallpaper so the translucent panel
    // separates cleanly (same geometry rationale as e2e_context_menu).
    let (rx, ry) = (700.0_f32, 300.0_f32);
    let base = base_desktop();

    let (frame, ()) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |handle| ScriptedScenario::new(handle).right_click(rx, ry).into_events(),
        |_shell| (),
    )
    .expect("right-click capture");

    // Menu top-left == click point (fits on-screen, no clamping). Probe the menu
    // panel rect for substantial new paint.
    let (ox, oy) = (rx.round() as u32, ry.round() as u32);
    let menu_rect = (ox, oy, 200u32, 148u32); // CONTEXT_MENU_WIDTH x 5-item height
    let painted = changed_vs(&frame, &base, menu_rect, 16);
    let menu_area = (menu_rect.2 * menu_rect.3) as usize;

    assert!(
        painted > menu_area / 3,
        "CONTEXT-MENU LATENCY: one render after a right-click, the menu rect at \
         ({ox},{oy}) changed only {painted}/{menu_area} px vs base (expected \
         > 1/3). The menu did not open within one frame of the click."
    );
}

/// TEETH for test 3: with NO right-click, the same rect is unchanged — so the
/// paint above is the menu reacting to the click, not pre-existing chrome.
#[test]
fn right_click_context_menu_teeth_absent_without_click() {
    let (rx, ry) = (700.0_f32, 300.0_f32);
    let base = base_desktop();
    // A no-op (pointer move only, no button) must not open a menu.
    let (frame, ()) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |handle| ScriptedScenario::new(handle).pointer_move(rx, ry).into_events(),
        |_shell| (),
    )
    .expect("no-click capture");

    let (ox, oy) = (rx.round() as u32, ry.round() as u32);
    let menu_rect = (ox, oy, 200u32, 148u32);
    let painted = changed_vs(&frame, &base, menu_rect, 16);
    let menu_area = (menu_rect.2 * menu_rect.3) as usize;
    assert!(
        painted < menu_area / 10,
        "TOOTHLESS: the menu rect changed {painted}/{menu_area} px WITHOUT a \
         right-click — something already paints there, so test 3 cannot attribute \
         its paint to the click."
    );
}

// ===========================================================================
// 4. CLICK → MENU STATE TOGGLES IN ONE DISPATCH (status-bar notification click).
//
//    Clicking the status-bar notification indicator must flip
//    `notification_center_open` in the single integrated event-dispatch
//    (handle_platform_event -> execute_action), not after additional ticks. The
//    scripted-capture path runs that exact integrated chain, so reading the state
//    back after one click = one-dispatch latency. This also guards the t58/t59
//    single-owner DOUBLE-TOGGLE regression (a click that opened-then-instantly-
//    closed would read back false here).
//
//    SEAM NOTE: the indicator's hit region is the documented fixed band
//    `36..=80 px` from the RIGHT edge of the status bar (events.rs), so we can
//    target it WITHOUT a private geometry accessor. The session-menu item, by
//    contrast, has NO public per-item bounds accessor on `ShellStatusBar`
//    (`status_bar_item_bounds` is `pub(crate)`), so a cross-crate click on it
//    cannot be reliably positioned here — its single-dispatch toggle is covered
//    by the keyboard-nav test (§6) and `toggle_session_menu` instead. (Seam
//    request: a public `status_bar item bounds` accessor would let an e2e click
//    target arbitrary bar items.)
// ===========================================================================

#[test]
fn status_bar_notification_click_opens_center_in_one_dispatch() {
    // The indicator hit region is 36..=80 px from the right edge of the status
    // bar (events.rs). Click at 58px from the right, vertically centred in the
    // ~34px bar. The bar always carries a NotificationIndicator item, so this
    // hit region routes OpenNotificationCenter regardless of notification count.
    let (_frame, opened) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |handle| {
            let x = 1280.0 - 58.0;
            let y = 17.0;
            ScriptedScenario::new(handle).left_click(x, y).into_events()
        },
        |shell| {
            shell.notification_center_open()
        },
    )
    .expect("notification-click capture");

    assert!(
        opened,
        "CLICK LATENCY: one left-click on the status-bar notification indicator \
         did not open the notification center within the single integrated \
         dispatch (notification_center_open() = {opened}). Check the 36..80px \
         hit-region arm (events.rs) and execute_action(OpenNotificationCenter); a \
         false may also indicate the t59 double-toggle (open-then-close)."
    );
}

/// TEETH for §4: without a click the center stays closed — the open state above
/// is the click opening it, not a default-open panel.
#[test]
fn status_bar_notification_click_teeth_closed_by_default() {
    let (_frame, open) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |_h| Vec::new(),
        |shell| shell.notification_center_open(),
    )
    .expect("default-state capture");
    assert!(
        !open,
        "TOOTHLESS: the notification center is open by default \
         (notification_center_open() = {open}) — §4's open state would not be \
         attributable to the click."
    );
}

// ===========================================================================
// 5. KEY → INPUT REFLECTED PER KEYSTROKE (no per-character lag).
//
//    Typing into the focused app must reflect each character with no extra-frame
//    lag: after the full scripted key sequence (one dispatch per key) the typed
//    string is present in the focused app's buffer. The capture path dispatches
//    every KeyInput through the real handle_platform_event chain, so the string
//    being complete after the sequence = one-dispatch-per-key latency (no
//    coalescing/loss/lag).
// ===========================================================================

#[test]
fn typed_keys_reflected_in_focused_app_buffer() {
    // Open + focus a window via a dock click, click into its body, then type.
    let ((dock_cx, dock_cy), _item) = first_dock_item();

    let (_frame, reached) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |handle| {
            ScriptedScenario::new(handle)
                .left_click(dock_cx, dock_cy) // open + focus the app window
                .left_click(640.0, 380.0) // click into the window body / field
                .type_text("hello")
                .into_events()
        },
        |shell| {
            // t70-s6: a registered app view receives the chars in its own model;
            // otherwise the shell's legacy focused_app_text buffer applies. Either
            // is the "input reflected" signal.
            if let Some(view) = shell.focused_app_view() {
                let model = view.content_view(80, 24);
                model
                    .title
                    .iter()
                    .map(String::as_str)
                    .chain(model.rows.iter().map(|r| r.text.as_str()))
                    .any(|t| t.contains("hello"))
            } else {
                shell.focused_app_text() == Some("hello")
            }
        },
    )
    .expect("typing capture");

    assert!(
        reached,
        "KEY LATENCY: after typing 'hello' into the focused app, the typed text \
         was NOT reflected in the app buffer this dispatch sequence. Each keystroke \
         must land in the same dispatch that delivered it (no per-character lag, \
         no dropped/coalesced keys). Check route_char_to_focused_app / the app \
         text-input seam."
    );
}

/// TEETH for §5: a DIFFERENT typed string is NOT spuriously reported as
/// reached, so the match above reflects the actual keystrokes, not a constant.
#[test]
fn typed_keys_teeth_wrong_string_not_reflected() {
    let ((dock_cx, dock_cy), _item) = first_dock_item();
    let (_frame, reached_wrong) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |handle| {
            ScriptedScenario::new(handle)
                .left_click(dock_cx, dock_cy)
                .left_click(640.0, 380.0)
                .type_text("hello")
                .into_events()
        },
        |shell| {
            // Probe for a string we did NOT type.
            if let Some(view) = shell.focused_app_view() {
                let model = view.content_view(80, 24);
                model
                    .title
                    .iter()
                    .map(String::as_str)
                    .chain(model.rows.iter().map(|r| r.text.as_str()))
                    .any(|t| t.contains("zzzzz"))
            } else {
                shell.focused_app_text() == Some("zzzzz")
            }
        },
    )
    .expect("typing teeth capture");
    assert!(
        !reached_wrong,
        "TOOTHLESS: the buffer reports a string ('zzzzz') that was never typed — \
         §5's match does not reflect the real keystrokes."
    );
}

// ===========================================================================
// 6. KEYBOARD MENU-NAV → HIGHLIGHT ADVANCES PER KEYPRESS (one dispatch each).
//
//    With the session menu open, each ArrowDown must advance the highlight by
//    one item in the dispatch that delivered it. We compare the painted menu
//    after 1 vs 2 ArrowDowns: a different highlighted row means the second key
//    advanced the selection in its own dispatch (no per-key lag).
// ===========================================================================

fn session_menu_after_downs(downs: usize) -> Frame {
    capture_desktop_scripted_with(
        &scenario_options(THEME),
        |_h| Vec::new(),
        move |shell| {
            if !shell.session_menu_visible() {
                shell.toggle_session_menu();
            }
            for _ in 0..downs {
                if let Some(a) = shell.handle_platform_event(&key_press(KeyCode::ArrowDown)) {
                    shell.execute_action(&a);
                }
            }
        },
    )
    .expect("session-menu nav capture")
}

#[test]
fn keyboard_menu_nav_advances_highlight_per_keypress() {
    let one = session_menu_after_downs(1);
    let two = session_menu_after_downs(2);
    let differ = diff_frames(&one, &two, DiffOptions::default()).differing_pixels;
    assert!(
        differ > 50,
        "KEY-NAV LATENCY: pressing ArrowDown a second time changed only {differ} \
         px in the rendered session menu vs one press — the highlight did not \
         advance on the second keypress (per-key nav lag, or the highlight does \
         not paint). Each ArrowDown must move the selection in its own dispatch."
    );
}

/// TEETH for §6: the same number of ArrowDowns renders identically (the diff
/// is from the EXTRA keypress, not nondeterminism).
#[test]
fn keyboard_menu_nav_teeth_same_count_is_stable() {
    let a = session_menu_after_downs(1);
    let b = session_menu_after_downs(1);
    let differ = diff_frames(&a, &b, DiffOptions::exact()).differing_pixels;
    assert_eq!(
        differ, 0,
        "NON-DETERMINISTIC: two identical 1-ArrowDown captures differ by {differ} \
         px — §6's diff cannot be attributed to the extra keypress."
    );
}
