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

mod debug;
mod devtools;
mod event_handling;
mod event_loop;
mod loading;
mod render_thread;

use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use liquide_compositor::effects::QualityProfile;
use liquide_compositor::Compositor;
use liquide_devtools::DevToolsPanel;
use liquide_input::InputState;
use liquide_platform::NativeWindowHandle;
use liquide_renderer_cpu::SoftwareRenderer;
use liquide_shell::Shell;
use tracing::info;

use crate::telemetry::{TelemetryHandle, create_telemetry};

use render_thread::{RenderMsg, RenderedFrame};

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

        let shell = Shell::new(width as f32, height as f32);

        // Load @font-face rules from CSS stylesheets into the font database.
        let mut css_font_count = 0usize;
        for face in shell.font_faces() {
            use liquide_theme_css::value::FontSource;
            let weight = face.weight.map(|(lo, _)| lo).unwrap_or(400);
            let italic = face.style.as_deref() == Some("italic");
            for src in &face.sources {
                match src {
                    FontSource::Url { url, .. } => {
                        // Resolve relative to assets directory
                        let path = if url.starts_with('/') || url.contains("://") {
                            std::path::PathBuf::from(url)
                        } else {
                            std::path::PathBuf::from("assets").join(url)
                        };
                        if path.exists() {
                            if font_db
                                .load_file(&path, &face.family, weight, italic)
                                .is_ok()
                            {
                                css_font_count += 1;
                                break; // First successful source wins
                            }
                        }
                    }
                    FontSource::Local(name) => {
                        // Check if already loaded by family name
                        if font_db.resolve(name, weight, italic).is_some() {
                            break; // Already available
                        }
                    }
                }
            }
        }
        if css_font_count > 0 {
            info!(
                css_fonts = css_font_count,
                "loaded @font-face fonts from CSS"
            );
        }

        let tile_size = 64;
        Self {
            shell,
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
            static DEVTOOLS_CSS: &str =
                include_str!("../../../../assets/themes/components/devtools.css");
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
