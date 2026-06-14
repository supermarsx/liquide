//! E2E TEMPORAL / FLICKER stability suite (task t58, executor t58-flicker).
//!
//! ## What this suite encodes
//!
//! A correct desktop environment must be *temporally stable*: when nothing
//! changes, consecutive frames must be IDENTICAL; when something opens, it must
//! STAY open and settle (not appear-then-vanish, not re-layout/jump frame to
//! frame); the loading overlay must give way to a stable desktop without leaving
//! artifacts; and no real present after the first should be blank/half-rendered.
//!
//! These tests assert those properties directly. Per the prime directive: if the
//! DE thrashes/flickers, the test is SUPPOSED to go red — that red is the finding.
//! Nothing here is weakened to force green.
//!
//! ## HONESTY BOUNDARY — what the offscreen CPU path CANNOT cover (out of scope)
//!
//! This suite renders through the deterministic, single-threaded headless capture
//! path (`DesktopCompositor::capture_once*`) and reads back the CPU framebuffer.
//! It therefore tests **content / temporal stability of the rendered scene**:
//! does the produced pixel content stay stable across frames, and do transitions
//! settle cleanly.
//!
//! It CANNOT detect *true Win32 / RDP present-layer flicker*:
//!   - tearing (a flip mid-scanout),
//!   - double-buffer flash / a blank back-buffer briefly shown,
//!   - DWM / RDP redraw storms, present-vs-vsync races,
//!   - any artifact that lives between "the compositor produced correct pixels"
//!     and "the user's screen showed them".
//! Those are properties of the live present pipeline over a real display / RDP
//! session and need a LIVE on-screen test to observe. They are explicitly OUT OF
//! SCOPE here and are flagged, not silently assumed away.
//!
//! Additionally (seam limitation, reported to the coordinator): the
//! `StandalonePlatform` retains only the *last* presented frame
//! (`last_presented_frame()`) plus a `present_count()` — there is NO public hook
//! to inspect an arbitrary *mid-stream* presented frame. So "was any intermediate
//! present blank/partial?" cannot be answered frame-by-frame through the threaded
//! `run()` path with today's API. We test the strongest signal the API allows
//! (the final present is complete/non-blank, and the deterministic per-frame
//! render path never emits a blank/partial frame) and document the gap.

use liquide_visual_test::capture::{CaptureOptions, capture_desktop, capture_desktop_scripted};
use liquide_visual_test::diff::{DiffOptions, diff_frames};
use liquide_visual_test::scenarios::{
    crop_region, region_dock_band, region_launcher, region_status_bar, region_wallpaper,
    scenario_options,
};
use liquide_visual_test::{Frame, capture_desktop_scripted_with};

use liquide_platform::PlatformEvent;

/// Near-black desktop background reference + tolerance (matches t57-e2/e6: the
/// wallpaper centre measures ~rgb(4..5, 8..20, 20..44) under both themes).
const BG: [u8; 4] = [0, 0, 0, 255];
const BG_TOLERANCE: u8 = 48;

/// The default theme used across the suite.
const THEME: &str = "liquid-glass";

// ===========================================================================
// Helpers
// ===========================================================================

/// Exact byte-for-byte frame equality. Two frames of identical logical state,
/// both rendered at time `t0` on the single-threaded path, MUST be identical —
/// any difference is nondeterminism / content flicker.
fn frames_byte_identical(a: &Frame, b: &Frame) -> bool {
    a.width == b.width && a.height == b.height && a.rgba == b.rgba
}

/// Number of pixels that differ (beyond a small per-channel tolerance) between
/// two equally-sized frames/regions.
fn differing_pixels(a: &Frame, b: &Frame) -> usize {
    let opts = DiffOptions {
        per_channel_tolerance: 2,
        // We want the raw count regardless of pass/fail budget.
        max_differing_pixels: 0,
    };
    diff_frames(a, b, opts).differing_pixels
}

