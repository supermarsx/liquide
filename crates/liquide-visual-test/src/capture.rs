//! Headless deterministic frame capture.
//!
//! Drives a real [`DesktopCompositor`] over the in-tree [`StandalonePlatform`]
//! backend and reads back the actual CPU-rasterised pixels. Two entry points:
//!
//! - [`capture_desktop`] renders ONE deterministic frame via
//!   [`DesktopCompositor::capture_once`] (single-threaded, time `t0` — no
//!   animation advance, no glyph-upload race). Use for static-scene goldens.
//! - [`capture_desktop_scripted`] runs the threaded `run()` loop with a scripted
//!   input sequence + trailing `Quit`, then reads `last_presented_frame()`. Use
//!   for event-driven scenarios (e.g. a context menu opened on right-click).
//!
//! Both normalise the captured buffer to **RGBA8**, top-down, tightly packed.

use std::sync::{Mutex, OnceLock};

use liquide_compositor::pixel::PixelFormat;
use liquide_platform::standalone::{StandaloneConfig, StandalonePlatform};
use liquide_platform::{NativeWindowHandle, PlatformEvent};
use liquide_session::desktop::{CapturedFrame, DesktopCompositor};

/// Errors surfaced by the capture/diff/golden harness.
#[derive(Debug)]
pub enum VisualTestError {
    /// The standalone platform backend could not be created.
    Platform(String),
    /// The compositor produced no presentable frame (e.g. empty CPU buffer).
    NoFrame,
    /// A golden file could not be read/decoded.
    GoldenIo {
        path: String,
        source: std::io::Error,
    },
    /// An image could not be encoded/decoded.
    Image(String),
    /// A golden assertion failed (pixels differed beyond tolerance).
    Mismatch(String),
}

impl std::fmt::Display for VisualTestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Platform(m) => write!(f, "platform backend error: {m}"),
            Self::NoFrame => write!(f, "compositor produced no presentable frame"),
            Self::GoldenIo { path, source } => write!(f, "golden i/o error for {path}: {source}"),
            Self::Image(m) => write!(f, "image codec error: {m}"),
            Self::Mismatch(m) => write!(f, "golden mismatch: {m}"),
        }
    }
}

impl std::error::Error for VisualTestError {}

/// Options controlling a headless desktop capture.
///
/// Theme and assets directory are applied via process-global environment
/// variables (`LIQUIDE_THEME`, `LIQUIDE_ASSETS_DIR`) that the compositor reads
/// during construction, so all captures are serialised behind an internal lock
/// to avoid env races across parallel tests.
#[derive(Debug, Clone)]
pub struct CaptureOptions {
    /// Surface width in pixels (kept verbatim — dev-mode disables monitor probe).
    pub width: u32,
    /// Surface height in pixels.
    pub height: u32,
    /// Theme name written to `LIQUIDE_THEME` (e.g. `"liquid-glass"`, `"night"`).
    /// `None` leaves the env untouched (compositor default applies).
    pub theme: Option<String>,
    /// Absolute assets directory written to `LIQUIDE_ASSETS_DIR` so themes/fonts
    /// resolve regardless of the test CWD. `None` falls back to the compositor's
    /// own CWD/manifest/exe resolution. Defaults to the workspace `assets/` dir.
    pub assets_dir: Option<String>,
}

impl Default for CaptureOptions {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            theme: Some("liquid-glass".to_string()),
            assets_dir: default_assets_dir(),
        }
    }
}

impl CaptureOptions {
    /// Builder: set the surface size.
    #[must_use]
    pub fn size(mut self, width: u32, height: u32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    /// Builder: set the theme name.
    #[must_use]
    pub fn theme(mut self, theme: impl Into<String>) -> Self {
        self.theme = Some(theme.into());
        self
    }

    /// Builder: set an explicit absolute assets directory.
    #[must_use]
    pub fn assets_dir(mut self, dir: impl Into<String>) -> Self {
        self.assets_dir = Some(dir.into());
        self
    }
}

/// Best-effort default assets directory derived from this crate's manifest dir
/// (`<repo>/crates/liquide-visual-test/../../assets`), so captures find themes
/// and fonts regardless of the working directory CI/cargo picks.
fn default_assets_dir() -> Option<String> {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let assets = manifest.parent()?.parent()?.join("assets");
    assets
        .is_dir()
        .then(|| assets.to_string_lossy().into_owned())
}

/// A captured frame normalised to **RGBA8**, top-down, tightly packed.
///
/// `rgba.len() == (width * height * 4)` and `stride == width * 4`. This is the
/// canonical format consumed by the differ, golden compare, and PNG encoder.
#[derive(Debug, Clone)]
pub struct Frame {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// RGBA8 pixel bytes, `width * height * 4` long, row-major, top-down.
    pub rgba: Vec<u8>,
}

impl Frame {
    /// Tightly-packed row stride (`width * 4`).
    #[must_use]
    pub fn stride(&self) -> u32 {
        self.width * 4
    }

