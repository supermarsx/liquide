//! ADVERSARIAL end-to-end suite for HOVER STABILITY / FLICKER-ON-HOVER
//! (t66-hover).
//!
//! PRIME DIRECTIVE: a hoverable element (dock item, menu item, status-bar item)
//! that is hovered with the pointer held STEADY must render a STABLE picture —
//! the same pixels frame after frame. Anything that oscillates, flashes on/off,
//! or fades while the cursor never moved is FLICKER and these tests are written
//! to FAIL on it (failures are the desired finding — no fake green).
//!
//! ## VERDICT (t66-hover): there is NO hover flicker — there is a deeper bug.
//!
//! Reproduced offscreen on the deterministic capture path, hovering a dock item
//! (or navigating a menu) paints **ZERO** changed pixels at EVERY animation time
//! (delta swept 50 ms … 6000 ms; see `diag_hover_paint_sweep`). The hover STATE
//! is tracked correctly (`dock().hover_index() == Some(0)`, `cursor_shape()` →
//! `Pointer`), but the hover VISUAL never renders. With nothing painted there is
//! nothing to oscillate, so the "flicker" symptom cannot reproduce here.
//!
//! ROOT CAUSE — a selector/class mismatch + an unwired pseudo-state:
//!   * `dom_sync::sync_dock_template` injects a `.hovered` CSS *class* on the
//!     hovered dock item (crates/liquide-shell/src/shell/dom_sync.rs:393), but
//!     the theme only styles the `:hover` *pseudo-class* (`dock-item:hover`,
//!     assets/themes/liquid_glass.css:368) — there is NO `dock-item.hovered`
//!     rule, so the class never matches and the icon never restyles.
//!   * the pseudo-state path that WOULD match (`set_pseudo_state(item,
//!     PseudoStateFlags::HOVER, …)`) exists for the launcher / menus / titlebar
//!     buttons (desktop_dom.rs:605/657/689) but there is NO `set_dock_hover`
//!     and `set_menu_hover` is NEVER called from the render path — the only live
//!     caller is `set_launcher_hover` (threading.rs:193). The threaded dock
//!     update is an explicit no-op (threading.rs:179).
//!   * the same mismatch hits menus: dom_sync writes a `.selected` class
//!     (dom_sync.rs:856) but the theme styles `menu-item:hover` with no
//!     `menu-item.selected` rule — so keyboard-nav / hover highlight never
//!     paints either. (The launcher works precisely because it has BOTH
//!     `launcher-item.selected` AND the `set_launcher_hover` pseudo wiring.)
//!   * the dock TOOLTIP also never paints (0 px at all deltas): the canonical
//!     `TooltipManager` (tooltip_adapter.rs) is wired, but its overlay does not
//!     surface on the capture render — gated behind the same hover-render path.
//!
//! FIXER: the owning crate is **liquide-shell** (dom_sync + a missing
//! `set_dock_hover` / `set_menu_hover` call on the render path), with a one-line
//! theme alternative (add `.hovered` / `.selected` selectors to
//! assets/themes/*.css). Until then, the three "must paint" teeth below FAIL and
//! the steady-stability teeth pass VACUOUSLY (nothing renders → nothing flickers
//! — noted on each).
//!
//! ## Determinism model (t65-determinism) and how "consecutive frames" are made
//!
//! Every `capture_*` entry point builds a FRESH compositor + shell, runs the
//! loading prologue, then renders the read-back desktop frame. The shell's
//! per-frame animation clock is the single `frame_delta_ms` value the capture
//! advances the chrome by on each render pass (see
//! `DesktopCompositor::capture_once_scripted_with` and
//! `Shell::set_frame_delta_ms`). The renderer is otherwise deterministic, so:
//!
//!   * two captures of the SAME hover scenario with the SAME `frame_delta_ms`
//!     are byte-identical — this is the "render the same hovered frame twice"
//!     stability probe (a region that differs across these = oscillation = FAIL).
//!
//!   * a "consecutive frame" at a different point in a time-driven animation is
//!     reproduced by re-capturing the SAME steady hover with a DIFFERENT
//!     `frame_delta_ms` (a different accumulated animation time). If a hovered
//!     region's pixels depend on `frame_delta_ms` *without any input change*,
//!     that region animates underneath a steady cursor — i.e. it flickers
//!     frame-to-frame on a real machine where the delta varies every frame.
//!
//! Where useful, each test also reads SHELL STATE through the `&mut Shell`
//! readback seam (`dock().hover_index()`, `cursor_shape()`) to corroborate the
//! pixel verdict.
//!
//! ## Hover targets exercised
//!   1. DOCK ITEM hover — `dock-item:hover` background swap + the dock hover
//!      tooltip (the canonical `TooltipManager`, driven every frame by
//!      `sync_tooltip_manager(frame_delta_ms)`).
//!   2. SESSION-MENU item hover — `menu-item:hover` highlight (keyboard-nav
//!      hover-index), a state swap.
//!
//! See `.orchestration/logs/t66-hover.md` for the flicker verdict + root cause.