/// Render the default (no-action) desktop twice, deterministically.
fn capture_desktop_twice() -> (Frame, Frame) {
    let opts = scenario_options(THEME);
    let a = capture_desktop(&opts).expect("first desktop capture");
    let b = capture_desktop(&opts).expect("second desktop capture");
    (a, b)
}

/// Render the desktop with the launcher forced open via the shell mutate seam,
/// `repeat` times, returning all frames. The launcher is opened in EVERY render
/// (the capture path is stateless per call) so this models "the launcher stays
/// open across consecutive frames".
fn capture_launcher_open_repeated(repeat: usize) -> Vec<Frame> {
    let opts = scenario_options(THEME);
    (0..repeat)
        .map(|_| {
            capture_desktop_scripted_with(
                &opts,
                |_handle| Vec::<PlatformEvent>::new(),
                |shell| {
                    if !shell.launcher().is_visible() {
                        shell.launcher_mut().open();
                    }
                },
            )
            .expect("launcher-open capture")
        })
        .collect()
}

/// Render the desktop with one app window open, advancing the per-frame
/// animation delta by `delta_ms` each call so any unsettled animation would keep
/// moving. Returns the frame for each of the `deltas`.
fn capture_window_open_at_deltas(deltas: &[f32]) -> Vec<Frame> {
    let opts = scenario_options(THEME);
    deltas
        .iter()
        .map(|&d| {
            capture_desktop_scripted_with(
                &opts,
                |_handle| Vec::<PlatformEvent>::new(),
                move |shell| {
                    let _ = shell.open_app_window("com.liquide.files");
                    shell.set_frame_delta_ms(d);
                },
            )
            .expect("window-open capture")
        })
        .collect()
}

// ===========================================================================
// 1. STEADY-STATE STABILITY — identical state across frames must be IDENTICAL.
// ===========================================================================

/// The desktop, rendered twice at the same logical state with the pointer not
/// moved and time pinned at `t0`, must produce BYTE-IDENTICAL frames. Any region
/// that changes when nothing should change is content flicker.
///
/// This is the headline steady-state assertion. It is intentionally strict:
/// because the capture path does not advance the clock or move the cursor
/// between the two renders, there is NO legitimately-dynamic region (no clock
/// tick, no cursor move), so the WHOLE frame must match exactly.
#[test]
fn steady_state_desktop_is_byte_identical_across_frames() {
    let (a, b) = capture_desktop_twice();

    assert_eq!((a.width, a.height), (b.width, b.height), "frame size drift");
    assert!(
        frames_byte_identical(&a, &b),
        "desktop is NOT stable across two identical-state renders: {} pixels \
         differ (whole frame). At pinned time t0 with a stationary cursor there \
         is no legitimately-dynamic region — any difference is content flicker / \
         render nondeterminism.",
        differing_pixels(&a, &b)
    );
}

/// Per-chrome-region steady-state: even if some unexpected dynamic region snuck
/// in, the load-bearing chrome bands (status bar, dock, a chrome-free wallpaper
/// patch) must each be byte-stable across identical renders. This localises a
/// flicker to a specific surface if the whole-frame test ever regresses.
#[test]
fn steady_state_chrome_regions_are_stable() {
    let (a, b) = capture_desktop_twice();

    for (name, region) in [
        ("status-bar", region_status_bar(a.width, a.height)),
        ("dock", region_dock_band(a.width, a.height)),
        ("wallpaper", region_wallpaper(a.width, a.height)),
    ] {
        let ra = crop_region(&a, region);
        let rb = crop_region(&b, region);
        assert!(
            frames_byte_identical(&ra, &rb),
            "{name} region flickers across identical-state renders: {} pixels \
             differ — this surface re-renders unstably when nothing changed",
            differing_pixels(&ra, &rb)
        );
    }
}

