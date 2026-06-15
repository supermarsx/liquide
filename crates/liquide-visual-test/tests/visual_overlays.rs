//! Per-surface visual-regression tests for OVERLAYS & DIALOGS (t57-e3 / plan A2).
//!
//! Companion to `visual_regression.rs` (top chrome) and `visual_windows.rs`
//! (window/session surfaces). Each test below drives the REAL headless
//! `DesktopCompositor` into a transient overlay state via the t57-e1 (A0)
//! scenario builders, asserts the surface paints CONTENT in its target region
//! (not the whole frame — recon Section 3's blind spot), and pins a golden so a
//! paint/z-order/cascade regression is caught.
//!
//! Goldens owned by THIS slice are namespaced `overlay_*` to avoid collisions
//! with the goldens owned by the other A1/A3 visual files.
//!
//! Bless goldens with:
//!   `BLESS=1 cargo test -p liquide-visual-test --test visual_overlays`
//! (or `LIQUIDE_UPDATE_GOLDEN=1`). Run without it to assert.
//!
//! TEETH: every non-ignored test asserts CONTENT in the surface's region AND a
//! golden. `notification_toast_paints` is the proven-teeth case — reverting the
//! toast paint (or the notification-area crop) collapses the region diff/non-bg
//! count below threshold and the test fails (see .orchestration/logs/t57-e3.md).
//!
//! IGNORE GATES — ALL RESOLVED. Both formerly-gated surfaces now paint and are
//! blessed:
//!   - `dialog_message_box_paints`  — RESOLVED (t65-s3 paint; t67-dialog
//!     vertical-centering; t69-effects2 shadow). Blessed by t69-harden.
//!   - `tooltip_paints_near_anchor` — RESOLVED (t67-tooltip scene overlay).
//!     Un-ignored + blessed by t69-harden (verified painting, 1248 px stable).
//! No overlay tests remain `#[ignore]`d.

use liquide_visual_test::diff::{DiffOptions, diff_frames};
use liquide_visual_test::golden::assert_golden;
use liquide_visual_test::scenarios::{
    context_menu_capture, crop_region, dialog_open, notification_center_open, notification_shown,
    region_notification_area, themed_desktop_capture, tooltip_shown,
};

/// The desktop background under liquid-glass is dark; use a black reference with
/// generous tolerance for the non-background content heuristics (matches the
/// convention in `visual_regression.rs`).
const BG_REFERENCE: [u8; 4] = [0, 0, 0, 255];
const BG_TOLERANCE: u8 = 24;

/// Canonical theme for the overlay goldens. Liquid-glass exercises the real
/// `liquid_glass.css` cascade (the overlays' background/blur/border come from
/// it), and is the theme e1's builders default to in their smoke tests.
const THEME: &str = "liquid-glass";

// ===========================================================================
// context_menu — complementary item-content tooth.
// ===========================================================================
//
// The right-click->menu *appears* gate already lives in
// `visual_regression.rs::context_menu_opens_on_right_click` (a CHANGED-pixels
// differential). To avoid duplicating that file/scenario, this slice adds a
// COMPLEMENTARY assertion from a different angle: the menu region must carry a
// substantial body of ITEM content (non-background pixels = the menu panel +
// row glyphs), and that region crop is pinned as its own golden. This catches a
// menu that opens but renders empty/itemless rows even if the changed-pixel
// count happened to clear the differential threshold.

/// context_menu: right-click opens a menu whose region carries real item
/// content (panel + rows), pinned as a golden. Complementary to the differential
/// gate in `visual_regression.rs`.
#[test]
fn context_menu_region_has_items() {
    let base = themed_desktop_capture(THEME).expect("baseline desktop capture");

    // Right-click at the desktop centre; the menu opens down-right of the cursor.
    let cx = (base.width / 2) as f32;
    let cy = (base.height / 2) as f32;
    let menu = context_menu_capture(THEME, cx, cy).expect("context-menu capture");

    // Crop the menu region (anchored at the cursor, generous to contain the
    // panel + several item rows).
    let rx = cx as u32;
    let ry = cy as u32;
    let rw = (menu.width - rx).min(220);
    let rh = (menu.height - ry).min(320);
    let menu_region = menu.crop(rx, ry, rw, rh);

    // CONTENT TOOTH: the opened menu region is full of item/panel pixels. An
    // empty/itemless menu (or one that failed to paint) falls far below this.
    let content = menu_region.non_background_pixels(BG_REFERENCE, BG_TOLERANCE);
    assert!(
        content > 4_000,
        "context-menu region has only {content} non-background pixels — the menu \
         opened but rendered no item content (check the `context-menu` item \
         template / row glyph paint)."
    );

    assert_golden("overlay_context_menu_items", &menu_region);
}

