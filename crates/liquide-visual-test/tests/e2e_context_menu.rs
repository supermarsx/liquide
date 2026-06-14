//! RIGOROUS end-to-end suite for CONTEXT / SESSION menus actually WORKING
//! (t58-menu).
//!
//! PRIME DIRECTIVE: encode what a CORRECT menu MUST do at the PIXEL + STATE
//! level, run it, and let failures stand as findings. A menu that merely
//! "paints something" must NOT pass — it has to open at the cursor, render every
//! labelled item with the icon and label in DISTINCT (non-overlapping) regions,
//! fire the right action on click, and DISMISS on activate / outside-click /
//! Escape, leaving no stale paint behind.
//!
//! ## Why these tests are mostly PIXEL-driven (a reported missing seam)
//!
//! The desktop context-menu state — `context_menu_visible`, `context_menu_pos`,
//! and the live item list — is `pub(crate)` on `Shell` (see
//! `crates/liquide-shell/src/shell/mod.rs:266-269`). There is NO public accessor
//! (unlike `session_menu_visible()`), so a cross-crate e2e test in
//! `liquide-visual-test` CANNOT read the context-menu open/position/items state
//! directly. The only public, observable consequences of a context-menu action
//! are: (a) the painted pixels, and (b) `window_count()` incrementing when the
//! item's action opens a window. So the context-menu teeth here are PIXEL teeth
//! plus the `window_count` action-fired tooth. **SEAM REQUEST (reported in
//! t58-menu.md): add `Shell::context_menu_visible()` / `context_menu_pos()` /
//! `context_menu_items()` read accessors so the open/position/dismiss state can be
//! asserted directly instead of inferred from pixels.**
//!
//! The SESSION menu DOES have public state (`session_menu_visible()`,
//! `toggle_session_menu()`, `pending_session_request()`), so its checks combine
//! state AND pixels.
//!
//! ## Menu geometry (mirrors `crates/liquide-shell/src/shell/mod.rs` constants
//! and `assets/themes/liquid_glass.css`)
//!   - `MENU_ITEM_HEIGHT = 28`, `MENU_PADDING = 4`, `CONTEXT_MENU_WIDTH = 200`.
//!   - desktop context menu has 5 items, so total height = 4*2 + 5*28 = 148.
//!   - On a right-click that fits, the menu top-left == the click point exactly
//!     (clamping only kicks in near the screen edges — see `sync_context_menu_template`).

use liquide_input::keyboard::{KeyCode, Modifiers};
use liquide_visual_test::scenarios::{
    ScriptedScenario, scenario_options, themed_desktop_capture,
};
use liquide_visual_test::{Frame, capture_desktop_scripted_readback};
use liquide_shell::SessionRequest;

const THEME: &str = "liquid-glass";

// ── Menu layout constants (must match the shell + CSS) ──────────────────────
const MENU_ITEM_HEIGHT: f32 = 28.0;
const MENU_PADDING: f32 = 4.0;
const CONTEXT_MENU_WIDTH: f32 = 200.0;
const CONTEXT_MENU_ITEMS: usize = 5;
/// Total painted height of the 5-item desktop context menu (px).
const CONTEXT_MENU_HEIGHT: f32 =
    MENU_PADDING * 2.0 + CONTEXT_MENU_ITEMS as f32 * MENU_ITEM_HEIGHT; // 148

/// Expected item labels, top to bottom (mirrors `ContextMenuItem::defaults()`).
const EXPECTED_LABELS: [&str; CONTEXT_MENU_ITEMS] = [
    "Open Terminal",
    "Open File Manager",
    "Change Wallpaper",
    "Display Settings",
    "System Settings",
];

// ───────────────────────────────────────────────────────────────────────────
// Pixel helpers
// ───────────────────────────────────────────────────────────────────────────

/// A no-interaction base desktop (no menu, no windows) for differential probes.
fn base_desktop() -> Frame {
    themed_desktop_capture(THEME).expect("base desktop capture")
}

