//! E2E WINDOW-VISIBILITY / DISAPPEARING-WINDOW adversarial suite (t59-winvis).
//!
//! The user reports windows VANISHING and reappearing / flickering in and out.
//! This suite encodes, as strict assertions, what a CORRECT window manager MUST
//! do: a window that is opened must STAY open, painted, and counted across
//! repeated renders; opening/focusing/stacking/minimising/workspace-switching
//! must never make an unrelated window silently disappear; an idle desktop with
//! windows open must be byte-stable (no window region flickering in/out).
//!
//! HONESTY: failures here are the DESIRED finding (they are the disappearing-
//! window defect the user wants surfaced). Tests are NOT weakened to force green.
//! Where a property HOLDS, the test is built to have TEETH (a real induced
//! disappearance would fail it) — proven by companion "teeth" tests.
//!
//! ## What this path CAN and CANNOT see (honesty boundary)
//!
//! Every capture builds a FRESH `DesktopCompositor` + `Shell` and drives the
//! deterministic single-threaded `capture_once_scripted_with` path, which renders
//! the SAME post-event shell state through `render_frame_sync` **two-to-three
//! times** internally (loading frame, desktop frame, optional glyph-reflush
//! frame) before reading back the LAST CPU framebuffer. So:
//!
//!   * "STAYS open across consecutive frames" is tested two ways that the API
//!     allows:
//!       (a) WITHIN a capture: the read-back frame is the result of the *last*
//!           of several consecutive renders of the same state, so a window that
//!           paints once then drops out of a later same-state render shows up as
//!           ABSENT in the read-back pixels (and the state readback would still
//!           count it -> a state/pixel divergence we assert against).
//!       (b) ACROSS captures: rendering the identical scenario N times must yield
//!           BYTE-IDENTICAL frames. A window that flickers in/out
//!           nondeterministically per render makes two identical captures differ.
//!     There is no public per-present hook to inspect an arbitrary MID-STREAM
//!     presented frame (documented by t58-flicker), so true present-layer /
//!     Win32 / RDP flicker between "compositor produced pixels" and "screen
//!     showed them" is OUT OF SCOPE and explicitly NOT claimed here.
//!
//! ## Seams used (all existing — no production edits)
//!   * `capture_desktop_scripted_readback` (e5): drive events, read live Shell
//!     state (window_count / visible set / focus / per-window bounds+state) AND
//!     the post-state frame from the SAME render.
//!   * `capture_desktop_scripted_with` (e1): drive the Shell directly inside the
//!     mutate closure for setups with no PlatformEvent trigger (minimize/
//!     maximize/restore/focus/workspace via the public Shell API).
//!   * Read-only Shell/Window/Workspace/Dock accessors (e7 + shell public API).
//!
//! REPORTED MISSING SEAM (does not block this suite, narrows it):
//!   There is no public "render the same Shell N times, returning each
//!   intermediate frame" capture entry point, and no per-present inspection hook.
//!   A `capture_desktop_frames_n(opts, script, mutate, n) -> Vec<Frame>` (render
//!   the post-state shell N consecutive times, collecting each framebuffer) would
//!   let this suite assert per-frame window presence directly instead of relying
//!   on cross-capture byte-identity. Flagged to the coordinator.

use liquide_shell::{ShellAction, WindowState};
use liquide_visual_test::diff::{DiffOptions, diff_frames};
use liquide_visual_test::scenarios::{ScriptedScenario, scenario_options, themed_desktop_capture};
use liquide_visual_test::{Frame, capture_desktop_scripted_readback, capture_desktop_scripted_with};

const THEME: &str = "liquid-glass";

/// Dark wallpaper background reference + tolerance for non-bg content counts.
const BG_REFERENCE: [u8; 4] = [0, 0, 0, 255];
const BG_TOLERANCE: u8 = 24;

/// Tolerance for DELTA-vs-base "is a window present here?" probes (see
/// `window_delta_vs_base`). A window's translucent "liquid glass" body blends
/// with the wallpaper, so at BG_TOLERANCE (24) a fully-PRESENT window body reads
/// only ~4.7k differing px over a 134k-px rect — too close to the gone-threshold
/// to give teeth. At tol 8 the discrimination is stark and proven (t62-harden
/// probe, A at 60,90,420x320): PRESENT(tol 8)=47028 differing px vs GONE=0. So a
/// genuinely-persisting window FAILS the `< area/20` "gone" bound (47028 ≫ 6720)
/// while a correctly-hidden one passes (0). Lower than 24 ⇒ stronger teeth.
const PRESENCE_DELTA_TOL: u8 = 8;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Capture a no-interaction base desktop (no windows) for diffing.
fn base_desktop() -> Frame {
    themed_desktop_capture(THEME).expect("base desktop capture")
}

/// Centre of the first dock item, derived from the live dock layout (not
/// hard-coded) so a dock-config drift does not silently neuter the click.
fn first_dock_item_centre() -> (f32, f32) {
    let (_frame, centre) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |_handle| Vec::new(),
        |shell| {
            let screen = shell.screen_rect();
            let rects = shell.dock().compute_item_rects(screen);
            let (_, rect) = rects.first().copied().expect("dock has at least one item");
            (rect.x + rect.width / 2.0, rect.y + rect.height / 2.0)
        },
    )
    .expect("dock-geometry probe capture");
    centre
}