    /// Construct a [`Frame`] from a raw [`CapturedFrame`], converting the
    /// compositor's `Bgra8` buffer (and honouring any padded stride) to packed
    /// RGBA8.
    #[must_use]
    pub fn from_captured(cap: &CapturedFrame) -> Self {
        let w = cap.width as usize;
        let h = cap.height as usize;
        let src_stride = cap.stride as usize;
        let mut rgba = vec![0u8; w * h * 4];
        for y in 0..h {
            let src_row = &cap.pixels[y * src_stride..];
            let dst_row = &mut rgba[y * w * 4..];
            for x in 0..w {
                let s = &src_row[x * 4..x * 4 + 4];
                let d = &mut dst_row[x * 4..x * 4 + 4];
                match cap.format {
                    PixelFormat::Bgra8 => {
                        d[0] = s[2]; // R <- B
                        d[1] = s[1]; // G
                        d[2] = s[0]; // B <- R
                        d[3] = s[3]; // A
                    }
                    // Rgba8 (and any other already-RGB-ordered format): copy.
                    _ => d.copy_from_slice(s),
                }
            }
        }
        Self {
            width: cap.width,
            height: cap.height,
            rgba,
        }
    }

    /// Build a [`Frame`] directly from a standalone presented frame (used by the
    /// scripted run() path, which reads `last_presented_frame()`).
    #[must_use]
    fn from_presented(
        width: u32,
        height: u32,
        stride: u32,
        format: PixelFormat,
        pixels: Vec<u8>,
    ) -> Self {
        Self::from_captured(&CapturedFrame {
            width,
            height,
            stride,
            format,
            pixels,
        })
    }

    /// Read-only RGBA pixel at `(x, y)`, or `None` if out of bounds.
    #[must_use]
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let off = (y * self.width + x) as usize * 4;
        Some([
            self.rgba[off],
            self.rgba[off + 1],
            self.rgba[off + 2],
            self.rgba[off + 3],
        ])
    }

    /// Crop a sub-rectangle into a new [`Frame`]. The rect is clamped to bounds;
    /// an empty/out-of-bounds rect yields a zero-sized frame.
    #[must_use]
    pub fn crop(&self, x: u32, y: u32, w: u32, h: u32) -> Frame {
        let x = x.min(self.width);
        let y = y.min(self.height);
        let w = w.min(self.width - x);
        let h = h.min(self.height - y);
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        for row in 0..h {
            let src_off = ((y + row) * self.width + x) as usize * 4;
            let dst_off = (row * w) as usize * 4;
            let len = (w * 4) as usize;
            rgba[dst_off..dst_off + len].copy_from_slice(&self.rgba[src_off..src_off + len]);
        }
        Frame {
            width: w,
            height: h,
            rgba,
        }
    }

    /// True if every pixel is identical (a dead/blank pipeline tell-tale).
    #[must_use]
    pub fn is_uniform(&self) -> bool {
        if self.rgba.len() < 4 {
            return true;
        }
        let first = &self.rgba[0..4];
        self.rgba.chunks_exact(4).all(|px| px == first)
    }

    /// Count pixels whose max-channel distance from `background` exceeds
    /// `tolerance`. Used by the text-content heuristic (non-bg pixels in a text
    /// bounding box must exceed a threshold, catching blank/notdef renders).
    #[must_use]
    pub fn non_background_pixels(&self, background: [u8; 4], tolerance: u8) -> usize {
        self.rgba
            .chunks_exact(4)
            .filter(|px| {
                px.iter()
                    .zip(background.iter())
                    .any(|(&a, &b)| a.abs_diff(b) > tolerance)
            })
            .count()
    }

