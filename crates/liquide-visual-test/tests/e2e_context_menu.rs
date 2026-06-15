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
    // A click point comfortably inside the screen so the menu is NOT clamped, AND
    // over the DARKER (right) part of the gradient wallpaper so the translucent
    // panel separates cleanly from the background (see TOLERANCE note below):
    // 700 < 1280-200-4, 300 < 720-148-4. Menu top-left must equal (700, 300).
    let (rx, ry) = (700.0_f32, 300.0_f32);
    let base = base_desktop();
    let frame = capture_right_click(rx, ry);
    let (ox, oy) = menu_origin(rx, ry);

    // The menu rect [ox, ox+200] x [oy, oy+148] must carry substantial paint
    // that differs from the bare desktop (the panel + 5 item rows).
    //
    // PANEL-vs-BACKGROUND TOLERANCE (recalibrated, t69-harden2): the context menu
    // is a TRANSLUCENT "liquid glass" panel. The desktop background is now a
    // DESIGNED GRADIENT wallpaper (t69-wallpaper) — a purple/blue bloom on the LEFT
    // fading to near-black on the RIGHT. Over the bloom, the translucent panel fill
    // composites to almost exactly the bloom colour (measured max-channel delta ~7
    // at the old (300,250) click point), so the panel fill no longer crosses any
    // tolerance and a fully-painted menu read as "not painted" — the original
    // failure. The fix is geometric, not a weakened threshold: clicking over the
    // DARK side of the gradient (700,300: bg ~rgb(8,9,23)) restores a clean panel
    // separation — the panel fill ~rgb(16,22,47) gives a max-channel delta well
    // above `tol=16`, so the WHOLE panel counts (measured 25_454/29_600 px, 86%).
    // `tol=16` still excludes the faint full-screen scrim the menu lays (delta ~12).
    // This keeps full teeth: a menu painted at the wrong spot reads near-0 panel
    // coverage in this rect, so an absent/mispositioned menu still fails.
    let panel_tol = 16u8;
    let menu_changed = changed_vs_base(
        &frame,
        &base,
        ox,
        oy,
        CONTEXT_MENU_WIDTH as u32,
        CONTEXT_MENU_HEIGHT as u32,
        panel_tol,
    );
    let menu_area = (CONTEXT_MENU_WIDTH * CONTEXT_MENU_HEIGHT) as usize;
    assert!(
        menu_changed > menu_area / 3,
        "context menu did not paint at the cursor: only {menu_changed}/{menu_area} pixels in \
         the menu rect at ({ox},{oy}) changed vs the bare desktop (expected > 1/3 of the rect). \
         The menu is missing, blank, or painted somewhere else."
    );

    // NOT at the origin: the top-left 200x148 corner of the screen (where a
    // broken (0,0)-anchored menu would land) must carry FAR less change than the
    // menu rect. RELATIVE (not absolute) tooth, t69-harden2: the menu open lays a
    // faint full-screen scrim, and over the bright LEFT-side wallpaper bloom that
    // scrim now crosses `tol=16` for ~35% of the corner — so the old absolute
    // "corner ~unchanged" probe no longer holds. The discriminating signal is that
    // the real menu's PANEL makes its rect change ~2.3x more than the bare scrim
    // changes the corner; a corner-anchored menu would invert that ratio. We
    // require the menu rect to change at least 1.5x the corner.
    let corner_changed =
        changed_vs_base(&frame, &base, 0, 40, CONTEXT_MENU_WIDTH as u32, 108, panel_tol);
    assert!(
        menu_changed > corner_changed * 3 / 2,
        "context menu does not appear anchored at the click point: the menu rect at ({ox},{oy}) \
         changed {menu_changed} px but the screen corner changed {corner_changed} px — the menu \
         rect should change far more than the corner (a corner-anchored or absent menu inverts \
         this)."
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
// 3. ICON / LABEL NO-OVERLAP — the icon (a contiguous ink cluster near the LEFT
//    edge) and the label text must occupy DISTINCT, non-overlapping column spans
//    separated by a clear (zero-ink) gap.
//
//    BOX MODEL (proven render, t62-harden — see `.orchestration/logs/t62-icon.md`
//    and the `dump_context_menu_region` diagnostic):
//      - the `context-menu` PANEL has `padding: 4` + `border-width: 1` = 5px left
//        inset, so the item content area starts 5px right of the panel origin.
//      - `menu-item { padding-left: 12 }`  → icon box at 5 + 12 = col 17.
//      - `menu-item-icon { width: 16; margin-right: 8 }` → icon box [17, 33),
//        label at 17 + 16 + 8 = col 41.
//    The rasterised icon glyph fills [~19, 32] (its 16px box with a ~5% inset),
//    then a CLEAR gap [~33, 40], then the label glyphs from col ~41. The earlier
//    expectation (`pad_left = 12`, `label_start = 36`) ignored the panel's own
//    5px inset and miscounted the icon's right half as label intrusion.
//
//    This test does NOT hardcode those column numbers: it locates the icon
//    cluster and the label dynamically from the ink profile, so it stays correct
//    under theme/padding changes. The invariant it enforces is the real contract:
//    the icon cluster and the label are separated by at least one fully-clear
//    column (no shared ink, no abutting glyphs).
//
//    KEEP TEETH: if the icon ink and the label ink ever share columns (the real
//    overlap bug — icon glyph spilling onto the label, or the label starting
//    inside the icon box with no separator), there is no clear gap between the
//    two ink clusters and this FAILS. Proven by injecting/reverting the overlap
//    in t62-harden.
// ===========================================================================

/// A contiguous run of inked columns `[start, end)` (relative to the band x0).
#[derive(Debug, Clone, Copy)]
struct InkCluster {
    start: usize,
    end: usize,
}

/// Split a column-ink profile into maximal contiguous runs of inked columns,
/// tolerating up to `bridge` consecutive zero-ink columns inside a single run
/// (so the internal hollows of a glyph/icon do not fragment it). A column counts
/// as inked when its ink height exceeds `min_ink` (suppresses 1px AA specks).
fn ink_clusters(profile: &[usize], min_ink: usize, bridge: usize) -> Vec<InkCluster> {
    let mut clusters: Vec<InkCluster> = Vec::new();
    let mut run_start: Option<usize> = None;
    let mut gap = 0usize;
    for (i, &c) in profile.iter().enumerate() {
        if c > min_ink {
            if run_start.is_none() {
                run_start = Some(i);
            }
            gap = 0;
        } else if let Some(start) = run_start {
            gap += 1;
            if gap > bridge {
                clusters.push(InkCluster { start, end: i - gap + 1 });
                run_start = None;
                gap = 0;
            }
        }
    }
    if let Some(start) = run_start {
        clusters.push(InkCluster { start, end: profile.len() - gap });
    }
    clusters
}

#[test]
fn context_menu_icon_and_label_do_not_overlap() {
    let (rx, ry) = (300.0_f32, 250.0_f32);
    let frame = capture_right_click(rx, ry);
    let (ox, oy) = menu_origin(rx, ry);

    // The icon box's left edge in CORRECT geometry (panel inset 5 + padding-left
    // 12 = 17). The icon is the first ink cluster at/near this column; the label
    // is the next cluster to its right. We do NOT assume the icon's exact width
    // or the label's exact start — only that, when an icon is drawn, a clear gap
    // separates it from the label.
    let icon_box_left = (MENU_PADDING as u32) + 1 /* border */ + 12 /* menu-item padding-left */; // 17
    let icon_box_w = 16u32;
    // Where the icon cluster may legitimately begin (its box left, minus a small
    // tolerance for left-bearing/AA). Anything starting well left of this is the
    // overlap bug (content collapsed toward the border edge).
    let icon_search_left = icon_box_left.saturating_sub(2) as usize;

    let mut violations = Vec::new();
    for (i, label) in EXPECTED_LABELS.iter().enumerate() {
        let row_y = oy + (MENU_PADDING + i as f32 * MENU_ITEM_HEIGHT).round() as u32;
        let row_h = MENU_ITEM_HEIGHT as u32;
        let profile = column_ink_profile(&frame, ox, row_y, CONTEXT_MENU_WIDTH as u32, row_h);

        // Decompose the row into ink clusters. `bridge = 2` keeps a glyph/icon
        // with thin internal hollows as one cluster. `min_ink = 2` (a column counts
        // only when its contrast-ink height is ≥3 px) suppresses 1–2px antialiasing
        // SPECKS at the panel border edge. (t65-harden2: removing the hardcoded
        // backdrop so the themed `desktop-background` shows lowered the panel-vs-bg
        // contrast just enough that a 2px AA speck appears at col 0 of the top row;
        // at the old `min_ink = 0` that speck registered as the row's first ink
        // cluster, left of the icon box, falsely tripping the "collapsed to border"
        // overlap check. Real icon/label glyph columns start at ≥3px (the icon box
        // begins at col ~18 with 3–8px ink), so they survive this threshold intact —
        // see `.orchestration/logs/t65-harden2.md`.) TEETH PRESERVED: a genuine
        // collapse-to-border or icon overrun paints far more than 2px per column and
        // is still detected (the `teeth_overlap_cluster_logic` test, with 10px ink,
        // continues to fire).
        let clusters = ink_clusters(&profile, 2, 2);
        if clusters.is_empty() {
            // No ink at all -> a blank row is the all-items test's concern; we
            // cannot judge icon/label overlap with nothing drawn.
            continue;
        }

        // Identify the icon cluster: the first cluster whose start is within the
        // icon box region (near `icon_box_left`). If the first cluster starts far
        // to the LEFT of the icon box (collapsed to the border edge) that is the
        // overlap/padding-drop regression — flag it.
        let first = clusters[0];
        if first.start < icon_search_left {
            violations.push(format!(
                "row {i} ({label}): first ink cluster starts at col {} — LEFT of the icon box \
                 (col {icon_box_left}); content has collapsed toward the panel border edge \
                 (padding/inset dropped), which is the icon/label overlap regression",
                first.start
            ));
            continue;
        }

        // Is the first cluster the ICON (it sits in the icon box and is roughly
        // icon-width), or is this an icon-less row whose first cluster is already
        // the label? An icon cluster starts within ~[icon_box_left-1, icon_box_left+4]
        // and spans on the order of the 16px box; a label-first row's first
        // cluster starts further right (past the icon box + its margin).
        let icon_box_right = (icon_box_left + icon_box_w) as usize;
        let starts_in_icon_box =
            first.start >= icon_search_left && first.start <= (icon_box_left + 4) as usize;

        if !starts_in_icon_box {
            // This row has no icon (e.g. "Change Wallpaper"): the first cluster is
            // the label and it correctly begins at/after the icon box's reserved
            // region. Nothing to separate; no overlap possible.
            continue;
        }

        // The first cluster is the icon. It MUST end at/before the icon box's
        // right edge (plus a tiny AA tolerance). An icon that bleeds well past
        // its 16px box is spilling toward the label (the glyph-oversize bug).
        if first.end > icon_box_right + 4 {
            violations.push(format!(
                "row {i} ({label}): icon cluster spans cols [{}, {}) — it overruns its 16px box \
                 [{icon_box_left}, {icon_box_right}) and bleeds toward the label band",
                first.start, first.end
            ));
            continue;
        }

        // There MUST be a SECOND cluster (the label) to the right, and a clear
        // zero-ink separator between the icon's end and the label's start.
        let Some(label_cluster) = clusters.get(1).copied() else {
            // Icon present but no label cluster found — covered by the all-items
            // ink test; not an overlap.
            continue;
        };
        let gap = label_cluster.start.saturating_sub(first.end);
        if gap == 0 {
            violations.push(format!(
                "row {i} ({label}): NO clear separator between the icon (ends col {}) and the \
                 label (starts col {}) — they abut/overlap; the icon and label must not share or \
                 touch columns",
                first.end, label_cluster.start
            ));
        }
    }

    assert!(
        violations.is_empty(),
        "ICON/LABEL OVERLAP detected: {violations:?}. The correct box model reserves the icon box \
         at col {icon_box_left} (panel inset 5 + menu-item padding-left 12), width 16, then a clear \
         margin before the label — the rendered menu must keep the icon and label in distinct, \
         gap-separated column spans."
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

    // DISMISSED (pixels): the LEFT strip of the former menu rect, BELOW the opened
    // window's titlebar band, must contain NO menu PANEL — no panel border or item
    // ink. The opened app window is centred; this left strip at x in [rx, rx+24]
    // within the former menu rect is a clean dismissal probe.
    //
    // INTERNAL-INK probe, not diff-vs-base (t69-harden2): the old probe required
    // the strip to read close to the BARE desktop. That broke with the new depth /
    // gradient work — activating "Open Terminal" lays a faint full-screen scrim that
    // DIMS the desktop (and, over the gradient wallpaper's bright LEFT-side bloom,
    // that dimming alone shifts the strip by ~30/channel vs the bare desktop), so a
    // correctly DISMISSED menu still "differed from base" everywhere. The robust
    // dismissal signal is INTERNAL contrast: a stale menu panel carries border +
    // glyph ink (measured strip ink = 82 while OPEN), whereas a dismissed strip is
    // uniform desktop fill — even when scrim-dimmed — and carries ZERO ink
    // (measured: 0 dismissed, 0 bare). `column_ink_profile` deviates from the
    // strip's OWN mean, so a uniform-but-dimmed strip reads clean. Teeth intact: a
    // persistent menu reads tens of ink columns and fails.
    let (ox, oy) = menu_origin(rx, ry);
    // Probe the lower portion of the former menu rect (rows 2..5), 24px wide at
    // the menu's left edge — clear of a centred window's body at this x.
    let probe_y = oy + (MENU_PADDING + 2.0 * MENU_ITEM_HEIGHT) as u32;
    let probe_h = (3.0 * MENU_ITEM_HEIGHT) as u32;
    let profile = column_ink_profile(&frame, ox, probe_y, 24, probe_h);
    let stale_ink: usize = profile.iter().sum();
    assert!(
        stale_ink < 16,
        "context menu appears STILL PAINTED after activation: the former menu's left strip carries \
         {stale_ink} ink pixels (panel border / item glyphs) — expected ~0 once dismissed. A \
         persistent menu after activation is a dismissal bug."
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

    // PANEL-PRESENCE tolerance. The context menu is a TRANSLUCENT "liquid glass"
    // panel: over the wallpaper its fill blends to a small-but-consistent
    // per-pixel delta (mostly 4..23), with only the borders/text/icons producing
    // large deltas. Measuring the panel's PRESENCE (not just its bright glyph
    // ink) therefore requires a low tolerance — at tol=24 the glass fill is
    // mostly invisible (only ~8985 px counted) and the test undercounts a
    // fully-painted menu. The deterministic capture renders at t0, so an
    // UNPAINTED region reads EXACTLY base (0 px differ even at tol=4); a painted
    // panel reads ~25k px differ. tol=8 cleanly separates the two with a wide
    // margin (proven, t62-harden probe):
    //   tol=8:  A_stale(empty)=0   B_painted(panel)=25353
    //   tol=24: A_stale(empty)=0   B_painted(panel)=8985   <- glass fill lost
    // Per t62-paint the menu paints fully; this recalibration measures that
    // panel delta-vs-base rather than only bright ink, while KEEPING TEETH (an
    // empty/stale region still reads ~0, far below both thresholds).
    let panel_tol = 8u8;
    let menu_area = (CONTEXT_MENU_WIDTH * CONTEXT_MENU_HEIGHT) as usize;

    // The SECOND menu must be painted at B (panel present).
    let b_painted = changed_vs_base(
        &frame,
        &base,
        box_,
        boy,
        CONTEXT_MENU_WIDTH as u32,
        CONTEXT_MENU_HEIGHT as u32,
        panel_tol,
    );
    assert!(
        b_painted > menu_area / 3,
        "the reopened context menu did not paint at the second point B=({box_},{boy}): only \
         {b_painted}/{menu_area} pixels (tol {panel_tol}) differ from the bare desktop."
    );

    // The FIRST menu must be GONE at A (A and B rects are disjoint: A occupies
    // [200,400]x[200,348], B occupies [700,900]x[450,598]). This is the real
    // tooth: a stale first menu would leave a full panel (~25k px) here. We use
    // the SAME low tolerance, so a lingering glass panel cannot hide under it.
    let a_stale = changed_vs_base(
        &frame,
        &base,
        aox,
        aoy,
        CONTEXT_MENU_WIDTH as u32,
        CONTEXT_MENU_HEIGHT as u32,
        panel_tol,
    );
    assert!(
        a_stale < menu_area / 6,
        "STALE MENU: after reopening at B, the first menu is still painted at A=({aox},{aoy}): \
         {a_stale}/{menu_area} pixels (tol {panel_tol}) still differ from the bare desktop \
         (expected < 1/6)."
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
    //
    // TOLERANCE (recalibrated, t65-harden2): the session menu shares the same
    // translucent "liquid glass" panel as the context menu, so the same backdrop
    // change (hardcoded rgb(5,8,20) → themed rgb(12,14,28)) dropped its panel-fill
    // delta below the old `tol=24` (at 24 only ~1.5k px crossed — right on the
    // threshold and fragile). `tol=16` (below the ~17 panel-fill delta, above the
    // 0-px bare-capture noise) counts the full panel: 22_037 px, a wide margin over
    // the 1_500 floor while KEEPING TEETH — two bare desktops differ by 0 px at this
    // tolerance, so a menu that fails to paint reads ~0 and fails. See
    // `.orchestration/logs/t65-harden2.md`.
    let changed = changed_vs_base(&frame, &base, 0, 0, frame.width, frame.height, 16);
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

/// TEETH guard (pure logic, no capture) for
/// `context_menu_icon_and_label_do_not_overlap`'s separator check: prove the
/// cluster/gap logic FIRES on a real overlap and PASSES on correct geometry.
/// This keeps the corrected test honest — if someone relaxes the cluster/gap
/// rules, these synthetic overlaps stop being detected and this fails.
#[test]
fn teeth_overlap_cluster_logic() {
    // 1) Correct: icon at [17,32), gap [32,41), label [41,55).
    let mut good = vec![0usize; 60];
    for c in 17..32 { good[c] = 10; }
    for c in 41..55 { good[c] = 10; }
    let cg = ink_clusters(&good, 0, 2);
    assert_eq!(cg.len(), 2, "good profile -> 2 clusters");
    assert!(cg[1].start.saturating_sub(cg[0].end) > 0, "good has a gap");

    // 2) Overlap: icon at [17,32) and label glyph ink starts at col 32 (abutting,
    //    no clear column). bridge=2 will MERGE them into one cluster -> the test
    //    sees only one cluster (icon) and either no label cluster (skip) — so the
    //    stronger tooth is the icon-overrun check. Make the icon spill to col 40.
    let mut spill = vec![0usize; 60];
    for c in 17..40 { spill[c] = 10; }   // icon glyph oversized, overruns box
    for c in 44..58 { spill[c] = 10; }
    let cs = ink_clusters(&spill, 0, 2);
    let icon = cs[0];
    let icon_box_right = (17 + 16) as usize; // 33
    assert!(icon.end > icon_box_right + 4, "spill: icon overruns its box -> overlap detected (end={})", icon.end);

    // 3) Collapsed-to-border (the padding-drop regression): first ink at col 2.
    let mut collapsed = vec![0usize; 60];
    for c in 2..18 { collapsed[c] = 10; }
    for c in 22..36 { collapsed[c] = 10; }
    let cc = ink_clusters(&collapsed, 0, 2);
    let icon_search_left = (17u32 - 2) as usize; // 15
    assert!(cc[0].start < icon_search_left, "collapsed: first cluster left of icon box -> overlap detected (start={})", cc[0].start);
}
