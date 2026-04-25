//! Platform swap-chain presenter skeletons.
//!
//! This module exposes a minimal API surface for window-system-integrated
//! presenters (Win32 D3D11, Wayland, X11, DRM, Metal). Real implementations
//! are feature-gated behind `real-presenters` and deferred to a later
//! milestone. The default build provides a stub type whose `present()`
//! returns `Err(ClientRendererError::PresenterError(..))` so consumers
//! exercising the API fail honestly.

use liquide_compositor::pixel::PixelFormat;

use crate::presenter::Presenter;
use crate::surface::RenderSurface;

/// Platform-specific swap-chain presenter. On Windows this will eventually
/// wrap a DXGI flip-model swap chain driven by D3D11. Today it is a
/// skeleton whose `present()` returns an honest `Err`.
pub struct SwapChainPresenter {
    width: u32,
    height: u32,
    format: PixelFormat,
    #[cfg(all(target_os = "windows", feature = "real-presenters"))]
    inner: Option<win32_d3d11::SwapChain>,
}

impl SwapChainPresenter {
    /// Create a new skeleton presenter for the given output dimensions.
    #[must_use]
    pub fn new(width: u32, height: u32, format: PixelFormat) -> Self {
        Self {
            width,
            height,
            format,
            #[cfg(all(target_os = "windows", feature = "real-presenters"))]
            inner: None,
        }
    }

    /// Output width in pixels.
    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Output height in pixels.
    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Output pixel format.
    #[must_use]
    pub fn format(&self) -> PixelFormat {
        self.format
    }
}

impl Presenter for SwapChainPresenter {
    fn present(&mut self, _surface: &RenderSurface) -> crate::Result<()> {
        #[cfg(all(target_os = "windows", feature = "real-presenters"))]
        {
            if let Some(_sc) = &mut self.inner {
                // Real D3D11 flip-model present path lives behind the
                // `real-presenters` feature. Implementation deferred.
                return Err(crate::ClientRendererError::PresenterError(
                    "D3D11 flip-model present not yet implemented".to_string(),
                ));
            }
        }
        Err(crate::ClientRendererError::PresenterError(
            "SwapChainPresenter is a skeleton; enable the `real-presenters` feature".to_string(),
        ))
    }

    fn supports_format(&self, format: PixelFormat) -> bool {
        format == self.format
    }
}

#[cfg(all(target_os = "windows", feature = "real-presenters"))]
mod win32_d3d11 {
    //! Windows D3D11 flip-model swap chain skeleton.
    //!
    //! Only the type surface is present; real integration pulls in
    //! `windows` crate (DXGI, D3D11) and is deferred.

    pub struct SwapChain {
        pub(super) _width: u32,
        pub(super) _height: u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skeleton_present_errors_honestly() {
        let mut p = SwapChainPresenter::new(1920, 1080, PixelFormat::Bgra8);
        let s = RenderSurface::new(1920, 1080, PixelFormat::Bgra8);
        assert!(p.present(&s).is_err());
    }
}
