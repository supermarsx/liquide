//! Per-surface visual-regression tests for the WINDOW / SESSION surfaces
//! (t57-e4, plan slice A3). Built on the t57-e1 (A0) harness foundation.
//!
//! Four surfaces, each driven through e1's per-surface scenario builders and
//! asserted with real CONTENT / DIFFERENTIAL teeth (not mere "renders without
//! panic"):
//!
//! | scenario           | assertion                                   | f-slice gate |
//! |--------------------|---------------------------------------------|--------------|
//! | window_decorations | one open window paints a titlebar + buttons | (PASSES now) |
//! | workspace_switch   | switching changes which windows render      | t57-f7       |
//! | overview           | overview overlay paints tiles               | t57-f-overview |
//! | lockscreen         | lock surface paints clock / prompt          | t57-f9       |
//!
//! ## Pass vs ignore (verified empirically against the current tree, 2026-06-13)
//!
//! Only `window_decorations` produces a real surface today: opening
//! `com.liquide.files` paints an 800x550 window with a 36px titlebar carrying
//! the close / maximize / minimize decoration buttons (the shell's `scene.rs`
//! emits a `SceneNodeKind::Decoration` with `DecorationButtons`). It is GREEN
//! and blessed, and carries the teeth proof for this slice.
//!
//! The other three are scaffolded with their FULL assertions but `#[ignore]`d
//! because the matching feature is not yet wired (confirmed: each returns a
//! frame byte-identical to the no-interaction base desktop). Per the A3 plan
//! note (mirroring t56-f4's menu pattern) the paired f-slice REMOVES the
//! `#[ignore]` as its acceptance gate:
//!   - `workspace_switch` — Super+Ctrl+Right switches workspace *state*, but
//!     `visible_windows()` does not yet filter by active workspace, so the
//!     rendered frame is unchanged. Un-ignored by **t57-f7**.
//!   - `overview` — `execute_action` (tick.rs) drops `TaskOverview` /
//!     `WorkspaceOverview` to `_ => false`, so the Super+Tab hotkey is a no-op
//!     and no overview overlay paints. Un-ignored by **t57-f-overview**. NOTE:
//!     the t57 plan's f-list has NO explicit overview slice — an overview fixer
//!     must be added (flagged to the coordinator in .orchestration/logs/t57-e4.md).
//!   - `lockscreen` — Super+L drives `LockSession` state, but the canonical
//!     lockscreen surface does not paint. Un-ignored by **t57-f9**.
//!
//! Bless goldens with:
//!   `BLESS=1 cargo test -p liquide-visual-test --test visual_windows`
//! (or `LIQUIDE_UPDATE_GOLDEN=1`). Run without it to assert.

use liquide_visual_test::diff::{DiffOptions, diff_frames};
use liquide_visual_test::golden::assert_golden;
use liquide_visual_test::scenarios::{
    lockscreen, overview, themed_desktop_capture, window_decorations, workspace_switch,
};

/// The desktop wallpaper is dark; use black as the background reference with a
/// generous tolerance so wallpaper gradient noise is not counted as content.
const BG_REFERENCE: [u8; 4] = [0, 0, 0, 255];
const BG_TOLERANCE: u8 = 24;

// ---------------------------------------------------------------------------
// window_decorations — PASSES now; carries this slice's teeth.
// ---------------------------------------------------------------------------

// Geometry of the `com.liquide.files` window opened by the
// `window_decorations` builder, on the canonical 1280x720 surface:
//   open_app_window("com.liquide.files") => 800x550, centred horizontally.
//   x = (1280 - 800) / 2 = 240  => window spans x ∈ [240, 1040].
// The titlebar is the top `title_bar_height` (36px) band of the window; the
// close / maximize / minimize buttons cluster in its top-right corner
// (close at right-32-4). We anchor crops generously inside those regions so
// the assertion tolerates small layout/centering shifts.
const WIN_X: u32 = 240;
const WIN_W: u32 = 800;