/// TEETH PROOF for the steady-state tests: confirm the byte-identity check is
/// not vacuously green by inducing a REAL change (open the launcher in the
/// second render only) and asserting the comparator DOES see a difference. If
/// this ever passes-as-stable, the steady-state tests above are toothless.
#[test]
fn steady_state_comparator_has_teeth() {
    let opts = scenario_options(THEME);
    let base = capture_desktop(&opts).expect("base desktop");
    let changed = capture_desktop_scripted_with(
        &opts,
        |_handle| Vec::<PlatformEvent>::new(),
        |shell| {
            if !shell.launcher().is_visible() {
                shell.launcher_mut().open();
            }
        },
    )
    .expect("launcher-open desktop");

    let diff = differing_pixels(&base, &changed);
    assert!(
        diff > 500,
        "induced change (launcher opened) produced only {diff} differing pixels \
         — the steady-state comparator cannot see real changes, so the stability \
         tests would be vacuously green (no teeth)"
    );
}

// ===========================================================================
// 2. SETTLING — an opened surface STAYS visible and stable across frames.
// ===========================================================================

/// After opening the launcher it must STAY open and STABLE across consecutive
/// frames: it must not appear-then-vanish, and it must not re-layout/jump
/// between frames. We render the launcher-open desktop several times and assert
///   (a) the launcher region is painted in EVERY frame (never blanks/vanishes),
///   (b) every frame is byte-identical to the first (no jitter / re-layout).
#[test]
fn settling_open_launcher_stays_visible_and_stable() {
    let frames = capture_launcher_open_repeated(4);
    let first = &frames[0];

    // First, prove the launcher actually opened (the surface is present at all).
    let base = capture_desktop(&scenario_options(THEME)).expect("base desktop");
    let base_launcher = crop_region(&base, region_launcher(base.width, base.height));
    let open_launcher = crop_region(first, region_launcher(first.width, first.height));
    assert!(
        differing_pixels(&base_launcher, &open_launcher) > 200,
        "launcher did not actually open (region matches the closed desktop) — \
         cannot meaningfully test its settling"
    );

    for (i, f) in frames.iter().enumerate() {
        // (a) The launcher region must remain painted in every frame.
        let region = crop_region(f, region_launcher(f.width, f.height));
        assert!(
            region.non_background_pixels(BG, BG_TOLERANCE) > 200,
            "frame {i}: the open launcher VANISHED (its region went background) \
             — surface flickers in/out across frames"
        );
        // (b) No re-layout / jitter: every frame equals the first.
        assert!(
            frames_byte_identical(first, f),
            "frame {i}: the open launcher is NOT stable vs frame 0 ({} px differ) \
             — it re-layouts/jumps between frames (flicker)",
            differing_pixels(first, f)
        );
    }
}

/// After opening an app window it must STAY open and STABLE across consecutive
/// frames. Same contract as the launcher: present in every frame, byte-identical
/// frame to frame.
#[test]
fn settling_open_window_stays_visible_and_stable() {
    // Open the window at a FIXED frame delta in every render so we are testing
    // "the same settled state across frames", not animation progression.
    let frames = capture_window_open_at_deltas(&[16.67, 16.67, 16.67, 16.67]);
    let first = &frames[0];

    // The window must have actually opened (the desktop changed substantially).
    let base = capture_desktop(&scenario_options(THEME)).expect("base desktop");
    assert!(
        differing_pixels(&base, first) > 5_000,
        "app window did not actually open (frame ~= empty desktop) — cannot test \
         its settling"
    );

    for (i, f) in frames.iter().enumerate() {
        assert!(
            !f.is_uniform(),
            "frame {i}: window-open frame is uniform — pipeline produced a dead \
             frame mid-sequence (the window flickered out)"
        );
        assert!(
            frames_byte_identical(first, f),
            "frame {i}: the open window is NOT stable vs frame 0 ({} px differ) — \
             it re-layouts/jumps/flickers between frames",
            differing_pixels(first, f)
        );
    }
}

