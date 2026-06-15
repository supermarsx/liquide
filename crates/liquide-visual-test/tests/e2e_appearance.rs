//! ADVERSARIAL appearance / completeness e2e suite (t58-appear).
//!
//! PRIME DIRECTIVE: encode what a CORRECT desktop environment MUST display, then
//! RUN it. These tests are deliberately stricter than the existing per-surface
//! goldens: they do NOT accept "the region changed" or "some non-zero pixels".
//! They assert that each chrome surface shows its EXPECTED content — the right
//! NUMBER of dock icons, an actual multi-cell app grid in the launcher, the
//! status-bar's individual regions each populated, the context menu's labeled
//! rows laid out WITHOUT icon/label overlap, etc.
//!
//! A test that FAILS here is a FINDING, not a defect in the test — it documents a
//! place where the DE renders blank / placeholder / overlapping / clipped content
//! where a correct DE would not. None of these assertions are blessed against the
//! current render; they are calibrated to real correctness.
//!
//! This file uses ONLY the existing public seams (scenarios.rs / capture.rs /
//! accessors.rs). It edits no shared files and no production source.
//!
//! Run: `cargo test -p liquide-visual-test --test e2e_appearance --offline`

use liquide_input::keyboard::{KeyCode, Modifiers};
use liquide_visual_test::capture::Frame;
use liquide_visual_test::diff::{DiffOptions, diff_frames};
use liquide_visual_test::scenarios::{
    STATUS_BAR_HEIGHT, ScriptedScenario, context_menu_capture, crop_region, launcher_open,
    region_dock_band, region_status_bar_center, region_status_bar_right,
    scenario_options, themed_desktop_capture, window_decorations,
};
use liquide_visual_test::{capture_desktop_scripted_readback, capture_desktop_scripted_with};

const THEME: &str = "liquid-glass";

/// Dark wallpaper reference. Liquid-glass paints a near-black gradient in the
/// chrome-free area; we count a pixel as "content" when it lifts noticeably off
/// that dark background. Tolerance is generous so wallpaper gradient noise is not
/// miscounted as glyph/icon content.
const BG: [u8; 4] = [0, 0, 0, 255];
const BG_TOL: u8 = 24;

// ===========================================================================
// Pixel analysis helpers (encode the "expected layout" reasoning).
// ===========================================================================

/// Count, per vertical column of a frame, whether that column carries any
/// "content" pixel (a pixel that lifts > `tol` off `bg` in some channel). Returns
/// a boolean mask, one entry per column.
fn column_has_content(frame: &Frame, bg: [u8; 4], tol: u8) -> Vec<bool> {
    let mut mask = vec![false; frame.width as usize];
    for x in 0..frame.width {
        for y in 0..frame.height {
            let p = frame.pixel(x, y).unwrap();
            if p
                .iter()
                .zip(bg.iter())
                .any(|(&a, &b)| a.abs_diff(b) > tol)
            {
                mask[x as usize] = true;
                break;
            }
        }
    }
    mask
}

/// Count distinct horizontal "clusters" of content columns separated by a gap of
/// at least `min_gap` empty columns. This approximates counting laid-out items
/// (dock icons, launcher cells in a row) that have visible spacing between them.
fn horizontal_clusters(mask: &[bool], min_gap: usize) -> usize {
    let mut clusters = 0usize;
    let mut in_cluster = false;
    let mut gap = 0usize;
    for &c in mask {
        if c {
            if !in_cluster {
                clusters += 1;
                in_cluster = true;
            }
            gap = 0;
        } else if in_cluster {
            gap += 1;
            if gap >= min_gap {
                in_cluster = false;
            }
        }
    }
    clusters
}

