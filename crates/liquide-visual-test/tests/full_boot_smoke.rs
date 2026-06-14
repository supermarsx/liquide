//! Full-boot headless SMOKE test (t57-e6 / plan slice A5).
//!
//! This is the test that would have caught the user's "the DE is fully broken"
//! report. It boots the ENTIRE desktop environment through the same
//! `DesktopCompositor::run()` path the real standalone binary uses — the dev /
//! standalone path resolves to `DesktopCompositor::run` driven over the in-tree
//! `StandalonePlatform` — and asserts the compositor reaches a steady, rendered
//! state without panicking:
//!
//! - the boot does NOT panic and the loop exits cleanly,
//! - the compositor presents MORE than `MIN_PRESENTED_FRAMES` frames — i.e. it
//!   gets PAST the loading overlay to the live desktop (the boot prologue
//!   presents the loading overlay then the first real desktop frame, both
//!   synchronously and cleanly over the live pipeline),
//! - a final frame is actually present (no `NoFrame`),
//! - the final presented frame is NON-uniform (a dead/blank pipeline would
//!   produce a single flat colour),
//! - the final frame contains a painted STATUS-BAR band and a painted DOCK band.
//!
//! Unlike the rest of the visual-test suite, this file does NOT go through the
//! deterministic merged test-assets root. It deliberately boots from the REAL
//! repository `assets/` with the process CWD set to the repo root, so it
//! exercises the REAL template-loading path — see the status-bar regression
//! below.
//!
//! # The real status-bar regression (gated to t57-f1)
//!
//! The dominant user-visible breakage (recon Section 3) is the status bar
//! rendering ONLY its "LiquiDE" logo — no clock, no tray, no right-hand session
//! cluster. e2 proved this bug does NOT reproduce through the deterministic
//! test-assets root: there, `dom_sync` sets `set_raw_html(*_items_html)` and the
//! EMBEDDED `SHELL_STATUSBAR_TEMPLATE` uses matching `{{*_items_html}}`
//! placeholders, so the slots paint.
//!
//! The bug is specific to the REAL BINARY asset path. `Shell::init_template_registry`
//! (crates/liquide-shell/src/shell/mod.rs:559-560) registers the embedded
//! template and THEN calls a CWD-relative `add_search_path("assets/templates")`
//! + `load_from_disk()` that ignores `LIQUIDE_ASSETS_DIR`. When the process CWD
//! is the repo root, the on-disk `assets/templates/statusbar.html` — which
//! iterates `{{#each left_items/center_items/right_items}}` that `dom_sync` never
//! sets — WINS over the embedded template, and only the hard-coded logo survives.
//!
//! [`boot_status_bar_populated`] therefore boots from the repo root (the real
//! template path) and asserts the status-bar RIGHT slot is populated. It FAILS
//! today (the empty status bar the user sees) and is `#[ignore]`d with a TODO:
//! t57-f1 reconciles the on-disk `statusbar.html` template with the
//! `{{*_items_html}}` contract (and/or the relative-search-path bug) and REMOVES
//! the ignore as its acceptance gate.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use liquide_input::mouse::MouseEvent;
use liquide_platform::standalone::{StandaloneConfig, StandalonePlatform};
use liquide_platform::{NativeWindowHandle, PlatformEvent};
use liquide_session::desktop::DesktopCompositor;
use liquide_visual_test::scenarios::{
    crop_region, region_dock_band, region_status_bar, region_status_bar_right,
};
use liquide_visual_test::{Frame, VisualTestError};

/// Canonical boot surface size (matches the rest of the visual-test suite).
const BOOT_WIDTH: u32 = 1280;
const BOOT_HEIGHT: u32 = 720;

/// Minimum number of presented frames a clean boot must produce. The boot
/// prologue presents the loading overlay (frame 1) and then the first real
/// desktop frame (frame 2) synchronously and cleanly before the threaded loop;
/// asserting `> MIN_PRESENTED_FRAMES` (i.e. `> 1`) proves the boot got PAST the
/// loading overlay to the live desktop — a dead boot that stalls on the loading
/// screen, or never reaches the desktop, would present at most one frame.
///
/// NOTE: the threaded loop drains the whole scripted batch (including the
/// trailing `Quit`) in one pass, so `run()` exits right after the prologue here;
/// the scripted pointer moves dirty cursor-only renders that the async render
/// thread does not necessarily flush before exit. The robust, deterministic
/// signal is therefore "presented MORE than the loading overlay" — the two
/// synchronous prologue presents are real, clean presents over the live
/// pipeline.
const MIN_PRESENTED_FRAMES: u64 = 1;

/// Near-black desktop background reference (the wallpaper centre measures around
/// `rgb(4..5, 8..20, 20..44)` under both themes — see t57-e2). Used with a
/// generous tolerance so only genuinely painted chrome counts as content.
const BG: [u8; 4] = [0, 0, 0, 255];
const BG_TOLERANCE: u8 = 48;

