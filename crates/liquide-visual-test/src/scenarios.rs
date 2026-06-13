//! Reusable scenario builders for the four e2e visual-regression cases (t56-f7).
//!
//! Each scenario maps 1:1 to a user-reported symptom and to a t56 hypothesis:
//!
//! | scenario                          | symptom              | hypothesis |
//! |-----------------------------------|----------------------|------------|
//! | [`themed_desktop_capture`]        | "CSS not applying"   | H1 (theme) |
//! | [`text_capture`] (+ heuristic)    | "weird fonts"        | H2 (fonts) |
//! | [`status_bar_capture`]            | "janky bars"         | e2/e4      |
//! | [`context_menu_capture`]          | "dead context menus" | e4/e5/e6   |
//!
//! All captures are routed through a **dedicated deterministic test assets
//! root** ([`test_assets_root`]) so golden images are byte-stable across
//! machines and CI: it pins the UI font to the vendored Apache-2.0 test font
//! (`test-assets/fonts/Inter/InterVariable.ttf`) regardless of whether the real
//! `scripts/download-fonts.ps1` font set has been installed, while still using
//! the live `assets/themes/` CSS so the H1 differential remains meaningful and
//! themes never drift from a committed copy.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use liquide_input::mouse::{ButtonState, MouseButton, MouseEvent};
use liquide_platform::{NativeWindowHandle, PlatformEvent};

use crate::capture::{
    CaptureOptions, Frame, VisualTestError, capture_desktop, capture_desktop_scripted_sync,
};

/// Canonical golden surface size. Small to keep committed PNGs tiny while still
/// exercising the full status-bar / desktop layout.
pub const SCENARIO_WIDTH: u32 = 1280;
/// Canonical golden surface height.
pub const SCENARIO_HEIGHT: u32 = 720;

/// The status bar is `position: fixed; top: 0; height: 34` (see
/// `assets/themes/night.css`). Crop a slightly taller band to tolerate the
/// 1px bottom border + subpixel rounding.
pub const STATUS_BAR_HEIGHT: u32 = 36;

/// This crate's own `test-assets/` directory (the deterministic font lives here).
#[must_use]
pub fn crate_test_assets_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("test-assets")
}

/// The workspace `assets/` directory (live themes/templates).
#[must_use]
pub fn workspace_assets_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("assets")
}

/// Build (once per process) a merged, deterministic assets root under
/// `target/visual-test/assets-root/` and return its absolute path.
///
/// The merged root contains:
/// - `fonts/`     — copied from THIS crate's `test-assets/fonts/` (the pinned
///   Apache-2.0 test font, so disk-loading always wins over f2's embedded
///   fallback and every machine renders identical glyphs).
/// - `themes/`, `templates/`, `desktop.html` — copied from the live workspace
///   `assets/` so the real CSS cascade is exercised (the H1 differential is only
///   meaningful against the real `liquid_glass.css` / `night.css`).
///
/// Copying (rather than committing a second theme copy) keeps the harness from
/// drifting out of sync with the real themes.
pub fn test_assets_root() -> &'static Path {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| build_test_assets_root().expect("failed to build test assets root"))
        .as_path()
}

fn build_test_assets_root() -> std::io::Result<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("visual-test")
        .join("assets-root");

    // Pinned test font (overwrite each run so a changed test font propagates).
    let crate_fonts = crate_test_assets_dir().join("fonts");
    copy_dir_recursive(&crate_fonts, &root.join("fonts"))?;

    // Live themes/templates so the real CSS is exercised.
    let ws = workspace_assets_dir();
    for sub in ["themes", "templates"] {
        let src = ws.join(sub);
        if src.is_dir() {
            copy_dir_recursive(&src, &root.join(sub))?;
        }
    }
    let desktop_html = ws.join("desktop.html");
    if desktop_html.is_file() {
        std::fs::create_dir_all(&root)?;
        std::fs::copy(&desktop_html, root.join("desktop.html"))?;
    }

    Ok(root)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