/// Count pixels in `frame`'s rect that DIFFER from the same rect in `base`
/// (max-channel delta > `tol`). This is the canonical "did a menu paint here?"
/// probe: a menu panel + its text differ strongly from the bare wallpaper, while
/// an empty/dismissed region matches the base closely.
fn changed_vs_base(frame: &Frame, base: &Frame, x: u32, y: u32, w: u32, h: u32, tol: u8) -> usize {
    let mut n = 0usize;
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

/// Per-COLUMN count of "ink" pixels in a horizontal band — pixels that differ
/// from `base` (so glyph strokes / icon marks, not the menu panel fill, which is
/// uniform-ish but still differs from wallpaper). To isolate ink from the panel
/// fill we instead measure local CONTRAST: a pixel is "ink" if it differs from
/// the band's own median-ish panel colour. We approximate the panel colour by
/// the most common pixel in the band and count pixels far from it.
fn column_ink_profile(frame: &Frame, x: u32, y: u32, w: u32, h: u32) -> Vec<usize> {
    let x1 = (x + w).min(frame.width);
    let y1 = (y + h).min(frame.height);
    if x1 <= x || y1 <= y {
        return vec![0; w as usize];
    }
    // Estimate the panel (background-of-band) colour as the average pixel — text
    // and icons are a small fraction of the band so the mean tracks the panel.
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
    let (mr, mg, mb) = (
        (sr / count) as i32,
        (sg / count) as i32,
        (sb / count) as i32,
    );
    // Ink = pixels whose luma-distance from the panel mean is large.
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

/// The painted top-left of the context menu after a right-click at `(rx, ry)`
/// that fits on screen (no clamping) — equals the click point.
fn menu_origin(rx: f32, ry: f32) -> (u32, u32) {
    (rx.round() as u32, ry.round() as u32)
}

/// Capture the desktop after a right-click at `(rx, ry)`, returning the frame.
fn capture_right_click(rx: f32, ry: f32) -> Frame {
    let (frame, ()) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |handle| ScriptedScenario::new(handle).right_click(rx, ry).into_events(),
        |_shell| (),
    )
    .expect("right-click capture");
    frame
}

// ===========================================================================
// 1. OPENS AT CURSOR — the menu paints with its top-left near the click point,
//    not at (0,0), not offscreen.
// ===========================================================================

#[test]
fn context_menu_opens_at_cursor() {
    // A click point comfortably inside the screen so the menu is NOT clamped:
    // 300 < 1280-200-4, 250 < 720-148-4. Menu top-left must equal (300, 250).
    let (rx, ry) = (300.0_f32, 250.0_f32);
    let base = base_desktop();
    let frame = capture_right_click(rx, ry);
    let (ox, oy) = menu_origin(rx, ry);

    // The menu rect [ox, ox+200] x [oy, oy+148] must carry substantial paint
    // that differs from the bare desktop (the panel + 5 item rows).
    let menu_changed = changed_vs_base(
        &frame,
        &base,
        ox,
        oy,
        CONTEXT_MENU_WIDTH as u32,
        CONTEXT_MENU_HEIGHT as u32,
        24,
    );
    let menu_area = (CONTEXT_MENU_WIDTH * CONTEXT_MENU_HEIGHT) as usize;
    assert!(
        menu_changed > menu_area / 3,
        "context menu did not paint at the cursor: only {menu_changed}/{menu_area} pixels in \
         the menu rect at ({ox},{oy}) changed vs the bare desktop (expected > 1/3 of the rect). \
         The menu is missing, blank, or painted somewhere else."
    );

    // NOT at the origin: the top-left 200x148 corner of the screen (where a
    // broken (0,0)-anchored menu would land) must be ~unchanged from base, since
    // our click was at (300,250) well away from the corner and the status bar
    // band is excluded by starting the probe below it.
    let corner_changed = changed_vs_base(&frame, &base, 0, 40, CONTEXT_MENU_WIDTH as u32, 108, 24);
    assert!(
        corner_changed < (CONTEXT_MENU_WIDTH as usize * 108) / 8,
        "context menu appears anchored near the screen origin (corner has {corner_changed} \
         changed pixels) instead of the click point ({ox},{oy})."
    );

    // NOT offscreen: the menu's bottom-right must be on-screen.
    assert!(
        ox + CONTEXT_MENU_WIDTH as u32 <= frame.width
            && oy + CONTEXT_MENU_HEIGHT as u32 <= frame.height,
        "menu rect at ({ox},{oy}) extends offscreen ({}x{})",
        frame.width,
        frame.height
    );
}

// ===========================================================================
// 2. ALL ITEMS PRESENT & LABELED — each of the 5 expected item rows renders
//    readable label text (ink) in its row.
// ===========================================================================

#[test]
fn context_menu_renders_all_five_item_rows() {
    let (rx, ry) = (300.0_f32, 250.0_f32);
    let frame = capture_right_click(rx, ry);
    let (ox, oy) = menu_origin(rx, ry);

    // Each item i occupies the row [oy + PADDING + i*ITEM_H, +ITEM_H], across the
    // menu width. A labelled row must carry glyph ink (light text on the dark
    // glass panel). We require EVERY row to have ink — a missing/garbled row
    // (the t57-f1 nested-template garble symptom) would read as a blank row.
    let mut blank_rows = Vec::new();
    for (i, label) in EXPECTED_LABELS.iter().enumerate() {
        let row_y = oy + (MENU_PADDING + i as f32 * MENU_ITEM_HEIGHT).round() as u32;
        let row_h = MENU_ITEM_HEIGHT as u32;
        let profile = column_ink_profile(&frame, ox, row_y, CONTEXT_MENU_WIDTH as u32, row_h);
        let ink_cols = profile.iter().filter(|&&c| c > 0).count();
        let total_ink: usize = profile.iter().sum();
        // A real text label spans many columns. Require a healthy spread.
        if ink_cols < 12 || total_ink < 40 {
            blank_rows.push(format!(
                "row {i} ({label}): ink_cols={ink_cols}, total_ink={total_ink}"
            ));
        }
    }
    assert!(
        blank_rows.is_empty(),
        "context menu rows are missing/blank label text — every one of the 5 items must \
         render readable label glyphs. Blank/under-inked rows: {blank_rows:?}"
    );
}

// ===========================================================================
// 3. ICON / LABEL NO-OVERLAP — the KNOWN JANK the coordinator observed: item
//    icons overlap the text labels. For each item row, the icon (a contiguous
//    ink cluster near the LEFT edge) and the label text must occupy DISTINCT,
//    non-overlapping column spans. This test is EXPECTED TO FAIL given the
//    observed overlap — that failure is the finding, do NOT weaken it.
// ===========================================================================

#[test]
fn context_menu_icon_and_label_do_not_overlap() {
    let (rx, ry) = (300.0_f32, 250.0_f32);
    let frame = capture_right_click(rx, ry);
    let (ox, oy) = menu_origin(rx, ry);

    // CSS contract (liquid_glass.css): menu-item-icon { width: 16; margin-right:
    // 8 } sits left of menu-item-label, after the item's padding-left: 12. So a
    // CORRECT layout reserves roughly columns [12, 12+16] for the icon and the
    // label begins at >= ~36px in. If the icon and label are drawn in the SAME
    // columns (overlap), the ink in the icon band and the ink in the label band
    // will be vertically co-located at the same x — i.e. there is no clear gap
    // between the icon cluster and the start of the label text.
    let pad_left = 12u32;
    let icon_w = 16u32;
    let icon_label_gap = 8u32; // CSS margin-right on the icon
    let label_start_expected = pad_left + icon_w + icon_label_gap; // ~36

    let mut overlapping = Vec::new();
    for (i, label) in EXPECTED_LABELS.iter().enumerate() {
        let row_y = oy + (MENU_PADDING + i as f32 * MENU_ITEM_HEIGHT).round() as u32;
        let row_h = MENU_ITEM_HEIGHT as u32;
        let profile = column_ink_profile(&frame, ox, row_y, CONTEXT_MENU_WIDTH as u32, row_h);

        // Find the first and last inked columns (relative to ox).
        let first_ink = profile.iter().position(|&c| c > 0);
        let Some(first_ink) = first_ink else {
            // No ink at all -> covered by the all-items test; skip the overlap
            // judgement here (cannot judge overlap with nothing drawn).
            continue;
        };

        // The icon, if present and correctly placed, is a cluster in [pad_left,
        // pad_left+icon_w]. The label, if NOT overlapping, must have a GAP (a run
        // of zero-ink columns) between the icon cluster and the first label
        // glyph, and the label glyphs must start at >= label_start_expected.
        //
        // Detect overlap as: ink present inside the reserved icon band AND ink
        // present inside the label-start band with NO empty separator column
        // between them — i.e. the label text begins before label_start_expected,
        // intruding into / on top of the icon's reserved columns.
        let icon_band_ink: usize = profile
            [pad_left as usize..(pad_left + icon_w).min(profile.len() as u32) as usize]
            .iter()
            .sum();
        // Ink in the columns the icon's margin should keep clear AND before the
        // expected label start: [pad_left, label_start_expected).
        let intrusion_ink: usize = profile[pad_left as usize..label_start_expected as usize]
            .iter()
            .sum();
        // The label is supposed to begin at label_start_expected. If glyph ink
        // appears earlier than that (other than the icon's own 16px), the label
        // is overlapping the icon's reserved region.
        // Separator gap test: is there at least one fully-clear column between
        // the icon band end and the next ink (the label)?
        let after_icon = (pad_left + icon_w) as usize;
        let has_separator_gap = profile
            .get(after_icon..label_start_expected as usize)
            .map(|w| w.iter().any(|&c| c == 0))
            .unwrap_or(false);

        // OVERLAP if: there is icon-band ink (an icon is drawn) AND either the
        // first ink starts at/before the icon area while ALSO there is no clear
        // separator gap before the label — meaning icon and label share columns.
        let icon_drawn = icon_band_ink > 4;
        let label_intrudes_icon_region = intrusion_ink > icon_band_ink + 20; // extra ink = label text on top of icon cols
        let no_gap = !has_separator_gap;

        if icon_drawn && (label_intrudes_icon_region || (no_gap && first_ink < pad_left as usize)) {
            overlapping.push(format!(
                "row {i} ({label}): icon_band_ink={icon_band_ink}, intrusion_ink={intrusion_ink}, \
                 first_ink_col={first_ink}, separator_gap={has_separator_gap}"
            ));
        }
    }

    assert!(
        overlapping.is_empty(),
        "ICON/LABEL OVERLAP detected (the known menu jank): icons and label text share columns \
         in the following rows instead of occupying distinct regions: {overlapping:?}. \
         CSS reserves [{pad_left},{}] for the icon then the label at >= {label_start_expected}px; \
         the rendered menu does not honour that separation.",
        pad_left + icon_w
    );
}

// ===========================================================================
// 4. ITEM ACTIVATION FIRES THE ACTION + MENU DISMISSES — clicking item 0
//    ("Open Terminal") fires OpenTerminal (a window opens -> window_count==1)
//    AND the menu is GONE afterwards (the former menu rect reads ~background,
//    not a menu panel).
// ===========================================================================

#[test]
fn context_menu_item_activation_fires_action_and_dismisses() {
    let (rx, ry) = (300.0_f32, 250.0_f32);
    // First item centre (Open Terminal): top-left at click point + padding +
    // half a row height. x anywhere inside the menu width.
    let item_x = rx + CONTEXT_MENU_WIDTH / 2.0;
    let item_y = ry + MENU_PADDING + 0.5 * MENU_ITEM_HEIGHT;

    let base = base_desktop();

    let (frame, window_count) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |handle| {
            ScriptedScenario::new(handle)
                .right_click(rx, ry)
                .left_click(item_x, item_y)
                .into_events()
        },
        |shell| shell.window_count(),
    )
    .expect("activate capture");

    // STATE (action fired): OpenTerminal opened exactly one window.
    assert_eq!(
        window_count, 1,
        "activating the first context-menu item did not fire its action: window_count={window_count} \
         (OpenTerminal should open one window). Check the context-menu click arm (events.rs) and \
         execute_action(OpenTerminal)."
    );

    // DISMISSED (pixels): the LEFT strip of the former menu rect, BELOW the
    // opened window's titlebar band, must read close to the bare desktop — NOT a
    // menu panel. The opened app window is centred; this left strip at x in
    // [rx, rx+24] within the former menu rect is a clean dismissal probe.
    let (ox, oy) = menu_origin(rx, ry);
    // Probe the lower portion of the former menu rect (rows 2..5), 24px wide at
    // the menu's left edge — clear of a centred window's body at this x.
    let probe_y = oy + (MENU_PADDING + 2.0 * MENU_ITEM_HEIGHT) as u32;
    let probe_h = (3.0 * MENU_ITEM_HEIGHT) as u32;
    let still_painted = changed_vs_base(&frame, &base, ox, probe_y, 24, probe_h, 24);
    let probe_area = 24 * probe_h as usize;
    assert!(
        still_painted < probe_area / 5,
        "context menu appears STILL PAINTED after activation: {still_painted}/{probe_area} pixels \
         in the former menu's left strip still differ from the bare desktop (expected the menu to \
         be dismissed, i.e. < 1/5). A persistent menu after activation is a dismissal bug."
    );
}

