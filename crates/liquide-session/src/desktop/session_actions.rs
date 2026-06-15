//! Host consumption of shell-recorded session + screenshot requests
//! (t73-session items 2 and 3).
//!
//! The shell is a state model: it RECORDS a `pending_session_request`
//! (Log Out / Restart / Shut Down) and a `pending_screenshot`
//! (Full / Window / Region / Clipboard / Record) when the corresponding
//! gesture fires, but it never performs the effect itself (t68-features §7/§10).
//! The session host (this crate, which owns the run loop, the platform backend,
//! and the rendered framebuffer) is the correct place to consume those requests
//! and carry out the real action.
//!
//! ## Power-action safety
//!
//! Restart / Shut Down (and Suspend, exposed via the supervisor path) perform
//! REAL OS power calls only when [`DesktopCompositor::set_real_runtime`] has been
//! turned on by the live binary. In tests and headless runs `real_runtime` is
//! `false`, so the request is consumed and recorded (observable) but no power
//! call is made — a test can never accidentally shut the machine down.
//!
//! ## Screenshot fulfillment
//!
//! Screenshot requests are fulfilled from the session's own most-recently
//! presented framebuffer (no new platform capture API needed — that would be a
//! peer-owned platform change and is intentionally avoided). The frame is
//! encoded to a real PNG and written to disk.

use tracing::{info, warn};

use liquide_shell::SessionRequest;
use liquide_shell::shell::ScreenshotRequest;

use super::screenshot::{self, ScreenshotFrame};
use super::{DesktopCompositor, DispatchedSessionAction};

impl DesktopCompositor {
    /// Consume any pending session-lifecycle request the shell recorded and
    /// dispatch it (t73-session item 2).
    ///
    /// - **Log Out** ends the desktop loop (sets `quit_requested` so the loop
    ///   flushes the final frame and exits cleanly).
    /// - **Restart / Shut Down** perform a real platform power call when
    ///   `real_runtime` is on; otherwise they only record the dispatched action.
    ///
    /// The **Lock** path is NOT routed through here: the shell drives the
    /// canonical lock-screen state directly (`ShellAction::LockSession` →
    /// `lock_session()`), so locking is already live without a host request.
    ///
    /// Returns `true` if a request was consumed and dispatched.
    pub(super) fn consume_session_request(&mut self) -> bool {
        let Some(request) = self.shell.take_session_request() else {
            return false;
        };

        let action = match request {
            SessionRequest::LogOut => DispatchedSessionAction::LogOut,
            SessionRequest::Restart => DispatchedSessionAction::Restart,
            SessionRequest::Shutdown => DispatchedSessionAction::Shutdown,
        };
        self.last_session_action = Some(action);

        match action {
            DispatchedSessionAction::LogOut => {
                info!("session request: log out — requesting desktop loop shutdown");
                // End the session by requesting quit; the loop flushes the
                // final frame before exiting (same path as a window-close).
                self.quit_requested = true;
            }
            DispatchedSessionAction::Restart => {
                if self.real_runtime {
                    info!("session request: restart — performing real platform power call");
                    perform_power_action(PowerAction::Restart);
                } else {
                    info!(
                        "session request: restart — state-only (real_runtime off; \
                         no power call performed)"
                    );
                }
                self.quit_requested = true;
            }
            DispatchedSessionAction::Shutdown => {
                if self.real_runtime {
                    info!("session request: shutdown — performing real platform power call");
                    perform_power_action(PowerAction::Shutdown);
                } else {
                    info!(
                        "session request: shutdown — state-only (real_runtime off; \
                         no power call performed)"
                    );
                }
                self.quit_requested = true;
            }
        }

        true
    }

    /// Consume any pending screenshot request the shell recorded and fulfil it
    /// by writing a PNG of the last presented frame to disk (t73-session item 3).
    ///
    /// Returns the path written on success, or `None` when there was no request
    /// or no frame to capture / the write failed (a warning is logged on
    /// failure so the request is never silently dropped).
    ///
    /// Region / Window / Clipboard modes degrade to a full-frame PNG-to-disk:
    /// an interactive region selector and a true clipboard write need extra
    /// platform plumbing (escalation territory). The request is still consumed
    /// and a file is written, so the user gets output rather than nothing.
    pub(super) fn consume_screenshot_request(&mut self) -> Option<std::path::PathBuf> {
        let request = self.shell.take_screenshot_request()?;

        // `Record` is a recording start/stop, not a single still capture; there
        // is no per-frame PNG to write for it here. Consume it (so it is not
        // left pending forever) but do not write a file.
        if matches!(request, ScreenshotRequest::Record) {
            info!("screenshot request: record — consumed (recording is not a still capture)");
            return None;
        }

        let Some(snapshot) = self.last_presented_frame.clone() else {
            warn!("screenshot request received but no frame has been presented yet; skipping");
            return None;
        };

        let mode_tag = match request {
            ScreenshotRequest::Full => "full",
            ScreenshotRequest::Window => "window",
            ScreenshotRequest::Region => "region",
            ScreenshotRequest::ToClipboard => "clipboard",
            ScreenshotRequest::Record => "record",
        };

        let dir = screenshot::screenshot_directory();
        let path = dir.join(screenshot::default_filename(mode_tag));

        let frame = ScreenshotFrame {
            width: snapshot.width,
            height: snapshot.height,
            stride: snapshot.stride,
            pixels: &snapshot.pixels,
        };
        match screenshot::write_png(&frame, &path) {
            Ok(()) => {
                info!(
                    mode = mode_tag,
                    path = %path.display(),
                    "screenshot saved to disk as PNG"
                );
                Some(path)
            }
            Err(err) => {
                warn!(
                    mode = mode_tag,
                    path = %path.display(),
                    error = %err,
                    "failed to write screenshot PNG"
                );
                None
            }
        }
    }
}

