//! Main event loop for the standalone compositor.
//!
//! Multiplexes DRM page-flip events, input device events, and Wayland client
//! connections using `epoll` on Linux. On non-Linux platforms the loop is a
//! no-op stub that returns immediately.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tracing::{debug, error, info, trace, warn};

use crate::display::DisplayOutput;
use crate::input::InputDeviceSummary;
use crate::wayland::WaylandServerState;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Token values stored in `epoll_event::u64` to identify the fd source.
#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FdToken {
    Drm = 1,
    Input = 2,
    Wayland = 3,
    Timer = 4,
    Signal = 5,
}

/// Per-frame timing diagnostics.
#[derive(Debug, Clone, Default)]
pub struct FrameStats {
    /// Total number of frames presented.
    pub frames_presented: u64,
    /// Number of frames skipped because no damage was pending.
    pub frames_skipped: u64,
    /// Timestamp of the last presented frame.
    pub last_present_ns: u64,
    /// Duration of the last frame (render + present), in microseconds.
    pub last_frame_us: u64,
    /// Number of events processed in the last iteration.
    pub last_event_count: u32,
}

/// Configuration for the event loop, derived from display modes and user prefs.
pub struct EventLoopConfig {
    /// Target frame interval derived from the display refresh rate.
    pub frame_interval: Duration,
    /// DRM device file descriptor (if available).
    pub drm_fd: Option<i32>,
    /// Whether the Wayland server is active.
    pub wayland_active: bool,
}

impl Default for EventLoopConfig {
    fn default() -> Self {
        Self {
            // Default to 60 Hz
            frame_interval: Duration::from_nanos(16_666_667),
            drm_fd: None,
            wayland_active: false,
        }
    }
}

impl EventLoopConfig {
    /// Build from a refresh rate in Hz. Falls back to 60 Hz if `hz` is 0.
    pub fn with_refresh_hz(hz: u32) -> Self {
        let hz = if hz == 0 { 60 } else { hz };
        let nanos = 1_000_000_000u64 / u64::from(hz);
        Self {
            frame_interval: Duration::from_nanos(nanos),
            ..Self::default()
        }
    }
}

/// The standalone compositor event loop.
///
/// Owns the `running` flag so external code (signal handlers, shutdown
/// requests) can request a graceful stop via [`EventLoop::stop()`].
pub struct EventLoop {
    running: Arc<AtomicBool>,
    config: EventLoopConfig,
    stats: FrameStats,

    // Subsystem state (non-owning references are impractical across the
    // unsafe epoll boundary, so we track lightweight state here).
    pending_pageflip: bool,
    damage_pending: bool,

    #[cfg(target_os = "linux")]
    epoll_fd: i32,
    #[cfg(target_os = "linux")]
    signal_fd: i32,
}

impl EventLoop {
    /// Create a new event loop with the given configuration.
    ///
    /// The returned `Arc<AtomicBool>` is the shared `running` flag — set it
    /// to `false` from any thread (e.g. a signal handler) to request shutdown.
    pub fn new(config: EventLoopConfig) -> std::io::Result<(Self, Arc<AtomicBool>)> {
        let running = Arc::new(AtomicBool::new(false));
        let running_clone = Arc::clone(&running);

        #[cfg(target_os = "linux")]
        let (epoll_fd, signal_fd) = create_epoll_and_signalfd()?;

        Ok((
            Self {
                running,
                config,
                stats: FrameStats::default(),
                pending_pageflip: false,
                damage_pending: true, // render the first frame unconditionally
                #[cfg(target_os = "linux")]
                epoll_fd,
                #[cfg(target_os = "linux")]
                signal_fd,
            },
            running_clone,
        ))
    }