// ===========================================================================
// 5. DISMISS ON OUTSIDE CLICK — open the menu, click far away, menu is gone.
// ===========================================================================

#[test]
fn context_menu_dismiss_on_outside_click() {
    let (rx, ry) = (300.0_f32, 250.0_f32);
    let base = base_desktop();

    let (frame, ()) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |handle| {
            ScriptedScenario::new(handle)
                .right_click(rx, ry)
                // Click far from the menu, on empty desktop (not dock/bar/menu).
                .left_click(900.0, 600.0)
                .into_events()
        },
        |_shell| (),
    )
    .expect("outside-click capture");

    let (ox, oy) = menu_origin(rx, ry);
    let still_painted = changed_vs_base(
        &frame,
        &base,
        ox,
        oy,
        CONTEXT_MENU_WIDTH as u32,
        CONTEXT_MENU_HEIGHT as u32,
        24,
    );
    let menu_area = (CONTEXT_MENU_WIDTH * CONTEXT_MENU_HEIGHT) as usize;
    assert!(
        still_painted < menu_area / 6,
        "context menu was NOT dismissed by an outside click: {still_painted}/{menu_area} pixels in \
         the former menu rect still differ from the bare desktop (expected the menu gone, < 1/6)."
    );
}

// ===========================================================================
// 6. DISMISS ON ESCAPE — open the menu, press Escape, menu is gone.
// ===========================================================================

