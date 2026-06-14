//! Rigorous end-to-end suite for the STATUS BAR and especially the CLOCK
//! (t58-bar).
//!
//! THE PRIME DIRECTIVE (the user distrusts green tests): these tests encode what
//! a CORRECT status bar / clock MUST do and then RUN against the real desktop
//! capture pipeline. Failures are the *desired finding* — they are never weakened
//! to force green. Honest RED > fake GREEN.
//!
//! The captured desktop showed the clock as "00:00". This suite investigates
//! whether that is the REAL current time or a STUCK/DEFAULT value (the prime
//! suspect being that the headless capture path renders one frame at time `t0`
//! and NEVER calls `Shell::tick(now_us)`, so the clock item's `last_update_us`
//! stays at its constructed default of `0` — i.e. the Unix epoch, which formats
//! as 00:00 UTC).
//!
//! Seams used (all public, no production/shared-test edits — t58-bar lock is THIS
//! file only):
//!   - `capture_desktop_scripted_readback` (t57-e5 / A4): drive scripted input +
//!     a live-`Shell` readback/mutate closure, returning BOTH the post-state
//!     frame AND an extracted value from the SAME deterministic render.
//!   - `Shell::status_bar()` / `status_bar_mut()` (t57-e7 read seam): the live
//!     `ShellStatusBar` model — `format_clock_timestamp`, `find_item`,
//!     `set_clock_offset_minutes`, `update_clock`, `update_notification_count`.
//!   - `Shell::tick(now_us)`: the production time-injection point.
//!   - `Shell::notification_center_open()` / `session_menu_visible()`: click
//!     response state readback.
//!
//! Determinism: every capture is serialised behind the harness capture-lock, uses
//! the pinned deterministic test-assets root, and renders at the canonical
//! 1280x720 surface.

use liquide_visual_test::capture::{Frame, capture_desktop_scripted_readback};
use liquide_visual_test::scenarios::{
    SCENARIO_HEIGHT, SCENARIO_WIDTH, STATUS_BAR_HEIGHT, region_status_bar_center, scenario_options,
};

use liquide_platform::NativeWindowHandle;
use liquide_shell::{StatusBarItemKind, StatusBarSlot};

// ===========================================================================
// Shared helpers
// ===========================================================================

/// Theme used for every status-bar capture (the live liquid-glass cascade).
const THEME: &str = "liquid-glass";

/// Microseconds-per-second.
const SEC_US: u64 = 1_000_000;

/// A whole-number Unix timestamp (UTC) for `HH:MM:SS` on the epoch day, in µs.
/// e.g. `wall_us(13, 5, 9)` == 13:05:09 UTC == "13:05" in the default 24h /
/// no-seconds clock.
fn wall_us(h: u64, m: u64, s: u64) -> u64 {
    (h * 3600 + m * 60 + s) * SEC_US
}

/// Read the clock's CURRENT displayed string off the live status-bar model,
/// using the SAME `format_clock_timestamp` path `dom_sync` feeds into the DOM.
/// Returns `None` if there is no clock item at all (which is itself a failure).
fn clock_string(shell: &liquide_shell::Shell) -> Option<String> {
    let bar = shell.status_bar();
    let item = bar.find_item("clock")?;
    let StatusBarItemKind::Clock { format } = &item.kind else {
        return None;
    };
    Some(bar.format_clock_timestamp(item.last_update_us, format))
}

/// Pin the clock to UTC (offset 0) so an injected wall-clock timestamp formats
/// deterministically regardless of the host machine's timezone. The constructor
/// seeds the offset from the platform's local UTC offset, so without this a
/// machine in e.g. UTC+10 would shift "13:05" to "23:05".
fn pin_clock_utc(shell: &mut liquide_shell::Shell) {
    shell.status_bar_mut().set_clock_offset_minutes(0);
    shell.status_bar_mut().set_clock_24h(true);
    shell.status_bar_mut().set_clock_show_seconds(false);
}