/// A real OS power action.
#[derive(Debug, Clone, Copy)]
enum PowerAction {
    Restart,
    Shutdown,
}

/// Perform a real OS power action via the platform's power command.
///
/// Kept here (rather than a new `liquide-platform` API) because the platform
/// crate is owned by a peer and adding a power-backend trait is an escalation.
/// This invokes the well-known per-OS power command. It is reached ONLY when
/// `real_runtime` is enabled, so it never fires in tests/headless runs.
fn perform_power_action(action: PowerAction) {
    #[cfg(target_os = "windows")]
    let (program, args): (&str, &[&str]) = match action {
        PowerAction::Restart => ("shutdown", &["/r", "/t", "0"]),
        PowerAction::Shutdown => ("shutdown", &["/s", "/t", "0"]),
    };
    #[cfg(not(target_os = "windows"))]
    let (program, args): (&str, &[&str]) = match action {
        PowerAction::Restart => ("systemctl", &["reboot"]),
        PowerAction::Shutdown => ("systemctl", &["poweroff"]),
    };

    match std::process::Command::new(program).args(args).spawn() {
        Ok(_) => info!(?action, program, "dispatched OS power command"),
        Err(err) => warn!(?action, program, error = %err, "failed to dispatch OS power command"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_shell::shell::ScreenshotRequest;

    fn desktop_with_presented_frame(w: u32, h: u32) -> DesktopCompositor {
        let mut desktop = DesktopCompositor::new(w, h);
        let stride = w * 4;
        let pixels = vec![32u8; (stride * h) as usize];
        desktop.last_presented_frame = Some(super::super::PresentedFrameSnapshot {
            pixels: std::sync::Arc::new(pixels),
            width: w,
            height: h,
            stride,
        });
        desktop
    }

    #[test]
    fn session_request_is_consumed_and_dispatched_state_only() {
        // t73-session item 2: the host consumes the shell's recorded
        // session-lifecycle request and dispatches it. With real_runtime OFF
        // (the test default) no power call fires — only state changes.
        let mut desktop = DesktopCompositor::new(64, 64);
        assert!(!desktop.real_runtime, "tests must default to state-only");

        // Nothing pending yet.
        assert!(!desktop.consume_session_request());
        assert_eq!(desktop.last_session_action(), None);

        // Drive the shell into recording a Shutdown request.
        desktop
            .shell_mut()
            .execute_action(&liquide_shell::ShellAction::Shutdown);
        assert_eq!(
            desktop.shell().pending_session_request(),
            Some(SessionRequest::Shutdown),
            "the shell must have recorded the request"
        );

        // Host consumes + dispatches it.
        assert!(desktop.consume_session_request());
        assert_eq!(
            desktop.last_session_action(),
            Some(DispatchedSessionAction::Shutdown),
            "the dispatched action must be observable"
        );
        assert!(
            desktop.shell().pending_session_request().is_none(),
            "the request must be taken (consumed exactly once)"
        );
        assert!(
            desktop.quit_requested,
            "shutdown must request the desktop loop to wind down"
        );

        // A second consume finds nothing (idempotent).
        assert!(!desktop.consume_session_request());
    }

    #[test]
    fn logout_request_consumed_requests_quit() {
        let mut desktop = DesktopCompositor::new(64, 64);
        desktop
            .shell_mut()
            .execute_action(&liquide_shell::ShellAction::LogOut);
        assert!(desktop.consume_session_request());
        assert_eq!(
            desktop.last_session_action(),
            Some(DispatchedSessionAction::LogOut)
        );
        assert!(desktop.quit_requested);
    }

    #[test]
    fn screenshot_request_writes_a_png_to_disk() {
        // t73-session item 3: a recorded screenshot request is consumed and a
        // real PNG of the presented frame is written to disk.
        let tmp = std::env::temp_dir().join(format!(
            "liquide-t73-shot-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&tmp).unwrap();
        // SAFETY: single-threaded test; redirect output to the temp dir.
        unsafe {
            std::env::set_var("LIQUIDE_SCREENSHOT_DIR", &tmp);
        }

        let mut desktop = desktop_with_presented_frame(8, 6);

        // No request pending → nothing written.
        assert!(desktop.consume_screenshot_request().is_none());

        // Record a Full screenshot request via the shell.
        desktop
            .shell_mut()
            .execute_action(&liquide_shell::ShellAction::ScreenshotFull);
        assert_eq!(
            desktop.shell().pending_screenshot(),
            Some(ScreenshotRequest::Full)
        );

        let path = desktop
            .consume_screenshot_request()
            .expect("a PNG path must be returned");
        assert!(path.exists(), "the PNG file must exist on disk");
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(
            &bytes[0..8],
            &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a],
            "the file must be a valid PNG"
        );
        assert!(
            desktop.shell().pending_screenshot().is_none(),
            "the request must be consumed exactly once"
        );

        unsafe {
            std::env::remove_var("LIQUIDE_SCREENSHOT_DIR");
        }
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn screenshot_without_a_frame_is_a_noop() {
        let mut desktop = DesktopCompositor::new(64, 64);
        desktop
            .shell_mut()
            .execute_action(&liquide_shell::ShellAction::ScreenshotFull);
        // No presented frame yet → consumed but nothing written.
        assert!(desktop.consume_screenshot_request().is_none());
        assert!(
            desktop.shell().pending_screenshot().is_none(),
            "the request is still consumed so it does not linger"
        );
    }
}