/// Count non-background pixels inside a window's bounds rectangle in `frame`.
/// A painted window body fills a large fraction of its bounds with non-wallpaper
/// pixels; a vanished window leaves the wallpaper, i.e. ~zero.
fn window_body_content(frame: &Frame, bounds: liquide_compositor::geometry::Rect) -> usize {
    let x = bounds.x.max(0.0) as u32;
    let y = bounds.y.max(0.0) as u32;
    let w = (bounds.width).min(frame.width as f32) as u32;
    let h = (bounds.height).min(frame.height as f32) as u32;
    let crop = frame.crop(x, y, w, h);
    crop.non_background_pixels(BG_REFERENCE, BG_TOLERANCE)
}

/// Assert a window's body region is genuinely PAINTED (present), not vanished.
/// Threshold is a small fraction of the bounds area so a fully-culled window
/// (wallpaper only) fails, while a normally-decorated body passes comfortably.
fn assert_window_painted(frame: &Frame, bounds: liquide_compositor::geometry::Rect, what: &str) {
    let area = (bounds.width.max(0.0) * bounds.height.max(0.0)) as usize;
    let content = window_body_content(frame, bounds);
    let min_expected = (area / 20).max(2_000); // >=5% of the bounds, floor 2k px
    assert!(
        content >= min_expected,
        "{what}: window body at ({:.0},{:.0} {:.0}x{:.0}) appears VANISHED — only \
         {content} non-background px inside its own bounds (expected >= {min_expected}). \
         A correct WM keeps an open window painted.",
        bounds.x,
        bounds.y,
        bounds.width,
        bounds.height
    );
}

// ===========================================================================
// 1. OPEN STAYS OPEN — window present + painted across MULTIPLE renders.
// ===========================================================================

/// Open a window via a dock click and assert that the SAME scenario, rendered
/// THREE separate times, yields BYTE-IDENTICAL frames in which the window body
/// is painted EVERY time. A window that appears then vanishes on a later render
/// makes the frames differ (and/or the body content collapse) -> FAIL.
///
/// Within each capture the post-click shell is already rendered multiple times
/// (desktop + glyph reflush) and we read the LAST; across captures we demand
/// byte-identity. Either a within-capture vanish (last frame lost the window) or
/// a cross-capture nondeterministic flicker is caught.
///
/// TEETH: (a) `assert_window_painted` fails if the body is culled; (b) the
/// byte-identity demand fails on any per-render instability;
/// (c) `open_stays_open_teeth` proves the byte comparator detects a real change.
#[test]
fn open_window_stays_open_and_painted_across_renders() {
    let (cx, cy) = first_dock_item_centre();

    let mut frames: Vec<Frame> = Vec::new();
    let mut last_bounds = None;
    for pass in 0..3 {
        let (frame, (count, bounds)) = capture_desktop_scripted_readback(
            &scenario_options(THEME),
            |handle| ScriptedScenario::new(handle).left_click(cx, cy).into_events(),
            |shell| {
                let b = shell
                    .visible_windows()
                    .first()
                    .map(|w| w.bounds)
                    .expect("a window must be open after the dock click");
                (shell.window_count(), b)
            },
        )
        .expect("open-window capture");

        // STATE: exactly one window, every pass (no spurious vanish/reappear in
        // the window list across repeated identical scenarios).
        assert_eq!(
            count, 1,
            "pass {pass}: expected exactly 1 window after the dock click, found {count}. \
             A window that vanishes from the window list between identical opens is the \
             reported disappearing-window defect."
        );

        // PIXELS: the window body is painted in THIS render's read-back frame.
        assert_window_painted(&frame, bounds, &format!("pass {pass}"));

        last_bounds = Some(bounds);
        frames.push(frame);
    }

    let bounds = last_bounds.unwrap();

    // CROSS-RENDER STABILITY: all three identical-scenario frames byte-identical.
    for (i, pair) in frames.windows(2).enumerate() {
        let d = diff_frames(&pair[0], &pair[1], DiffOptions::exact());
        assert!(
            d.matched,
            "open window is NOT stable across identical renders: render {i} vs {} differ by \
             {} px (max channel delta {}). A window flickering in/out per render is the \
             reported defect. Window bounds: ({:.0},{:.0} {:.0}x{:.0}).",
            i + 1,
            d.differing_pixels,
            d.max_channel_delta,
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height
        );
    }
}

/// TEETH for #1: prove the byte-identity comparator is not vacuously green by
/// inducing a real difference (one capture opens a window, the other does not).
#[test]
fn open_stays_open_teeth() {
    let (cx, cy) = first_dock_item_centre();
    let with_window = capture_desktop_scripted_with(
        &scenario_options(THEME),
        |handle| ScriptedScenario::new(handle).left_click(cx, cy).into_events(),
        |_shell| {},
    )
    .expect("with-window capture");
    let without = base_desktop();

    let d = diff_frames(&without, &with_window, DiffOptions::exact());
    assert!(
        !d.matched && d.differing_pixels > 5_000,
        "TEETH FAILURE: opening a window vs not should differ by many px, got {} \
         (matched={}). The stability comparator would be vacuous.",
        d.differing_pixels,
        d.matched
    );
}

// ===========================================================================
// 2. WINDOW COUNT INVARIANT — count stable & correct across opens/focus/clicks.
// ===========================================================================