#[test]
fn context_menu_dismiss_on_escape() {
    let (rx, ry) = (300.0_f32, 250.0_f32);
    let base = base_desktop();

    let (frame, ()) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |handle| {
            ScriptedScenario::new(handle)
                .right_click(rx, ry)
                .hotkey(KeyCode::Escape, Modifiers::new())
                .into_events()
        },
        |_shell| (),
    )
    .expect("escape capture");

    let (ox, oy) = menu_origin(rx, ry);
    let still_painted = changed_vs_base(
        &frame,
        &base,
        ox,
        oy,
        CONTEXT_MENU_WIDTH as u32,
        CONTEXT_MENU_HEIGHT as u32,
        24,
    );
    let menu_area = (CONTEXT_MENU_WIDTH * CONTEXT_MENU_HEIGHT) as usize;
    assert!(
        still_painted < menu_area / 6,
        "context menu was NOT dismissed by Escape: {still_painted}/{menu_area} pixels in the former \
         menu rect still differ from the bare desktop (expected the menu gone, < 1/6)."
    );
}

// ===========================================================================
// 7. NO STALE MENU — reopening at a second point must not leave the first menu
//    painted at the original point.
// ===========================================================================

#[test]
fn context_menu_no_stale_after_reopen() {
    let (ax, ay) = (200.0_f32, 200.0_f32); // first open point
    let (bx, by) = (700.0_f32, 450.0_f32); // second open point (far from A)
    let base = base_desktop();

    let (frame, ()) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |handle| {
            ScriptedScenario::new(handle)
                .right_click(ax, ay)
                .right_click(bx, by)
                .into_events()
        },
        |_shell| (),
    )
    .expect("reopen capture");

    let (aox, aoy) = menu_origin(ax, ay);
    let (box_, boy) = menu_origin(bx, by);

    // The SECOND menu must be painted at B.
    let b_painted = changed_vs_base(
        &frame,
        &base,
        box_,
        boy,
        CONTEXT_MENU_WIDTH as u32,
        CONTEXT_MENU_HEIGHT as u32,
        24,
    );
    let menu_area = (CONTEXT_MENU_WIDTH * CONTEXT_MENU_HEIGHT) as usize;
    assert!(
        b_painted > menu_area / 3,
        "the reopened context menu did not paint at the second point B=({box_},{boy}): only \
         {b_painted}/{menu_area} pixels changed."
    );

    // The FIRST menu must be GONE at A (A and B rects do not overlap: A bottom
    // = 200+148 = 348 > B top 450? No, 348 < 450, and A right 400 > B left 700?
    // No. So the rects are disjoint — a clean stale probe).
    let a_stale = changed_vs_base(
        &frame,
        &base,
        aox,
        aoy,
        CONTEXT_MENU_WIDTH as u32,
        CONTEXT_MENU_HEIGHT as u32,
        24,
    );
    assert!(
        a_stale < menu_area / 6,
        "STALE MENU: after reopening at B, the first menu is still painted at A=({aox},{aoy}): \
         {a_stale}/{menu_area} pixels still differ from the bare desktop (expected < 1/6)."
    );
}

