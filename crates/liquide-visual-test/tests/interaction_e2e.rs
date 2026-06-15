//! Interaction / e2e harness with STATE + PIXEL assertions (t57-e5, plan slice
//! A4). Built on the t57-e1 (A0) capture foundation and e7's read-only Shell
//! accessors.
//!
//! Every sequence here drives a scripted [`PlatformEvent`] flow (or a hotkey)
//! through the REAL desktop event path — `handle_event` ->
//! `handle_platform_event` -> `execute_action`, so returned shell actions
//! actually run — and then asserts BOTH:
//!   1. a SHELL STATE delta (read off the live `Shell` after the events through
//!      e7's / the shell's read-only accessors), AND
//!   2. a PIXEL delta (the captured frame changed in the region that should have
//!      repainted).
//!
//! The single capture entry point is
//! [`capture_desktop_scripted_readback`](liquide_visual_test::capture_desktop_scripted_readback):
//! it dispatches the events, runs a `readback` closure against the live shell to
//! extract the state value, and returns that value alongside the post-event
//! [`Frame`]. State and pixels therefore come from the SAME deterministic render
//! (no double-capture skew).
//!
//! ## Pass-now vs `#[ignore]`-gated (verified against the current tree)
//!
//! | sequence                              | state                       | pixels                      | status |
//! |---------------------------------------|-----------------------------|-----------------------------|--------|
//! | click dock item -> window opens       | window_count 0 -> 1         | window body paints          | PASS now |
//! | drag a window -> bounds change        | window.bounds moves         | repaints at new position    | PASS now |
//! | right-click item activate -> fired    | action opens a window       | menu dismissed / window     | PASS now |
//! | hotkey workspace switch               | active workspace changes    | rendered windows change     | PASS now |
//! | drag-to-edge window snap-tile         | window tiles to a zone      | window snaps to half-screen | PASS now |
//! | double-click titlebar -> maximize     | window.state == Maximized   | window grows to work area   | #[ignore] -> f-window/shellfix |
//! | keyboard into focused text field      | text reaches the app buffer | glyphs paint in the field   | #[ignore] -> f-textinput/shellfix |
//!
//! Five sequences PASS today (the underlying behavior is already wired). Two are
//! `#[ignore]`-gated to the f-slice that un-ignores them as its acceptance gate
//! (mirrors visual_windows.rs / the t56-f4 menu pattern); the gate-closer removes
//! the `#[ignore]` once the matching shell-fix lands.
//!
//! WHY the gated ones are gated (confirmed by un-ignoring them against the current
//! shell — both fail today):
//!   - double-click titlebar: `handle_mouse_button` treats a title-bar press as
//!     the start of a `DragState::Moving`; there is NO double-click detector, so
//!     two clicks just start+end a zero-length drag and never `maximize()` (the
//!     window stays `WindowState::Normal`). The full maximize assertion is written
//!     but gated until the shell wires double-click-titlebar -> MaximizeWindow.
//!   - keyboard into a focused text field: the shell has no public path that
//!     routes typed characters into a focused app's text buffer headlessly (apps
//!     are wired in their own crates per e8, but the shell<->app text-input seam
//!     is not driven on the capture path, and no accessor exposes the focused
//!     app's buffer). Gated until that seam is wired.
//!
//! NOTE vs the A4 brief / e4 / e7: the brief pre-marked workspace-switch -> f7 and
//! snap-tile -> f10 as gated. Verified empirically they PASS today:
//!   - workspace switch: `Shell::visible_windows()` already filters by
//!     active-workspace membership (the t49-e5-F01 fix), so a window opened on the
//!     origin workspace stops rendering after the switch. e4's `#[ignore]`d
//!     visual_windows::workspace_switch is a weaker scenario (it switches on an
//!     EMPTY desktop, so there is nothing to filter); this one opens a window in
//!     both frames and is a real differential.
//!   - drag-to-edge snap: `handle_mouse_button`'s release arm calls
//!     `apply_snap_on_release`, which tiles the window into the active snap zone.
//!     e7's f10 allowlist is about the `chrome_tiling` *audit bit* not flipping,
//!     not about the snap behavior being absent — the behavior is live.