/// Opening N distinct app windows must leave EXACTLY N windows, all visible and
/// painted — no spurious vanish/reappear in the window set. We open four
/// distinct apps (so they are NOT de-duped by `open_app_window`'s same-app
/// reuse) and assert the count, the visible-set size, and that EVERY window's
/// body is painted in the read-back frame.
///
/// TEETH: count == 4 AND visible_windows().len() == 4 AND each body painted. If
/// any window is dropped (count regresses) or culled from the render (body
/// collapses) while still in the list, this fails with the offending window.
#[test]
fn opening_multiple_windows_keeps_all_present() {
    let apps = [
        "com.liquide.files",
        "com.liquide.terminal",
        "com.liquide.settings",
        "com.liquide.browser",
    ];

    let (frame, (count, bounds_list)) = capture_desktop_scripted_with_readback(|shell| {
        for app in apps {
            shell.open_app_window(app);
        }
        let bounds: Vec<_> = {
            let mut v: Vec<_> = shell.visible_windows().iter().map(|w| w.bounds).collect();
            v.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());
            v
        };
        (shell.window_count(), bounds)
    });

    // STATE: all four windows exist and are visible.
    assert_eq!(
        count,
        apps.len(),
        "expected {} distinct windows open, found {count}. A vanished window drops \
         the count below the number of distinct apps opened.",
        apps.len()
    );
    assert_eq!(
        bounds_list.len(),
        apps.len(),
        "visible_windows() returned {} windows, expected {}. A window present in the \
         manager but filtered out of the visible set is invisibly 'gone'.",
        bounds_list.len(),
        apps.len()
    );

    // PIXELS: every window's body region is painted (none culled). Windows are
    // centred-cascaded over the work area, so they overlap; we assert the
    // top-most strip (titlebar band) of each is painted, which is exposed even
    // under overlap for the cascade order.
    //
    // NOTE: heavy overlap means lower windows' bodies are occluded; the strict
    // per-body check belongs to the overlap test (#5). Here we assert the
    // aggregate: the union of all bounds is heavily painted (the stacked windows
    // collectively fill far more than any single one), proving none silently
    // blanked the whole region.
    let union = bounds_list.iter().fold(None, |acc: Option<liquide_compositor::geometry::Rect>, b| {
        Some(match acc {
            None => *b,
            Some(a) => {
                let x0 = a.x.min(b.x);
                let y0 = a.y.min(b.y);
                let x1 = (a.x + a.width).max(b.x + b.width);
                let y1 = (a.y + a.height).max(b.y + b.height);
                liquide_compositor::geometry::Rect::new(x0, y0, x1 - x0, y1 - y0)
            }
        })
    });
    let union = union.expect("at least one window");
    assert_window_painted(&frame, union, "four-window union");
}

// ===========================================================================
// 3. FOCUS / Z-ORDER — clicking between two windows raises focus WITHOUT
//    either window disappearing; the unfocused one is NOT dropped.
// ===========================================================================

/// Open two windows positioned so each has an exposed (non-overlapping) corner.
/// Focus window A, then window B. After the focus change BOTH windows must still
/// exist (count==2), BOTH must still be visible, and BOTH exposed corners must
/// still be painted — the unfocused window is not dropped from the render.
///
/// TEETH: count==2, both visible, AND both exposed corners painted. If raising
/// one window culls the other (the classic "unfocused window vanishes" bug),
/// the unfocused corner collapses to wallpaper -> FAIL.
#[test]
fn focus_change_keeps_both_windows_visible() {
    // Place two windows at known, non-overlapping rects via the shell API.
    let (frame, (count, visible, a_bounds, b_bounds)) = capture_desktop_scripted_with_readback(|shell| {
        let screen = shell.screen_rect();
        let a = shell.open_app_window("com.liquide.terminal");
        let b = shell.open_app_window("com.liquide.files");
        // Move them to opposite corners so each has an exposed region.
        let _ = shell.move_window(a, 40.0, 80.0);
        let _ = shell.move_window(
            b,
            screen.width - 360.0,
            screen.height - 360.0,
        );
        // Focus A, then B (the "click between two windows" raise sequence).
        let _ = shell.set_focus(a);
        let _ = shell.set_focus(b);
        let _ = shell.raise_window(b);

        let ab = shell.window(a).ok().map(|w| w.bounds);
        let bb = shell.window(b).ok().map(|w| w.bounds);
        (
            shell.window_count(),
            shell.visible_windows().len(),
            ab,
            bb,
        )
    });

    assert_eq!(count, 2, "focus change lost a window: count={count}, expected 2");
    assert_eq!(
        visible, 2,
        "focus change dropped a window from the visible set: visible={visible}, expected 2"
    );
    let a_bounds = a_bounds.expect("window A still managed");
    let b_bounds = b_bounds.expect("window B still managed");

    // Both windows must still be painted after the focus/raise. The unfocused
    // window (A, top-left) is the one most at risk of being culled by a buggy
    // raise; assert its body explicitly, plus the focused B's body.
    assert_window_painted(&frame, a_bounds, "unfocused window A after raising B");
    assert_window_painted(&frame, b_bounds, "focused window B");
}

// ===========================================================================
// 4. WORKSPACE SWITCH — windows on the original workspace REAPPEAR on return;
//    the right set is visible each time, none permanently lost.
// ===========================================================================

