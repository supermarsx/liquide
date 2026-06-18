//! ADVERSARIAL window-decoration containment screenshot test (t123-button-overflow).
//!
//! BUG UNDER TEST: the window handle-bar capability buttons
//! (close / maximize / minimize / pin) OVERFLOW the window titlebar ("handle")
//! itself — their painted pixels land OUTSIDE the visible titlebar rectangle
//! (past its right edge, onto the wallpaper beyond the window).
//!
//! ROOT CAUSE: the `window-titlebar` box is `box-sizing: content-box` (the
//! engine default), so `width: 100%` resolves the *content* width to 100% of the
//! window frame (800px) and then ADDS `padding-left + padding-right` (12 + 8),
//! making the titlebar box 820px wide — 20px wider than the 800px window frame.
//! The right-aligned `titlebar-buttons` cluster is therefore pushed 20px past the
//! window's right edge, so the close button is laid out at x≈1038..1052 while the
//! window/visible-titlebar right edge is 1040 — the buttons paint ~12px PAST the
//! visible titlebar onto the wallpaper.
//!
//! WHAT "the titlebar" MEANS HERE (the visible handle): the decoration renderer
//! paints the titlebar background across the WINDOW frame width (the window's
//! `bounds`), at the laid-out titlebar HEIGHT — not the overgrown 820px layout
//! box. So the visible handle is `(window.x, window.y, window.width,
//! titlebar.height)`. A correctly-laid-out cluster must sit inside THAT.
//!
//! WHAT THIS TEST DOES (exactly what the bug-report demanded):
//!   1. RENDERS a decorated window to a real framebuffer via the SAME capture
//!      path that produces `window_decorations.png`
//!      (`capture_desktop_scripted_readback` over `open_app_window`).
//!   2. Resolves the titlebar handle rectangle AND every button rectangle from
//!      the LAID-OUT CSS boxes (`hit_test_engine().bounds_for_node(...)`) + the
//!      window frame box, NOT from constants — a theme/CSS change that moves the
//!      boxes moves the assertion.
//!   3. PROGRAMMATICALLY asserts that every titlebar button's PAINTED pixels are
//!      fully CONTAINED within the titlebar handle's bounds (no button fill/glyph
//!      pixel lands past the handle's right / top / bottom edge).
//!
//! The decoration buttons are painted by the window `Decoration` scene node whose
//! per-button rects are anchored to these exact laid-out CSS boxes
//! (`scene.rs::decoration_layout_from_css` -> `DecorationButtonRects`), so a
//! button box that overflows the handle paints glyph/fill pixels outside it.
//!
//! This test is RED against the buggy CSS (close button laid out at x≈1038..1052
//! while the visible titlebar right edge is the window's 1040, so painted button
//! pixels spill ~12px past the window edge onto the wallpaper) and GREEN once the
//! titlebar is constrained (`box-sizing: border-box`) so the cluster sits inside.
//!
//! Run: `cargo test -p liquide-visual-test --test window_button_containment --offline`

use liquide_platform::NativeWindowHandle;
use liquide_visual_test::capture::Frame;
use liquide_visual_test::scenarios::themed_desktop_capture;
use liquide_visual_test::{capture_desktop_scripted_readback, scenario_options};

const THEME: &str = "liquid-glass";

/// A laid-out box resolved from the live CSS layout tree (absolute screen px).
#[derive(Clone, Copy, Debug)]
struct Box {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl Box {
    fn right(&self) -> f32 {
        self.x + self.w
    }
    fn bottom(&self) -> f32 {
        self.y + self.h
    }
    /// True if point (px, py) sample-center lies within this box.
    fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.right() && py >= self.y && py < self.bottom()
    }
}

