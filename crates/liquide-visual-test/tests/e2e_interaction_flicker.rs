//! E2E INTERACTION-FLICKER suite (task t72, executor t72-flicker).
//!
//! User report: "flickering all across the DE" on interactions — right-click
//! menus especially, plus hover/click/drag/open/close/repeat. This suite
//! reproduces (or proves absent) each *interaction*-driven flicker source the
//! coordinator flagged and, for every property that holds, carries a teeth proof
//! so it cannot pass vacuously.
//!
//! ## The four suspected interaction-flicker sources (coordinator brief)
//!
//!   1. **render_live glyph pop-in (PRIME NEW SUSPECT).** t69 made live render
//!      non-blocking: `render_live(LiveFull)` waits at most
//!      `LIVE_GLYPH_DRAIN_BUDGET_MS = 4 ms` for glyphs, paints with whatever is
//!      ready, sets `has_pending_glyphs`, and the session schedules a follow-up.
//!      On a NEW surface (menu open) the first painted frame can show text as
//!      missing/notdef, then a later frame fills it → a 1-frame blank-then-fill
//!      flash.
//!   2. **Origin-twin glass.** A duplicate glass panel appearing at (0,0) when a
//!      menu opens, then vanishing — an element appearing/disappearing = flicker.
//!      (Being addressed by peer t71-fix; this suite verifies whether it still
//!      reproduces in the captured scene.)
//!   3. **Blur-cache churn.** Per-frame scene-node-ID churn defeats the blur
//!      cache (keyed on `node.id`, see `liquide-renderer-cpu/.../effects.rs`),
//!      so glass blur recomputes frame-to-frame on a STEADY surface → blur
//!      flicker. Detectable as byte-instability across identical-state renders.
//!   4. **Present cadence / full rebuild on interaction.** Opening/closing a menu
//!      or hovering triggers an oscillating present/redraw or scene rebuild that
//!      flashes. Detectable as the post-interaction steady state failing to be
//!      byte-identical to the equivalent direct state, or a closed surface not
//!      returning to the bare-desktop bytes.
//!
//! ## HONESTY BOUNDARY — what this offscreen CPU path can and cannot see
//!
//! Every capture here renders through the DETERMINISTIC, single-threaded headless
//! path (`DesktopCompositor::capture_once*`), which uses the renderer's
//! `Renderer::render` entry — **`RenderMode::Capture`**, which BLOCK-DRAINS glyphs
//! (`GLYPH_DRAIN_BUDGET_MS`) and additionally does a glyph-reflush pass
//! (`render_thread.rs` step 5). The live desktop loop instead uses
//! `render_live(LiveFull)` with the 4 ms budget and NO guaranteed in-frame
//! reflush. So:
//!
//!   - This suite CAN prove the *content* of a captured menu/hover/click/drag
//!     frame is complete and that identical logical states render byte-identically
//!     (covers sources 2, 3, 4 — origin-twin, blur-cache churn, rebuild churn).
//!   - This suite CANNOT, through the capture seam alone, reproduce the *live*
//!     glyph pop-in (source 1): the capture path is specifically immune to it
//!     (block-drain + reflush). The relevant test below therefore asserts the
//!     capture-path INVARIANT (first menu frame text is complete) — which holds —
//!     and the live divergence is ROOT-CAUSED by code inspection in the test doc
//!     and the t72 log, with the fix crate named. We do NOT fake a red here for a
//!     race the seam cannot exercise; we state the boundary plainly.
//!
//! Additionally (seam limitation, same as e2e_temporal): `StandalonePlatform`
//! retains only the LAST presented frame, so the threaded `run()` path cannot be
//! frame-stepped. Consecutive-frame assertions are therefore expressed as
//! independent deterministic captures of each logical state (the capture path is
//! stateless per call), which is the strongest signal the API allows.

use liquide_input::keyboard::{KeyCode, Modifiers};
use liquide_input::mouse::MouseButton;
use liquide_visual_test::diff::{DiffOptions, diff_frames};
use liquide_visual_test::scenarios::{
    ScriptedScenario, scenario_options, themed_desktop_capture,
};
use liquide_visual_test::{
    Frame, capture_desktop_scripted_readback, capture_desktop_scripted_sync,
};

