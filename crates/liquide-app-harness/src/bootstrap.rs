//! `AppBootstrap` — the builder and driver of an app's event loop.

use anyhow::Result;
#[cfg(test)]
use liquide_platform::NullPlatform;
use liquide_platform::PlatformBackend;
use liquide_ui_core::{widget::Widget, Event, UiTheme};

use crate::event_loop::{AppRunReport, EventLoop, FrameStats};

/// Logical window size in device pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

impl Size {
    #[must_use]
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}
impl Default for Size {
    fn default() -> Self {
        Self::new(1024, 768)
    }
}
/// Context passed to the root-widget builder.
///
/// Intentionally narrow: exposes only the information the root builder
/// legitimately needs at construction time. If a future use case demands
/// more state (font manager handle, IPC client, etc.) add it here rather
/// than in `AppBootstrap` itself.
pub struct AppCx {
    pub(crate) app_id: String,
    pub(crate) display_name: String,
    pub(crate) theme: UiTheme,
    pub(crate) size: Size,
    pub(crate) ime_enabled: bool,
}

impl AppCx {
    #[must_use]
    pub fn app_id(&self) -> &str {
        &self.app_id
    }

    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    #[must_use]
    pub fn theme(&self) -> &UiTheme {
        &self.theme
    }

    #[must_use]
    pub fn size(&self) -> Size {
        self.size
    }

    #[must_use]
    pub fn ime_enabled(&self) -> bool {
        self.ime_enabled
    }

    /// Request an additional top-level window.
    ///
    /// Multi-window support is deferred — the harness currently manages
    /// a single primary window. This stub exists so the API surface is
    /// forward-compatible; consumers should not rely on it.
    pub fn spawn_window(&mut self, _title: &str) -> Result<()> {
        anyhow::bail!(
            "liquide-app-harness: spawn_window is not yet implemented (single-window only)"
        )
    }
}

/// Builder + driver that wires a LiquiDE app's pipeline end-to-end.
pub struct AppBootstrap {
    app_id: String,
    display_name: String,
    initial_size: Size,
    ime_enabled: bool,
    theme: UiTheme,
    platform: Option<Box<dyn PlatformBackend>>,
}

impl AppBootstrap {
    /// Create a new bootstrap for an application.
    ///
    /// `app_id` should be a reverse-DNS identifier (e.g.
    /// `"com.liquide.apps.files"`). `display_name` is shown in the title
    /// bar / taskbar.
    #[must_use]
    pub fn new(app_id: &str, display_name: &str) -> Self {
        Self {
            app_id: app_id.to_string(),
            display_name: display_name.to_string(),
            initial_size: Size::default(),
            ime_enabled: true,
            theme: UiTheme::default(),
            platform: None,
        }
    }

    #[must_use]
    pub fn with_initial_size(mut self, size: Size) -> Self {
        self.initial_size = size;
        self
    }

    #[must_use]
    pub fn with_ime(mut self, enabled: bool) -> Self {
        self.ime_enabled = enabled;
        self
    }

    #[must_use]
    pub fn with_theme(mut self, theme: UiTheme) -> Self {
        self.theme = theme;
        self
    }

    /// Inject a custom [`PlatformBackend`].
    ///
    /// Intended primarily for tests that want to drive the harness
    /// against a scripted or headless backend without creating real OS
    /// windows.
    /// Production code should omit this and let `run` pick the native
    /// backend via [`liquide_platform::create_platform`].
    #[must_use]
    pub fn with_platform(mut self, platform: Box<dyn PlatformBackend>) -> Self {
        self.platform = Some(platform);
        self
    }

    /// Run the event loop until the window is closed or `Quit` is posted.
    pub fn run<F>(self, root_builder: F) -> Result<()>
    where
        F: FnOnce(&mut AppCx) -> Box<dyn Widget>,
    {
        let _ = self.run_with_report(root_builder)?;
        Ok(())
    }

    /// Run until quit and return frame + present introspection.
    pub fn run_with_report<F>(self, root_builder: F) -> Result<AppRunReport>
    where
        F: FnOnce(&mut AppCx) -> Box<dyn Widget>,
    {
        let (mut cx, mut ev_loop, mut root) = self.prepare(root_builder)?;
        let report = ev_loop.run_until_quit_with_report(&mut cx, root.as_mut())?;
        drop(root);
        Ok(report)
    }

    /// Run for exactly `frames` synthetic ticks and then return.
    ///
    /// Used by the smoke test and by downstream apps that want a
    /// deterministic "headless" harness pass. Each tick runs the full
    /// measure → layout → paint → present chain.
    pub fn run_for_frames<F>(self, frames: u32, root_builder: F) -> Result<FrameStats>
    where
        F: FnOnce(&mut AppCx) -> Box<dyn Widget>,
    {
        Ok(self.run_for_frames_with_report(frames, root_builder)?.stats)
    }

    /// Run for exactly `frames` synthetic ticks and return frame +
    /// present introspection.
    pub fn run_for_frames_with_report<F>(self, frames: u32, root_builder: F) -> Result<AppRunReport>
    where
        F: FnOnce(&mut AppCx) -> Box<dyn Widget>,
    {
        let (mut cx, mut ev_loop, mut root) = self.prepare(root_builder)?;
        let report = ev_loop.run_for_frames_with_report(&mut cx, root.as_mut(), frames)?;
        drop(root);
        Ok(report)
    }

    fn prepare<F>(self, root_builder: F) -> Result<(AppCx, EventLoop, Box<dyn Widget>)>
    where
        F: FnOnce(&mut AppCx) -> Box<dyn Widget>,
    {
        let platform: Box<dyn PlatformBackend> = match self.platform {
            Some(p) => p,
            None => {
                // Production path: pick the best real backend.
                // In test builds we default to the null backend so that
                // `cargo test -p liquide-app-harness` never tries to
                // create a real OS window.
                #[cfg(not(test))]
                {
                    liquide_platform::create_platform()
                        .map_err(|e| anyhow::anyhow!("platform init failed: {e}"))?
                }
                #[cfg(test)]
                {
                    Box::new(NullPlatform::new())
                }
            }
        };

        let mut cx = AppCx {
            app_id: self.app_id,
            display_name: self.display_name,
            theme: self.theme,
            size: self.initial_size,
            ime_enabled: self.ime_enabled,
        };

        let mut ev_loop = EventLoop::new(platform);
        ev_loop.create_window(&cx)?;
        let mut root = root_builder(&mut cx);

        // Prime the widget tree with a Resize event so the first tick has
        // a valid layout state.
        let _ = root.handle_event(&Event::Resize {
            width: cx.size.width as f32,
            height: cx.size.height as f32,
        });

        Ok((cx, ev_loop, root))
    }
}
