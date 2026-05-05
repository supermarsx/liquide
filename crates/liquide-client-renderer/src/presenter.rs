//! Display output abstraction for presenting rendered frames.

use std::collections::HashMap;

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
/// Useful for testing and screenshot capture. Maintains a small buffer pool
/// keyed by `(width, height, format)` so resolution or format changes don't
/// re-allocate on every present — a common case when switching between
/// monitors at different scales.
pub struct BufferPresenter {
    /// Key for the most recently captured frame in `pool`.
    last_key: Option<(u32, u32, PixelFormat)>,
    width: u32,
    height: u32,
    format: PixelFormat,
    frame_count: u64,
    /// Reusable per-(w,h,format) scratch buffers.
    pool: HashMap<(u32, u32, PixelFormat), Vec<u8>>,
    /// Upper bound on the number of distinct buffers held in the pool.
    pool_capacity: usize,
}

impl BufferPresenter {
    /// Create a new buffer presenter with the default pool capacity (4).
    #[must_use]
    pub fn new() -> Self {
        Self::with_pool_capacity(4)
    }

    /// Create a new buffer presenter with a custom pool capacity.
    #[must_use]
    pub fn with_pool_capacity(pool_capacity: usize) -> Self {
        Self {
            last_key: None,
            width: 0,
            height: 0,
            format: PixelFormat::Bgra8,
            frame_count: 0,
            pool: HashMap::new(),
            pool_capacity: pool_capacity.max(1),
        }
    }

    /// The most recently captured frame data.
    #[must_use]
    pub fn buffer(&self) -> &[u8] {
        match self.last_key {
            Some(key) => self.pool.get(&key).map(|buf| buf.as_slice()).unwrap_or(&[]),
            None => &[],
        }
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

    /// Pixel format of the last captured frame.
    #[must_use]
    pub fn format(&self) -> PixelFormat {
        self.format
    }

    /// Number of frames captured.
    #[must_use]
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Current number of pooled buffer slots.
    #[must_use]
    pub fn pool_len(&self) -> usize {
        self.pool.len()
    }
}

impl Default for BufferPresenter {
    fn default() -> Self {
        Self::new()
    }
}

impl Presenter for BufferPresenter {
    fn present(&mut self, surface: &RenderSurface) -> crate::Result<()> {
        let key = (surface.width(), surface.height(), surface.format());
        let src = surface.pixels();

        // Evict a least-recently-inserted slot if we're at capacity and need
        // a new one. HashMap iteration order is unspecified but fine for a
        // simple LRU approximation at this pool size.
        if !self.pool.contains_key(&key) && self.pool.len() >= self.pool_capacity {
            if let Some(victim) = self.pool.keys().next().copied() {
                self.pool.remove(&victim);
            }
        }

        let slot = self
            .pool
            .entry(key)
            .or_insert_with(|| Vec::with_capacity(src.len()));
        slot.clear();
        slot.extend_from_slice(src);

        self.last_key = Some(key);
        self.width = surface.width();
        self.height = surface.height();
        self.format = surface.format();
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
            "BufferPresenter({}x{}, frames={}, pool={})",
            self.width,
            self.height,
            self.frame_count,
            self.pool.len()
        )
    }
}
