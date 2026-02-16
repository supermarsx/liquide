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

use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use liquide_compositor::damage::DamageSet;
use liquide_compositor::effects::QualityProfile;
use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::{Color, PixelFormat};
use liquide_compositor::scene::{CursorShape, NodeProperties, SceneNode, SceneNodeKind};
use liquide_compositor::{Compositor, CompositorContract};
use liquide_devtools::{DevToolsPanel, FrameSnapshot};
use liquide_input::InputState;
use liquide_input::event::InputEvent;
use liquide_input::keyboard::{KeyCode, KeyState};
use liquide_platform::{NativeWindowHandle, NativeWindowParams, PlatformBackend, PlatformEvent};
use liquide_renderer_cpu::{Renderer, SoftwareRenderer};
use liquide_shell::Shell;
use tracing::{debug, info, warn};

use crate::telemetry::{TelemetryHandle, create_telemetry};

// ---------------------------------------------------------------------------
// Render thread types
// ---------------------------------------------------------------------------

/// A render job sent from the main thread to the render thread.
struct RenderJob {
    scene: SceneNode,
    cursor_x: f32,
    cursor_y: f32,
    cursor_shape: CursorShape,
    width: u32,
    height: u32,
    tile_size: u32,
    /// Window ID being dragged (for skeleton rendering - outline only).
    dragged_window: Option<u64>,
}

/// A completed rendered frame sent back from the render thread.
struct RenderedFrame {
    pixels: Vec<u8>,
    width: u32,
    height: u32,
    stride: u32,
    format: PixelFormat,
    render_ms: f64,
    blur_enabled: bool,
    /// `true` when the renderer had text nodes whose glyphs were still
    /// being rasterised.  The main thread uses this to schedule a quick
    /// follow-up render so the real TrueType glyphs appear without delay.
    has_pending_glyphs: bool,
}

/// Message sent to the render thread.
enum RenderMsg {
    Job(RenderJob),
    Resize { width: u32, height: u32 },
    Shutdown,
}

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
    /// Compositor moved to the render thread after loading completes.
    compositor: Option<Compositor>,
    /// Synchronous renderer used only for the loading screen.
    /// Moved to the render thread after loading completes.
    renderer: Option<SoftwareRenderer>,
    input_state: InputState,
    width: u32,
    height: u32,
    /// Tile size used by the compositor.
    tile_size: u32,
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
    /// Channel to send render jobs to the background render thread.
    render_tx: Option<mpsc::Sender<RenderMsg>>,
    /// Channel to receive completed frames from the render thread.
    frame_rx: Option<mpsc::Receiver<RenderedFrame>>,
    /// Handle to the background render thread.
    render_thread: Option<thread::JoinHandle<()>>,
    /// Whether a render job is currently in flight (avoid double-submit).
    render_in_flight: bool,
    /// Telemetry system for performance monitoring.
    telemetry: TelemetryHandle,
    /// Whether developer mode is enabled (windowed + devtools).
    dev_mode: bool,
    /// DevTools panel (only active in dev_mode).
    devtools: Option<DevToolsPanel>,
}

impl DesktopCompositor {
    /// Create a new desktop compositor with the given initial resolution.
    ///
    /// Uses a 64-pixel tile size and the [`QualityProfile::Balanced`]
    /// profile.  The shell is initialized with matching screen dimensions.
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        // Load TrueType fonts before creating the renderer so that
        // all text is rendered with the proper typefaces.
        let mut font_db = liquide_font_rasterizer::FontDatabase::new();
        let font_count = font_db.load_default_fonts("assets");
        info!(fonts_loaded = font_count, "loaded TrueType font faces");

