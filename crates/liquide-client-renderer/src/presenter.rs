//! Display output abstraction for presenting rendered frames.

use liquide_compositor::pixel::PixelFormat;

use crate::surface::RenderSurface;

/// Abstract display output for rendered frames.
///
/// Implementations handle presenting the surface contents to a display,
/// window, or buffer. The trait is `Send` so presenters can be used
/// across thread boundaries.
pub trait Presenter: Send {
    /// Present the given surface to the display.
    fn present(&mut self, surface: &RenderSurface) -> crate::Result<()>;

    /// Check if this presenter supports the given pixel format.
    fn supports_format(&self, format: PixelFormat) -> bool;
}

/// Null presenter that discards all frames (headless / benchmarking).
pub struct NullPresenter;

impl Presenter for NullPresenter {
    fn present(&mut self, _surface: &RenderSurface) -> crate::Result<()> {
        Ok(())
    }

    fn supports_format(&self, _format: PixelFormat) -> bool {
        true
    }
}

impl std::fmt::Display for NullPresenter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NullPresenter")
    }
}

/// Presenter that captures each frame into an in-memory buffer.
///
/// Useful for testing and screenshot capture.
pub struct BufferPresenter {
    buffer: Vec<u8>,
    width: u32,
    height: u32,
    frame_count: u64,
}

impl BufferPresenter {
    /// Create a new buffer presenter.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            width: 0,
            height: 0,
            frame_count: 0,
        }
    }

    /// The most recently captured frame data.
    #[must_use]
    pub fn buffer(&self) -> &[u8] {
        &self.buffer
    }

    /// Width of the last captured frame.
    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height of the last captured frame.
    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Number of frames captured.
    #[must_use]
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }
}

impl Default for BufferPresenter {
    fn default() -> Self {
        Self::new()
    }
}

impl Presenter for BufferPresenter {
    fn present(&mut self, surface: &RenderSurface) -> crate::Result<()> {
        self.buffer = surface.pixels().to_vec();
        self.width = surface.width();
        self.height = surface.height();
        self.frame_count += 1;
        Ok(())
    }

    fn supports_format(&self, _format: PixelFormat) -> bool {
        true
    }
}

impl std::fmt::Display for BufferPresenter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "BufferPresenter({}x{}, frames={})",
            self.width, self.height, self.frame_count
        )
    }
}