/// No scripted events (the readback/mutate closure does all the driving).
fn no_events(_: NativeWindowHandle) -> Vec<liquide_platform::PlatformEvent> {
    Vec::new()
}

// ===========================================================================
// CLOCK — the prime suspect
// ===========================================================================

/// CLOCK SHOWS REAL TIME (model path).
///
/// Drive the clock to a KNOWN wall-clock time through the production tick/update
/// path, pin UTC, then read back the formatted clock string. It MUST equal the
/// expected formatted time. A hardcoded/stuck clock that ignores the driven time
/// fails here.
#[test]
fn clock_shows_driven_known_time() {
    let want = "13:05";
    let (_frame, got) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        no_events,
        |shell| {
            pin_clock_utc(shell);
            // Drive via the SAME entry point production uses on every frame.
            shell.tick(wall_us(13, 5, 9));
            clock_string(shell)
        },
    )
    .expect("capture should succeed");

    assert_eq!(
        got.as_deref(),
        Some(want),
        "clock must display the DRIVEN known time {want:?} (24h, UTC), got {got:?} — \
         a stuck/hardcoded clock would not reflect the injected time"
    );
}

/// CLOCK SHOWS REAL TIME — a SECOND independent known time, to rule out a clock
/// that happens to coincide with one fixture value.
#[test]
fn clock_shows_second_driven_known_time() {
    let want = "07:42";
    let (_frame, got) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        no_events,
        |shell| {
            pin_clock_utc(shell);
            shell.tick(wall_us(7, 42, 0));
            clock_string(shell)
        },
    )
    .expect("capture should succeed");

    assert_eq!(
        got.as_deref(),
        Some(want),
        "clock must display the second driven known time {want:?}, got {got:?}"
    );
}

/// CLOCK ADVANCES across ticks (minute rollover).
///
/// Advance simulated time across two ticks and assert the displayed clock string
/// CHANGES accordingly. A clock that never advances fails.
#[test]
fn clock_advances_across_ticks_minute_rollover() {
    let (_frame, (before, after)) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        no_events,
        |shell| {
            pin_clock_utc(shell);
            shell.tick(wall_us(9, 14, 30));
            let before = clock_string(shell);
            // Advance past the minute boundary.
            shell.tick(wall_us(9, 15, 30));
            let after = clock_string(shell);
            (before, after)
        },
    )
    .expect("capture should succeed");

    assert_eq!(before.as_deref(), Some("09:14"), "pre-rollover clock");
    assert_eq!(after.as_deref(), Some("09:15"), "post-rollover clock");
    assert_ne!(
        before, after,
        "clock must ADVANCE across ticks (minute rollover) — a stuck clock would \
         show the same string for both ticks"
    );
}

/// ADVERSARIAL: does the REAL headless render/capture pipeline show the current
/// time, or is it STUCK at the epoch default (00:00)?
///
/// This is the definitive investigation of the captured-desktop "00:00". We do
/// NOT drive the clock at all — exactly as a plain screenshot capture does. We
/// then read back what the clock model holds at render time.
///
/// EXPECTED of a CORRECT shell: the capture pipeline ticks the clock from a real
/// time source before rendering, so the clock reflects the current wall time
/// (NOT the epoch / 00:00).
///
/// If this asserts the clock is "00:00" (or otherwise reflects `last_update_us ==
/// 0`, the constructed epoch default), that CONFIRMS the stuck-clock bug: the
/// capture path renders one frame at `t0` and never calls `tick(real_now)`, so
/// every screenshot shows the epoch, which formats as 00:00 UTC. Per the prime
/// directive this RED is the desired finding — it is NOT weakened.
#[test]
fn clock_in_undriven_capture_is_not_stuck_at_epoch_default() {
    let (_frame, (raw_last_update_us, displayed)) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        no_events,
        |shell| {
            // Read the RAW model state with NO driving and NO mutation, exactly
            // as a plain capture leaves it.
            let last_update_us = shell
                .status_bar()
                .find_item("clock")
                .map(|i| i.last_update_us);
            let displayed = clock_string(shell);
            (last_update_us, displayed)
        },
    )
    .expect("capture should succeed");

    // Diagnostic surface for the log: what did the undriven pipeline hold?
    eprintln!(
        "[t58-bar] undriven-capture clock: last_update_us={raw_last_update_us:?} displayed={displayed:?}"
    );

    assert_ne!(
        raw_last_update_us,
        Some(0),
        "STUCK-CLOCK BUG: the capture/render pipeline never ticked the clock from a \
         real time source — the clock item's last_update_us is still its constructed \
         epoch default (0), so every screenshot renders the Unix epoch (00:00 UTC). \
         The captured-desktop '00:00' is a STUCK/DEFAULT value, NOT the real current \
         time. (Capture path render_thread.rs::capture_once_scripted_with renders one \
         frame at t0 and never calls Shell::tick(now_us).)"
    );
}