    /// Encode this frame to a PNG file at `path` (creates parent dirs).
    pub fn save_png(&self, path: impl AsRef<std::path::Path>) -> Result<(), VisualTestError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| VisualTestError::Image(format!("create_dir_all {parent:?}: {e}")))?;
        }
        image::save_buffer(
            path,
            &self.rgba,
            self.width,
            self.height,
            image::ColorType::Rgba8,
        )
        .map_err(|e| VisualTestError::Image(format!("save_buffer {path:?}: {e}")))
    }

    /// Load a frame from a PNG file (decoded to RGBA8).
    pub fn load_png(path: impl AsRef<std::path::Path>) -> Result<Frame, VisualTestError> {
        let path = path.as_ref();
        let img = image::open(path).map_err(|e| VisualTestError::GoldenIo {
            path: path.display().to_string(),
            source: std::io::Error::other(e.to_string()),
        })?;
        let rgba = img.to_rgba8();
        Ok(Frame {
            width: rgba.width(),
            height: rgba.height(),
            rgba: rgba.into_raw(),
        })
    }
}

/// Process-global capture lock.
///
/// Capture configures the compositor via process env vars (`LIQUIDE_THEME`,
/// `LIQUIDE_ASSETS_DIR`) which the compositor reads at construction time. To
/// keep parallel `cargo test` threads from racing on those env vars, every
/// capture holds this lock for the whole construct-and-render window.
fn capture_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Apply theme/assets env, build a dev-mode [`DesktopCompositor`] at the
/// requested size, and a matching headless [`StandalonePlatform`].
fn build(
    opts: &CaptureOptions,
) -> Result<(DesktopCompositor, StandalonePlatform), VisualTestError> {
    if let Some(theme) = &opts.theme {
        // SAFETY: single-threaded section under capture_lock(); see capture_lock.
        unsafe { std::env::set_var("LIQUIDE_THEME", theme) };
    }
    if let Some(dir) = &opts.assets_dir {
        // SAFETY: single-threaded section under capture_lock(); see capture_lock.
        unsafe { std::env::set_var("LIQUIDE_ASSETS_DIR", dir) };
    }

    let platform = StandalonePlatform::new(StandaloneConfig {
        width: opts.width,
        height: opts.height,
        hardware_cursor: false,
        ..StandaloneConfig::default()
    })
    .map_err(|e| VisualTestError::Platform(e.to_string()))?;

    let mut desktop = DesktopCompositor::new(opts.width, opts.height);
    // Dev mode keeps the requested resolution (run/capture skip the monitor
    // probe) and uses the windowed prologue path.
    desktop.set_dev_mode(true);

    Ok((desktop, platform))
}

/// Capture a single deterministic desktop frame.
///
/// Renders exactly one frame through [`DesktopCompositor::capture_once`] — the
/// synchronous prologue (window create -> loading overlay -> first desktop
/// frame), with a glyph reflush pass for stable text — without spawning the
/// render thread. Returns the frame normalised to packed RGBA8.
pub fn capture_desktop(opts: &CaptureOptions) -> Result<Frame, VisualTestError> {
    let _guard = capture_lock().lock().unwrap_or_else(|p| p.into_inner());
    let (mut desktop, mut platform) = build(opts)?;
    let captured = desktop
        .capture_once(&mut platform)
        .ok_or(VisualTestError::NoFrame)?;
    Ok(Frame::from_captured(&captured))
}

/// Capture after driving the threaded `run()` loop with a scripted input
/// sequence.
///
/// The first window the compositor creates is assigned the deterministic
/// [`NativeWindowHandle`]`(1)`, which is passed to `script` so callers can build
/// pointer/keyboard events targeting it. The returned events are queued before
/// `run()` and a trailing [`PlatformEvent::Quit`] is appended automatically so
/// the loop exits after processing them; the last presented frame is then read
/// back via `last_presented_frame()`.
///
/// Use this for event-driven scenarios (e.g. a context menu that only appears
/// after a right-click) where the single-frame [`capture_desktop`] cannot
/// observe the post-event state.
pub fn capture_desktop_scripted<F>(
    opts: &CaptureOptions,
    script: F,
) -> Result<Frame, VisualTestError>
where
    F: FnOnce(NativeWindowHandle) -> Vec<PlatformEvent>,
{
    let _guard = capture_lock().lock().unwrap_or_else(|p| p.into_inner());
    let (mut desktop, mut platform) = build(opts)?;

    // The desktop's first created window is handle(1) (StandaloneWindowHost
    // allocates monotonically from 1).
    let mut events = script(NativeWindowHandle(1));
    events.push(PlatformEvent::Quit);
    platform.push_events(events);

    desktop.run(&mut platform);

    let presented = platform
        .last_presented_frame()
        .ok_or(VisualTestError::NoFrame)?;
    Ok(Frame::from_presented(
        presented.width,
        presented.height,
        presented.stride,
        presented.format,
        presented.pixels,
    ))
}