/// Capture options for a scenario at the canonical size, pinned to the
/// deterministic test assets root and the given theme.
#[must_use]
pub fn scenario_options(theme: &str) -> CaptureOptions {
    CaptureOptions::default()
        .size(SCENARIO_WIDTH, SCENARIO_HEIGHT)
        .theme(theme)
        .assets_dir(test_assets_root().to_string_lossy().into_owned())
}

/// Capture a full desktop frame under `theme` (deterministic single frame).
///
/// Used by both the themed-desktop differential (night vs liquid-glass) and as
/// the base frame for the text / status-bar scenarios.
pub fn themed_desktop_capture(theme: &str) -> Result<Frame, VisualTestError> {
    capture_desktop(&scenario_options(theme))
}

/// Capture a desktop frame for the text-content heuristic.
///
/// The desktop always renders glyph-bearing chrome (status-bar clock, logo,
/// labels), so we capture the standard themed desktop and let the caller crop
/// to a known text-bearing region.
pub fn text_capture(theme: &str) -> Result<Frame, VisualTestError> {
    themed_desktop_capture(theme)
}

/// Capture and crop the top status-bar band under `theme`.
pub fn status_bar_capture(theme: &str) -> Result<Frame, VisualTestError> {
    let frame = themed_desktop_capture(theme)?;
    Ok(frame.crop(0, 0, frame.width, STATUS_BAR_HEIGHT.min(frame.height)))
}

/// A scripted right-click at the given desktop point, captured AFTER the click
/// is processed (so an opened context-menu overlay is visible in the frame).
///
/// Uses the deterministic synchronous [`capture_desktop_scripted_sync`] path
/// (t56-f4): the events are applied during `capture_once`'s prologue and the
/// post-click frame is read straight off the CPU framebuffer. The earlier
/// threaded `run()` path ([`capture_desktop_scripted`]) exited on the trailing
/// `Quit` before ever presenting the post-click frame, which made the
/// (correctly painted) context menu read back as zero changed pixels. Moves the
/// pointer to `(x, y)`, then a right-button press + release on the desktop
/// window (`NativeWindowHandle(1)`).
pub fn context_menu_capture(theme: &str, x: f32, y: f32) -> Result<Frame, VisualTestError> {
    capture_desktop_scripted_sync(&scenario_options(theme), |handle| {
        right_click_events(handle, x, y)
    })
}

/// Build the Move / press / release event sequence for a right-click at
/// `(x, y)` targeting `handle`.
#[must_use]
pub fn right_click_events(handle: NativeWindowHandle, x: f32, y: f32) -> Vec<PlatformEvent> {
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
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assets_root_has_pinned_font_and_real_themes() {
        let root = test_assets_root();
        assert!(
            root.join("fonts")
                .join("Inter")
                .join("InterVariable.ttf")
                .is_file(),
            "deterministic test font must be present under the merged assets root"
        );
        assert!(
            root.join("themes").join("night.css").is_file(),
            "live night theme must be copied into the merged assets root"
        );
        // The liquid-glass theme is on disk as liquid_glass.css (underscore);
        // f1's resolver tolerates the hyphen spelling at load time.
        assert!(
            root.join("themes").join("liquid_glass.css").is_file(),
            "live liquid-glass theme must be copied into the merged assets root"
        );
    }

    #[test]
    fn right_click_events_are_move_press_release() {
        let evs = right_click_events(NativeWindowHandle(1), 100.0, 200.0);
        assert_eq!(evs.len(), 3);
        assert!(matches!(
            evs[0],
            PlatformEvent::MouseInput {
                event: MouseEvent::Move { .. },
                ..
            }
        ));
        assert!(matches!(
            evs[2],
            PlatformEvent::MouseInput {
                event: MouseEvent::Button {
                    button: MouseButton::Right,
                    state: ButtonState::Released,
                    ..
                },
                ..
            }
        ));
    }
}