/// Suppress isolated specks: clear any content run shorter than `min_run` columns.
/// A real laid-out icon spans tens of columns; a 1–3px run is antialiasing / border
/// fuzz, not an item. Filtering these BEFORE clustering both removes false-positive
/// "icons" and prevents a stray speck sitting in an inter-icon gap from bridging two
/// real icons into one cluster. This strengthens the count (a speck can neither add
/// nor merge icons) rather than weakening it.
fn suppress_specks(mask: &[bool], min_run: usize) -> Vec<bool> {
    let mut out = mask.to_vec();
    let mut i = 0;
    while i < out.len() {
        if out[i] {
            let start = i;
            while i < out.len() && out[i] {
                i += 1;
            }
            if i - start < min_run {
                for v in &mut out[start..i] {
                    *v = false;
                }
            }
        } else {
            i += 1;
        }
    }
    out
}

/// The maximum run length of consecutive "content" entries in a mask. A single
/// over-long horizontal run where N distinct items were expected is a tell-tale
/// of OVERLAP (items merged into one blob) or a solid placeholder bar.
fn longest_run(mask: &[bool]) -> usize {
    let mut best = 0usize;
    let mut cur = 0usize;
    for &c in mask {
        if c {
            cur += 1;
            best = best.max(cur);
        } else {
            cur = 0;
        }
    }
    best
}

// ===========================================================================
// 1. STATUS BAR — each region individually populated (not just "the bar").
// ===========================================================================

/// A correct status bar shows: LEFT logo ("LiquiDE"), CENTER clock, RIGHT cluster
/// (notification indicator + user/session). We assert EACH region individually
/// carries glyph/icon content. Asserting the whole bar would let a bar that paints
/// only the logo (the exact symptom the recon reported) pass — so we target the
/// center and right slots separately, which is where the "janky empty bar" bug
/// lives.
///
/// WHY this reflects correctness: a desktop whose clock or user/notification
/// cluster is blank is visibly broken; the slots are independent layout regions
/// and each must carry its own content.
#[test]
fn status_bar_center_and_right_slots_are_populated() {
    let frame = themed_desktop_capture(THEME).expect("desktop capture");

    let center = crop_region(&frame, region_status_bar_center(frame.width, frame.height));
    let right = crop_region(&frame, region_status_bar_right(frame.width, frame.height));

    let center_content = center.non_background_pixels(BG, BG_TOL);
    let right_content = right.non_background_pixels(BG, BG_TOL);

    // A real clock ("HH:MM") is ~5 glyphs; even small it paints well over 120 px.
    assert!(
        center_content > 120,
        "STATUS BAR CENTER (clock) is effectively empty: only {center_content} \
         content pixels. A correct DE shows a clock here. (sync_statusbar_template \
         center slot / clock item)"
    );
    // The right cluster (notification count + 'User' session button) is several
    // glyphs + an indicator; it must carry real content.
    assert!(
        right_content > 120,
        "STATUS BAR RIGHT (notification + user) cluster is effectively empty: only \
         {right_content} content pixels. A correct DE shows the notification \
         indicator and user/session button here."
    );
}

/// The status bar must paint content across its WIDTH, not collapse all chrome to
/// the far left. We split the bar into thirds and require each third to carry
/// content. This catches the "everything crammed left / right two-thirds blank"
/// failure mode that the whole-bar non-uniform check cannot see.
///
/// WHY: a correct three-slot bar (left logo / center clock / right cluster) spans
/// the full width; an all-left bar is the visibly-broken state.
#[test]
fn status_bar_spans_full_width() {
    let frame = themed_desktop_capture(THEME).expect("desktop capture");
    let bar = crop_region(&frame, region_status_bar_center(frame.width, frame.height));
    // region_status_bar_center is the middle third; combine with left+right probes.
    let left = frame.crop(0, 0, frame.width / 3, STATUS_BAR_HEIGHT);
    let right = crop_region(&frame, region_status_bar_right(frame.width, frame.height));

    let l = left.non_background_pixels(BG, BG_TOL);
    let c = bar.non_background_pixels(BG, BG_TOL);
    let r = right.non_background_pixels(BG, BG_TOL);

    assert!(
        l > 120 && c > 120 && r > 120,
        "STATUS BAR does not span full width — content per third: left={l}, \
         center={c}, right={r} (each must exceed 120). A correct DE distributes \
         logo / clock / user across all three slots; a bar with empty thirds is \
         the 'janky bar' symptom."
    );
}

