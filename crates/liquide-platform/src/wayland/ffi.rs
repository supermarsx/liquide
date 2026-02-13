//! Raw FFI bindings to libwayland-client and Linux system calls.
//!
//! All Wayland protocol objects are opaque pointers behind `*mut c_void`.
//! Struct definitions are only provided for types whose layout we must
//! know: [`wl_interface`], [`wl_message`], and [`wl_array`].

#![allow(non_camel_case_types, dead_code)]

use std::ffi::{c_char, c_int, c_void};

// ── Opaque Wayland protocol object types ────────────────────────────────

/// Opaque type alias — every Wayland proxy is represented as `*mut c_void`.
pub type wl_display = c_void;
pub type wl_registry = c_void;
pub type wl_compositor = c_void;
pub type wl_surface = c_void;
pub type wl_shm = c_void;
pub type wl_shm_pool = c_void;
pub type wl_buffer = c_void;
pub type wl_seat = c_void;
pub type wl_keyboard = c_void;
pub type wl_pointer = c_void;
pub type wl_output = c_void;
pub type wl_callback = c_void;
pub type wl_region = c_void;
pub type wl_proxy = c_void;

// XDG shell protocol objects (same library at runtime).
pub type xdg_wm_base = c_void;
pub type xdg_surface = c_void;
pub type xdg_toplevel = c_void;

// ── Wayland wire-protocol structs ───────────────────────────────────────

/// Describes a Wayland interface (name, version, request/event signatures).
#[repr(C)]
pub struct wl_interface {
    pub name: *const c_char,
    pub version: c_int,
    pub method_count: c_int,
    pub methods: *const wl_message,
    pub event_count: c_int,
    pub events: *const wl_message,
}

unsafe impl Send for wl_interface {}
unsafe impl Sync for wl_interface {}

/// Describes a single request or event message.
#[repr(C)]
pub struct wl_message {
    pub name: *const c_char,
    pub signature: *const c_char,
    pub types: *const *const wl_interface,
}

unsafe impl Send for wl_message {}
unsafe impl Sync for wl_message {}

/// A dynamically-sized array used by the Wayland protocol.
#[repr(C)]
pub struct wl_array {
    pub size: usize,
    pub alloc: usize,
    pub data: *mut c_void,
}

// ── SHM format constants ────────────────────────────────────────────────

/// ARGB8888 — in memory on little-endian this is BGRA byte order.
pub const WL_SHM_FORMAT_ARGB8888: u32 = 0;
/// XRGB8888 — opaque variant (alpha channel ignored).
pub const WL_SHM_FORMAT_XRGB8888: u32 = 1;

// ── Keyboard key state constants ────────────────────────────────────────

pub const WL_KEYBOARD_KEY_STATE_RELEASED: u32 = 0;
pub const WL_KEYBOARD_KEY_STATE_PRESSED: u32 = 1;

// ── Seat capability flags ───────────────────────────────────────────────

pub const WL_SEAT_CAPABILITY_POINTER: u32 = 1;
pub const WL_SEAT_CAPABILITY_KEYBOARD: u32 = 2;
pub const WL_SEAT_CAPABILITY_TOUCH: u32 = 4;

// ── Pointer button codes (linux/input-event-codes.h) ────────────────────

pub const BTN_LEFT: u32 = 0x110;
pub const BTN_RIGHT: u32 = 0x111;
pub const BTN_MIDDLE: u32 = 0x112;
pub const BTN_SIDE: u32 = 0x113;
pub const BTN_EXTRA: u32 = 0x114;

// ── Pointer axis values ─────────────────────────────────────────────────

pub const WL_POINTER_AXIS_VERTICAL_SCROLL: u32 = 0;
pub const WL_POINTER_AXIS_HORIZONTAL_SCROLL: u32 = 1;

// ── wl_proxy_marshal_flags: flags ───────────────────────────────────────

pub const WL_MARSHAL_FLAG_DESTROY: u32 = 1;

// ── Wayland protocol opcodes ────────────────────────────────────────────

// wl_display requests
pub const WL_DISPLAY_GET_REGISTRY: u32 = 1;

// wl_registry requests
pub const WL_REGISTRY_BIND: u32 = 0;

// wl_compositor requests
pub const WL_COMPOSITOR_CREATE_SURFACE: u32 = 0;
pub const WL_COMPOSITOR_CREATE_REGION: u32 = 1;

