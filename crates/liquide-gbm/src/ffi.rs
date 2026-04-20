//! Runtime-loaded GBM FFI bindings via `dlopen`.
//!
//! All GBM symbols are resolved at runtime from `libgbm.so.1` (or `libgbm.so`
//! as fallback). This avoids a hard link-time dependency and lets the compositor
//! produce a clear error when libgbm is not installed.

#![cfg(target_os = "linux")]

use std::ffi::{c_void, CStr, CString};
use std::os::raw::c_int;
use std::sync::OnceLock;

// ---------------------------------------------------------------------------
// Opaque types – mirrors of the C `struct gbm_*` types.  We never dereference
// these; they exist only so our pointer types are distinct from `*mut c_void`.
// ---------------------------------------------------------------------------

#[repr(C)]
pub(crate) struct gbm_device {
    _opaque: [u8; 0],
}

#[repr(C)]
pub(crate) struct gbm_bo {
    _opaque: [u8; 0],
}

#[repr(C)]
pub(crate) struct gbm_surface {
    _opaque: [u8; 0],
}

/// Union returned by `gbm_bo_get_handle`. We only use the `u32` member.
#[repr(C)]
#[derive(Copy, Clone)]
pub(crate) union gbm_bo_handle {
    pub u32_: u32,
    pub u64_: u64,
    pub ptr: *mut c_void,
    pub fd: i32,
}

// ---------------------------------------------------------------------------
// Function-pointer table
// ---------------------------------------------------------------------------

pub(crate) struct GbmFfi {
    _lib: *mut c_void,
    pub gbm_create_device: unsafe extern "C" fn(fd: c_int) -> *mut gbm_device,
    pub gbm_device_destroy: unsafe extern "C" fn(dev: *mut gbm_device),
    pub gbm_device_get_fd: unsafe extern "C" fn(dev: *const gbm_device) -> c_int,
    pub gbm_bo_create: unsafe extern "C" fn(
        dev: *mut gbm_device,
        width: u32,
        height: u32,
        format: u32,
        flags: u32,
    ) -> *mut gbm_bo,
    pub gbm_bo_destroy: unsafe extern "C" fn(bo: *mut gbm_bo),
    pub gbm_bo_get_handle: unsafe extern "C" fn(bo: *mut gbm_bo) -> gbm_bo_handle,
    pub gbm_bo_get_stride: unsafe extern "C" fn(bo: *mut gbm_bo) -> u32,
    pub gbm_bo_get_fd: unsafe extern "C" fn(bo: *mut gbm_bo) -> c_int,
    pub gbm_bo_get_modifier: unsafe extern "C" fn(bo: *mut gbm_bo) -> u64,
    pub gbm_bo_map: unsafe extern "C" fn(
        bo: *mut gbm_bo,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        flags: u32,
        stride: *mut u32,
        map_data: *mut *mut c_void,
    ) -> *mut c_void,
    pub gbm_bo_unmap: unsafe extern "C" fn(bo: *mut gbm_bo, map_data: *mut c_void),
    pub gbm_surface_create: unsafe extern "C" fn(
        dev: *mut gbm_device,
        width: u32,
        height: u32,
        format: u32,
        flags: u32,
    ) -> *mut gbm_surface,
    pub gbm_surface_destroy: unsafe extern "C" fn(surface: *mut gbm_surface),
    pub gbm_surface_lock_front_buffer:
        unsafe extern "C" fn(surface: *mut gbm_surface) -> *mut gbm_bo,
    pub gbm_surface_release_buffer:
        unsafe extern "C" fn(surface: *mut gbm_surface, bo: *mut gbm_bo),
}

// SAFETY: The loaded function pointers are immutable after initialisation and
// the underlying C library is thread-safe for distinct objects.
unsafe impl Send for GbmFfi {}
unsafe impl Sync for GbmFfi {}

/// GBM_BO_MAP flags
pub(crate) const GBM_BO_TRANSFER_WRITE: u32 = 2;

// ---------------------------------------------------------------------------
// dlopen helpers
// ---------------------------------------------------------------------------