// ===========================================================================
// notification_shown — toast paints body + title in the notification area.
// ===========================================================================

/// notification_shown: an injected notification paints a toast (title + body) in
/// the top-right notification area. PASSES NOW (e1 noted the daemon is ticked;
/// verified: the toast adds ~10k changed pixels in the notification region).
///
/// TEETH (proven): reverting the toast paint or the notification-area crop drops
/// the region diff + non-bg count below threshold and this fails.
#[test]
fn notification_toast_paints() {
    let base = themed_desktop_capture(THEME).expect("baseline desktop capture");
    let shown = notification_shown(THEME).expect("notification capture");

    assert_eq!(
        (base.width, base.height),
        (shown.width, shown.height),
        "baseline and notification frames must share dimensions"
    );

    let region = region_notification_area(shown.width, shown.height);
    let before = crop_region(&base, region);
    let after = crop_region(&shown, region);

    // DIFFERENTIAL TOOTH: the toast adds a block of new pixels in the
    // notification area versus the no-notification baseline.
    let delta = diff_frames(&before, &after, DiffOptions::default());
    assert!(
        !delta.matched && delta.differing_pixels > 1_000,
        "NOTIFICATION TOAST DID NOT PAINT. Injecting a notification produced only \
         {} changed pixels in the notification area (threshold 1000). Expected the \
         toast (title + body) to paint there (check chrome_notification_server tick \
         + sync_notifications_template).",
        delta.differing_pixels
    );

    // CONTENT TOOTH: the toast carries real title/body glyph content.
    let content = after.non_background_pixels(BG_REFERENCE, BG_TOLERANCE);
    assert!(
        content > 1_000,
        "notification area has only {content} non-background pixels — the toast \
         body/title text is not rendering."
    );

    assert_golden("overlay_notification_toast", &after);
}

// ===========================================================================
// notification_center_open — panel paints.
// ===========================================================================

/// notification_center_open: toggling the notification center paints a panel that
/// is DISTINCT from a bare toast (the panel adds content beyond the single
/// toast). PASSES NOW.
///
/// e1's wiring note: `execute_action` drops `OpenNotificationCenter` (`_ =>
/// false`), so the action-routed path is dead; e1's builder instead toggles the
/// center via the shell's public `toggle_notification_center`, and the panel DOES
/// paint that way. This test therefore guards the panel paint (and implicitly
/// documents that the toggle API is wired even though the action arm is not — the
/// action arm is f4's concern, tracked by t57-f4 / the wiring audit A6).
#[test]
fn notification_center_panel_paints() {
    let base = themed_desktop_capture(THEME).expect("baseline desktop capture");
    let toast = notification_shown(THEME).expect("toast-only capture");
    let center = notification_center_open(THEME).expect("notification-center capture");

    assert_eq!(
        (base.width, base.height),
        (center.width, center.height),
        "baseline and center frames must share dimensions"
    );

    // TOOTH 1: the center frame differs from the bare-toast frame — i.e. a panel
    // (not just the toast) was added. If the toggle were a no-op these would be
    // identical.
    let vs_toast = diff_frames(&toast, &center, DiffOptions::default());
    assert!(
        !vs_toast.matched && vs_toast.differing_pixels > 500,
        "NOTIFICATION CENTER PANEL DID NOT PAINT. The center frame differs from the \
         bare-toast frame by only {} pixels (threshold 500) — toggling the center \
         added no panel content (check toggle_notification_center + \
         sync_notification_center_template).",
        vs_toast.differing_pixels
    );

    // TOOTH 2: the notification region carries substantial panel content.
    let region = region_notification_area(center.width, center.height);
    let region_after = crop_region(&center, region);
    let content = region_after.non_background_pixels(BG_REFERENCE, BG_TOLERANCE);
    assert!(
        content > 2_000,
        "notification-center region has only {content} non-background pixels — the \
         panel is not painting its body."
    );

    assert_golden("overlay_notification_center", &region_after);
}