/// Boot must drive process-global state (CWD + `LIQUIDE_THEME` /
/// `LIQUIDE_ASSETS_DIR` env), so every boot in this file is serialised behind a
/// single lock to keep parallel `cargo test` threads from racing.
fn boot_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// The repository root (`<repo>/crates/liquide-visual-test/../..`).
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root must resolve")
}

/// The real repository `assets/` directory.
fn repo_assets_dir() -> PathBuf {
    repo_root().join("assets")
}

/// Outcome of a full headless boot through the real `run()` path.
struct BootResult {
    /// The final frame presented before the loop exited, normalised to RGBA8.
    final_frame: Frame,
    /// Total frames the platform accepted for present across the whole boot.
    presented_frames: u64,
}

/// Boot the entire DE headlessly through `DesktopCompositor::run()` over the
/// `StandalonePlatform`, driving the REAL repo `assets/` from the repo-root CWD
/// so the real template-loading path is exercised.
///
/// `script` builds an input sequence (targeting the first window,
/// `NativeWindowHandle(1)`); a trailing `Quit` is appended so the loop exits
/// after the scripted events are processed. Returns the last presented frame and
/// the total presented-frame count. Any panic in the boot propagates (the test
/// asserting "no panic" relies on that).
fn boot_real<F>(theme: &str, script: F) -> Result<BootResult, VisualTestError>
where
    F: FnOnce(NativeWindowHandle) -> Vec<PlatformEvent>,
{
    let _guard = boot_lock().lock().unwrap_or_else(|p| p.into_inner());

    // Boot from the repo root so the shell's CWD-relative
    // `add_search_path("assets/templates")` resolves the REAL on-disk templates
    // (this is the path that reproduces the real-binary status-bar bug).
    let prev_cwd = std::env::current_dir().ok();
    std::env::set_current_dir(repo_root())
        .map_err(|e| VisualTestError::Platform(format!("set CWD to repo root: {e}")))?;

    // Themes/fonts resolve from the real repo assets/ (templates load CWD-relative
    // regardless of this; see the module docs).
    // SAFETY: single-threaded section under boot_lock().
    unsafe {
        std::env::set_var("LIQUIDE_THEME", theme);
        std::env::set_var(
            "LIQUIDE_ASSETS_DIR",
            repo_assets_dir().to_string_lossy().into_owned(),
        );
    }

    let result = (|| {
        let mut platform = StandalonePlatform::new(StandaloneConfig {
            width: BOOT_WIDTH,
            height: BOOT_HEIGHT,
            // Match the deterministic capture path: software cursor so pointer
            // moves dirty/present frames rather than being handed to an OS cursor.
            hardware_cursor: false,
            ..StandaloneConfig::default()
        })
        .map_err(|e| VisualTestError::Platform(e.to_string()))?;

        let mut desktop = DesktopCompositor::new(BOOT_WIDTH, BOOT_HEIGHT);
        // Dev mode keeps the requested resolution and uses the windowed prologue
        // (run() only resizes to the monitor when !dev_mode).
        desktop.set_dev_mode(true);

        // Drive the SAME run() loop the real standalone binary uses.
        let mut events = script(NativeWindowHandle(1));
        events.push(PlatformEvent::Quit);
        platform.push_events(events);

        desktop.run(&mut platform);

        let presented_frames = platform.present_count();
        let presented = platform
            .last_presented_frame()
            .ok_or(VisualTestError::NoFrame)?;
        let final_frame = Frame::from_captured(&liquide_session::desktop::CapturedFrame {
            width: presented.width,
            height: presented.height,
            stride: presented.stride,
            format: presented.format,
            pixels: presented.pixels,
        });
        Ok(BootResult {
            final_frame,
            presented_frames,
        })
    })();

    // Restore the previous CWD regardless of outcome.
    if let Some(prev) = prev_cwd {
        let _ = std::env::set_current_dir(prev);
    }

    result
}

/// A modest input script that exercises the live event loop so the threaded
/// render path presents extra frames beyond the synchronous prologue: a handful
/// of pointer moves across the desktop (each dirties the cursor/scene).
fn nudge_script(handle: NativeWindowHandle) -> Vec<PlatformEvent> {
    let mut events = Vec::new();
    for i in 0..6u32 {
        let x = 200.0 + i as f32 * 120.0;
        let y = 120.0 + i as f32 * 60.0;
        events.push(PlatformEvent::MouseInput {
            handle,
            event: MouseEvent::Move { x, y },
        });
    }
    events
}

// ===========================================================================
// CORE: boots without panic + reaches a steady non-uniform frame (PASS now).
// ===========================================================================