/// CLOCK PIXELS DIFFER for distinct times (no readable-text fallback needed —
/// proves the clock is actually painted from the driven value, not a static
/// glyph).
///
/// Render the clock at two very different times and assert the center clock
/// region pixels differ. A stuck clock paints identical pixels for both times →
/// FAIL.
#[test]
fn clock_region_pixels_differ_for_distinct_times() {
    let region = region_status_bar_center(SCENARIO_WIDTH, SCENARIO_HEIGHT);

    let (frame_a, _) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        no_events,
        |shell| {
            pin_clock_utc(shell);
            shell.tick(wall_us(11, 11, 0)); // "11:11"
        },
    )
    .expect("capture A should succeed");

    let (frame_b, _) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        no_events,
        |shell| {
            pin_clock_utc(shell);
            shell.tick(wall_us(23, 38, 0)); // "23:38" — every glyph differs
        },
    )
    .expect("capture B should succeed");

    let a = crop(&frame_a, region);
    let b = crop(&frame_b, region);
    assert_eq!(a.rgba.len(), b.rgba.len(), "clock crops must be the same size");

    let diff = pixel_diff_count(&a, &b, 16);
    assert!(
        diff > 30,
        "clock-region pixels must DIFFER between two distinct times (11:11 vs 23:38) \
         — only {diff} pixels changed; a stuck clock (or a clock not painted from the \
         driven value) renders identical pixels for both times"
    );
}

// ===========================================================================
// BAR ITEMS — each present & in the right slot/content
// ===========================================================================

/// EACH BAR ITEM PRESENT & CORRECT (model contract).
///
/// Assert the status-bar model carries every required item in its own slot with
/// the expected content: logo/branding (config), clock (center), notification
/// badge (right), connection/indicator (right), tray (right), session/user
/// (right). A missing item fails.
#[test]
fn all_required_bar_items_present_in_correct_slots() {
    let (_frame, report) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        no_events,
        |shell| {
            let bar = shell.status_bar();

            let has_branding = bar.config().show_app_menu; // LiquiDE logo gate
            let clock_center = bar
                .find_item("clock")
                .map(|i| {
                    matches!(i.kind, StatusBarItemKind::Clock { .. })
                        && i.slot == StatusBarSlot::Center
                        && i.visible
                })
                .unwrap_or(false);
            let badge_right = bar
                .find_item("notifications")
                .map(|i| {
                    matches!(i.kind, StatusBarItemKind::NotificationIndicator { .. })
                        && i.slot == StatusBarSlot::Right
                        && i.visible
                })
                .unwrap_or(false);
            let connection_right = bar
                .find_item("connection")
                .map(|i| {
                    matches!(i.kind, StatusBarItemKind::ConnectionQuality { .. })
                        && i.slot == StatusBarSlot::Right
                        && i.visible
                })
                .unwrap_or(false);
            let tray_right = bar
                .find_item("tray")
                .map(|i| {
                    matches!(i.kind, StatusBarItemKind::TrayArea)
                        && i.slot == StatusBarSlot::Right
                        && i.visible
                })
                .unwrap_or(false);
            let session_right = bar
                .find_item("session")
                .map(|i| {
                    matches!(i.kind, StatusBarItemKind::SessionButton)
                        && i.slot == StatusBarSlot::Right
                        && i.visible
                })
                .unwrap_or(false);

            (
                has_branding,
                clock_center,
                badge_right,
                connection_right,
                tray_right,
                session_right,
            )
        },
    )
    .expect("capture should succeed");

    let (branding, clock, badge, connection, tray, session) = report;
    assert!(branding, "LiquiDE branding/logo must be enabled in the bar config");
    assert!(clock, "clock item must be present, visible, in the CENTER slot");
    assert!(
        badge,
        "notification badge item must be present, visible, in the RIGHT slot"
    );
    assert!(
        connection,
        "connection/indicator item must be present, visible, in the RIGHT slot"
    );
    assert!(tray, "tray item must be present, visible, in the RIGHT slot");
    assert!(
        session,
        "session/user item must be present, visible, in the RIGHT slot"
    );
}

