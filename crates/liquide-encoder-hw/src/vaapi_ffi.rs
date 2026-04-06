//! Runtime FFI bindings to libva (loaded via dlopen).
//!
//! All symbols are loaded at runtime so the binary compiles on any platform.
//! On non-Unix targets every function returns `None` / is a no-op.

#![allow(non_camel_case_types)]

use std::ffi::c_void;
use std::sync::OnceLock;

// ---------------------------------------------------------------------------
// VA-API C types (from va/va.h)
// ---------------------------------------------------------------------------

pub type VADisplay = *mut c_void;
pub type VASurfaceID = u32;
pub type VAConfigID = u32;
pub type VAContextID = u32;
pub type VABufferID = u32;
pub type VAStatus = i32;
pub type VAProfile = i32;
pub type VAEntrypoint = i32;

// ---------------------------------------------------------------------------
// VA constants
// ---------------------------------------------------------------------------

pub const VA_STATUS_SUCCESS: VAStatus = 0;

/// H.264 High profile.
pub const VA_PROFILE_H264_HIGH: VAProfile = 8;
/// HEVC / H.265 Main profile.
pub const VA_PROFILE_HEVC_MAIN: VAProfile = 12;
/// Encode slice entry-point.
pub const VA_ENTRYPOINT_ENCSLICE: VAEntrypoint = 5;
/// YUV 4:2:0 render-target format.
pub const VA_RT_FORMAT_YUV420: u32 = 0x0000_0001;

// VA buffer types we need for encoding.
pub const VA_ENC_SEQUENCE_PARAMETER_BUFFER_TYPE: u32 = 21;
pub const VA_ENC_PICTURE_PARAMETER_BUFFER_TYPE: u32 = 22;
pub const VA_ENC_SLICE_PARAMETER_BUFFER_TYPE: u32 = 23;
pub const VA_ENC_CODED_BUFFER_TYPE: u32 = 10;

// ---------------------------------------------------------------------------
// DMA-BUF import constants and structures
// ---------------------------------------------------------------------------

/// VASurfaceAttribType: memory type.
pub const VA_SURFACE_ATTRIB_MEM_TYPE: u32 = 0x0000_0001;
/// VASurfaceAttribType: external buffer descriptor.
pub const VA_SURFACE_ATTRIB_EXTERNAL_BUFFERS: u32 = 0x0000_0004;
/// Memory type: DRM PRIME (DMA-BUF fd).
pub const VA_SURFACE_ATTRIB_MEM_TYPE_DRM_PRIME: u32 = 0x2000_0000;
/// VA_SURFACE_ATTRIB_SETTABLE flag.
pub const VA_SURFACE_ATTRIB_SETTABLE: u32 = 2;

/// FOURCC for BGRX (BGRA without alpha channel), little-endian 'XRGB'.
pub const VA_FOURCC_BGRX: u32 = 0x5852_4742;

/// Surface attribute for `vaCreateSurfaces`.
#[repr(C)]
pub struct VASurfaceAttrib {
    /// Attribute type (e.g. `VA_SURFACE_ATTRIB_MEM_TYPE`).
    pub type_: u32,
    /// Flags (e.g. `VA_SURFACE_ATTRIB_SETTABLE`).
    pub flags: u32,
    /// Value type: 0 = int, 1 = float, 3 = pointer.
    pub value_type: u32,
    /// The value — interpreted as int or pointer depending on `value_type`.
    pub value: u64,
}

/// External buffer descriptor for DMA-BUF import via `vaCreateSurfaces`.
#[repr(C)]
pub struct VASurfaceAttribExternalBuffers {
    /// Pixel format (VA_FOURCC).
    pub pixel_format: u32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Total data size in bytes.
    pub data_size: u32,
    /// Number of planes.
    pub num_planes: u32,
    /// Per-plane row pitches.
    pub pitches: [u32; 4],
    /// Per-plane byte offsets.
    pub offsets: [u32; 4],
    /// Pointer to array of DMA-BUF file descriptors (as `i64`).
    pub buffers: *const i64,
    /// Number of buffers.
    pub num_buffers: u32,
    /// Flags (reserved, set to 0).
    pub flags: u32,
    /// Private data (reserved, set to null).
    pub private_data: *const c_void,
}

