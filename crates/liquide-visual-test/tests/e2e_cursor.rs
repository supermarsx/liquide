//! Cursor-movement artifact / residue regression suite (t66-cursor).
//!
//! REPRODUCES + GUARDS AGAINST: "artifacts when moving the mouse" — cursor
//! trails, residue, or smearing left at OLD pointer positions after the pointer
//! moves on.
//!
//! ## What is being tested
//!
//! The software cursor is drawn as the top-most [`FlatNode`] at flatten time
//! (`liquide-session/src/desktop/render_thread.rs`). Two render paths place it:
//!
//!  1. FULL render (`render_frame_sync` / `render_full_job`): rebuilds the whole
//!     scene with the cursor at the current position and clears the damaged
//!     region before repainting. This is the path the deterministic synchronous
//!     capture (`capture_desktop_scripted_sync`) exercises — it `mark_all`s
//!     full-frame damage, so a leftover cursor at the old position can only
//!     appear if the cursor node itself is duplicated or mis-placed.
//!  2. CURSOR-ONLY render (`submit_cursor_only_render` ->
//!     `RenderMsg::CursorOnly`): reuses the cached scene and repaints ONLY the
//!     union of the old-cursor and new-cursor tile regions
//!     (`render_thread.rs` ~L1073-1121). This is the partial-damage fast path
//!     where residue bugs hide: if the OLD cursor tiles are not in the damage
//!     set (or are not cleared/repainted), the cursor smears. It is reached on
//!     the threaded `run()` loop (`event_loop.rs:268/311`) when the pointer
//!     moves and nothing else is dirty.
//!
//! ## Strategy (no hardcoded cursor colors)
//!
//! Every assertion is differential. We define a "cursor signature" for a region
//! as the count of pixels in that region that differ (beyond an AA tolerance)
//! from the SAME region of a REFERENCE frame in which the cursor is known to be
//! elsewhere. A cursor present in a region => large signature; a clean
//! (residue-free) region => signature at/near zero. This needs no assumption
//! about the cursor's exact pixels, only that drawing a 24px high-contrast
//! cursor over the wallpaper changes a meaningful number of pixels.
//!
//! Positions are chosen on the open wallpaper (clear of the top status bar and
//! the bottom dock band) so a residue is unambiguous and not confused with
//! chrome.

use liquide_input::mouse::MouseEvent;
use liquide_platform::{NativeWindowHandle, PlatformEvent};
use liquide_visual_test::diff::{DiffOptions, diff_frames};
use liquide_visual_test::scenarios::{SCENARIO_HEIGHT, SCENARIO_WIDTH, scenario_options};
use liquide_visual_test::{
    CaptureOptions, Frame, capture_desktop, capture_desktop_scripted,
    capture_desktop_scripted_sync,
};

const THEME: &str = "liquid-glass";

/// Software cursor box size (mirrors `cursor_state::CURSOR_SIZE = 24`). We crop a
/// slightly larger window around a position so the whole glyph + AA fringe is in
/// the region regardless of sub-pixel placement.
const CURSOR_BOX: u32 = 24;
const PAD: u32 = 8;

/// AA-tolerant per-channel delta for "this pixel changed".
const TOL: u8 = 16;

/// A region (x, y, w, h) centred on the cursor's TOP-LEFT origin `(px, py)`.
///
/// The cursor draws from its top-left at the position, so we anchor the region a
/// few px above/left and make it generous enough to capture the full glyph.
fn cursor_region(px: f32, py: f32) -> (u32, u32, u32, u32) {
    let x = (px as i64 - PAD as i64).max(0) as u32;
    let y = (py as i64 - PAD as i64).max(0) as u32;
    let w = CURSOR_BOX + PAD * 2;
    let h = CURSOR_BOX + PAD * 2;
    (
        x.min(SCENARIO_WIDTH.saturating_sub(1)),
        y.min(SCENARIO_HEIGHT.saturating_sub(1)),
        w.min(SCENARIO_WIDTH - x.min(SCENARIO_WIDTH - 1)),
        h.min(SCENARIO_HEIGHT - y.min(SCENARIO_HEIGHT - 1)),
    )
}

/// "Cursor signature" of `region` in `frame` measured against `reference`: the
/// number of pixels that differ (beyond [`TOL`]) between the two frames' crops
/// of the same region. High => something (the cursor) is present in `frame` that
/// is absent in `reference`. Near-zero => the region matches the reference
/// (no cursor, no residue).
fn signature(frame: &Frame, reference: &Frame, region: (u32, u32, u32, u32)) -> usize {
    let (x, y, w, h) = region;
    let a = frame.crop(x, y, w, h);
    let b = reference.crop(x, y, w, h);
    let res = diff_frames(
        &b,
        &a,
        DiffOptions {
            per_channel_tolerance: TOL,
            // No budget: we want the raw differing-pixel count, so make the
            // budget unreachable and read `differing_pixels` off the result.
            max_differing_pixels: usize::MAX,
        },
    );
    res.differing_pixels
}