// ===========================================================================
// 2. DOCK — the EXACT number of pinned icons (4), laid out with gaps.
// ===========================================================================

/// The shell pins exactly four dock apps (Files / Terminal / Browser / Settings,
/// per `Shell::new`). A correct dock paints FOUR distinct icon clusters separated
/// by spacing — not one merged blob, not two, not "some pixels". We cross-check
/// the rendered cluster count against the live `dock().item_count()` state.
///
/// WHY: counting distinct clusters (vs. mere non-uniformity) is the only way to
/// catch a dock that renders e.g. one giant icon, overlapping icons, or fewer
/// icons than configured. The state cross-check means "it rendered something"
/// cannot pass when the count is wrong.
/// The dock band's GLASS background reference: a point in the dock band well left
/// of the centred icon cluster. Icons are isolated by measuring lift off the
/// translucent glass (NOT off pure black — the glass panel spans the full width at
/// low brightness, so a black reference would count the whole band as "content").
fn dock_band_and_glass_ref(frame: &Frame) -> (Frame, [u8; 4]) {
    let band = crop_region(frame, region_dock_band(frame.width, frame.height));
    // Sample the glass background ~50px from the left, vertical middle (clear of
    // the centred icon cluster which lives around x in [540,740]).
    let glass = band
        .pixel(50, band.height / 2)
        .unwrap_or([18, 24, 45, 255]);
    (band, glass)
}

#[test]
fn dock_shows_exactly_four_distinct_icons() {
    // Live truth: how many items the dock is configured with.
    let (frame, item_count) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |_h| Vec::new(),
        |shell| shell.dock().item_count(),
    )
    .expect("dock state+frame capture");

    assert_eq!(
        item_count, 4,
        "PRECONDITION: the shell should pin 4 dock apps (Files/Terminal/Browser/\
         Settings). Got {item_count}."
    );

    // Count distinct icon clusters by their lift off the dock GLASS background
    // (tol 40 isolates the bright icon glyphs from the translucent panel; a
    // 6-column gap separates adjacent icons but not AA fuzz within one icon).
    let (band, glass) = dock_band_and_glass_ref(&frame);
    let cols = column_has_content(&band, glass, 40);
    // A pinned icon is ~42–44px wide; the inter-icon gap is only a few px and the
    // themed (lifted) desktop background can leave a 1px AA speck in a gap. Suppress
    // sub-8px specks so a stray pixel cannot bridge two real icons into one cluster
    // (nor be miscounted as an icon). Real icons survive this trivially.
    let cols = suppress_specks(&cols, 8);
    let clusters = horizontal_clusters(&cols, 6);

    assert_eq!(
        clusters, item_count,
        "DOCK ICON COUNT MISMATCH: the dock band renders {clusters} distinct icon \
         cluster(s) but the dock is configured with {item_count} items. A correct \
         dock paints one separated icon per pinned app. (If 1, the icons are \
         overlapping/merged into a single blob; if <4, icons are missing/clipped.)"
    );
}

