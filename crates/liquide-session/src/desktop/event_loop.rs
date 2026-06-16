//! Main desktop event loop — window creation, loading screen, and frame dispatch.

use std::thread;
use std::time::{Duration, Instant};

use liquide_compositor::geometry::Rect;
use liquide_platform::{NativeWindowParams, PlatformBackend};
use tracing::{debug, info};

use super::paint_state::TimerAction;
use super::{DesktopCompositor, RenderMsg};

impl DesktopCompositor {
    /// Flush any in-flight render frame so the final desktop state is presented
    /// before the loop exits on quit (t60-runtime #1).
    ///
    /// If a render job is still in flight when a quit is requested, give the
    /// worker a brief, bounded window to complete and present that last frame.
    /// Bounded so a hung render thread cannot block shutdown indefinitely
    /// (mirrors the in-loop watchdog rationale, t60-runtime #3).
    fn flush_pending_present_for_quit(&mut self, platform: &mut dyn PlatformBackend) {
        if !self.render_in_flight {
            // Nothing pending — still try a present in case a completed frame is
            // sitting in the channel unconsumed.
            let _ = self.try_present(platform);
            return;
        }

        let deadline = Instant::now() + Duration::from_millis(500);
        while self.render_in_flight && Instant::now() < deadline {
            let _ = self.refresh_present_pacing(platform);
            if self.try_present(platform) {
                break;
            }
            thread::sleep(Duration::from_micros(200));
        }
    }

    /// Maximum time a single render job may be in flight before the watchdog
    /// assumes the render thread is hung and recovers (t60-runtime #3).
    ///
    /// Lowered 500ms -> 150ms (t77-A3) so a genuinely stalled worker is
    /// recovered ~3.3x sooner, tightening worst-case input-to-recovery latency.
    /// This is safe because the live render path's per-frame glyph-drain budget
    /// is now ~1ms (t77-A2), so a healthy frame completes far inside 150ms — the
    /// watchdog only ever fires on a real stall, never on a slow-but-progressing
    /// frame.
    const RENDER_WATCHDOG_TIMEOUT: Duration = Duration::from_millis(150);

    /// Budget for a single render frame's in-flight (submit -> present) time.
    /// A frame that takes longer than this is counted as a "slow frame" and
    /// logged so future event-loop / render regressions are observable
    /// (t77-A3). 16ms is one frame at 60fps — the responsiveness floor we want
    /// to hold. This is a TELEMETRY threshold only; it never alters rendering.
    const SLOW_FRAME_BUDGET: Duration = Duration::from_millis(16);

    /// Name of the cumulative slow-frame counter registered on
    /// [`Self::viewer_metrics`]. Incremented once per presented frame whose
    /// in-flight time exceeded [`Self::SLOW_FRAME_BUDGET`].
    const SLOW_FRAME_METRIC: &'static str = "session.event_loop.slow_frames";

    /// Recover from a hung render thread: if a job has been in flight longer than
    /// [`Self::RENDER_WATCHDOG_TIMEOUT`], release the in-flight flag, log a
    /// warning, and mark the frame dirty so a fresh job is submitted. This stops
    /// the main loop from spin-yielding at 100% CPU forever when the worker
    /// stalls without panicking (which would otherwise disconnect the channel).
    ///
    /// Returns `true` if the watchdog fired.
    fn check_render_watchdog(&mut self) -> bool {
        if !self.render_in_flight {
            return false;
        }
        let Some(since) = self.render_inflight_since else {
            return false;
        };
        if since.elapsed() < Self::RENDER_WATCHDOG_TIMEOUT {
            return false;
        }

        tracing::warn!(
            elapsed_ms = format!("{:.0}", since.elapsed().as_secs_f64() * 1000.0),
            "render thread watchdog fired: render job stuck in flight; \
             releasing and re-marking dirty"
        );
        self.render_in_flight = false;
        self.render_inflight_since = None;
        self.mark_full_dirty();
        true
    }

    /// Ensure the slow-frame telemetry counter is registered on the viewer
    /// metrics registry. Idempotent — `register` is a no-op if the metric
    /// already exists, so this is safe to call once per `run()`.
    fn register_slow_frame_metric(&self) {
        self.viewer_metrics.register(
            Self::SLOW_FRAME_METRIC,
            liquide_telemetry_viewer::metrics::MetricKind::Counter,
        );
    }