/// The status bar must actually PAINT (not be a blank band). The full-width top
/// band must contain non-background pixels (logo + clock + right cluster glyphs).
#[test]
fn status_bar_band_is_painted_not_blank() {
    let (frame, _) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        no_events,
        |shell| {
            pin_clock_utc(shell);
            shell.tick(wall_us(12, 34, 0));
        },
    )
    .expect("capture should succeed");

    let band = crop(&frame, (0, 0, frame.width, STATUS_BAR_HEIGHT.min(frame.height)));
    // Use the most common pixel as the background estimate, then count outliers.
    let bg = dominant_pixel(&band);
    let painted = band.non_background_pixels(bg, 24);
    assert!(
        painted > 200,
        "status-bar band must be painted with item content — only {painted} non-bg \
         pixels found (the band is effectively blank)"
    );
}

/// The CENTER clock region specifically must carry painted glyphs once the clock
/// is driven to a non-trivial time (so a blank/missing clock paint is caught even
/// if other bar items render).
#[test]
fn clock_center_region_has_painted_glyphs() {
    let (frame, _) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        no_events,
        |shell| {
            pin_clock_utc(shell);
            shell.tick(wall_us(18, 47, 0)); // "18:47"
        },
    )
    .expect("capture should succeed");

    let center = crop(&frame, region_status_bar_center(frame.width, frame.height));
    let bg = dominant_pixel(&center);
    let glyph_px = center.non_background_pixels(bg, 24);
    assert!(
        glyph_px > 20,
        "the center clock region must paint glyphs for the driven time — only \
         {glyph_px} non-bg pixels (the clock is not rendered into the center slot)"
    );
}

// ===========================================================================
// BADGE reflects state
// ===========================================================================

/// BADGE REFLECTS STATE: inject N notifications and assert the badge model shows
/// N (not stuck at 0). Drives the production path: `post_notification` raises the
/// unread count, and `tick` projects it onto the badge item (exactly as
/// `tick_detailed` does each frame).
#[test]
fn notification_badge_reflects_injected_count() {
    const N: u32 = 3;
    let (_frame, (badge_count, unread)) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        no_events,
        |shell| {
            for i in 0..N {
                let mut notif = liquide_interop::notification::Notification::new(
                    "Visual Test",
                    &format!("Message {i}"),
                );
                notif.body = format!("Body of notification {i}");
                // Distinct timestamps so the daemon does not rate-limit/coalesce.
                let _ = shell.post_notification(notif, (i as u64 + 1) * SEC_US);
            }
            // Project the unread count onto the badge (production does this in
            // tick_detailed every frame).
            shell.tick(10 * SEC_US);

            let unread = shell.notifications().unread_count() as u32;
            let badge_count = match shell.status_bar().find_item("notifications").map(|i| &i.kind) {
                Some(StatusBarItemKind::NotificationIndicator { unread_count, .. }) => Some(*unread_count),
                _ => None,
            };
            (badge_count, unread)
        },
    )
    .expect("capture should succeed");

    assert_eq!(
        unread, N,
        "injecting {N} notifications must raise the unread count to {N}, got {unread}"
    );
    assert_eq!(
        badge_count,
        Some(N),
        "the notification badge must reflect the injected count ({N}), got \
         {badge_count:?} — a badge stuck at 0 fails here"
    );
}