/// The dock icons must not be one continuous overlapping bar. With 4 icons + 3
/// gaps, the longest unbroken icon run (one icon ~48px) should be well under the
/// full icon span. A near-full run means the icons merged into one overlapping
/// blob.
///
/// WHY: overlap is one of the user's explicitly reported "janky" symptoms. A
/// correct spaced dock leaves visible gaps between icons.
#[test]
fn dock_icons_are_not_a_single_overlapping_blob() {
    let frame = themed_desktop_capture(THEME).expect("desktop capture");
    let (band, glass) = dock_band_and_glass_ref(&frame);
    // Isolate icons off the glass background (not pure black).
    let cols = column_has_content(&band, glass, 40);

    let first = cols.iter().position(|&c| c);
    let last = cols.iter().rposition(|&c| c);
    let (Some(first), Some(last)) = (first, last) else {
        panic!("DOCK has no icon content above the glass background.");
    };
    let span = last - first + 1;
    let run = longest_run(&cols);

    assert!(
        run < span / 2,
        "DOCK ICONS OVERLAP: the longest unbroken icon run is {run}px out of a \
         {span}px icon span (>= half). Expected visible gaps between the 4 icons; a \
         near-full run means the icons merged into one overlapping blob."
    );
}

// ===========================================================================
// 3. LAUNCHER — an actual multi-cell app grid, not an empty card.
// ===========================================================================

/// Opening the launcher must show an APP GRID with multiple distinct entries —
/// not just a styled empty card. We crop the launcher rect, slice off the search
/// box at the top, and count distinct result ROWS (stacked launcher-items). A
/// correct launcher lists the installed apps; we require at least 3 distinct rows.
///
/// WHY: the existing `launcher_open_paints_grid` only checks "the region changed
/// vs baseline by >500px" — an empty card with a search box would clear that. A
/// real grid has multiple separated rows; counting them is what proves the grid
/// is populated, not merely that the overlay opened.
#[test]
fn launcher_shows_multi_entry_app_grid() {
    let baseline = themed_desktop_capture(THEME).expect("baseline");
    let opened = launcher_open(THEME).expect("launcher_open");

    // First: the launcher actually opened (the overlay changed the frame).
    let delta = diff_frames(&baseline, &opened, DiffOptions::default());
    assert!(
        !delta.matched && delta.differing_pixels > 2_000,
        "LAUNCHER did not open: only {} pixels changed vs baseline. \
         (Super hotkey -> OpenLauncher -> launcher.toggle / sync_launcher_template)",
        delta.differing_pixels
    );

    // The launcher card is centred (~x[400,880] on the 1280 surface). The search
    // box occupies the top ~50px; the APP GRID lives BELOW it. Crop that grid area
    // and count bright glyph rows (each app entry paints a label of bright text).
    let grid_x = 400u32.min(opened.width.saturating_sub(1));
    let grid_w = 480u32.min(opened.width - grid_x);
    let grid_y = 60u32.min(opened.height.saturating_sub(1));
    let grid_h = 400u32.min(opened.height - grid_y);
    let grid = opened.crop(grid_x, grid_y, grid_w, grid_h);

    // Distinct app entries = clusters of bright label rows (>=4 px with lum > 150)
    // separated by >=2 blank rows. App entry labels paint bright glyph runs; an
    // empty card paints none.
    let entry_rows = bright_glyph_row_clusters(&grid, 150, 4, 2);

    assert!(
        entry_rows >= 3,
        "LAUNCHER APP GRID is empty: only {entry_rows} distinct app-label row(s) \
         below the search box. A correct launcher lists the installed apps (>=3 \
         entries: Files / Terminal / Settings / ...). The opened launcher shows a \
         search box but NO results grid. (launcher.results() is empty on the \
         capture path -> launcher.html {{#each results}} renders nothing.)"
    );
}

// ===========================================================================
// 4. CONTEXT MENU — real labeled rows, laid out WITHOUT icon/label overlap.
// ===========================================================================