use liquide_input::keyboard::{KeyCode, KeyEvent, KeyState, Modifiers};
use liquide_input::mouse::{ButtonState, MouseButton, MouseEvent};
use liquide_platform::{NativeWindowHandle, PlatformEvent};
use liquide_visual_test::diff::{DiffOptions, diff_frames};
use liquide_visual_test::scenarios::{
    ScriptedScenario, scenario_options, themed_desktop_capture,
};
use liquide_visual_test::{Frame, capture_desktop_scripted_readback, capture_desktop_scripted_with};

const THEME: &str = "liquid-glass";

// ── TooltipConfig::default() constants (liquide-tooltip/src/config.rs) ───────
// These drive the per-frame `TooltipManager` lifecycle on a steady dock hover.
const TT_SHOW_DELAY_MS: f32 = 500.0;
const TT_FADE_IN_MS: f32 = 150.0;
const TT_DISPLAY_DURATION_MS: f32 = 5000.0;

// ───────────────────────────────────────────────────────────────────────────
// Helpers
// ───────────────────────────────────────────────────────────────────────────

/// A no-interaction base desktop (no hover, no menus) for differential probes
/// and clean-revert checks.
fn base_desktop() -> Frame {
    themed_desktop_capture(THEME).expect("base desktop capture")
}

/// Centre of the first dock item, read off the live dock layout (not hard-coded)
/// so it tolerates dock-config drift. Mirrors interaction_e2e.rs.
fn first_dock_item_centre() -> (f32, f32) {
    let (_f, centre) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |_h| Vec::new(),
        |shell| {
            let screen = shell.screen_rect();
            let rects = shell.dock().compute_item_rects(screen);
            let (_, rect) = rects.first().copied().expect("dock has at least one item");
            (rect.x + rect.width / 2.0, rect.y + rect.height / 2.0)
        },
    )
    .expect("dock-geometry probe");
    centre
}

/// Top-left rect of the first dock item (screen px), for the dock-icon hover
/// region probe.
fn first_dock_item_rect() -> (u32, u32, u32, u32) {
    let (_f, r) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |_h| Vec::new(),
        |shell| {
            let screen = shell.screen_rect();
            let rects = shell.dock().compute_item_rects(screen);
            let (_, rect) = rects.first().copied().expect("dock item");
            (
                rect.x.max(0.0) as u32,
                rect.y.max(0.0) as u32,
                rect.width as u32,
                rect.height as u32,
            )
        },
    )
    .expect("dock-rect probe");
    r
}

/// Render a steady dock hover over the first dock item at the given
/// `frame_delta_ms` (the per-frame animation clock). The pointer is moved ONTO
/// the item and left there; nothing else changes between captures except the
/// animation delta, so any pixel difference between two such frames is a
/// time-driven animation running underneath a motionless cursor.
fn dock_hover_frame(delta_ms: f32) -> Frame {
    let (cx, cy) = first_dock_item_centre();
    capture_desktop_scripted_with(
        &scenario_options(THEME),
        |handle| ScriptedScenario::new(handle).pointer_move(cx, cy).into_events(),
        move |shell| shell.set_frame_delta_ms(delta_ms),
    )
    .expect("dock-hover capture")
}

