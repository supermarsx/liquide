//! Runtime WIRING-AUDIT (t57-e7, plan slice A6).
//!
//! This is the test that makes *un-wiring* fail CI. It boots the real desktop
//! environment headlessly (the same `DesktopCompositor` + `StandalonePlatform`
//! path the visual goldens use), performs a set of representative interactions,
//! and then asserts which canonical managers ran their **live** drive path this
//! session via [`liquide_shell::Shell::wiring_report`].
//!
//! ## How the audit works
//!
//! Each canonical manager / chrome adapter sets a [`WiringBit`] in the shell the
//! first time it runs its live (non-test) drive path — the status bar / dock /
//! launcher / context menu set theirs from the per-frame `sync_dom` render path;
//! the window-class/groups/tree/effects, tiling, notification daemon, tooltip,
//! lock screen, and workspace managers set theirs from their interaction paths.
//! The bits never feed back into behavior; they are a read-only audit channel.
//!
//! ## The tracked contract
//!
//! Managers that a representative interaction *does* drive are asserted DRIVEN.
//! Managers that have no live runtime consumer yet (or whose live consumer is a
//! not-yet-wired interaction) are listed in [`ALLOWLIST`] with the owning
//! f-slice. The DRIVEN set is therefore a contract: if a future change removes a
//! live consumer of a DRIVEN manager, its bit stops flipping and this test goes
//! RED — exactly the regression the user asked us to guard against. When an
//! f-slice wires an allowlisted manager, it moves the entry from [`ALLOWLIST`]
//! to [`EXPECTED_DRIVEN`] (its acceptance gate), so the allowlist can only
//! shrink.

use std::cell::Cell;

use liquide_input::{ButtonState, MouseButton, MouseEvent};
use liquide_platform::PlatformEvent;
use liquide_shell::shortcuts::ShellAction;
use liquide_shell::{WiringBit, WiringReport};
use liquide_visual_test::{capture_desktop_scripted_with, scenario_options};

/// Managers that the representative boot + interactions MUST drive. Removing a
/// live consumer of any of these flips its bit off and fails this test.
const EXPECTED_DRIVEN: &[WiringBit] = &[
    WiringBit::StatusBar,          // sync_dom render path (clock/tray slots)
    WiringBit::Dock,               // sync_dom render path (liquid-glass dock)
    WiringBit::Launcher,           // OpenLauncher -> launcher visible -> sync_dom
    WiringBit::ContextMenu,        // right-click on desktop -> menu visible -> sync_dom
    WiringBit::NotificationServer, // post_notification -> canonical daemon
    WiringBit::WindowClass,        // open_app_window -> register_window_chrome
    WiringBit::WindowGroups,       // open_app_window -> register_window_chrome
    WiringBit::WindowTree,         // open_app_window -> register_window_tree
    WiringBit::WindowEffects,      // open_app_window -> register_window_tree
    WiringBit::LockScreen,         // LockSession action -> canonical lock screen
    WiringBit::Workspace,          // WorkspaceAdd + switch -> canonical switch
    WiringBit::Tooltip,            // dock-item hover + dwell -> TooltipManager visible (f6b)
    WiringBit::ShellServices,      // open_app_window -> plan_app_launch consults the registry
];

/// Managers that are CONSTRUCTED in the codebase but have **no live runtime
/// consumer reachable by a representative interaction yet** — a tracked,
/// owner-stamped backlog. Each entry names the f-slice that will wire it; when
/// that slice lands its live drive path, it moves the manager into
/// [`EXPECTED_DRIVEN`] (its acceptance gate) and deletes the allowlist line.
///
/// Seeded from the e2/e3/e4 verified-dead findings (state.md / t57-e1 log):
const ALLOWLIST: &[(WiringBit, &str)] = &[
    // t57-f10 added a LIVE consumer of the canonical tiling engine on the drag
    // path: `apply_snap_on_release` now calls `canonical_tiling().add_window()`,
    // so a drag-to-snap gesture genuinely drives `liquide_tiling::TilingEngine`
    // (proven by `interaction_e2e::drag_to_edge_snaps_window`, which PASSES). The
    // Tiling bit therefore CAN flip via a real interaction — but this audit's
    // `boot_and_drive` cannot observe it: it drives a single render after a
    // right-click (so the ContextMenu bit can flip while the menu is visible),
    // and a post-window-open title-bar drag-to-edge in the same script would
    // dismiss that menu before the render (breaking the ContextMenu DRIVEN
    // assertion). Splitting the audit into two boots to drive both is out of the
    // gate-closer's scope; the snap behavior is fully regression-guarded by the
    // e5 interaction test, so Tiling stays allowlisted here with this note.
    (
        WiringBit::Tiling,
        "f10 DONE: live consumer in apply_snap_on_release (canonical_tiling().add_window); \
         driven+guarded by interaction_e2e::drag_to_edge_snaps_window. This single-render \
         audit cannot drive a post-menu drag without dismissing the context menu.",
    ),
];