    /// Record a just-presented frame's in-flight duration against the slow-frame
    /// budget. If it exceeded [`Self::SLOW_FRAME_BUDGET`], bump the cumulative
    /// counter and emit a `warn` so a regression that re-introduces multi-frame
    /// stalls is observable in logs and telemetry (t77-A3).
    ///
    /// `inflight` is the time the frame spent in flight (submit -> present),
    /// captured by the caller BEFORE `try_present` consumes and clears
    /// `render_inflight_since` (which is owned by the render-thread module and
    /// must not be touched here).
    ///
    /// Returns `true` if the frame was counted as slow (used by tests).
    fn record_frame_telemetry(&self, inflight: Duration) -> bool {
        if inflight <= Self::SLOW_FRAME_BUDGET {
            return false;
        }
        self.viewer_metrics.increment(Self::SLOW_FRAME_METRIC, 1);
        let count = self
            .viewer_metrics
            .get(Self::SLOW_FRAME_METRIC)
            .unwrap_or(0);
        tracing::warn!(
            frame_ms = format!("{:.1}", inflight.as_secs_f64() * 1000.0),
            budget_ms = Self::SLOW_FRAME_BUDGET.as_millis(),
            slow_frames_total = count,
            "slow frame: render exceeded budget"
        );
        true
    }

    /// Deepest the no-event idle sleep is allowed to go (t77-A3).
    ///
    /// `platform.poll_event()` in this loop is a NON-BLOCKING poll (verified
    /// across every backend: win32 pumps the message queue then drains, x11 /
    /// wayland dispatch-pending then drain, standalone / macos pop a queue — none
    /// block on input; the separate `wait_event()` is the blocking variant and is
    /// NOT used here). Because the poll never wakes on input, this sleep directly
    /// bounds worst-case input-pickup latency: an event that lands the instant we
    /// go to sleep is not seen until we wake. We therefore keep the cap MODEST
    /// (24ms, ~1.5 frames at 60fps) rather than aggressive — a fully idle desktop
    /// drops CPU, but a click is still picked up within ~24ms worst case. The
    /// `had_event` branch stays at <=1ms so an actively-used desktop has no added
    /// latency at all.
    const IDLE_SLEEP_MAX: Duration = Duration::from_millis(24);

    /// Choose the idle sleep for the `!render_in_flight && !awaiting_ack` case
    /// (the steady-state tail of the run loop). Pure so it is unit-testable
    /// independently of the platform backend (t77-A3).
    ///
    /// Contract:
    /// - `dirty && !frame_interval.is_zero()`: throttled — sleep the remaining
    ///   time until the next frame is due (never longer than one frame).
    /// - `!dirty && had_event`: we just processed input this iteration, so more
    ///   may be arriving — sleep at most 1ms to pick it up immediately.
    /// - `!dirty && !had_event`: fully idle — sleep up to [`Self::IDLE_SLEEP_MAX`]
    ///   to drop CPU, clamped to `frame_interval` so an uncapped (0) interval
    ///   still floors at 1ms.
    /// - otherwise (`dirty && frame_interval.is_zero()`): no throttle, render
    ///   immediately — zero sleep.
    fn idle_sleep(
        dirty: bool,
        had_event: bool,
        frame_interval: Duration,
        last_render_elapsed: Duration,
    ) -> Duration {
        if dirty && !frame_interval.is_zero() {
            // Dirty but throttled — sleep until the next frame is due.
            frame_interval.saturating_sub(last_render_elapsed)
        } else if !dirty {
            if had_event {
                // Input was processed this iteration — stay hot for the next one.
                // DO NOT raise this above 1ms: with a non-blocking poll it is the
                // active-use input latency floor.
                Duration::from_millis(1)
            } else {
                // Fully idle: sleep up to IDLE_SLEEP_MAX to shed CPU, but clamp to
                // the frame interval so a 0 (uncapped) interval floors at 1ms.
                let cap = Self::IDLE_SLEEP_MAX.as_millis() as u64;
                Duration::from_millis(frame_interval.as_millis().clamp(1, cap as u128) as u64)
            }
        } else {
            // dirty && frame_interval.is_zero() — no throttle, render now.
            Duration::ZERO
        }
    }