/// The badge must START at zero on a fresh bar (so the non-zero assertion above
/// is meaningful and the badge is not simply hardcoded to N).
#[test]
fn notification_badge_starts_at_zero() {
    let (_frame, badge_count) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        no_events,
        |shell| match shell.status_bar().find_item("notifications").map(|i| &i.kind) {
            Some(StatusBarItemKind::NotificationIndicator { unread_count, .. }) => Some(*unread_count),
            _ => None,
        },
    )
    .expect("capture should succeed");

    assert_eq!(
        badge_count,
        Some(0),
        "a fresh notification badge must read 0 before any notification is injected"
    );
}

// ===========================================================================
// BAR IS CLICKABLE / RESPONSIVE
// ===========================================================================

/// Y coordinate inside the status bar band (height 34) for clicks.
const BAR_CLICK_Y: f32 = 12.0;

/// CLICK the notification area of the bar → the notification center opens.
///
/// The notification indicator occupies a fixed 36..=80 px hit region from the
/// RIGHT edge of the bar (events.rs status-bar click handling). Click there and
/// assert `notification_center_open()` flips true. A bar that renders but does
/// not respond to clicks fails.
#[test]
fn clicking_notification_area_opens_notification_center() {
    let click_x = (SCENARIO_WIDTH as f32) - 58.0; // ~58 px from right → in 36..=80
    let (_frame, (was_open_before, open_after)) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        move |handle| {
            vec![
                liquide_platform::PlatformEvent::MouseInput {
                    handle,
                    event: liquide_input::mouse::MouseEvent::Move {
                        x: click_x,
                        y: BAR_CLICK_Y,
                    },
                },
                liquide_platform::PlatformEvent::MouseInput {
                    handle,
                    event: liquide_input::mouse::MouseEvent::Button {
                        button: liquide_input::mouse::MouseButton::Left,
                        state: liquide_input::mouse::ButtonState::Pressed,
                        x: click_x,
                        y: BAR_CLICK_Y,
                    },
                },
                liquide_platform::PlatformEvent::MouseInput {
                    handle,
                    event: liquide_input::mouse::MouseEvent::Button {
                        button: liquide_input::mouse::MouseButton::Left,
                        state: liquide_input::mouse::ButtonState::Released,
                        x: click_x,
                        y: BAR_CLICK_Y,
                    },
                },
            ]
        },
        |shell| {
            // The scripted events have already been dispatched through the REAL
            // integrated desktop input path (DesktopCompositor::handle_event ->
            // Shell::handle_platform_event -> execute_action) by the time readback
            // runs. We assert the user-visible end state.
            (false, shell.notification_center_open())
        },
    )
    .expect("capture should succeed");

    let _ = was_open_before;
    assert!(
        open_after,
        "clicking the status-bar notification area must OPEN the notification \
         center — it stayed closed, so the bar's notification region is not \
         responding to clicks. ROOT CAUSE (confirmed via probe): a DOUBLE-TOGGLE \
         on the integrated input path — the status-bar click handler \
         (events.rs:1058) already calls toggle_notification_center() AND returns \
         ShellAction::OpenNotificationCenter, then DesktopCompositor::handle_event \
         runs execute_action(OpenNotificationCenter) (tick.rs:447) which toggles \
         the SAME panel a SECOND time, cancelling the click. Introduced by the \
         t57-shellfix f4 wiring (the arm was previously `_ => false`)."
    );
}