const THEME: &str = "liquid-glass";

// ── Menu geometry (mirrors the shell + e2e_context_menu) ────────────────────
const MENU_ITEM_HEIGHT: f32 = 28.0;
const MENU_PADDING: f32 = 4.0;
const CONTEXT_MENU_WIDTH: f32 = 200.0;
const CONTEXT_MENU_ITEMS: usize = 5;
const CONTEXT_MENU_HEIGHT: f32 =
    MENU_PADDING * 2.0 + CONTEXT_MENU_ITEMS as f32 * MENU_ITEM_HEIGHT; // 148

/// The five desktop context-menu labels (mirrors `ContextMenuItem::defaults()`).
const EXPECTED_LABELS: [&str; CONTEXT_MENU_ITEMS] = [
    "Open Terminal",
    "Open File Manager",
    "Change Wallpaper",
    "Display Settings",
    "System Settings",
];

// ===========================================================================
// Helpers
// ===========================================================================

/// Exact byte-for-byte frame equality. Two captures of the SAME logical state,
/// both at time `t0` on the single-threaded path, MUST be identical — any
/// difference is render nondeterminism / content flicker.
fn frames_byte_identical(a: &Frame, b: &Frame) -> bool {
    a.width == b.width && a.height == b.height && a.rgba == b.rgba
}

/// Raw count of pixels differing beyond a tiny per-channel tolerance.
fn differing_pixels(a: &Frame, b: &Frame) -> usize {
    diff_frames(
        a,
        b,
        DiffOptions {
            per_channel_tolerance: 2,
            max_differing_pixels: 0,
        },
    )
    .differing_pixels
}

/// The painted top-left of the context menu after a right-click at `(rx, ry)`
/// that fits on screen (no clamping) — equals the click point.
fn menu_origin(rx: f32, ry: f32) -> (u32, u32) {
    (rx.round() as u32, ry.round() as u32)
}

/// Bare desktop (no menu/windows) — the reference for "menu closed" and corner
/// probes.
fn base_desktop() -> Frame {
    themed_desktop_capture(THEME).expect("base desktop capture")
}

/// Capture the desktop after a right-click at `(rx, ry)` via the deterministic
/// synchronous path (menu visible in the read-back frame).
fn capture_right_click(rx: f32, ry: f32) -> Frame {
    capture_desktop_scripted_sync(&scenario_options(THEME), |handle| {
        ScriptedScenario::new(handle).right_click(rx, ry).into_events()
    })
    .expect("right-click capture")
}

/// A CURSOR-NEUTRAL baseline: the bare desktop with the SOFTWARE CURSOR moved to
/// `(fx, fy)` and nothing else done. This is the correct reference for any
/// interaction whose pointer ends at `(fx, fy)`.
///
/// WHY THIS EXISTS (a real measurement, documented so the suite is not naive):
/// the capture path renders a SOFTWARE cursor (StandaloneConfig::hardware_cursor
/// = false). A click/drag/menu interaction MOVES the pointer, so the captured
/// end-frame draws the cursor at the new spot AND no longer draws it at the
/// desktop's default centre. Comparing such a frame to the plain centre-cursor
/// `base_desktop()` reports ~664 px differing — but that is ENTIRELY the cursor's
/// own pixels at two positions (measured: 332 px removed from centre + 332 px
/// added at the new spot; the "black" pixels are the cursor's outline, not a
/// clear-without-repaint bug). That is correct cursor tracking, NOT flicker. To
/// test for genuine *interaction residue* we cancel the cursor by comparing
/// against a baseline whose cursor is at the SAME final position; any remaining
/// diff is then real residue. (Verified: click / menu-open+escape / drag-release
/// each diff this baseline by 0 px — no residue.) The cancellation is exact, so
/// the test keeps full teeth: real residue away from the cursor still shows.
fn cursor_neutral_baseline(fx: f32, fy: f32) -> Frame {
    capture_desktop_scripted_sync(&scenario_options(THEME), move |handle| {
        ScriptedScenario::new(handle).pointer_move(fx, fy).into_events()
    })
    .expect("cursor-neutral baseline capture")
}