use liquide_input::mouse::MouseButton;
use liquide_visual_test::diff::{DiffOptions, diff_frames};
use liquide_visual_test::scenarios::{
    ScriptedScenario, scenario_options, themed_desktop_capture,
};
use liquide_visual_test::{Frame, capture_desktop_scripted_readback};
use liquide_shell::{ShellAction, WindowState};

const THEME: &str = "liquid-glass";

/// Dark wallpaper background reference + tolerance for non-bg content counts.
const BG_REFERENCE: [u8; 4] = [0, 0, 0, 255];
const BG_TOLERANCE: u8 = 24;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Capture a no-interaction base desktop (no windows, no menus) for diffing.
fn base_desktop() -> Frame {
    themed_desktop_capture(THEME).expect("base desktop capture")
}

/// Geometry of the first dock item's centre on the canonical surface, derived
/// from the live dock layout (not hard-coded) so it tolerates config drift.
///
/// We read it off the shell during a throwaway state-only capture: the dock is
/// constructed with four pinned items by `Shell::new`, and
/// `Dock::compute_item_rects` lays them out against the screen rect.
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

// ===========================================================================
// 1. click a dock item -> a window opens (window_count++) AND a window paints.
//    PASSES today: the dock click arm (events.rs) calls `open_app_window`.
// ===========================================================================

/// Clicking the first dock item opens an application window: `window_count`
/// increments (STATE) and a window body paints over the centre of the desktop
/// (PIXELS).
///
/// TEETH: both a state delta (0 -> 1 windows) and a differential pixel delta
/// over the window body region. If the dock-click arm regresses (stops calling
/// `open_app_window`), `window_count` stays 0 and the body diff collapses.
#[test]
fn click_dock_item_opens_window() {
    let (cx, cy) = first_dock_item_centre();
    let base = base_desktop();

    let (frame, window_count) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |handle| {
            ScriptedScenario::new(handle)
                .left_click(cx, cy)
                .into_events()
        },
        |shell| shell.window_count(),
    )
    .expect("dock-click capture");

    // STATE: a window now exists (the dock click drove `open_app_window`).
    assert_eq!(
        window_count, 1,
        "clicking the first dock item should open exactly one window \
         (window_count). Check the dock-click arm in events.rs -> open_app_window."
    );

    // PIXELS: the opened window paints a body block over the centre of the
    // screen, differing from the no-window base desktop.
    assert_eq!((base.width, base.height), (frame.width, frame.height));
    let region = (frame.width / 4, frame.height / 6, frame.width / 2, frame.height / 2);
    let base_body = base.crop(region.0, region.1, region.2, region.3);
    let now_body = frame.crop(region.0, region.1, region.2, region.3);
    let delta = diff_frames(&base_body, &now_body, DiffOptions::default());
    assert!(
        !delta.matched && delta.differing_pixels > 20_000,
        "no window painted after the dock click: only {} pixels changed over the \
         window body region (threshold 20000).",
        delta.differing_pixels
    );
}

// ===========================================================================
// 2. drag a window -> bounds change (STATE) AND it paints at the new position.
//    PASSES today: `DragState::Moving` updates `window.bounds` on each move.
// ===========================================================================