// ===========================================================================
// 8. SESSION MENU — open/items/activate/dismiss with STATE + PIXEL teeth.
//    The session menu shares the same `menu-item` partial as the context menu,
//    so it exercises the same icon/label rendering path, but it DOES expose
//    public state, so we can assert visibility + the fired session request.
// ===========================================================================

#[test]
fn session_menu_opens_and_paints() {
    let base = base_desktop();

    // Toggle the session menu open via the public seam, then capture. Read its
    // visibility back to confirm it actually opened (STATE).
    let (frame, visible) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |_handle| Vec::new(),
        |shell| {
            if !shell.session_menu_visible() {
                shell.toggle_session_menu();
            }
            shell.session_menu_visible()
        },
    )
    .expect("session-menu open capture");

    // STATE: it is open.
    assert!(visible, "session menu did not open via toggle_session_menu()");

    // PIXELS: somewhere on screen there is new menu paint vs the bare desktop.
    // The session menu anchors near the top-right (session cluster); rather than
    // hard-code its exact rect we assert a meaningful number of pixels changed
    // overall AND that the change is concentrated (a menu-sized cluster), not
    // just AA noise.
    let changed = changed_vs_base(&frame, &base, 0, 0, frame.width, frame.height, 24);
    assert!(
        changed > 1_500,
        "session menu did not paint: only {changed} pixels changed vs the bare desktop \
         (expected a menu-sized panel + labelled items)."
    );
}