// wl_surface requests
pub const WL_SURFACE_DESTROY: u32 = 0;
pub const WL_SURFACE_ATTACH: u32 = 1;
pub const WL_SURFACE_DAMAGE: u32 = 2;
pub const WL_SURFACE_FRAME: u32 = 3;
pub const WL_SURFACE_SET_OPAQUE_REGION: u32 = 4;
pub const WL_SURFACE_COMMIT: u32 = 6;
pub const WL_SURFACE_DAMAGE_BUFFER: u32 = 9;

// wl_shm requests
pub const WL_SHM_CREATE_POOL: u32 = 0;

// wl_shm_pool requests
pub const WL_SHM_POOL_CREATE_BUFFER: u32 = 0;
pub const WL_SHM_POOL_DESTROY: u32 = 2;

// wl_buffer requests
pub const WL_BUFFER_DESTROY: u32 = 0;

// wl_seat requests
pub const WL_SEAT_GET_POINTER: u32 = 0;
pub const WL_SEAT_GET_KEYBOARD: u32 = 1;

// wl_region requests
pub const WL_REGION_DESTROY: u32 = 0;

// xdg_wm_base requests
pub const XDG_WM_BASE_GET_XDG_SURFACE: u32 = 2;
pub const XDG_WM_BASE_PONG: u32 = 3;

// xdg_surface requests
pub const XDG_SURFACE_GET_TOPLEVEL: u32 = 1;
pub const XDG_SURFACE_ACK_CONFIGURE: u32 = 4;
pub const XDG_SURFACE_DESTROY: u32 = 0;

// xdg_toplevel requests
pub const XDG_TOPLEVEL_DESTROY: u32 = 0;
pub const XDG_TOPLEVEL_SET_TITLE: u32 = 2;
pub const XDG_TOPLEVEL_SET_APP_ID: u32 = 3;
pub const XDG_TOPLEVEL_SET_MAXIMIZED: u32 = 6;
pub const XDG_TOPLEVEL_UNSET_MAXIMIZED: u32 = 7;
pub const XDG_TOPLEVEL_SET_MINIMIZED: u32 = 9;
pub const XDG_TOPLEVEL_SET_FULLSCREEN: u32 = 8;
pub const XDG_TOPLEVEL_UNSET_FULLSCREEN: u32 = 10;

// wl_callback requests (none — only events)

// ── Listener callback function-pointer types ────────────────────────────

// wl_registry events
pub type wl_registry_global_fn = unsafe extern "C" fn(
    data: *mut c_void,
    registry: *mut wl_proxy,
    name: u32,
    interface: *const c_char,
    version: u32,
);
pub type wl_registry_global_remove_fn =
    unsafe extern "C" fn(data: *mut c_void, registry: *mut wl_proxy, name: u32);

/// Listener struct for wl_registry events.
#[repr(C)]
pub struct wl_registry_listener {
    pub global: wl_registry_global_fn,
    pub global_remove: wl_registry_global_remove_fn,
}

// wl_seat events
pub type wl_seat_capabilities_fn =
    unsafe extern "C" fn(data: *mut c_void, seat: *mut wl_proxy, capabilities: u32);
pub type wl_seat_name_fn =
    unsafe extern "C" fn(data: *mut c_void, seat: *mut wl_proxy, name: *const c_char);

#[repr(C)]
pub struct wl_seat_listener {
    pub capabilities: wl_seat_capabilities_fn,
    pub name: wl_seat_name_fn,
}

// wl_keyboard events
pub type wl_keyboard_keymap_fn = unsafe extern "C" fn(
    data: *mut c_void,
    keyboard: *mut wl_proxy,
    format: u32,
    fd: i32,
    size: u32,
);
pub type wl_keyboard_enter_fn = unsafe extern "C" fn(
    data: *mut c_void,
    keyboard: *mut wl_proxy,
    serial: u32,
    surface: *mut wl_proxy,
    keys: *mut wl_array,
);
pub type wl_keyboard_leave_fn = unsafe extern "C" fn(
    data: *mut c_void,
    keyboard: *mut wl_proxy,
    serial: u32,
    surface: *mut wl_proxy,
);
pub type wl_keyboard_key_fn = unsafe extern "C" fn(
    data: *mut c_void,
    keyboard: *mut wl_proxy,
    serial: u32,
    time: u32,
    key: u32,
    state: u32,
);
pub type wl_keyboard_modifiers_fn = unsafe extern "C" fn(
    data: *mut c_void,
    keyboard: *mut wl_proxy,
    serial: u32,
    mods_depressed: u32,
    mods_latched: u32,
    mods_locked: u32,
    group: u32,
);
pub type wl_keyboard_repeat_info_fn = unsafe extern "C" fn(
    data: *mut c_void,
    keyboard: *mut wl_proxy,
    rate: i32,
    delay: i32,
);