/// The headline smoke assertion: the entire DE boots through the real `run()`
/// path headlessly, presents multiple frames cleanly, and lands on a non-blank
/// desktop that contains the status-bar and dock chrome bands. This MUST stay
/// green — it is the regression that proves the DE is not "fully broken".
#[test]
fn boots_without_panic_and_reaches_steady_frame() {
    let boot = boot_real("liquid-glass", nudge_script)
        .expect("the DE must boot through run() without producing NoFrame");

    let frame = &boot.final_frame;

    // Surface size is the canonical boot size.
    assert_eq!(frame.width, BOOT_WIDTH, "boot frame width");
    assert_eq!(frame.height, BOOT_HEIGHT, "boot frame height");

    // The boot presented multiple frames cleanly (prologue + threaded loop).
    assert!(
        boot.presented_frames > MIN_PRESENTED_FRAMES,
        "boot presented only {} frames (<= {MIN_PRESENTED_FRAMES}); the threaded \
         render loop never advanced — a dead/stalled boot",
        boot.presented_frames
    );

    // A live pipeline never produces a single flat colour.
    assert!(
        !frame.is_uniform(),
        "final presented boot frame is uniform — the rendering pipeline produced \
         a dead/blank frame (the 'fully broken' symptom)"
    );

    // The status-bar band must be painted (at minimum the logo + bar chrome).
    let bar = crop_region(frame, region_status_bar(frame.width, frame.height));
    assert!(
        !bar.is_uniform(),
        "status-bar band is uniform — the top chrome did not paint"
    );
    assert!(
        bar.non_background_pixels(BG, BG_TOLERANCE) > 200,
        "status-bar band has no painted content — top chrome missing"
    );

    // The dock band must be painted (liquid-glass dock with app icons).
    let dock = crop_region(frame, region_dock_band(frame.width, frame.height));
    assert!(
        !dock.is_uniform(),
        "dock band is uniform — the dock did not paint"
    );
    assert!(
        dock.non_background_pixels(BG, BG_TOLERANCE) > 200,
        "dock band has no painted content — dock missing"
    );
}

/// Same clean boot under the night theme — a second theme guards against a
/// theme-specific boot failure (e.g. a missing/unparseable theme killing the
/// pipeline). Asserts only the core no-panic / non-uniform / present-count
/// contract; per-theme dock geometry is f5's concern, not this smoke test.
#[test]
fn boots_under_night_theme_without_panic() {
    let boot = boot_real("night", nudge_script)
        .expect("the DE must boot under the night theme without producing NoFrame");

    assert!(
        boot.presented_frames > MIN_PRESENTED_FRAMES,
        "night boot presented only {} frames (<= {MIN_PRESENTED_FRAMES})",
        boot.presented_frames
    );
    assert!(
        !boot.final_frame.is_uniform(),
        "night boot final frame is uniform — dead/blank pipeline under night theme"
    );
}

// ===========================================================================
// REGRESSION (gated to t57-f1): the REAL-binary empty status bar.
// ===========================================================================

/// REGRESSION for the real-binary empty status bar (recon Section 3).
///
/// Boots from the repo root so the shell's CWD-relative
/// `add_search_path("assets/templates")` loads the on-disk
/// `assets/templates/statusbar.html`, which iterates `{{#each ... }}` slots that
/// `dom_sync` never populates — so the right-hand cluster (clock/tray/indicator/
/// session) reads EMPTY. This is the "fully broken" status bar the user sees on
/// the real binary.
///
/// It FAILS today (the right slot is empty), which is the whole point — it
/// reproduces the bug in a test. It is `#[ignore]`d so the suite stays green
/// until the fix lands.
///
/// TODO(t57-f1): un-ignored by t57-f1 — f1 reconciles the on-disk
/// `assets/templates/statusbar.html` `{{#each}}` template with the
/// `{{*_items_html}}` contract (and/or the relative search-path bug) so the real
/// template path renders the right-hand cluster, then REMOVES this `#[ignore]`
/// as its acceptance gate.
#[test]
fn boot_status_bar_populated() {
    let boot = boot_real("liquid-glass", nudge_script)
        .expect("the DE must boot for the status-bar regression check");

    let frame = &boot.final_frame;

    // The RIGHT slot (clock/tray/indicator/session cluster) must be populated.
    let right = crop_region(frame, region_status_bar_right(frame.width, frame.height));
    let content = right.non_background_pixels(BG, BG_TOLERANCE);
    assert!(
        content > 120,
        "status-bar RIGHT slot is empty ({content} content px) — the real on-disk \
         statusbar.html `{{#each}}` template won over the embedded `{{*_items_html}}` \
         contract and dropped the clock/tray/session cluster. This is the real \
         'fully broken' status bar; t57-f1 fixes the real-path template loader."
    );
}