    /// Get a clone of the running flag for external shutdown requests.
    pub fn running_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.running)
    }

    /// Request the loop to stop after the current iteration.
    pub fn stop(&self) {
        self.running.store(false, Ordering::Release);
    }

    /// Whether the loop is currently running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    /// Current frame statistics.
    pub fn stats(&self) -> &FrameStats {
        &self.stats
    }

    /// Register a file descriptor with the event loop.
    #[cfg(target_os = "linux")]
    pub fn register_fd(&self, fd: i32, token: u64) -> std::io::Result<()> {
        let mut ev = libc::epoll_event {
            events: (libc::EPOLLIN | libc::EPOLLET) as u32,
            u64: token,
        };
        // SAFETY: epoll_fd is a valid epoll instance, fd is a valid open descriptor,
        // and ev is a correctly initialised epoll_event on the stack.
        let ret = unsafe {
            libc::epoll_ctl(self.epoll_fd, libc::EPOLL_CTL_ADD, fd, &mut ev)
        };
        if ret < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn register_fd(&self, _fd: i32, _token: u64) -> std::io::Result<()> {
        Ok(())
    }

    /// Register the DRM device fd for page-flip events.
    pub fn register_drm(&mut self, drm_fd: i32) -> std::io::Result<()> {
        self.config.drm_fd = Some(drm_fd);
        self.register_fd(drm_fd, FdToken::Drm as u64)
    }

    /// Register an input device fd for readability events.
    pub fn register_input(&self, input_fd: i32) -> std::io::Result<()> {
        self.register_fd(input_fd, FdToken::Input as u64)
    }

    /// Register the Wayland server socket fd for incoming connections.
    pub fn register_wayland(&mut self, wayland_fd: i32) -> std::io::Result<()> {
        self.config.wayland_active = true;
        self.register_fd(wayland_fd, FdToken::Wayland as u64)
    }

    /// Notify the event loop that the scene has changed and a new frame
    /// should be rendered.
    pub fn mark_damaged(&mut self) {
        self.damage_pending = true;
    }

    // ── Main loop ───────────────────────────────────────────────────

    /// Run the event loop until [`stop()`] is called.
    ///
    /// `on_input`, `on_wayland`, and `on_render` are callbacks invoked from
    /// the loop body so the caller can wire them into the compositor without
    /// the event loop owning the subsystem objects directly.
    ///
    /// On non-Linux platforms this immediately returns `Ok(())`.
    #[cfg(target_os = "linux")]
    pub fn run<FI, FW, FR>(
        &mut self,
        mut on_input: FI,
        mut on_wayland: FW,
        mut on_render: FR,
    ) -> std::io::Result<()>
    where
        FI: FnMut() -> bool,   // returns true if damage was produced
        FW: FnMut() -> bool,   // returns true if damage was produced
        FR: FnMut() -> bool,   // returns true if frame was submitted
    {
        self.running.store(true, Ordering::Release);
        info!(
            interval_ms = self.config.frame_interval.as_millis(),
            "event loop starting"
        );

        // Register the signalfd for SIGTERM/SIGINT.
        self.register_fd(self.signal_fd, FdToken::Signal as u64)?;

        let mut events = [libc::epoll_event { events: 0, u64: 0 }; 16];
        let timeout_ms = self.config.frame_interval.as_millis() as i32;

        while self.running.load(Ordering::Acquire) {
            let frame_start = Instant::now();
            let mut event_count: u32 = 0;

            // ── 1. epoll_wait ───────────────────────────────────────
            // SAFETY: epoll_fd is valid, events array is on the stack with
            // correct length, timeout_ms is a non-negative duration.
            let n = unsafe {
                libc::epoll_wait(
                    self.epoll_fd,
                    events.as_mut_ptr(),
                    events.len() as i32,
                    timeout_ms,
                )
            };

            if n < 0 {
                let err = std::io::Error::last_os_error();
                if err.raw_os_error() == Some(libc::EINTR) {
                    continue;
                }
                error!(%err, "epoll_wait failed");
                return Err(err);
            }

            // ── 2. Dispatch ready fds ───────────────────────────────
            for i in 0..n as usize {
                event_count += 1;
                match events[i].u64 {
                    t if t == FdToken::Drm as u64 => {
                        self.handle_drm_event();
                    }
                    t if t == FdToken::Input as u64 => {
                        if on_input() {
                            self.damage_pending = true;
                        }
                    }
                    t if t == FdToken::Wayland as u64 => {
                        if on_wayland() {
                            self.damage_pending = true;
                        }
                    }
                    t if t == FdToken::Signal as u64 => {
                        self.handle_signal();
                    }
                    t if t == FdToken::Timer as u64 => {
                        trace!("timer expired");
                    }
                    other => {
                        warn!(token = other, "unknown epoll token");
                    }
                }
            }

            // ── 3. Render if damaged and not waiting for pageflip ───
            if self.damage_pending && !self.pending_pageflip {
                if on_render() {
                    self.pending_pageflip = true;
                    self.damage_pending = false;
                    self.stats.frames_presented += 1;
                    self.stats.last_present_ns = frame_start
                        .elapsed()
                        .as_nanos() as u64;
                } else {
                    // Render callback returned false — no frame submitted.
                    debug!("render callback did not submit a frame");
                }
            } else if !self.damage_pending {
                self.stats.frames_skipped += 1;
            }

            // ── 4. Record frame timing ──────────────────────────────
            let elapsed = frame_start.elapsed();
            self.stats.last_frame_us = elapsed.as_micros() as u64;
            self.stats.last_event_count = event_count;

            trace!(
                events = event_count,
                frame_us = self.stats.last_frame_us,
                presented = self.stats.frames_presented,
                skipped = self.stats.frames_skipped,
                "frame"
            );
        }

        info!(
            presented = self.stats.frames_presented,
            skipped = self.stats.frames_skipped,
            "event loop stopped"
        );
        Ok(())
    }

    /// Non-Linux stub — immediately returns.
    #[cfg(not(target_os = "linux"))]
    pub fn run<FI, FW, FR>(
        &mut self,
        _on_input: FI,
        _on_wayland: FW,
        _on_render: FR,
    ) -> std::io::Result<()>
    where
        FI: FnMut() -> bool,
        FW: FnMut() -> bool,
        FR: FnMut() -> bool,
    {
        info!("event loop not available on this platform");
        Ok(())
    }

    // ── Internal helpers ────────────────────────────────────────────

    /// Handle a DRM page-flip completion event.
    #[cfg(target_os = "linux")]
    fn handle_drm_event(&mut self) {
        if let Some(drm_fd) = self.config.drm_fd {
            // Read DRM events from the fd. The fd is in edge-triggered mode
            // so we must drain all pending data.
            let mut buf = [0u8; 4096];
            loop {
                // SAFETY: drm_fd is a valid DRM device fd set to non-blocking
                // via epoll edge-trigger. buf is correctly sized.
                let len = unsafe {
                    libc::read(drm_fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len())
                };
                if len <= 0 {
                    break;
                }
                // We successfully consumed page-flip events — unblock the
                // next frame submission.
                self.pending_pageflip = false;
                trace!(bytes = len, "DRM event data consumed");
            }
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn handle_drm_event(&mut self) {
        self.pending_pageflip = false;
    }

    /// Handle a signal (SIGTERM/SIGINT) arriving on the signalfd.
    #[cfg(target_os = "linux")]
    fn handle_signal(&mut self) {
        // Read the signalfd_siginfo to acknowledge the signal.
        let mut buf = [0u8; 128]; // signalfd_siginfo is 128 bytes
        // SAFETY: signal_fd is a valid signalfd, buf is exactly 128 bytes
        // which is the size of struct signalfd_siginfo.
        let _ = unsafe {
            libc::read(
                self.signal_fd,
                buf.as_mut_ptr() as *mut libc::c_void,
                buf.len(),
            )
        };
        info!("received termination signal — shutting down");
        self.running.store(false, Ordering::Release);
    }

    #[cfg(not(target_os = "linux"))]
    fn handle_signal(&mut self) {
        self.running.store(false, Ordering::Release);
    }
}

impl Drop for EventLoop {
    fn drop(&mut self) {
        #[cfg(target_os = "linux")]
        {
            if self.epoll_fd >= 0 {
                // SAFETY: epoll_fd is a valid fd created by epoll_create1.
                unsafe { libc::close(self.epoll_fd) };
            }
            if self.signal_fd >= 0 {
                // SAFETY: signal_fd is a valid fd created by signalfd.
                unsafe { libc::close(self.signal_fd) };
            }
        }
        debug!("event loop resources released");
    }
}

// ---------------------------------------------------------------------------
// Linux-specific setup helpers
// ---------------------------------------------------------------------------

/// Creates the epoll instance and a signalfd for SIGTERM + SIGINT.
#[cfg(target_os = "linux")]
fn create_epoll_and_signalfd() -> std::io::Result<(i32, i32)> {
    // ── epoll ───────────────────────────────────────────────────────
    // SAFETY: epoll_create1 with EPOLL_CLOEXEC is safe; we check the return.
    let epoll_fd = unsafe { libc::epoll_create1(libc::EPOLL_CLOEXEC) };
    if epoll_fd < 0 {
        return Err(std::io::Error::last_os_error());
    }

    // ── signalfd for graceful shutdown ──────────────────────────────
    let mut mask: libc::sigset_t = unsafe { std::mem::zeroed() };
    // SAFETY: mask is a zeroed sigset_t on the stack.
    unsafe {
        libc::sigemptyset(&mut mask);
        libc::sigaddset(&mut mask, libc::SIGTERM);
        libc::sigaddset(&mut mask, libc::SIGINT);
        // Block these signals so they arrive on the signalfd instead
        // of triggering the default handler.
        libc::sigprocmask(libc::SIG_BLOCK, &mask, std::ptr::null_mut());
    }

    // SAFETY: mask is correctly initialised above. SFD_NONBLOCK | SFD_CLOEXEC
    // are safe flags; we check the return value.
    let signal_fd = unsafe { libc::signalfd(-1, &mask, libc::SFD_NONBLOCK | libc::SFD_CLOEXEC) };
    if signal_fd < 0 {
        // SAFETY: epoll_fd is valid; clean up on failure.
        unsafe { libc::close(epoll_fd) };
        return Err(std::io::Error::last_os_error());
    }

    Ok((epoll_fd, signal_fd))
}