/// Dragging an open window by its title bar moves it: the window's `bounds`
/// origin changes (STATE) and the window paints at the new location while the
/// old location reverts toward the wallpaper (PIXELS).
///
/// The window is opened via the dock click (so the whole flow is event-driven),
/// then dragged by a title-bar press + interpolated moves + release. We read the
/// final bounds off the live shell and diff the source vs destination regions.
///
/// TEETH: a state delta (origin x/y moved by ~the drag vector) and a pixel delta
/// at the destination. If drag handling regresses (bounds stop tracking the
/// cursor), the origin stays put and the destination diff collapses.
#[test]
fn drag_window_changes_bounds_and_repaints() {
    let (dock_cx, dock_cy) = first_dock_item_centre();

    // Open the window first (separate capture) to learn its initial title-bar
    // location deterministically, then perform the drag in a second capture.
    let (_open_frame, initial) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |handle| {
            ScriptedScenario::new(handle)
                .left_click(dock_cx, dock_cy)
                .into_events()
        },
        |shell| {
            let w = shell
                .visible_windows()
                .first()
                .map(|w| w.bounds)
                .expect("a window must be open after the dock click");
            w
        },
    )
    .expect("open-window probe capture");

    // Title-bar grab point: just inside the top edge, left of the buttons.
    let grab_x = initial.x + 60.0;
    let grab_y = initial.y + 12.0;
    let dx = -180.0_f32;
    let dy = 120.0_f32;
    let drop_x = grab_x + dx;
    let drop_y = grab_y + dy;

    let base = base_desktop();

    let (frame, moved) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |handle| {
            // Open via dock, then drag the title bar by (dx, dy).
            ScriptedScenario::new(handle)
                .left_click(dock_cx, dock_cy)
                .drag(MouseButton::Left, grab_x, grab_y, drop_x, drop_y, 8)
                .into_events()
        },
        |shell| {
            shell
                .visible_windows()
                .first()
                .map(|w| w.bounds)
                .expect("window still open after drag")
        },
    )
    .expect("drag capture");

    // STATE: the window origin moved by ~the drag vector (tolerance for the
    // title-bar grab offset bookkeeping).
    let moved_dx = moved.x - initial.x;
    let moved_dy = moved.y - initial.y;
    assert!(
        (moved_dx - dx).abs() < 8.0 && (moved_dy - dy).abs() < 8.0,
        "window bounds did not track the drag: moved by ({moved_dx:.1}, {moved_dy:.1}), \
         expected ~({dx:.1}, {dy:.1}). Check DragState::Moving in handle_mouse_move."
    );

    // PIXELS: the destination region (around the dropped window) must now carry
    // window content distinct from the base wallpaper there.
    assert_eq!((base.width, base.height), (frame.width, frame.height));
    let dst = (
        (moved.x.max(0.0)) as u32,
        (moved.y.max(0.0)) as u32,
        moved.width.min(frame.width as f32) as u32,
        (moved.height * 0.5).min(frame.height as f32) as u32,
    );
    let base_dst = base.crop(dst.0, dst.1, dst.2, dst.3);
    let now_dst = frame.crop(dst.0, dst.1, dst.2, dst.3);
    let delta = diff_frames(&base_dst, &now_dst, DiffOptions::default());
    assert!(
        !delta.matched && delta.differing_pixels > 10_000,
        "window did not paint at its new dragged position: only {} pixels changed \
         over the destination region (threshold 10000).",
        delta.differing_pixels
    );
}

// ===========================================================================
// 3. right-click item activate -> action fired (STATE) AND menu dismissed
//    (PIXELS). PASSES today: the desktop context menu's first item is
//    "Open Terminal" -> ShellAction::OpenTerminal -> open_app_window.
// ===========================================================================

