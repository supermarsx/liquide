//! Shared DRM ioctl helpers.
//!
//! Internal `pub(crate)` module that hosts the generic Linux ioctl encoding
//! machinery used by sibling DRM modules (`crtc`, `encoder`, ...). Driver-
//! specific request constants and request structs stay in their owning
//! modules; only the encoding/dispatch primitives live here.

// On non-Linux test builds, the production callers (`crtc`, `encoder`, ...)
// are themselves Linux-gated, so several items here look unused to the
// compiler even though they are part of the crate's API surface on Linux.
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use crate::error::{DrmError, Result};
use core::ffi::c_ulong;

const IOC_NRBITS: u32 = 8;
const IOC_TYPEBITS: u32 = 8;
const IOC_SIZEBITS: u32 = 14;
const IOC_NRSHIFT: u32 = 0;
const IOC_TYPESHIFT: u32 = IOC_NRSHIFT + IOC_NRBITS;
const IOC_SIZESHIFT: u32 = IOC_TYPESHIFT + IOC_TYPEBITS;
const IOC_DIRSHIFT: u32 = IOC_SIZESHIFT + IOC_SIZEBITS;
const IOC_WRITE: u32 = 1;
const IOC_READ: u32 = 2;

pub(crate) const DRM_IOCTL_BASE: u32 = b'd' as u32;

/// Encode a `_IOWR`-style DRM ioctl request number.
pub(crate) const fn drm_iowr(nr: u32, size: usize) -> c_ulong {
    (((IOC_READ | IOC_WRITE) as u64) << IOC_DIRSHIFT
        | (DRM_IOCTL_BASE as u64) << IOC_TYPESHIFT
        | (nr as u64) << IOC_NRSHIFT
        | ((size as u64) << IOC_SIZESHIFT)) as c_ulong
}

/// Invoke a DRM ioctl, mapping kernel errors to [`DrmError::Ioctl`].
///
/// Under `#[cfg(test)]`, calls are first routed through [`mock::dispatch`]; if a
/// scripted handler is installed (see [`mock::install_scoped`]) it intercepts
/// the call and produces a synthetic kernel response. With no handler installed,
/// behavior is identical to a non-test build.
pub(crate) fn drm_ioctl<T>(
    fd: i32,
    request: c_ulong,
    name: &str,
    arg: &mut T,
) -> Result<()> {
    #[cfg(test)]
    {
        let arg_ptr = arg as *mut T as *mut u8;
        if let Some(result) = mock::dispatch(fd, request, name, arg_ptr) {
            return result;
        }
    }
    #[cfg(target_os = "linux")]
    {
        // SAFETY: `arg` points to initialized storage for the duration of the ioctl call.
        let result = unsafe { libc::ioctl(fd, request, arg as *mut T) };
        if result < 0 {
            return Err(DrmError::Ioctl {
                name: name.to_string(),
                reason: std::io::Error::last_os_error().to_string(),
            });
        }
        Ok(())
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (fd, request, arg);
        Err(DrmError::Ioctl {
            name: name.to_string(),
            reason: "no mock ioctl handler installed".to_string(),
        })
    }
}

/// Test-only mock dispatch layer for [`drm_ioctl`].
///
/// Tests can install a thread-local handler via [`install_scoped`] (RAII) or
/// [`install`] / [`clear`]. While installed, every `drm_ioctl` call is routed
/// to the handler instead of `libc::ioctl`, allowing host-side regressions on
/// any platform.
///
/// The handler receives an [`IoctlCall`] containing the raw `fd`, `request`,
/// `name`, and a `*mut u8` pointer to the caller's `&mut T` arg. The handler is
/// responsible for casting the pointer back to the appropriate type and
/// writing any synthetic kernel response into it.
#[cfg(test)]
pub(crate) mod mock {
    use crate::error::Result;
    use core::ffi::c_ulong;
    use std::cell::RefCell;

    /// Captured ioctl invocation passed to a mock handler.
    pub struct IoctlCall {
        pub fd: i32,
        pub request: c_ulong,
        pub name: String,
        /// Raw pointer to the caller's `&mut T`. Cast back to `*mut T` (or to
        /// the appropriate `#[repr(C)]` request struct) to read the
        /// caller-provided fields and write synthetic kernel responses.
        pub arg: *mut u8,
    }

    /// Snapshot of an intercepted call, suitable for assertion (omits raw ptr).
    #[allow(dead_code)]
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct IoctlCallRecord {
        pub fd: i32,
        pub request: c_ulong,
        pub name: String,
    }

    type Handler = Box<dyn FnMut(IoctlCall) -> Result<()> + Send>;

    thread_local! {
        static HANDLER: RefCell<Option<Handler>> = const { RefCell::new(None) };
    }

    /// Install a thread-local mock handler. Replaces any previous handler.
    pub fn install<F>(handler: F)
    where
        F: FnMut(IoctlCall) -> Result<()> + Send + 'static,
    {
        HANDLER.with(|h| *h.borrow_mut() = Some(Box::new(handler)));
    }

    /// Drop any installed mock handler.
    pub fn clear() {
        HANDLER.with(|h| *h.borrow_mut() = None);
    }

    pub(super) fn dispatch(
        fd: i32,
        request: c_ulong,
        name: &str,
        arg: *mut u8,
    ) -> Option<Result<()>> {
        HANDLER.with(|h| {
            let mut borrow = h.borrow_mut();
            borrow.as_mut().map(|handler| {
                handler(IoctlCall {
                    fd,
                    request,
                    name: name.to_string(),
                    arg,
                })
            })
        })
    }

    /// RAII guard that clears the installed handler on drop.
    pub struct ScopedHandler {
        _private: (),
    }

    impl Drop for ScopedHandler {
        fn drop(&mut self) {
            clear();
        }
    }

    /// Install a handler scoped to the returned guard's lifetime.
    pub fn install_scoped<F>(handler: F) -> ScopedHandler
    where
        F: FnMut(IoctlCall) -> Result<()> + Send + 'static,
    {
        install(handler);
        ScopedHandler { _private: () }
    }
}

/// Convert a mutable slice into a u64 pointer suitable for DRM ioctl pointer fields.
///
/// Returns 0 for empty slices (matching kernel expectation that pointer + count = 0
/// signals "no buffer provided").
pub(crate) fn slice_ptr_u64<T>(slice: &mut [T]) -> u64 {
    if slice.is_empty() {
        0
    } else {
        slice.as_mut_ptr() as usize as u64
    }
}