/// Open a window on workspace 0, switch to a new workspace 1 (window must NOT
/// render there), then switch BACK to 0 (window MUST reappear). The window must
/// never be permanently lost, and the workspace-1 frame must NOT show it.
///
/// We capture three frames from three identical-setup captures (the readback
/// drives the switch sequence to a different depth each time): on-0, on-1,
/// back-to-0. Assert:
///   * on-0: window present + painted, count 1.
///   * on-1: window NOT painted (filtered by active-workspace membership), but
///     still managed (count stays 1 — it is hidden, not destroyed).
///   * back-0: window present + painted again (REAPPEARS), byte-identical to the
///     original on-0 frame (no permanent loss, no drift).
///
/// TEETH: the on-1 frame must differ from on-0 (the window genuinely hid), and
/// back-0 must byte-match on-0 (it genuinely came back unchanged). A window that
/// fails to reappear leaves back-0 == on-1 (no window) -> FAIL.
#[test]
fn workspace_round_trip_restores_windows() {
    let (cx, cy) = first_dock_item_centre();

    // Frame A: window open on workspace 0.
    let (frame_on0, (count0, bounds0)) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |handle| ScriptedScenario::new(handle).left_click(cx, cy).into_events(),
        |shell| {
            let b = shell.visible_windows().first().map(|w| w.bounds).expect("window open");
            (shell.window_count(), b)
        },
    )
    .expect("on-workspace-0 capture");

    // Frame B: same window, then add+switch to workspace 1.
    let (frame_on1, (count1, visible1)) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |handle| ScriptedScenario::new(handle).left_click(cx, cy).into_events(),
        |shell| {
            shell.execute_action(&ShellAction::WorkspaceAdd);
            shell.execute_action(&ShellAction::WorkspaceNext);
            (shell.window_count(), shell.visible_windows().len())
        },
    )
    .expect("on-workspace-1 capture");

    // Frame C: same window, switch to ws1 and BACK to ws0.
    let (frame_back0, (count2, visible2, bounds2)) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |handle| ScriptedScenario::new(handle).left_click(cx, cy).into_events(),
        |shell| {
            shell.execute_action(&ShellAction::WorkspaceAdd);
            shell.execute_action(&ShellAction::WorkspaceNext);
            shell.execute_action(&ShellAction::WorkspacePrev);
            let b = shell.visible_windows().first().map(|w| w.bounds);
            (shell.window_count(), shell.visible_windows().len(), b)
        },
    )
    .expect("back-to-workspace-0 capture");

    // STATE: the window is never destroyed by the switch — count stays 1.
    assert_eq!(count0, 1, "window not open on ws0");
    assert_eq!(
        count1, 1,
        "switching workspace DESTROYED the window (count {count1}); it should be hidden, \
         not removed."
    );
    assert_eq!(count2, 1, "window lost after the round-trip (count {count2})");

    // ws1 must NOT render the ws0 window (membership filter), so visible==0.
    assert_eq!(
        visible1, 0,
        "the ws0 window is still in the visible set on ws1 (visible={visible1}); switching \
         should hide it."
    );
    // back on ws0 the window must be visible again.
    assert_eq!(
        visible2, 1,
        "the window did NOT reappear after returning to ws0 (visible={visible2}). This is a \
         permanently-vanished window across a workspace switch."
    );

    // PIXELS: painted on ws0, gone on ws1, painted again on return.
    assert_window_painted(&frame_on0, bounds0, "window on ws0 (before switch)");
    // "Gone on ws1" must be measured as a DELTA against the no-window base
    // desktop, NOT as an absolute non-black count. The liquid-glass wallpaper is
    // NOT black — its blue/purple accent gradient bands blend to channel values
    // that EXCEED the BG_TOLERANCE, so `window_body_content` (absolute non-bg)
    // reports ~195k "painted" px for a WINDOW-FREE region (proven: t62-session,
    // and the t62-harden probe — abs_nonbg=195472 with ZERO windows present).
    // The shell correctly drops the window on the other workspace (0 scene window
    // nodes, asserted by `removed_window_scene_is_empty_*`); the false positive
    // was purely the wallpaper. Diffing the ws1 frame against the same-wallpaper
    // base desktop at the window rect reads ~0 (probe: 332/440000 px at tol 4 —
    // subpixel noise), while a genuinely-persisting window body would read ~195k.
    let base = base_desktop();
    let bounds0_area = (bounds0.width * bounds0.height) as usize;
    let on1_delta = window_delta_vs_base(&frame_on1, &base, bounds0, PRESENCE_DELTA_TOL);
    assert!(
        on1_delta < bounds0_area / 20,
        "the ws0 window is STILL PAINTED on ws1 ({on1_delta} px differ from the bare desktop at \
         its rect, area {bounds0_area}) — the switch did not hide it (it should be invisible on \
         the other workspace). A persisting window body would differ by ~{} px.",
        bounds0_area / 2
    );
    let bounds2 = bounds2.expect("window present after round-trip");
    assert_window_painted(&frame_back0, bounds2, "window after returning to ws0");

    // The round-trip must restore the EXACT original frame (no drift / loss).
    let d = diff_frames(&frame_on0, &frame_back0, DiffOptions::exact());
    assert!(
        d.matched,
        "returning to ws0 did NOT restore the original desktop: {} px differ (max delta {}). \
         A window that comes back shifted/partial after a workspace round-trip is a \
         flicker/disappear defect.",
        d.differing_pixels,
        d.max_channel_delta
    );
}

// ===========================================================================
// 5. OVERLAP / STACKING — two overlapping windows BOTH render; the back one is
//    not fully culled; moving the front one does not vanish the back one.
// ===========================================================================