#[repr(C)]
pub struct wl_keyboard_listener {
    pub keymap: wl_keyboard_keymap_fn,
    pub enter: wl_keyboard_enter_fn,
    pub leave: wl_keyboard_leave_fn,
    pub key: wl_keyboard_key_fn,
    pub modifiers: wl_keyboard_modifiers_fn,
    pub repeat_info: wl_keyboard_repeat_info_fn,
}

// wl_pointer events
pub type wl_pointer_enter_fn = unsafe extern "C" fn(
    data: *mut c_void,
    pointer: *mut wl_proxy,
    serial: u32,
    surface: *mut wl_proxy,
    sx: i32, // wl_fixed_t (24.8)
    sy: i32,
);
pub type wl_pointer_leave_fn = unsafe extern "C" fn(
    data: *mut c_void,
    pointer: *mut wl_proxy,
    serial: u32,
    surface: *mut wl_proxy,
);
pub type wl_pointer_motion_fn = unsafe extern "C" fn(
    data: *mut c_void,
    pointer: *mut wl_proxy,
    time: u32,
    sx: i32,
    sy: i32,
);
pub type wl_pointer_button_fn = unsafe extern "C" fn(
    data: *mut c_void,
    pointer: *mut wl_proxy,
    serial: u32,
    time: u32,
    button: u32,
    state: u32,
);
pub type wl_pointer_axis_fn = unsafe extern "C" fn(
    data: *mut c_void,
    pointer: *mut wl_proxy,
    time: u32,
    axis: u32,
    value: i32,
);
pub type wl_pointer_frame_fn =
    unsafe extern "C" fn(data: *mut c_void, pointer: *mut wl_proxy);
pub type wl_pointer_axis_source_fn =
    unsafe extern "C" fn(data: *mut c_void, pointer: *mut wl_proxy, axis_source: u32);
pub type wl_pointer_axis_stop_fn =
    unsafe extern "C" fn(data: *mut c_void, pointer: *mut wl_proxy, time: u32, axis: u32);
pub type wl_pointer_axis_discrete_fn =
    unsafe extern "C" fn(data: *mut c_void, pointer: *mut wl_proxy, axis: u32, discrete: i32);

#[repr(C)]
pub struct wl_pointer_listener {
    pub enter: wl_pointer_enter_fn,
    pub leave: wl_pointer_leave_fn,
    pub motion: wl_pointer_motion_fn,
    pub button: wl_pointer_button_fn,
    pub axis: wl_pointer_axis_fn,
    pub frame: wl_pointer_frame_fn,
    pub axis_source: wl_pointer_axis_source_fn,
    pub axis_stop: wl_pointer_axis_stop_fn,
    pub axis_discrete: wl_pointer_axis_discrete_fn,
}

// wl_output events
pub type wl_output_geometry_fn = unsafe extern "C" fn(
    data: *mut c_void,
    output: *mut wl_proxy,
    x: i32,
    y: i32,
    physical_width: i32,
    physical_height: i32,
    subpixel: i32,
    make: *const c_char,
    model: *const c_char,
    transform: i32,
);
pub type wl_output_mode_fn = unsafe extern "C" fn(
    data: *mut c_void,
    output: *mut wl_proxy,
    flags: u32,
    width: i32,
    height: i32,
    refresh: i32,
);
pub type wl_output_done_fn =
    unsafe extern "C" fn(data: *mut c_void, output: *mut wl_proxy);
pub type wl_output_scale_fn =
    unsafe extern "C" fn(data: *mut c_void, output: *mut wl_proxy, factor: i32);

#[repr(C)]
pub struct wl_output_listener {
    pub geometry: wl_output_geometry_fn,
    pub mode: wl_output_mode_fn,
    pub done: wl_output_done_fn,
    pub scale: wl_output_scale_fn,
}

// xdg_wm_base events
pub type xdg_wm_base_ping_fn =
    unsafe extern "C" fn(data: *mut c_void, wm_base: *mut wl_proxy, serial: u32);

#[repr(C)]
pub struct xdg_wm_base_listener {
    pub ping: xdg_wm_base_ping_fn,
}

