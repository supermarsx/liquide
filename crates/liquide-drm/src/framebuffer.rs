//! Typed DRM framebuffer shape.
//!
//! Provides:
//! - [`FramebufferId`] newtype mirroring [`crate::CrtcId`] / [`crate::EncoderId`].
//! - `#[repr(C)]` argument structs for `DRM_IOCTL_MODE_CREATE_DUMB`,
//!   `DRM_IOCTL_MODE_DESTROY_DUMB`, and `DRM_IOCTL_MODE_ADDFB2`.
//! - Pure helpers translating those argument structs from typed inputs.
//! - A [`DumbBuffer`] RAII handle whose Linux `Drop` calls a private
//!   `destroy_dumb_via_ioctl` stub.
//! - A [`DrmFramebuffer`] whose Linux `Drop` calls `rmfb_via_ioctl` first and
//!   then drops the inner [`DumbBuffer`] (FB released before dumb destroy).
//!
//! The Linux-side allocation paths still return `Err(...)` — this slice only
//! lands the typed shape and RAII scaffolding. Real DRM ioctl invocation is a
//! follow-up task.

use crate::device::DrmDevice;
use crate::error::{DrmError, Result};

/// Unique identifier for a DRM framebuffer object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FramebufferId(pub u32);

/// DRM fourcc pixel format identifier (kernel uapi `drm_fourcc.h`).
///
/// A fourcc tag is a 32-bit identifier formed from four ASCII characters
/// in little-endian byte order. For example `XRGB8888` corresponds to
/// the bytes `'X','R','2','4'`, which encodes to `0x34325258`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fourcc(pub u32);

impl Fourcc {
    /// XRGB8888 — packed 32-bit, ignored alpha. `'X','R','2','4'`.
    pub const XRGB8888: Self = Self(0x34325258);
    /// ARGB8888 — packed 32-bit BGRA in memory order with alpha. `'A','R','2','4'`.
    pub const ARGB8888: Self = Self(0x34325241);
    /// XBGR8888. `'X','B','2','4'`.
    pub const XBGR8888: Self = Self(0x34324258);
    /// ABGR8888. `'A','B','2','4'`.
    pub const ABGR8888: Self = Self(0x34324241);

    /// Construct a `Fourcc` from its four ASCII bytes interpreted in
    /// little-endian order, matching the kernel `fourcc_code!` macro.
    pub const fn from_bytes(bytes: [u8; 4]) -> Self {
        Self(u32::from_le_bytes(bytes))
    }
}

// Pin the named-constant encoding at compile time so accidental drift in
// the literals fails the build rather than the synthetic regressions.
const _: () = {
    assert!(Fourcc::XRGB8888.0 == u32::from_le_bytes([b'X', b'R', b'2', b'4']));
    assert!(Fourcc::ARGB8888.0 == u32::from_le_bytes([b'A', b'R', b'2', b'4']));
    assert!(Fourcc::XBGR8888.0 == u32::from_le_bytes([b'X', b'B', b'2', b'4']));
    assert!(Fourcc::ABGR8888.0 == u32::from_le_bytes([b'A', b'B', b'2', b'4']));
};

/// Encoded `DRM_IOCTL_MODE_CREATE_DUMB` request number.
///
/// `DRM_IOWR(0xB2, struct drm_mode_create_dumb)` per the kernel `drm.h`
/// uapi headers. The reserved `0xB3` slot is `MODE_MAP_DUMB` and is not
/// in scope for this slice.
#[cfg(any(test, target_os = "linux"))]
const DRM_IOCTL_MODE_CREATE_DUMB: core::ffi::c_ulong =
    crate::ioctl::drm_iowr(0xB2, std::mem::size_of::<DrmModeCreateDumb>());

/// Encoded `DRM_IOCTL_MODE_DESTROY_DUMB` request number.
///
/// `DRM_IOWR(0xB4, struct drm_mode_destroy_dumb)` per the kernel `drm.h`
/// uapi headers.
#[cfg(any(test, target_os = "linux"))]
const DRM_IOCTL_MODE_DESTROY_DUMB: core::ffi::c_ulong =
    crate::ioctl::drm_iowr(0xB4, std::mem::size_of::<DrmModeDestroyDumb>());

/// Encoded `DRM_IOCTL_MODE_ADDFB2` request number.
///
/// `DRM_IOWR(0xB8, struct drm_mode_fb_cmd2)` per the kernel `drm.h` uapi
/// headers.
#[cfg(any(test, target_os = "linux"))]
const DRM_IOCTL_MODE_ADDFB2: core::ffi::c_ulong =
    crate::ioctl::drm_iowr(0xB8, std::mem::size_of::<DrmModeFbCmd2>());