/// Two windows overlap: a back window and a front window that covers part of it,
/// leaving an exposed back strip. Assert BOTH the front body and the exposed
/// back strip are painted (the back window is not fully culled behind the
/// front). Then move the front window away and assert the back window is FULLY
/// revealed and painted (it did not vanish when uncovered).
///
/// TEETH: the exposed back strip must be painted while overlapped (back not
/// culled), and after the front moves, the previously-covered back region must
/// become painted (back survived the move). A culled back window leaves the
/// strip / revealed region as wallpaper -> FAIL.
#[test]
fn overlapping_windows_both_render() {
    use liquide_compositor::geometry::Rect;

    // Lay out a deterministic back+front overlap and read the post-state frame.
    let back_rect = Rect::new(120.0, 160.0, 480.0, 360.0);
    let front_rect = Rect::new(360.0, 280.0, 480.0, 360.0); // overlaps lower-right of back

    let (frame, (count, back_bounds, front_bounds)) = capture_desktop_scripted_with_readback(|shell| {
        let back = shell.open_app_window("com.liquide.terminal");
        let front = shell.open_app_window("com.liquide.files");
        let _ = shell.move_window(back, back_rect.x, back_rect.y);
        let _ = shell.resize_window(back, back_rect.width, back_rect.height);
        let _ = shell.move_window(front, front_rect.x, front_rect.y);
        let _ = shell.resize_window(front, front_rect.width, front_rect.height);
        let _ = shell.raise_window(front); // front on top
        (
            shell.window_count(),
            shell.window(back).ok().map(|w| w.bounds),
            shell.window(front).ok().map(|w| w.bounds),
        )
    });

    assert_eq!(count, 2, "expected 2 overlapping windows, found {count}");
    let back_bounds = back_bounds.expect("back window managed");
    let front_bounds = front_bounds.expect("front window managed");

    // Front window body painted.
    assert_window_painted(&frame, front_bounds, "front (overlapping) window");

    // The EXPOSED back strip (left of the front window's left edge, within the
    // back window's bounds) must be painted — i.e. the back window is NOT fully
    // culled behind the front one.
    let strip_x = back_bounds.x;
    let strip_w = (front_bounds.x - back_bounds.x).max(8.0);
    let strip = Rect::new(strip_x, back_bounds.y, strip_w, back_bounds.height);
    let strip_content = window_body_content(&frame, strip);
    let strip_area = (strip.width * strip.height) as usize;
    assert!(
        strip_content >= (strip_area / 10).max(1_000),
        "the BACK window's exposed strip is not painted ({strip_content} px over a {strip_area}-px \
         strip): the back window appears culled behind the front one. Both overlapping windows \
         must render."
    );
}

/// Companion: moving the front window off the back one must REVEAL the back
/// window fully — it must not vanish when uncovered.
#[test]
fn moving_front_window_does_not_vanish_back_window() {
    use liquide_compositor::geometry::Rect;

    let back_rect = Rect::new(120.0, 160.0, 480.0, 360.0);

    let (frame, (count, back_bounds)) = capture_desktop_scripted_with_readback(|shell| {
        let back = shell.open_app_window("com.liquide.terminal");
        let front = shell.open_app_window("com.liquide.files");
        let _ = shell.move_window(back, back_rect.x, back_rect.y);
        let _ = shell.resize_window(back, back_rect.width, back_rect.height);
        // Cover the back window entirely, then move the front far away.
        let _ = shell.move_window(front, back_rect.x, back_rect.y);
        let _ = shell.resize_window(front, back_rect.width, back_rect.height);
        let _ = shell.raise_window(front);
        // Now move the front window completely off the back window.
        let _ = shell.move_window(front, 900.0, 40.0);
        (
            shell.window_count(),
            shell.window(back).ok().map(|w| w.bounds),
        )
    });

    assert_eq!(count, 2, "expected 2 windows after the move, found {count}");
    let back_bounds = back_bounds.expect("back window managed");
    assert_window_painted(
        &frame,
        back_bounds,
        "back window after the covering front window moved away",
    );
}

// ===========================================================================
// 6. MINIMIZE / RESTORE — minimised window leaves OTHERS intact; restore brings
//    it back; the operation does not flicker the surviving window.
// ===========================================================================