/// Coded-buffer segment returned by `vaMapBuffer` on an encoded-output buffer.
#[repr(C)]
pub struct VACodedBufferSegment {
    /// Size of the coded data in bytes.
    pub size: u32,
    /// Bit-offset of the first bit in the buffer (usually 0).
    pub bit_offset: u32,
    /// Status flags.
    pub status: u32,
    /// Reserved.
    pub reserved: u32,
    /// Pointer to the coded data.
    pub buf: *mut c_void,
    /// Pointer to the next segment (linked list), or null.
    pub next: *mut VACodedBufferSegment,
}

// ---------------------------------------------------------------------------
// Dynamically-loaded function table
// ---------------------------------------------------------------------------

/// Dynamically loaded libva + libva-drm function pointers.
pub struct VaLib {
    // Keep handles alive so the OS doesn't unload the libraries.
    _handle: *mut c_void,
    _drm_handle: *mut c_void,

    // --- core libva ---
    pub va_initialize:
        unsafe extern "C" fn(VADisplay, *mut i32, *mut i32) -> VAStatus,
    pub va_terminate: unsafe extern "C" fn(VADisplay) -> VAStatus,
    pub va_max_num_profiles: unsafe extern "C" fn(VADisplay) -> i32,
    pub va_query_config_profiles:
        unsafe extern "C" fn(VADisplay, *mut VAProfile, *mut i32) -> VAStatus,
    pub va_max_num_entrypoints: unsafe extern "C" fn(VADisplay) -> i32,
    pub va_query_config_entrypoints: unsafe extern "C" fn(
        VADisplay,
        VAProfile,
        *mut VAEntrypoint,
        *mut i32,
    ) -> VAStatus,
    pub va_create_config: unsafe extern "C" fn(
        VADisplay,
        VAProfile,
        VAEntrypoint,
        *mut c_void, // attrib list (may be null)
        i32,         // num attribs
        *mut VAConfigID,
    ) -> VAStatus,
    pub va_destroy_config:
        unsafe extern "C" fn(VADisplay, VAConfigID) -> VAStatus,
    pub va_create_context: unsafe extern "C" fn(
        VADisplay,
        VAConfigID,
        i32,              // width
        i32,              // height
        i32,              // flags
        *mut VASurfaceID, // render targets
        i32,              // num render targets
        *mut VAContextID,
    ) -> VAStatus,
    pub va_destroy_context:
        unsafe extern "C" fn(VADisplay, VAContextID) -> VAStatus,
    pub va_create_surfaces: unsafe extern "C" fn(
        VADisplay,
        u32,              // RT format
        u32,              // width
        u32,              // height
        *mut VASurfaceID, // out surfaces
        u32,              // num surfaces
        *mut c_void,      // attrib list
        u32,              // num attribs
    ) -> VAStatus,
    pub va_destroy_surfaces:
        unsafe extern "C" fn(VADisplay, *mut VASurfaceID, i32) -> VAStatus,
    pub va_create_buffer: unsafe extern "C" fn(
        VADisplay,
        VAContextID,
        u32,         // buffer type
        u32,         // size
        u32,         // num elements
        *mut c_void, // data (may be null)
        *mut VABufferID,
    ) -> VAStatus,
    pub va_destroy_buffer:
        unsafe extern "C" fn(VADisplay, VABufferID) -> VAStatus,
    pub va_begin_picture:
        unsafe extern "C" fn(VADisplay, VAContextID, VASurfaceID) -> VAStatus,
    pub va_render_picture: unsafe extern "C" fn(
        VADisplay,
        VAContextID,
        *mut VABufferID,
        i32,
    ) -> VAStatus,
    pub va_end_picture:
        unsafe extern "C" fn(VADisplay, VAContextID) -> VAStatus,
    pub va_sync_surface:
        unsafe extern "C" fn(VADisplay, VASurfaceID) -> VAStatus,
    pub va_map_buffer: unsafe extern "C" fn(
        VADisplay,
        VABufferID,
        *mut *mut c_void,
    ) -> VAStatus,
    pub va_unmap_buffer:
        unsafe extern "C" fn(VADisplay, VABufferID) -> VAStatus,

    // --- libva-drm ---
    pub va_get_display_drm: unsafe extern "C" fn(i32) -> VADisplay,
}

// Safety: VaLib only holds function pointers and opaque library handles.
// The function pointers themselves are thread-safe (they're just addresses).
unsafe impl Send for VaLib {}
unsafe impl Sync for VaLib {}

static VA_LIB: OnceLock<Option<VaLib>> = OnceLock::new();