static GBM_LIB: OnceLock<Result<GbmFfi, String>> = OnceLock::new();

/// Obtain a reference to the lazily-loaded GBM FFI table.
pub(crate) fn gbm_ffi() -> Result<&'static GbmFfi, String> {
    GBM_LIB
        .get_or_init(|| load_gbm_ffi())
        .as_ref()
        .map_err(|e| e.clone())
}

fn load_gbm_ffi() -> Result<GbmFfi, String> {
    // SAFETY: We are loading a well-known system library.  The library names
    // are hardcoded constants, not user-controlled.
    let lib = unsafe { try_dlopen(&[b"libgbm.so.1\0", b"libgbm.so\0"])? };

    macro_rules! load_sym {
        ($lib:expr, $name:literal) => {{
            let cname = concat!($name, "\0");
            // SAFETY: `lib` is a valid library handle obtained from dlopen,
            // and `cname` is a NUL-terminated constant string naming a known
            // GBM symbol.
            let sym = unsafe { libc::dlsym($lib, cname.as_ptr() as *const _) };
            if sym.is_null() {
                let err = dl_error();
                return Err(format!(
                    "failed to load symbol `{}`: {}",
                    $name, err
                ));
            }
            // SAFETY: We verified the symbol is non-null. The transmute
            // converts to the matching extern "C" fn signature which is
            // correct per the GBM C ABI.
            unsafe { std::mem::transmute(sym) }
        }};
    }

    Ok(GbmFfi {
        _lib: lib,
        gbm_create_device: load_sym!(lib, "gbm_create_device"),
        gbm_device_destroy: load_sym!(lib, "gbm_device_destroy"),
        gbm_device_get_fd: load_sym!(lib, "gbm_device_get_fd"),
        gbm_bo_create: load_sym!(lib, "gbm_bo_create"),
        gbm_bo_destroy: load_sym!(lib, "gbm_bo_destroy"),
        gbm_bo_get_handle: load_sym!(lib, "gbm_bo_get_handle"),
        gbm_bo_get_stride: load_sym!(lib, "gbm_bo_get_stride"),
        gbm_bo_get_fd: load_sym!(lib, "gbm_bo_get_fd"),
        gbm_bo_get_modifier: load_sym!(lib, "gbm_bo_get_modifier"),
        gbm_bo_map: load_sym!(lib, "gbm_bo_map"),
        gbm_bo_unmap: load_sym!(lib, "gbm_bo_unmap"),
        gbm_surface_create: load_sym!(lib, "gbm_surface_create"),
        gbm_surface_destroy: load_sym!(lib, "gbm_surface_destroy"),
        gbm_surface_lock_front_buffer: load_sym!(lib, "gbm_surface_lock_front_buffer"),
        gbm_surface_release_buffer: load_sym!(lib, "gbm_surface_release_buffer"),
    })
}

/// Try opening a shared library from a list of candidate names (first success wins).
///
/// # Safety
/// Caller must pass NUL-terminated byte strings that name known system libraries.
unsafe fn try_dlopen(names: &[&[u8]]) -> Result<*mut c_void, String> {
    for name in names {
        let ptr = libc::dlopen(name.as_ptr() as *const _, libc::RTLD_NOW | libc::RTLD_LOCAL);
        if !ptr.is_null() {
            let name_str = CStr::from_bytes_with_nul(name)
                .map(|c| c.to_string_lossy())
                .unwrap_or_default();
            tracing::info!("loaded GBM library: {}", name_str);
            return Ok(ptr);
        }
    }
    Err(format!(
        "could not load libgbm — is mesa/libgbm installed? ({})",
        dl_error()
    ))
}

fn dl_error() -> String {
    // SAFETY: dlerror returns a thread-local NUL-terminated string or null.
    let ptr = unsafe { libc::dlerror() };
    if ptr.is_null() {
        "unknown error".to_string()
    } else {
        // SAFETY: ptr is non-null and NUL-terminated per dlerror contract.
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned()
    }
}