    /// Run the desktop event loop using the given platform backend.
    ///
    /// Detects the primary screen size, creates a borderless fullscreen
    /// window, shows a polished loading overlay, then enters a
    /// non-blocking poll loop that:
    /// - Drains all pending platform events each iteration.
    /// - Runs periodic ticks (clock, notifications) every ~1s.
    /// - Re-renders when dirty (throttled by `frame_interval`).
    /// - Sleeps briefly when idle.
    pub fn run(&mut self, platform: &mut dyn PlatformBackend) {
        let run_start = Instant::now();

        // Detect the actual primary screen size and resize the compositor
        // to match so the framebuffer covers the full display.
        // In dev mode, keep the requested resolution for windowed mode.
        if !self.dt.dev_mode {
            let screen_rect = platform.display().virtual_screen_rect();
            let screen_w = screen_rect.width as u32;
            let screen_h = screen_rect.height as u32;
            if screen_w > 0 && screen_h > 0 && (screen_w != self.width || screen_h != self.height) {
                info!(
                    old_w = self.width,
                    old_h = self.height,
                    new_w = screen_w,
                    new_h = screen_h,
                    "resizing compositor to match primary screen"
                );
                self.width = screen_w;
                self.height = screen_h;
                if let Some(ref mut compositor) = self.compositor {
                    let _ = compositor.resize(screen_w, screen_h);
                }
                self.shell.resize_screen(screen_w as f32, screen_h as f32);
                self.cursor.x = screen_w as f32 / 2.0;
                self.cursor.y = screen_h as f32 / 2.0;
            }
        }

        // Read the real platform monitor set and install the multi-monitor
        // DesktopLayout on the shell (t73-multimon §3.1). This makes per-monitor
        // chrome/work-area reservations and real MoveToMonitor live. A single
        // monitor (or headless Null backend) yields a single-monitor layout that
        // behaves exactly as the legacy single-screen path.
        if !self.dt.dev_mode {
            self.install_desktop_layout(platform);
        }

        // Create a borderless fullscreen desktop window, or a resizable
        // windowed mode when dev_mode is active.
        debug!("creating desktop window {}x{}", self.width, self.height);
        let t_win = Instant::now();
        let params = if self.dt.dev_mode {
            // Dev mode: create a normal resizable window at the requested
            // size (not fullscreen) so the desktop can be inspected alongside
            // other host windows.
            info!("dev mode: creating resizable windowed compositor");
            NativeWindowParams {
                title: "Liquide Desktop [DEV]".to_string(),
                geometry: Rect::new(40.0, 40.0, self.width as f32, self.height as f32),
                window_type: "normal".to_string(),
                parent: None,
                app_id: "com.liquide.desktop.dev".to_string(),
            }
        } else {
            NativeWindowParams {
                title: "Liquide Desktop".to_string(),
                geometry: Rect::new(0.0, 0.0, self.width as f32, self.height as f32),
                window_type: "desktop".to_string(),
                parent: None,
                app_id: "com.liquide.desktop".to_string(),
            }
        };
        if let Ok(handle) = platform.window_host().create_window(params) {
            self.window_handle = Some(handle);
        }
        info!(
            width = self.width,
            height = self.height,
            windowed = self.dt.dev_mode,
            elapsed_ms = format!("{:.1}", t_win.elapsed().as_secs_f64() * 1000.0),
            "desktop window created"
        );

        // Probe for hardware cursor support. If the platform supports it,
        // we skip software cursor rendering entirely — zero CPU for mouse moves.
        if let Some(handle) = self.window_handle {
            let shape = self.shell.cursor_shape();
            if platform.set_cursor_shape(handle, shape.css_name()) {
                self.cursor.use_hardware = true;
                info!("hardware cursor enabled — zero-cost mouse movement");
            }
        }

        // Show loading overlay (synchronous — render thread not spawned yet).
        debug!("rendering loading overlay");
        self.loading = true;
        self.render_frame_sync(platform);
        let _ = self.wait_for_present_ready(platform, "loading overlay");
        info!(
            elapsed_ms = format!("{:.1}", run_start.elapsed().as_secs_f64() * 1000.0),
            "loading overlay presented"
        );

        // Drain any initial window events (WM_SIZE, WM_PAINT, etc.) that
        // fired during window creation so we have the correct client area
        // before rendering the first desktop frame.
        while let Some(event) = platform.poll_event() {
            self.handle_event(&event);
        }

        // Transition from loading to desktop.
        debug!("rendering first desktop frame");
        self.loading = false;
        // Seed the shell clock (and other periodic state) from the REAL
        // wall-clock BEFORE the first desktop frame is rendered. `tick()` reads
        // `SystemTime::now()` and drives `Shell::tick_detailed(now_us)`, which
        // updates the status-bar clock item from its constructed epoch default
        // (`last_update_us == 0`, which formats as 00:00) to the current local
        // time. Without this seed the first frame — and every frame until the
        // first ~1s periodic timer tick fires below — would render the Unix
        // epoch ("00:00"). The periodic timer keeps it advancing thereafter.
        //
        // DETERMINISM: this wall-clock seed lives ONLY on the real `run()`
        // path. The headless capture path
        // (`render_thread.rs::capture_once_scripted_with`) deliberately does
        // NOT call `tick()`; it renders at time `t0` and lets tests inject time
        // explicitly via the `mutate` closure, so goldens stay deterministic.
        self.tick();
        self.dirty = true;
        self.render_frame_sync(platform);
        let _ = self.wait_for_present_ready(platform, "initial desktop frame");
        self.dirty = false;
        info!(
            elapsed_ms = format!("{:.1}", run_start.elapsed().as_secs_f64() * 1000.0),
            "first desktop frame presented"
        );

        // Spawn the background render thread now that loading is done.
        self.spawn_render_thread();

        // Register the slow-frame telemetry counter so the present path can
        // record budget overruns (t77-A3). Idempotent.
        self.register_slow_frame_metric();

        // Non-blocking event loop with threaded rendering.
        info!(
            fps_cap = if self.frame_interval.is_zero() {
                0
            } else {
                (1_000_000 / self.frame_interval.as_micros().max(1)) as u32
            },
            debug_perf = self.debug_perf,
            "entering threaded event loop"
        );

        while self.running {
            let _ = self.refresh_present_pacing(platform);

            // Drain all pending events.
            let mut had_event = false;
            while let Some(event) = platform.poll_event() {
                had_event = true;
                if self.handle_event(&event) {
                    self.mark_full_dirty();
                }
            }

            // Consume any host-side requests the shell recorded this iteration
            // (t73-session items 2 & 3). The shell only RECORDS intent
            // (pending_session_request / pending_screenshot); the host performs
            // the effect. Session-lifecycle actions may set `quit_requested`
            // (handled by the flush-and-exit path just below); the screenshot
            // request is fulfilled from the last presented framebuffer (a PNG on
            // disk).
            let _ = self.consume_session_request();
            let _ = self.consume_screenshot_request();

            // Honour a pending quit ONLY after flushing the final frame
            // (t60-runtime #1). A Quit/close event sets `quit_requested` rather
            // than stopping the loop outright, so any in-flight render job is
            // presented before exit — without this the last desktop frame is
            // orphaned and the window flashes black/stale on close.
            if self.quit_requested {
                self.flush_pending_present_for_quit(platform);
                self.running = false;
                break;
            }

            // Sync hardware cursor shape to the platform backend.
            // This is a cheap Win32 SetCursor call — no rendering needed.
            if let Some(shape) = self.cursor.consume_hw_sync() {
                if let Some(handle) = self.window_handle {
                    platform.set_cursor_shape(handle, shape.css_name());
                }
            }

            // When hardware cursor is active, cursor-only moves are free —
            // the OS draws the cursor. Discard cursor_dirty entirely.
            if self.cursor.use_hardware {
                self.cursor.dirty = false;
            }

            // Watchdog: recover from a hung (non-panicking) render thread.
            // Without this, a stuck worker leaves `render_in_flight` true
            // forever and the main loop spin-yields at 100% CPU indefinitely
            // (t60-runtime #3). After the timeout we release the in-flight flag,
            // mark the frame dirty, and let the loop submit a fresh job.
            self.check_render_watchdog();

            // Check for completed frames from the render thread.
            //
            // Capture the in-flight duration BEFORE `try_present` — it consumes
            // the frame and clears `render_inflight_since` (owned by the
            // render-thread module). If a frame is presented this iteration,
            // record it against the slow-frame budget for telemetry (t77-A3).
            let inflight_before_present = self.render_inflight_since.map(|s| s.elapsed());
            if self.try_present(platform) {
                if let Some(inflight) = inflight_before_present {
                    self.record_frame_telemetry(inflight);
                }
                self.last_render = Instant::now();
                // If still dirty (events arrived during rendering),
                // submit a new render job immediately.
                if self.dirty && !self.present_pacing.awaiting_ack {
                    self.submit_render();
                    self.dirty = false;
                    self.cursor.dirty = false;
                } else if self.cursor.dirty && !self.present_pacing.awaiting_ack {
                    self.submit_cursor_only_render();
                    self.cursor.dirty = false;
                }
            }

            // Periodic tick every ~1s for clock / notification expiry.
            // Periodic telemetry report every ~10s.
            // Both are driven by TimerManager from liquide-message-queue.
            for action in self.paint.check_timers() {
                match action {
                    TimerAction::Tick => {
                        if self.tick() {
                            self.dirty = true;
                        }
                    }
                    TimerAction::TelemetryReport => {
                        self.print_telemetry_report();
                    }
                }
            }

            // Submit a render job if dirty and render thread is free.
            if self.dirty && !self.render_in_flight && !self.present_pacing.awaiting_ack {
                // During drag, bypass frame interval throttle for immediate
                // visual feedback — the blur suppression keeps frame cost low.
                let can_render = self.shell.is_dragging()
                    || self.frame_interval.is_zero()
                    || self.last_render.elapsed() >= self.frame_interval;
                if can_render {
                    self.submit_render();
                    self.dirty = false;
                    self.cursor.dirty = false;
                }
            } else if self.cursor.dirty
                && !self.dirty
                && !self.render_in_flight
                && !self.present_pacing.awaiting_ack
            {
                // Cursor moved but nothing else changed — use fast path
                // that reuses the cached scene without running the CSS pipeline.
                let can_render = self.frame_interval.is_zero()
                    || self.last_render.elapsed() >= self.frame_interval;
                if can_render {
                    self.submit_cursor_only_render();
                    self.cursor.dirty = false;
                }
            }

            // Efficient idle with adaptive precision.
            let target_sleep = if self.render_in_flight || self.present_pacing.awaiting_ack {
                // Render in progress — brief yield to check for completion.
                // (Left at 100us deliberately: changing it risks present cadence.)
                Duration::from_micros(100)
            } else {
                // Steady-state idle selection, factored into a pure helper so the
                // dirty/had_event/idle branches stay unit-testable (t77-A3). The
                // no-event branch shedds CPU by sleeping up to IDLE_SLEEP_MAX
                // (24ms); the had_event branch stays at <=1ms so an actively-used
                // desktop keeps immediate input pickup with a non-blocking poll.
                Self::idle_sleep(
                    self.dirty,
                    had_event,
                    self.frame_interval,
                    self.last_render.elapsed(),
                )
            };

            // For sub-millisecond sleeps, use spin-wait instead of OS sleep
            // (OS scheduler can't reliably sleep < 1ms on most platforms).
            if target_sleep <= Duration::from_micros(500) {
                if target_sleep > Duration::ZERO {
                    let deadline = Instant::now() + target_sleep;
                    while Instant::now() < deadline {
                        std::hint::spin_loop();
                    }
                }
            } else {
                thread::sleep(target_sleep);
            }
        }

        // Shut down render thread.
        if let Some(ref tx) = self.render_tx {
            let _ = tx.send(RenderMsg::Shutdown);
        }
        if let Some(handle) = self.render_thread.take() {
            let _ = handle.join();
        }
        info!("render thread joined");

        // Shut down per-window render threads.
        self.window_render.shutdown_all();

        // Clean up the window on exit.
        if let Some(handle) = self.window_handle.take() {
            let _ = platform.window_host().destroy_window(handle);
        }

        info!(
            total_frames = self.frame_count,
            uptime_s = format!("{:.1}", run_start.elapsed().as_secs_f64()),
            "event loop exited"
        );
    }
}