/// Per-column "ink" profile in a band: pixels whose luma-distance from the band
/// mean is large (glyph strokes / icon marks against the panel fill). Mirrors the
/// `column_ink_profile` used by `e2e_context_menu`, kept local so this file owns
/// its own measurement.
fn column_ink_profile(frame: &Frame, x: u32, y: u32, w: u32, h: u32) -> Vec<usize> {
    let x1 = (x + w).min(frame.width);
    let y1 = (y + h).min(frame.height);
    if x1 <= x || y1 <= y {
        return vec![0; w as usize];
    }
    let (mut sr, mut sg, mut sb) = (0u64, 0u64, 0u64);
    let mut count = 0u64;
    for py in y..y1 {
        for px in x..x1 {
            let p = frame.pixel(px, py).unwrap();
            sr += p[0] as u64;
            sg += p[1] as u64;
            sb += p[2] as u64;
            count += 1;
        }
    }
    let (mr, mg, mb) = ((sr / count) as i32, (sg / count) as i32, (sb / count) as i32);
    let mut profile = vec![0usize; (x1 - x) as usize];
    for px in x..x1 {
        let mut ink = 0usize;
        for py in y..y1 {
            let p = frame.pixel(px, py).unwrap();
            let d = (p[0] as i32 - mr).abs()
                + (p[1] as i32 - mg).abs()
                + (p[2] as i32 - mb).abs();
            if d > 90 {
                ink += 1;
            }
        }
        profile[(px - x) as usize] = ink;
    }
    profile
}

/// Total label ink across all 5 menu item rows of a menu opened at `(ox, oy)`.
/// A fully-painted menu carries thousands of ink pixels; a blank-text
/// (glyph-pop-in) menu reads near-zero.
fn menu_total_label_ink(frame: &Frame, ox: u32, oy: u32) -> usize {
    let mut total = 0usize;
    for i in 0..CONTEXT_MENU_ITEMS {
        let row_y = oy + (MENU_PADDING + i as f32 * MENU_ITEM_HEIGHT).round() as u32;
        let row_h = MENU_ITEM_HEIGHT as u32;
        let profile = column_ink_profile(frame, ox, row_y, CONTEXT_MENU_WIDTH as u32, row_h);
        total += profile.iter().sum::<usize>();
    }
    total
}

/// Count rows (of the 5) that carry a healthy spread of label ink.
fn menu_inked_rows(frame: &Frame, ox: u32, oy: u32) -> usize {
    let mut inked = 0usize;
    for i in 0..CONTEXT_MENU_ITEMS {
        let row_y = oy + (MENU_PADDING + i as f32 * MENU_ITEM_HEIGHT).round() as u32;
        let row_h = MENU_ITEM_HEIGHT as u32;
        let profile = column_ink_profile(frame, ox, row_y, CONTEXT_MENU_WIDTH as u32, row_h);
        let ink_cols = profile.iter().filter(|&&c| c > 0).count();
        let total_ink: usize = profile.iter().sum();
        if ink_cols >= 12 && total_ink >= 40 {
            inked += 1;
        }
    }
    inked
}