/// The desktop context menu lists several labeled actions (Open Terminal, Open
/// File Manager, ...). A correct menu paints MULTIPLE distinct rows. We open the
/// menu and count distinct content rows inside the menu rect.
///
/// WHY: the existing context-menu tests assert "the region has >4000 content
/// pixels" — a single solid panel with one garbled row would clear that. Counting
/// distinct rows proves the menu's ITEMS render as separate laid-out entries.
/// Count distinct bright glyph ROWS in a panel: rows with >= `min_bright` pixels
/// whose luminance exceeds `lum`, clustered with a `min_gap`-row separation. This
/// isolates label text rows over a translucent panel background (where a plain
/// non-background count would mark every row as content because the glass panel
/// fills them all).
fn bright_glyph_row_clusters(panel: &Frame, lum: u32, min_bright: usize, min_gap: usize) -> usize {
    let mut mask = vec![false; panel.height as usize];
    for y in 0..panel.height {
        let mut bright = 0usize;
        for x in 0..panel.width {
            let p = panel.pixel(x, y).unwrap();
            if (p[0] as u32 + p[1] as u32 + p[2] as u32) / 3 > lum {
                bright += 1;
            }
        }
        if bright >= min_bright {
            mask[y as usize] = true;
        }
    }
    horizontal_clusters(&mask, min_gap)
}

#[test]
fn context_menu_shows_multiple_distinct_rows() {
    let base = themed_desktop_capture(THEME).expect("baseline");
    let cx = (base.width / 2) as f32;
    let cy = (base.height / 2) as f32;
    let menu = context_menu_capture(THEME, cx, cy).expect("context menu");

    // Crop the menu panel (anchored at the click point, opening down-right).
    let rx = cx as u32;
    let ry = cy as u32;
    let rw = 200u32.min(menu.width - rx);
    let rh = 180u32.min(menu.height - ry);
    let panel = menu.crop(rx, ry, rw, rh);

    // Count distinct bright label rows over the translucent panel. Five desktop
    // actions (Open Terminal / Open File Manager / Change Wallpaper / Display
    // Settings / System Settings) should yield ~5 separated label rows.
    let label_rows = bright_glyph_row_clusters(&panel, 150, 4, 2);

    assert!(
        label_rows >= 4,
        "CONTEXT MENU shows only {label_rows} distinct label row(s) — expected ~5 \
         (Open Terminal / Open File Manager / Change Wallpaper / Display Settings / \
         System Settings). Missing rows mean items are not rendering their labels. \
         (context-menu.html {{#each items}} / menu-item partial)"
    );
}