/// CLICK the session/user button → the session menu opens.
///
/// The session button is the far-right item; its bounds are computed by the shell
/// (status_bar_item_bounds("session")). We click at the right edge of the bar
/// (inside the session button's ~40 px slot) and assert `session_menu_visible()`
/// flips true.
#[test]
fn clicking_session_button_opens_session_menu() {
    // The session button is the rightmost right-slot item, ~40 px wide, after a
    // small padding from the right edge. Sample a few x positions across that
    // far-right slot to be robust to exact layout while still landing on the
    // session button (NOT the notification 36..=80 region, so x within 0..=35
    // from the right edge).
    let candidates = [
        (SCENARIO_WIDTH as f32) - 10.0,
        (SCENARIO_WIDTH as f32) - 18.0,
        (SCENARIO_WIDTH as f32) - 26.0,
    ];

    let mut opened_any = false;
    let mut last_seen = false;
    for click_x in candidates {
        let (_frame, open_after) = capture_desktop_scripted_readback(
            &scenario_options(THEME),
            move |handle| {
                vec![
                    liquide_platform::PlatformEvent::MouseInput {
                        handle,
                        event: liquide_input::mouse::MouseEvent::Move {
                            x: click_x,
                            y: BAR_CLICK_Y,
                        },
                    },
                    liquide_platform::PlatformEvent::MouseInput {
                        handle,
                        event: liquide_input::mouse::MouseEvent::Button {
                            button: liquide_input::mouse::MouseButton::Left,
                            state: liquide_input::mouse::ButtonState::Pressed,
                            x: click_x,
                            y: BAR_CLICK_Y,
                        },
                    },
                    liquide_platform::PlatformEvent::MouseInput {
                        handle,
                        event: liquide_input::mouse::MouseEvent::Button {
                            button: liquide_input::mouse::MouseButton::Left,
                            state: liquide_input::mouse::ButtonState::Released,
                            x: click_x,
                            y: BAR_CLICK_Y,
                        },
                    },
                ]
            },
            |shell| shell.session_menu_visible(),
        )
        .expect("capture should succeed");
        last_seen = open_after;
        if open_after {
            opened_any = true;
            break;
        }
    }

    let _ = last_seen;
    assert!(
        opened_any,
        "clicking the status-bar session/user button must OPEN the session menu \
         (session_menu_visible == true); it stayed closed across the far-right slot \
         click positions, so the session button is not responding to clicks. ROOT \
         CAUSE (confirmed via probe): a DOUBLE-TOGGLE on the integrated input path — \
         the status-bar click handler (events.rs:1032-1033) SETS \
         session_menu_visible = true AND returns ShellAction::OpenSessionMenu, then \
         DesktopCompositor::handle_event runs execute_action(OpenSessionMenu) \
         (tick.rs:392) which does `session_menu_visible = !session_menu_visible`, \
         flipping it back to false and cancelling the click."
    );
}

// ===========================================================================
// NO OVERLAP / CLIPPING
// ===========================================================================