impl VaLib {
    /// Try to load `libva.so.2` + `libva-drm.so.2` at runtime.
    ///
    /// Returns `None` if the libraries are not found (e.g. on Windows, or a
    /// Linux system without Mesa). The result is cached for the process
    /// lifetime.
    pub fn load() -> Option<&'static VaLib> {
        VA_LIB
            .get_or_init(|| {
                #[cfg(target_os = "linux")]
                {
                    Self::try_load_linux()
                }
                #[cfg(not(target_os = "linux"))]
                {
                    None
                }
            })
            .as_ref()
    }

    #[cfg(target_os = "linux")]
    fn try_load_linux() -> Option<VaLib> {
        // We declare the tiny subset of libc we need inline so we don't pull
        // in the whole libc crate just for dlopen/dlsym/open/close.
        extern "C" {
            fn dlopen(filename: *const u8, flags: i32) -> *mut c_void;
            fn dlsym(handle: *mut c_void, symbol: *const u8) -> *mut c_void;
            fn open(path: *const u8, flags: i32) -> i32;
            fn close(fd: i32) -> i32;
        }

        const RTLD_NOW: i32 = 0x0002;
        const RTLD_LOCAL: i32 = 0;
        const _O_RDWR: i32 = 2;

        // --- load libraries ---
        let handle = unsafe {
            dlopen(b"libva.so.2\0".as_ptr(), RTLD_NOW | RTLD_LOCAL)
        };
        if handle.is_null() {
            return None;
        }

        let drm_handle = unsafe {
            dlopen(b"libva-drm.so.2\0".as_ptr(), RTLD_NOW | RTLD_LOCAL)
        };
        if drm_handle.is_null() {
            // libva-drm is required for `vaGetDisplayDRM`.
            return None;
        }

        /// Load a symbol from a handle, returning `None` on failure.
        macro_rules! sym {
            ($h:expr, $name:literal) => {{
                let p = unsafe { dlsym($h, concat!($name, "\0").as_ptr()) };
                if p.is_null() {
                    return None;
                }
                unsafe { std::mem::transmute(p) }
            }};
        }

        Some(VaLib {
            _handle: handle,
            _drm_handle: drm_handle,

            va_initialize: sym!(handle, "vaInitialize"),
            va_terminate: sym!(handle, "vaTerminate"),
            va_max_num_profiles: sym!(handle, "vaMaxNumProfiles"),
            va_query_config_profiles: sym!(handle, "vaQueryConfigProfiles"),
            va_max_num_entrypoints: sym!(handle, "vaMaxNumEntrypoints"),
            va_query_config_entrypoints: sym!(
                handle,
                "vaQueryConfigEntrypoints"
            ),
            va_create_config: sym!(handle, "vaCreateConfig"),
            va_destroy_config: sym!(handle, "vaDestroyConfig"),
            va_create_context: sym!(handle, "vaCreateContext"),
            va_destroy_context: sym!(handle, "vaDestroyContext"),
            va_create_surfaces: sym!(handle, "vaCreateSurfaces"),
            va_destroy_surfaces: sym!(handle, "vaDestroySurfaces"),
            va_create_buffer: sym!(handle, "vaCreateBuffer"),
            va_destroy_buffer: sym!(handle, "vaDestroyBuffer"),
            va_begin_picture: sym!(handle, "vaBeginPicture"),
            va_render_picture: sym!(handle, "vaRenderPicture"),
            va_end_picture: sym!(handle, "vaEndPicture"),
            va_sync_surface: sym!(handle, "vaSyncSurface"),
            va_map_buffer: sym!(handle, "vaMapBuffer"),
            va_unmap_buffer: sym!(handle, "vaUnmapBuffer"),
            va_get_display_drm: sym!(drm_handle, "vaGetDisplayDRM"),
        })
    }
}

// ---------------------------------------------------------------------------
// Thin helpers for opening/closing render nodes
// ---------------------------------------------------------------------------

/// Open a DRM render node. Returns the fd or -1 on failure.
#[cfg(target_os = "linux")]
pub fn open_render_node(path: &[u8]) -> i32 {
    extern "C" {
        fn open(path: *const u8, flags: i32) -> i32;
    }
    const O_RDWR: i32 = 2;
    // Caller must ensure `path` is null-terminated.
    unsafe { open(path.as_ptr(), O_RDWR) }
}

/// Close a file descriptor.
#[cfg(target_os = "linux")]
pub fn close_fd(fd: i32) {
    extern "C" {
        fn close(fd: i32) -> i32;
    }
    unsafe {
        close(fd);
    }
}
