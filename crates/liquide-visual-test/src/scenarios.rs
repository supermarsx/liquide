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

use liquide_input::keyboard::{KeyCode, KeyEvent, KeyState, Modifiers};
use liquide_input::mouse::{ButtonState, MouseButton, MouseEvent, ScrollAxis};
use liquide_platform::{NativeWindowHandle, PlatformEvent};

use crate::capture::{
    CaptureOptions, Frame, VisualTestError, capture_desktop, capture_desktop_scripted_sync,
    capture_desktop_scripted_with,
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

// ===========================================================================
// Typed scripted-scenario builder (t57-e1 / A0)
// ===========================================================================
//
// Generalises the `right_click_events` helper into a fluent builder that emits a
// `Vec<PlatformEvent>` for an arbitrary input sequence: pointer moves, button
// press/release, double-click, drag, key down/up, text entry, hotkeys with
// modifiers, and scroll/wheel. Every event targets a single window handle (the
// desktop window — `NativeWindowHandle(1)` under the capture path).
//
// Peers (e2-e7) build a `ScriptedScenario`, then either drive it through
// [`capture_scripted`] (renders the post-state frame) or read the raw events via
// [`ScriptedScenario::events`] for use with the lower-level capture entry points.

/// Default per-frame timestamp step (microseconds) handed to synthesized
/// [`KeyEvent`]s. Deterministic; the capture path does not advance the clock.
const KEY_TS_US: u64 = 0;

/// A fluent builder for a deterministic sequence of [`PlatformEvent`]s targeting
/// one window handle.
///
/// ```ignore
/// let events = ScriptedScenario::new(handle)
///     .pointer_move(100.0, 200.0)
///     .left_click(100.0, 200.0)
///     .hotkey(KeyCode::T, Modifiers::from_bits(Modifiers::SUPER))
///     .into_events();
/// ```
#[derive(Debug, Clone)]
pub struct ScriptedScenario {
    handle: NativeWindowHandle,
    events: Vec<PlatformEvent>,
}

impl ScriptedScenario {
    /// Start a new empty scenario targeting `handle`.
    #[must_use]
    pub fn new(handle: NativeWindowHandle) -> Self {
        Self {
            handle,
            events: Vec::new(),
        }
    }

    /// Consume the builder, returning the accumulated events.
    #[must_use]
    pub fn into_events(self) -> Vec<PlatformEvent> {
        self.events
    }

    /// Borrow the accumulated events without consuming the builder.
    #[must_use]
    pub fn events(&self) -> &[PlatformEvent] {
        &self.events
    }

    fn mouse(mut self, event: MouseEvent) -> Self {
        self.events.push(PlatformEvent::MouseInput {
            handle: self.handle,
            event,
        });
        self
    }

    fn key(mut self, event: KeyEvent) -> Self {
        self.events.push(PlatformEvent::KeyInput {
            handle: self.handle,
            event,
        });
        self
    }

    /// Move the pointer to `(x, y)`.
    #[must_use]
    pub fn pointer_move(self, x: f32, y: f32) -> Self {
        self.mouse(MouseEvent::Move { x, y })
    }

    /// Press `button` at `(x, y)`.
    #[must_use]
    pub fn button_press(self, button: MouseButton, x: f32, y: f32) -> Self {
        self.mouse(MouseEvent::Button {
            button,
            state: ButtonState::Pressed,
            x,
            y,
        })
    }

    /// Release `button` at `(x, y)`.
    #[must_use]
    pub fn button_release(self, button: MouseButton, x: f32, y: f32) -> Self {
        self.mouse(MouseEvent::Button {
            button,
            state: ButtonState::Released,
            x,
            y,
        })
    }

    /// Full left-click (move + press + release) at `(x, y)`.
    #[must_use]
    pub fn left_click(self, x: f32, y: f32) -> Self {
        self.pointer_move(x, y)
            .button_press(MouseButton::Left, x, y)
            .button_release(MouseButton::Left, x, y)
    }

    /// Full right-click (move + press + release) at `(x, y)`.
    #[must_use]
    pub fn right_click(self, x: f32, y: f32) -> Self {
        self.pointer_move(x, y)
            .button_press(MouseButton::Right, x, y)
            .button_release(MouseButton::Right, x, y)
    }

    /// Double left-click at `(x, y)` (two press/release pairs after a move).
    #[must_use]
    pub fn double_click(self, x: f32, y: f32) -> Self {
        self.pointer_move(x, y)
            .button_press(MouseButton::Left, x, y)
            .button_release(MouseButton::Left, x, y)
            .button_press(MouseButton::Left, x, y)
            .button_release(MouseButton::Left, x, y)
    }

    /// Drag `button` from `(x0, y0)` to `(x1, y1)` through `steps` interpolated
    /// intermediate moves (press at start, moves, release at end). `steps` is
    /// clamped to at least 1.
    #[must_use]
    pub fn drag(
        mut self,
        button: MouseButton,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        steps: u32,
    ) -> Self {
        let steps = steps.max(1);
        self = self
            .pointer_move(x0, y0)
            .button_press(button, x0, y0);
        for i in 1..=steps {
            let t = i as f32 / steps as f32;
            let x = x0 + (x1 - x0) * t;
            let y = y0 + (y1 - y0) * t;
            self = self.pointer_move(x, y);
        }
        self.button_release(button, x1, y1)
    }

    /// Press a key (down only) with the given modifiers.
    #[must_use]
    pub fn key_down(self, key: KeyCode, modifiers: Modifiers) -> Self {
        self.key(KeyEvent::new(key, KeyState::Pressed, modifiers, 0, KEY_TS_US))
    }

    /// Release a key (up only) with the given modifiers.
    #[must_use]
    pub fn key_up(self, key: KeyCode, modifiers: Modifiers) -> Self {
        self.key(KeyEvent::new(key, KeyState::Released, modifiers, 0, KEY_TS_US))
    }

    /// Press + release a key with modifiers (a hotkey chord). Only the press is
    /// routed to actions by the shell, but the release keeps the sequence
    /// well-formed.
    #[must_use]
    pub fn hotkey(self, key: KeyCode, modifiers: Modifiers) -> Self {
        self.key_down(key, modifiers).key_up(key, modifiers)
    }

    /// Type a single character key (press + release, no modifiers). Only handles
    /// ASCII letters/digits/space that map to a [`KeyCode`]; other characters are
    /// skipped.
    #[must_use]
    pub fn type_char(self, c: char) -> Self {
        match char_to_keycode(c) {
            Some(key) => self.hotkey(key, Modifiers::new()),
            None => self,
        }
    }

    /// Type a run of text by mapping each character to a key press/release.
    #[must_use]
    pub fn type_text(mut self, text: &str) -> Self {
        for c in text.chars() {
            self = self.type_char(c);
        }
        self
    }

    /// Scroll/wheel by `delta` on `axis` at `(x, y)`.
    #[must_use]
    pub fn scroll(self, axis: ScrollAxis, delta: f32, x: f32, y: f32) -> Self {
        self.mouse(MouseEvent::Scroll { axis, delta, x, y })
    }

    /// Vertical wheel scroll by `delta` at `(x, y)`.
    #[must_use]
    pub fn wheel(self, delta: f32, x: f32, y: f32) -> Self {
        self.scroll(ScrollAxis::Vertical, delta, x, y)
    }
}

/// Map an ASCII character to a [`KeyCode`] for text entry (letters lowercased,
/// digits, and space). Returns `None` for unsupported characters.
#[must_use]
fn char_to_keycode(c: char) -> Option<KeyCode> {
    let key = match c.to_ascii_lowercase() {
        'a' => KeyCode::A,
        'b' => KeyCode::B,
        'c' => KeyCode::C,
        'd' => KeyCode::D,
        'e' => KeyCode::E,
        'f' => KeyCode::F,
        'g' => KeyCode::G,
        'h' => KeyCode::H,
        'i' => KeyCode::I,
        'j' => KeyCode::J,
        'k' => KeyCode::K,
        'l' => KeyCode::L,
        'm' => KeyCode::M,
        'n' => KeyCode::N,
        'o' => KeyCode::O,
        'p' => KeyCode::P,
        'q' => KeyCode::Q,
        'r' => KeyCode::R,
        's' => KeyCode::S,
        't' => KeyCode::T,
        'u' => KeyCode::U,
        'v' => KeyCode::V,
        'w' => KeyCode::W,
        'x' => KeyCode::X,
        'y' => KeyCode::Y,
        'z' => KeyCode::Z,
        '0' => KeyCode::Digit0,
        '1' => KeyCode::Digit1,
        '2' => KeyCode::Digit2,
        '3' => KeyCode::Digit3,
        '4' => KeyCode::Digit4,
        '5' => KeyCode::Digit5,
        '6' => KeyCode::Digit6,
        '7' => KeyCode::Digit7,
        '8' => KeyCode::Digit8,
        '9' => KeyCode::Digit9,
        ' ' => KeyCode::Space,
        _ => return None,
    };
    Some(key)
}

/// Render a [`ScriptedScenario`] under `theme` and return the post-state frame.
///
/// Drives the deterministic synchronous capture path: the events are dispatched
/// to the shell after the loading prologue and before the read-back render.
pub fn capture_scripted(
    theme: &str,
    build: impl FnOnce(NativeWindowHandle) -> ScriptedScenario,
) -> Result<Frame, VisualTestError> {
    capture_desktop_scripted_sync(&scenario_options(theme), |handle| {
        build(handle).into_events()
    })
}

// ===========================================================================
// Named crop regions (t57-e1 / A0)
// ===========================================================================
//
// Deterministic pixel rectangles for the canonical SCENARIO_WIDTH x
// SCENARIO_HEIGHT surface so per-surface content assertions can target a slot,
// not the whole frame. Coordinates derive from the live layout constants:
//   - status bar: position fixed; top 0; height 34 (band cropped to 36).
//   - dock: bottom-anchored, icon_size 48 + 8 pad => ~56 px tall band; we crop a
//     generous bottom band to tolerate CSS glass blur/shadow/margins.
//   - launcher: CSS `launcher { width: 480; max-height: 600 }` (events.rs:942),
//     anchored near the top-left/centre; we crop a left-of-centre column tall
//     enough to contain it.
//   - notification area: top-right toast column (the notification daemon anchors
//     toasts to the top-right under the bar).
//
// Regions are returned as `(x, y, w, h)` and clamped by [`Frame::crop`]; callers
// pass them straight into `frame.crop(...)`.

/// A named crop rectangle `(x, y, w, h)` in frame pixels.
pub type Region = (u32, u32, u32, u32);

/// Height of the bottom dock band to crop (icon 48 + pad 8, plus headroom for
/// CSS shadow/blur/margins).
pub const DOCK_BAND_HEIGHT: u32 = 96;

/// The top status-bar band (full width, [`STATUS_BAR_HEIGHT`] tall).
#[must_use]
pub fn region_status_bar(width: u32, _height: u32) -> Region {
    (0, 0, width, STATUS_BAR_HEIGHT)
}

/// The bottom dock band (full width, [`DOCK_BAND_HEIGHT`] tall, bottom-anchored).
#[must_use]
pub fn region_dock_band(width: u32, height: u32) -> Region {
    let h = DOCK_BAND_HEIGHT.min(height);
    (0, height.saturating_sub(h), width, h)
}

/// The status-bar CENTER slot (middle third of the bar) — where the clock lives.
#[must_use]
pub fn region_status_bar_center(width: u32, _height: u32) -> Region {
    let third = width / 3;
    (third, 0, width - 2 * third, STATUS_BAR_HEIGHT)
}

/// The status-bar RIGHT slot (right third of the bar) — clock/tray/indicators/
/// session cluster (recon Section 3: this is the slot that reads empty pre-f1).
#[must_use]
pub fn region_status_bar_right(width: u32, _height: u32) -> Region {
    let third = width / 3;
    (2 * third, 0, width - 2 * third, STATUS_BAR_HEIGHT)
}

/// The launcher overlay rectangle (left-of-centre column under the bar; sized to
/// the CSS `launcher { width: 480; max-height: 600 }`). Clamped to the frame.
#[must_use]
pub fn region_launcher(width: u32, height: u32) -> Region {
    let lw = 480u32.min(width);
    let lh = 600u32.min(height.saturating_sub(STATUS_BAR_HEIGHT));
    (0, STATUS_BAR_HEIGHT, lw, lh)
}

/// The notification-area rectangle (top-right column below the bar) where toasts
/// and the notification-center panel anchor.
#[must_use]
pub fn region_notification_area(width: u32, height: u32) -> Region {
    let nw = 400u32.min(width);
    let nh = (height / 2).max(1);
    (width.saturating_sub(nw), STATUS_BAR_HEIGHT, nw, nh)
}

/// A chrome-free desktop/wallpaper region: a central rectangle clear of the bar,
/// dock band, launcher column, and notification column.
#[must_use]
pub fn region_wallpaper(width: u32, height: u32) -> Region {
    let x = width / 3;
    let y = STATUS_BAR_HEIGHT + (height / 8);
    let w = (width / 3).max(1);
    let h = (height / 3).max(1);
    (x, y, w.min(width.saturating_sub(x)), h.min(height.saturating_sub(y)))
}

/// Crop a frame to a [`Region`].
#[must_use]
pub fn crop_region(frame: &Frame, region: Region) -> Frame {
    let (x, y, w, h) = region;
    frame.crop(x, y, w, h)
}

// ===========================================================================
// Per-surface scenario builders (t57-e1 / A0)
// ===========================================================================
//
// Each builder pins the deterministic test-assets root + a theme and returns the
// post-state Frame, driving the shell into a target chrome state via either a
// hotkey/pointer scripted sequence (`capture_scripted`) or a direct shell
// mutation (`capture_desktop_scripted_with`).
//
// WIRING STATUS (read by the paired f-slice owners):
//   - launcher_open: hotkey Super opens the launcher (execute_action wires
//     OpenLauncher -> launcher.toggle). State is driven; paint is f3's gate.
//   - notification_shown / notification_center_open: driven via the shell's
//     public `post_notification` / `toggle_notification_center`. Paint is f4's
//     gate (dom_sync notification templates).
//   - dialog_open: driven via `request_message_dialog` (sets chrome_active_dialog).
//     Paint is f-slice work; may return the base frame until a dialog template is
//     wired.
//   - tooltip_shown: hovers a dock item via pointer; the dwell/show path is f6's
//     gate. Best-effort — may return the base frame until the tooltip seam is
//     driven on the live render path.
//   - lockscreen: hotkey Super+L drives LockSession (execute_action wires it to
//     the canonical lockscreen). Paint of the lock surface is f9's gate.
//   - workspace_switch: hotkey Super+Ctrl+Right switches workspace (execute_action
//     wires WorkspaceNext). Differential paint is f7's gate.
//   - overview: hotkey Super+Tab (TaskOverview) — NOT wired in execute_action
//     (`_ => false`); returns the base frame until f-slice wires it.
//   - window_decorations: opens an app window via `open_app_window`. Decoration
//     paint is f8's gate.
//   - context_menu / status_bar / dock / wallpaper: existing/crop-based, proven.

/// Open the launcher overlay via the Super hotkey and return the frame.
///
/// The shortcut `LeftSuper + SUPER` maps to `ShellAction::OpenLauncher`, which
/// `execute_action` routes to `launcher.toggle()`.
pub fn launcher_open(theme: &str) -> Result<Frame, VisualTestError> {
    capture_scripted(theme, |handle| {
        ScriptedScenario::new(handle)
            .hotkey(KeyCode::LeftSuper, Modifiers::from_bits(Modifiers::SUPER))
    })
}

/// Inject a notification toast (via the shell's canonical `post_notification`),
/// then return the frame so the toast is visible.
pub fn notification_shown(theme: &str) -> Result<Frame, VisualTestError> {
    capture_desktop_scripted_with(
        &scenario_options(theme),
        |_handle| Vec::new(),
        |shell| {
            let mut notif = liquide_interop::notification::Notification::new(
                "Visual Test",
                "Build complete",
            );
            notif.body = "Your project finished building successfully.".to_string();
            // Deterministic clock: t0 microseconds (matches the capture path).
            let _ = shell.post_notification(notif, 0);
        },
    )
}

/// Open the notification-center panel (via `toggle_notification_center`) after
/// posting one notification so the panel has content, and return the frame.
pub fn notification_center_open(theme: &str) -> Result<Frame, VisualTestError> {
    capture_desktop_scripted_with(
        &scenario_options(theme),
        |_handle| Vec::new(),
        |shell| {
            let mut notif = liquide_interop::notification::Notification::new(
                "Visual Test",
                "Notification center entry",
            );
            notif.body = "An item to populate the notification center.".to_string();
            let _ = shell.post_notification(notif, 0);
            if !shell.notification_center_open() {
                shell.toggle_notification_center();
            }
        },
    )
}

/// Request a message-box dialog (via the canonical `request_message_dialog`) and
/// return the frame.
///
/// NOTE: this sets `chrome_active_dialog`; whether a dialog surface paints
/// depends on a dom_sync dialog template that may not yet be wired — the paired
/// f-slice owner should confirm and wire if needed.
pub fn dialog_open(theme: &str) -> Result<Frame, VisualTestError> {
    capture_desktop_scripted_with(
        &scenario_options(theme),
        |_handle| Vec::new(),
        |shell| {
            let _ = shell.request_message_dialog(
                liquide_shell::notification::ShellDialogKind::Info,
                "Confirm action",
                "Are you sure you want to proceed?",
            );
        },
    )
}

/// Hover the pointer over the first dock item to trigger a tooltip, then return
/// the frame.
///
/// FRAME-TIMING FIX (t57-gateclose, closing f6's escalation): the dock-hover
/// path (`shell/events.rs`) sets `tooltip_text` / `tooltip_pos`, and the render
/// path advances the canonical `TooltipManager` by `frame_delta_ms` every frame
/// (`dom_sync::sync_tooltip_template` -> `sync_tooltip_manager`). A single
/// capture render at the default ~16.67 ms delta can never elapse the manager's
/// ~500 ms show-delay dwell, so the tooltip stayed Pending and never surfaced —
/// the tooltip WAS wired on the live path, the single-frame builder just could
/// not dwell.
///
/// We drive the hover via the real `PlatformEvent` pointer move (so the live
/// hover input path sets the tooltip state), then use the mutate seam to bump
/// `frame_delta_ms` well past the show-delay + fade-in (and below the 5 s
/// display-duration auto-hide) before the captured render. The capture render's
/// `sync_tooltip_manager(frame_delta_ms)` then progresses the manager
/// Pending -> FadingIn -> Visible in that frame (cf. the F07 single-large-frame
/// regression in `tooltip_adapter.rs`), so the tooltip actually paints.
pub fn tooltip_shown(theme: &str) -> Result<Frame, VisualTestError> {
    // The dock is bottom-anchored and horizontally centred; the first icon sits
    // just left of centre, ~28 px above the bottom edge.
    let cx = (SCENARIO_WIDTH as f32) / 2.0 - 80.0;
    let cy = (SCENARIO_HEIGHT as f32) - 28.0;
    capture_desktop_scripted_with(
        &scenario_options(theme),
        // Hover the first dock item through the real input path so the shell's
        // dock-hover handler sets `tooltip_text` / `tooltip_pos`.
        |handle| ScriptedScenario::new(handle).pointer_move(cx, cy).into_events(),
        // Advance the per-frame delta past the dwell so the captured render
        // elapses the canonical manager's show-delay + fade-in (800 ms is
        // > show_delay 500 + fade_in 150, < display_duration 5000).
        |shell| shell.set_frame_delta_ms(800.0),
    )
}

/// Lock the session via the Super+L hotkey and return the frame.
///
/// `Super + L` maps to `ShellAction::LockSession`, which `execute_action` routes
/// to the canonical lockscreen. The lock-surface paint is f9's gate.
pub fn lockscreen(theme: &str) -> Result<Frame, VisualTestError> {
    capture_scripted(theme, |handle| {
        ScriptedScenario::new(handle).hotkey(KeyCode::L, Modifiers::from_bits(Modifiers::SUPER))
    })
}

/// Switch to the next workspace via the Super+Ctrl+Right hotkey and return the
/// frame. (Differential paint is f7's gate.)
pub fn workspace_switch(theme: &str) -> Result<Frame, VisualTestError> {
    capture_scripted(theme, |handle| {
        ScriptedScenario::new(handle).hotkey(
            KeyCode::ArrowRight,
            Modifiers::from_bits(Modifiers::SUPER | Modifiers::CTRL),
        )
    })
}

/// Open the task overview via the Super+Tab hotkey and return the frame.
///
/// NOTE: `TaskOverview` is NOT wired in `execute_action` yet (`_ => false`), so
/// this returns the base desktop frame until the f-slice wires the overview
/// overlay.
pub fn overview(theme: &str) -> Result<Frame, VisualTestError> {
    capture_scripted(theme, |handle| {
        ScriptedScenario::new(handle).hotkey(KeyCode::Tab, Modifiers::from_bits(Modifiers::SUPER))
    })
}

/// Open one app window (via the shell's `open_app_window`) so window decorations
/// (titlebar + close/min/max) are present, and return the frame.
pub fn window_decorations(theme: &str) -> Result<Frame, VisualTestError> {
    capture_desktop_scripted_with(
        &scenario_options(theme),
        |_handle| Vec::new(),
        |shell| {
            let _ = shell.open_app_window("com.liquide.files");
        },
    )
}

/// Capture and crop the bottom dock band under `theme`.
pub fn dock_capture(theme: &str) -> Result<Frame, VisualTestError> {
    let frame = themed_desktop_capture(theme)?;
    Ok(crop_region(
        &frame,
        region_dock_band(frame.width, frame.height),
    ))
}

/// Capture and crop a chrome-free wallpaper region under `theme`.
pub fn wallpaper_capture(theme: &str) -> Result<Frame, VisualTestError> {
    let frame = themed_desktop_capture(theme)?;
    Ok(crop_region(
        &frame,
        region_wallpaper(frame.width, frame.height),
    ))
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

    // ---- typed scripted-scenario builder (pure, no capture) ----

    #[test]
    fn scripted_scenario_left_click_is_move_press_release() {
        let evs = ScriptedScenario::new(NativeWindowHandle(1))
            .left_click(10.0, 20.0)
            .into_events();
        assert_eq!(evs.len(), 3);
        assert!(matches!(
            evs[0],
            PlatformEvent::MouseInput {
                event: MouseEvent::Move { .. },
                ..
            }
        ));
        assert!(matches!(
            evs[1],
            PlatformEvent::MouseInput {
                event: MouseEvent::Button {
                    button: MouseButton::Left,
                    state: ButtonState::Pressed,
                    ..
                },
                ..
            }
        ));
    }

    #[test]
    fn scripted_scenario_double_click_has_two_press_release_pairs() {
        let evs = ScriptedScenario::new(NativeWindowHandle(1))
            .double_click(5.0, 5.0)
            .into_events();
        // move + (press,release) * 2 = 5 events.
        assert_eq!(evs.len(), 5);
    }

    #[test]
    fn scripted_scenario_drag_has_press_moves_release() {
        let evs = ScriptedScenario::new(NativeWindowHandle(1))
            .drag(MouseButton::Left, 0.0, 0.0, 100.0, 0.0, 4)
            .into_events();
        // move(start) + press + 4 interpolated moves + release = 7.
        assert_eq!(evs.len(), 7);
        assert!(matches!(
            evs.last().unwrap(),
            PlatformEvent::MouseInput {
                event: MouseEvent::Button {
                    state: ButtonState::Released,
                    ..
                },
                ..
            }
        ));
    }

    #[test]
    fn scripted_scenario_hotkey_is_down_then_up() {
        let evs = ScriptedScenario::new(NativeWindowHandle(1))
            .hotkey(KeyCode::T, Modifiers::from_bits(Modifiers::SUPER))
            .into_events();
        assert_eq!(evs.len(), 2);
        assert!(matches!(
            evs[0],
            PlatformEvent::KeyInput {
                event: KeyEvent {
                    state: KeyState::Pressed,
                    ..
                },
                ..
            }
        ));
        assert!(matches!(
            evs[1],
            PlatformEvent::KeyInput {
                event: KeyEvent {
                    state: KeyState::Released,
                    ..
                },
                ..
            }
        ));
    }

    #[test]
    fn scripted_scenario_type_text_maps_chars() {
        let evs = ScriptedScenario::new(NativeWindowHandle(1))
            .type_text("ab 1")
            .into_events();
        // 'a','b',' ','1' = 4 chars * (down+up) = 8 events.
        assert_eq!(evs.len(), 8);
    }

    #[test]
    fn scripted_scenario_scroll_emits_scroll_event() {
        let evs = ScriptedScenario::new(NativeWindowHandle(1))
            .wheel(-3.0, 50.0, 50.0)
            .into_events();
        assert_eq!(evs.len(), 1);
        assert!(matches!(
            evs[0],
            PlatformEvent::MouseInput {
                event: MouseEvent::Scroll { .. },
                ..
            }
        ));
    }

    // ---- named crop regions (pure) ----

    #[test]
    fn regions_are_within_bounds() {
        let (w, h) = (SCENARIO_WIDTH, SCENARIO_HEIGHT);
        for (name, (x, y, rw, rh)) in [
            ("status_bar", region_status_bar(w, h)),
            ("dock_band", region_dock_band(w, h)),
            ("status_bar_center", region_status_bar_center(w, h)),
            ("status_bar_right", region_status_bar_right(w, h)),
            ("launcher", region_launcher(w, h)),
            ("notification_area", region_notification_area(w, h)),
            ("wallpaper", region_wallpaper(w, h)),
        ] {
            assert!(x + rw <= w, "{name} region exceeds width");
            assert!(y + rh <= h, "{name} region exceeds height");
            assert!(rw > 0 && rh > 0, "{name} region is empty");
        }
    }

    #[test]
    fn dock_band_is_bottom_anchored() {
        let (_x, y, _w, h) = region_dock_band(SCENARIO_WIDTH, SCENARIO_HEIGHT);
        assert_eq!(y + h, SCENARIO_HEIGHT, "dock band must touch the bottom edge");
    }

    // ---- per-surface scenario builders (capture; confirm no panic + a frame) ----

    fn assert_renders(frame: Frame) {
        assert_eq!(frame.width, SCENARIO_WIDTH, "scenario frame width");
        assert_eq!(frame.height, SCENARIO_HEIGHT, "scenario frame height");
        assert!(
            !frame.is_uniform(),
            "scenario frame is uniform — pipeline produced a dead/blank frame"
        );
    }

    #[test]
    fn launcher_open_renders() {
        assert_renders(launcher_open("liquid-glass").expect("launcher_open should capture"));
    }

    #[test]
    fn notification_shown_renders() {
        assert_renders(
            notification_shown("liquid-glass").expect("notification_shown should capture"),
        );
    }

    #[test]
    fn notification_center_open_renders() {
        assert_renders(
            notification_center_open("liquid-glass")
                .expect("notification_center_open should capture"),
        );
    }

    #[test]
    fn dialog_open_renders() {
        assert_renders(dialog_open("liquid-glass").expect("dialog_open should capture"));
    }

    #[test]
    fn tooltip_shown_renders() {
        assert_renders(tooltip_shown("liquid-glass").expect("tooltip_shown should capture"));
    }

    #[test]
    fn lockscreen_renders() {
        assert_renders(lockscreen("liquid-glass").expect("lockscreen should capture"));
    }

    #[test]
    fn workspace_switch_renders() {
        assert_renders(workspace_switch("liquid-glass").expect("workspace_switch should capture"));
    }

    #[test]
    fn overview_renders() {
        assert_renders(overview("liquid-glass").expect("overview should capture"));
    }

    #[test]
    fn window_decorations_renders() {
        assert_renders(
            window_decorations("liquid-glass").expect("window_decorations should capture"),
        );
    }

    #[test]
    fn dock_capture_crops_bottom_band() {
        let frame = dock_capture("liquid-glass").expect("dock_capture should capture");
        assert_eq!(frame.width, SCENARIO_WIDTH);
        assert_eq!(frame.height, DOCK_BAND_HEIGHT);
    }

    #[test]
    fn wallpaper_capture_crops_region() {
        let frame = wallpaper_capture("liquid-glass").expect("wallpaper_capture should capture");
        assert!(frame.width > 0 && frame.height > 0);
    }
}