/// Encoded `DRM_IOCTL_MODE_RMFB` request number.
///
/// `DRM_IOWR(0xAF, unsigned int)` per the kernel `drm.h` uapi headers —
/// the argument is just the framebuffer id.
#[cfg(any(test, target_os = "linux"))]
const DRM_IOCTL_MODE_RMFB: core::ffi::c_ulong =
    crate::ioctl::drm_iowr(0xAF, std::mem::size_of::<core::ffi::c_uint>());

/// `DRM_IOCTL_MODE_CREATE_DUMB` argument layout.
#[cfg(any(test, target_os = "linux"))]
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct DrmModeCreateDumb {
    pub height: u32,
    pub width: u32,
    pub bpp: u32,
    pub flags: u32,
    pub handle: u32,
    pub pitch: u32,
    pub size: u64,
}

/// `DRM_IOCTL_MODE_DESTROY_DUMB` argument layout.
#[cfg(any(test, target_os = "linux"))]
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct DrmModeDestroyDumb {
    pub handle: u32,
}

/// `DRM_IOCTL_MODE_ADDFB2` argument layout.
#[cfg(any(test, target_os = "linux"))]
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct DrmModeFbCmd2 {
    pub fb_id: u32,
    pub width: u32,
    pub height: u32,
    pub pixel_format: u32,
    pub flags: u32,
    pub handles: [u32; 4],
    pub pitches: [u32; 4],
    pub offsets: [u32; 4],
    pub modifier: [u64; 4],
}

/// Translate `(width, height, bpp)` into a zero-output `DrmModeCreateDumb`
/// argument block ready for `DRM_IOCTL_MODE_CREATE_DUMB`.
///
/// `flags` is left at zero (kernel default). Output fields (`handle`,
/// `pitch`, `size`) are zeroed and populated by the kernel on success.
#[cfg(any(test, target_os = "linux"))]
pub(crate) fn create_dumb_buffer_args(width: u32, height: u32, bpp: u32) -> DrmModeCreateDumb {
    DrmModeCreateDumb {
        height,
        width,
        bpp,
        flags: 0,
        handle: 0,
        pitch: 0,
        size: 0,
    }
}

/// Translate a [`DumbBuffer`] plus a fourcc `pixel_format` and ioctl `flags`
/// into a single-plane `DrmModeFbCmd2` argument block.
///
/// Only plane 0 is populated — `handles[0]` carries the GEM handle and
/// `pitches[0]` carries the row stride. All remaining slots, including
/// `offsets` and `modifier`, are zeroed.
#[cfg(any(test, target_os = "linux"))]
pub(crate) fn add_fb2_args(buffer: &DumbBuffer, pixel_format: Fourcc, flags: u32) -> DrmModeFbCmd2 {
    let mut cmd = DrmModeFbCmd2 {
        fb_id: 0,
        width: buffer.width,
        height: buffer.height,
        pixel_format: pixel_format.0,
        flags,
        handles: [0; 4],
        pitches: [0; 4],
        offsets: [0; 4],
        modifier: [0; 4],
    };
    cmd.handles[0] = buffer.handle;
    cmd.pitches[0] = buffer.pitch;
    cmd
}

/// RAII handle for a kernel-allocated dumb buffer.
///
/// The Linux `Drop` impl issues `DRM_IOCTL_MODE_DESTROY_DUMB` via a private
/// stub. On non-Linux targets the drop is a host-safe no-op.
#[derive(Debug)]
#[allow(dead_code)] // fields consumed by Linux Drop ioctls and host-safe tests
pub struct DumbBuffer {
    /// GEM buffer handle returned by `DRM_IOCTL_MODE_CREATE_DUMB`.
    pub(crate) handle: u32,
    /// Row stride in bytes.
    pub(crate) pitch: u32,
    /// Total buffer size in bytes.
    pub(crate) size: u64,
    /// Width in pixels.
    pub(crate) width: u32,
    /// Height in pixels.
    pub(crate) height: u32,
    /// Bits per pixel.
    pub(crate) bpp: u32,
    /// DRM device fd used to issue the destroy ioctl on drop.
    pub(crate) device_fd: i32,
}

/// A KMS framebuffer object backed by a [`DumbBuffer`].
///
/// On Linux, `Drop` first issues `DRM_IOCTL_MODE_RMFB` (so the kernel
/// releases its reference to the buffer object) and then drops the inner
/// `DumbBuffer`, which in turn issues `DRM_IOCTL_MODE_DESTROY_DUMB`.
#[derive(Debug)]
pub struct DrmFramebuffer {
    /// KMS framebuffer object id.
    pub id: FramebufferId,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// DRM fourcc pixel format used when registering this FB via
    /// `DRM_IOCTL_MODE_ADDFB2`.
    pub pixel_format: Fourcc,
    /// Backing dumb buffer. Released *after* the FB is removed.
    pub dumb: DumbBuffer,
}