/// The dock hover TOOLTIP rect: the tooltip is anchored above the dock item
/// (`tip_y = item_rect.y - 32`, `events.rs`). We probe a band spanning the full
/// item width above the icon, tall enough to cover the tooltip box, clamped into
/// the frame. This region is empty wallpaper at base, so any paint here is the
/// tooltip.
fn tooltip_region(item: (u32, u32, u32, u32)) -> (u32, u32, u32, u32) {
    let (ix, iy, iw, _ih) = item;
    let top = iy.saturating_sub(40);
    let h = iy.saturating_sub(top); // up to 40 px band above the icon
    // Widen a little either side to capture a tooltip box wider than the icon.
    let x = ix.saturating_sub(30);
    let w = iw + 60;
    (x, top, w, h.max(1))
}

/// Count pixels in `frame`'s rect whose max-channel delta from the same rect in
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

/// Diff two crops of the same rect from two frames, returning the differing
/// pixel count at the given tolerance.
fn region_diff(a: &Frame, b: &Frame, rect: (u32, u32, u32, u32), tol: u8) -> usize {
    let (x, y, w, h) = rect;
    let ca = a.crop(x, y, w, h);
    let cb = b.crop(x, y, w, h);
    diff_frames(&ca, &cb, DiffOptions::default().tolerance(tol).budget(0)).differing_pixels
}

// ===========================================================================
// 1. DOCK HOVER — STEADY STATE IS STABLE (teeth: byte-identical re-render).
//
// Two captures of the IDENTICAL steady dock hover at the SAME animation delta
// must be byte-for-byte identical over the dock band: the `dock-item:hover`
// background swap is an instantaneous CSS state (no transition declared), so a
// held hover must not oscillate. If this region differs across two identical
// renders, the dock hover itself is non-deterministic/flickering.
// ===========================================================================

#[test]
fn dock_hover_steady_is_byte_stable() {
    // NOTE (t66 verdict): this PASSES, but VACUOUSLY — see the module verdict.
    // The dock hover highlight never paints (0 px change vs base, all deltas),
    // so two steady-hover frames trivially match. Kept as a real stability tooth
    // for AFTER the hover-render bug is fixed: once `dock-item:hover` actually
    // paints, this asserts the held hover does not oscillate render-to-render.
    //
    // A delta safely inside the tooltip's fully-Visible plateau is irrelevant
    // here because we compare two captures with the SAME delta — the dock band
    // (icon row) must match exactly regardless.
    let item = first_dock_item_rect();
    let a = dock_hover_frame(800.0);
    let b = dock_hover_frame(800.0);

    // Probe the dock ICON itself (not the tooltip band above it): the hovered
    // item's own pixels must be identical render-to-render.
    let dock_rect = (
        item.0.saturating_sub(4),
        item.1.saturating_sub(4),
        item.2 + 8,
        item.3 + 8,
    );
    let differ = region_diff(&a, &b, dock_rect, 0);
    assert_eq!(
        differ, 0,
        "DOCK HOVER FLICKER: the hovered dock icon rect {dock_rect:?} differs by {differ} \
         pixels across two identical steady-hover renders at the same animation delta. A held \
         hover over a dock item must be byte-stable (the `dock-item:hover` swap has no transition)."
    );
}

// ===========================================================================
// 2. DOCK HOVER — VISUALLY DISTINCT FROM NOT-HOVERED (teeth for tests 1 & 3:
//    prove the hover actually changes the icon, so a "stable" verdict is not a
//    no-op).
// ===========================================================================