/// Capture a fully clean baseline desktop (cursor parked at the centre — the
/// default `DesktopCompositor::new` position width/2,height/2) with NO scripted
/// input. Used as the residue reference for the wallpaper positions, which are
/// all far from the centre.
fn clean_baseline() -> Frame {
    capture_desktop(&scenario_options(THEME)).expect("clean baseline capture")
}

/// Move the cursor to `(x, y)` via the deterministic synchronous full-render
/// capture and return the resulting frame. This is the same path every other
/// e2e test uses; it guarantees the post-move frame is the one read back.
fn move_to_sync(x: f32, y: f32) -> Frame {
    capture_desktop_scripted_sync(&scenario_options(THEME), |h| {
        vec![PlatformEvent::MouseInput {
            handle: h,
            event: MouseEvent::Move { x, y },
        }]
    })
    .expect("scripted sync move capture")
}

/// Three open-wallpaper probe positions, well clear of the status bar (top 36px)
/// and dock band (bottom ~96px), and far enough apart that their cursor regions
/// never overlap.
const A: (f32, f32) = (260.0, 220.0);
const B: (f32, f32) = (760.0, 300.0);
const C: (f32, f32) = (1040.0, 470.0);

/// Sanity floor: a drawn cursor must change at least this many pixels in its
/// region vs the cursor-absent reference. The 24px cursor glyph covers well over
/// this; the threshold rejects "cursor never drew" false-greens.
const PRESENT_MIN: usize = 120;

/// Residue ceiling: a vacated region must match the reference to within this many
/// pixels. Allows a thin AA fringe / sub-pixel rounding but rejects a leftover
/// cursor body (which would be 120+ px).
const RESIDUE_MAX: usize = 40;

// ===========================================================================
// 1. NO TRAIL (full-render path): cursor at A, then at B -> A clean, B has cursor
// ===========================================================================

#[test]
fn no_trail_a_then_b_full_render() {
    let baseline = clean_baseline();

    let frame_a = move_to_sync(A.0, A.1);
    let frame_b = move_to_sync(B.0, B.1);

    let region_a = cursor_region(A.0, A.1);
    let region_b = cursor_region(B.0, B.1);

    // 1a. Frame A actually drew a cursor at A (guards against "cursor never
    //     rendered", which would make every no-residue check vacuously pass).
    let cursor_at_a_in_a = signature(&frame_a, &baseline, region_a);
    assert!(
        cursor_at_a_in_a >= PRESENT_MIN,
        "expected a cursor drawn at A in frame_a (signature {cursor_at_a_in_a} < {PRESENT_MIN}); \
         the cursor never rendered — every residue check below would be vacuous"
    );

    // 1b. THE ARTIFACT CHECK: after moving to B, region A must be back to the
    //     cursor-free background. A leftover cursor at A is a trail FAIL.
    let residue_at_a = signature(&frame_b, &baseline, region_a);
    assert!(
        residue_at_a <= RESIDUE_MAX,
        "CURSOR TRAIL: after moving A->B, {residue_at_a} px around the OLD position A \
         still differ from the cursor-free baseline (residue ceiling {RESIDUE_MAX}). \
         A leftover cursor remained at A. Root-cause the old-cursor damage clear in \
         render_thread.rs."
    );

    // 1c. The cursor appears correctly at B.
    let cursor_at_b_in_b = signature(&frame_b, &baseline, region_b);
    assert!(
        cursor_at_b_in_b >= PRESENT_MIN,
        "expected the cursor at the NEW position B in frame_b (signature \
         {cursor_at_b_in_b} < {PRESENT_MIN}); the cursor did not follow the pointer to B"
    );
}

// ===========================================================================
// 2. MULTI-STEP MOVE (full-render path): A->B->C, only the last has a cursor
// ===========================================================================

#[test]
fn multi_step_move_no_accumulation_full_render() {
    let baseline = clean_baseline();

    let region_a = cursor_region(A.0, A.1);
    let region_b = cursor_region(B.0, B.1);
    let region_c = cursor_region(C.0, C.1);

    // Walk A -> B -> C, capturing the post-move frame at each step. After each
    // step ONLY the current position may carry cursor pixels; all previously
    // visited positions must be clean (no accumulation / smear).
    let _f_a = move_to_sync(A.0, A.1);
    let f_b = move_to_sync(B.0, B.1);
    let f_c = move_to_sync(C.0, C.1);

    // After arriving at B: A clean, B has cursor.
    assert!(
        signature(&f_b, &baseline, region_a) <= RESIDUE_MAX,
        "ACCUMULATION: residue at A after step A->B"
    );
    assert!(
        signature(&f_b, &baseline, region_b) >= PRESENT_MIN,
        "cursor missing at B after step A->B"
    );

    // After arriving at C: A clean, B clean, C has cursor.
    assert!(
        signature(&f_c, &baseline, region_a) <= RESIDUE_MAX,
        "SMEAR: residue at A after step B->C (cursor accumulated across two moves)"
    );
    assert!(
        signature(&f_c, &baseline, region_b) <= RESIDUE_MAX,
        "SMEAR: residue at B after step B->C (old cursor not cleared)"
    );
    assert!(
        signature(&f_c, &baseline, region_c) >= PRESENT_MIN,
        "cursor missing at final position C"
    );
}