// ===========================================================================
// SOURCE 1 — render_live glyph pop-in (PRIME SUSPECT)
// ===========================================================================
//
// CONTRACT under test (capture path): the FIRST captured frame of a freshly
// opened context menu must show EVERY item's label text complete — no
// blank-then-fill. The capture path block-drains glyphs + reflushes, so this
// invariant HOLDS here. That is the point: it proves the *capture* surface is
// safe and gives the live path a concrete target (the live first menu frame must
// reach this same completeness, not pop in over 1-2 frames).
//
// LIVE ROOT CAUSE (cannot be reproduced through this seam — stated, not faked):
// the interactive loop opens the menu, marks dirty, and `submit_render` →
// `render_full_job` → `renderer.render_live(.., RenderMode::LiveFull)`
// (render_thread.rs cursor-only path uses LiveCursor; the full path uses the job
// mode). `render_with_mode` (renderer/mod.rs:866) under LiveFull waits only
// `LIVE_GLYPH_DRAIN_BUDGET_MS = 4 ms` (renderer/mod.rs:77) for the menu's glyphs.
// The menu labels are NEW glyph keys for the menu's font/size, requested for the
// first time in `text.rs` first-pass (text.rs:156-166), which sets
// `has_pending_glyphs = true`. If the font worker has not rasterised them within
// 4 ms, the first menu frame paints with glyphs MISSING (the atlas `get` returns
// None so nothing is drawn for that char) → a blank-text menu for one frame. The
// session then schedules ONE damage-only follow-up (`schedule_glyph_fill_resubmit`,
// render_thread.rs:653) and the next frame fills the text → the 1-frame flash.

#[test]
fn glyph_popin_first_menu_frame_text_is_complete_capture_path() {
    let (rx, ry) = (300.0_f32, 250.0_f32);
    let frame = capture_right_click(rx, ry);
    let (ox, oy) = menu_origin(rx, ry);

    let inked = menu_inked_rows(&frame, ox, oy);
    let total_ink = menu_total_label_ink(&frame, ox, oy);

    // Every one of the 5 labelled rows must carry readable glyph ink on the FIRST
    // (and only) captured menu frame. A blank-then-fill pop-in would leave one or
    // more rows under-inked on this frame.
    assert_eq!(
        inked, CONTEXT_MENU_ITEMS,
        "GLYPH POP-IN on the capture path: only {inked}/{CONTEXT_MENU_ITEMS} menu rows have \
         complete label ink on the first menu frame (total_ink={total_ink}). The deterministic \
         capture path block-drains glyphs + reflushes, so this MUST be complete; if it is not, \
         the block-drain/reflush guarantee (renderer render() RenderMode::Capture + \
         render_thread.rs step 5) has regressed. Labels expected: {EXPECTED_LABELS:?}"
    );
    // Quantify a healthy ink floor so a near-blank (notdef boxes only) menu fails.
    assert!(
        total_ink > 400,
        "first menu frame carries only {total_ink} label-ink px across 5 rows — far below a \
         fully-rendered menu (blank/notdef text pop-in)."
    );
}

/// Two independent captures of a freshly opened menu must render BYTE-IDENTICAL
/// text. If the glyph drain were racy on the capture path (the live pop-in
/// leaking in), the two captures would paint different amounts of text and
/// diverge. Byte-identity here proves the capture path's glyph handling is
/// deterministic (and is the teeth for the completeness test: it is stable, not
/// just "complete this once").
#[test]
fn glyph_popin_menu_text_is_deterministic_across_captures() {
    let (rx, ry) = (300.0_f32, 250.0_f32);
    let a = capture_right_click(rx, ry);
    let b = capture_right_click(rx, ry);
    assert!(
        frames_byte_identical(&a, &b),
        "the opened context menu is NOT deterministic across two captures: {} px differ. A racy \
         glyph drain (the live pop-in mechanism leaking onto the capture path) would paint \
         different text each run.",
        differing_pixels(&a, &b)
    );
}

// ===========================================================================
// SOURCE 2 — origin-twin glass (duplicate panel at (0,0) on menu open)
// ===========================================================================
//
// The reported symptom: opening a menu makes a duplicate glass panel flash at the
// screen ORIGIN (0,0) that is not there on the bare desktop and is not the menu
// (the menu is at the click point). We probe the top-left corner region: opening
// a menu far from the origin must NOT introduce a panel-sized glass artifact at
// (0,0). The menu open does lay a faint full-screen scrim (a uniform dim), which
// is legitimate; an origin TWIN is a structured PANEL (borders + item ink), which
// reads as concentrated INK, not a uniform dim. We measure ink (deviation from
// the corner's own mean), so a uniform scrim reads ~0 while a twin panel reads
// high.

