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

    /// Decide how to mark the desktop dirty after an event was handled.
    ///
    /// Background: every event that returns `need_redraw` used to call
    /// [`Self::mark_full_dirty`], which drops any targeted damage hint to `None`.
    /// On the live threaded path a `None` hint forces the per-frame full-scene
    /// diff to be the *only* thing that can avoid a ~300ms full-frame raster —
    /// and whenever the diff yields `None`/empty the frame falls all the way to
    /// full (residual interactive lag, t79 Bug 2 #1).
    ///
    /// For pure hover (`MouseEvent::Move`) interactions the shell already knows
    /// exactly which interactive-overlay rects can change (the open menu panel,
    /// the dock band, a hovered titlebar button). We plumb that as a real
    /// targeted damage hint so the hover frame carries a small, explicit damage
    /// set instead of falling to the full path. The hint is unioned across the
    /// overlay footprint BEFORE and AFTER the event (so a hover that moves the
    /// highlight off one item / dismisses a panel still repaints the vacated
    /// pixels) and the render thread further UNIONs it with the scene diff
    /// (`render_thread.rs`), so it can only ever ADD damage, never narrow it.
    ///
    /// Every other event kind (button/scroll/key/resize/redraw, and any move
    /// that is not confined to a known overlay) keeps the conservative
    /// full-dirty path — a click can open a window, start a drag, swap themes,
    /// etc., which the overlay hint does not bound.
    pub(super) fn mark_dirty_for_event(
        &mut self,
        event: &liquide_platform::PlatformEvent,
        overlay_before: Vec<Rect>,
    ) {
        use liquide_input::mouse::MouseEvent;
        use liquide_platform::PlatformEvent;

        let is_hover_move = matches!(
            event,
            PlatformEvent::MouseInput {
                event: MouseEvent::Move { .. },
                ..
            }
        );

        if !is_hover_move {
            self.mark_full_dirty();
            return;
        }

        // Combine the pre- and post-event overlay footprints. The post-event
        // footprint reflects the new menu/dock/titlebar hover state; the
        // pre-event one covers anything the move just left (the
        // previously-hovered item's vacated region). Each rect is marked
        // independently (the shell returns a disjoint SET, not one bbox) so the
        // empty space between a top menu and the bottom dock is never repainted.
        let overlay_after = self.shell.interactive_overlay_damage();

        // No menu open before OR after this move => we cannot bound the change;
        // keep the conservative full repaint (matches the legacy behavior for
        // ordinary hovers, e.g. one that surfaces a tooltip).
        if overlay_before.is_empty() && overlay_after.is_empty() {
            self.mark_full_dirty();
            return;
        }

        for rect in overlay_before.into_iter().chain(overlay_after) {
            self.mark_rect_dirty(rect);
        }
    }

    /// Handle a single platform event drained from the backend. Shared by the
    /// non-blocking drain at the top of the loop and the event-driven timed
    /// wait at the tail (so an event that wakes the idle park is handled with
    /// exactly the same routing — including the overlay-damage hint and the
    /// monitor-hotplug special case).
    fn dispatch_platform_event(
        &mut self,
        platform: &mut dyn PlatformBackend,
        event: liquide_platform::PlatformEvent,
    ) {
        // Monitor hotplug (t93 gap #5c): `DisplaysChanged` needs the live
        // platform handle to re-enumerate displays, which `handle_event` does
        // not receive — so it is handled here (which owns `platform`). The
        // handler re-installs the layout, migrates stranded windows, resizes to
        // the new primary, and marks dirty.
        if matches!(event, liquide_platform::PlatformEvent::DisplaysChanged) {
            if self.handle_displays_changed(platform) {
                self.mark_full_dirty();
            }
            return;
        }
        // Snapshot the interactive-overlay footprint BEFORE handling the event
        // so a hover that moves/closes a menu can union the OLD and NEW
        // footprints (the disappearing panel's pixels must be in the damage
        // hint or they go stale — t80-hint).
        let overlay_before = self.shell.interactive_overlay_damage();
        if self.handle_event(&event) {
            self.mark_dirty_for_event(&event, overlay_before);
        }
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

    /// Deepest the fully-idle event-driven park is allowed to go (t97-wakeup).
    ///
    /// The run loop no longer relies on a non-blocking poll + sleep for idle
    /// wakeups: it parks on [`PlatformBackend::wait_event_timeout`], which wakes
    /// the INSTANT real input arrives (Win32 `MsgWaitForMultipleObjectsEx`).
    /// Because input no longer has to wait out the timeout, this cap does NOT
    /// bound input-pickup latency (that is now sub-ms / event-driven). It only
    /// bounds how stale a *timer-driven* update (the ~1s clock tick, notification
    /// expiry) may get when the desktop is otherwise idle. 100ms keeps the clock
    /// and expiry comfortably timely (well under the 1s tick) while letting the
    /// CPU drop to ~0% — the thread is parked in the OS wait, not spinning.
    const IDLE_WAIT_MAX: Duration = Duration::from_millis(100);

    /// Choose how long to PARK on [`PlatformBackend::wait_event_timeout`] for the
    /// `!render_in_flight && !awaiting_ack` case (the steady-state tail of the
    /// run loop). Pure so it is unit-testable independently of the platform
    /// backend (t97-wakeup).
    ///
    /// The returned value is a *parking budget*, not a fixed sleep: the timed
    /// wait returns immediately when an event lands, so a longer budget never
    /// adds latency — it only bounds how long the loop waits when NOTHING
    /// happens. Contract:
    /// - `dirty && !frame_interval.is_zero()`: throttled — wait the remaining
    ///   time until the next frame is due (never longer than one frame). A new
    ///   event still wakes us early.
    /// - `dirty && frame_interval.is_zero()`: no throttle — return ZERO so the
    ///   ready/active frame is submitted and presented ASAP with NO wait (this
    ///   is what removes the old 1ms active wakeup floor).
    /// - `!dirty`: idle (whether or not input was just processed) — park up to
    ///   [`Self::IDLE_WAIT_MAX`]. The timed wait wakes immediately on the next
    ///   event, so there is no input-latency penalty and no busy-spin: the
    ///   thread is blocked in the OS wait, not looping. `frame_interval` clamps
    ///   the lower bound so a finite interval shorter than the cap still bounds
    ///   timer latency to one frame.
    fn wait_budget(
        dirty: bool,
        frame_interval: Duration,
        last_render_elapsed: Duration,
    ) -> Duration {
        if dirty {
            if frame_interval.is_zero() {
                // Uncapped + dirty — present ASAP, no wait. (No 1ms floor.)
                Duration::ZERO
            } else {
                // Dirty but throttled — wait until the next frame is due.
                frame_interval.saturating_sub(last_render_elapsed)
            }
        } else {
            // Idle: park on events. The timed wait wakes immediately on input,
            // so this is the CPU-shedding park, NOT an input-latency cap. Clamp
            // to the frame interval when finite so timer-driven updates stay at
            // most one frame stale; otherwise park up to IDLE_WAIT_MAX.
            if frame_interval.is_zero() || frame_interval >= Self::IDLE_WAIT_MAX {
                Self::IDLE_WAIT_MAX
            } else {
                frame_interval
            }
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

            // Drain all pending events (non-blocking) so a burst of input is
            // processed in one iteration before we decide whether to park.
            while let Some(event) = platform.poll_event() {
                self.dispatch_platform_event(platform, event);
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

            // Event-driven wakeup with adaptive precision (t97-wakeup).
            //
            // We compute a PARK BUDGET and hand it to the platform's TIMED wait
            // (`wait_event_timeout`), which returns the INSTANT an event lands or
            // when the budget elapses — whichever first. This replaces the old
            // "compute a fixed sleep then sleep" tail, which had two costs the
            // timed wait removes: (1) a 1ms floor on active wakeups (capping the
            // loop at 1000fps even with a ready frame), and (2) an up-to-24ms
            // input-pickup spike at idle because the poll was non-blocking.
            let park = if self.render_in_flight || self.present_pacing.awaiting_ack {
                // Render in progress — short park to check for completion
                // promptly. (Kept at 100us: changing it risks present cadence.)
                // A real input event still wakes us immediately.
                Duration::from_micros(100)
            } else {
                // Steady-state budget, factored into a pure helper so the
                // dirty/throttle/idle branches stay unit-testable. ZERO when an
                // active/ready frame should present ASAP (no 1ms floor); a
                // CPU-shedding event park (up to IDLE_WAIT_MAX) when idle — the
                // timed wait makes that park wake on input with sub-ms latency,
                // so it is NOT a busy-spin and NOT an input-latency cap.
                Self::wait_budget(self.dirty, self.frame_interval, self.last_render.elapsed())
            };

            // ACTIVE/READY (park == ZERO): do NOT spin and do NOT block — fall
            // straight back to the top of the loop so the just-submitted /
            // ready frame is presented and the next is produced immediately.
            // IDLE/THROTTLED (park > ZERO): park on the platform's TIMED wait.
            // It blocks on the OS event primitive (CPU ~0%) and returns the
            // instant input arrives; if it yields an event, dispatch it right
            // here so it is not lost (the next iteration then renders it).
            if !park.is_zero() {
                if let Some(event) = platform.wait_event_timeout(park) {
                    self.dispatch_platform_event(platform, event);
                }
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
    fn default_submit_cadence_is_uncapped_no_60fps_ceiling() {
        // ANTI-FAKE-GREEN (snappy lever #1): the desktop must boot UNCAPPED so a
        // ready frame presents immediately instead of waiting out a 16.67ms
        // (60fps) interval. This test fails if the artificial 60fps submit cap is
        // reinstated (DEFAULT_TARGET_FPS back to 60 → a 16ms frame_interval).
        let desktop = DesktopCompositor::new(64, 64);
        assert!(
            desktop.frame_interval.is_zero(),
            "default frame_interval must be zero (uncapped) — got {:?}; a non-zero \
             interval reinstates the artificial submit cap",
            desktop.frame_interval
        );
    }

    #[test]
    fn ready_frame_submits_without_waiting_a_frame_interval() {
        // ANTI-FAKE-GREEN: with the cap removed, a frame that became dirty THIS
        // INSTANT (last_render just now, zero elapsed) must be allowed to submit
        // immediately — i.e. the throttle gate from the run loop
        // (frame_interval.is_zero() || last_render.elapsed() >= frame_interval)
        // is satisfied with no wait. Mirrors the gate at event_loop.rs:509-511.
        let desktop = DesktopCompositor::new(64, 64);
        let elapsed_now = Duration::ZERO; // a frame ready the instant it was rendered
        let can_render_immediately =
            desktop.frame_interval.is_zero() || elapsed_now >= desktop.frame_interval;
        assert!(
            can_render_immediately,
            "a ready frame must submit immediately under the uncapped cadence, \
             not wait out a 16.67ms interval"
        );

        // A READY/active frame (dirty + uncapped) must present ASAP with NO
        // wait at all — this is what removes the old 1ms active wakeup floor
        // that capped the active loop at 1000fps.
        let active = DesktopCompositor::wait_budget(
            /*dirty*/ true,
            desktop.frame_interval,
            Duration::ZERO,
        );
        assert_eq!(
            active,
            Duration::ZERO,
            "an active/ready frame must be scheduled with NO wait (no 1ms floor), got {active:?}"
        );

        // And it must NOT busy-spin while idle: the idle path parks on a real,
        // non-zero event-wait budget (a wait, not a spin) even when uncapped.
        let idle = DesktopCompositor::wait_budget(
            /*dirty*/ false,
            desktop.frame_interval,
            Duration::ZERO,
        );
        assert!(
            idle >= Duration::from_millis(1),
            "idle loop must park (not busy-spin) even when uncapped, got {idle:?}"
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

    // ---- wait-budget selection (t97-wakeup) --------------------------------

    /// One 60fps frame, a representative finite interval.
    const FI_60: Duration = Duration::from_millis(16);

    #[test]
    fn wait_budget_active_ready_frame_has_no_floor() {
        // ANTI-FAKE-GREEN (t97-wakeup): an active/ready frame (dirty + uncapped)
        // must be scheduled with a ZERO park — NO 1ms (or any) wait. This is the
        // core fix: the old loop floored every wakeup at >=1ms, capping the
        // active loop at 1000fps even with a frame ready right now. Fails if a
        // non-zero floor is reinstated for the active path.
        let s = DesktopCompositor::wait_budget(
            /*dirty*/ true,
            /*frame_interval*/ Duration::ZERO,
            Duration::ZERO,
        );
        assert_eq!(
            s,
            Duration::ZERO,
            "an active/ready frame must present ASAP with NO wait (no 1ms floor), got {s:?}"
        );
    }

    #[test]
    fn wait_budget_idle_parks_not_spins() {
        // ANTI-FAKE-GREEN (t97-wakeup): the fully-idle branch must yield a real,
        // NON-ZERO park budget so the loop BLOCKS on the platform's timed wait
        // (CPU ~0%) instead of busy-spinning. Fails if idle collapses to ZERO
        // (which would spin the loop) — true for both a finite and uncapped
        // frame interval.
        let idle_uncapped =
            DesktopCompositor::wait_budget(false, Duration::ZERO, Duration::ZERO);
        assert!(
            idle_uncapped >= Duration::from_millis(1),
            "idle (uncapped) must park, not spin: got {idle_uncapped:?}"
        );
        let idle_finite = DesktopCompositor::wait_budget(false, FI_60, Duration::ZERO);
        assert!(
            idle_finite >= Duration::from_millis(1),
            "idle (finite interval) must park, not spin: got {idle_finite:?}"
        );
    }

    #[test]
    fn wait_budget_idle_caps_at_idle_wait_max() {
        // The idle park is bounded by IDLE_WAIT_MAX so a timer-driven update
        // (clock/notification expiry) can't get arbitrarily stale. With an
        // uncapped (or large) frame interval the budget pins at the cap exactly.
        let s = DesktopCompositor::wait_budget(false, Duration::ZERO, Duration::ZERO);
        assert_eq!(
            s,
            DesktopCompositor::IDLE_WAIT_MAX,
            "uncapped idle park must clamp to IDLE_WAIT_MAX"
        );
        let big = Duration::from_millis(1000);
        let s = DesktopCompositor::wait_budget(false, big, Duration::ZERO);
        assert_eq!(
            s,
            DesktopCompositor::IDLE_WAIT_MAX,
            "an idle interval larger than the cap must clamp to IDLE_WAIT_MAX"
        );
        assert_eq!(
            DesktopCompositor::IDLE_WAIT_MAX,
            Duration::from_millis(100),
            "IDLE_WAIT_MAX must be 100ms (timer-latency bound, well under the 1s tick)"
        );
    }

    #[test]
    fn wait_budget_idle_clamps_to_finite_interval() {
        // A finite frame interval SHORTER than the cap bounds the idle park to
        // one frame (so a capped desktop still produces timer updates promptly).
        let s = DesktopCompositor::wait_budget(false, FI_60, Duration::ZERO);
        assert_eq!(s, FI_60, "idle park must clamp to a sub-cap finite interval");
    }

    #[test]
    fn wait_budget_dirty_throttled_waits_until_next_frame() {
        // Dirty + a finite frame interval: park the REMAINING time until the
        // next frame is due (interval - elapsed), never longer than one frame.
        // An event still wakes the timed wait early.
        let half = FI_60 / 2;
        let s = DesktopCompositor::wait_budget(/*dirty*/ true, FI_60, half);
        assert_eq!(s, FI_60 - half, "must wait the remaining frame budget");

        // Past the interval: nothing left to wait, render now.
        let s = DesktopCompositor::wait_budget(true, FI_60, FI_60 * 2);
        assert_eq!(s, Duration::ZERO, "overdue frame must not wait");
    }

    #[test]
    fn wait_budget_dirty_uncapped_renders_immediately() {
        // Dirty + uncapped (0) interval: no throttle, zero park.
        let s = DesktopCompositor::wait_budget(true, Duration::ZERO, Duration::ZERO);
        assert_eq!(s, Duration::ZERO);
    }
}