/// Deterministic scripted capture: inject an input sequence, then read back the
/// frame rendered *after* those events were processed — using the synchronous
/// [`DesktopCompositor::capture_once`] path (the same single-threaded prologue
/// the static [`capture_desktop`] uses), NOT the threaded `run()` loop.
///
/// # Why this exists (t56-f4)
///
/// [`capture_desktop_scripted`] drives `run()` with a trailing `Quit`. But the
/// event-loop drains *all* queued events in one batch — so when `Quit` rides in
/// the same batch as the scripted click, `running` flips to `false` and the loop
/// exits *before* the now-dirty post-click frame is ever submitted/presented.
/// `last_presented_frame()` therefore still holds the pre-click desktop, and an
/// opened context-menu overlay shows up as **zero** changed pixels — which had
/// been mis-attributed to a `scene_bridge` paint/z-order bug. The scene bridge
/// is correct (the menu paints at the right place/z); the capture path was the
/// fault.
///
/// `capture_once_scripted` instead dispatches the scripted events to the shell
/// AFTER the loading prologue (the shell only routes events when `!loading`, so
/// events drained during the prologue are intentionally swallowed) and BEFORE
/// the captured desktop frame is rendered. The scripted right-click is therefore
/// processed and the desktop frame is rendered *with the menu visible*, then
/// read straight off the CPU framebuffer. Deterministic, single-threaded, and
/// consistent with the other three scenarios. The window handle the script
/// targets is [`NativeWindowHandle`]`(1)` (the first window the host allocates).
pub fn capture_desktop_scripted_sync<F>(
    opts: &CaptureOptions,
    script: F,
) -> Result<Frame, VisualTestError>
where
    F: FnOnce(NativeWindowHandle) -> Vec<PlatformEvent>,
{
    let _guard = capture_lock().lock().unwrap_or_else(|p| p.into_inner());
    let (mut desktop, mut platform) = build(opts)?;

    let events = script(NativeWindowHandle(1));
    let captured = desktop
        .capture_once_scripted(&mut platform, events)
        .ok_or(VisualTestError::NoFrame)?;
    Ok(Frame::from_captured(&captured))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_returns_nonempty_correctly_sized_frame() {
        // Smoke test (also begins to close H5: a REAL non-zero frame, unlike the
        // old zero-filled app-harness path).
        let opts = CaptureOptions::default().size(320, 240);
        let frame = capture_desktop(&opts).expect("capture should succeed");
        assert_eq!(frame.width, 320, "frame width");
        assert_eq!(frame.height, 240, "frame height");
        assert_eq!(
            frame.rgba.len(),
            320 * 240 * 4,
            "rgba buffer must be tightly packed"
        );
        // A real desktop render must not be a single flat color.
        assert!(
            !frame.is_uniform(),
            "captured desktop frame is uniform — pipeline likely produced an empty/dead frame"
        );
    }

    #[test]
    fn crop_extracts_subregion() {
        let frame = Frame {
            width: 4,
            height: 4,
            rgba: (0..4 * 4 * 4).map(|i| i as u8).collect(),
        };
        let c = frame.crop(1, 1, 2, 2);
        assert_eq!(c.width, 2);
        assert_eq!(c.height, 2);
        assert_eq!(c.rgba.len(), 2 * 2 * 4);
        // Top-left of the crop equals pixel (1,1) of the source.
        assert_eq!(c.pixel(0, 0), frame.pixel(1, 1));
    }

    #[test]
    fn bgra_to_rgba_swaps_channels() {
        let cap = CapturedFrame {
            width: 1,
            height: 1,
            stride: 4,
            format: PixelFormat::Bgra8,
            pixels: vec![10, 20, 30, 40], // B,G,R,A
        };
        let f = Frame::from_captured(&cap);
        assert_eq!(f.pixel(0, 0), Some([30, 20, 10, 40])); // R,G,B,A
    }
}