#[test]
fn origin_twin_no_glass_panel_at_origin_on_menu_open() {
    // Open the menu far from the origin so a correctly-placed menu cannot overlap
    // the corner probe.
    let (rx, ry) = (700.0_f32, 400.0_f32);
    let frame = capture_right_click(rx, ry);

    // Probe a menu-sized region anchored at the origin but BELOW the status bar
    // (rows 40..188), so the legitimate status-bar chrome is excluded.
    let probe_x = 0u32;
    let probe_y = 40u32;
    let probe_w = CONTEXT_MENU_WIDTH as u32;
    let probe_h = CONTEXT_MENU_HEIGHT as u32;

    // INK probe (not diff-vs-base): a twin glass PANEL carries border + item ink;
    // the legitimate menu-open scrim is a uniform dim and carries ~0 ink. Sum the
    // per-column ink across the probe; a real panel would read in the hundreds+.
    let profile = column_ink_profile(&frame, probe_x, probe_y, probe_w, probe_h);
    let origin_ink: usize = profile.iter().sum();

    // Teeth reference: the REAL menu at the click point carries this much ink — we
    // compute it so the threshold is anchored to a real panel, not a guess.
    let (ox, oy) = menu_origin(rx, ry);
    let menu_ink = menu_total_label_ink(&frame, ox, oy);
    assert!(
        menu_ink > 400,
        "precondition: the real menu did not paint (menu_ink={menu_ink}) — cannot calibrate the \
         origin-twin probe"
    );

    assert!(
        origin_ink < menu_ink / 4,
        "ORIGIN-TWIN GLASS reproduces: opening a menu at ({rx},{ry}) put {origin_ink} ink px at \
         the screen origin (probe [{probe_x},{probe_y}] {probe_w}x{probe_h}) — comparable to the \
         real menu's {menu_ink} ink px. A duplicate glass panel at (0,0) is a flicker source \
         (it appears on open and vanishes on close). Fix crate: liquide-shell (scene/template \
         build) — confirm with peer t71-fix's origin-twin work."
    );
}

/// TEETH for the origin-twin probe: the probe measures real ink. Prove that the
/// SAME ink probe, pointed at the actual menu panel, reports a large value — so
/// the "< menu_ink/4" assertion above is not vacuously satisfied by an
/// always-zero probe.
#[test]
fn origin_twin_probe_has_teeth() {
    let (rx, ry) = (700.0_f32, 400.0_f32);
    let frame = capture_right_click(rx, ry);
    let (ox, oy) = menu_origin(rx, ry);
    // The probe applied to the real menu rect must see substantial ink.
    let profile = column_ink_profile(
        &frame,
        ox,
        oy,
        CONTEXT_MENU_WIDTH as u32,
        CONTEXT_MENU_HEIGHT as u32,
    );
    let menu_ink: usize = profile.iter().sum();
    assert!(
        menu_ink > 400,
        "origin-twin ink probe is toothless: applied to the real menu it reports only {menu_ink} \
         ink px (expected a panel's worth). The probe cannot distinguish a panel from blank, so \
         the origin-twin assertion would be vacuous."
    );
}

// ===========================================================================
// SOURCE 3 — blur-cache churn (per-frame node-ID churn defeats the blur cache)
// ===========================================================================
//
// The blur cache is keyed on `node.id` (liquide-renderer-cpu/.../effects.rs:82,
// blur_worker.rs get_cached/request_blur). If a glass surface's scene-node ID
// CHURNS frame-to-frame on a steady (unchanging) scene, every frame misses the
// cache → re-requests blur → the async blur result can lag/flash. The observable
// content tell-tale is byte-INSTABILITY across two identical-state renders of a
// scene that contains glass (the menu panel, the dock, the status bar). If node
// IDs were churning, the captured menu would not be byte-stable run to run.
//
// NOTE: the capture path's `render()` block-drains blur via `poll_results` and is
// synchronous, so a single capture settles; the discriminating signal for
// *churn* is whether two captures of the same state are identical AND whether the
// menu-open scene is identical to itself. A non-deterministic node-ID source
// (counter seeded by time / a monotonic that resets differently) would surface
// as a diff here.