// xdg_surface events
pub type xdg_surface_configure_fn =
    unsafe extern "C" fn(data: *mut c_void, surface: *mut wl_proxy, serial: u32);

#[repr(C)]
pub struct xdg_surface_listener {
    pub configure: xdg_surface_configure_fn,
}

// xdg_toplevel events
pub type xdg_toplevel_configure_fn = unsafe extern "C" fn(
    data: *mut c_void,
    toplevel: *mut wl_proxy,
    width: i32,
    height: i32,
    states: *mut wl_array,
);
pub type xdg_toplevel_close_fn =
    unsafe extern "C" fn(data: *mut c_void, toplevel: *mut wl_proxy);

#[repr(C)]
pub struct xdg_toplevel_listener {
    pub configure: xdg_toplevel_configure_fn,
    pub close: xdg_toplevel_close_fn,
}

// wl_buffer events
pub type wl_buffer_release_fn =
    unsafe extern "C" fn(data: *mut c_void, buffer: *mut wl_proxy);

#[repr(C)]
pub struct wl_buffer_listener {
    pub release: wl_buffer_release_fn,
}

// wl_callback events
pub type wl_callback_done_fn =
    unsafe extern "C" fn(data: *mut c_void, callback: *mut wl_proxy, callback_data: u32);

#[repr(C)]
pub struct wl_callback_listener {
    pub done: wl_callback_done_fn,
}

// ── libwayland-client functions ─────────────────────────────────────────

#[link(name = "wayland-client")]
unsafe extern "C" {
    // Display
    pub fn wl_display_connect(name: *const c_char) -> *mut wl_display;
    pub fn wl_display_disconnect(display: *mut wl_display);
    pub fn wl_display_dispatch(display: *mut wl_display) -> c_int;
    pub fn wl_display_dispatch_pending(display: *mut wl_display) -> c_int;
    pub fn wl_display_roundtrip(display: *mut wl_display) -> c_int;
    pub fn wl_display_flush(display: *mut wl_display) -> c_int;
    pub fn wl_display_get_fd(display: *mut wl_display) -> c_int;

    // Proxy
    pub fn wl_proxy_marshal_flags(
        proxy: *mut wl_proxy,
        opcode: u32,
        interface: *const wl_interface,
        version: u32,
        flags: u32,
        ...
    ) -> *mut wl_proxy;
    pub fn wl_proxy_add_listener(
        proxy: *mut wl_proxy,
        listener: *mut c_void,
        data: *mut c_void,
    ) -> c_int;
    pub fn wl_proxy_get_version(proxy: *mut wl_proxy) -> u32;
    pub fn wl_proxy_destroy(proxy: *mut wl_proxy);
    pub fn wl_proxy_marshal(proxy: *mut wl_proxy, opcode: u32, ...);
    pub fn wl_proxy_set_tag(proxy: *mut wl_proxy, tag: *const *const c_char);
}

// ── Linux system calls for shared memory ────────────────────────────────

unsafe extern "C" {
    /// Create an anonymous file descriptor (Linux 3.17+).
    pub fn memfd_create(name: *const c_char, flags: c_int) -> c_int;

    /// Set file size.
    pub fn ftruncate(fd: c_int, length: i64) -> c_int;

    /// Memory-map a file descriptor.
    pub fn mmap(
        addr: *mut c_void,
        length: usize,
        prot: c_int,
        flags: c_int,
        fd: c_int,
        offset: i64,
    ) -> *mut c_void;

    /// Unmap memory.
    pub fn munmap(addr: *mut c_void, length: usize) -> c_int;

    /// Close a file descriptor.
    pub fn close(fd: c_int) -> c_int;

    /// Copy memory.
    pub fn memcpy(dest: *mut c_void, src: *const c_void, n: usize) -> *mut c_void;
}

// mmap prot flags
pub const PROT_READ: c_int = 0x1;
pub const PROT_WRITE: c_int = 0x2;

// mmap map flags
pub const MAP_SHARED: c_int = 0x01;

// mmap failure sentinel
pub const MAP_FAILED: *mut c_void = !0usize as *mut c_void;

// memfd_create flags
pub const MFD_CLOEXEC: c_int = 0x0001;

// ── Wayland fixed-point helpers ─────────────────────────────────────────

/// Convert a Wayland `wl_fixed_t` (24.8 fixed-point as i32) to `f32`.
#[inline]
pub fn wl_fixed_to_f32(fixed: i32) -> f32 {
    (fixed as f64 / 256.0) as f32
}