/// window_decorations — opening one app window paints a decorated window with a
/// titlebar AND close/min/max buttons.
///
/// TEETH: this is a *differential + content* guard. We diff the window region
/// against the no-window base desktop (the window must add a large block of new
/// pixels) AND assert the top-right button cluster carries content distinct from
/// the wallpaper. If the open-window / decoration paint regresses (e.g. the
/// `SceneNodeKind::Decoration` stops being emitted, or `open_app_window`
/// silently no-ops), the region collapses back to the base desktop and this
/// fails. Proven by reverting `window_decorations` to return the base frame
/// (see .orchestration/logs/t57-e4.md) — the diff drops to 0 and the test fails.
#[test]
fn window_decorations_paints_titlebar_and_buttons() {
    let theme = "liquid-glass";

    let base = themed_desktop_capture(theme).expect("base desktop capture");
    let framed = window_decorations(theme).expect("window_decorations capture");

    assert_eq!(
        (base.width, base.height),
        (framed.width, framed.height),
        "base and windowed frames must share the canonical surface size"
    );
    assert!(
        !framed.is_uniform(),
        "windowed frame is uniform — the pipeline produced a dead/blank frame"
    );

    // TOOTH 1 (differential): the window must add a large block of new pixels
    // over the centre of the screen versus the no-window base desktop.
    let win_region = (WIN_X, framed.height / 6, WIN_W, framed.height / 2);
    let base_win = base.crop(win_region.0, win_region.1, win_region.2, win_region.3);
    let framed_win = framed.crop(win_region.0, win_region.1, win_region.2, win_region.3);
    let body = diff_frames(&base_win, &framed_win, DiffOptions::default());
    assert!(
        !body.matched && body.differing_pixels > 50_000,
        "open window did not paint: only {} pixels changed over the window region \
         versus the base desktop (threshold 50000). Expected open_app_window to \
         create and paint a decorated window. Check Shell::open_app_window and the \
         window scene path.",
        body.differing_pixels
    );

    // TOOTH 2 (titlebar): the top band of the window (its 36px titlebar) must
    // carry content distinct from the base wallpaper there.
    let title_h = 40u32;
    let base_title = base.crop(WIN_X, win_region.1, WIN_W, title_h);
    let framed_title = framed.crop(WIN_X, win_region.1, WIN_W, title_h);
    let title = diff_frames(&base_title, &framed_title, DiffOptions::default());
    assert!(
        !title.matched && title.differing_pixels > 5_000,
        "window titlebar did not paint: only {} pixels changed in the titlebar \
         band (threshold 5000). Expected the title-bar Decoration node to paint.",
        title.differing_pixels
    );

    // TOOTH 3 (close/min/max buttons): the top-right corner of the window — where
    // the close (right-36), maximize (right-68), minimize (right-100) buttons
    // sit — must carry content distinct from the base wallpaper.
    let btn_w = 150u32;
    let btn_x = (WIN_X + WIN_W).saturating_sub(btn_w);
    let base_btns = base.crop(btn_x, win_region.1, btn_w, title_h);
    let framed_btns = framed.crop(btn_x, win_region.1, btn_w, title_h);
    let btns = diff_frames(&base_btns, &framed_btns, DiffOptions::default());
    let btns_content = framed_btns.non_background_pixels(BG_REFERENCE, BG_TOLERANCE);
    assert!(
        !btns.matched && btns.differing_pixels > 1_000 && btns_content > 1_000,
        "window control buttons did not paint: {} changed / {} non-bg pixels in \
         the top-right button cluster (threshold 1000 each). Expected the \
         close/maximize/minimize DecorationButtons to paint.",
        btns.differing_pixels, btns_content
    );

    // Pin the decorated-window frame so future regressions are caught.
    assert_golden("window_decorations", &framed);
}

// ---------------------------------------------------------------------------
// workspace_switch — #[ignore] until t57-f7.
// ---------------------------------------------------------------------------

/// workspace_switch — switching to the next workspace changes which windows
/// render, so the post-switch frame DIFFERS from the pre-switch frame.
///
/// DIFFERENTIAL TOOTH: we open a window on the current workspace, capture, then
/// switch workspace (Super+Ctrl+Right) and capture again; the two frames must
/// differ (the window belongs to the prior workspace and should no longer
/// render). Today `visible_windows()` does not filter by active-workspace
/// membership, so switching leaves the rendered frame unchanged.
///
/// TODO: un-ignored by t57-f7 (filter visible_windows / hover / click paths by
/// active-workspace membership). When f7 lands, remove `#[ignore]` and this test
/// is f7's acceptance gate.
#[test]
fn workspace_switch_changes_rendered_windows() {
    let theme = "liquid-glass";

    // Baseline desktop with a window open on the current workspace.
    let before = window_decorations(theme).expect("pre-switch capture");

    // After switching workspace, the window from the prior workspace should no
    // longer render, so the frame must differ.
    let after = workspace_switch(theme).expect("post-switch capture");

    assert_eq!(
        (before.width, before.height),
        (after.width, after.height),
        "pre/post-switch frames must share the canonical surface size"
    );

    let delta = diff_frames(&before, &after, DiffOptions::default());
    assert!(
        !delta.matched && delta.differing_pixels > 10_000,
        "workspace switch did not change the rendered windows: only {} pixels \
         differ between the pre- and post-switch frames (threshold 10000). \
         Expected the switch to change which windows render (active-workspace \
         filtering in visible_windows). Check WorkspaceManager wiring and the \
         WorkspaceNext action path.",
        delta.differing_pixels
    );
}