#[test]
fn blur_cache_steady_menu_scene_is_byte_identical_across_renders() {
    // The open menu is a glass panel over the glass dock/bar — the densest glass
    // scene an interaction produces. Two identical-state captures must match to
    // the byte. A diff implies per-frame churn (node-ID or blur-cache) on a
    // steady surface.
    let (rx, ry) = (500.0_f32, 300.0_f32);
    let a = capture_right_click(rx, ry);
    let b = capture_right_click(rx, ry);
    assert!(
        frames_byte_identical(&a, &b),
        "BLUR-CACHE / NODE-ID CHURN: two identical-state captures of the open-menu glass scene \
         differ by {} px. On a steady surface the scene-node IDs (and thus the blur cache keyed \
         on node.id) must be stable; churn here is a frame-to-frame glass-blur flicker source. \
         Fix crate: liquide-shell (stable scene-node IDs) / liquide-renderer-cpu (blur cache).",
        differing_pixels(&a, &b)
    );
}

#[test]
fn blur_cache_steady_dock_band_stable_with_menu_open() {
    // Open a menu, then check the DOCK band (a persistent glass surface unrelated
    // to the menu) is byte-identical across two such captures. If opening a menu
    // churns unrelated glass nodes' IDs, the dock's blur would flicker even though
    // the dock did not change.
    let (rx, ry) = (500.0_f32, 300.0_f32);
    let a = capture_right_click(rx, ry);
    let b = capture_right_click(rx, ry);

    let dock_h = 96u32.min(a.height);
    let dock_a = a.crop(0, a.height - dock_h, a.width, dock_h);
    let dock_b = b.crop(0, b.height - dock_h, b.width, dock_h);
    assert!(
        frames_byte_identical(&dock_a, &dock_b),
        "the dock's glass band flickers across identical menu-open captures: {} px differ — an \
         unrelated persistent glass surface re-renders unstably when a menu is open (blur-cache / \
         node-ID churn).",
        differing_pixels(&dock_a, &dock_b)
    );
}

// ===========================================================================
// SOURCE 4 — present cadence / full rebuild on interaction
// ===========================================================================
//
// If opening/closing a menu, hovering, clicking, or dragging triggers an
// oscillating present/redraw or a scene rebuild that flashes, the post-
// interaction STEADY STATE will not match the equivalent direct state. We assert:
//   (a) a closed menu (open→close) returns the screen to the BARE-desktop bytes
//       (no stale paint, no leftover scrim) — element does not linger;
//   (b) repeated open/close converges to that same closed steady state;
//   (c) a hover on/off ends at the bare desktop (hover does not leave residue);
//   (d) the same logical interaction, captured twice, is byte-identical
//       (no cadence-driven nondeterminism).

/// (a) Open then dismiss-by-Escape must return to the EXACT bare desktop bytes.
/// A close that leaves a stale menu/scrim, or that lands on a different present
/// phase, would diverge from the bare desktop.
#[test]
fn rebuild_menu_open_then_close_returns_to_bare_desktop() {
    let (rx, ry) = (400.0_f32, 300.0_f32);
    // The interaction's pointer ends at the right-click point, so compare against
    // a baseline whose cursor is also there (cancels the software cursor; any
    // remaining diff is real menu/scrim residue).
    let base = cursor_neutral_baseline(rx, ry);
    let closed = capture_desktop_scripted_sync(&scenario_options(THEME), |handle| {
        ScriptedScenario::new(handle)
            .right_click(rx, ry)
            .hotkey(KeyCode::Escape, Modifiers::new())
            .into_events()
    })
    .expect("open-then-escape capture");

    assert!(
        frames_byte_identical(&base, &closed),
        "open→close did NOT return to the bare desktop: {} px differ. A dismissed menu must leave \
         ZERO residue (no stale panel, no leftover full-screen scrim) and land on the same steady \
         state as a never-opened desktop. Residue here is a close-flicker source. \
         Fix crate: liquide-shell (menu dismiss clears scrim/scene + damage).",
        differing_pixels(&base, &closed)
    );
}