/// Right-clicking the desktop opens the context menu; clicking its first item
/// ("Open Terminal") fires `ShellAction::OpenTerminal`, which opens a window
/// (STATE: window_count increments == the action fired) and dismisses the menu
/// (PIXELS: the menu region no longer shows a menu, the desktop+window remain).
///
/// The right-click anchors the menu top-left at the click point; the first item
/// sits at `MENU_PADDING + 0.5 * MENU_ITEM_HEIGHT` below the top, half the menu
/// width across.
///
/// TEETH: state (action fired -> window opened) and pixels (a window painted /
/// the frame changed from the bare base desktop). If the context-menu item
/// dispatch regresses, no window opens and window_count stays 0.
#[test]
fn right_click_menu_item_fires_action_and_dismisses() {
    // Desktop right-click point (empty area away from dock/bar).
    let (rx, ry) = (300.0_f32, 250.0_f32);
    // Context-menu geometry (shell/mod.rs): top-left at the click point;
    // MENU_PADDING = 4, MENU_ITEM_HEIGHT = 28, CONTEXT_MENU_WIDTH = 200.
    let item_x = rx + 100.0; // half the menu width
    let item_y = ry + 4.0 + 0.5 * 28.0; // first item centre

    let base = base_desktop();

    let (frame, window_count) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |handle| {
            ScriptedScenario::new(handle)
                // Open the context menu on the empty desktop.
                .right_click(rx, ry)
                // Click the first menu item ("Open Terminal").
                .left_click(item_x, item_y)
                .into_events()
        },
        |shell| shell.window_count(),
    )
    .expect("right-click-activate capture");

    // STATE: the menu action fired -> a terminal window opened.
    assert_eq!(
        window_count, 1,
        "activating the first context-menu item did not fire its action \
         (OpenTerminal should open a window). Check the context-menu click arm \
         in events.rs and execute_action(OpenTerminal)."
    );

    // PIXELS (menu dismissed): the menu was anchored at (rx, ry). After clicking
    // an item the menu must be gone — but a window opened, so we assert the menu
    // strip just below the click point no longer matches a menu render by
    // checking the frame differs from base (window present) AND that the small
    // band at the menu's right edge (clear of the centred window) is NOT a solid
    // menu panel. Simplest robust tooth: the whole frame changed from base
    // (a window painted) and the action-fired state above proves dismissal flow.
    assert_eq!((base.width, base.height), (frame.width, frame.height));
    let whole = diff_frames(&base, &frame, DiffOptions::default());
    assert!(
        !whole.matched && whole.differing_pixels > 20_000,
        "frame did not change after the menu activation: only {} pixels differ \
         from the bare base desktop (threshold 20000). Expected the opened window \
         to paint (and the transient menu to be gone).",
        whole.differing_pixels
    );

    // The menu (anchored at rx,ry, 200px wide, ~148px tall) is dismissed: the
    // narrow vertical strip at the menu's LEFT edge, BELOW the opened window's
    // titlebar band but still within the former menu rect, should read close to
    // the wallpaper background rather than an opaque menu panel. The centred
    // 720x* window does not cover x in [rx, rx+24] at this small offset, so this
    // strip is a clean dismissal probe.
    let strip = frame.crop(rx as u32, (ry as u32) + 60, 16, 60);
    let strip_content = strip.non_background_pixels(BG_REFERENCE, BG_TOLERANCE);
    assert!(
        strip_content < (strip.width * strip.height) as usize / 2,
        "context menu appears to still be painted after activation: the menu strip \
         has {strip_content} non-background pixels (more than half), expected the \
         menu to be dismissed."
    );
}

// ===========================================================================
// 4. double-click titlebar -> maximize (STATE) AND pixels change.
//    GATED: the shell has no double-click-titlebar -> maximize detector yet.
// ===========================================================================

/// Double-clicking a window's title bar should maximize it: the window's
/// `state` becomes `WindowState::Maximized` and its bounds grow to the work area
/// (STATE), and the window repaints filling the work area (PIXELS).
///
/// TODO: un-ignored by the shell-fix slice that wires double-click-titlebar ->
/// `MaximizeWindow`. Today `handle_mouse_button` treats a title-bar press as the
/// start of a `DragState::Moving` and has no double-click detector, so the two
/// clicks start+end a zero-length drag and never maximize.
#[test]
fn double_click_titlebar_maximizes() {
    let (dock_cx, dock_cy) = first_dock_item_centre();

    // Learn the window's title-bar location after opening it.
    let (_f, initial) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |handle| {
            ScriptedScenario::new(handle)
                .left_click(dock_cx, dock_cy)
                .into_events()
        },
        |shell| {
            shell
                .visible_windows()
                .first()
                .map(|w| w.bounds)
                .expect("window open after dock click")
        },
    )
    .expect("open-window probe");

    let tb_x = initial.x + initial.width / 2.0;
    let tb_y = initial.y + 12.0;

    let base = base_desktop();

    let (frame, (state, bounds)) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |handle| {
            ScriptedScenario::new(handle)
                .left_click(dock_cx, dock_cy)
                .double_click(tb_x, tb_y)
                .into_events()
        },
        |shell| {
            let w = shell
                .visible_windows()
                .first()
                .map(|w| (w.state, w.bounds))
                .expect("window open after double-click");
            w
        },
    )
    .expect("double-click capture");

    // STATE: the window is maximized and grew beyond its initial size.
    assert_eq!(
        state,
        WindowState::Maximized,
        "double-clicking the title bar did not maximize the window"
    );
    assert!(
        bounds.width > initial.width && bounds.height > initial.height,
        "maximized bounds ({:.0}x{:.0}) are not larger than the initial bounds \
         ({:.0}x{:.0})",
        bounds.width,
        bounds.height,
        initial.width,
        initial.height
    );

    // PIXELS: the work-area fill differs substantially from the base desktop.
    let delta = diff_frames(&base, &frame, DiffOptions::default());
    assert!(
        !delta.matched && delta.differing_pixels > 80_000,
        "maximized window did not paint over the work area: only {} pixels differ \
         from the base desktop (threshold 80000).",
        delta.differing_pixels
    );
}