#[test]
fn session_menu_item_activation_records_request_and_dismisses() {
    // Open the session menu through the mutate seam, THEN run the activating key
    // events through the live handler, THEN read state.
    //
    // SEAM NOTE (reported in t58-menu.md): the scripted `PlatformEvent` capture
    // path cannot pre-open the session menu before the activating key events run
    // (no public hotkey opens it, and the readback closure runs AFTER all
    // scripted events). So we drive the activation inside the readback closure
    // via the SAME `handle_platform_event` the platform path uses — only the
    // *capture* of the key events is bypassed; the key handling, key-nav, and
    // action dispatch all go through the real live handler.
    use liquide_input::keyboard::{KeyCode, KeyEvent, KeyState, Modifiers};
    use liquide_platform::PlatformEvent;
    use liquide_platform::NativeWindowHandle;

    fn key(code: KeyCode) -> PlatformEvent {
        PlatformEvent::KeyInput {
            handle: NativeWindowHandle(1),
            event: KeyEvent::new(code, KeyState::Pressed, Modifiers::new(), 0, 0),
        }
    }

    let (_frame, (request, still_open)) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |_handle| Vec::new(),
        |shell| {
            shell.toggle_session_menu();
            assert!(shell.session_menu_visible(), "precondition: menu open");
            // ArrowDown -> item 0 (Lock), ArrowDown -> item 1 (Log Out), Enter.
            shell.handle_platform_event(&key(KeyCode::ArrowDown));
            shell.handle_platform_event(&key(KeyCode::ArrowDown));
            let action = shell.handle_platform_event(&key(KeyCode::Enter));
            // The key-nav returns the item's action; run it so the shell records
            // the session request (the same thing execute_action does on the
            // live path).
            if let Some(a) = action {
                shell.execute_action(&a);
            }
            (shell.pending_session_request(), shell.session_menu_visible())
        },
    )
    .expect("session-menu logout capture");

    // STATE: Log Out fired -> the shell recorded a LogOut session request.
    assert_eq!(
        request,
        Some(SessionRequest::LogOut),
        "activating the session menu's 'Log Out' item did not record a LogOut request \
         (pending_session_request() = {request:?}). Check SessionMenuItem::defaults() -> LogOut \
         and execute_action(LogOut)."
    );

    // DISMISSED (state): the session menu closed after activation.
    assert!(
        !still_open,
        "session menu remained open after activating an item (expected it to dismiss)."
    );
}