/// (b) Repeated open/close (3 cycles) must converge to the same bare-desktop
/// steady state as a single open/close — interaction repetition must not
/// accumulate residue or drift the present phase.
#[test]
fn rebuild_repeated_open_close_converges_to_bare_desktop() {
    let (rx, ry) = (400.0_f32, 300.0_f32);
    // Every cycle's pointer ends at the right-click point, so the final cursor is
    // there — compare against the cursor-matched baseline.
    let base = cursor_neutral_baseline(rx, ry);

    let after_repeats = capture_desktop_scripted_sync(&scenario_options(THEME), |handle| {
        let mut s = ScriptedScenario::new(handle);
        for _ in 0..3 {
            s = s
                .right_click(rx, ry)
                .hotkey(KeyCode::Escape, Modifiers::new());
        }
        s.into_events()
    })
    .expect("repeated open/close capture");

    assert!(
        frames_byte_identical(&base, &after_repeats),
        "after 3 open/close cycles the desktop did NOT converge to the bare state: {} px differ. \
         Repeated interaction must not accumulate stale paint or drift the redraw — divergence is \
         a repeat-interaction flicker source.",
        differing_pixels(&base, &after_repeats)
    );
}

/// (c) Hover-on then hover-off (move pointer over the dock, then back to empty
/// desktop) must end at a steady state with NO hover residue. We compare two
/// identical hover sequences for byte-identity (the strict, seam-honest signal):
/// a cadence/rebuild flicker on hover would make the end state nondeterministic.
#[test]
fn rebuild_hover_on_off_is_deterministic() {
    // Dock is bottom-centred; first icon ~80px left of centre, ~28px above bottom.
    let opts = scenario_options(THEME);
    let (w, h) = (opts.width as f32, opts.height as f32);
    let dock_x = w / 2.0 - 80.0;
    let dock_y = h - 28.0;
    let empty_x = w * 0.8;
    let empty_y = h * 0.35;

    let seq = |handle| {
        ScriptedScenario::new(handle)
            .pointer_move(dock_x, dock_y) // hover on
            .pointer_move(empty_x, empty_y) // hover off
            .into_events()
    };
    let a = capture_desktop_scripted_sync(&opts, seq).expect("hover seq A");
    let b = capture_desktop_scripted_sync(&opts, seq).expect("hover seq B");

    assert!(
        frames_byte_identical(&a, &b),
        "hover on→off is NOT deterministic across two identical runs: {} px differ — a hover that \
         drives an oscillating redraw/rebuild lands on a different frame each time (hover flicker).",
        differing_pixels(&a, &b)
    );
}

/// (d) A left-click on empty desktop (no action) must be a no-op steady state:
/// byte-identical to the bare desktop. A click that triggers a spurious
/// rebuild/present flash would leave the click frame differing from the bare
/// desktop.
#[test]
fn rebuild_click_on_empty_desktop_is_noop() {
    // Click on empty desktop, away from any chrome.
    let opts = scenario_options(THEME);
    let (cx, cy) = (opts.width as f32 * 0.5, opts.height as f32 * 0.45);
    // Cursor ends at the click point; compare against the cursor-matched baseline
    // so only genuine click residue (not the moved cursor) can fail this.
    let base = cursor_neutral_baseline(cx, cy);
    let clicked = capture_desktop_scripted_sync(&opts, |handle| {
        ScriptedScenario::new(handle).left_click(cx, cy).into_events()
    })
    .expect("empty-click capture");

    assert!(
        frames_byte_identical(&base, &clicked),
        "a left-click on empty desktop changed {} px vs the cursor-matched bare desktop — an \
         interaction with no visible effect should be a no-op steady state; a difference is a \
         click-driven rebuild/present flash.",
        differing_pixels(&base, &clicked)
    );
}