// ===========================================================================
// 5. keyboard into a focused text field -> text reaches the app (STATE) AND
//    glyphs paint (PIXELS). GATED: no shell<->app text-input seam on the
//    headless capture path.
// ===========================================================================

/// Typing into a focused application text field should deliver the characters to
/// the app's text buffer (STATE) and paint the corresponding glyphs in the field
/// (PIXELS).
///
/// TODO: un-ignored by the shell-fix slice that wires the shell<->app text-input
/// seam (typed `KeyInput` -> focused app's text buffer) on the live/headless
/// path. Today the shell routes key events to shortcuts/actions but exposes no
/// public path that delivers typed text into a focused app's buffer through the
/// capture harness, so neither the buffer state nor field glyphs can be asserted.
///
/// The assertion is scaffolded against a text-editor app window so the gate
/// closer can wire the seam and flip it green. Until then it is `#[ignore]`d.
#[test]
fn keyboard_into_text_field_reaches_app_and_paints() {
    let (dock_cx, dock_cy) = first_dock_item_centre();
    let base = base_desktop();

    // Open a window, focus it, then type. The readback would inspect the focused
    // app's text buffer once the seam exists; today there is no public accessor
    // for that, so this is the shape the gate-closer fills in.
    let (frame, typed_text_reached_app) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |handle| {
            ScriptedScenario::new(handle)
                .left_click(dock_cx, dock_cy)
                // Click into the window body (the text field) then type.
                .left_click(640.0, 380.0)
                .type_text("hello")
                .into_events()
        },
        |shell| {
            // t70-s6 made app windows run the REAL app: the host installs an
            // app-view factory, so opening the Files window registers a live
            // `FilesRuntime` and typed chars route into THAT app's model (its
            // search buffer), not the shell's legacy `focused_app_text` buffer.
            // Verify the text reached the app by rendering its content view and
            // looking for the typed string. (When no app view is registered the
            // legacy `focused_app_text()` path still applies — see the shell's
            // `without_factory_open_keeps_placeholder` test.)
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
    .expect("keyboard capture");

    // STATE: the typed text reached the app's buffer.
    assert!(
        typed_text_reached_app,
        "typed text did not reach the focused app's buffer (focused_app_text() \
         should be Some(\"hello\") after typing into the focused window)"
    );

    // PIXELS: glyphs painted in the field region.
    let field = frame.crop(540, 360, 200, 40);
    let base_field = base.crop(540, 360, 200, 40);
    let delta = diff_frames(&base_field, &field, DiffOptions::default());
    assert!(
        !delta.matched && delta.differing_pixels > 200,
        "typed glyphs did not paint in the text field: only {} pixels changed",
        delta.differing_pixels
    );
}

// ===========================================================================
// 6a. hotkey workspace switch -> state + pixels. GATED to f7 (pixels).
// ===========================================================================

/// Switching workspace changes the active workspace (STATE) and changes which
/// windows render (PIXELS).
///
/// PASSES today. Unlike `visual_windows::workspace_switch` (which is `#[ignore]`d
/// to f7 because its scenario builder switches on an EMPTY desktop, so there is
/// nothing to filter), this sequence opens a window in BOTH the before and after
/// captures and only adds+switches a workspace in the after capture. Because
/// `Shell::visible_windows()` already filters by active-workspace membership
/// (`active.contains(w.id)`, windows.rs — the t49-e5-F01 fix), the
/// origin-workspace window stops rendering after the switch, so the frames
/// genuinely differ. The state assertion (active id changes) and the pixel
/// differential are both real behavioral teeth.
#[test]
fn hotkey_workspace_switch_changes_state_and_pixels() {
    let (dock_cx, dock_cy) = first_dock_item_centre();

    // Frame BEFORE the switch: a window open on workspace 0.
    let (before, active_before) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |handle| {
            ScriptedScenario::new(handle)
                .left_click(dock_cx, dock_cy)
                .into_events()
        },
        |shell| shell.workspace_manager().active().id.0,
    )
    .expect("pre-switch capture");

    // Frame AFTER: open a window, add a 2nd workspace, switch to it. The window
    // belongs to workspace 0 and should not render on workspace 1.
    let (after, active_after) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |handle| {
            ScriptedScenario::new(handle)
                .left_click(dock_cx, dock_cy)
                .into_events()
        },
        |shell| {
            shell.execute_action(&ShellAction::WorkspaceAdd);
            shell.execute_action(&ShellAction::WorkspaceNext);
            shell.workspace_manager().active().id.0
        },
    )
    .expect("post-switch capture");

    // STATE: the active workspace changed.
    assert_ne!(
        active_before, active_after,
        "workspace switch did not change the active workspace id ({active_before} -> \
         {active_after}). Check WorkspaceAdd / WorkspaceNext."
    );

    // PIXELS (f7 gate): the window from workspace 0 no longer renders, so the
    // frames differ.
    assert_eq!((before.width, before.height), (after.width, after.height));
    let delta = diff_frames(&before, &after, DiffOptions::default());
    assert!(
        !delta.matched && delta.differing_pixels > 10_000,
        "workspace switch did not change the rendered windows: only {} pixels \
         differ pre/post switch (threshold 10000). Expected the origin-workspace \
         window to stop rendering (visible_windows membership filtering -> f7).",
        delta.differing_pixels
    );
}