#[test]
fn dock_hover_actually_changes_the_icon() {
    let item = first_dock_item_rect();
    let base = base_desktop();
    // Use a delta BELOW the show-delay so the tooltip is still Pending (invisible)
    // and only the dock-item:hover background swap is in play — isolating the
    // icon change from the tooltip.
    let hovered = dock_hover_frame(100.0);

    let dock_rect = (item.0, item.1, item.2, item.3);
    let changed = changed_vs(&hovered, &base, dock_rect, 8);
    assert!(
        changed > 40,
        "HOVER RENDERS NOTHING: only {changed} px in {dock_rect:?} differ from the un-hovered \
         base while hovering the dock item (expected the `dock-item:hover` background/colour \
         swap). hover_index IS Some(0) (state is set) but no pixels change. ROOT CAUSE: \
         dom_sync injects a `.hovered` CLASS (dom_sync.rs:393) but the theme styles the `:hover` \
         PSEUDO-class (`dock-item:hover`, liquid_glass.css:368) — no `dock-item.hovered` rule \
         matches — and no `set_dock_hover` sets PseudoStateFlags::HOVER on the render path (cf. \
         set_launcher_hover, desktop_dom.rs:605). FIXER: liquide-shell."
    );
}

// ===========================================================================
// 3. DOCK HOVER — STATE IS IDEMPOTENT UNDER REPEATED MOVES (teeth: re-sending
//    the same pointer position must not toggle hover state or cursor shape).
// ===========================================================================

#[test]
fn dock_hover_state_idempotent_under_repeated_moves() {
    let (cx, cy) = first_dock_item_centre();
    let (_f, (hover_idx, cursor)) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |handle| {
            // Send the SAME hover position three times in a row, as a real
            // pointer-still stream of identical Move events would.
            ScriptedScenario::new(handle)
                .pointer_move(cx, cy)
                .pointer_move(cx, cy)
                .pointer_move(cx, cy)
                .into_events()
        },
        |shell| (shell.dock().hover_index(), shell.cursor_shape()),
    )
    .expect("repeated-hover capture");

    assert_eq!(
        hover_idx,
        Some(0),
        "after repeated identical hover moves the dock hover index is {hover_idx:?}, expected \
         Some(0) — hover state must latch idempotently, not toggle off on a repeat move."
    );
    // The cursor over a dock item is a Pointer; repeated moves must not flip it.
    assert_eq!(
        format!("{cursor:?}"),
        "Pointer",
        "cursor shape over a hovered dock item is {cursor:?}, expected Pointer — repeated \
         identical moves must not oscillate the cursor shape."
    );
}

// ===========================================================================
// 4. DOCK HOVER — ENTER THEN LEAVE CLEANLY REVERTS (no residual highlight).
//    Move onto the item, then move far away onto empty wallpaper; the dock icon
//    must read back to the un-hovered base (no stuck hover paint).
// ===========================================================================

#[test]
fn dock_hover_leave_reverts_cleanly() {
    let (cx, cy) = first_dock_item_centre();
    let item = first_dock_item_rect();
    let base = base_desktop();

    let (frame, hover_idx) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |handle| {
            ScriptedScenario::new(handle)
                .pointer_move(cx, cy) // enter
                .pointer_move(20.0, 360.0) // leave, far away on empty wallpaper
                .into_events()
        },
        |shell| shell.dock().hover_index(),
    )
    .expect("enter-leave capture");

    // STATE: hover cleared.
    assert_eq!(
        hover_idx, None,
        "after leaving the dock, hover_index is {hover_idx:?}, expected None — a clean leave \
         must clear the hover state (else a residual highlight stays painted)."
    );

    // PIXELS: the dock icon reverted to the un-hovered base.
    let dock_rect = (item.0, item.1, item.2, item.3);
    let residual = changed_vs(&frame, &base, dock_rect, 8);
    assert!(
        residual < (item.2 * item.3) as usize / 20,
        "RESIDUAL HOVER HIGHLIGHT: {residual} px in the dock icon rect {dock_rect:?} still \
         differ from the un-hovered base after the pointer left (expected ~0). The hover \
         highlight did not cleanly revert."
    );
}

