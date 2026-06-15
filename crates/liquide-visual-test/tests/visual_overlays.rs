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
//! IGNORE GATES (un-ignored by the paired f-slice as ITS acceptance gate,
//! mirroring the t56-f4 menu pattern):
//!   - `dialog_message_box_paints`  -> t57-f9 (or the dialog f-slice)
//!   - `tooltip_paints_near_anchor` -> t57-f6
//! These surfaces are STATE-wired (the shell mutation lands) but do NOT paint
//! yet, so they are gated rather than blessed. Do not bless their goldens.

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
/// button — verified by inspecting the captured frame
/// (`target/visual-test/diag_dialog_full.png`).
///
/// REGION CORRECTION: the dialog is HORIZONTALLY centred but TOP-ANCHORED (the
/// known `%`-height vertical-centering limit in the overlay layout — the dialog
/// box sits at the top of the screen, ~y0..140, not vertically centred). The
/// previous central-half crop (`width/4, height/4, width/2, height/2`) therefore
/// missed the dialog entirely and read only ~332 changed pixels ("DIALOG DID NOT
/// PAINT"). That was a TEST-REGION bug, not a paint/wiring gap: the dialog does
/// paint, just above the old crop. We now crop the top-centre band where the
/// dialog actually anchors and verify it there.
///
/// PRODUCTION FOLLOW-UP (not a wiring gap, does not block this test): the dialog
/// should be VERTICALLY centred over the desktop. The overlay layout currently
/// top-anchors it (percentage-height centering limitation in `liquide-layout` /
/// the dialog overlay positioning). Tracked for the shell/layout owner.
#[test]
fn dialog_message_box_paints() {
    let base = themed_desktop_capture(THEME).expect("baseline desktop capture");
    let dialog = dialog_open(THEME).expect("dialog capture");

    assert_eq!(
        (base.width, base.height),
        (dialog.width, dialog.height),
        "baseline and dialog frames must share dimensions"
    );

    // The dialog is horizontally centred and TOP-ANCHORED: crop a top-centre band
    // (centre half of the width, top ~140 px) where the dialog box paints.
    let bw = (dialog.width / 2).max(1);
    let bx = dialog.width / 4;
    let by = 0u32;
    let bh = 140u32.min(dialog.height);
    let before = base.crop(bx, by, bw, bh);
    let after = dialog.crop(bx, by, bw, bh);

    // DIFFERENTIAL TOOTH: the dialog adds a large block of new pixels over the
    // bare desktop in this band (panel + title + body + button). A no-paint
    // regression (the pre-s3 state, or a broken sync_dialog_template) collapses
    // this far below threshold.
    let delta = diff_frames(&before, &after, DiffOptions::default());
    assert!(
        !delta.matched && delta.differing_pixels > 1_000,
        "DIALOG DID NOT PAINT. Requesting a message-box produced only {} changed \
         pixels in the top-centre region where the dialog anchors — expected a \
         dialog (title + body + buttons) to paint. Check the dom_sync dialog \
         template (sync_dialog_template / chrome_active_dialog). NOTE: the dialog \
         is top-anchored, not vertically centred (a known layout limit); if you \
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
/// STATUS (t66-harden): RED — KEPT RED on purpose; this is a REAL, still-broken
/// production gap, not a golden drift. **Do NOT bless `overlay_tooltip`.**
///
/// What works now: t65-s3 wired the dock `:hover` PSEUDO-state (`set_dock_hover`),
/// so the hovered dock ICON does change (the icon-swap repaints ~1.4k px). What
/// is STILL broken: the dock-hover TOOLTIP BUBBLE ("Files" label in a
/// `var(--tooltip-bg)` box) never surfaces on the capture render. Proven (t66-
/// harden, corroborated by `e2e_hover::diag_hover_paint_sweep`): in the bleed-free
/// band ABOVE the icon row — where ONLY the floating tooltip can paint — the
/// hovered-vs-base change is EXACTLY 0 px, and it stays 0 across every animation
/// delta swept 50 ms … 6000 ms (so it is not a dwell/timing miss — the tooltip
/// overlay is simply not emitted/painted).
///
/// The committed `overlay_tooltip.png` golden (blessed at f046183, before the
/// 6499a2d hover rework) shows a real "Files" bubble + a hover-highlight box; the
/// CURRENT render shows neither. Re-blessing it would bake the REGRESSION in, so
/// it is intentionally left mismatching and this test stays RED.
///
/// IMPORTANT — this test previously passed dishonestly: it cropped a band that
/// INCLUDED the dock icon row, so the icon hover-swap (~1.4k px) cleared the
/// `> 150` differential even though the tooltip painted nothing. The differential
/// below is now restricted to the bleed-free float band (above the icon tops), so
/// it fails for the RIGHT reason — the tooltip is genuinely absent.
///
/// PRODUCTION FOLLOW-UP (liquide-shell — outside the visual-test lock): emit/paint
/// the dock-hover tooltip overlay on the render path. The canonical
/// `TooltipManager` state is wired (`tooltip_adapter.rs`, driven from
/// `dom_sync.rs::sync_tooltip_template`), but its overlay does not reach the
/// painted scene on a steady dock hover. See `.orchestration/logs/t66-hover.md`
/// (ROOT CAUSE) and `.orchestration/logs/t66-harden.md`.
///
/// `#[ignore]`d (NOT blessed, NOT deleted) so the suite stays green while this
/// genuinely-broken surface is reported as a production gap. The assertion keeps
/// full teeth: remove `#[ignore]` once the tooltip-render gap is fixed, then
/// bless `overlay_tooltip` from the verified-correct render.
#[test]
#[ignore = "REAL GAP: dock-hover tooltip overlay never paints (liquide-shell render path); \
            0 px in the bleed-free float band at all deltas — see t66-hover.md / t66-harden.md. \
            Do NOT bless overlay_tooltip (would bake in the regression)."]
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
    // cluster of new pixels in the float band above the dock. Currently 0 — the
    // tooltip overlay is not painted (see doc comment). This stays RED until the
    // production tooltip-render gap is fixed; do NOT relax the band back over the
    // icon row to make it green (that would be fake-green via icon-swap bleed).
    let delta = diff_frames(&before, &after, DiffOptions::default());
    assert!(
        !delta.matched && delta.differing_pixels > 150,
        "TOOLTIP DID NOT PAINT NEAR THE ANCHOR. Hovering a dock item produced only \
         {} changed pixels in the FLOAT BAND above the dock icon (bleed-free of the \
         icon hover-swap) — expected the styled tooltip bubble (\"Files\" label in a \
         `var(--tooltip-bg)` box) to paint here. PROVEN 0 px across all animation \
         deltas, so this is the unwired dock-hover tooltip OVERLAY render path \
         (liquide-shell: emit the TooltipManager overlay into the painted scene; \
         see .orchestration/logs/t66-hover.md), NOT a dwell/timing miss.",
        delta.differing_pixels
    );

    assert_golden("overlay_tooltip", &after);
}