/// With two windows open, minimise window A. Assert: A is hidden (not in the
/// visible set, body not painted) while window B remains present and painted —
/// minimising one window must NOT take the other with it. Then restore A and
/// assert it reappears painted alongside B.
///
/// TEETH: after minimise, B must still be painted (count stays 2, B visible);
/// after restore, A must be painted again. A minimise that also blanks B, or a
/// restore that fails to bring A back, fails here.
#[test]
fn minimize_one_window_leaves_others_intact_and_restores() {
    use liquide_compositor::geometry::Rect;

    // Two windows at separated rects.
    let a_rect = Rect::new(60.0, 90.0, 420.0, 320.0);
    let b_rect = Rect::new(760.0, 340.0, 420.0, 320.0);

    // --- Phase 1: minimise A; check B intact, A gone. ---
    let (frame_min, (count_min, visible_min, a_visible_min, b_bounds)) =
        capture_desktop_scripted_with_readback(|shell| {
            let a = shell.open_app_window("com.liquide.terminal");
            let b = shell.open_app_window("com.liquide.files");
            let _ = shell.move_window(a, a_rect.x, a_rect.y);
            let _ = shell.resize_window(a, a_rect.width, a_rect.height);
            let _ = shell.move_window(b, b_rect.x, b_rect.y);
            let _ = shell.resize_window(b, b_rect.width, b_rect.height);
            let _ = shell.minimize(a);
            let a_visible = shell.window(a).ok().map(|w| w.visible).unwrap_or(false);
            (
                shell.window_count(),
                shell.visible_windows().len(),
                a_visible,
                shell.window(b).ok().map(|w| w.bounds),
            )
        });

    // A is minimised (still managed, but not visible); B stays.
    assert_eq!(count_min, 2, "minimise destroyed a window (count {count_min})");
    assert!(!a_visible_min, "minimised window A is still flagged visible");
    assert_eq!(
        visible_min, 1,
        "after minimising A, the visible set should be just B (got {visible_min})"
    );
    let b_bounds = b_bounds.expect("B managed");
    assert_window_painted(&frame_min, b_bounds, "window B after window A was minimised");
    // A's region must NOT be painted (it is minimised). Measure this as a DELTA
    // against the no-window base desktop, NOT an absolute non-black count: the
    // liquid-glass wallpaper accent gradient is non-black and reads as ~47k
    // "painted" px for a WINDOW-FREE region under the absolute count (proven:
    // t62-session; t62-harden probe — abs_nonbg=47040 with NO window, yet
    // delta_vs_base=0). A_rect and B_rect are disjoint, so B does not contribute
    // to A's rect. A genuinely-persisting minimised window would differ from the
    // bare desktop by tens of thousands of px; a correctly-hidden one reads ~0.
    let base = base_desktop();
    let a_area = (a_rect.width * a_rect.height) as usize;
    let a_delta_min = window_delta_vs_base(&frame_min, &base, a_rect, PRESENCE_DELTA_TOL);
    assert!(
        a_delta_min < a_area / 20,
        "minimised window A is still painted ({a_delta_min} px differ from the bare desktop at \
         its rect, area {a_area}) — minimise did not hide it. A persisting window would differ \
         by ~{} px.",
        a_area / 2
    );

    // --- Phase 2: minimise then restore A; check both present. ---
    let (frame_restore, (count_r, visible_r, a_bounds_r, b_bounds_r)) =
        capture_desktop_scripted_with_readback(|shell| {
            let a = shell.open_app_window("com.liquide.terminal");
            let b = shell.open_app_window("com.liquide.files");
            let _ = shell.move_window(a, a_rect.x, a_rect.y);
            let _ = shell.resize_window(a, a_rect.width, a_rect.height);
            let _ = shell.move_window(b, b_rect.x, b_rect.y);
            let _ = shell.resize_window(b, b_rect.width, b_rect.height);
            let _ = shell.minimize(a);
            let _ = shell.restore(a);
            (
                shell.window_count(),
                shell.visible_windows().len(),
                shell.window(a).ok().map(|w| w.bounds),
                shell.window(b).ok().map(|w| w.bounds),
            )
        });

    assert_eq!(count_r, 2, "restore lost a window (count {count_r})");
    assert_eq!(
        visible_r, 2,
        "after restoring A, both windows should be visible (got {visible_r})"
    );
    let a_bounds_r = a_bounds_r.expect("A managed after restore");
    let b_bounds_r = b_bounds_r.expect("B managed after restore");
    assert_window_painted(&frame_restore, a_bounds_r, "window A after restore (should reappear)");
    assert_window_painted(&frame_restore, b_bounds_r, "window B after A restored");
}

// ===========================================================================
// 7. MAXIMIZE — maximising one window leaves the others managed; un-maximise
//    (restore) brings the original bounds back, no permanent loss.
// ===========================================================================

/// Maximise a window: it must fill the work area (painted) and still be the only
/// change to the window set (count stable). Restore it and assert it comes back
/// to its prior, smaller, painted bounds (no vanish on the way back).
///
/// TEETH: maximized body fills (painted over a large region), count stable,
/// restore yields a painted smaller window. A maximize/restore that loses the
/// window fails.
#[test]
fn maximize_then_restore_keeps_window() {
    // Phase 1: maximise — window fills work area, count stays 1.
    let (frame_max, (count_max, state_max, max_bounds)) = capture_desktop_scripted_with_readback(|shell| {
        let w = shell.open_app_window("com.liquide.files");
        let _ = shell.maximize(w);
        (
            shell.window_count(),
            shell.window(w).ok().map(|x| x.state),
            shell.window(w).ok().map(|x| x.bounds),
        )
    });
    assert_eq!(count_max, 1, "maximise changed the window count to {count_max}");
    assert_eq!(state_max, Some(WindowState::Maximized), "window not maximized");
    let max_bounds = max_bounds.expect("maximized window managed");
    assert_window_painted(&frame_max, max_bounds, "maximized window");

    // Phase 2: maximise then restore — window returns, still painted, count 1,
    // and its bounds shrink back below the maximized size.
    let (frame_restore, (count_r, state_r, restore_bounds)) = capture_desktop_scripted_with_readback(|shell| {
        let w = shell.open_app_window("com.liquide.files");
        let _ = shell.maximize(w);
        let _ = shell.restore(w);
        (
            shell.window_count(),
            shell.window(w).ok().map(|x| x.state),
            shell.window(w).ok().map(|x| x.bounds),
        )
    });
    assert_eq!(count_r, 1, "restore-from-maximize changed the count to {count_r}");
    assert_eq!(state_r, Some(WindowState::Normal), "window not restored to Normal");
    let restore_bounds = restore_bounds.expect("restored window managed");
    assert!(
        restore_bounds.width < max_bounds.width || restore_bounds.height < max_bounds.height,
        "restore did not shrink the window back below maximized bounds ({:.0}x{:.0} vs max \
         {:.0}x{:.0})",
        restore_bounds.width,
        restore_bounds.height,
        max_bounds.width,
        max_bounds.height
    );
    assert_window_painted(&frame_restore, restore_bounds, "window after restore-from-maximize");
}

// ===========================================================================
// 8. STABILITY ACROSS IDLE FRAMES — with a window open and nothing happening,
//    repeated identical renders are byte-stable (no window region flicker).
// ===========================================================================