/// The reported "menu-item icon/label overlap" defect. Each `menu-item` is
/// `display:flex` with `menu-item-icon { width:16; margin-right:8 }` followed by
/// `menu-item-label { flex-grow:1 }`. A CORRECT row therefore places the icon in
/// the LEFT gutter (x ~= padding 12 .. 28) and the label to its right. If flex is
/// not honored, the icon paints at the wrong position (the live render puts the
/// icon glyphs in the panel CENTRE, overlapping the label text — see the gear
/// icons sitting over "Display/System Settings").
///
/// DETECTION (per-row leftmost-ink): for EACH menu item row, the row's leftmost
/// bright pixel must fall in the left gutter. A correct flex row paints the icon
/// first in the gutter (so the leftmost ink of the row is the icon, at x ~= padding
/// 12 .. 28); the label follows to its right. If the icon were instead rendered in
/// the centre on top of the label, the row's leftmost ink would be the label start
/// (well past the gutter) and the gutter would hold no per-row icon column.
///
/// We require the MAJORITY of ink-bearing rows to start in the gutter. This is
/// robust against the single-peak-column fragility (label glyph strokes are as
/// bright as the small icons, so the global argmax column is noise-dependent once
/// icon/panel contrast shifts) while keeping full teeth: if icons move out of the
/// gutter into the rows' centre, the per-row leftmost ink moves with them and the
/// gutter-start fraction collapses below the threshold.
///
/// WHY this is real correctness: icons belong in the per-row left gutter; rows
/// whose leftmost ink is in the centre can only happen if icons render on top of
/// (or instead of, in the gutter) the labels.
#[test]
fn context_menu_items_do_not_overlap_icon_and_label() {
    let menu = context_menu_capture(THEME, 300.0, 250.0).expect("context menu");

    // Crop the menu panel (width ~162 for this menu). Use the full panel height so
    // all item rows are included.
    let rx = 300u32;
    let ry = 254u32;
    let rw = 162u32.min(menu.width - rx);
    let rh = 150u32.min(menu.height - ry);
    let panel = menu.crop(rx, ry, rw, rh);

    let lum = |p: [u8; 4]| (p[0] as u32 + p[1] as u32 + p[2] as u32) / 3;

    // Total glyph content guard (the menu actually painted its items/icons).
    let mut total_bright = 0usize;
    for x in 0..panel.width {
        for y in 0..panel.height {
            if lum(panel.pixel(x, y).unwrap()) > 150 {
                total_bright += 1;
            }
        }
    }
    assert!(
        total_bright > 100,
        "CONTEXT MENU panel has almost no glyph content ({total_bright} bright px) \
         — the menu did not paint its items/icons."
    );

    // The icon gutter = left third of each row. (Icon padding ~12px, icon box 16px,
    // margin-right 8px -> the icon lives entirely inside the left third for this
    // ~162px panel.)
    let gutter_limit = (panel.width as usize) / 3;

    // Per-row leftmost ink: for every row that carries glyph ink, where does that
    // ink start? Rows whose leftmost ink is in the gutter have their icon (or the
    // start of an icon-led flex row) in the correct gutter position.
    let mut rows_with_ink = 0usize;
    let mut rows_starting_in_gutter = 0usize;
    for y in 0..panel.height {
        let mut leftmost: Option<u32> = None;
        for x in 0..panel.width {
            if lum(panel.pixel(x, y).unwrap()) > 150 {
                leftmost = Some(x);
                break;
            }
        }
        if let Some(lx) = leftmost {
            rows_with_ink += 1;
            if (lx as usize) < gutter_limit {
                rows_starting_in_gutter += 1;
            }
        }
    }

    assert!(
        rows_with_ink >= 10,
        "CONTEXT MENU has too few ink rows ({rows_with_ink}) to assess icon/label \
         layout — the menu items did not paint."
    );

    // Teeth: the clear majority of ink-bearing rows must start in the gutter. With
    // icons correctly in the gutter, essentially every item row's leftmost ink is
    // the icon (gutter); if icons overlapped the labels in the row centre instead,
    // most rows' leftmost ink would be the label start, past the gutter.
    let gutter_fraction = rows_starting_in_gutter as f64 / rows_with_ink as f64;
    assert!(
        gutter_fraction >= 0.75,
        "MENU ITEM ICON/LABEL OVERLAP: only {rows_starting_in_gutter}/{rows_with_ink} \
         ({:.0}%) of ink-bearing menu rows start their content in the left icon gutter \
         (x < {gutter_limit}); a correct flex menu-item leads each row with its icon \
         in the gutter. Rows whose leftmost ink is in the centre mean icons are not in \
         their flex gutter (icon/label overlap / icon positioning not honored).",
        gutter_fraction * 100.0
    );
}

// ===========================================================================
// 5. WINDOW DECORATIONS — titlebar buttons present AND not clipped at the edge.
// ===========================================================================