// ===========================================================================
// 5. DOCK HOVER TOOLTIP — would-be FLICKER ON A STEADY HOVER.
//
// THEORY: with the pointer held MOTIONLESS on a dock item, the canonical
// `TooltipManager` is advanced by `frame_delta_ms` every render
// (`Shell::sync_tooltip_manager`, tooltip_adapter.rs:122), running
// Pending → FadingIn → Visible → (after display_duration) FadingOut → Hidden.
// If the tooltip painted, its opacity would change frame-to-frame with the
// per-frame delta (no pointer movement) = flicker.
//
// ACTUAL (t66 verdict): the tooltip NEVER paints (0 px at all deltas), so this
// PASSES VACUOUSLY. Kept as the stability tooth that would catch the fade
// oscillation once the tooltip-render bug is fixed; the auto-hide flash facet is
// asserted (and currently FAILS at the "must paint" precondition) in test 6.
// ===========================================================================

#[test]
fn dock_hover_tooltip_steady_is_stable_during_fade() {
    let item = first_dock_item_rect();
    let tip = tooltip_region(item);

    // Two frames whose accumulated tooltip time both land inside the fade window
    // but at different opacities: one just into fade-in, one near its end. On a
    // real steady hover the per-frame delta varies, so successive frames sit at
    // different opacities — exactly this difference.
    //
    // A single `TooltipManager::update(dt)` advances at most one lifecycle state,
    // so to land mid-fade we drive the delta to (show_delay + part-of-fade): the
    // first render pass crosses Pending→FadingIn, leaving the manager partway
    // through fade-in proportional to the overrun.
    let f_low = dock_hover_frame(TT_SHOW_DELAY_MS + TT_FADE_IN_MS * 0.25); // ~ low opacity
    let f_high = dock_hover_frame(TT_SHOW_DELAY_MS + TT_FADE_IN_MS * 0.90); // ~ high opacity

    let differ = region_diff(&f_low, &f_high, tip, 6);
    assert_eq!(
        differ, 0,
        "TOOLTIP FLICKER (fade): the dock hover tooltip region {tip:?} differs by {differ} px \
         between two consecutive steady-hover frames whose only difference is the per-frame \
         animation delta. The pointer never moved, yet the tooltip's opacity (and paint) changes \
         frame-to-frame. ROOT CAUSE: Shell::sync_tooltip_manager(frame_delta_ms) advances the \
         TooltipManager fade-in/out every frame (crates/liquide-shell/src/tooltip_adapter.rs:122; \
         driven from crates/liquide-shell/src/shell/dom_sync.rs:917)."
    );
}

// ===========================================================================
// 6. DOCK HOVER TOOLTIP — AUTO-HIDE FLASH-OFF ON A STEADY HOVER (the second
//    facet of the finding).
//
// The default tooltip `display_duration_ms` is 5000: after being shown the
// tooltip auto-hides EVEN THOUGH the cursor is still on the item. So a frame
// taken while the tooltip is up vs a later frame past the display duration show
// the tooltip present, then GONE — a flash-off with no input change.
//
// We compare a "tooltip-up" frame against the un-hovered base in the tooltip
// region (it should paint) AND assert that the same steady hover, advanced past
// the display duration, KEEPS painting. It does not (it auto-hides), so this
// FAILS.
// ===========================================================================