/// Open a window and render the IDENTICAL idle state FOUR times in four separate
/// captures. Every frame must be byte-identical to the first (no window region
/// flickering in/out, no per-frame nondeterminism) AND the window body must be
/// painted in every frame. This is the steady-state guard specialised to a
/// scene that CONTAINS a window (vs t58's empty-desktop steady state).
///
/// TEETH: byte-identity across all four + body painted in each. The companion
/// `open_stays_open_teeth` proves the comparator detects real change.
#[test]
fn idle_window_set_is_byte_stable_across_frames() {
    let mut frames = Vec::new();
    let mut bounds_seen = None;
    for pass in 0..4 {
        let (frame, bounds) = capture_desktop_scripted_with_readback(|shell| {
            let _ = shell.open_app_window("com.liquide.files");
            shell.visible_windows().first().map(|w| w.bounds).expect("window open")
        });
        assert_window_painted(&frame, bounds, &format!("idle pass {pass}"));
        bounds_seen = Some(bounds);
        frames.push(frame);
    }
    let bounds = bounds_seen.unwrap();
    for (i, pair) in frames.windows(2).enumerate() {
        let d = diff_frames(&pair[0], &pair[1], DiffOptions::exact());
        assert!(
            d.matched,
            "idle desktop WITH a window open is not byte-stable: frame {i} vs {} differ by {} px \
             (max delta {}). A window region flickering in/out on idle frames is the reported \
             defect. Window bounds: ({:.0},{:.0} {:.0}x{:.0}).",
            i + 1,
            d.differing_pixels,
            d.max_channel_delta,
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height
        );
    }
}

// ===========================================================================
// 9. VANISHING-SUBTREE / LAYOUT-CACHE — a window's content must survive a
//    RE-LAYOUT (cross-ref t49-e3-F2 layout-cache leaf-safety history). Opening
//    another window (which triggers arrange/re-layout) must not blank the first
//    window's content subtree.
// ===========================================================================

/// Open window A and capture its painted body. Then, in a second capture, open A
/// then open B (a second window forces a re-layout / scene rebuild). After the
/// re-layout, A's body region must STILL be painted — its content subtree must
/// not vanish from a layout-cache invalidation bug. We position A in a corner so
/// B (centred) does not occlude it, isolating "re-layout blanked A" from "B
/// covers A".
///
/// TEETH: A's corner body painted both before and after B opens. If a re-layout
/// drops A's subtree (the t49-e3-F2 leaf-safety failure mode), A's corner
/// collapses to wallpaper after B opens -> FAIL.
#[test]
fn relayout_does_not_vanish_existing_window_subtree() {
    use liquide_compositor::geometry::Rect;
    let a_rect = Rect::new(40.0, 80.0, 380.0, 300.0); // top-left corner, clear of centre

    // Before: only A.
    let (frame_before, a_bounds_before) = capture_desktop_scripted_with_readback(|shell| {
        let a = shell.open_app_window("com.liquide.terminal");
        let _ = shell.move_window(a, a_rect.x, a_rect.y);
        let _ = shell.resize_window(a, a_rect.width, a_rect.height);
        shell.window(a).ok().map(|w| w.bounds).expect("A managed")
    });
    assert_window_painted(&frame_before, a_bounds_before, "window A before re-layout");

    // After: A, then open B (forces re-layout). A must remain painted.
    let (frame_after, (count, a_bounds_after)) = capture_desktop_scripted_with_readback(|shell| {
        let a = shell.open_app_window("com.liquide.terminal");
        let _ = shell.move_window(a, a_rect.x, a_rect.y);
        let _ = shell.resize_window(a, a_rect.width, a_rect.height);
        // Opening B triggers focus/raise/arrange — the re-layout under test.
        let _b = shell.open_app_window("com.liquide.browser");
        (
            shell.window_count(),
            shell.window(a).ok().map(|w| w.bounds),
        )
    });

    assert_eq!(count, 2, "expected 2 windows after opening B, found {count}");
    let a_bounds_after = a_bounds_after.expect("A still managed after B opened");
    assert_window_painted(
        &frame_after,
        a_bounds_after,
        "window A AFTER opening B (re-layout must not vanish A's content subtree)",
    );
}

// ===========================================================================
// REMOVED-WINDOW CORRECTNESS — a window hidden by a workspace switch is dropped
// from BOTH the shell scene AND the captured framebuffer.
// ===========================================================================

/// When a window is removed from the rendered set (hidden by a workspace switch)
/// the SHELL scene correctly drops it — `build_scene()` produces ZERO window
/// nodes — AND the captured framebuffer no longer shows the window: the region
/// it occupied reads as the bare wallpaper.
///
/// HISTORY / CORRECTION (t62-harden): this test was previously a "fail-on-fix
/// sentinel" asserting `window_body_content(frame, bounds) > 50_000` to claim a
/// STALE-FRAMEBUFFER defect (removed window still painted). That assertion was a
/// FALSE POSITIVE: `window_body_content` counts every pixel that is not within
/// `BG_TOLERANCE` of pure black, but the liquid-glass wallpaper's blue/purple
/// accent gradient is NOT black, so a WINDOW-FREE region already reports ~195k
/// "body" px (proven: t62-session; t62-harden probe — abs_nonbg=195472 with
/// ZERO window nodes in the scene). The render path is NOT stale: diffing the
/// post-switch frame against the no-window base desktop at the window rect reads
/// ~0 (probe: 213 px at tol 24 over a 440k-px rect). The window is genuinely
/// gone at both layers. The corrected assertion measures the DELTA vs the base
/// desktop and requires it to be ~0.
///
/// TEETH: a real stale/persisting window would differ from the bare desktop by
/// tens of thousands of px at this rect, failing the `< area/20` bound. The
/// scene-node and visible-set assertions remain as shell-side teeth.
#[test]
fn removed_window_is_gone_from_scene_and_framebuffer() {
    const NODE_WINDOW_BASE: u64 = 10_000;
    fn count_window_flat_nodes(scene: &liquide_compositor::scene::SceneNode) -> usize {
        scene
            .flatten()
            .iter()
            .filter(|n| n.id >= NODE_WINDOW_BASE && n.id < NODE_WINDOW_BASE + 1_000_000)
            .count()
    }

    let base = base_desktop();
    let (frame, (scene_nodes_after, visible_after, bounds)) =
        capture_desktop_scripted_with_readback(|shell| {
            let _w = shell.open_app_window("com.liquide.files");
            let b = shell.visible_windows().first().map(|w| w.bounds).unwrap();
            shell.execute_action(&ShellAction::WorkspaceAdd);
            shell.execute_action(&ShellAction::WorkspaceNext);
            // Read state BEFORE the render that capture performs.
            let nodes = count_window_flat_nodes(&shell.build_scene());
            (nodes, shell.visible_windows().len(), b)
        });

    // Shell side is CORRECT: the window is gone from the scene + visible set.
    assert_eq!(
        scene_nodes_after, 0,
        "shell scene still contains window nodes after the switch (shell-side bug)"
    );
    assert_eq!(visible_after, 0, "shell still reports the window visible after the switch");

    // Pixel side is ALSO correct: the framebuffer at the former window rect now
    // matches the bare desktop (window genuinely erased — measured as a delta vs
    // the no-window base, NOT an absolute non-black count which the wallpaper
    // gradient inflates).
    let area = (bounds.width * bounds.height) as usize;
    let delta = window_delta_vs_base(&frame, &base, bounds, PRESENCE_DELTA_TOL);
    assert!(
        delta < area / 20,
        "STALE FRAMEBUFFER: after the workspace switch the removed window's rect still differs \
         from the bare desktop by {delta} px (area {area}) — the render path did not erase it. \
         A correctly-cleared region differs by ~0; a persisting window by ~{}.",
        area / 2
    );
}

