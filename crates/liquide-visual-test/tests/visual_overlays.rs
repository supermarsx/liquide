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
/// IGNORED — un-ignored by **t57-f9** (or the dedicated dialog f-slice) as its
/// acceptance gate. e1's finding: `request_message_dialog` sets
/// `chrome_active_dialog` (STATE wired) but no dom_sync dialog template paints,
/// so `dialog_open` returns the base desktop frame (verified: 0 changed pixels
/// vs baseline). When the dialog surface is wired, remove `#[ignore]` and bless
/// the `overlay_dialog_message_box` golden.
#[test]
fn dialog_message_box_paints() {
    let base = themed_desktop_capture(THEME).expect("baseline desktop capture");
    let dialog = dialog_open(THEME).expect("dialog capture");

    // The dialog is centred; examine the central region where it should appear.
    let dw = (dialog.width / 2).max(1);
    let dh = (dialog.height / 2).max(1);
    let dx = dialog.width / 4;
    let dy = dialog.height / 4;
    let before = base.crop(dx, dy, dw, dh);
    let after = dialog.crop(dx, dy, dw, dh);

    let delta = diff_frames(&before, &after, DiffOptions::default());
    assert!(
        !delta.matched && delta.differing_pixels > 1_000,
        "DIALOG DID NOT PAINT. Requesting a message-box produced only {} changed \
         pixels in the central region — expected a dialog (title + body + buttons) \
         to paint. Wire the dom_sync dialog template (chrome_active_dialog).",
        delta.differing_pixels
    );

    assert_golden("overlay_dialog_message_box", &after);
}

// ===========================================================================
// tooltip_shown — tooltip paints near the dock-item anchor.  [#[ignore] -> t57-f6]
// ===========================================================================

/// tooltip_shown: hovering a dock item shows a tooltip near the anchor.
///
/// History (resolved): e1/e3 found the single-frame builder could not elapse the
/// tooltip dwell. t57-gateclose FIXED the frame-timing in `scenarios::tooltip_shown`
/// (it bumps `frame_delta_ms` past the show-delay via the mutate seam) so the
/// canonical `TooltipManager` becomes visible and the shell emits a `<tooltip>`
/// overlay carrying the hovered item's label. At that point the tooltip still
/// rendered UNSTYLED at (0,0) because the `tooltip { position: fixed; ... }` rule
/// (and all `tooltip-content` / `tooltip-arrow` styling) lived ONLY in
/// `assets/themes/components.css`, which `DesktopCompositor::load_external_css`
/// never loaded.
///
/// t57-f6b wired `variables.css` + `components.css` into `load_external_css`'s
/// load chain (crates/liquide-session/src/desktop/mod.rs), so the tooltip now
/// picks up `position: fixed` + styling and paints near the dock anchor. This
/// test is therefore un-ignored: the differential tooth below proves the tooltip
/// paints a band of new pixels above the dock (not at (0,0)), and the
/// `overlay_tooltip` golden pins the result. See .orchestration/logs/t57-f6b.md.
#[test]
fn tooltip_paints_near_anchor() {
    let base = themed_desktop_capture(THEME).expect("baseline desktop capture");
    let hovered = tooltip_shown(THEME).expect("tooltip capture");

    // The tooltip anchors just above the hovered dock item (bottom-centre); crop
    // a band above the dock where the tooltip floats.
    let band_h = 140u32.min(hovered.height);
    let dock_top = hovered.height.saturating_sub(160);
    let before = base.crop(0, dock_top, hovered.width, band_h);
    let after = hovered.crop(0, dock_top, hovered.width, band_h);

    // DIFFERENTIAL TOOTH: the styled tooltip ("Files" label in a dark
    // `var(--tooltip-bg)` bubble + arrow) paints a localized cluster of new
    // pixels near the anchor band. Threshold calibrated against the REAL render
    // verified by t57-V (the tooltip bubble is short and its dark bg is
    // low-contrast over the already-dark wallpaper, so it changes ~210 px in this
    // band at the default per-channel tolerance of 4 — not the thousands a
    // high-contrast surface would). A tooltip that fails to paint, or that falls
    // back to the (0,0) origin (the pre-f6b unstyled bug), changes ~0 px here and
    // `matched` stays true, so this still has teeth against regression.
    let delta = diff_frames(&before, &after, DiffOptions::default());
    assert!(
        !delta.matched && delta.differing_pixels > 150,
        "TOOLTIP DID NOT PAINT NEAR THE ANCHOR. Hovering a dock item produced only \
         {} changed pixels in the band above the dock — expected the styled tooltip \
         bubble (components.css `tooltip{{position:fixed}}` + `tooltip-content`) to \
         paint here. If 0, the dwell/show path or the components.css load chain \
         regressed (chrome_tooltip / sync_tooltip_template / load_external_css).",
        delta.differing_pixels
    );

    assert_golden("overlay_tooltip", &after);
}