#[test]
fn dock_hover_tooltip_does_not_auto_hide_while_hovered() {
    let item = first_dock_item_rect();
    let tip = tooltip_region(item);
    let base = base_desktop();

    // Frame A: tooltip shown (Pending→FadingIn→Visible reached within the
    // capture's render passes at this delta — mirrors the proven `tooltip_shown`
    // scenario, which uses 800 ms).
    let shown = dock_hover_frame(800.0);
    let shown_paint = changed_vs(&shown, &base, tip, 10);

    // Precondition / teeth: the tooltip actually painted while hovering. If it
    // never paints, this test cannot judge auto-hide — surface that explicitly.
    assert!(
        shown_paint > 30,
        "TOOLTIP RENDERS NOTHING: only {shown_paint} px in {tip:?} differ from base on a steady \
         dock hover (swept 50–6000 ms, all 0 — see diag_hover_paint_sweep). The dock-hover \
         tooltip never surfaces on the capture render, so the auto-hide flicker cannot even be \
         evaluated. ROOT CAUSE: the dock hover-render path is unwired (see module verdict / \
         test 2). The would-be auto-hide flash (TooltipConfig::display_duration_ms = 5000, \
         config.rs:39 → Visible→FadingOut in manager.rs:236) is moot until the tooltip paints. \
         FIXER: liquide-shell."
    );

    // Frame B: the SAME steady hover, but the animation clock advanced well past
    // the display duration. A correct tooltip stays up while the cursor is on the
    // item; the buggy one auto-hides. To push the manager past Visible→FadingOut
    // →Hidden we need several updates; we approximate one large stride here. If
    // the tooltip is still painted (no auto-hide), this region matches Frame A;
    // if it flashed off, it collapses toward the bare base.
    let later = dock_hover_frame(TT_DISPLAY_DURATION_MS + 1000.0);
    let later_paint = changed_vs(&later, &base, tip, 10);

    assert!(
        later_paint >= shown_paint / 2,
        "TOOLTIP AUTO-HIDE FLASH: a steady dock hover painted {shown_paint} px of tooltip, but \
         after the animation clock advanced past the 5000 ms display-duration the tooltip region \
         dropped to {later_paint} px — the tooltip flashed OFF while the cursor never moved. \
         ROOT CAUSE: TooltipConfig::display_duration_ms = 5000 (crates/liquide-tooltip/src/\
         config.rs:39) drives Visible→FadingOut in TooltipManager::update \
         (crates/liquide-tooltip/src/manager.rs:236), advanced every frame by \
         Shell::sync_tooltip_manager (tooltip_adapter.rs:122). A hover tooltip must persist \
         while hovered."
    );
}

// ===========================================================================
// 7. HOVER vs UNDERLYING — hovering a dock item must not corrupt neighbours.
//    The status bar (top band) and a neighbouring wallpaper strip must be
//    byte-identical hovered vs not, except for the intended dock/tooltip change.
// ===========================================================================

#[test]
fn dock_hover_does_not_disturb_status_bar() {
    let base = base_desktop();
    let hovered = dock_hover_frame(100.0); // tooltip still Pending; only dock swap

    // The status bar lives in the top 34 px; hovering the bottom dock must not
    // touch it.
    let bar = (0u32, 0u32, base.width, 34u32);
    let differ = region_diff(&hovered, &base, bar, 4);
    assert!(
        differ < 64,
        "HOVER CORRUPTION: hovering a dock item changed {differ} px in the status-bar band \
         {bar:?} (expected ~0). A bottom-dock hover must not repaint/flicker the top bar."
    );
}

// ===========================================================================
// 8. SESSION-MENU ITEM HOVER — STEADY HIGHLIGHT IS STABLE.
//
// The session menu's item highlight is driven by `session_menu_hover_index`
// (a state swap, no transition). Open the menu and set a steady hovered item
// via keyboard nav, render twice, assert the menu region is byte-identical.
// A `menu-item:hover` swap must not oscillate while the selection is held.
// ===========================================================================

fn session_menu_frame_with_nav(down_presses: usize) -> Frame {
    fn key(code: KeyCode) -> PlatformEvent {
        PlatformEvent::KeyInput {
            handle: NativeWindowHandle(1),
            event: KeyEvent::new(code, KeyState::Pressed, Modifiers::new(), 0, 0),
        }
    }
    capture_desktop_scripted_with(
        &scenario_options(THEME),
        |_h| Vec::new(),
        move |shell| {
            if !shell.session_menu_visible() {
                shell.toggle_session_menu();
            }
            // Drive the highlight to a steady item via keyboard hover-nav.
            for _ in 0..down_presses {
                shell.handle_platform_event(&key(KeyCode::ArrowDown));
            }
        },
    )
    .expect("session-menu hover capture")
}

#[test]
fn session_menu_item_hover_steady_is_byte_stable() {
    // NOTE (t66 verdict): PASSES VACUOUSLY — the menu highlight never paints
    // (`menu-item.selected` has no CSS rule; `set_menu_hover` is never called on
    // the render path). Kept as a stability tooth for after the fix. The
    // companion `session_menu_item_hover_moves_highlight` FAILS, proving the
    // highlight is unpainted (so this stability check is currently vacuous).
    let a = session_menu_frame_with_nav(1);
    let b = session_menu_frame_with_nav(1);
    // Compare the whole frame: the menu + its highlighted item must be identical
    // across two identical renders (the highlight is a pure state swap).
    let differ = diff_frames(&a, &b, DiffOptions::exact()).differing_pixels;
    assert_eq!(
        differ, 0,
        "SESSION-MENU HOVER FLICKER: {differ} pixels differ across two identical renders of the \
         session menu with the same item highlighted. A held `menu-item:hover`/selection must be \
         byte-stable."
    );
}

