//! Desktop compositor loop — wires the shell, compositor, renderer, input,
//! and platform backend into a running desktop environment.
//!
//! [`DesktopCompositor`] owns a [`Shell`], [`Compositor`],
//! [`SoftwareRenderer`], and [`InputState`].  Each frame it:
//!
//! 1. Asks the shell for the current scene graph (`shell.build_scene()`).
//! 2. Submits the scene to the compositor's double-buffered pipeline.
//! 3. Flattens + renders into the back buffer via the software renderer.
//! 4. Presents the rendered frame to the platform window.
//!
//! Platform events are routed through the shell's `handle_platform_event`
//! method, which translates them into `ShellAction`s that modify shell
//! state (focus, window management, launcher toggle, etc.).

use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use liquide_compositor::{Compositor, CompositorContract};
use liquide_compositor::damage::DamageSet;
use liquide_compositor::effects::QualityProfile;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::Color;
use liquide_compositor::scene::{NodeProperties, SceneNode, SceneNodeKind};
use liquide_input::InputState;
use liquide_input::event::InputEvent;
use liquide_platform::{
    NativeWindowHandle, NativeWindowParams, PlatformBackend, PlatformEvent,
};
use liquide_renderer_cpu::{Renderer, SoftwareRenderer};
use liquide_shell::Shell;
use tracing::{debug, info, warn};

/// The desktop compositor loop.
///
/// Holds the shell (window management, dock, status bar, launcher,
/// notifications, shortcuts), the compositor (scene graph, damage
/// tracking, double-buffering), the software renderer, input state,
/// and the native window handle.
///
/// Call [`DesktopCompositor::run`] to enter the blocking event loop.
pub struct DesktopCompositor {
    shell: Shell,
    compositor: Compositor,
    renderer: SoftwareRenderer,
    input_state: InputState,
    width: u32,
    height: u32,
    window_handle: Option<NativeWindowHandle>,
    frame_count: u64,
    running: bool,
    dirty: bool,
    last_tick: Instant,
    last_render: Instant,
    cursor_x: f32,
    cursor_y: f32,
    loading: bool,
    /// Minimum interval between frames. 0 = unlimited.
    frame_interval: Duration,
    /// Whether to emit per-frame performance timings at debug level.
    debug_perf: bool,
}