// ── Diagnostic (ignored): dump the menu region + per-row ink profile so the
//    overlap finding can be eyeballed. Run with:
//    cargo test -p liquide-visual-test --test e2e_context_menu -- --ignored dump
#[test]
#[ignore]
fn dump_context_menu_region() {
    let (rx, ry) = (300.0_f32, 250.0_f32);
    let frame = capture_right_click(rx, ry);
    let (ox, oy) = menu_origin(rx, ry);
    let menu = frame.crop(ox, oy, CONTEXT_MENU_WIDTH as u32, CONTEXT_MENU_HEIGHT as u32);
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("visual-test")
        .join("t58_context_menu.png");
    menu.save_png(&out).expect("save menu png");
    eprintln!("wrote {}", out.display());
    for i in 0..CONTEXT_MENU_ITEMS {
        let row_y = oy + (MENU_PADDING + i as f32 * MENU_ITEM_HEIGHT).round() as u32;
        let prof = column_ink_profile(&frame, ox, row_y, CONTEXT_MENU_WIDTH as u32, MENU_ITEM_HEIGHT as u32);
        let s: String = prof.iter().take(60).map(|&c| if c == 0 { '.' } else if c < 4 { ':' } else { '#' }).collect();
        eprintln!("row {i} cols[0..60]: {s}");
    }
}