// ===========================================================================
// 3. CURSOR-ONLY PATH (threaded run, partial damage): the residue hot-spot
// ===========================================================================

/// Drive the THREADED `run()` loop (the real binary's loop) with a sequence of
/// pointer moves so the cursor-only partial-damage fast path
/// (`submit_cursor_only_render`) is exercised, then assert no residue is left at
/// the intermediate positions in the last presented frame.
///
/// NOTE on determinism: `capture_desktop_scripted` appends a trailing `Quit`,
/// and the event loop drains queued events in one batch; the cursor-only render
/// + present can lag the final move. So we assert the robust invariant that
/// holds regardless of WHICH of the visited positions ended up being the final
/// rendered one: AT MOST ONE of the visited wallpaper positions may carry a
/// cursor, and it must be the LAST one that actually rendered. Concretely: the
/// two EARLIER positions (A, B) must be residue-free. If even those smear, the
/// cursor-only damage path failed to clear vacated tiles.
#[test]
fn cursor_only_path_no_residue_threaded_run() {
    let baseline = clean_baseline();

    let frame = capture_desktop_scripted(&scenario_options(THEME), |h: NativeWindowHandle| {
        // Several discrete moves across the open wallpaper, each of which (on the
        // threaded loop) triggers a cursor-only render against the prior position.
        vec![
            mv(h, A.0, A.1),
            mv(h, B.0, B.1),
            mv(h, C.0, C.1),
        ]
    })
    .expect("threaded scripted cursor-move capture");

    let region_a = cursor_region(A.0, A.1);
    let region_b = cursor_region(B.0, B.1);
    let region_c = cursor_region(C.0, C.1);

    let sig_a = signature(&frame, &baseline, region_a);
    let sig_b = signature(&frame, &baseline, region_b);
    let sig_c = signature(&frame, &baseline, region_c);

    // Whichever frame was last presented, the two positions the cursor LEFT must
    // be clean. A and B were both vacated (the pointer ended its script at C);
    // even if present-pacing means C is the rendered one or a slightly earlier
    // frame is the last presented, A is unambiguously a vacated position that
    // must not retain a cursor.
    assert!(
        sig_a <= RESIDUE_MAX,
        "CURSOR-ONLY TRAIL: vacated position A retains {sig_a} px of cursor residue \
         (ceiling {RESIDUE_MAX}) on the threaded partial-damage path. The old-cursor \
         tiles were not cleared/repainted by the CursorOnly render \
         (render_thread.rs damage union ~L1084-1107 / clear_damage_tiles ~L1109)."
    );

    // At least one visited position must show the cursor (else the cursor never
    // rendered on the threaded path and the residue check is vacuous).
    assert!(
        sig_a >= PRESENT_MIN || sig_b >= PRESENT_MIN || sig_c >= PRESENT_MIN,
        "no cursor found at ANY visited position on the threaded run \
         (A={sig_a}, B={sig_b}, C={sig_c}); the cursor-only path produced no \
         visible cursor — cannot validate residue"
    );

    // No SMEAR: the cursor must occupy AT MOST ONE position (no accumulation
    // across the multi-move sequence). Count positions that look "occupied".
    let occupied = [sig_a, sig_b, sig_c]
        .iter()
        .filter(|&&s| s >= PRESENT_MIN)
        .count();
    assert!(
        occupied <= 1,
        "CURSOR SMEAR: the cursor appears at {occupied} of the 3 visited positions \
         (A={sig_a}, B={sig_b}, C={sig_c}); a single pointer should leave a single \
         cursor. Old positions were not cleared on the cursor-only path."
    );
}

/// Build a pointer-move PlatformEvent.
fn mv(h: NativeWindowHandle, x: f32, y: f32) -> PlatformEvent {
    PlatformEvent::MouseInput {
        handle: h,
        event: MouseEvent::Move { x, y },
    }
}

// ===========================================================================
// 4. CURSOR OVER CONTENT: moving over content does not corrupt it afterwards
// ===========================================================================