#[cfg(test)]
mod watchdog_tests {
    use super::*;

    #[test]
    fn watchdog_fires_after_timeout_and_recovers() {
        // REGRESSION (t60-runtime #3): a hung render thread leaves
        // render_in_flight stuck true; the watchdog must release it so the loop
        // does not spin at 100% CPU forever.
        let mut desktop = DesktopCompositor::new(64, 64);
        desktop.render_in_flight = true;
        desktop.render_inflight_since =
            Some(Instant::now() - DesktopCompositor::RENDER_WATCHDOG_TIMEOUT - Duration::from_millis(50));
        desktop.dirty = false;

        assert!(desktop.check_render_watchdog(), "watchdog should fire");
        assert!(!desktop.render_in_flight, "in-flight flag must be released");
        assert!(desktop.render_inflight_since.is_none());
        assert!(desktop.dirty, "frame must be re-marked dirty for re-submit");
    }

    #[test]
    fn watchdog_does_not_fire_before_timeout() {
        let mut desktop = DesktopCompositor::new(64, 64);
        desktop.render_in_flight = true;
        desktop.render_inflight_since = Some(Instant::now());

        assert!(!desktop.check_render_watchdog(), "watchdog must not fire early");
        assert!(desktop.render_in_flight, "in-flight flag must be preserved");
    }