// ===========================================================================
// dialog_open — message-box paints buttons + title.  [#[ignore] -> t57-f9]
// ===========================================================================

/// dialog_open: a requested message-box paints a dialog surface (title + body +
/// buttons) over the desktop.
///
/// RESOLVED (t66-harden). t65-s3 converted the dialog from the imperative
/// blank-rect path to the DOM/CSS template pipeline (`sync_dialog_template`,
/// driven by `chrome_active_dialog`), so a real themed dialog now paints WITH
/// text: a dark-slate rounded panel carrying the title ("Confirm action"), the
/// body message ("Are you sure you want to proceed?"), and a labelled "OK"
/// button — verified by inspecting the captured frame.
///
/// VERTICAL-CENTERING FIX (t67-dialog, this wave). The dialog is now BOTH
/// horizontally AND vertically centred over the desktop — the `liquide-layout`
/// flex engine previously failed to expand a single (nowrap) flex line to the
/// container's definite cross size, so `align-items:center` had no free space
/// and the dialog overlay stayed top-anchored (~y0..140). That layout gap is
/// fixed, so the dialog now anchors at the screen centre. VERIFIED by inspecting
/// the full-frame capture (t69-harden): the "Confirm action" panel sits centred
/// at ~(530..750, 293..453) on the 1280x720 surface, carrying its title, body,
/// the "OK" button, AND a blue drop-shadow halo beneath it (the t69-effects2
/// elevation now renders). The crop below therefore moves from the old
/// top-centre band back to the CENTRE band where the dialog now lands.
///
/// SHADOW (t69-effects2, this wave): the dialog's `box-shadow` literal (12px/40px
/// elevation) now renders a real drop-shadow halo under the panel — captured
/// inside the centre crop and baked into the re-blessed golden.
#[test]
fn dialog_message_box_paints() {
    let base = themed_desktop_capture(THEME).expect("baseline desktop capture");
    let dialog = dialog_open(THEME).expect("dialog capture");

    assert_eq!(
        (base.width, base.height),
        (dialog.width, dialog.height),
        "baseline and dialog frames must share dimensions"
    );

    // The dialog is now horizontally AND vertically centred (t67-dialog flex
    // fix): crop the CENTRE band (centre half of the width, centre half of the
    // height) where the dialog box + its drop-shadow now paint.
    let bw = (dialog.width / 2).max(1);
    let bx = dialog.width / 4;
    let bh = (dialog.height / 2).max(1);
    let by = dialog.height / 4;
    let before = base.crop(bx, by, bw, bh);
    let after = dialog.crop(bx, by, bw, bh);

    // DIFFERENTIAL TOOTH: the dialog adds a large block of new pixels over the
    // bare desktop in this band (panel + title + body + button + shadow). A
    // no-paint regression (the pre-s3 state, or a broken sync_dialog_template)
    // collapses this far below threshold.
    let delta = diff_frames(&before, &after, DiffOptions::default());
    assert!(
        !delta.matched && delta.differing_pixels > 1_000,
        "DIALOG DID NOT PAINT. Requesting a message-box produced only {} changed \
         pixels in the centre region where the dialog now anchors — expected a \
         dialog (title + body + buttons) to paint. Check the dom_sync dialog \
         template (sync_dialog_template / chrome_active_dialog) and the overlay \
         vertical-centering (liquide-layout flex single-line cross sizing). If you \
         see paint elsewhere, the dialog moved — re-crop to where it lands.",
        delta.differing_pixels
    );

    // CONTENT TOOTH: the dialog carries substantial panel + glyph content (title,
    // body, button label). The pre-s3 blank-rect dialog had NO text; this region
    // is full of non-background pixels now that text renders.
    let content = after.non_background_pixels(BG_REFERENCE, BG_TOLERANCE);
    assert!(
        content > 4_000,
        "dialog region has only {content} non-background pixels — the dialog panel \
         opened but its title/body/button text is not rendering."
    );

    assert_golden("overlay_dialog_message_box", &after);
}