impl DrmFramebuffer {
    /// Allocates a dumb buffer + FB object via `CREATE_DUMB` + `ADDFB2`.
    ///
    /// On Linux, runs the full lifecycle:
    ///   1. `CREATE_DUMB` to allocate a GEM-backed scanout buffer.
    ///   2. `ADDFB2` to register a KMS framebuffer object pointing at it.
    ///   3. On error from step 2, the local `dumb` drops and issues
    ///      `DESTROY_DUMB` automatically — releasing the kernel-side handle.
    ///
    /// Format defaults to [`Fourcc::XRGB8888`], the most universally
    /// supported dumb-buffer format. A future slice may parametrize it.
    /// `depth` is accepted for API compatibility with legacy `ADDFB` callers
    /// but is ignored — `ADDFB2` takes a fourcc directly.
    #[cfg(target_os = "linux")]
    pub fn create(
        device: &DrmDevice,
        width: u32,
        height: u32,
        bpp: u32,
        depth: u32,
    ) -> Result<Self> {
        let _ = depth;
        create_via_fd(device.fd(), width, height, bpp)
    }

    #[cfg(not(target_os = "linux"))]
    pub fn create(
        _device: &DrmDevice,
        _width: u32,
        _height: u32,
        _bpp: u32,
        _depth: u32,
    ) -> Result<Self> {
        Err(DrmError::NoDevice)
    }

    /// Memory-maps the framebuffer for CPU writes. Currently a stub.
    #[cfg(target_os = "linux")]
    pub fn map(&self, device: &DrmDevice) -> Result<*mut u8> {
        let _ = device;
        Err(DrmError::BufferAlloc("map not yet implemented".to_string()))
    }

    #[cfg(not(target_os = "linux"))]
    pub fn map(&self, _device: &DrmDevice) -> Result<*mut u8> {
        Err(DrmError::NoDevice)
    }
}

