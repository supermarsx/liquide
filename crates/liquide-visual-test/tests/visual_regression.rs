//! Visual-regression scenario tests (t56-f7).
//!
//! Four end-to-end cases, one per user-reported symptom, all driving the REAL
//! headless `DesktopCompositor` (not the old zero-filled app-harness) through
//! f6's capture/golden API and f7's deterministic test-assets root.
//!
//! These tests have TEETH: `themed_desktop_renders` is a *differential* guard
//! (night vs liquid-glass MUST differ — catches H1's silent theme fallback) and
//! `text_renders_glyphs` is a *content* heuristic (real glyph pixels must be
//! present — catches H2's notdef/bitmap fallback). The differential teeth were
//! proven by temporarily forcing both captures to the same theme and confirming
//! the test fails (see .orchestration/logs/t56-f7.md).
//!
//! Bless goldens with: `BLESS=1 cargo test -p liquide-visual-test --test visual_regression`
//! (or `LIQUIDE_UPDATE_GOLDEN=1`). Run without it to assert.

use liquide_visual_test::diff::{DiffOptions, diff_frames};
use liquide_visual_test::golden::assert_golden;
use liquide_visual_test::scenarios::{
    STATUS_BAR_HEIGHT, context_menu_capture, status_bar_capture, text_capture,
    themed_desktop_capture,
};

/// The desktop background under the night theme is near-black; use a black
/// reference for the non-background content heuristics with generous tolerance.
const BG_REFERENCE: [u8; 4] = [0, 0, 0, 255];
const BG_TOLERANCE: u8 = 24;

/// Scenario 1 — themed desktop renders (catches **H1**: CSS theme not loading).
///
/// Captures the desktop under the embedded-ish `night` theme AND the packaged
/// `liquid-glass` theme and asserts the two frames DIFFER. Before f1, the
/// hyphen/underscore filename mismatch made `liquid-glass` silently fall back to
/// the night styling, so the two captures collapsed to identical pixels — this
/// differential is exactly what catches that regression. The liquid-glass result
/// is also pinned with a golden.
#[test]
fn themed_desktop_renders() {
    let night = themed_desktop_capture("night").expect("night capture");
    let glass = themed_desktop_capture("liquid-glass").expect("liquid-glass capture");

    assert_eq!(
        (night.width, night.height),
        (glass.width, glass.height),
        "both captures must share the canonical surface size"
    );

    // Neither frame may be a dead/flat pipeline.
    assert!(
        !night.is_uniform(),
        "night desktop frame is uniform (dead pipeline)"
    );
    assert!(
        !glass.is_uniform(),
        "liquid-glass desktop frame is uniform (dead pipeline)"
    );

    // THE DIFFERENTIAL TOOTH: the two themed captures must NOT be identical.
    // If they are, the packaged theme silently failed to load (H1 regressed).
    let result = diff_frames(&night, &glass, DiffOptions::default());
    assert!(
        !result.matched,
        "night and liquid-glass captures are IDENTICAL ({} differing pixels, \
         max delta {}). This means the liquid-glass theme silently fell back to \
         night styling — H1 (theme file not loading) has regressed. Check \
         DesktopCompositor::resolve_theme_file / load_external_css.",
        result.differing_pixels, result.max_channel_delta
    );

    // Pin the liquid-glass result.
    assert_golden("themed_desktop_liquid_glass", &glass);
}

/// Scenario 2 — text renders with real glyphs (catches **H2**: missing fonts).
///
/// The desktop chrome always renders glyph-bearing text (status-bar clock, logo,
/// labels). With the deterministic test font registered, a real render produces
/// many non-background pixels in the text-bearing status-bar band; a blank or
/// 8x16-notdef render does not. We assert the content heuristic AND a golden.
#[test]
fn text_renders_glyphs() {
    let frame = text_capture("night").expect("text capture");
    assert!(!frame.is_uniform(), "text frame is uniform (dead pipeline)");

    // Crop to the status-bar band, which is guaranteed to contain text glyphs
    // (clock / logo / labels) regardless of desktop content.
    let band = frame.crop(0, 0, frame.width, STATUS_BAR_HEIGHT.min(frame.height));

    // THE CONTENT TOOTH: real glyphs paint many non-background pixels. A blank
    // or notdef-only render fails this. Threshold is deliberately well above
    // what a few stray AA pixels would produce, but far below a full glyph run.
    let non_bg = band.non_background_pixels(BG_REFERENCE, BG_TOLERANCE);
    assert!(
        non_bg > 400,
        "status-bar text region has only {non_bg} non-background pixels — \
         glyphs are not rendering (H2: fonts missing / notdef fallback). \
         Expected a populated proportional-font render.",
    );

    assert_golden("text_status_bar_glyphs", &band);
}