/// Render a desktop with a window/menu present, move the cursor OVER that
/// content, then move it AWAY, and assert the content region is byte-stable
/// against the same content without the cursor having passed over it.
///
/// We use the context-menu overlay as the "content": a right-click opens a menu
/// at a known wallpaper point; the menu paints a solid panel. We then capture
/// the same menu with the cursor parked elsewhere and compare the menu interior
/// region — if hovering corrupted the underlying content (e.g. the cursor-only
/// path repainted a tile with stale/garbage pixels), the interiors diverge.
#[test]
fn cursor_over_content_leaves_no_corruption() {
    // Menu anchor on the open wallpaper.
    let menu_x = 520.0;
    let menu_y = 360.0;

    // Reference: open the menu and leave the cursor on the menu's spawn point
    // (where the right-click landed). This is the canonical menu appearance.
    let menu_ref = capture_desktop_scripted_sync(&scenario_options(THEME), |h| {
        right_click(h, menu_x, menu_y)
    })
    .expect("menu reference capture");

    // Hover the cursor across the menu body, then move it far away. After the
    // cursor leaves, the menu content underneath must be intact.
    let menu_after_hover = capture_desktop_scripted_sync(&scenario_options(THEME), |h| {
        let mut ev = right_click(h, menu_x, menu_y);
        // Drag the pointer down through the menu body, then off to a far corner.
        ev.push(mv(h, menu_x + 20.0, menu_y + 40.0));
        ev.push(mv(h, menu_x + 20.0, menu_y + 80.0));
        ev.push(mv(h, 80.0, 80.0)); // park far away (top-left)
        ev
    })
    .expect("menu after-hover capture");

    // Compare the menu interior — BELOW the spawn point so neither frame has the
    // cursor sitting in this sub-region (the ref's cursor is at the spawn point /
    // top of the menu; the hovered frame's cursor is parked at the far corner).
    // Any difference here is content corruption left behind by the cursor.
    let interior = (
        menu_x as u32 + 4,
        menu_y as u32 + 110,
        140u32,
        60u32,
    );
    let (ix, iy, iw, ih) = interior;
    let a = menu_ref.crop(ix, iy, iw, ih);
    let b = menu_after_hover.crop(ix, iy, iw, ih);

    let res = diff_frames(
        &a,
        &b,
        DiffOptions {
            per_channel_tolerance: TOL,
            max_differing_pixels: RESIDUE_MAX,
        },
    );

    // Guard: the interior must be non-uniform in the reference (i.e. it really
    // contains menu content), otherwise the comparison is vacuous.
    assert!(
        !a.is_uniform(),
        "menu interior reference region is uniform — the menu may not have opened; \
         cannot validate content-under-cursor corruption"
    );

    assert!(
        res.matched,
        "CONTENT CORRUPTION: after the cursor hovered over the menu and left, the \
         menu interior differs from the un-hovered reference by {} px (max delta \
         {}). The cursor-only damage path repainted content tiles incorrectly when \
         the cursor passed over them.",
        res.differing_pixels, res.max_channel_delta
    );
}

/// A right-click (move + press + release) at `(x, y)` to open a context menu.
fn right_click(h: NativeWindowHandle, x: f32, y: f32) -> Vec<PlatformEvent> {
    use liquide_input::mouse::{ButtonState, MouseButton};
    vec![
        mv(h, x, y),
        PlatformEvent::MouseInput {
            handle: h,
            event: MouseEvent::Button {
                button: MouseButton::Right,
                state: ButtonState::Pressed,
                x,
                y,
            },
        },
        PlatformEvent::MouseInput {
            handle: h,
            event: MouseEvent::Button {
                button: MouseButton::Right,
                state: ButtonState::Released,
                x,
                y,
            },
        },
    ]
}

// ===========================================================================
// Surface-size sanity (keeps the probe positions inside the canonical surface)
// ===========================================================================

#[test]
fn probe_positions_are_on_surface_and_disjoint() {
    let opts: CaptureOptions = scenario_options(THEME);
    assert_eq!(opts.width, SCENARIO_WIDTH);
    assert_eq!(opts.height, SCENARIO_HEIGHT);
    for (name, (px, py)) in [("A", A), ("B", B), ("C", C)] {
        let (x, y, w, h) = cursor_region(px, py);
        assert!(
            x + w <= SCENARIO_WIDTH && y + h <= SCENARIO_HEIGHT,
            "probe {name} region escapes the surface"
        );
        // Clear of the top status bar (36) and bottom dock band (~96).
        let py_top = py;
        let py_bottom = py + CURSOR_BOX as f32 + PAD as f32;
        let dock_top = SCENARIO_HEIGHT as f32 - 96.0;
        assert!(py_top > 36.0 + PAD as f32, "probe {name} overlaps the status bar");
        assert!(py_bottom < dock_top, "probe {name} overlaps the dock band");
    }
}
