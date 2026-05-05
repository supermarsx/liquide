//! Main desktop event loop — window creation, loading screen, and frame dispatch.

use std::thread;
use std::time::{Duration, Instant};

use liquide_compositor::geometry::Rect;
use liquide_platform::{NativeWindowParams, PlatformBackend};
use tracing::{debug, info};

use super::paint_state::TimerAction;
use super::{DesktopCompositor, RenderMsg};

impl DesktopCompositor {
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

            // Check for completed frames from the render thread.
            if self.try_present(platform) {
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
                Duration::from_micros(100)
            } else if self.dirty && !self.frame_interval.is_zero() {
                // Dirty but throttled — sleep until next frame is due.
                let elapsed = self.last_render.elapsed();
                if elapsed < self.frame_interval {
                    self.frame_interval - elapsed
                } else {
                    Duration::ZERO
                }
            } else if !self.dirty {
                // Nothing to render — sleep longer when no events arriving.
                if had_event {
                    Duration::from_millis(1)
                } else {
                    Duration::from_millis(self.frame_interval.as_millis().clamp(1, 16) as u64)
                }
            } else {
                Duration::ZERO
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