/// SETTLING UNDER ANIMATION ADVANCE: render the open-window desktop while
/// advancing the per-frame animation clock (`set_frame_delta_ms`) across a wide
/// range. A correctly-settling DE converges — once any open/transition animation
/// has elapsed, further time advance must NOT keep changing the content. The
/// later, well-past-any-transition frames must be stable relative to each other.
///
/// This catches an animation that never settles (oscillates / keeps thrashing
/// forever) — a temporal-instability the fixed-delta tests above cannot see.
#[test]
fn settling_window_converges_under_time_advance() {
    // Increasing per-frame deltas: 0.5s, 1s, 2s, 4s of elapsed animation time.
    // Any reasonable open/fade/slide transition settles well under 0.5s, so all
    // four should already be at the converged steady state.
    let frames = capture_window_open_at_deltas(&[500.0, 1000.0, 2000.0, 4000.0]);

    let reference = &frames[0];
    for (i, f) in frames.iter().enumerate().skip(1) {
        let diff = differing_pixels(reference, f);
        assert!(
            diff == 0,
            "after the transition window should have settled, advancing the \
             animation clock further STILL changes the frame: frame {i} differs \
             from the first post-transition frame by {diff} px — an animation/\
             content that never settles (perpetual flicker)"
        );
    }
}

// ===========================================================================
// 3. LOADING -> DESKTOP TRANSITION — loading gives way to a stable desktop.
// ===========================================================================

/// The boot prologue presents a loading overlay and THEN the first desktop
/// frame. The frame read back after `capture_once` (the post-loading desktop)
/// must be the DESKTOP, not the loading overlay, and must contain real chrome —
/// i.e. loading gave way cleanly with no leftover overlay artifact dominating
/// the frame.
#[test]
fn loading_gives_way_to_stable_desktop() {
    let opts = scenario_options(THEME);
    let desktop = capture_desktop(&opts).expect("desktop after loading prologue");

    // Not a dead/blank frame.
    assert!(
        !desktop.is_uniform(),
        "post-loading frame is uniform — the loading->desktop transition left a \
         blank/dead frame"
    );

    // Real chrome is present (status bar + dock painted): this is the desktop,
    // not a bare loading overlay.
    let bar = crop_region(&desktop, region_status_bar(desktop.width, desktop.height));
    let dock = crop_region(&desktop, region_dock_band(desktop.width, desktop.height));
    assert!(
        bar.non_background_pixels(BG, BG_TOLERANCE) > 200,
        "post-loading frame has no status-bar chrome — desktop did not take over \
         from the loading overlay"
    );
    assert!(
        dock.non_background_pixels(BG, BG_TOLERANCE) > 200,
        "post-loading frame has no dock chrome — desktop did not take over from \
         the loading overlay"
    );
}

/// The loading->desktop transition must be DETERMINISTIC and non-oscillating:
/// running the full prologue twice must land on the exact same desktop. An
/// unstable transition (settling differently each boot, or leaving variable
/// artifacts) would diverge here.
#[test]
fn loading_transition_is_deterministic() {
    let (a, b) = capture_desktop_twice();
    assert!(
        frames_byte_identical(&a, &b),
        "the loading->desktop transition is non-deterministic: two boots settle \
         to different desktops ({} px differ) — the transition oscillates / \
         leaves variable artifacts",
        differing_pixels(&a, &b)
    );
}

// ===========================================================================
// 4. NO PARTIAL / TORN / BLANK CONTENT after the first real present.
// ===========================================================================