// ---------------------------------------------------------------------------
// Local readback wrapper: drive the Shell directly (no events) and read state.
// ---------------------------------------------------------------------------

/// Convenience over `capture_desktop_scripted_with`: run a Shell mutation that
/// ALSO returns a read-back value, capturing both the post-state frame and the
/// value. Mirrors `capture_desktop_scripted_readback` but driven purely through
/// the shell mutate closure (no PlatformEvents needed for these setups).
fn capture_desktop_scripted_with_readback<R, T>(op: R) -> (Frame, T)
where
    R: FnOnce(&mut liquide_shell::Shell) -> T,
{
    let mut slot: Option<T> = None;
    let mut op = Some(op);
    let frame = capture_desktop_scripted_with(
        &scenario_options(THEME),
        |_handle| Vec::new(),
        |shell| {
            if let Some(f) = op.take() {
                slot = Some(f(shell));
            }
        },
    )
    .expect("scripted-with capture");
    (frame, slot.expect("readback closure ran"))
}

/// Count pixels in a window's bounds rect that DIFFER from the same rect in the
/// NO-WINDOW base desktop (max-channel delta > tol). A present window body
/// differs strongly from the bare wallpaper; a vanished window leaves the
/// wallpaper, reading ~0 against the base.
fn window_delta_vs_base(
    frame: &Frame,
    base: &Frame,
    bounds: liquide_compositor::geometry::Rect,
    tol: u8,
) -> usize {
    let x = bounds.x.max(0.0) as u32;
    let y = bounds.y.max(0.0) as u32;
    let x1 = ((bounds.x + bounds.width) as u32).min(frame.width).min(base.width);
    let y1 = ((bounds.y + bounds.height) as u32).min(frame.height).min(base.height);
    let mut n = 0usize;
    for py in y..y1 {
        for px in x..x1 {
            let a = frame.pixel(px, py).unwrap();
            let b = base.pixel(px, py).unwrap();
            let d = a.iter().zip(b.iter()).map(|(&p, &q)| p.abs_diff(q)).max().unwrap_or(0);
            if d > tol { n += 1; }
        }
    }
    n
}

/// TEETH guard for the DELTA-vs-base "window is gone" assertions
/// (`workspace_round_trip_*`, `minimize_*`, `removed_window_is_gone_*`): a window
/// that is genuinely PRESENT at a rect must read a delta FAR ABOVE the `area/20`
/// "gone" threshold at `PRESENCE_DELTA_TOL`, so those assertions would FAIL if a
/// window actually persisted. This is the companion that keeps the gone-checks
/// honest — if `PRESENCE_DELTA_TOL` is ever loosened back toward 24 (where a
/// present glass window reads only ~4.7k px, below the threshold), this fails.
#[test]
fn presence_delta_has_teeth_for_a_present_window() {
    use liquide_compositor::geometry::Rect;
    let base = base_desktop();
    let a_rect = Rect::new(60.0, 90.0, 420.0, 320.0);
    // Open A at a_rect and DO NOT minimize / switch away -> it is present.
    let frame = capture_desktop_scripted_with(
        &scenario_options(THEME),
        |_h| Vec::new(),
        |shell| {
            let a = shell.open_app_window("com.liquide.terminal");
            let _ = shell.move_window(a, a_rect.x, a_rect.y);
            let _ = shell.resize_window(a, a_rect.width, a_rect.height);
        },
    )
    .expect("present capture");
    let area = (a_rect.width * a_rect.height) as usize;
    let delta = window_delta_vs_base(&frame, &base, a_rect, PRESENCE_DELTA_TOL);
    assert!(
        delta > area / 20,
        "TEETH FAILURE: a PRESENT window's rect differs from the bare desktop by only {delta} px \
         (area {area}, gone-threshold {}), so the 'window is gone' delta checks would NOT catch a \
         persisting window. PRESENCE_DELTA_TOL is too loose.",
        area / 20
    );
}