/// Scenario 3 — status bar renders and is styled (catches "janky bars").
///
/// Crops the top status-bar band and asserts it is present, non-uniform (styled,
/// not a flat fill), and pins it with a golden. A failure here means the bar's
/// property cascade (which depends on H1) or geometry/layout broke.
#[test]
fn status_bar_renders() {
    let band = status_bar_capture("night").expect("status bar capture");
    assert!(
        band.width > 0 && band.height > 0,
        "status bar band is empty"
    );

    // A styled bar (background + text + separators) is never a single flat
    // colour; a missing/unstyled bar would be uniform.
    assert!(
        !band.is_uniform(),
        "status-bar band is uniform — the bar is unstyled or not painted \
         (check the bar property cascade / theme load)."
    );

    // Structural sub-assert: the band carries real content (text + chrome).
    let content = band.non_background_pixels(BG_REFERENCE, BG_TOLERANCE);
    assert!(
        content > 200,
        "status-bar band has only {content} non-background pixels — looks empty/janky."
    );

    assert_golden("status_bar_band", &band);
}

/// Scenario 4 — context menu opens on right-click (the **f4 GATE**, now closed).
///
/// Injects a scripted right-click on the desktop window and captures the
/// post-click frame, then differences it against a no-click baseline in the
/// region where the menu opens (down-right of the cursor): an opened menu adds a
/// substantial block of new pixels there.
///
/// ROOT CAUSE (t56-f4): f7's earlier "NOT PAINTED" verdict was a *capture-path*
/// artifact, NOT a `scene_bridge` paint/z-order bug. The scene bridge emits the
/// `position:fixed; z-index:25` overlay correctly (menu background/border/items
/// land in the chrome z-band, on-screen, above windows — verified by dumping the
/// built scene tree). The fault was [`capture_desktop_scripted`]: it drove the
/// threaded `run()` loop with a trailing `Quit`, and the loop drains all queued
/// events in one batch — so `running` flipped to `false` and the loop exited
/// *before* the now-dirty post-click frame was presented. `last_presented_frame`
/// thus returned the pre-click desktop ⇒ exactly 0 changed pixels.
///
/// FIX: `context_menu_capture` now uses [`capture_desktop_scripted_sync`]
/// (`capture_once` applies the queued events during its synchronous prologue,
/// then reads back the desktop frame rendered *with the menu visible*). With
/// that, the menu region gains tens of thousands of changed pixels and this
/// guard passes. The `context_menu_open` golden is blessed so future regressions
/// (menu failing to paint OR the capture path regressing) are caught.
#[test]
fn context_menu_opens_on_right_click() {
    let theme = "night";

    // Baseline: the desktop with no click.
    let baseline = themed_desktop_capture(theme).expect("baseline desktop capture");

    // Right-click near the centre of the desktop (below the status bar).
    let click_x = (baseline.width / 2) as f32;
    let click_y = (baseline.height / 2) as f32;
    let clicked = context_menu_capture(theme, click_x, click_y).expect("context-menu capture");

    assert_eq!(
        (baseline.width, baseline.height),
        (clicked.width, clicked.height),
        "baseline and post-click frames must share dimensions"
    );

    // The context menu opens at/near the cursor. Examine a region anchored at
    // the click point (menus open down-right from the cursor by convention) and
    // count pixels that CHANGED versus the baseline there.
    let region_x = click_x as u32;
    let region_y = click_y as u32;
    let region_w = (clicked.width - region_x).min(260);
    let region_h = (clicked.height - region_y).min(360);

    let before = baseline.crop(region_x, region_y, region_w, region_h);
    let after = clicked.crop(region_x, region_y, region_w, region_h);
    let delta = diff_frames(&before, &after, DiffOptions::default());

    let menu_appeared = !delta.matched && delta.differing_pixels > 500;

    // THE GATE: a scripted right-click MUST open a context menu that paints in
    // the menu region. Regressions here mean either the overlay stopped painting
    // (scene_bridge / theme cascade) OR the scripted-capture path stopped
    // observing the post-click frame (capture_desktop_scripted_sync).
    assert!(
        menu_appeared,
        "CONTEXT MENU DID NOT PAINT. A scripted right-click at \
         ({click_x},{click_y}) produced only {} changed pixels in the menu \
         region (threshold 500). Expected the position:fixed z-index:25 \
         context-menu overlay to appear. Check (a) the overlay still reaches the \
         scene (liquide-shell scene_bridge.rs / theme `context-menu` rule) and \
         (b) the scripted capture still observes the post-event frame \
         (capture_desktop_scripted_sync / capture_once event drain).",
        delta.differing_pixels
    );

    // Menu painted: pin the post-click frame so future regressions are caught.
    assert_golden("context_menu_open", &clicked);
}