#[test]
fn session_menu_item_hover_moves_highlight() {
    // Teeth for the stability test: navigating to a DIFFERENT item must actually
    // change the painted highlight (so "byte-stable" isn't a no-op blank menu).
    let one = session_menu_frame_with_nav(1);
    let two = session_menu_frame_with_nav(2);
    let differ = diff_frames(&one, &two, DiffOptions::default()).differing_pixels;
    assert!(
        differ > 50,
        "MENU HIGHLIGHT RENDERS NOTHING: only {differ} px differ between item-1 and item-2 \
         selection in the session menu. The keyboard-nav/hover highlight is not painted. ROOT \
         CAUSE: dom_sync writes a `.selected` CLASS (dom_sync.rs:856) but the theme styles \
         `menu-item:hover` (liquid_glass.css:583) — no `menu-item.selected` rule — and \
         `set_menu_hover` (desktop_dom.rs:657) is never called on the render path. (The launcher \
         works because it has BOTH a `.selected` rule AND set_launcher_hover.) FIXER: liquide-shell."
    );
}

// ===========================================================================
// 9. SANITY: a fully steady hover at a delta in the tooltip's VISIBLE PLATEAU
//    is reproducible (two captures at the SAME plateau delta match). This is the
//    positive control: WHERE the animation is not mid-transition, hover IS
//    stable — proving the flicker in tests 5/6 is specifically the fade/auto-
//    hide transition, not global nondeterminism.
// ===========================================================================

#[test]
fn dock_hover_same_delta_is_reproducible_everywhere() {
    // Same delta twice -> identical frames, full-frame exact. This must always
    // hold (renderer determinism); if it fails, the harness/renderer is the
    // problem, not hover specifically.
    let a = dock_hover_frame(800.0);
    let b = dock_hover_frame(800.0);
    let differ = diff_frames(&a, &b, DiffOptions::exact()).differing_pixels;
    assert_eq!(
        differ, 0,
        "NON-DETERMINISTIC RENDER: two captures of the identical steady hover at the same \
         frame_delta_ms differ by {differ} px. Determinism (t65) is violated; the per-frame \
         flicker findings must be interpreted against this."
    );
}

// Temporary diagnostic (ignored): sweep tooltip deltas + dock-hover deltas and
// report tooltip-region / dock-icon paint vs base, to confirm whether ANY hover
// pixels ever appear. Run: cargo test ... -- --ignored diag_hover_paint_sweep
#[test]
#[ignore]
fn diag_hover_paint_sweep() {
    let item = first_dock_item_rect();
    let tip = tooltip_region(item);
    let dock_rect = (item.0, item.1, item.2, item.3);
    let base = base_desktop();
    eprintln!("item rect = {item:?}, tip region = {tip:?}");
    for d in [50.0, 200.0, 510.0, 600.0, 660.0, 800.0, 1200.0, 5000.0, 6000.0] {
        let f = dock_hover_frame(d);
        let tip_px = changed_vs(&f, &base, tip, 8);
        let dock_px = changed_vs(&f, &base, dock_rect, 8);
        eprintln!("delta={d:>7.0}  dock_icon_changed={dock_px:>5}  tooltip_changed={tip_px:>5}");
    }
}

// Bridge a click helper so unused-import lints stay quiet if the suite evolves.
#[allow(dead_code)]
fn _press(handle: NativeWindowHandle, x: f32, y: f32) -> PlatformEvent {
    PlatformEvent::MouseInput {
        handle,
        event: MouseEvent::Button {
            button: MouseButton::Left,
            state: ButtonState::Pressed,
            x,
            y,
        },
    }
}