impl DesktopCompositor {
    /// Create a new desktop compositor with the given initial resolution.
    ///
    /// Uses a 64-pixel tile size and the [`QualityProfile::Balanced`]
    /// profile.  The shell is initialized with matching screen dimensions.
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            shell: Shell::new(width as f32, height as f32),
            compositor: Compositor::new(width, height, 64, QualityProfile::Balanced),
            renderer: SoftwareRenderer::new(),
            input_state: InputState::new(),
            width,
            height,
            window_handle: None,
            frame_count: 0,
            running: true,
            dirty: true,
            last_tick: Instant::now(),
            last_render: Instant::now(),
            cursor_x: width as f32 / 2.0,
            cursor_y: height as f32 / 2.0,
            loading: true,
            frame_interval: Duration::from_millis(16), // ~60fps default
            debug_perf: false,
        }
    }

    /// Set the maximum frames per second. 0 means unlimited.
    pub fn set_fps_cap(&mut self, fps: u32) {
        self.frame_interval = if fps == 0 {
            Duration::ZERO
        } else {
            Duration::from_micros(1_000_000 / fps as u64)
        };
    }

    /// Enable or disable per-frame perf timing output.
    pub fn set_debug_perf(&mut self, enabled: bool) {
        self.debug_perf = enabled;
    }

    /// Build a loading overlay scene — shown during first-frame startup.
    ///
    /// Uses solid colored rects instead of Glass to avoid expensive blur
    /// operations on a solid background (blur on uniform pixels is wasted
    /// work and allocates large temp buffers for zero visual effect).
    fn build_loading_scene(&self) -> SceneNode {
        let screen = Rect::new(0.0, 0.0, self.width as f32, self.height as f32);

        let mut root = SceneNode::new(
            0,
            SceneNodeKind::Root,
            NodeProperties::new(screen),
        );

        // Dark background
        root.add_child(SceneNode::new(
            1,
            SceneNodeKind::Background {
                color: Color::new(20, 25, 35, 255),
            },
            NodeProperties::new(screen).with_z_order(0),
        ));

        // Center panel (solid tinted rect — no blur needed on solid bg)
        let panel_w = 360.0_f32;
        let panel_h = 120.0_f32;
        let px = (self.width as f32 - panel_w) / 2.0;
        let py = (self.height as f32 - panel_h) / 2.0;
        let panel = Rect::new(px, py, panel_w, panel_h);

        root.add_child(SceneNode::new(
            2,
            SceneNodeKind::Background {
                color: Color::new(40, 45, 60, 255),
            },
            NodeProperties::new(panel).with_z_order(10),
        ));

        // Accent line across top of panel
        let accent = Rect::new(px, py, panel_w, 3.0);
        root.add_child(SceneNode::new(
            3,
            SceneNodeKind::Background {
                color: Color::new(80, 140, 220, 200),
            },
            NodeProperties::new(accent).with_z_order(11),
        ));

        // Pulsing dot (centered)
        let dot_size = 12.0_f32;
        let dot = Rect::new(
            px + (panel_w - dot_size) / 2.0,
            py + (panel_h - dot_size) / 2.0,
            dot_size,
            dot_size,
        );
        root.add_child(SceneNode::new(
            4,
            SceneNodeKind::Background {
                color: Color::new(80, 160, 240, 255),
            },
            NodeProperties::new(dot).with_z_order(12),
        ));

        root
    }

    /// Run one frame: build scene from shell (or loading), render, present.
    pub fn render_frame(&mut self, platform: &mut dyn PlatformBackend) {
        let frame_start = Instant::now();

        // 1. Build the scene graph.
        let t0 = Instant::now();
        let mut scene = if self.loading {
            self.build_loading_scene()
        } else {
            self.shell.build_scene()
        };
        let scene_ms = t0.elapsed().as_secs_f64() * 1000.0;

        // 2. Add software cursor to the scene.
        if !self.loading {
            let cursor_size = 16.0_f32;
            let cursor_bounds = Rect::new(
                self.cursor_x,
                self.cursor_y,
                cursor_size,
                cursor_size,
            );
            scene.add_child(SceneNode::new(
                999_999,
                SceneNodeKind::Cursor,
                NodeProperties::new(cursor_bounds).with_z_order(9999),
            ));
        }

        // 3. Submit to compositor and swap buffers.
        let t1 = Instant::now();
        let _ = self.compositor.submit_scene(scene);
        self.compositor.begin_frame();
        let submit_ms = t1.elapsed().as_secs_f64() * 1000.0;

        // 4. Full-screen damage.
        let tile_size = self.compositor.tile_size();
        let grid_w = self.width.div_ceil(tile_size);
        let grid_h = self.height.div_ceil(tile_size);
        let mut damage = DamageSet::new(tile_size);
        damage.mark_all(grid_w, grid_h);

        // 5. Flatten the scene into a z-sorted list of visible leaf nodes.
        let t2 = Instant::now();
        let flat_nodes = self
            .compositor
            .scene()
            .map(|s| s.flatten())
            .unwrap_or_default();
        let flatten_ms = t2.elapsed().as_secs_f64() * 1000.0;

        // 6. Render into the back buffer.
        let t3 = Instant::now();
        let fb = self.compositor.frame_buffer_mut();
        let _ = self.renderer.render(&flat_nodes, fb, &damage);
        let render_ms = t3.elapsed().as_secs_f64() * 1000.0;

        // 7. Present the just-rendered back buffer to the platform window.
        let t4 = Instant::now();
        if let Some(handle) = self.window_handle {
            let _ = platform.present_frame(
                handle,
                &fb.pixels,
                fb.width,
                fb.height,
                fb.stride,
                fb.format,
            );
        }
        let present_ms = t4.elapsed().as_secs_f64() * 1000.0;

        self.frame_count += 1;

        // Report frame timing to the compositor for adaptive quality.
        let frame_ms = frame_start.elapsed().as_secs_f64() * 1000.0;
        self.compositor.report_frame_time(frame_ms);

        // Feed render time to the renderer for adaptive blur toggling.
        let blur_was_on = self.renderer.blur_enabled();
        self.renderer.report_render_time(render_ms);
        if blur_was_on != self.renderer.blur_enabled() {
            info!(
                blur = self.renderer.blur_enabled(),
                avg_ms = format!("{:.1}", render_ms),
                "adaptive blur toggled"
            );
        }

        // Debug perf output.
        if self.debug_perf {
            debug!(
                frame = self.frame_count,
                total_ms = format!("{:.2}", frame_ms),
                scene_ms = format!("{:.2}", scene_ms),
                submit_ms = format!("{:.2}", submit_ms),
                flatten_ms = format!("{:.2}", flatten_ms),
                render_ms = format!("{:.2}", render_ms),
                present_ms = format!("{:.2}", present_ms),
                nodes = flat_nodes.len(),
                blur = self.renderer.blur_enabled(),
                loading = self.loading,
                "frame timing"
            );
        }

        // Warn on slow frames.
        if frame_ms > 100.0 {
            warn!(
                frame = self.frame_count,
                total_ms = format!("{:.1}", frame_ms),
                render_ms = format!("{:.1}", render_ms),
                nodes = flat_nodes.len(),
                "slow frame detected"
            );
        }
    }

    /// Handle a platform event: route through shell and input state.
    ///
    /// Returns `true` if the event requires a redraw.
    pub fn handle_event(&mut self, event: &PlatformEvent) -> bool {
        let mut needs_redraw = false;

        match event {
            PlatformEvent::WindowResized {
                width, height, ..
            } => {
                self.width = *width;
                self.height = *height;
                let _ = self.compositor.resize(*width, *height);
                self.shell.resize_screen(*width as f32, *height as f32);
                needs_redraw = true;
            }
            PlatformEvent::WindowCloseRequested { .. } | PlatformEvent::Quit => {
                self.running = false;
            }
            PlatformEvent::WindowRedraw { .. } => {
                needs_redraw = true;
            }
            PlatformEvent::KeyInput { event: ke, .. } => {
                self.input_state
                    .handle_event(&InputEvent::Keyboard(*ke));
            }
            PlatformEvent::MouseInput { event: me, .. } => {
                // Track cursor position for software cursor rendering.
                use liquide_input::mouse::MouseEvent;
                match me {
                    MouseEvent::Move { x, y } => {
                        self.cursor_x = *x;
                        self.cursor_y = *y;
                        needs_redraw = true;
                    }
                    MouseEvent::Button { x, y, .. } => {
                        self.cursor_x = *x;
                        self.cursor_y = *y;
                    }
                    _ => {}
                }
                self.input_state
                    .handle_event(&InputEvent::Mouse(*me));
            }
            PlatformEvent::TouchInput { event: te, .. } => {
                self.input_state
                    .handle_event(&InputEvent::Touch(*te));
            }
            _ => {}
        }

        // Route the event through the shell for higher-level actions
        // (keyboard shortcuts, mouse-click focus, dock hover, etc.).
        if !self.loading {
            if let Some(action) = self.shell.handle_platform_event(event) {
                if self.shell.execute_action(&action) {
                    needs_redraw = true;
                }
            }
        }

        needs_redraw
    }

    /// Perform periodic updates (clock, notification expiry, etc.).
    ///
    /// Returns `true` if something visually changed and a redraw is needed.
    fn tick(&mut self) -> bool {
        let now_us = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;
        self.shell.tick(now_us)
    }

    /// Run the desktop event loop using the given platform backend.
    ///
    /// Creates the main desktop window, shows a loading overlay,
    /// then enters a non-blocking poll loop that:
    /// - Drains all pending platform events each iteration.
    /// - Runs periodic ticks (clock, notifications) every ~1s.
    /// - Re-renders when dirty (throttled by `frame_interval`).
    /// - Sleeps briefly when idle.
    pub fn run(&mut self, platform: &mut dyn PlatformBackend) {
        let run_start = Instant::now();

        // Create the main desktop window.
        debug!("creating desktop window {}x{}", self.width, self.height);
        let t_win = Instant::now();
        let params = NativeWindowParams {
            title: "Liquide Desktop".to_string(),
            geometry: Rect::new(
                0.0,
                0.0,
                self.width as f32,
                self.height as f32,
            ),
            window_type: "normal".to_string(),
            parent: None,
            app_id: "com.liquide.desktop".to_string(),
        };
        if let Ok(handle) = platform.window_host().create_window(params) {
            self.window_handle = Some(handle);
        }
        info!(
            elapsed_ms = format!("{:.1}", t_win.elapsed().as_secs_f64() * 1000.0),
            "window created"
        );

        // Show loading overlay (cheap — no blur, just solid rects).
        debug!("rendering loading overlay");
        self.loading = true;
        self.render_frame(platform);
        info!(
            elapsed_ms = format!("{:.1}", run_start.elapsed().as_secs_f64() * 1000.0),
            "loading overlay presented"
        );

        // Transition from loading to desktop after the first frame.
        debug!("rendering first desktop frame");
        self.loading = false;
        self.dirty = true;
        self.render_frame(platform);
        self.dirty = false;
        info!(
            elapsed_ms = format!("{:.1}", run_start.elapsed().as_secs_f64() * 1000.0),
            "first desktop frame presented"
        );

        // Non-blocking event loop.
        info!(
            fps_cap = if self.frame_interval.is_zero() {
                0
            } else {
                (1_000_000 / self.frame_interval.as_micros().max(1)) as u32
            },
            debug_perf = self.debug_perf,
            "entering event loop"
        );

        while self.running {
            // Drain all pending events.
            let mut had_event = false;
            while let Some(event) = platform.poll_event() {
                had_event = true;
                if self.handle_event(&event) {
                    self.dirty = true;
                }
            }

            // Periodic tick every ~1s for clock / notification expiry.
            // Only marks dirty if something actually changed visually.
            if self.last_tick.elapsed().as_millis() >= 1000 {
                if self.tick() {
                    self.dirty = true;
                }
                self.last_tick = Instant::now();
            }

            // Re-render if anything changed, throttled by frame_interval.
            if self.dirty {
                let can_render = self.frame_interval.is_zero()
                    || self.last_render.elapsed() >= self.frame_interval;
                if can_render {
                    self.render_frame(platform);
                    self.dirty = false;
                    self.last_render = Instant::now();
                }
            }

            // If no events were processed this iteration, sleep briefly
            // to avoid busy-spinning and burning CPU.
            if !had_event {
                thread::sleep(Duration::from_millis(8));
            }
        }

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

    /// Whether the compositor is still running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Total number of frames rendered so far.
    #[must_use]
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Read-only access to the current input state.
    #[must_use]
    pub fn input_state(&self) -> &InputState {
        &self.input_state
    }

    /// Read-only access to the shell.
    #[must_use]
    pub fn shell(&self) -> &Shell {
        &self.shell
    }

    /// Mutable access to the shell.
    pub fn shell_mut(&mut self) -> &mut Shell {
        &mut self.shell
    }
}
