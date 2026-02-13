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

use std::time::{Instant, SystemTime, UNIX_EPOCH};

use liquide_compositor::{Compositor, CompositorContract};
use liquide_compositor::damage::DamageSet;
use liquide_compositor::effects::QualityProfile;
use liquide_compositor::geometry::Rect;
use liquide_input::InputState;
use liquide_input::event::InputEvent;
use liquide_platform::{
    NativeWindowHandle, NativeWindowParams, PlatformBackend, PlatformEvent,
};
use liquide_renderer_cpu::{Renderer, SoftwareRenderer};
use liquide_shell::Shell;

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
    last_tick: Instant,
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
            last_tick: Instant::now(),
        }
    }

    /// Run one frame: build scene from shell, render, present.
    ///
    /// The method carefully sequences borrows so that the compositor is
    /// never borrowed immutably and mutably at the same time:
    ///
    /// 1. Build scene from shell state (owned `SceneNode`).
    /// 2. Submit to compositor and swap double-buffer.
    /// 3. Flatten scene to owned `Vec<FlatNode>` — immutable borrow ends.
    /// 4. Render into back buffer — mutable borrows of disjoint fields.
    /// 5. Present the just-rendered back buffer to the platform window.
    pub fn render_frame(&mut self, platform: &mut dyn PlatformBackend) {
        let frame_start = Instant::now();

        // 1. Build the scene graph from the shell's full state.
        let scene = self.shell.build_scene();

        // 2. Submit to compositor and swap buffers.
        let _ = self.compositor.submit_scene(scene);
        self.compositor.begin_frame();

        // 3. Full-screen damage — the damage tracker inside the compositor
        //    can also compute incremental damage, but for the desktop loop
        //    a full redraw every frame is the safest default.
        let tile_size = self.compositor.tile_size();
        let grid_w = self.width.div_ceil(tile_size);
        let grid_h = self.height.div_ceil(tile_size);
        let mut damage = DamageSet::new(tile_size);
        damage.mark_all(grid_w, grid_h);

        // 4. Flatten the scene into a z-sorted list of visible leaf nodes.
        //    The temporary immutable borrow of `self.compositor.scene()` is
        //    released once `flatten()` returns the owned `Vec<FlatNode>`.
        let flat_nodes = self
            .compositor
            .scene()
            .map(|s| s.flatten())
            .unwrap_or_default();

        // 5. Render into the back buffer.
        let fb = self.compositor.frame_buffer_mut();
        let _ = self.renderer.render(&flat_nodes, fb, &damage);

        // 6. Present the just-rendered back buffer to the platform window.
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

        self.frame_count += 1;

        // Report frame timing to the compositor for adaptive quality.
        let frame_ms = frame_start.elapsed().as_secs_f64() * 1000.0;
        self.compositor.report_frame_time(frame_ms);
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
        if let Some(action) = self.shell.handle_platform_event(event) {
            if self.shell.execute_action(&action) {
                needs_redraw = true;
            }
        }

        needs_redraw
    }

    /// Perform periodic updates (clock, notification expiry, etc.).
    fn tick(&mut self) {
        let now_us = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;
        self.shell.tick(now_us);
    }

    /// Run the desktop event loop using the given platform backend.
    ///
    /// Creates the main desktop window, performs an initial render,
    /// and then enters a blocking event loop that:
    /// - Routes platform events through the shell.
    /// - Runs periodic ticks (clock, notifications) every 100ms.
    /// - Re-renders whenever the shell state has changed.
    pub fn run(&mut self, platform: &mut dyn PlatformBackend) {
        // Create the main desktop window.
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

        // Initial render.
        self.render_frame(platform);

        // Event loop.
        while self.running {
            let event = platform.wait_event();
            let needs_redraw = self.handle_event(&event);

            // Periodic tick every ~100ms for clock / notification expiry.
            if self.last_tick.elapsed().as_millis() >= 100 {
                self.tick();
                self.last_tick = Instant::now();
            }

            if needs_redraw {
                self.render_frame(platform);
            }
        }

        // Clean up the window on exit.
        if let Some(handle) = self.window_handle.take() {
            let _ = platform.window_host().destroy_window(handle);
        }
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