        let tile_size = 64;
        Self {
            shell: Shell::new(width as f32, height as f32),
            compositor: Some(Compositor::new(
                width,
                height,
                tile_size,
                QualityProfile::Balanced,
            )),
            renderer: Some(SoftwareRenderer::with_font_db(font_db)),
            input_state: InputState::new(),
            width,
            height,
            tile_size,
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
            render_tx: None,
            frame_rx: None,
            render_thread: None,
            render_in_flight: false,
            telemetry: create_telemetry(60), // 60fps target
            dev_mode: false,
            devtools: None,
        }
    }

    /// Enable developer mode (windowed, resizable, devtools available).
    pub fn set_dev_mode(&mut self, enabled: bool) {
        self.dev_mode = enabled;
        if enabled && self.devtools.is_none() {
            let mut panel = DevToolsPanel::with_defaults();
            panel.set_screen_size(self.width as f32, self.height as f32);
            self.devtools = Some(panel);

            // Load devtools structural CSS into the pipeline.
            static DEVTOOLS_CSS: &str = include_str!("../../../assets/themes/components/devtools.css");
            self.shell.add_stylesheet(DEVTOOLS_CSS);

            info!("devtools panel initialized (F12 to toggle)");
        } else if !enabled {
            // Unmount devtools from the DOM when disabling.
            self.shell.unmount_template("devtools-panel");
            self.devtools = None;
        }
    }

    /// Whether developer mode is enabled.
    pub fn is_dev_mode(&self) -> bool {
        self.dev_mode
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

        let mut root = SceneNode::new(0, SceneNodeKind::Root, NodeProperties::new(screen));

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
                NodeProperties::new(Rect::new(lx, brand_y, letter_w, letter_h)).with_z_order(13),
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

    /// Synchronise the devtools template into the shell DOM.
    ///
    /// Must be called **before** `shell.build_scene()` so the CSS pipeline
    /// can lay out and paint the devtools panel.  Uses the previous frame's
    /// layout / style data (one-frame-behind is expected for dev tools).
    fn sync_devtools_template(&mut self) {
        if !self.dev_mode {
            return;
        }

        // Determine visibility first with a shared borrow.
        let visible = self.devtools.as_ref().map_or(false, |d| d.is_visible());

        if visible {
            // Build the template from (previous frame's) data.
            // We clone just the TemplateNode out so all shared borrows are dropped
            // before the mutable mount call.
            let template = {
                let devtools = self.devtools.as_ref().unwrap();
                let doc = self.shell.document();
                match (self.shell.layout_tree(), self.shell.style_map()) {
                    (Some(layout), Some(styles)) => {
                        devtools.render_template(doc, layout, styles)
                    }
                    _ => {
                        // First frame — minimal stub so the pipeline has something.
                        liquide_devtools::TemplateNode::el("devtools-panel")
                            .id("devtools-panel")
                    }
                }
            };
            self.shell.mount_template("devtools-panel", &template);
        } else {
            self.shell.unmount_template("devtools-panel");
        }
    }

    /// Run one frame synchronously: build scene, render, present.
    ///
    /// Used only for the loading screen before the render thread is spawned.
    fn render_frame_sync(&mut self, platform: &mut dyn PlatformBackend) {
        let frame_start = Instant::now();

        // 1. Build the scene graph.
        let mut scene = if self.loading {
            self.build_loading_scene()
        } else {
            // Mount devtools template before the CSS pipeline runs.
            self.sync_devtools_template();
            self.shell.build_scene()
        };

        // 1b. Overlay devtools panel scene nodes (if active).
        if !self.loading && self.dev_mode {
            if let Some(ref mut devtools) = self.devtools {
                let doc = self.shell.document();
                // Refresh inspector tree from live DOM so the Elements tab is populated.
                devtools.refresh_inspector(doc);

                // Push pipeline stats for the Debugger tab.
                if let Ok(tel) = self.telemetry.read() {
                    let fm = tel.frame_metrics();
                    devtools.push_frame_snapshot(FrameSnapshot {
                        frame_number: self.frame_count,
                        fps: fm.current_fps,
                        avg_frame_ms: fm.avg_frame_ms,
                        css_rule_count: self.shell.css_rule_count(),
                        css_variable_count: self.shell.css_variable_count(),
                        stylesheet_count: self.shell.stylesheet_count(),
                        viewport_w: self.width as f32,
                        viewport_h: self.height as f32,
                    });
                }

                if let (Some(layout), Some(styles)) =
                    (self.shell.layout_tree(), self.shell.style_map())
                {
                    // Refresh scene graph debugger from the current scene.
                    devtools.scene_debugger.snapshot(&scene);
                    for node in devtools.build_scene(doc, layout, styles) {
                        scene.add_child(node);
                    }
                }
            }
        }

        // 2. Add software cursor to the scene.
        if !self.loading {
            let cursor_size = 24.0_f32;
            let cursor_bounds = Rect::new(self.cursor_x, self.cursor_y, cursor_size, cursor_size);
            scene.add_child(SceneNode::new(
                999_999,
                SceneNodeKind::Cursor {
                    shape: self.shell.cursor_shape(),
                },
                NodeProperties::new(cursor_bounds).with_z_order(9999),
            ));
        }

        // 3. Submit to compositor and swap buffers (during loading only).
        if let Some(ref mut compositor) = self.compositor {
            let _ = compositor.submit_scene(scene);
            compositor.begin_frame();
        } else {
            // Should not happen during loading screen
            return;
        }

        // 4. Full-screen damage.
        let tile_size = self.tile_size;
        let grid_w = self.width.div_ceil(tile_size);
        let grid_h = self.height.div_ceil(tile_size);
        let mut damage = DamageSet::new(tile_size);
        damage.mark_all(grid_w, grid_h);

        // 5. Flatten the scene.
        let flat_nodes = self
            .compositor
            .as_ref()
            .and_then(|c| c.scene())
            .map(|s| s.flatten())
            .unwrap_or_default();

        // 6. Render into the back buffer.
        if let (Some(renderer), Some(compositor)) = (&mut self.renderer, &mut self.compositor) {
            let fb = compositor.frame_buffer_mut();
            let _ = renderer.render(&flat_nodes, fb, &damage);
        }

        // 7. Present.
        if let Some(handle) = self.window_handle {
            if let Some(compositor) = self.compositor.as_ref() {
                let fb = compositor.frame_buffer();
                let _ = platform.present_frame(
                    handle, &fb.pixels, fb.width, fb.height, fb.stride, fb.format,
                );
            }
        }

        self.frame_count += 1;

        let frame_ms = frame_start.elapsed().as_secs_f64() * 1000.0;
        if let Some(ref mut compositor) = self.compositor {
            compositor.report_frame_time(frame_ms);
        }
    }

    /// Submit a render job to the background render thread.
    ///
    /// Builds lightweight scene graph and sends to render thread.
    /// Flattening and rendering happen asynchronously off the main thread.
    fn submit_render(&mut self) {
        if self.render_in_flight || self.render_tx.is_none() {
            return;
        }

        // Build the scene graph (lightweight tree construction).
        self.sync_devtools_template();
        let mut scene = self.shell.build_scene();

        // Overlay devtools panel scene nodes (if active).
        if self.dev_mode {
            if let Some(ref mut devtools) = self.devtools {
                let doc = self.shell.document();
                devtools.refresh_inspector(doc);

                // Push pipeline stats for the Debugger tab.
                if let Ok(tel) = self.telemetry.read() {
                    let fm = tel.frame_metrics();
                    devtools.push_frame_snapshot(FrameSnapshot {
                        frame_number: self.frame_count,
                        fps: fm.current_fps,
                        avg_frame_ms: fm.avg_frame_ms,
                        css_rule_count: self.shell.css_rule_count(),
                        css_variable_count: self.shell.css_variable_count(),
                        stylesheet_count: self.shell.stylesheet_count(),
                        viewport_w: self.width as f32,
                        viewport_h: self.height as f32,
                    });
                }

                if let (Some(layout), Some(styles)) =
                    (self.shell.layout_tree(), self.shell.style_map())
                {
                    devtools.scene_debugger.snapshot(&scene);
                    for node in devtools.build_scene(doc, layout, styles) {
                        scene.add_child(node);
                    }
                }
            }
        }

        // Get current state for telemetry.
        let dragged_window = self.shell.dragged_window();

        // Update telemetry for interactive window.
        if let Some(wid) = dragged_window {
            if let Ok(mut telemetry) = self.telemetry.write() {
                telemetry.set_window_interactive(wid.0, true);
            }
        }

        let job = RenderJob {
            scene,
            cursor_x: self.cursor_x,
            cursor_y: self.cursor_y,
            cursor_shape: self.shell.cursor_shape(),
            width: self.width,
            height: self.height,
            tile_size: self.tile_size,
            dragged_window: dragged_window.map(|wid| wid.0),
        };

        if let Some(ref tx) = self.render_tx {
            if tx.send(RenderMsg::Job(job)).is_ok() {
                self.render_in_flight = true;
            }
        }
    }

    /// Check for a completed frame from the render thread and present it.
    ///
    /// Returns `true` if a frame was presented.
    fn try_present(&mut self, platform: &mut dyn PlatformBackend) -> bool {
        let rx = match &self.frame_rx {
            Some(rx) => rx,
            None => return false,
        };

        match rx.try_recv() {
            Ok(frame) => {
                self.render_in_flight = false;

                // Present the rendered pixels.
                let t4 = Instant::now();
                if let Some(handle) = self.window_handle {
                    let _ = platform.present_frame(
                        handle,
                        &frame.pixels,
                        frame.width,
                        frame.height,
                        frame.stride,
                        frame.format,
                    );
                }
                let present_ms = t4.elapsed().as_secs_f64() * 1000.0;

                self.frame_count += 1;

                // Record telemetry.
                let total_frame_ms = frame.render_ms + present_ms;
                if let Ok(mut telemetry) = self.telemetry.write() {
                    telemetry.record_frame(total_frame_ms);
                }

                // Report timing.
                if let Some(ref mut compositor) = self.compositor {
                    compositor.report_frame_time(frame.render_ms + present_ms);
                }

                if self.debug_perf {
                    debug!(
                        frame = self.frame_count,
                        render_ms = format!("{:.2}", frame.render_ms),
                        present_ms = format!("{:.2}", present_ms),
                        blur = frame.blur_enabled,
                        "frame presented (threaded)"
                    );
                }

                if frame.render_ms > 100.0 {
                    warn!(
                        frame = self.frame_count,
                        render_ms = format!("{:.1}", frame.render_ms),
                        present_ms = format!("{:.1}", present_ms),
                        "slow frame detected"
                    );
                }

                // If the renderer still has glyphs being rasterised,
                // schedule an immediate follow-up render so the real
                // TrueType glyphs appear without visible delay.
                if frame.has_pending_glyphs {
                    self.dirty = true;
                }

                true
            }
            Err(mpsc::TryRecvError::Empty) => false,
            Err(mpsc::TryRecvError::Disconnected) => {
                warn!("render thread disconnected");
                self.render_in_flight = false;
                false
            }
        }
    }

    /// Spawn the background render thread.
    ///
    /// Moves the `SoftwareRenderer` and `Compositor` to a dedicated thread
    /// that processes render jobs asynchronously. This allows the main thread
    /// to keep processing input events while rendering happens in parallel.
    fn spawn_render_thread(&mut self) {
        let renderer = match self.renderer.take() {
            Some(r) => r,
            None => return, // already spawned
        };

        let compositor = match self.compositor.take() {
            Some(c) => c,
            None => return, // already spawned
        };

        let (job_tx, job_rx) = mpsc::channel::<RenderMsg>();
        let (frame_tx, frame_rx) = mpsc::channel::<RenderedFrame>();
        let debug_perf = self.debug_perf;

        let handle = thread::Builder::new()
            .name("render-worker".into())
            .spawn(move || {
                Self::render_thread_fn(renderer, compositor, job_rx, frame_tx, debug_perf);
            })
            .expect("failed to spawn render thread");

        self.render_tx = Some(job_tx);
        self.frame_rx = Some(frame_rx);
        self.render_thread = Some(handle);

        info!("render thread spawned");
    }

    /// The render thread's main loop.
    /// Handles scene flattening, skeleton filtering, and rendering.
    fn render_thread_fn(
        mut renderer: SoftwareRenderer,
        mut compositor: Compositor,
        rx: mpsc::Receiver<RenderMsg>,
        tx: mpsc::Sender<RenderedFrame>,
        _debug_perf: bool,
    ) {
        let mut fb: Option<FrameBuffer> = None;

        while let Ok(msg) = rx.recv() {
            match msg {
                RenderMsg::Shutdown => break,
                RenderMsg::Resize { width, height } => {
                    let _ = compositor.resize(width, height);
                    // Recreate framebuffer on next render.
                    if fb
                        .as_ref()
                        .is_some_and(|f| f.width != width || f.height != height)
                    {
                        fb = None;
                    }
                }
                RenderMsg::Job(job) => {
                    // Drain any stale jobs — only render the latest.
                    let mut latest_job = job;
                    while let Ok(msg) = rx.try_recv() {
                        match msg {
                            RenderMsg::Shutdown => return,
                            RenderMsg::Resize { width, height } => {
                                let _ = compositor.resize(width, height);
                                fb = None;
                            }
                            RenderMsg::Job(j) => {
                                latest_job = j;
                            }
                        }
                    }

                    let t_total = Instant::now();

                    // 1. Add cursor to scene.
                    let mut scene = latest_job.scene;
                    let cursor_size = 24.0_f32;
                    let cursor_bounds = Rect::new(
                        latest_job.cursor_x,
                        latest_job.cursor_y,
                        cursor_size,
                        cursor_size,
                    );
                    scene.add_child(SceneNode::new(
                        999_999,
                        SceneNodeKind::Cursor {
                            shape: latest_job.cursor_shape,
                        },
                        NodeProperties::new(cursor_bounds).with_z_order(9999),
                    ));

                    // 2. Submit to compositor and flatten.
                    let _ = compositor.submit_scene(scene);
                    compositor.begin_frame();

                    let mut flat_nodes =
                        compositor.scene().map(|s| s.flatten()).unwrap_or_default();

                    // 3. Skeleton mode filtering during drag.
                    if let Some(window_id) = latest_job.dragged_window {
                        const NODE_WINDOW_BASE: u64 = 10_000;
                        const NODE_WINDOW_STRIDE: u64 = 10;
                        let win_base = NODE_WINDOW_BASE + window_id * NODE_WINDOW_STRIDE;
                        let win_end = win_base + NODE_WINDOW_STRIDE;

                        flat_nodes.retain(|node| {
                            let node_id = node.id;
                            let is_dragged_window_node = node_id >= win_base && node_id < win_end;

                            if is_dragged_window_node {
                                // For dragged window: only keep basic decoration border
                                matches!(node.kind, SceneNodeKind::Decoration { .. })
                            } else {
                                // All other windows and UI elements: render normally
                                true
                            }
                        });
                    }

                    // 4. Ensure framebuffer matches requested dimensions.
                    let needs_new = fb.as_ref().map_or(true, |f| {
                        f.width != latest_job.width || f.height != latest_job.height
                    });
                    if needs_new {
                        fb = Some(FrameBuffer::new(
                            latest_job.width,
                            latest_job.height,
                            PixelFormat::Bgra8,
                        ));
                    }
                    let framebuf = fb.as_mut().unwrap();

                    // 5. Build damage set.
                    let mut damage = DamageSet::new(latest_job.tile_size);
                    let grid_w = latest_job.width.div_ceil(latest_job.tile_size);
                    let grid_h = latest_job.height.div_ceil(latest_job.tile_size);
                    damage.mark_all(grid_w, grid_h);

                    // 6. Render with performance optimizations for dragging.
                    let t_render = Instant::now();

                    let saved_blur = renderer.blur_enabled();
                    let saved_lod_mode = renderer.get_lod_performance_mode();

                    if latest_job.dragged_window.is_some() && saved_blur {
                        renderer.set_blur_enabled(false);
                    }
                    if latest_job.dragged_window.is_some() {
                        renderer.set_lod_performance_mode(
                            liquide_renderer_cpu::lod::PerformanceMode::Performance,
                        );
                    }
                    renderer.set_skeleton_window(latest_job.dragged_window);

                    let _ = renderer.render(&flat_nodes, framebuf, &damage);

                    // Restore rendering quality.
                    renderer.set_skeleton_window(None);
                    if latest_job.dragged_window.is_some() && saved_blur {
                        renderer.set_blur_enabled(true);
                    }
                    if latest_job.dragged_window.is_some() {
                        renderer.set_lod_performance_mode(saved_lod_mode);
                    }

                    let render_ms = t_render.elapsed().as_secs_f64() * 1000.0;
                    let total_ms = t_total.elapsed().as_secs_f64() * 1000.0;

                    // Report render time for adaptive blur.
                    renderer.report_render_time(render_ms);

                    // Report frame time to compositor.
                    compositor.report_frame_time(total_ms);

                    // Send completed frame back.
                    let result = RenderedFrame {
                        pixels: framebuf.pixels.clone(),
                        width: framebuf.width,
                        height: framebuf.height,
                        stride: framebuf.stride,
                        format: framebuf.format,
                        render_ms: total_ms,
                        blur_enabled: renderer.blur_enabled(),
                        has_pending_glyphs: renderer.has_pending_glyphs(),
                    };

                    if tx.send(result).is_err() {
                        break; // main thread dropped
                    }
                }
            }
        }
    }

    /// Handle a platform event: route through shell and input state.
    ///
    /// Returns `true` if the event requires a redraw.
    pub fn handle_event(&mut self, event: &PlatformEvent) -> bool {
        let mut needs_redraw = false;

        match event {
            PlatformEvent::WindowResized { width, height, .. } => {
                self.width = *width;
                self.height = *height;

                // During loading, resize compositor directly
                if let Some(ref mut compositor) = self.compositor {
                    let _ = compositor.resize(*width, *height);
                } else if let Some(ref tx) = self.render_tx {
                    // After loading, notify render thread
                    let _ = tx.send(RenderMsg::Resize {
                        width: *width,
                        height: *height,
                    });
                }

                self.shell.resize_screen(*width as f32, *height as f32);
                if let Some(ref mut devtools) = self.devtools {
                    devtools.set_screen_size(*width as f32, *height as f32);
                }
                needs_redraw = true;
            }
            PlatformEvent::WindowCloseRequested { .. } | PlatformEvent::Quit => {
                self.running = false;
            }
            PlatformEvent::WindowRedraw { .. } => {
                needs_redraw = true;
            }
            PlatformEvent::KeyInput { event: ke, .. } => {
                // DevTools keyboard shortcuts (intercept before shell).
                if self.dev_mode {
                    if let Some(ref mut devtools) = self.devtools {
                        if ke.state == KeyState::Pressed {
                            // Map KeyCode to a string for devtools handle_key.
                            let key_str: Option<&str> = match ke.key {
                                KeyCode::F12 => Some("F12"),
                                KeyCode::Tab => Some("Tab"),
                                KeyCode::Escape => Some("Escape"),
                                KeyCode::Enter => Some("Enter"),
                                KeyCode::Backspace => Some("Backspace"),
                                KeyCode::Delete => Some("Delete"),
                                KeyCode::ArrowUp => Some("ArrowUp"),
                                KeyCode::ArrowDown => Some("ArrowDown"),
                                KeyCode::ArrowLeft => Some("ArrowLeft"),
                                KeyCode::ArrowRight => Some("ArrowRight"),
                                KeyCode::Home => Some("Home"),
                                KeyCode::End => Some("End"),
                                KeyCode::Space => Some(" "),
                                // Letters.
                                KeyCode::A => Some(if ke.modifiers.shift() { "A" } else { "a" }),
                                KeyCode::B => Some(if ke.modifiers.shift() { "B" } else { "b" }),
                                KeyCode::C => Some(if ke.modifiers.shift() { "C" } else { "c" }),
                                KeyCode::D => Some(if ke.modifiers.shift() { "D" } else { "d" }),
                                KeyCode::E => Some(if ke.modifiers.shift() { "E" } else { "e" }),
                                KeyCode::F => Some(if ke.modifiers.shift() { "F" } else { "f" }),
                                KeyCode::G => Some(if ke.modifiers.shift() { "G" } else { "g" }),
                                KeyCode::H => Some(if ke.modifiers.shift() { "H" } else { "h" }),
                                KeyCode::I => Some(if ke.modifiers.shift() { "I" } else { "i" }),
                                KeyCode::J => Some(if ke.modifiers.shift() { "J" } else { "j" }),
                                KeyCode::K => Some(if ke.modifiers.shift() { "K" } else { "k" }),
                                KeyCode::L => Some(if ke.modifiers.shift() { "L" } else { "l" }),
                                KeyCode::M => Some(if ke.modifiers.shift() { "M" } else { "m" }),
                                KeyCode::N => Some(if ke.modifiers.shift() { "N" } else { "n" }),
                                KeyCode::O => Some(if ke.modifiers.shift() { "O" } else { "o" }),
                                KeyCode::P => Some(if ke.modifiers.shift() { "P" } else { "p" }),
                                KeyCode::Q => Some(if ke.modifiers.shift() { "Q" } else { "q" }),
                                KeyCode::R => Some(if ke.modifiers.shift() { "R" } else { "r" }),
                                KeyCode::S => Some(if ke.modifiers.shift() { "S" } else { "s" }),
                                KeyCode::T => Some(if ke.modifiers.shift() { "T" } else { "t" }),
                                KeyCode::U => Some(if ke.modifiers.shift() { "U" } else { "u" }),
                                KeyCode::V => Some(if ke.modifiers.shift() { "V" } else { "v" }),
                                KeyCode::W => Some(if ke.modifiers.shift() { "W" } else { "w" }),
                                KeyCode::X => Some(if ke.modifiers.shift() { "X" } else { "x" }),
                                KeyCode::Y => Some(if ke.modifiers.shift() { "Y" } else { "y" }),
                                KeyCode::Z => Some(if ke.modifiers.shift() { "Z" } else { "z" }),
                                // Digits / shifted symbols.
                                KeyCode::Digit0 => {
                                    Some(if ke.modifiers.shift() { ")" } else { "0" })
                                }
                                KeyCode::Digit1 => {
                                    Some(if ke.modifiers.shift() { "!" } else { "1" })
                                }
                                KeyCode::Digit2 => {
                                    Some(if ke.modifiers.shift() { "@" } else { "2" })
                                }
                                KeyCode::Digit3 => {
                                    Some(if ke.modifiers.shift() { "#" } else { "3" })
                                }
                                KeyCode::Digit4 => {
                                    Some(if ke.modifiers.shift() { "$" } else { "4" })
                                }
                                KeyCode::Digit5 => {
                                    Some(if ke.modifiers.shift() { "%" } else { "5" })
                                }
                                KeyCode::Digit6 => {
                                    Some(if ke.modifiers.shift() { "^" } else { "6" })
                                }
                                KeyCode::Digit7 => {
                                    Some(if ke.modifiers.shift() { "&" } else { "7" })
                                }
                                KeyCode::Digit8 => {
                                    Some(if ke.modifiers.shift() { "*" } else { "8" })
                                }
                                KeyCode::Digit9 => {
                                    Some(if ke.modifiers.shift() { "(" } else { "9" })
                                }
                                // Punctuation.
                                KeyCode::Period => {
                                    Some(if ke.modifiers.shift() { ">" } else { "." })
                                }
                                KeyCode::Comma => {
                                    Some(if ke.modifiers.shift() { "<" } else { "," })
                                }
                                KeyCode::Slash => {
                                    Some(if ke.modifiers.shift() { "?" } else { "/" })
                                }
                                KeyCode::Semicolon => {
                                    Some(if ke.modifiers.shift() { ":" } else { ";" })
                                }
                                KeyCode::Quote => {
                                    Some(if ke.modifiers.shift() { "\"" } else { "'" })
                                }
                                KeyCode::BracketLeft => {
                                    Some(if ke.modifiers.shift() { "{" } else { "[" })
                                }
                                KeyCode::BracketRight => {
                                    Some(if ke.modifiers.shift() { "}" } else { "]" })
                                }
                                KeyCode::Backslash => {
                                    Some(if ke.modifiers.shift() { "|" } else { "\\" })
                                }
                                KeyCode::Minus => {
                                    Some(if ke.modifiers.shift() { "_" } else { "-" })
                                }
                                KeyCode::Equal => {
                                    Some(if ke.modifiers.shift() { "+" } else { "=" })
                                }
                                KeyCode::Grave => {
                                    Some(if ke.modifiers.shift() { "~" } else { "`" })
                                }
                                _ => None,
                            };
                            if let Some(k) = key_str {
                                // For Enter in console, also pass doc/layout/styles for command execution.
                                if k == "Enter" && devtools.is_console_focused() {
                                    if let (Some(layout), Some(styles)) =
                                        (self.shell.layout_tree(), self.shell.style_map())
                                    {
                                        let doc = self.shell.document();
                                        devtools.handle_console_key(
                                            "Enter", false, false, doc, layout, styles,
                                        );
                                        needs_redraw = true;
                                    }
                                } else if devtools.handle_key(
                                    k,
                                    ke.modifiers.ctrl(),
                                    ke.modifiers.shift(),
                                    ke.modifiers.alt(),
                                ) {
                                    needs_redraw = true;
                                }
                            }
                        }
                    }
                }
                self.input_state.handle_event(&InputEvent::Keyboard(*ke));
            }
            PlatformEvent::MouseInput { event: me, .. } => {
                // Track cursor position for software cursor rendering.
                use liquide_input::mouse::MouseEvent;
                match me {
                    MouseEvent::Move { x, y } => {
                        // Only redraw if cursor position actually changed
                        // (avoid redundant full redraws on minor sub-pixel jitter).
                        let new_x = *x;
                        let new_y = *y;
                        if (new_x - self.cursor_x).abs() > 0.1
                            || (new_y - self.cursor_y).abs() > 0.1
                        {
                            self.cursor_x = new_x;
                            self.cursor_y = new_y;
                            needs_redraw = true;
                        }
                        // Forward to devtools element picker.
                        if self.dev_mode {
                            if let Some(ref mut devtools) = self.devtools {
                                if let (Some(hit_test), Some(layout)) =
                                    (self.shell.hit_test_engine(), self.shell.layout_tree())
                                {
                                    let doc = self.shell.document();
                                    if devtools.on_mouse_move(new_x, new_y, hit_test, doc, layout) {
                                        needs_redraw = true;
                                    }
                                }
                            }
                        }
                    }
                    MouseEvent::Button {
                        x,
                        y,
                        button,
                        state,
                    } => {
                        self.cursor_x = *x;
                        self.cursor_y = *y;
                        // Only react on button press, not release.
                        if *state == liquide_input::mouse::ButtonState::Pressed
                            && *button == liquide_input::mouse::MouseButton::Left
                        {
                            // Forward click to devtools panel (tabs, tree nodes, etc.)
                            // and element picker / viewport click-to-inspect.
                            if self.dev_mode {
                                if let Some(ref mut devtools) = self.devtools {
                                    if let Some(styles) = self.shell.style_map() {
                                        if devtools.on_panel_click(*x, *y, styles) {
                                            needs_redraw = true;
                                        } else if devtools.on_click(styles) {
                                            needs_redraw = true;
                                        } else if let Some(hit_test) = self.shell.hit_test_engine()
                                        {
                                            // Click-to-inspect: clicking outside the panel
                                            // selects the element under the cursor.
                                            if devtools.on_viewport_click(*x, *y, hit_test, styles)
                                            {
                                                needs_redraw = true;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        // Right-click: context menu in devtools.
                        if *state == liquide_input::mouse::ButtonState::Pressed
                            && *button == liquide_input::mouse::MouseButton::Right
                        {
                            if self.dev_mode {
                                if let Some(ref mut devtools) = self.devtools {
                                    if let Some(styles) = self.shell.style_map() {
                                        if devtools.on_right_click(*x, *y, styles) {
                                            needs_redraw = true;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    MouseEvent::Scroll { delta, x, y, .. } => {
                        // Forward scroll to devtools panel.
                        if self.dev_mode {
                            if let Some(ref mut devtools) = self.devtools {
                                // Convert scroll delta: positive delta = scroll up
                                // in most platform conventions, but we want positive
                                // = scroll content down (increase offset).
                                let scroll_px = -delta * 36.0;
                                if devtools.on_scroll(*x, *y, scroll_px) {
                                    needs_redraw = true;
                                }
                            }
                        }
                    }
                    _ => {}
                }
                self.input_state.handle_event(&InputEvent::Mouse(*me));
            }
            PlatformEvent::TouchInput { event: te, .. } => {
                self.input_state.handle_event(&InputEvent::Touch(*te));
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
        // In dev mode, keep the requested resolution for windowed mode.
        if !self.dev_mode {
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
                self.cursor_x = screen_w as f32 / 2.0;
                self.cursor_y = screen_h as f32 / 2.0;
            }
        }

        // Create a borderless fullscreen desktop window, or a resizable
        // windowed mode when dev_mode is active.
        debug!("creating desktop window {}x{}", self.width, self.height);
        let t_win = Instant::now();
        let params = if self.dev_mode {
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
            windowed = self.dev_mode,
            elapsed_ms = format!("{:.1}", t_win.elapsed().as_secs_f64() * 1000.0),
            "desktop window created"
        );

        // Show loading overlay (synchronous — render thread not spawned yet).
        debug!("rendering loading overlay");
        self.loading = true;
        self.render_frame_sync(platform);
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
        self.dirty = false;
        info!(
            elapsed_ms = format!("{:.1}", run_start.elapsed().as_secs_f64() * 1000.0),
            "first desktop frame presented"
        );

        // Spawn the background render thread now that loading is done.
        self.spawn_render_thread();

        // Non-blocking event loop with threaded rendering.
        //
        // The main thread handles input events and scene building.
        // The render thread handles the expensive CPU rendering in parallel.
        // This ensures the shell stays responsive to mouse/keyboard input
        // even when rendering takes hundreds of milliseconds.
        info!(
            fps_cap = if self.frame_interval.is_zero() {
                0
            } else {
                (1_000_000 / self.frame_interval.as_micros().max(1)) as u32
            },
            debug_perf = self.debug_perf,
            "entering threaded event loop"
        );

        let mut last_telemetry_report = Instant::now();
        let telemetry_report_interval = Duration::from_secs(10);

        while self.running {
            // Drain all pending events.
            let mut had_event = false;
            while let Some(event) = platform.poll_event() {
                had_event = true;
                if self.handle_event(&event) {
                    self.dirty = true;
                }
            }

            // Check for completed frames from the render thread.
            if self.try_present(platform) {
                self.last_render = Instant::now();
                // If still dirty (events arrived during rendering),
                // submit a new render job immediately.
                if self.dirty {
                    self.submit_render();
                    self.dirty = false;
                }
            }

            // Periodic tick every ~1s for clock / notification expiry.
            if self.last_tick.elapsed().as_millis() >= 1000 {
                if self.tick() {
                    self.dirty = true;
                }
                self.last_tick = Instant::now();
            }

            // Periodic telemetry report every 10 seconds.
            if last_telemetry_report.elapsed() >= telemetry_report_interval {
                self.print_telemetry_report();
                last_telemetry_report = Instant::now();
            }

            // Submit a render job if dirty and render thread is free.
            if self.dirty && !self.render_in_flight {
                // During drag, bypass frame interval throttle for immediate
                // visual feedback — the blur suppression keeps frame cost low.
                let can_render = self.shell.is_dragging()
                    || self.frame_interval.is_zero()
                    || self.last_render.elapsed() >= self.frame_interval;
                if can_render {
                    self.submit_render();
                    self.dirty = false;
                }
            }

            // Efficient idle: sleep to avoid busy-spinning.
            if self.render_in_flight {
                // Render in progress — brief yield to check for completion
                // and events frequently for responsive input.
                thread::sleep(Duration::from_millis(1));
            } else if self.dirty && !self.frame_interval.is_zero() {
                // Dirty but throttled — sleep until next frame is due.
                let elapsed = self.last_render.elapsed();
                if elapsed < self.frame_interval {
                    let remaining = self.frame_interval - elapsed;
                    thread::sleep(remaining.min(Duration::from_millis(4)));
                }
            } else if !self.dirty {
                // Nothing to render — sleep longer when no events arriving.
                let sleep_ms = if had_event {
                    1
                } else {
                    self.frame_interval.as_millis().clamp(1, 16) as u64
                };
                thread::sleep(Duration::from_millis(sleep_ms));
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
#[allow(dead_code)]
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
        SceneNodeKind::Cursor { .. } => "Cursor",
        SceneNodeKind::Text { .. } => "Text",
        SceneNodeKind::Icon { .. } => "Icon",
        SceneNodeKind::LockScreen => "LockScreen",
        SceneNodeKind::CrashScreen => "CrashScreen",
        SceneNodeKind::Workspace { .. } => "Workspace",
        SceneNodeKind::RenderLayer { .. } => "RenderLayer",
        SceneNodeKind::ClipPath { .. } => "ClipPath",
        SceneNodeKind::Filter { .. } => "Filter",
        SceneNodeKind::Image { .. } => "Image",
        SceneNodeKind::GradientFill { .. } => "GradientFill",
        SceneNodeKind::BackdropFilter { .. } => "BackdropFilter",
        SceneNodeKind::BackgroundFill { .. } => "BackgroundFill",
        SceneNodeKind::Outline { .. } => "Outline",
        SceneNodeKind::BoxShadows { .. } => "BoxShadows",
        SceneNodeKind::Mask { .. } => "Mask",
        SceneNodeKind::Border { .. } => "Border",
        SceneNodeKind::BorderImage { .. } => "BorderImage",
        SceneNodeKind::TextCaret { .. } => "TextCaret",
        SceneNodeKind::SelectionOverlay { .. } => "SelectionOverlay",
    }
}

/// Extract color info from a scene node kind for debug logging.
#[cfg(debug_assertions)]
#[allow(dead_code)]
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

impl DesktopCompositor {
    /// Get a clone of the telemetry handle for monitoring.
    pub fn telemetry(&self) -> TelemetryHandle {
        self.telemetry.clone()
    }

    /// Print comprehensive telemetry status report to log.
    pub fn print_telemetry_report(&self) {
        if let Ok(telemetry) = self.telemetry.read() {
            let report = telemetry.status_report();
            info!("\n{}", report);
        }
    }
}