/// Other audited contracts that are NOT manager bits but were proven dead by
/// e1/e2/e3/e4 and are owned by an f-slice. Recorded here so the allowlist is a
/// complete, owner-stamped backlog of known-dormant wiring (the audit asserts
/// these *stay* dead until their slice lands; see `dead_action_arms_documented`).
///
/// t57-gateclose trimmed the now-wired entries (the allowlist can only shrink):
/// - `TaskOverview` / `WorkspaceOverview` — WIRED by f-overview (execute_action
///   arms toggle `overview_visible` + emit the overview overlay scene; proven by
///   `visual_windows::overview_paints_tiles`). Removed.
/// - `OpenNotificationCenter` — WIRED by f4 (execute_action arm calls
///   `toggle_notification_center`). Removed.
/// - `dialog_paint(chrome_active_dialog)` — WIRED by f9 (DialogContent retained +
///   `add_dialog_overlay` paints; proven by `visual_overlays::dialog_message_box_paints`).
///   Removed.
///
/// t177-warnings-deadcode wired the last remaining entry:
/// - `shell_services(chrome_shell_services)` — WIRED: `Shell::open_app_window`
///   now consults the canonical registry via `plan_app_launch`, caching it in
///   `chrome_shell_services` and flipping the `ShellServices` wiring bit. Moved
///   into [`EXPECTED_DRIVEN`]; the cargo `never read` warning is gone.
///
/// No documented dead arms remain; the list is kept (empty) as the owner-stamped
/// home for any future known-dormant non-manager wiring.
const DEAD_ACTION_ARMS: &[(&str, &str)] = &[];

/// Boot the DE headlessly, perform representative interactions, and return the
/// post-render wiring report.
///
/// `script` runs a right-click on the empty desktop (opens the context menu);
/// `mutate` performs the remaining interactions through the shell's public API,
/// then drives one live render sync and reads the report back through a cell.
fn boot_and_drive() -> WiringReport {
    let report: Cell<WiringReport> = Cell::new(WiringReport::default());

    let opts = scenario_options("liquid-glass");

    // Right-click on an empty desktop region (top-left, away from dock/bar) so
    // the context menu opens. Events are dispatched BEFORE `mutate`, while the
    // desktop has no windows, so the click lands on empty desktop.
    let script = |handle| -> Vec<PlatformEvent> {
        let (x, y) = (200.0_f32, 200.0_f32);
        // Dock-item hover anchor (matches `scenarios::tooltip_shown`): bottom-
        // centre, just left of centre, ~28 px above the bottom edge. Hovering it
        // sets the live `tooltip_text` / `tooltip_pos` hover state (events.rs).
        // A pointer MOVE (not a click) does NOT dismiss the context menu opened
        // above, so the ContextMenu DRIVEN assertion is preserved.
        let (dx, dy) = (
            liquide_visual_test::scenarios::SCENARIO_WIDTH as f32 / 2.0 - 80.0,
            liquide_visual_test::scenarios::SCENARIO_HEIGHT as f32 - 28.0,
        );
        vec![
            PlatformEvent::MouseInput {
                handle,
                event: MouseEvent::Move { x, y },
            },
            PlatformEvent::MouseInput {
                handle,
                event: MouseEvent::Button {
                    button: MouseButton::Right,
                    state: ButtonState::Pressed,
                    x,
                    y,
                },
            },
            PlatformEvent::MouseInput {
                handle,
                event: MouseEvent::Button {
                    button: MouseButton::Right,
                    state: ButtonState::Released,
                    x,
                    y,
                },
            },
            // Hover the first dock item to set the tooltip hover state.
            PlatformEvent::MouseInput {
                handle,
                event: MouseEvent::Move { x: dx, y: dy },
            },
        ]
    };

    capture_desktop_scripted_with(&opts, script, |shell| {
        // 1. Tick the clock — drives the status bar update path.
        shell.tick(16_000);

        // 2. Open an application window — drives window class/groups/tree/effects.
        shell.open_app_window("com.liquide.files");

        // 3. Post a notification — drives the canonical notification daemon.
        let mut notif =
            liquide_interop::notification::Notification::new("Wiring Audit", "live consumer check");
        notif.body = "audit notification".to_string();
        let _ = shell.post_notification(notif, 16_000);

        // 4. Toggle the notification center (panel state).
        if !shell.notification_center_open() {
            shell.toggle_notification_center();
        }

        // 5. Open the launcher overlay so its render path runs this frame.
        shell.execute_action(&ShellAction::OpenLauncher);

        // 6. Lock the session — drives the canonical lock-screen state machine.
        shell.execute_action(&ShellAction::LockSession);

        // 7. Add a workspace and switch — drives a REAL canonical workspace
        //    switch (needs >= 2 workspaces to actually change the active one).
        shell.execute_action(&ShellAction::WorkspaceAdd);
        let _ = shell.workspace_manager(); // read-only touch (keeps intent explicit)
        shell.execute_action(&ShellAction::WorkspaceNext);

        // 8. The dock-item hover was driven in the `script` phase (sets the live
        //    `tooltip_text` / `tooltip_pos`). Advance the per-frame delta past the
        //    tooltip dwell (show_delay 500 + fade_in 150, < display_duration 5000)
        //    so the canonical TooltipManager progresses to Visible on the final
        //    render below. After f6b wired components.css the tooltip is styled +
        //    `position: fixed`, so the manager-visible path runs and the Tooltip
        //    bit flips (proven end-to-end by
        //    `visual_overlays::tooltip_paints_near_anchor`).
        shell.set_frame_delta_ms(800.0);

        // Drive one live DOM render sync (the same `sync_dom` the compositor runs
        // every frame) so the render-path bits — status bar, dock, launcher,
        // context menu, tooltip — are recorded, then read the report back.
        report.set(shell.wiring_report_after_sync());
    })
    .expect("headless boot + interaction capture must succeed");

    report.get()
}