/// The decoration geometry read off the live shell during the capture, returned
/// alongside the rendered frame so the pixel assertions can use the REAL boxes.
struct DecoGeom {
    titlebar: Box,
    buttons: Vec<(&'static str, Box)>,
}

/// Render a decorated window and read its laid-out titlebar + button boxes from
/// the CSS layout (not constants), returning both the framebuffer and geometry.
fn capture_window_with_geometry() -> (Frame, DecoGeom) {
    let opts = scenario_options(THEME);
    capture_desktop_scripted_readback(
        &opts,
        |_h: NativeWindowHandle| Vec::new(),
        |shell| {
            // Open one decorated app window, then build the scene so the CSS
            // pipeline lays out the window-frame subtree and populates the
            // hit-test engine (the laid-out boxes we read below).
            let _ = shell.open_app_window("com.liquide.files");
            let _ = shell.build_scene();

            let wid = shell
                .visible_windows()
                .first()
                .expect("an app window must be open")
                .id;

            let doc = shell.document();
            let ht = shell
                .hit_test_engine()
                .expect("hit-test engine must exist after build_scene");

            let resolve = |suffix: &str| -> Box {
                let id = format!("window-deco-{}-{suffix}", wid.0);
                let node = doc
                    .get_element_by_id(&id)
                    .unwrap_or_else(|| panic!("decoration element #{id} must exist"));
                let r = ht
                    .bounds_for_node(node)
                    .unwrap_or_else(|| panic!("#{id} must have a laid-out box"));
                Box {
                    x: r.x,
                    y: r.y,
                    w: r.width,
                    h: r.height,
                }
            };

            // Window frame box (the decoration paints the titlebar background
            // across THIS width — the visible handle), and the laid-out titlebar
            // box (the source of the handle's HEIGHT). The visible titlebar
            // handle is the frame width at the titlebar height, anchored at the
            // titlebar's laid-out origin.
            let frame_id = format!("window-deco-{}", wid.0);
            let frame_node = doc
                .get_element_by_id(&frame_id)
                .expect("window-frame element must exist");
            let frame = ht
                .bounds_for_node(frame_node)
                .expect("window-frame must have a laid-out box");
            let tb_box = resolve("titlebar");
            let titlebar = Box {
                x: tb_box.x,
                y: tb_box.y,
                w: frame.width,
                h: tb_box.h,
            };

            let buttons = vec![
                ("close", resolve("close")),
                ("max", resolve("max")),
                ("min", resolve("min")),
                ("pin", resolve("pin")),
            ];
            DecoGeom { titlebar, buttons }
        },
    )
    .map(|(frame, geom)| (frame, geom))
    .expect("capture window decorations")
}

/// A pixel counts as "painted by the decoration" when the windowed render
/// differs noticeably from the windowless baseline at that coordinate — i.e. the
/// window/its buttons changed it. This isolates real button fill/glyph paint from
/// the unchanged wallpaper, so wallpaper pixels beyond the window edge are NOT
/// miscounted as button content.
fn changed_by_window(framed: [u8; 4], baseline: [u8; 4], tol: u8) -> bool {
    framed
        .iter()
        .zip(baseline.iter())
        .any(|(&a, &b)| a.abs_diff(b) > tol)
}

#[test]
fn titlebar_buttons_are_contained_within_the_titlebar() {
    // Windowless baseline so we can isolate pixels the window/buttons painted
    // from the unchanged wallpaper (the overflow region beyond the window edge).
    let baseline = themed_desktop_capture(THEME).expect("baseline desktop capture");
    let (frame, geom) = capture_window_with_geometry();
    assert_eq!(
        (frame.width, frame.height),
        (baseline.width, baseline.height),
        "framed and baseline captures must share dimensions"
    );
    let tb = geom.titlebar;

    // Sanity: the boxes must be real (non-degenerate) before we trust them.
    assert!(
        tb.w > 1.0 && tb.h > 1.0,
        "titlebar box must be non-degenerate, got {tb:?}"
    );

    // Tolerance generous enough to ignore JPEG-free wallpaper gradient noise but
    // tight enough to catch a painted button fill/glyph against the baseline.
    const TOL: u8 = 40;

    // For every button, scan its laid-out box plus a margin and count pixels that
    // the window painted (differ from baseline) AND fall OUTSIDE the titlebar
    // handle rectangle. A correctly contained button paints zero such pixels.
    let mut total_overflow = 0usize;
    let mut report = String::new();
    for (name, b) in &geom.buttons {
        // Scan a region covering the button box (+2px margin to catch a glyph
        // that bleeds a hair past the fill), clamped to the frame.
        let scan_x0 = (b.x - 2.0).floor().max(0.0) as u32;
        let scan_x1 = ((b.right() + 2.0).ceil() as u32).min(frame.width);
        let scan_y0 = (b.y - 2.0).floor().max(0.0) as u32;
        let scan_y1 = ((b.bottom() + 2.0).ceil() as u32).min(frame.height);

        let mut overflow = 0usize;
        let mut max_x_overflow = 0f32;
        let mut max_below = 0f32;
        let mut max_above = 0f32;
        for y in scan_y0..scan_y1 {
            for x in scan_x0..scan_x1 {
                let Some(px) = frame.pixel(x, y) else { continue };
                let Some(base) = baseline.pixel(x, y) else {
                    continue;
                };
                if !changed_by_window(px, base, TOL) {
                    continue;
                }
                // Sample center of the pixel for the containment test.
                let cx = x as f32 + 0.5;
                let cy = y as f32 + 0.5;
                if !tb.contains(cx, cy) {
                    overflow += 1;
                    if cx >= tb.right() {
                        max_x_overflow = max_x_overflow.max(cx - tb.right());
                    }
                    if cy >= tb.bottom() {
                        max_below = max_below.max(cy - tb.bottom());
                    }
                    if cy < tb.y {
                        max_above = max_above.max(tb.y - cy);
                    }
                }
            }
        }
        if overflow > 0 {
            report.push_str(&format!(
                "  {name}: {overflow} painted px outside titlebar \
                 (box={b:?}; right overhang {max_x_overflow:.1}px, \
                 below {max_below:.1}px, above {max_above:.1}px)\n"
            ));
        }
        total_overflow += overflow;
    }

    assert_eq!(
        total_overflow, 0,
        "TITLEBAR BUTTON OVERFLOW: {total_overflow} painted button pixels land \
         OUTSIDE the laid-out titlebar rectangle {tb:?} (right edge {:.1}). \
         The capability buttons must be fully contained within the titlebar.\n{report}",
        tb.right()
    );
}

/// Geometric companion: each button's laid-out CSS box must itself be fully
/// inside the titlebar box. This catches the layout overflow directly (before
/// rasterization), proving the fix constrains the cluster — not just that the
/// renderer happened to clip it.
#[test]
fn titlebar_button_boxes_fit_inside_the_titlebar() {
    let (_frame, geom) = capture_window_with_geometry();
    let tb = geom.titlebar;

    let mut report = String::new();
    let mut bad = 0usize;
    for (name, b) in &geom.buttons {
        // Allow a sub-pixel epsilon for rounding, but a 1px+ overhang is a bug.
        let eps = 1.0;
        let right_over = b.right() - tb.right();
        let bottom_over = b.bottom() - tb.bottom();
        let top_over = tb.y - b.y;
        let left_over = tb.x - b.x;
        if right_over > eps || bottom_over > eps || top_over > eps || left_over > eps {
            bad += 1;
            report.push_str(&format!(
                "  {name} box {b:?} escapes titlebar {tb:?}: right+{right_over:.1} \
                 bottom+{bottom_over:.1} top+{top_over:.1} left+{left_over:.1}\n"
            ));
        }
    }

    assert_eq!(
        bad, 0,
        "BUTTON BOX OVERFLOW: {bad} button box(es) extend past the titlebar \
         rectangle.\n{report}"
    );
}