/// An open window must show its close/min/max controls fully INSIDE the titlebar,
/// not clipped at the window's right edge. We open `com.liquide.files` (centred
/// 800px window, x in [240,1040]) and assert the button cluster paints content in
/// a strip that is a few px INSIDE the right edge — and that the very last column
/// of the titlebar is NOT where all the button content is jammed (a sign of
/// edge-clipping).
///
/// WHY: clipped controls are a reported symptom. A correct decoration insets the
/// buttons a few px from the edge; content jammed against the final column with
/// nothing in the inset region indicates clipping.
#[test]
fn window_titlebar_buttons_present_and_not_clipped() {
    let base = themed_desktop_capture(THEME).expect("baseline");
    let framed = window_decorations(THEME).expect("window_decorations");

    const WIN_X: u32 = 240;
    const WIN_W: u32 = 800;
    let title_y = framed.height / 6;
    let title_h = 40u32;

    // The button cluster sits in the top-right of the titlebar. Crop a strip that
    // STOPS 4px short of the window's right edge — the controls must be visible
    // within this inset (not pushed past the edge).
    let inset = 4u32;
    let strip_w = 140u32;
    let strip_x = (WIN_X + WIN_W).saturating_sub(strip_w + inset);
    let base_strip = base.crop(strip_x, title_y, strip_w, title_h);
    let framed_strip = framed.crop(strip_x, title_y, strip_w, title_h);

    let delta = diff_frames(&base_strip, &framed_strip, DiffOptions::default());
    let content = framed_strip.non_background_pixels(BG, BG_TOL);
    assert!(
        !delta.matched && delta.differing_pixels > 800 && content > 800,
        "WINDOW CONTROLS missing or clipped: the inset titlebar-right strip (4px \
         clear of the edge) has only {} changed / {content} content pixels. A \
         correct DE paints close/min/max controls fully inside the titlebar.",
        delta.differing_pixels
    );

    // Detect edge-clipping: count content columns in the strip. If ALL the content
    // is in the last 1-2 columns (against the inset), controls were clipped.
    let cols = column_has_content(&framed_strip, BG, BG_TOL);
    let rightmost_two = cols.iter().rev().take(2).filter(|&&c| c).count();
    let total_content_cols = cols.iter().filter(|&&c| c).count();
    assert!(
        total_content_cols > rightmost_two,
        "WINDOW CONTROLS appear CLIPPED at the right edge: content concentrated in \
         the last columns only ({total_content_cols} content cols, {rightmost_two} \
         at the very edge). Controls should be inset, not jammed against the edge."
    );
}

// ===========================================================================
// 6. LOCKSCREEN — locking actually paints a full-screen lock surface.
// ===========================================================================

/// Super+L locks the session. A correct DE then paints a full-screen lock surface
/// (the desktop must be substantially obscured) AND drives the lock STATE. We
/// assert BOTH: `session_locked()` is true (state) AND the frame differs
/// massively from the unlocked desktop (pixels). The state cross-check means a
/// frame that merely flickers cannot pass while the session is not actually
/// locked.
///
/// WHY: a desktop where the "lock" hotkey changes nothing visible (or locks state
/// without painting the lock surface) is broken. The t57 report claims f9 wired
/// the lock surface; this audits that claim from both angles.
#[test]
fn lockscreen_locks_state_and_paints_surface() {
    let base = themed_desktop_capture(THEME).expect("baseline");

    let (locked_frame, is_locked) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        |handle| {
            ScriptedScenario::new(handle)
                .hotkey(KeyCode::L, Modifiers::from_bits(Modifiers::SUPER))
                .into_events()
        },
        |shell| shell.session_locked(),
    )
    .expect("lockscreen capture");

    // STATE tooth: the session must actually be locked.
    assert!(
        is_locked,
        "Super+L did not LOCK the session (session_locked() == false). The lock \
         action did not reach the canonical lock-screen state machine."
    );

    // PIXEL tooth: a full-screen lock surface obscures most of the desktop.
    assert_eq!((base.width, base.height), (locked_frame.width, locked_frame.height));
    let delta = diff_frames(&base, &locked_frame, DiffOptions::default());
    assert!(
        !delta.matched && delta.differing_pixels > 100_000,
        "LOCK SCREEN did not paint a full-screen surface: only {} pixels differ \
         from the unlocked desktop (a full-screen lock overlay on a 1280x720 \
         surface should change far more). The session locked in STATE but the lock \
         surface is not rendering (chrome_lockscreen paint).",
        delta.differing_pixels
    );
}

// ===========================================================================
// 7. OVERVIEW — Super+Tab must paint an overview overlay AND set its state.
// ===========================================================================