    #[test]
    fn watchdog_noop_when_nothing_in_flight() {
        let mut desktop = DesktopCompositor::new(64, 64);
        desktop.render_in_flight = false;
        desktop.render_inflight_since = None;

        assert!(!desktop.check_render_watchdog());
    }

    #[test]
    fn watchdog_timeout_is_lowered_to_150ms() {
        // GUARD (t77-A3): the watchdog interval was tightened 500ms -> 150ms to
        // recover a hung worker ~3.3x sooner. This pins the value so a revert to
        // the old 500ms floor fails here.
        assert_eq!(
            DesktopCompositor::RENDER_WATCHDOG_TIMEOUT,
            Duration::from_millis(150),
            "render watchdog must use the tightened 150ms timeout"
        );
        assert!(
            DesktopCompositor::RENDER_WATCHDOG_TIMEOUT < Duration::from_millis(500),
            "watchdog must be tighter than the legacy 500ms floor"
        );
    }

    #[test]
    fn slow_frame_telemetry_counts_only_budget_overruns() {
        // REGRESSION (t77-A3): a frame slower than SLOW_FRAME_BUDGET must bump
        // the slow-frame counter; a fast frame must NOT. The test fails if the
        // counter is mis-wired (counts everything, or counts nothing).
        let desktop = DesktopCompositor::new(64, 64);
        desktop.register_slow_frame_metric();

        // Baseline: counter registered, starts at zero.
        assert_eq!(
            desktop.viewer_metrics.get(DesktopCompositor::SLOW_FRAME_METRIC),
            Some(0),
            "slow-frame counter must be registered and start at zero"
        );

        // A frame comfortably inside budget must NOT be counted.
        let fast = DesktopCompositor::SLOW_FRAME_BUDGET / 4;
        assert!(
            !desktop.record_frame_telemetry(fast),
            "a fast frame must not be counted slow"
        );
        assert_eq!(
            desktop.viewer_metrics.get(DesktopCompositor::SLOW_FRAME_METRIC),
            Some(0),
            "fast frame must leave the counter at zero"
        );

        // A frame exceeding budget MUST be counted.
        let slow = DesktopCompositor::SLOW_FRAME_BUDGET + Duration::from_millis(10);
        assert!(
            desktop.record_frame_telemetry(slow),
            "a frame over budget must be counted slow"
        );
        assert_eq!(
            desktop.viewer_metrics.get(DesktopCompositor::SLOW_FRAME_METRIC),
            Some(1),
            "slow frame must increment the counter to 1"
        );

        // A second slow frame accumulates.
        assert!(desktop.record_frame_telemetry(slow));
        assert_eq!(
            desktop.viewer_metrics.get(DesktopCompositor::SLOW_FRAME_METRIC),
            Some(2),
            "slow frames must accumulate"
        );
    }

