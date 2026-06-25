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
    /// Backward-compatible entry (no drag-footprint hint): a caller that does
    /// not snapshot the dragged window's old bounds gets the original
    /// overlay-only routing. The live loop calls the `_with_drag` form; this
    /// 2-arg shim only exists for the existing render-thread test callers, so it
    /// is `cfg(test)`-only to avoid a dead-code warning in the release build.
    #[cfg(test)]
    pub(super) fn mark_dirty_for_event(
        &mut self,
        event: &liquide_platform::PlatformEvent,
        overlay_before: Vec<Rect>,
    ) {
        self.mark_dirty_for_event_with_drag(event, overlay_before, None);
    }

    /// `mark_dirty_for_event` with an optional snapshot of the dragged window's
    /// footprint captured BEFORE the event (the OLD position/size). When present
    /// and this is a window MOVE *or* RESIZE drag-frame, the damage is confined to
    /// the old∪new window footprint instead of falling to the full-frame path
    /// (t127-drag-perf move; t135-resizedrag resize).
    pub(super) fn mark_dirty_for_event_with_drag(
        &mut self,
        event: &liquide_platform::PlatformEvent,
        overlay_before: Vec<Rect>,
        drag_window_before: Option<Rect>,
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

        // Window MOVE or RESIZE drag (t127-drag-perf move; t135-resizedrag
        // resize): a drag only relocates/resizes the dragged window, so the only
        // pixels that change are its OLD footprint (revealed/repainted — for a
        // SHRINKING resize this is the band that was inside the larger old window
        // and is now behind it) and its NEW footprint (painted). Confine the
        // frame's damage to that union (each rect shadow/blur-margined) instead
        // of falling to the ~300ms full-frame raster. The OLD footprint MUST be
        // included or the window's previous position/size leaves a stale ghost.
        // If either bound is unavailable, fall through to the existing path (no
        // regression). The render thread further UNIONs this with the scene diff,
        // so it can only ADD damage, never narrow it.
        if let Some(old_bounds) = drag_window_before {
            let drag_rects = self.shell.drag_damage(old_bounds);
            if !drag_rects.is_empty() {
                // Union with any open-menu overlay footprint so a drag with a
                // menu still open does not under-damage the menu band.
                for rect in drag_rects.into_iter().chain(overlay_before) {
                    self.mark_rect_dirty(rect);
                }
                let overlay_after = self.shell.interactive_overlay_damage();
                for rect in overlay_after {
                    self.mark_rect_dirty(rect);
                }
                return;
            }
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
        // SEPARATE DEVTOOLS WINDOW routing (dev-mode only): if this event
        // targets the devtools window's `handle`, it drives the devtools panel —
        // NOT the main DE. Consumed here so a click/key/scroll in the devtools
        // window can never reach the desktop shell. Falls through for the main
        // window (or any other handle).
        if self.try_handle_devtools_window_event(platform, &event) {
            return;
        }
        // Snapshot the interactive-overlay footprint BEFORE handling the event
        // so a hover that moves/closes a menu can union the OLD and NEW
        // footprints (the disappearing panel's pixels must be in the damage
        // hint or they go stale — t80-hint).
        let overlay_before = self.shell.interactive_overlay_damage();
        // Snapshot the dragged window's footprint BEFORE handling the event so a
        // window MOVE drag-frame can union the OLD position (which must be
        // repainted/revealed or it leaves a stale ghost) with the NEW position
        // (t127-drag-perf). `None` whenever a move-drag is not in progress, in
        // which case the conservative full-frame path is kept.
        let drag_window_before = self
            .shell
            .dragged_window()
            .and_then(|id| self.shell.window(id).ok())
            .map(|w| w.bounds);
        if self.handle_event(&event) {
            self.mark_dirty_for_event_with_drag(&event, overlay_before, drag_window_before);
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

            // SEPARATE DEVTOOLS WINDOW (dev-mode only). In order:
            //  1. follow the panel's visibility (F12 / Ctrl+Shift+I toggled it)
            //     into a detach / close request,
            //  2. reconcile that request by creating / destroying the native
            //     window (never leaking it),
            //  3. present a fresh devtools frame from the LIVE shell state.
            // All three are no-ops when dev mode is off or no window is open, so
            // the non-dev-mode in-DE overlay path is unaffected.
            self.dt.dev_mode_follow_visibility();
            self.dt.sync_window(platform);
            if self.dt.has_window() {
                self.dt.render_window(&self.shell, platform);
            }

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

        // Tear down the separate devtools window (dev-mode only) so it is never
        // leaked on exit.
        self.dt.close_window(platform);

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

#[cfg(test)]
mod drag_damage_tests {
    //! t127-drag-perf: a window MOVE drag must confine its frame damage to the
    //! dragged window's OLD∪NEW footprint (+ shadow/blur margin) instead of
    //! falling to the ~300ms full-frame raster. These tests fail if the drag
    //! damage is full-frame, omits the OLD footprint (→ stale ghost of the old
    //! position), or under-damages either rect.

    use super::*;
    use liquide_compositor::geometry::Point;

    const SCREEN_W: u32 = 1920;
    const SCREEN_H: u32 = 1080;

    fn move_event(x: f32, y: f32) -> liquide_platform::PlatformEvent {
        liquide_platform::PlatformEvent::MouseInput {
            handle: liquide_platform::NativeWindowHandle(0),
            event: liquide_input::mouse::MouseEvent::Move { x, y },
        }
    }

    /// Stand up a desktop with one window and begin a move-drag on it. Returns
    /// the desktop, the window id, and the grab point (top-left + a small inset
    /// so the offset is non-zero).
    fn desktop_mid_move_drag() -> (DesktopCompositor, liquide_shell::WindowId, Point) {
        let mut desktop = DesktopCompositor::new(SCREEN_W, SCREEN_H);
        desktop.loading = false;
        desktop.shell.resize_screen(SCREEN_W as f32, SCREEN_H as f32);

        let bounds = Rect::new(300.0, 200.0, 400.0, 300.0);
        let wid = desktop.shell.open_window("drag", bounds);
        let grab = Point::new(bounds.x + 20.0, bounds.y + 10.0);
        assert!(
            desktop.shell.begin_move_drag(wid, grab),
            "move-drag must start"
        );
        assert!(desktop.shell.is_dragging());
        (desktop, wid, grab)
    }

    /// Inclusive tile range covered by `rect` (clamped to the grid), matching the
    /// renderer's `DamageSet::mark_rect` tiling. Used to assert the damage is a
    /// true SUPERSET of a footprint.
    fn tile_range(rect: Rect, tile_size: u32) -> (u32, u32, u32, u32) {
        let grid_w = SCREEN_W.div_ceil(tile_size);
        let grid_h = SCREEN_H.div_ceil(tile_size);
        let x0 = (rect.x.max(0.0) as u32) / tile_size;
        let y0 = (rect.y.max(0.0) as u32) / tile_size;
        let x1 = (((rect.x + rect.width).max(0.0) as u32) / tile_size).min(grid_w - 1);
        let y1 = (((rect.y + rect.height).max(0.0) as u32) / tile_size).min(grid_h - 1);
        (x0, y0, x1, y1)
    }

    fn has_tile(damage: &liquide_compositor::damage::DamageSet, tx: u32, ty: u32) -> bool {
        damage.is_full() || damage.tiles.iter().any(|t| t.x == tx && t.y == ty)
    }

    fn assert_superset_of(
        damage: &liquide_compositor::damage::DamageSet,
        rect: Rect,
        label: &str,
    ) {
        let (x0, y0, x1, y1) = tile_range(rect, damage.tile_size);
        for ty in y0..=y1 {
            for tx in x0..=x1 {
                assert!(
                    has_tile(damage, tx, ty),
                    "drag damage must cover the {label} footprint tile ({tx},{ty}); \
                     missing tile leaves a stale region"
                );
            }
        }
    }

    /// (a) A drag-move emits damage = old∪new footprint (+margin), NOT None/full,
    /// and NOT just the new position. (b) The damage is a SUPERSET of BOTH the
    /// old and new window rects.
    #[test]
    fn drag_move_emits_old_union_new_footprint_not_full() {
        let (mut desktop, wid, _grab) = desktop_mid_move_drag();

        // Snapshot the OLD bounds exactly as the live loop does (before the move
        // event is handled).
        let old_bounds = desktop.shell.window(wid).unwrap().bounds;

        // Clear any pending dirt from window-open so we observe only the drag
        // frame's damage.
        desktop.dirty = false;
        desktop.dirty_damage = None;

        // Move the cursor far enough that old and new footprints are DISJOINT
        // (a fling), so "old∪new" is provable: the new rect alone cannot cover
        // the old tiles.
        let drag_before = desktop
            .shell
            .dragged_window()
            .and_then(|id| desktop.shell.window(id).ok())
            .map(|w| w.bounds);
        let mv = move_event(900.0, 700.0);
        assert!(desktop.handle_event(&mv), "a drag-move must request a redraw");
        desktop.mark_dirty_for_event_with_drag(&mv, Vec::new(), drag_before);

        let new_bounds = desktop.shell.window(wid).unwrap().bounds;
        assert_ne!(
            (old_bounds.x, old_bounds.y),
            (new_bounds.x, new_bounds.y),
            "the move must have relocated the window"
        );

        let damage = desktop
            .dirty_damage
            .as_ref()
            .expect("a drag-move must carry a TARGETED damage hint, not None/full");

        // NOT full-frame.
        assert!(
            !damage.is_full(),
            "drag damage must be confined, not a full-frame repaint"
        );
        let grid_w = SCREEN_W.div_ceil(damage.tile_size);
        let grid_h = SCREEN_H.div_ceil(damage.tile_size);
        let full_tiles = grid_w * grid_h;
        assert!(
            (damage.tiles.len() as u32) < full_tiles,
            "drag damage ({} tiles) must be smaller than the full grid ({} tiles)",
            damage.tiles.len(),
            full_tiles
        );

        // SUPERSET of BOTH the old and the new footprint.
        assert_superset_of(damage, old_bounds, "OLD");
        assert_superset_of(damage, new_bounds, "NEW");

        // Must include the OLD footprint — proven because the OLD and NEW rects
        // are disjoint here, so a "just the new position" hint would MISS the
        // old tiles. Assert at least one old-only tile is present.
        let (ox0, oy0, _ox1, _oy1) = tile_range(old_bounds, damage.tile_size);
        let (nx0, ny0, nx1, ny1) = tile_range(new_bounds, damage.tile_size);
        let old_outside_new = !(ox0 >= nx0 && ox0 <= nx1 && oy0 >= ny0 && oy0 <= ny1);
        assert!(
            old_outside_new,
            "test setup: old and new footprints must be disjoint to prove old-inclusion"
        );
        assert!(
            has_tile(damage, ox0, oy0),
            "drag damage MUST include the OLD footprint (tile {ox0},{oy0}); \
             omitting it leaves a ghost of the window's previous position"
        );
    }

    /// The margin around each footprint is a real superset margin: the tile just
    /// OUTSIDE the bare window rect (inside the +48px margin band) is damaged.
    #[test]
    fn drag_damage_margin_covers_shadow_blur_band() {
        let (mut desktop, wid, _grab) = desktop_mid_move_drag();
        let old_bounds = desktop.shell.window(wid).unwrap().bounds;
        desktop.dirty = false;
        desktop.dirty_damage = None;

        let drag_before = Some(old_bounds);
        // Small move so old∪new stays compact; the margin band is what we probe.
        let mv = move_event(360.0, 230.0);
        assert!(desktop.handle_event(&mv));
        desktop.mark_dirty_for_event_with_drag(&mv, Vec::new(), drag_before);

        let damage = desktop.dirty_damage.as_ref().expect("targeted hint");
        let new_bounds = desktop.shell.window(wid).unwrap().bounds;

        // The expanded rect (what drag_move_damage actually emits) must be fully
        // covered — including the shadow/blur margin band around the window.
        let margin = liquide_shell::Shell::DRAG_FOOTPRINT_MARGIN;
        assert_superset_of(damage, old_bounds.expand(margin), "OLD+margin");
        assert_superset_of(damage, new_bounds.expand(margin), "NEW+margin");
    }

    /// (c) A non-drag hover move with no menu open still falls to the
    /// conservative full path (damage hint = None) — the drag arm must not
    /// hijack ordinary hovers.
    #[test]
    fn non_drag_move_without_menu_still_goes_full() {
        let mut desktop = DesktopCompositor::new(SCREEN_W, SCREEN_H);
        desktop.loading = false;
        desktop.shell.resize_screen(SCREEN_W as f32, SCREEN_H as f32);
        let _ = desktop.shell.open_window("hover", Rect::new(300.0, 200.0, 400.0, 300.0));
        assert!(!desktop.shell.is_dragging());

        // Seed a stale targeted hint to prove the full path CLEARS it.
        let mut stale = liquide_compositor::damage::DamageSet::new(desktop.tiles.tile_size);
        stale.mark_tile(0, 0);
        desktop.dirty_damage = Some(stale);

        let mv = move_event(800.0, 600.0);
        let _ = desktop.handle_event(&mv);
        // No drag in progress → drag_window_before is None (as the loop would
        // compute it), and no menu is open → full path.
        desktop.mark_dirty_for_event_with_drag(&mv, Vec::new(), None);

        assert!(
            desktop.dirty_damage.is_none(),
            "a non-drag hover with no menu must escalate to a FULL repaint (None hint)"
        );
    }

    /// A drag-move with the OLD bounds unavailable (None) must NOT under-damage:
    /// it falls back to the existing path (full repaint), never a partial hint.
    #[test]
    fn drag_move_without_old_bounds_falls_back_to_full() {
        let (mut desktop, _wid, _grab) = desktop_mid_move_drag();
        desktop.dirty = false;
        desktop.dirty_damage = None;

        let mv = move_event(900.0, 700.0);
        assert!(desktop.handle_event(&mv));
        // Old bounds unavailable → must not emit a confined (possibly
        // under-damaging) hint; falls through to full.
        desktop.mark_dirty_for_event_with_drag(&mv, Vec::new(), None);

        assert!(
            desktop.dirty_damage.is_none(),
            "missing old bounds must fall back to full-frame, never a partial hint"
        );
    }

    // ---- RESIZE drag (t135-resizedrag) -------------------------------------

    /// Stand up a desktop with one window and begin a RESIZE drag on the given
    /// edge. Returns the desktop, window id, and grab point.
    fn desktop_mid_resize_drag(
        edge: liquide_shell::HitZone,
        grab: Point,
    ) -> (DesktopCompositor, liquide_shell::WindowId, Rect) {
        let mut desktop = DesktopCompositor::new(SCREEN_W, SCREEN_H);
        desktop.loading = false;
        desktop.shell.resize_screen(SCREEN_W as f32, SCREEN_H as f32);

        let bounds = Rect::new(400.0, 300.0, 300.0, 200.0);
        let wid = desktop.shell.open_window("resize", bounds);
        assert!(
            desktop.shell.begin_resize_drag(wid, edge, grab),
            "resize-drag must start"
        );
        assert!(desktop.shell.is_dragging());
        (desktop, wid, bounds)
    }

    /// A RESIZE drag-frame must confine damage to old∪new footprint (NOT full),
    /// and the damage must be a SUPERSET of BOTH the old and new window rects.
    /// This is the resize analogue of `drag_move_emits_old_union_new_footprint_not_full`.
    #[test]
    fn drag_resize_emits_old_union_new_footprint_not_full() {
        let grab = Point::new(700.0, 400.0); // right edge of the 400,300,300,200 window
        let (mut desktop, wid, old_bounds) =
            desktop_mid_resize_drag(liquide_shell::HitZone::ResizeRight, grab);

        desktop.dirty = false;
        desktop.dirty_damage = None;

        let drag_before = desktop
            .shell
            .dragged_window()
            .and_then(|id| desktop.shell.window(id).ok())
            .map(|w| w.bounds);
        // Grow the right edge by +200px.
        let mv = move_event(grab.x + 200.0, grab.y);
        assert!(desktop.handle_event(&mv), "a resize-move must request a redraw");
        desktop.mark_dirty_for_event_with_drag(&mv, Vec::new(), drag_before);

        let new_bounds = desktop.shell.window(wid).unwrap().bounds;
        assert!(
            new_bounds.width > old_bounds.width,
            "the resize must have grown the window"
        );

        let damage = desktop
            .dirty_damage
            .as_ref()
            .expect("a resize-drag must carry a TARGETED damage hint, not None/full");

        assert!(
            !damage.is_full(),
            "resize damage must be confined, not a full-frame repaint"
        );
        let grid_w = SCREEN_W.div_ceil(damage.tile_size);
        let grid_h = SCREEN_H.div_ceil(damage.tile_size);
        let full_tiles = grid_w * grid_h;
        assert!(
            (damage.tiles.len() as u32) < full_tiles,
            "resize damage ({} tiles) must be smaller than the full grid ({} tiles)",
            damage.tiles.len(),
            full_tiles
        );

        assert_superset_of(damage, old_bounds, "OLD");
        assert_superset_of(damage, new_bounds, "NEW");
    }

    /// The CRITICAL shrink case: a left-edge inward resize shrinks the window and
    /// moves its origin right. The revealed old-only band (inside the OLD window,
    /// OUTSIDE the NEW window) MUST be damaged so the larger old window leaves no
    /// ghost. TEETH: the old-only tile is asserted present and proven NOT covered
    /// by the new footprint, so dropping the OLD rect would leave it stale.
    #[test]
    fn drag_resize_shrink_damages_revealed_old_only_band() {
        let grab = Point::new(400.0, 400.0); // left edge of the 400,300,300,200 window
        let (mut desktop, wid, old_bounds) =
            desktop_mid_resize_drag(liquide_shell::HitZone::ResizeLeft, grab);

        desktop.dirty = false;
        desktop.dirty_damage = None;

        let drag_before = Some(old_bounds);
        // Drag the left edge inward (to the right) by +150px → shrink from left.
        let mv = move_event(grab.x + 150.0, grab.y);
        assert!(desktop.handle_event(&mv));
        desktop.mark_dirty_for_event_with_drag(&mv, Vec::new(), drag_before);

        let new_bounds = desktop.shell.window(wid).unwrap().bounds;
        assert!(
            new_bounds.x > old_bounds.x && new_bounds.width < old_bounds.width,
            "left-inward resize must move origin right and shrink width"
        );

        let damage = desktop.dirty_damage.as_ref().expect("targeted hint");

        // The OLD footprint must be a superset (covers the revealed left band).
        assert_superset_of(damage, old_bounds, "OLD (revealed band)");

        // TEETH: a tile in the revealed old-only band must be damaged, and it must
        // NOT be covered by the new footprint — so the OLD rect is load-bearing.
        let revealed = Rect::new(old_bounds.x, old_bounds.y, 10.0, old_bounds.height);
        let (rx0, ry0, _rx1, _ry1) = tile_range(revealed, damage.tile_size);
        assert!(
            has_tile(damage, rx0, ry0),
            "the revealed old-only band tile ({rx0},{ry0}) must be damaged on shrink"
        );
        // Prove the new (margined) footprint does NOT cover that revealed tile.
        let margin = liquide_shell::Shell::DRAG_FOOTPRINT_MARGIN;
        let new_margined = new_bounds.expand(margin);
        let (nx0, ny0, nx1, ny1) = tile_range(new_margined, damage.tile_size);
        let covered_by_new =
            rx0 >= nx0 && rx0 <= nx1 && ry0 >= ny0 && ry0 <= ny1;
        assert!(
            !covered_by_new,
            "test integrity: the revealed tile ({rx0},{ry0}) must lie OUTSIDE the \
             new footprint so the OLD rect is what damages it (red teeth if dropped)"
        );
    }

    /// A resize-drag with the OLD bounds unavailable falls back to full-frame,
    /// never a partial (possibly under-damaging) hint — same safety as the move.
    #[test]
    fn drag_resize_without_old_bounds_falls_back_to_full() {
        let grab = Point::new(700.0, 400.0);
        let (mut desktop, _wid, _old) =
            desktop_mid_resize_drag(liquide_shell::HitZone::ResizeRight, grab);
        desktop.dirty = false;
        desktop.dirty_damage = None;

        let mv = move_event(grab.x + 100.0, grab.y);
        assert!(desktop.handle_event(&mv));
        desktop.mark_dirty_for_event_with_drag(&mv, Vec::new(), None);

        assert!(
            desktop.dirty_damage.is_none(),
            "missing old bounds must fall back to full-frame for resize too"
        );
    }
}
