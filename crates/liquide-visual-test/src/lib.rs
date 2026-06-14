//! Headless deterministic capture + golden-image regression harness for the
//! Liquide desktop environment.
//!
//! This crate exists to close the test-vs-runtime gap (the old app-harness
//! presented a zero-filled buffer and the e2e suite only asserted frame size,
//! so "all tests passed" over an empty frame). It drives the REAL
//! [`DesktopCompositor`](liquide_session::desktop::DesktopCompositor) headlessly
//! through the in-tree [`StandalonePlatform`] backend, reads back the actual
//! CPU-rasterised pixels, and compares them against committed golden PNGs.
//!
//! # Public API (for scenario authors / `liquide-session/tests`)
//!
//! Capture:
//! - [`CaptureOptions`] — fixed surface size, theme, assets dir, dev-mode flag.
//! - [`Frame`] — a captured frame normalised to **RGBA8**, top-down, tightly
//!   packed (`stride == width * 4`).
//! - [`capture_desktop`] — render ONE deterministic desktop frame and return it
//!   (uses `DesktopCompositor::capture_once`; single-threaded, time `t0`).
//! - [`capture_desktop_scripted`] — drive the threaded `run()` loop with a
//!   scripted input sequence + trailing `Quit`, then read back the last
//!   presented frame (for event-driven scenarios such as the context menu).
//!
//! Diff:
//! - [`diff::DiffOptions`], [`diff::DiffResult`], [`diff::diff_frames`] —
//!   threshold/perceptual comparison of two [`Frame`]s.
//!
//! Golden:
//! - [`golden::assert_golden`] / [`golden::assert_golden_with`] — load, compare,
//!   and (when blessing is enabled) update a golden PNG under `golden/`. On
//!   mismatch, writes `expected.png`, `actual.png`, `diff.png` under
//!   `target/visual-test/<name>/` and panics with the absolute paths.
//! - Blessing: set `LIQUIDE_UPDATE_GOLDEN=1` (or `BLESS=1`) to rewrite goldens
//!   from the current render instead of asserting.
//!
//! Helpers also useful to scenarios: [`Frame::crop`], [`Frame::is_uniform`],
//! [`Frame::non_background_pixels`], [`Frame::save_png`].

pub mod capture;
pub mod diff;
pub mod golden;
// t56-f7: reusable scenario builders + deterministic test-assets root (additive
// module declaration; the scenarios themselves live in scenarios.rs, f7's lock).
pub mod scenarios;

pub use capture::{
    CaptureOptions, Frame, VisualTestError, capture_desktop, capture_desktop_scripted,
    capture_desktop_scripted_readback, capture_desktop_scripted_sync, capture_desktop_scripted_with,
};

// t57-e1 (A0): typed scripted-scenario builder, per-surface scenario builders,
// and named crop regions — the foundation all Thrust-A test slices (e2-e7) call.
pub use scenarios::{
    Region, ScriptedScenario, capture_scripted, context_menu_capture, crop_region, dialog_open,
    dock_capture, launcher_open, lockscreen, notification_center_open, notification_shown, overview,
    region_dock_band, region_launcher, region_notification_area, region_status_bar,
    region_status_bar_center, region_status_bar_right, region_wallpaper, scenario_options,
    status_bar_capture, themed_desktop_capture, tooltip_shown, wallpaper_capture, window_decorations,
    workspace_switch,
};
