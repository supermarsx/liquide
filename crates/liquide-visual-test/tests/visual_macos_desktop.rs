//! Desktop-regression coverage for the DEFAULT theme: **macos-dark** (t185).
//!
//! WHY THIS FILE EXISTS — the suite was FALSELY GREEN. `visual_regression.rs`
//! covers the desktop only under `night` and `liquid-glass`, so the DEFAULT
//! theme (macos-dark) had ZERO full-desktop coverage. When the macOS retheme
//! shipped two visible regressions in the default render, every test still
//! passed:
//!
//!   1. DOCK RIGHT-PINNED — `macos_dark.css`/`night.css` centred the dock with
//!      `left:50%; transform:translateX(-50%)`, but the style engine's transform
//!      parser only accepts px (`value_resolve::parse_px`), so the `-50%`
//!      translate was DROPPED and the dock pinned its left edge to screen-centre
//!      (shifted hard right). Fixed by switching to the liquid-glass anchoring
//!      (`left:0; right:0; justify-content:center` → full-width bar, centred
//!      icons).
//!
//!   2. TOP-LEFT OPAQUE FILL / LEFT-RIGHT SEAM — the desktop backdrop's
//!      `linear-gradient(180deg, …)` fallback round-tripped through
//!      `liquide-paint::emit_gradient` (angle = `atan2(dy,dx)`, math convention)
//!      and back through `scene_bridge` (which re-expands the angle with the CSS
//!      convention `start=(0.5-0.5·sinθ, 0.5+0.5·cosθ)`). The two conventions
//!      are rotated 90° apart, so a vertical `to bottom` gradient was painted
//!      LEFT→RIGHT — a hard vertical seam at x=width/2 (the most-visible part
//!      reading as a dark box under the menu bar). liquid-glass escaped only
//!      because it paints a `background-image` and never round-trips its fallback
//!      gradient. Fixed by emitting the CSS-convention angle (`atan2(dx,-dy)`).
//!
//! These two structural tests have TEETH that catch THIS CLASS of bug regardless
//! of the golden: `dock_is_horizontally_centered` fails when the dock content's
//! horizontal centroid drifts off-centre (the right-pinned regression), and
//! `no_stray_opaque_fill_below_menu_bar` fails when a vertical seam splits the
//! desktop body (the gradient regression). Both were verified RED on the broken
//! render and GREEN after the fix (see .orchestration log).
//!
//! Bless the golden with:
//!   BLESS=1 cargo test -p liquide-visual-test --test visual_macos_desktop
//! then re-run WITHOUT bless to confirm determinism.

use liquide_visual_test::Frame;
use liquide_visual_test::golden::assert_golden;
use liquide_visual_test::scenarios::{
    DOCK_BAND_HEIGHT, STATUS_BAR_HEIGHT, region_dock_band, themed_desktop_capture,
};

/// The default theme name. macos-dark is the shell default (session/desktop:
/// `LIQUIDE_THEME` falls back to "macos-dark").
const DEFAULT_THEME: &str = "macos-dark";

/// Per-column count of BRIGHT pixels (mean RGB above `min_lum`) in a frame.
///
/// The dock's icons + chrome are markedly brighter than the near-black graphite
/// wallpaper, so a luminance gate cleanly isolates the dock from the backdrop
/// (a raw non-background count would be dominated by the wallpaper gradient and
/// mask the dock's horizontal position). Calibrated on the real renders: the
/// dock icons sit at mean-RGB ≈ 90..230, the wallpaper body at ≈ 30..45.
fn bright_column_content(frame: &Frame, min_lum: u32) -> Vec<u32> {
    let mut cols = vec![0u32; frame.width as usize];
    for y in 0..frame.height {
        for x in 0..frame.width {
            if let Some(px) = frame.pixel(x, y) {
                let lum = (px[0] as u32 + px[1] as u32 + px[2] as u32) / 3;
                if lum > min_lum {
                    cols[x as usize] += 1;
                }
            }
        }
    }
    cols
}

/// Scenario 1 — the DEFAULT-theme desktop is a real (non-uniform) render and is
/// pinned with a full-desktop golden. The golden is blessed ONLY from the FIXED
/// render (dock centred + no seam); a future regression to either symptom drifts
/// the golden AND trips one of the two structural teeth below.
#[test]
fn macos_dark_desktop_renders() {
    let frame = themed_desktop_capture(DEFAULT_THEME).expect("macos-dark desktop capture");
    assert!(
        !frame.is_uniform(),
        "macos-dark desktop frame is uniform (dead pipeline / theme not loading)"
    );
    assert_golden("macos_dark_desktop", &frame);
}