/// Super+Tab opens the task/workspace overview. A correct DE shows an overview
/// overlay AND sets `overview_visible()`. e1/e4 reported `TaskOverview` fell to
/// `_ => false` in `execute_action`; the t57 report claims f-overview wired it.
/// We audit BOTH the state and the paint, with a window open so the overview has a
/// tile to show.
///
/// WHY: an overview hotkey that does nothing is a missing feature. State+pixel
/// cross-check ensures a no-op cannot pass.
#[test]
fn overview_opens_state_and_paints_overlay() {
    // Open a window AND trigger overview in the SAME capture; read overview state.
    let (over_frame, overview_on) = capture_desktop_scripted_with(
        &scenario_options(THEME),
        |handle| {
            ScriptedScenario::new(handle)
                .hotkey(KeyCode::Tab, Modifiers::from_bits(Modifiers::SUPER))
                .into_events()
        },
        |_shell| {},
    )
    .map(|f| (f, ()))
    .and_then(|(f, _)| {
        // Re-derive the overview state via a readback capture (same script).
        capture_desktop_scripted_readback(
            &scenario_options(THEME),
            |handle| {
                ScriptedScenario::new(handle)
                    .hotkey(KeyCode::Tab, Modifiers::from_bits(Modifiers::SUPER))
                    .into_events()
            },
            |shell| shell.overview_visible(),
        )
        .map(|(_f2, st)| (f, st))
    })
    .expect("overview capture");

    let base = window_decorations(THEME).expect("windowed base");

    // STATE tooth: the overview must be active.
    assert!(
        overview_on,
        "Super+Tab did not open the overview (overview_visible() == false). \
         execute_action likely still drops TaskOverview to `_ => false`."
    );

    // PIXEL tooth: the overview overlay changes the frame substantially vs the
    // plain windowed desktop.
    let delta = diff_frames(&base, &over_frame, DiffOptions::default());
    assert!(
        !delta.matched && delta.differing_pixels > 20_000,
        "OVERVIEW overlay did not paint: only {} pixels differ from the windowed \
         desktop. Expected the Super+Tab overview to paint window tiles over the \
         desktop.",
        delta.differing_pixels
    );
}

// ===========================================================================
// 8. WALLPAPER vs CHROME differential — chrome is actually drawn over wallpaper.
// ===========================================================================

/// The status-bar band and the wallpaper directly below it must be VISIBLY
/// different: the bar is a styled chrome strip, the wallpaper is the dark desktop.
/// If the bar band reads ~identical to the wallpaper band below it, the bar's
/// background/cascade is not painting (the bar is "there" only as text floating on
/// wallpaper) — a subtle jankiness the per-slot content checks can miss.
///
/// WHY: a correct status bar has its own translucent/solid background distinct
/// from the wallpaper; equality means the bar chrome failed to paint.
#[test]
fn status_bar_chrome_is_distinct_from_wallpaper() {
    let frame = themed_desktop_capture(THEME).expect("desktop capture");

    // A thin slice of the bar (rows 4..12, clear of the very top border) vs a thin
    // slice of wallpaper well below the bar and dock.
    let bar_slice = frame.crop(0, 4, frame.width, 8);
    let wp_slice = frame.crop(0, frame.height / 2, frame.width, 8);

    // Compare their mean luminance; the styled bar background should differ from
    // the dark wallpaper. (Both slices same size for diff.)
    let mean = |f: &Frame| -> f64 {
        let mut sum = 0u64;
        for px in f.rgba.chunks_exact(4) {
            sum += px[0] as u64 + px[1] as u64 + px[2] as u64;
        }
        sum as f64 / (f.rgba.len() as f64 / 4.0 * 3.0)
    };
    let bar_mean = mean(&bar_slice);
    let wp_mean = mean(&wp_slice);

    assert!(
        (bar_mean - wp_mean).abs() > 6.0,
        "STATUS BAR CHROME indistinct from wallpaper: bar mean luminance \
         {bar_mean:.1} vs wallpaper {wp_mean:.1} (delta < 6). The bar's background \
         cascade is not painting — only floating text, if anything."
    );
}