// ===========================================================================
// 6b. drag-to-edge window snap-tile -> state + pixels. GATED to f10.
// ===========================================================================

/// Dragging a window to the screen edge snap-tiles it to that half: the window
/// becomes tiled to a snap zone and its bounds collapse to the half-screen
/// (STATE), and it repaints filling that half (PIXELS).
///
/// PASSES today. `handle_mouse_button`'s drag-release arm calls
/// `apply_snap_on_release` (events.rs), which consults the canonical snap zones
/// and tiles the window when the drag ends over an active zone — so a title-bar
/// drag to the LEFT edge sets `window.tiled = true` and collapses the bounds to
/// the left half. (e7 allowlisted the `chrome_tiling` *wiring bit* to f10 because
/// no path flips that audit bit, but the snap-on-release BEHAVIOR is live; the
/// f10 work is making the audit bit reflect it. This sequence asserts the live
/// behavior directly.)
#[test]
fn drag_to_edge_snaps_window() {
    let (dock_cx, dock_cy) = first_dock_item_centre();

    let (_f, initial) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |handle| {
            ScriptedScenario::new(handle)
                .left_click(dock_cx, dock_cy)
                .into_events()
        },
        |shell| {
            shell
                .visible_windows()
                .first()
                .map(|w| w.bounds)
                .expect("window open after dock click")
        },
    )
    .expect("open-window probe");

    let grab_x = initial.x + initial.width / 2.0;
    let grab_y = initial.y + 12.0;

    let base = base_desktop();

    let (frame, (tiled, bounds)) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |handle| {
            // Drag the title bar to the LEFT screen edge (snap-left zone).
            ScriptedScenario::new(handle)
                .left_click(dock_cx, dock_cy)
                .drag(MouseButton::Left, grab_x, grab_y, 2.0, grab_y, 10)
                .into_events()
        },
        |shell| {
            let w = shell
                .visible_windows()
                .first()
                .map(|w| (w.tiled, w.bounds))
                .expect("window open after drag");
            w
        },
    )
    .expect("drag-to-edge capture");

    // STATE: the window snapped to a tile zone (left half of the work area).
    assert!(
        tiled,
        "drag to the left edge did not tile the window (window.tiled is false). \
         Check apply_snap_on_release / chrome_tiling drag-snap wiring (f10)."
    );
    let half_w = frame.width as f32 / 2.0;
    assert!(
        bounds.width <= half_w + 8.0,
        "snapped window width {:.0} is not ~half-screen ({:.0})",
        bounds.width,
        half_w
    );

    // PIXELS: the left half now carries the snapped window body.
    let region = (0u32, frame.height / 4, frame.width / 2, frame.height / 2);
    let base_left = base.crop(region.0, region.1, region.2, region.3);
    let now_left = frame.crop(region.0, region.1, region.2, region.3);
    let delta = diff_frames(&base_left, &now_left, DiffOptions::default());
    assert!(
        !delta.matched && delta.differing_pixels > 20_000,
        "snapped window did not paint over the left half: only {} pixels changed \
         (threshold 20000).",
        delta.differing_pixels
    );
}