// Test-only drop-order recorder.
//
// Both `DumbBuffer::drop` and `DrmFramebuffer::drop` push a tag into this
// recorder so FB-before-dumb ordering can be asserted in synthetic
// host-safe tests.
#[cfg(test)]
thread_local! {
    pub(crate) static DROP_RECORDER: std::cell::RefCell<Vec<&'static str>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

impl Drop for DumbBuffer {
    fn drop(&mut self) {
        #[cfg(test)]
        {
            DROP_RECORDER.with(|r| r.borrow_mut().push("dumb"));
        }
        #[cfg(target_os = "linux")]
        {
            // Best-effort release; Drop must not panic.
            if let Err(_err) = destroy_dumb_via_ioctl(self.device_fd, self.handle) {
                // Nothing actionable here.
            }
        }
    }
}

impl Drop for DrmFramebuffer {
    fn drop(&mut self) {
        #[cfg(test)]
        {
            DROP_RECORDER.with(|r| r.borrow_mut().push("fb"));
        }
        #[cfg(target_os = "linux")]
        {
            // Tear down the FB object first so the kernel drops its
            // reference to the dumb buffer before `DESTROY_DUMB` runs as
            // part of the inner `DumbBuffer` field drop.
            if let Err(_err) = rmfb_via_ioctl(self.dumb.device_fd, self.id.0) {
                // Nothing actionable here.
            }
        }
        // `self.dumb` is dropped automatically after this method returns.
    }
}

/// Allocate a dumb buffer via `DRM_IOCTL_MODE_CREATE_DUMB`.
///
/// Takes a raw `fd` rather than `&DrmDevice` so host-side regressions can
/// drive it with a sentinel descriptor under the t40 mock dispatch layer
/// (see `crate::ioctl::mock`). On Linux production builds, the public
/// [`DrmFramebuffer::create`] entry point forwards `device.fd()` here.
///
/// On success the returned [`DumbBuffer`] carries the kernel-populated
/// `handle`, `pitch`, and `size`, plus the original `width`, `height`,
/// `bpp`, and the caller-supplied `device_fd` used by its `Drop` impl to
/// issue the matching `DESTROY_DUMB`.
#[cfg(any(test, target_os = "linux"))]
pub(crate) fn allocate_dumb_buffer_via_fd(
    fd: i32,
    width: u32,
    height: u32,
    bpp: u32,
) -> Result<DumbBuffer> {
    let mut args = create_dumb_buffer_args(width, height, bpp);
    crate::ioctl::drm_ioctl(fd, DRM_IOCTL_MODE_CREATE_DUMB, "MODE_CREATE_DUMB", &mut args)?;
    Ok(DumbBuffer {
        handle: args.handle,
        pitch: args.pitch,
        size: args.size,
        width: args.width,
        height: args.height,
        bpp: args.bpp,
        device_fd: fd,
    })
}

/// Issue `DRM_IOCTL_MODE_DESTROY_DUMB` for the given GEM handle.
///
/// Routed through [`crate::ioctl::drm_ioctl`] so under `#[cfg(test)]` the
/// t40 mock dispatch layer can intercept the call on any host OS. The
/// only production caller is [`DumbBuffer::drop`] under
/// `#[cfg(target_os = "linux")]`; on a non-Linux test build the function
/// is reached only from synthetic regressions, hence the `dead_code`
/// allowance for that configuration.
#[cfg(any(test, target_os = "linux"))]
#[cfg_attr(all(test, not(target_os = "linux")), allow(dead_code))]
pub(crate) fn destroy_dumb_via_ioctl(device_fd: i32, handle: u32) -> Result<()> {
    let mut args = DrmModeDestroyDumb { handle };
    crate::ioctl::drm_ioctl(
        device_fd,
        DRM_IOCTL_MODE_DESTROY_DUMB,
        "MODE_DESTROY_DUMB",
        &mut args,
    )
}

/// Issue `DRM_IOCTL_MODE_RMFB` for the given framebuffer id.
///
/// Routed through [`crate::ioctl::drm_ioctl`] so under `#[cfg(test)]` the
/// t40 mock dispatch layer can intercept the call on any host OS. Drop in
/// [`DrmFramebuffer`] is the only production caller and is itself
/// `#[cfg(target_os = "linux")]`-gated; on a non-Linux test build this
/// function is reached only from synthetic regressions.
#[cfg(any(test, target_os = "linux"))]
#[cfg_attr(all(test, not(target_os = "linux")), allow(dead_code))]
pub(crate) fn rmfb_via_ioctl(device_fd: i32, fb_id: u32) -> Result<()> {
    let mut arg: u32 = fb_id;
    crate::ioctl::drm_ioctl(device_fd, DRM_IOCTL_MODE_RMFB, "MODE_RMFB", &mut arg)
}

/// Register a KMS framebuffer over an existing [`DumbBuffer`] via
/// `DRM_IOCTL_MODE_ADDFB2`, returning the kernel-assigned [`FramebufferId`].
///
/// Single-plane only — plane 0 takes the buffer's GEM handle and pitch;
/// all higher slots are zeroed. See [`add_fb2_args`].
#[cfg(any(test, target_os = "linux"))]
pub(crate) fn add_fb2_via_fd(
    fd: i32,
    buffer: &DumbBuffer,
    pixel_format: Fourcc,
    flags: u32,
) -> Result<FramebufferId> {
    let mut args = add_fb2_args(buffer, pixel_format, flags);
    crate::ioctl::drm_ioctl(fd, DRM_IOCTL_MODE_ADDFB2, "MODE_ADDFB2", &mut args)?;
    Ok(FramebufferId(args.fb_id))
}

/// Full `CREATE_DUMB` → `ADDFB2` lifecycle against a raw fd.
///
/// Symmetric with [`allocate_dumb_buffer_via_fd`] / [`destroy_dumb_via_ioctl`]
/// so host-side regressions can drive the public [`DrmFramebuffer::create`]
/// surface under the t40 mock dispatch layer without constructing a real
/// [`DrmDevice`]. The Linux production entry point forwards `device.fd()`
/// here.
///
/// On `ADDFB2` failure the freshly-allocated [`DumbBuffer`] is dropped on
/// the error path, releasing the kernel-side handle via `DESTROY_DUMB`.
#[cfg(any(test, target_os = "linux"))]
pub(crate) fn create_via_fd(
    fd: i32,
    width: u32,
    height: u32,
    bpp: u32,
) -> Result<DrmFramebuffer> {
    let dumb = allocate_dumb_buffer_via_fd(fd, width, height, bpp)?;
    let pixel_format = Fourcc::XRGB8888;
    // Explicit `match` so the error path's `dumb` drop is obvious — `?` would
    // also work because `dumb` is a stack local that drops on early return.
    let id = match add_fb2_via_fd(fd, &dumb, pixel_format, 0) {
        Ok(id) => id,
        Err(err) => {
            // `dumb` drops here, releasing the kernel-side handle.
            return Err(err);
        }
    };
    Ok(DrmFramebuffer {
        id,
        width,
        height,
        pixel_format,
        dumb,
    })
}