// ===========================================================================
// tooltip_shown — tooltip paints near the dock-item anchor.  [#[ignore] -> t57-f6]
// ===========================================================================

/// tooltip_shown: hovering a dock item shows a tooltip near the anchor.
///
/// STATUS (t69-harden): RESOLVED — GREEN, un-ignored, golden re-blessed.
///
/// FIXED (t67-tooltip, this wave). The dock-hover tooltip bubble is now emitted
/// as a manual scene overlay on the render path (`liquide-shell` `scene.rs::
/// add_tooltip_overlay`, gated on `tooltip_manager_visible()`), mirroring the
/// overview/lockscreen overlays. The canonical `TooltipManager` state was always
/// wired (`tooltip_adapter.rs` / `dom_sync.rs::sync_tooltip_template`), but the
/// CSS pipeline produced ZERO paintable scene nodes for the fixed-position
/// `<tooltip>` block (no `display`/width → it collapsed); the scene overlay is
/// now the authoritative painter. The adapter also uses
/// `display_duration_ms: 0` so a steady hover does not auto-hide.
///
/// VERIFIED (t69-harden): inspected the full-frame hover capture — the "Files"
/// label bubble now paints in the bleed-free float band ABOVE the first dock
/// icon (the previously-absent surface). Corroborated by
/// `e2e_hover::diag_hover_paint_sweep` (t67-tooltip): the float-band change went
/// from EXACTLY 0 px (all deltas) to **1248 px, stable at every delta 510–6000
/// ms** (and correctly 0 px below the 500 ms show-delay).
///
/// The differential tooth below keeps FULL teeth: it is restricted to the
/// bleed-free float band ABOVE the icon row (so the dock icon hover-swap cannot
/// leak in and mask an absent tooltip). It cleared `0 px` while the tooltip was
/// broken and now clears the now-painting bubble — do NOT relax the band back
/// over the icon row.
#[test]
fn tooltip_paints_near_anchor() {
    let base = themed_desktop_capture(THEME).expect("baseline desktop capture");
    let hovered = tooltip_shown(THEME).expect("tooltip capture");

    assert_eq!(
        (base.width, base.height),
        (hovered.width, hovered.height),
        "baseline and hovered frames must share dimensions"
    );

    // The first dock item sits at ~(544, 668, 48, 48) on the 1280x720 surface; the
    // tooltip floats ABOVE it. Crop ONLY the float band above the icon tops so the
    // icon hover-swap cannot leak in and mask an absent tooltip. (The previous
    // band reached down into the icon row and the icon swap masked the missing
    // tooltip — see this test's doc comment.)
    let icon_top = hovered.height.saturating_sub(52); // ~668 on a 720-tall frame
    let float_h = 60u32.min(icon_top); // band y ~600..660, strictly above the icon
    let float_top = icon_top.saturating_sub(float_h);
    let before = base.crop(0, float_top, hovered.width, float_h);
    let after = hovered.crop(0, float_top, hovered.width, float_h);

    // DIFFERENTIAL TOOTH (bleed-free): the styled tooltip bubble must paint a
    // cluster of new pixels in the float band above the dock. Now ~1248 px (was
    // 0 px before t67-tooltip wired the scene overlay). Do NOT relax the band
    // back over the icon row (that would be fake-green via icon-swap bleed).
    let delta = diff_frames(&before, &after, DiffOptions::default());
    assert!(
        !delta.matched && delta.differing_pixels > 150,
        "TOOLTIP DID NOT PAINT NEAR THE ANCHOR. Hovering a dock item produced only \
         {} changed pixels in the FLOAT BAND above the dock icon (bleed-free of the \
         icon hover-swap) — expected the styled tooltip bubble (\"Files\" label) to \
         paint here. This regressed the t67-tooltip scene overlay (liquide-shell: \
         `scene.rs::add_tooltip_overlay`, gated on `tooltip_manager_visible()`); it \
         is NOT a dwell/timing miss (the bubble is delta-stable 510–6000 ms).",
        delta.differing_pixels
    );

    assert_golden("overlay_tooltip", &after);
}