/// The representative boot + interactions must drive every manager in
/// [`EXPECTED_DRIVEN`]. If a live consumer is removed, the bit stops flipping
/// and this assertion fails — the un-wiring guard.
#[test]
fn expected_managers_are_driven() {
    let report = boot_and_drive();

    let missing: Vec<&str> = EXPECTED_DRIVEN
        .iter()
        .filter(|b| !report.is_driven(**b))
        .map(|b| b.name())
        .collect();

    assert!(
        missing.is_empty(),
        "audited managers expected to be DRIVEN but were not (a live consumer was \
         removed / un-wired): {missing:?}\n\
         full driven set: {:?}\n\
         not driven: {:?}",
        report.driven().iter().map(|b| b.name()).collect::<Vec<_>>(),
        report
            .not_driven()
            .iter()
            .map(|b| b.name())
            .collect::<Vec<_>>(),
    );
}

/// Every manager that is NOT driven must be on the [`ALLOWLIST`] with an owning
/// f-slice. A new dormant manager (or one that lost its consumer but is not
/// covered by `EXPECTED_DRIVEN`) shows up here and fails until it is either
/// wired or explicitly allowlisted with an owner — no silent dormancy.
#[test]
fn not_driven_managers_are_all_allowlisted() {
    let report = boot_and_drive();

    let unjustified: Vec<&str> = report
        .not_driven()
        .iter()
        .filter(|b| !ALLOWLIST.iter().any(|(allowed, _)| allowed == *b))
        .map(|b| b.name())
        .collect();

    assert!(
        unjustified.is_empty(),
        "managers are NOT driven and NOT on the allowlist (wire them or add an \
         allowlist entry with an owning f-slice): {unjustified:?}",
    );
}

/// The allowlist must stay honest: every allowlisted manager must genuinely be
/// NOT driven. Once an f-slice wires a manager, its bit flips on and this test
/// fails — forcing the slice to move the entry into [`EXPECTED_DRIVEN`] (its
/// acceptance gate) and delete the allowlist line. The allowlist can only shrink.
#[test]
fn allowlist_entries_are_genuinely_dormant() {
    let report = boot_and_drive();

    let now_wired: Vec<&str> = ALLOWLIST
        .iter()
        .filter(|(bit, _)| report.is_driven(*bit))
        .map(|(bit, _)| bit.name())
        .collect();

    assert!(
        now_wired.is_empty(),
        "allowlisted managers are now DRIVEN — their f-slice landed wiring; move \
         them from ALLOWLIST into EXPECTED_DRIVEN (acceptance gate): {now_wired:?}",
    );
}

/// The DRIVEN set and the ALLOWLIST must together cover every audited manager
/// exactly once (no manager forgotten, none double-counted). Keeps the contract
/// total as managers are added.
#[test]
fn driven_and_allowlist_partition_all_managers() {
    for bit in WiringBit::ALL {
        let in_expected = EXPECTED_DRIVEN.contains(&bit);
        let in_allowlist = ALLOWLIST.iter().any(|(b, _)| *b == bit);
        assert!(
            in_expected ^ in_allowlist,
            "manager {:?} must be in EXACTLY ONE of EXPECTED_DRIVEN / ALLOWLIST \
             (expected={in_expected}, allowlist={in_allowlist})",
            bit.name(),
        );
    }
}

/// Documents the known-dead non-manager action arms / fields (verified by
/// e1/e2/e3/e4) so the backlog is owner-stamped in one place. Pure
/// documentation guard: it asserts each dead arm carries a non-empty owner so
/// the list cannot rot into anonymity.
#[test]
fn dead_action_arms_documented() {
    for (arm, owner) in DEAD_ACTION_ARMS {
        assert!(
            !arm.is_empty() && !owner.is_empty(),
            "every documented dead action arm must name an owning f-slice",
        );
    }
}