    #[test]
    fn slow_frame_budget_is_one_60fps_frame() {
        // The slow-frame budget is the 60fps frame interval (16ms): the
        // responsiveness floor we want regressions to trip on.
        assert_eq!(
            DesktopCompositor::SLOW_FRAME_BUDGET,
            Duration::from_millis(16)
        );
    }

    #[test]
    fn quit_event_requests_shutdown_without_stopping_loop_immediately() {
        // REGRESSION (t60-runtime #1): Quit must NOT stop the loop outright —
        // it requests shutdown so the final frame can flush first. `running`
        // stays true; only `quit_requested` is set.
        let mut desktop = DesktopCompositor::new(64, 64);
        desktop.loading = false;
        assert!(desktop.running);
        assert!(!desktop.quit_requested);

        desktop.handle_event(&liquide_platform::PlatformEvent::Quit);

        assert!(
            desktop.running,
            "loop must keep running until the final frame is flushed"
        );
        assert!(desktop.quit_requested, "quit must be recorded as requested");
    }

    // ---- idle-sleep selection (t77-A3) -------------------------------------

    /// One 60fps frame, the interval the real loop runs with by default.
    const FI_60: Duration = Duration::from_millis(16);

    #[test]
    fn idle_sleep_had_event_is_immediate_pickup() {
        // TOOTH (t77-A3): with a NON-BLOCKING poll, the no-render had_event sleep
        // bounds active-use input latency, so it must stay <=1ms. If someone
        // raises this branch to the deeper idle cap, this fails.
        let s = DesktopCompositor::idle_sleep(
            /*dirty*/ false,
            /*had_event*/ true,
            FI_60,
            /*last_render_elapsed*/ Duration::ZERO,
        );
        assert!(
            s <= Duration::from_millis(1),
            "had_event idle sleep must stay <=1ms for immediate input pickup, got {s:?}"
        );
    }