/// Scenario 2 (TOOTH) — the dock is horizontally CENTERED, not right-pinned.
///
/// Crops the bottom dock band and computes the horizontal CENTROID of the
/// painted dock content (icons + bar weighted by per-column non-bg pixels). On a
/// correctly-centred dock the centroid sits at ~width/2. The broken render
/// pinned the dock's left edge at screen-centre, dragging the centroid well into
/// the RIGHT half — this assertion catches exactly that.
#[test]
fn dock_is_horizontally_centered() {
    let frame = themed_desktop_capture(DEFAULT_THEME).expect("macos-dark desktop capture");
    let (rx, ry, rw, rh) = region_dock_band(frame.width, frame.height);
    let band = frame.crop(rx, ry, rw, rh);

    // Gate on bright (dock-icon) pixels so the centroid tracks the DOCK, not the
    // wallpaper. The dock chrome/icons clear lum>80; the graphite body does not.
    let cols = bright_column_content(&band, 80);
    let total: u64 = cols.iter().map(|&c| c as u64).sum();
    assert!(
        total > 200,
        "dock band has almost no bright pixels ({total}) — the dock did not paint \
         (theme/cascade broke)."
    );

    // Weighted horizontal centroid of the dock content, in band pixels.
    let weighted: u64 = cols
        .iter()
        .enumerate()
        .map(|(x, &c)| x as u64 * c as u64)
        .sum();
    let centroid = weighted as f64 / total as f64;
    let center = band.width as f64 / 2.0;

    // Allow a generous symmetric tolerance (12.5% of width). The centred fix
    // lands the centroid within a couple of pixels of centre; the right-pinned
    // regression parked it hundreds of pixels into the right half (well outside
    // this band), so the tooth is firmly RED-before / GREEN-after.
    let tolerance = band.width as f64 * 0.125;
    assert!(
        (centroid - center).abs() <= tolerance,
        "DOCK IS NOT HORIZONTALLY CENTERED. Content centroid x={centroid:.1} but \
         the band centre is {center:.1} (tolerance ±{tolerance:.1}). A centroid \
         dragged into the right half is the `left:50%; transform:translateX(-50%)` \
         regression — the engine drops the percentage translate, pinning the \
         dock's left edge at screen-centre. Anchor the dock like liquid_glass.css \
         (`left:0; right:0; justify-content:center`)."
    );
}

/// Scenario 3 (TOOTH) — NO stray opaque fill / vertical seam in the desktop body
/// below the menu bar.
///
/// The broken render painted the desktop backdrop gradient LEFT→RIGHT (a 90°
/// rotation from the intended `to bottom`), producing a hard vertical seam at
/// x=width/2: the left half a flat top-stop colour, the right half the gradient
/// — the top-left portion reading as a stray dark box under the menu bar. A
/// correct vertical gradient is HORIZONTALLY UNIFORM at any given y. We sample
/// several rows in the chrome-free body band and assert the per-row left-half vs
/// right-half mean colours match closely (no seam). The broken render splits by
/// tens of levels; the fixed render matches within a couple of levels.
#[test]
fn no_stray_opaque_fill_below_menu_bar() {
    let frame = themed_desktop_capture(DEFAULT_THEME).expect("macos-dark desktop capture");

    // Body band: strictly BELOW the menu bar and ABOVE the dock band, so the
    // only thing painted is the desktop backdrop (no chrome to confound the
    // left/right comparison).
    let top = STATUS_BAR_HEIGHT + 8;
    let bottom = frame.height.saturating_sub(DOCK_BAND_HEIGHT + 8);
    assert!(
        bottom > top + 4,
        "body band degenerate ({top}..{bottom}) — surface too small"
    );

    let half = frame.width / 2;
    // Inset from the exact centre column and the screen edges so a 1px AA seam
    // or edge vignette doesn't dominate; compare solid interior bands.
    let inset = (frame.width / 8).max(8);
    let left_lo = inset;
    let left_hi = half.saturating_sub(inset);
    let right_lo = half + inset;
    let right_hi = frame.width.saturating_sub(inset);

    let mean_rgb = |x0: u32, x1: u32, y: u32| -> (f64, f64, f64) {
        let mut r = 0u64;
        let mut g = 0u64;
        let mut b = 0u64;
        let mut n = 0u64;
        for x in x0..x1 {
            if let Some(px) = frame.pixel(x, y) {
                r += px[0] as u64;
                g += px[1] as u64;
                b += px[2] as u64;
                n += 1;
            }
        }
        let n = n.max(1) as f64;
        (r as f64 / n, g as f64 / n, b as f64 / n)
    };

    let mut max_seam = 0.0f64;
    let mut worst_y = top;
    let mut y = top;
    while y < bottom {
        let l = mean_rgb(left_lo, left_hi, y);
        let rr = mean_rgb(right_lo, right_hi, y);
        // L1 distance between the left-half and right-half mean colour at this y.
        let seam = (l.0 - rr.0).abs() + (l.1 - rr.1).abs() + (l.2 - rr.2).abs();
        if seam > max_seam {
            max_seam = seam;
            worst_y = y;
        }
        y += 16;
    }

    // A correct vertical gradient is x-uniform within a row, so left-mean ≈
    // right-mean (delta near 0). The broken left/right gradient produced a
    // double-digit-per-channel split → tens in L1. Threshold 9 (≈3/channel)
    // sits well above clean-render noise and far below the regression.
    assert!(
        max_seam < 9.0,
        "STRAY OPAQUE FILL / VERTICAL SEAM in the desktop body. At y={worst_y} the \
         left-half mean colour differs from the right-half mean by L1={max_seam:.1} \
         — the desktop backdrop is split LEFT/RIGHT instead of a smooth vertical \
         gradient. This is the `linear-gradient(180deg)` painted horizontally: the \
         emit_gradient↔scene_bridge angle conventions disagree by 90°. Emit the \
         CSS-convention angle (atan2(dx,-dy)) in liquide-paint::emit_gradient."
    );
}
