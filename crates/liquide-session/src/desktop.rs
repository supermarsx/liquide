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
    /// Renders a polished startup screen with a dark background, centered
    /// glass-style panel with branding elements and a progress bar.
    fn build_loading_scene(&self) -> SceneNode {
        let w = self.width as f32;
        let h = self.height as f32;
        let screen = Rect::new(0.0, 0.0, w, h);

        let mut root = SceneNode::new(
            0,
            SceneNodeKind::Root,
            NodeProperties::new(screen),
        );

        // Full-screen dark background with a subtle blue tint.
        root.add_child(SceneNode::new(
            1,
            SceneNodeKind::Background {
                color: Color::new(12, 16, 24, 255),
            },
            NodeProperties::new(screen).with_z_order(0),
        ));

        // Subtle radial-ish gradient: lighter center area behind the panel.
        let glow_size = 600.0_f32.min(w * 0.6);
        let glow = Rect::new(
            (w - glow_size) / 2.0,
            (h - glow_size * 0.6) / 2.0,
            glow_size,
            glow_size * 0.6,
        );
        root.add_child(SceneNode::new(
            2,
            SceneNodeKind::Background {
                color: Color::new(20, 30, 50, 120),
            },
            NodeProperties::new(glow).with_z_order(1),
        ));

        // Main panel — glass-style with a dark semi-transparent fill.
        let panel_w = 480.0_f32.min(w - 80.0);
        let panel_h = 200.0_f32.min(h - 80.0);
        let px = (w - panel_w) / 2.0;
        let py = (h - panel_h) / 2.0;
        let panel = Rect::new(px, py, panel_w, panel_h);

        root.add_child(SceneNode::new(
            10,
            SceneNodeKind::Background {
                color: Color::new(24, 28, 40, 230),
            },
            NodeProperties::new(panel).with_z_order(10),
        ));

        // Top accent bar — vibrant blue gradient strip.
        let accent = Rect::new(px, py, panel_w, 3.0);
        root.add_child(SceneNode::new(
            11,
            SceneNodeKind::Background {
                color: Color::new(60, 140, 240, 255),
            },
            NodeProperties::new(accent).with_z_order(11),
        ));

        // Side accent glow — thin vertical blue lines on panel edges.
        let left_accent = Rect::new(px, py + 3.0, 1.0, panel_h - 3.0);
        root.add_child(SceneNode::new(
            12,
            SceneNodeKind::Background {
                color: Color::new(60, 140, 240, 40),
            },
            NodeProperties::new(left_accent).with_z_order(12),
        ));
        let right_accent = Rect::new(px + panel_w - 1.0, py + 3.0, 1.0, panel_h - 3.0);
        root.add_child(SceneNode::new(
            13,
            SceneNodeKind::Background {
                color: Color::new(60, 140, 240, 40),
            },
            NodeProperties::new(right_accent).with_z_order(12),
        ));

        // Bottom border.
        let bottom_border = Rect::new(px, py + panel_h - 1.0, panel_w, 1.0);
        root.add_child(SceneNode::new(
            14,
            SceneNodeKind::Background {
                color: Color::new(60, 140, 240, 30),
            },
            NodeProperties::new(bottom_border).with_z_order(12),
        ));

        // "LIQUIDE" branding — rendered as 7 block letters since we
        // don't have text rendering yet.  Each letter is a small
        // colored rectangle arranged horizontally.
        let letter_w = 18.0_f32;
        let letter_h = 28.0_f32;
        let letter_gap = 8.0_f32;
        let brand_count = 7.0_f32; // L I Q U I D E
        let brand_total_w = brand_count * letter_w + (brand_count - 1.0) * letter_gap;
        let brand_x = px + (panel_w - brand_total_w) / 2.0;
        let brand_y = py + 35.0;

        for i in 0..7 {
            let lx = brand_x + i as f32 * (letter_w + letter_gap);
            // Alternate slightly different blues for visual interest.
            let blue = if i % 2 == 0 { 240 } else { 200 };
            let alpha = if i % 2 == 0 { 255 } else { 220 };
            root.add_child(SceneNode::new(
                20 + i as u64,
                SceneNodeKind::Background {
                    color: Color::new(60, 140, blue, alpha),
                },
                NodeProperties::new(Rect::new(lx, brand_y, letter_w, letter_h))
                    .with_z_order(13),
            ));
        }

        // Subtitle line — thin white bar below the branding.
        let sub_w = brand_total_w * 0.6;
        let sub_rect = Rect::new(
            px + (panel_w - sub_w) / 2.0,
            brand_y + letter_h + 16.0,
            sub_w,
            2.0,
        );
        root.add_child(SceneNode::new(
            30,
            SceneNodeKind::Background {
                color: Color::new(180, 190, 210, 100),
            },
            NodeProperties::new(sub_rect).with_z_order(13),
        ));

        // Progress bar track — dark inset.
        let bar_w = panel_w - 80.0;
        let bar_h = 6.0_f32;
        let bar_x = px + 40.0;
        let bar_y = py + panel_h - 45.0;
        let bar_track = Rect::new(bar_x, bar_y, bar_w, bar_h);
        root.add_child(SceneNode::new(
            40,
            SceneNodeKind::Background {
                color: Color::new(10, 14, 22, 200),
            },
            NodeProperties::new(bar_track).with_z_order(13),
        ));

        // Progress bar fill — animated blue glow.
        // Use frame_count to create a simple shimmer effect.
        let progress = 0.35_f32; // fixed 35% for static loading screen
        let fill_w = bar_w * progress;
        let bar_fill = Rect::new(bar_x, bar_y, fill_w, bar_h);
        root.add_child(SceneNode::new(
            41,
            SceneNodeKind::Background {
                color: Color::new(60, 150, 255, 255),
            },
            NodeProperties::new(bar_fill).with_z_order(14),
        ));

        // Progress bar leading edge glow.
        let edge_w = 20.0_f32.min(fill_w);
        let edge_rect = Rect::new(bar_x + fill_w - edge_w, bar_y - 1.0, edge_w, bar_h + 2.0);
        root.add_child(SceneNode::new(
            42,
            SceneNodeKind::Background {
                color: Color::new(120, 200, 255, 180),
            },
            NodeProperties::new(edge_rect).with_z_order(15),
        ));

        // Status text placeholder — thin gray bar below progress.
        let status_w = 120.0_f32;
        let status_rect = Rect::new(
            px + (panel_w - status_w) / 2.0,
            bar_y + bar_h + 12.0,
            status_w,
            3.0,
        );
        root.add_child(SceneNode::new(
            50,
            SceneNodeKind::Background {
                color: Color::new(120, 130, 150, 80),
            },
            NodeProperties::new(status_rect).with_z_order(13),
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
            let cursor_size = 24.0_f32;
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

        // 5b. Optional per-node dump (costly — allocates strings per node).
        // Only emitted in debug builds when --debug-perf is set.
        #[cfg(debug_assertions)]
        if self.debug_perf {
            debug!(count = flat_nodes.len(), "flattened nodes");
            for (i, node) in flat_nodes.iter().enumerate() {
                let kind_name = scene_node_kind_name(&node.kind);
                let b = &node.absolute_bounds;
                let color_str = scene_node_color_str(&node.kind);
                debug!(
                    idx = i,
                    id = node.id,
                    kind = kind_name,
                    x = format!("{:.0}", b.x),
                    y = format!("{:.0}", b.y),
                    w = format!("{:.0}", b.width),
                    h = format!("{:.0}", b.height),
                    z = node.z_order,
                    opacity = format!("{:.2}", node.opacity),
                    color = color_str,
                    "  node"
                );
            }
        }

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
                present_ms = format!("{:.1}", present_ms),
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
        let screen_rect = platform.display().virtual_screen_rect();
        let screen_w = screen_rect.width as u32;
        let screen_h = screen_rect.height as u32;
        if screen_w > 0 && screen_h > 0 && (screen_w != self.width || screen_h != self.height) {
            info!(
                old_w = self.width, old_h = self.height,
                new_w = screen_w, new_h = screen_h,
                "resizing compositor to match primary screen"
            );
            self.width = screen_w;
            self.height = screen_h;
            let _ = self.compositor.resize(screen_w, screen_h);
            self.shell.resize_screen(screen_w as f32, screen_h as f32);
            self.cursor_x = screen_w as f32 / 2.0;
            self.cursor_y = screen_h as f32 / 2.0;
        }

        // Create a borderless fullscreen desktop window.
        debug!("creating desktop window {}x{}", self.width, self.height);
        let t_win = Instant::now();
        let params = NativeWindowParams {
            title: "Liquide Desktop".to_string(),
            geometry: Rect::new(0.0, 0.0, self.width as f32, self.height as f32),
            window_type: "desktop".to_string(),
            parent: None,
            app_id: "com.liquide.desktop".to_string(),
        };
        if let Ok(handle) = platform.window_host().create_window(params) {
            self.window_handle = Some(handle);
        }
        info!(
            width = self.width, height = self.height,
            elapsed_ms = format!("{:.1}", t_win.elapsed().as_secs_f64() * 1000.0),
            "desktop window created (borderless fullscreen)"
        );

        // Show loading overlay.
        debug!("rendering loading overlay");
        self.loading = true;
        self.render_frame(platform);
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

            // Sleep to avoid busy-spinning and burning CPU.
            // Three cases:
            //   1. Dirty but throttled — sleep until the next frame is due.
            //   2. Idle (no events, not dirty) — sleep up to the frame budget.
            //   3. Had events but not dirty — yield briefly.
            if self.dirty && !self.frame_interval.is_zero() {
                let elapsed = self.last_render.elapsed();
                if elapsed < self.frame_interval {
                    let remaining = self.frame_interval - elapsed;
                    thread::sleep(remaining.min(Duration::from_millis(4)));
                }
            } else if !had_event && !self.dirty {
                let sleep_ms = if !self.frame_interval.is_zero() {
                    let elapsed = self.last_render.elapsed();
                    if elapsed < self.frame_interval {
                        let remaining = self.frame_interval - elapsed;
                        remaining.as_millis().min(16) as u64
                    } else {
                        1
                    }
                } else {
                    1
                };
                thread::sleep(Duration::from_millis(sleep_ms));
            } else if had_event && !self.dirty {
                thread::sleep(Duration::from_millis(1));
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

/// Short human-readable name for a scene node kind (for debug logging).
#[cfg(debug_assertions)]
fn scene_node_kind_name(kind: &SceneNodeKind) -> &'static str {
    match kind {
        SceneNodeKind::Root => "Root",
        SceneNodeKind::Background { .. } => "Background",
        SceneNodeKind::Surface { .. } => "Surface",
        SceneNodeKind::ChildSurface { .. } => "ChildSurface",
        SceneNodeKind::Glass(_) => "Glass",
        SceneNodeKind::Tint { .. } => "Tint",
        SceneNodeKind::Shadow { .. } => "Shadow",
        SceneNodeKind::Decoration { .. } => "Decoration",
        SceneNodeKind::BlurBackdrop => "BlurBackdrop",
        SceneNodeKind::BlurCache => "BlurCache",
        SceneNodeKind::Content => "Content",
        SceneNodeKind::Overlay => "Overlay",
        SceneNodeKind::ShellLayer => "ShellLayer",
        SceneNodeKind::Cursor => "Cursor",
        SceneNodeKind::Text { .. } => "Text",
        SceneNodeKind::Icon { .. } => "Icon",
        SceneNodeKind::LockScreen => "LockScreen",
        SceneNodeKind::CrashScreen => "CrashScreen",
        SceneNodeKind::Workspace { .. } => "Workspace",
    }
}

/// Extract color info from a scene node kind for debug logging.
#[cfg(debug_assertions)]
fn scene_node_color_str(kind: &SceneNodeKind) -> String {
    match kind {
        SceneNodeKind::Background { color } => {
            format!("rgba({},{},{},{})", color.r, color.g, color.b, color.a)
        }
        SceneNodeKind::Glass(params) => {
            let c = &params.tint_color;
            format!(
                "tint({},{},{},{}) blur={}",
                c.r, c.g, c.b, c.a, params.blur_radius
            )
        }
        SceneNodeKind::Tint { color } => {
            format!("rgba({},{},{},{})", color.r, color.g, color.b, color.a)
        }
        SceneNodeKind::Shadow { color, .. } => {
            format!("rgba({},{},{},{})", color.r, color.g, color.b, color.a)
        }
        SceneNodeKind::Decoration { background, .. } => {
            format!(
                "bg({},{},{},{})",
                background.r, background.g, background.b, background.a
            )
        }
        _ => "-".to_string(),
    }
}