    #[test]
    fn idle_sleep_no_event_uses_deeper_sleep() {
        // TOOTH (t77-A3): the fully-idle (!dirty && !had_event) branch must use
        // the deeper sleep to shed CPU — strictly longer than the 1ms had_event
        // floor — but never exceed the conservative IDLE_SLEEP_MAX (24ms) input
        // latency cap. At a 16ms frame interval the result is 16ms (clamp keeps
        // it under the 24ms cap). Fails if the deeper branch is reverted to the
        // had_event 1ms floor.
        let idle = DesktopCompositor::idle_sleep(false, false, FI_60, Duration::ZERO);
        let active = DesktopCompositor::idle_sleep(false, true, FI_60, Duration::ZERO);
        assert!(
            idle > active,
            "idle (no-event) sleep must be deeper than had_event sleep: idle={idle:?} active={active:?}"
        );
        assert!(
            idle <= DesktopCompositor::IDLE_SLEEP_MAX,
            "idle sleep must never exceed the 24ms input-latency cap, got {idle:?}"
        );
    }

    #[test]
    fn idle_sleep_no_event_cap_is_24ms() {
        // TOOTH (t77-A3): the no-event cap was raised 16 -> 24ms. With a frame
        // interval larger than the cap, the clamp must pin the sleep at exactly
        // 24ms. Fails if IDLE_SLEEP_MAX is reverted to 16ms (would yield 16).
        let big_interval = Duration::from_millis(1000);
        let s = DesktopCompositor::idle_sleep(false, false, big_interval, Duration::ZERO);
        assert_eq!(
            s,
            Duration::from_millis(24),
            "fully-idle sleep must clamp to the 24ms cap"
        );
        assert_eq!(
            DesktopCompositor::IDLE_SLEEP_MAX,
            Duration::from_millis(24),
            "IDLE_SLEEP_MAX must be the raised 24ms cap"
        );
    }

    #[test]
    fn idle_sleep_no_event_floors_at_1ms_when_uncapped() {
        // A 0 (uncapped) frame interval must still floor the idle sleep at 1ms,
        // not 0 — otherwise a fully idle uncapped desktop would busy-spin.
        let s = DesktopCompositor::idle_sleep(false, false, Duration::ZERO, Duration::ZERO);
        assert_eq!(s, Duration::from_millis(1));
    }

    #[test]
    fn idle_sleep_dirty_throttled_waits_until_next_frame() {
        // Dirty + a real frame interval: sleep the REMAINING time until the next
        // frame is due (interval - elapsed), never longer than one frame.
        let half = FI_60 / 2;
        let s = DesktopCompositor::idle_sleep(/*dirty*/ true, false, FI_60, half);
        assert_eq!(s, FI_60 - half, "must wait the remaining frame budget");

        // Past the interval: nothing left to wait, render now.
        let s = DesktopCompositor::idle_sleep(true, false, FI_60, FI_60 * 2);
        assert_eq!(s, Duration::ZERO, "overdue frame must not sleep");
    }

    #[test]
    fn idle_sleep_dirty_uncapped_renders_immediately() {
        // Dirty + uncapped (0) interval: no throttle, zero sleep.
        let s = DesktopCompositor::idle_sleep(true, false, Duration::ZERO, Duration::ZERO);
        assert_eq!(s, Duration::ZERO);
    }
}
