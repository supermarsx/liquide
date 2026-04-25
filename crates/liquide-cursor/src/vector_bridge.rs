//! Pluggable vector-cursor backend bridge.
//!
//! `liquide-cursor` cannot depend on `liquide-cursor-vector` (the vector
//! crate already depends on this one for `CursorShape`). To avoid the
//! dependency cycle while still letting the software renderer draw themed
//! vector cursors, the vector crate (or any other consumer) registers a
//! backend object here at startup, and the renderer looks it up lazily.
//!
//! If no backend is registered, the renderer falls back to a solid-colour
//! debug square so an unconfigured system still shows *something*.

use crate::shape::CursorShape;
use std::sync::{Arc, OnceLock, RwLock};

/// Output of a vector-cursor rasterisation.
#[derive(Debug, Clone)]
pub struct VectorCursorBitmap {
    /// RGBA8 (premultiplied) pixel data, row-major, no stride padding.
    pub pixels: Vec<u8>,
    /// Width in device pixels.
    pub width: u32,
    /// Height in device pixels.
    pub height: u32,
    /// Hotspot X in device pixels.
    pub hotspot_x: u32,
    /// Hotspot Y in device pixels.
    pub hotspot_y: u32,
}

/// Backend capable of rasterising a standard cursor shape.
///
/// Implementations are typically provided by `liquide-cursor-vector`.
pub trait VectorCursorBackend: Send + Sync {
    /// Render `shape` at nominal `size` logical pixels, applying `scale` to
    /// produce `size * scale`-sized device pixels. Returns `None` if the
    /// shape is not available in the backend (caller falls back).
    fn render(&self, shape: CursorShape, size: u32, scale: f32) -> Option<VectorCursorBitmap>;
}

static BACKEND: OnceLock<RwLock<Option<Arc<dyn VectorCursorBackend>>>> = OnceLock::new();

fn cell() -> &'static RwLock<Option<Arc<dyn VectorCursorBackend>>> {
    BACKEND.get_or_init(|| RwLock::new(None))
}

/// Install (or replace) the global vector-cursor backend.
///
/// Safe to call from any thread. Intended to be invoked once at startup by
/// whichever crate owns vector cursor assets (e.g. `liquide-cursor-vector`).
pub fn set_backend(backend: Arc<dyn VectorCursorBackend>) {
    if let Ok(mut slot) = cell().write() {
        *slot = Some(backend);
    }
}

/// Remove the currently installed backend (primarily for tests).
pub fn clear_backend() {
    if let Ok(mut slot) = cell().write() {
        *slot = None;
    }
}

/// Rasterise `shape` at `size` logical pixels × `scale` device-pixel factor
/// using the currently installed backend, if any.
pub fn render(shape: CursorShape, size: u32, scale: f32) -> Option<VectorCursorBitmap> {
    let backend = cell().read().ok()?.clone()?;
    backend.render(shape, size, scale)
}

/// Whether a backend is currently registered.
pub fn is_installed() -> bool {
    cell().read().map(|g| g.is_some()).unwrap_or(false)
}