/// (d') A drag on empty desktop, RELEASED, must settle back to the bare desktop.
/// A drag that leaves a selection-rect / skeleton residue, or that lands on a
/// mid-rebuild present, would diverge. (The drag is on empty desktop so no window
/// is moved; the end state should equal bare.)
#[test]
fn rebuild_drag_on_empty_desktop_settles_to_bare() {
    let opts = scenario_options(THEME);
    let (w, h) = (opts.width as f32, opts.height as f32);
    let (end_x, end_y) = (w * 0.6, h * 0.55);
    // The drag ends (cursor) at (end_x, end_y); compare against the cursor-matched
    // baseline so only real post-release residue (marquee/skeleton) can fail this.
    let base = cursor_neutral_baseline(end_x, end_y);
    let settled = capture_desktop_scripted_sync(&opts, |handle| {
        ScriptedScenario::new(handle)
            .drag(MouseButton::Left, w * 0.4, h * 0.4, end_x, end_y, 6)
            .into_events()
    })
    .expect("drag capture");

    // A drag on empty desktop may legitimately paint a transient selection
    // marquee DURING the drag, but on RELEASE (the last event) the captured
    // steady state must return to bare. If it does not, residue lingers.
    assert!(
        frames_byte_identical(&base, &settled),
        "after a drag-and-release on empty desktop the screen did not settle to the cursor-matched \
         bare desktop: {} px differ — leftover marquee/skeleton residue after release is a \
         drag-flicker source.",
        differing_pixels(&base, &settled)
    );
}

// ===========================================================================
// CROSS-CUT — menu OPEN steady state matches itself (no open-time flash)
// ===========================================================================
//
// Combines sources 1/3/4 into one strict signal for the headline interaction
// (right-click open): the opened-menu frame must be byte-identical across runs
// AND every menu row's text complete. This is the single assertion most directly
// tied to the user's "right-click menus flicker" report.
#[test]
fn menu_open_steady_state_is_complete_and_stable() {
    let (rx, ry) = (350.0_f32, 280.0_f32);
    let (ox, oy) = menu_origin(rx, ry);

    let (a, ()) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |handle| ScriptedScenario::new(handle).right_click(rx, ry).into_events(),
        |_shell| (),
    )
    .expect("menu open A");
    let (b, ()) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |handle| ScriptedScenario::new(handle).right_click(rx, ry).into_events(),
        |_shell| (),
    )
    .expect("menu open B");

    // Stable across runs (sources 3/4).
    assert!(
        frames_byte_identical(&a, &b),
        "open-menu steady state is unstable across runs: {} px differ (node-ID / blur-cache / \
         present-cadence churn).",
        differing_pixels(&a, &b)
    );
    // Text complete on the (only) menu frame (source 1, capture-path invariant).
    let inked = menu_inked_rows(&a, ox, oy);
    assert_eq!(
        inked, CONTEXT_MENU_ITEMS,
        "open-menu frame has only {inked}/{CONTEXT_MENU_ITEMS} rows of complete label text \
         (glyph pop-in)."
    );
}

// ===========================================================================
// TEETH — prove the byte-identity comparator can see a real interaction change.
// ===========================================================================
//
// If the strict `frames_byte_identical` checks above were vacuous (e.g. every
// capture identical regardless of state), the suite would be fake-green. Induce a
// REAL change (a menu open vs the bare desktop) and assert the comparator sees a
// large diff.
#[test]
fn interaction_comparator_has_teeth() {
    let base = base_desktop();
    let menu = capture_right_click(400.0, 300.0);
    let diff = differing_pixels(&base, &menu);
    assert!(
        diff > 2_000,
        "induced change (context menu opened) produced only {diff} differing px — the byte-identity \
         comparator cannot see a real interaction change, so the stability assertions would be \
         vacuously green."
    );
    // And the bare desktop must be byte-stable vs itself (the baseline of every
    // "returns to bare" assertion).
    let base2 = base_desktop();
    assert!(
        frames_byte_identical(&base, &base2),
        "the bare desktop is not byte-stable vs itself ({} px differ) — every 'returns to bare' \
         assertion rests on this; if the baseline flickers, those tests are unsound.",
        differing_pixels(&base, &base2)
    );
}