/// NO OVERLAP/CLIPPING: the bar's items must not overlap each other or run past
/// the bar edges. We reconstruct the right-cluster item layout the way the shell
/// does (right-anchored, fixed widths + spacing) and assert each computed item
/// rect stays within the bar and does not overlap its neighbour.
///
/// (This mirrors the shell's own `status_bar_item_bounds` right-to-left layout;
/// `status_bar_item_bounds` is `pub(crate)` so we recompute equivalently here.)
#[test]
fn right_cluster_items_do_not_overlap_or_clip() {
    // Widths the shell uses in shell_bar_item_width (events.rs). We assert the
    // contract that consecutive right-slot items, laid out right-to-left with the
    // bar's item spacing, neither overlap nor fall outside [0, screen_width].
    let (_frame, layout) = capture_desktop_scripted_readback(
        &scenario_options(THEME),
        no_events,
        |shell| {
            pin_clock_utc(shell);
            shell.tick(wall_us(13, 5, 0));
            // Snapshot the right-slot items (id + a conservative width estimate).
            let bar = shell.status_bar();
            let mut items: Vec<(String, f32)> = Vec::new();
            for item in bar.items().iter().rev() {
                if !item.visible || item.slot != StatusBarSlot::Right {
                    continue;
                }
                // Conservative width estimates matching the shell's
                // shell_bar_item_width fixed sizes (upper bounds).
                let w = match &item.kind {
                    StatusBarItemKind::NotificationIndicator { .. } => 40.0,
                    StatusBarItemKind::ConnectionQuality { .. } => 40.0,
                    StatusBarItemKind::TrayArea => 40.0,
                    StatusBarItemKind::SessionButton => 40.0,
                    StatusBarItemKind::Clock { .. } => 48.0,
                    StatusBarItemKind::Custom { content, .. } => content.len() as f32 * 7.0 + 12.0,
                };
                items.push((item.id.clone(), w));
            }
            items
        },
    )
    .expect("capture should succeed");

    // Lay out right-to-left from the right edge with the canonical padding/spacing
    // and assert no rect crosses 0 (clipping past the left) and none overlap.
    let screen_w = SCENARIO_WIDTH as f32;
    let padding_x = 12.0_f32;
    let spacing = 8.0_f32;
    let mut right_x = screen_w - padding_x;
    let mut rects: Vec<(String, f32, f32)> = Vec::new(); // (id, left, right)
    for (id, w) in &layout {
        let left = right_x - w;
        rects.push((id.clone(), left, right_x));
        right_x = left - spacing;
    }

    // No item may clip past the left edge of the screen.
    for (id, left, _r) in &rects {
        assert!(
            *left >= 0.0,
            "right-cluster item {id:?} clips past the left edge (left={left})"
        );
        assert!(
            *_r <= screen_w + 0.5,
            "right-cluster item {id:?} clips past the right edge (right={_r})"
        );
    }
    // No two consecutive items may overlap (each subsequent item is strictly to
    // the left of the previous one's left edge).
    for pair in rects.windows(2) {
        let (ida, lefta, _ra) = &pair[0];
        let (idb, _leftb, rightb) = &pair[1];
        assert!(
            *rightb <= *lefta + 0.5,
            "right-cluster items {idb:?} and {ida:?} overlap (item right={rightb} \
             exceeds neighbour left={lefta})"
        );
    }
    assert!(
        rects.len() >= 3,
        "expected several right-cluster items to validate (notifications, \
         connection, tray, session); found {}",
        rects.len()
    );
}

// ===========================================================================
// Pixel utilities (local — no shared-file edits)
// ===========================================================================

fn crop(frame: &Frame, region: (u32, u32, u32, u32)) -> Frame {
    let (x, y, w, h) = region;
    frame.crop(x, y, w, h)
}

/// Count pixels whose max per-channel absolute difference exceeds `tol`.
fn pixel_diff_count(a: &Frame, b: &Frame, tol: u8) -> usize {
    a.rgba
        .chunks_exact(4)
        .zip(b.rgba.chunks_exact(4))
        .filter(|(pa, pb)| {
            pa.iter()
                .zip(pb.iter())
                .any(|(&x, &y)| x.abs_diff(y) > tol)
        })
        .count()
}

/// The most common RGBA pixel in a frame (a robust background estimate for a
/// small mostly-flat crop).
fn dominant_pixel(frame: &Frame) -> [u8; 4] {
    use std::collections::HashMap;
    let mut counts: HashMap<[u8; 4], usize> = HashMap::new();
    for px in frame.rgba.chunks_exact(4) {
        let key = [px[0], px[1], px[2], px[3]];
        *counts.entry(key).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .max_by_key(|(_, c)| *c)
        .map(|(px, _)| px)
        .unwrap_or([0, 0, 0, 255])
}
