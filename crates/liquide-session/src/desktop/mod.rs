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

mod cursor_state;
mod debug;
mod devtools;
mod devtools_state;
mod event_handling;
mod event_loop;
mod loading;
pub mod lockfree_queue;
mod paint_state;
mod render_thread;
mod scene_split;
mod tile_state;
mod window_render;

use std::sync::Arc;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use liquide_compositor::Compositor;
use liquide_compositor::Renderer;
use liquide_compositor::effects::QualityProfile;
use liquide_encoder::tile::TileBatch;
use liquide_input::InputState;
use liquide_platform::NativeWindowHandle;
use liquide_render_coordinator::metrics::MetricsCollector;
use liquide_renderer_cpu::SoftwareRenderer;
use liquide_shell::Shell;
use liquide_telemetry_viewer::metrics::MetricsRegistry;
use tracing::info;

use crate::telemetry::{TelemetryHandle, create_telemetry};

use cursor_state::CursorState;
use devtools_state::DevToolsState;
use paint_state::PaintState;
use render_thread::{RenderMsg, RenderedFrame};
use tile_state::TileEncoderState;
use window_render::WindowRenderManager;

#[derive(Debug, Clone, Copy, Default)]
struct PresentPacingState {
    awaiting_ack: bool,
    last_acknowledged_present_count: u64,
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
    renderer: Option<Box<dyn Renderer>>,
    input_state: InputState,
    width: u32,
    height: u32,
    window_handle: Option<NativeWindowHandle>,
    frame_count: u64,
    running: bool,
    dirty: bool,
    last_render: Instant,
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
    /// Tracks backend present readiness for queued standalone pacing.
    present_pacing: PresentPacingState,
    /// Telemetry system for performance monitoring.
    telemetry: TelemetryHandle,
    /// Cursor position, shape, and hardware/software cursor management.
    cursor: CursorState,
    /// DevTools panel lifecycle and integration.
    dt: DevToolsState,
    /// Tile encoding for remote frame transmission.
    tiles: TileEncoderState,
    /// Paint coalescing and timer management (from liquide-message-queue).
    paint: PaintState,
    /// Render pipeline metrics (from liquide-render-coordinator).
    render_metrics: Arc<MetricsCollector>,
    /// Per-window chrome/content render thread pairs (opt-in fault isolation).
    window_render: WindowRenderManager,
    /// Telemetry viewer metrics registry — counters, gauges, histograms.
    viewer_metrics: MetricsRegistry,
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

        let mut shell = Shell::new(width as f32, height as f32);

        // Try loading external CSS themes from disk.
        Self::load_external_css(&mut shell);

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
            renderer: Some(Box::new(SoftwareRenderer::with_font_db(font_db))),
            input_state: InputState::new(),
            width,
            height,
            window_handle: None,
            frame_count: 0,
            running: true,
            dirty: true,
            last_render: Instant::now(),
            loading: true,
            frame_interval: Duration::from_millis(16), // ~60fps default
            debug_perf: false,
            render_tx: None,
            frame_rx: None,
            render_thread: None,
            render_in_flight: false,
            present_pacing: PresentPacingState::default(),
            telemetry: create_telemetry(60), // 60fps target
            cursor: CursorState::new(width as f32 / 2.0, height as f32 / 2.0),
            dt: DevToolsState::new(),
            tiles: TileEncoderState::new(width, height, tile_size),
            paint: PaintState::new(),
            render_metrics: Arc::new(MetricsCollector::new()),
            window_render: WindowRenderManager::new(),
            viewer_metrics: MetricsRegistry::with_builtins(),
        }
    }

    /// Drain encoded tile batches ready for network transmission.
    pub fn drain_encoded_batches(&mut self) -> Vec<TileBatch> {
        self.tiles.drain_batches()
    }

    /// Enable developer mode (windowed, resizable, devtools available).
    pub fn set_dev_mode(&mut self, enabled: bool) {
        self.dt
            .set_dev_mode(enabled, &mut self.shell, self.width, self.height);
    }

    /// Whether developer mode is enabled.
    pub fn is_dev_mode(&self) -> bool {
        self.dt.dev_mode
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

    /// Snapshot of render pipeline metrics (throughput, latency percentiles).
    #[must_use]
    pub fn render_metrics(&self) -> liquide_render_coordinator::metrics::RenderMetrics {
        self.render_metrics.snapshot()
    }

    /// Try loading external CSS theme files and user overrides.
    ///
    /// Search order:
    /// 1. `assets/themes/{theme_name}.css` — packaged themes
    /// 2. `~/.config/liquide/custom.css` — user overrides
    ///
    /// Any CSS found is appended to the shell's stylesheet pipeline.
    fn load_external_css(shell: &mut Shell) {
        // Try theme CSS from assets directory
        let theme_names = ["night", "liquid-glass", "sunset", "midday"];
        for name in &theme_names {
            let candidate = std::path::Path::new("assets")
                .join("themes")
                .join(format!("{}.css", name));
            if candidate.exists() {
                if let Ok(css) = std::fs::read_to_string(&candidate) {
                    info!(
                        theme = name,
                        "loaded external CSS theme from {:?}", candidate
                    );
                    shell.add_stylesheet(&css);
                }
            }
        }

        // Try user custom CSS
        let home = {
            #[cfg(windows)]
            {
                std::env::var_os("USERPROFILE").map(std::path::PathBuf::from)
            }
            #[cfg(not(windows))]
            {
                std::env::var_os("HOME").map(std::path::PathBuf::from)
            }
        };
        if let Some(home_dir) = home {
            let custom_css = home_dir.join(".config").join("liquide").join("custom.css");
            if custom_css.exists() {
                if let Ok(css) = std::fs::read_to_string(&custom_css) {
                    info!("loaded user custom CSS from {:?}", custom_css);
                    shell.add_stylesheet(&css);
                }
            }
        }
    }
}