// ---------------------------------------------------------------------------
// overview — #[ignore] until an overview fixer slice (NOT in the current plan).
// ---------------------------------------------------------------------------

/// overview — the task/workspace overview overlay paints tiles over the desktop.
///
/// CONTENT/DIFFERENTIAL TOOTH: opening the overview (Super+Tab) must add a
/// substantial overlay (window tiles) over the base desktop, so the frame
/// differs from the no-overview base. Today `execute_action` drops
/// `TaskOverview` / `WorkspaceOverview` to `_ => false`, so the hotkey is a
/// no-op and no overlay paints.
///
/// TODO: un-ignored by t57-f-overview. NOTE: the t57 plan's f-list has no
/// explicit overview slice — an overview fixer must be added (flagged to the
/// coordinator). When it lands, remove `#[ignore]` and this test is its gate.
#[test]
fn overview_paints_tiles() {
    let theme = "liquid-glass";

    // Open a window first so the overview has a tile to show, then diff the
    // overview frame against that same windowed base.
    let base = window_decorations(theme).expect("base capture");
    let over = overview(theme).expect("overview capture");

    assert_eq!(
        (base.width, base.height),
        (over.width, over.height),
        "base and overview frames must share the canonical surface size"
    );
    assert!(
        !over.is_uniform(),
        "overview frame is uniform — dead/blank pipeline"
    );

    let delta = diff_frames(&base, &over, DiffOptions::default());
    assert!(
        !delta.matched && delta.differing_pixels > 20_000,
        "overview overlay did not paint: only {} pixels differ from the base \
         desktop (threshold 20000). Expected the Super+Tab overview overlay to \
         paint window tiles. Check execute_action's TaskOverview/WorkspaceOverview \
         arms (currently `_ => false`) and the overview overlay scene path.",
        delta.differing_pixels
    );
}

// ---------------------------------------------------------------------------
// lockscreen — #[ignore] until t57-f9.
// ---------------------------------------------------------------------------

/// lockscreen — locking the session paints the lock surface (clock / prompt)
/// over the desktop.
///
/// CONTENT/DIFFERENTIAL TOOTH: the lock surface is a full-screen overlay, so the
/// locked frame must differ substantially from the unlocked desktop AND the
/// centre of the screen (where the clock / password prompt live) must carry
/// content. Today Super+L drives `LockSession` state but the canonical
/// lockscreen surface does not paint, so the frame is unchanged.
///
/// TODO: un-ignored by t57-f9 (wire the Lock action to drive chrome_lockscreen
/// and paint the lock surface). When f9 lands, remove `#[ignore]` and this test
/// is f9's acceptance gate.
#[test]
fn lockscreen_paints_clock_and_prompt() {
    let theme = "liquid-glass";

    let base = themed_desktop_capture(theme).expect("base desktop capture");
    let locked = lockscreen(theme).expect("lockscreen capture");

    assert_eq!(
        (base.width, base.height),
        (locked.width, locked.height),
        "base and locked frames must share the canonical surface size"
    );
    assert!(
        !locked.is_uniform(),
        "locked frame is uniform — dead/blank pipeline"
    );

    // The lock overlay covers the whole screen: the locked frame must differ
    // substantially from the unlocked desktop.
    let delta = diff_frames(&base, &locked, DiffOptions::default());
    assert!(
        !delta.matched && delta.differing_pixels > 20_000,
        "lock surface did not paint: only {} pixels differ from the unlocked \
         desktop (threshold 20000). Expected a full-screen lock overlay. Check \
         the LockSession action -> chrome_lockscreen drive path.",
        delta.differing_pixels
    );

    // The clock / password prompt cluster lives in the centre of the screen;
    // assert it carries content.
    let cx = locked.width / 4;
    let cy = locked.height / 3;
    let centre = locked.crop(cx, cy, locked.width / 2, locked.height / 3);
    let content = centre.non_background_pixels(BG_REFERENCE, BG_TOLERANCE);
    assert!(
        content > 1_000,
        "lock surface centre has only {content} non-background pixels — \
         the clock / password prompt is not painting."
    );
}