/// The deterministic per-frame render path must never emit a blank or partial
/// frame: every readback frame is fully painted edge-to-edge. We assert no
/// fully-UNPAINTED ROW exists in the always-filled status-bar band (a
/// torn/partial present, or a zeroed back-buffer shown mid-flip, would leave
/// whole rows at the pristine clear colour), and the frame is non-uniform.
///
/// CALIBRATION (empirically characterised against the live render — see the
/// t58-flicker log): the liquid-glass status bar is a TRANSLUCENT dark fill
/// measuring ~`rgb(16, 19, 40)` across its FULL 36 px band height, including the
/// lower rows below the text baseline. That fill clears a small per-channel
/// tolerance (`ROW_PAINTED_TOLERANCE = 12`) on every row, while a genuinely
/// torn/blank row (a zeroed clear-colour back-buffer = pure black) reads ZERO.
/// Using the larger `BG_TOLERANCE = 48` here would FALSELY flag the legitimately
/// translucent lower bar rows as "blank" — that tolerance is calibrated to
/// separate BRIGHT chrome content from the near-black wallpaper, not to answer
/// "is this row painted at all". The smaller tolerance is the correct encoding
/// of "no torn/unpainted row" and still has teeth: a real blank row reads 0 even
/// at tolerance 0.
#[test]
fn no_partial_or_blank_frame_on_capture_path() {
    /// A row of the translucent bar fill clears this; a pristine/zeroed
    /// back-buffer row (pure black) does not.
    const ROW_PAINTED_TOLERANCE: u8 = 12;

    let opts = scenario_options(THEME);
    let frame = capture_desktop(&opts).expect("desktop capture");

    assert!(!frame.is_uniform(), "captured frame is entirely blank/uniform");

    // The status-bar band spans the full width and is painted edge-to-edge with
    // the (translucent) bar fill; if a present were torn/partial, some rows of
    // this always-painted band would be entirely the clear colour (unpainted).
    let bar = crop_region(&frame, region_status_bar(frame.width, frame.height));
    for row in 0..bar.height {
        let row_strip = bar.crop(0, row, bar.width, 1);
        assert!(
            row_strip.non_background_pixels(BG, ROW_PAINTED_TOLERANCE) > 0,
            "status-bar row {row} is entirely unpainted (pristine clear colour) \
             — a partial/torn frame (the always-painted status-bar band has a \
             blank row)"
        );
    }
}

/// Through the THREADED `run()` path (the real boot loop), the FINAL presented
/// frame must be a complete, non-blank desktop — never a half-rendered or blank
/// back-buffer left as the last present. Multiple frames must have been
/// presented (got past the loading overlay).
///
/// LIMITATION (documented in the module header): `StandalonePlatform` exposes
/// only the LAST presented frame, so we cannot inspect each intermediate present
/// for blankness through this path. This asserts the strongest available signal:
/// the final present is complete + non-blank and the boot advanced past loading.
#[test]
fn threaded_run_final_present_is_complete_not_blank() {
    let opts = CaptureOptions::default()
        .theme(THEME)
        .assets_dir(
            scenario_options(THEME)
                .assets_dir
                .expect("scenario assets dir"),
        );

    // A no-op script: just the trailing Quit appended by the harness. The
    // prologue still presents the loading overlay + first desktop frame.
    let frame = capture_desktop_scripted(&opts, |_handle| Vec::<PlatformEvent>::new())
        .expect("threaded run produced no final frame (NoFrame)");

    assert!(
        !frame.is_uniform(),
        "the FINAL present from the threaded run() loop is uniform/blank — the \
         boot ended on a dead/half-rendered back-buffer (present-pipeline flicker \
         symptom)"
    );

    // The final present must be a real desktop: status-bar + dock painted.
    let bar = crop_region(&frame, region_status_bar(frame.width, frame.height));
    let dock = crop_region(&frame, region_dock_band(frame.width, frame.height));
    assert!(
        bar.non_background_pixels(BG, BG_TOLERANCE) > 200
            && dock.non_background_pixels(BG, BG_TOLERANCE) > 200,
        "the final threaded-run present is missing chrome (status-bar or dock \
         blank) — the last frame shown is partial/incomplete"
    );
}
