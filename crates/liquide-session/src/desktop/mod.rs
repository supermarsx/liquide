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
use liquide_transport::tile_channel::TileSender;
use tracing::info;

use crate::telemetry::{TelemetryHandle, create_telemetry};

use cursor_state::CursorState;
use devtools_state::DevToolsState;
use paint_state::PaintState;
use render_thread::{RenderMsg, RenderedFrame};
use tile_state::TileEncoderState;
use window_render::WindowRenderManager;

/// A single captured desktop frame returned by
/// [`DesktopCompositor::capture_once`]. Re-exported for the visual-test harness.
pub use render_thread::CapturedFrame;

const DEFAULT_TARGET_FPS: u32 = 60;

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
    /// Set when a Quit / close event arrives. The event loop honours this only
    /// AFTER flushing any in-flight frame so the final desktop state is
    /// presented before exit (no black/stale flash on close — t60-runtime #1).
    quit_requested: bool,
    dirty: bool,
    /// Optional accumulated damage for the pending dirty frame.
    ///
    /// `None` means the next full-scene job must repaint the whole frame.
    /// `Some` means the renderer can use those tiles as a damage hint.
    dirty_damage: Option<liquide_compositor::damage::DamageSet>,
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
    /// When the current in-flight render job was submitted. Used by the event
    /// loop's watchdog to recover from a hung (non-panicking) render thread that
    /// would otherwise leave `render_in_flight` stuck true and spin the main
    /// loop at 100% CPU forever (t60-runtime #3). `None` when nothing is in
    /// flight.
    render_inflight_since: Option<Instant>,
    /// Tracks backend present readiness for queued standalone pacing.
    present_pacing: PresentPacingState,
    /// Counts completed render frames received from the worker, used to gate
    /// presents on dirty/damage with a periodic keepalive (so a fully static
    /// scene does not flood the present/RDP path every frame — t59-present #2).
    present_gate_counter: u64,
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
        let asset_root = Self::resolve_asset_root();
        let font_count = font_db.load_default_fonts(&asset_root);
        if font_count == 0 {
            tracing::warn!(
                fonts_dir = ?asset_root.join("fonts"),
                "no TrueType font faces loaded from disk; text will fall back to \
                 the embedded/bitmap font (run scripts/download-fonts.ps1 or set \
                 LIQUIDE_ASSETS_DIR)"
            );
        } else {
            info!(fonts_loaded = font_count, "loaded TrueType font faces");
        }

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
                        // Resolve relative to the resolved assets directory
                        let path = if url.starts_with('/') || url.contains("://") {
                            std::path::PathBuf::from(url)
                        } else {
                            asset_root.join(url)
                        };
                        if path.exists() {
                            if font_db
                                .load_file(&path, &face.family, weight, italic)
                                .is_ok()
                            {
                                css_font_count += 1;
                                break; // First successful source wins
                            }
                        } else {
                            tracing::warn!(
                                family = %face.family,
                                ?path,
                                "@font-face source file not found; skipping source"
                            );
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
            quit_requested: false,
            dirty: true,
            dirty_damage: None,
            last_render: Instant::now(),
            loading: true,
            frame_interval: Duration::from_micros(1_000_000 / DEFAULT_TARGET_FPS as u64),
            debug_perf: false,
            render_tx: None,
            frame_rx: None,
            render_thread: None,
            render_in_flight: false,
            render_inflight_since: None,
            present_pacing: PresentPacingState::default(),
            present_gate_counter: 0,
            telemetry: create_telemetry(DEFAULT_TARGET_FPS),
            cursor: CursorState::new(width as f32 / 2.0, height as f32 / 2.0),
            dt: DevToolsState::new(),
            tiles: TileEncoderState::new(width, height, tile_size),
            paint: PaintState::new(),
            render_metrics: Arc::new(MetricsCollector::new()),
            window_render: WindowRenderManager::new(),
            viewer_metrics: MetricsRegistry::with_builtins(),
        }
    }

    /// Attach a remote transport sink for encoded tile batches.
    ///
    /// When a session is serving a remote client, the caller wires the
    /// transport [`TileSender`] here. From then on every frame encoded by the
    /// desktop loop (in `try_present`) is forwarded to the sink, so the
    /// tile-encode buffer actually drains to the network instead of only being
    /// bounded by the drop-oldest ring (t55-E8).
    ///
    /// On the local-display path no sink is attached, so the bounded ring (cap
    /// from t50-e18) remains the sole, memory-safe behaviour. If the attached
    /// transport later disconnects, the encoder transparently falls back to the
    /// bounded ring — never an unbounded leak.
    pub fn attach_remote_tile_sink(&mut self, sink: TileSender) {
        self.tiles.attach_sink(sink);
    }

    /// Drain encoded tile batches ready for network transmission.
    ///
    /// This is the pull-style drain retained for callers that poll the encoder
    /// directly (and for tests). When a transport sink is attached via
    /// [`Self::attach_remote_tile_sink`], batches are forwarded automatically
    /// and this returns whatever (if anything) remains buffered.
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

        if let Ok(mut telemetry) = self.telemetry.write() {
            telemetry.set_target_fps(fps);
        }

        if let Some(compositor) = self.compositor.as_mut() {
            compositor.set_target_fps(fps);
        }

        self.tiles.set_target_fps(fps);
        self.shell
            .set_frame_delta_ms(if fps > 0 { 1000.0 / fps as f32 } else { 16.667 });
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

    /// Resolve the directory that contains the packaged `assets/` tree.
    ///
    /// Asset loading used to be unconditionally relative to the process CWD,
    /// which silently failed whenever the binary was launched from anywhere
    /// other than the repository root (t56-H3). This resolver tries, in order:
    ///
    /// 1. `$LIQUIDE_ASSETS_DIR` — explicit operator override (used verbatim).
    /// 2. `./assets` relative to the current working directory.
    /// 3. `<exe-dir>/assets` — next to the running executable (installed/dev).
    /// 4. `<CARGO_MANIFEST_DIR>/../../assets` — the workspace-root `assets/`
    ///    tree, so the binary finds assets when run from a crate subdirectory
    ///    or via `cargo run` from anywhere in the tree.
    ///
    /// Returns the first candidate whose directory exists. If none exist it
    /// falls back to `./assets` (preserving the historical behaviour) so the
    /// caller's `exists()` checks still produce loud, path-named warnings.
    fn resolve_asset_root() -> std::path::PathBuf {
        // 1. Explicit override.
        if let Some(dir) = std::env::var_os("LIQUIDE_ASSETS_DIR") {
            let candidate = std::path::PathBuf::from(dir);
            if candidate.is_dir() {
                return candidate;
            }
            tracing::warn!(
                ?candidate,
                "LIQUIDE_ASSETS_DIR is set but does not point to a directory; ignoring"
            );
        }

        // 2. CWD-relative (the historical default).
        let cwd_relative = std::path::PathBuf::from("assets");
        if cwd_relative.is_dir() {
            return cwd_relative;
        }

        // 3. Next to the executable.
        if let Ok(exe) = std::env::current_exe() {
            if let Some(exe_dir) = exe.parent() {
                let candidate = exe_dir.join("assets");
                if candidate.is_dir() {
                    return candidate;
                }
            }
        }

        // 4. Workspace-root assets derived from this crate's manifest dir
        //    (crates/liquide-session -> ../../assets).
        let manifest_relative = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("assets");
        if manifest_relative.is_dir() {
            return manifest_relative;
        }

        // Nothing found: return the CWD-relative path so downstream
        // `exists()` checks log a clear, path-named warning.
        cwd_relative
    }

    /// Resolve the packaged theme CSS file for `theme_name`, tolerating the
    /// hyphen/underscore spelling difference between the requested theme name
    /// (`liquid-glass`) and the on-disk filename (`liquid_glass.css`) (t56-H1).
    ///
    /// Returns the first candidate that exists, or `None` if neither spelling
    /// is present so the caller can emit a loud warning naming the path tried.
    fn resolve_theme_file(
        themes_dir: &std::path::Path,
        theme_name: &str,
    ) -> Option<std::path::PathBuf> {
        // Try the requested spelling first, then both normalized spellings so
        // `liquid-glass` resolves `liquid_glass.css` and vice versa.
        let mut spellings = vec![theme_name.to_string()];
        let hyphenated = theme_name.replace('_', "-");
        let underscored = theme_name.replace('-', "_");
        if !spellings.contains(&hyphenated) {
            spellings.push(hyphenated);
        }
        if !spellings.contains(&underscored) {
            spellings.push(underscored);
        }
        for spelling in spellings {
            let candidate = themes_dir.join(format!("{}.css", spelling));
            if candidate.exists() {
                return Some(candidate);
            }
        }
        None
    }

    /// Load a packaged base-layer stylesheet (`themes/{file_name}`) into the
    /// shell's style pipeline via `add_stylesheet`.
    ///
    /// Base layers (design-token variables + shared component defaults) must be
    /// loaded BEFORE the active per-theme file so theme rules can override them
    /// via normal source-order cascade. Resolves through the t56 asset resolver
    /// (`resolve_asset_root`), and emits a loud `warn!` if the file is expected
    /// but missing (consistent with t56's loud-failure pattern), since a missing
    /// base layer leaves shared components — e.g. tooltips/popovers — unstyled.
    fn load_base_layer_css(shell: &mut Shell, themes_dir: &std::path::Path, file_name: &str) {
        let candidate = themes_dir.join(file_name);
        match std::fs::read_to_string(&candidate) {
            Ok(css) => {
                info!("loaded base-layer CSS from {:?}", candidate);
                shell.add_stylesheet(&css);
            }
            Err(err) => {
                tracing::warn!(
                    ?candidate,
                    error = %err,
                    "base-layer CSS not loaded; shared component styling \
                     (tooltips, popovers, etc.) may render unstyled"
                );
            }
        }
    }

    /// Try loading external CSS theme files and user overrides.
    ///
    /// Load order (later stylesheets cascade over earlier ones at equal
    /// specificity):
    /// 1. `<asset-root>/themes/variables.css` — design-token `:root` defaults
    /// 2. `<asset-root>/themes/components.css` — shared component defaults
    ///    (tooltip/popover/etc.), which consume the tokens above
    /// 3. `<asset-root>/themes/{theme_name}.css` — the active packaged theme
    ///    (overrides the base layers)
    /// 4. `~/.config/liquide/custom.css` — user overrides (highest priority)
    ///
    /// Any CSS found is appended to the shell's stylesheet pipeline.
    fn load_external_css(shell: &mut Shell) {
        // Load exactly one packaged theme. Appending every theme makes the UI
        // a cascade mashup and forces extra style work on every scene rebuild.
        let theme_name =
            std::env::var("LIQUIDE_THEME").unwrap_or_else(|_| "liquid-glass".to_string());
        let theme_name = match theme_name.as_str() {
            "liquid-glass" | "night" | "sunset" | "midday" => theme_name,
            invalid => {
                tracing::warn!(
                    theme = invalid,
                    "unknown LIQUIDE_THEME, falling back to liquid-glass"
                );
                "liquid-glass".to_string()
            }
        };
        let themes_dir = Self::resolve_asset_root().join("themes");

        // BASE LAYERS first (before the active theme) so theme rules win on
        // equal-specificity selectors. variables.css defines the `:root` design
        // tokens that components.css references via `var(--…)`, so it must load
        // before components.css. components.css carries the shared component
        // styling (notably `tooltip { position: fixed; z-index: … }` plus
        // popover/dialog/search-bar/etc.) that no per-theme file defines — if it
        // is not loaded those components fall into normal flow at (0,0) (t57-f6b).
        Self::load_base_layer_css(shell, &themes_dir, "variables.css");
        Self::load_base_layer_css(shell, &themes_dir, "components.css");

        match Self::resolve_theme_file(&themes_dir, &theme_name) {
            Some(candidate) => match std::fs::read_to_string(&candidate) {
                Ok(css) => {
                    info!(
                        theme = theme_name,
                        "loaded external CSS theme from {:?}", candidate
                    );
                    shell.load_css_theme(&candidate);
                    shell.add_stylesheet(&css);
                }
                Err(err) => {
                    tracing::warn!(
                        theme = theme_name,
                        ?candidate,
                        error = %err,
                        "failed to read external CSS theme file"
                    );
                }
            },
            None => {
                tracing::warn!(
                    theme = theme_name,
                    themes_dir = ?themes_dir,
                    "external CSS theme not found (tried hyphen/underscore spellings); \
                     using embedded fallback theme"
                );
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
