//! Wayland platform backend — libwayland-client via raw FFI.
//!
//! Implements [`PlatformBackend`] for Wayland compositors using the
//! `wl_shm` shared-memory buffer mechanism for frame presentation and
//! the `xdg-shell` protocol for window management.
//!
//! This module is only compiled on Linux (`#[cfg(target_os = "linux")]`).

pub mod ffi;
pub mod input;

use std::collections::{HashMap, VecDeque};
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::ptr;

use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::PixelFormat;
use liquide_input::keyboard::{KeyCode, KeyEvent, KeyState, Modifiers};
use liquide_input::mouse::{ButtonState, MouseButton, MouseEvent, ScrollAxis};

use crate::display::{DisplayBackend, MonitorInfo};
use crate::dnd::{NativeDragDrop, NullDragDrop};
use crate::event_loop::PlatformEvent;
use crate::keymap::KeymapTranslator;
use crate::notifications::{NativeNotifications, NullNativeNotifications};
use crate::taskbar::{NullTaskbar, TaskbarIntegration};
use crate::tray::{NativeTray, NullNativeTray};
use crate::window_host::{NativeWindowHandle, NativeWindowHost, NativeWindowParams};
use crate::{PlatformBackend, PlatformError, PlatformResult};

use self::ffi::*;
use self::input::{linux_scancode_to_keycode, wayland_modifiers_to_modifiers};

// ── Per-window state ────────────────────────────────────────────────────

/// State tracked for each Wayland window (surface + xdg wrappers + buffers).
struct WaylandWindow {
    /// Our internal window handle.
    handle: NativeWindowHandle,
    /// The `wl_surface` proxy.
    wl_surface: *mut wl_proxy,
    /// The `xdg_surface` proxy.
    xdg_surface: *mut wl_proxy,
    /// The `xdg_toplevel` proxy.
    xdg_toplevel: *mut wl_proxy,
    /// Current width in pixels.
    width: u32,
    /// Current height in pixels.
    height: u32,
    /// Whether the initial configure has been received.
    configured: bool,
    /// SHM buffer currently attached (if any).
    buffer: Option<ShmBuffer>,
}

/// A shared-memory buffer for frame presentation.
struct ShmBuffer {
    /// The `wl_buffer` proxy.
    wl_buffer: *mut wl_proxy,
    /// The `wl_shm_pool` proxy.
    wl_shm_pool: *mut wl_proxy,
    /// Memory-mapped data pointer.
    data: *mut u8,
    /// Total byte size of the mapping.
    size: usize,
    /// File descriptor backing the pool.
    fd: c_int,
    /// Whether the compositor has released this buffer.
    released: bool,
}

/// Information collected from `wl_output` events.
#[derive(Debug, Clone)]
struct OutputInfo {
    /// Global name from the registry.
    name: u32,
    /// The `wl_output` proxy.
    proxy: *mut wl_proxy,
    /// Pixel X position.
    x: i32,
    /// Pixel Y position.
    y: i32,
    /// Mode width.
    width: i32,
    /// Mode height.
    height: i32,
    /// Refresh rate in millihertz.
    refresh_mhz: i32,
    /// Scale factor.
    scale: i32,
    /// Make string.
    make: String,
    /// Model string.
    model: String,
}

impl Default for OutputInfo {
    fn default() -> Self {
        Self {
            name: 0,
            proxy: ptr::null_mut(),
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            refresh_mhz: 0,
            scale: 1,
            make: String::new(),
            model: String::new(),
        }
    }
}

// ── Shared state passed through `data` pointers ─────────────────────────

/// Mutable state shared between the platform and all Wayland callbacks
/// via raw `*mut c_void` data pointers.
///
/// This struct is pinned on the heap via `Box` and its address is
/// handed to every `wl_proxy_add_listener` call.
struct WaylandState {
    // Bound globals (written by registry callback, read by platform)
    compositor: *mut wl_proxy,
    shm: *mut wl_proxy,
    seat: *mut wl_proxy,
    xdg_wm_base: *mut wl_proxy,
    keyboard: *mut wl_proxy,
    pointer: *mut wl_proxy,

    // Output tracking
    outputs: Vec<OutputInfo>,

    // Window tracking (keyed by wl_surface pointer for callback lookup)
    surface_to_handle: HashMap<usize, NativeWindowHandle>,
    windows: HashMap<u64, WaylandWindow>,
    next_handle: u64,

    // Event queue filled by callbacks, drained by poll/wait
    event_queue: VecDeque<PlatformEvent>,

    // Keyboard state
    current_modifiers: Modifiers,
    keyboard_focus_surface: *mut wl_proxy,

    // Pointer state
    pointer_x: f32,
    pointer_y: f32,
    pointer_focus_surface: *mut wl_proxy,

    // Display pointer (needed for xdg_wm_base pong and marshalers)
    display: *mut wl_display,
}

impl WaylandState {
    fn focused_window_handle(&self, surface: *mut wl_proxy) -> Option<NativeWindowHandle> {
        self.surface_to_handle.get(&(surface as usize)).copied()
    }
}

// ── Main platform struct ────────────────────────────────────────────────

/// Wayland platform backend.
///
/// Holds ownership of the display connection and all associated state.
/// The connection is single-threaded; `unsafe impl Send` is provided
/// because the entire struct is moved to whichever thread drives the
/// event loop.
pub struct WaylandPlatform {
    /// The connected `wl_display`.
    display: *mut wl_display,
    /// The `wl_registry` proxy.
    registry: *mut wl_proxy,
    /// Heap-allocated shared state (pointer-stable for callbacks).
    state: Box<WaylandState>,

    // Listener structs must live as long as the proxies they are attached to.
    // We box them so their addresses are stable.
    _registry_listener: Box<wl_registry_listener>,
    _seat_listener: Box<wl_seat_listener>,
    _keyboard_listener: Box<wl_keyboard_listener>,
    _pointer_listener: Box<wl_pointer_listener>,
    _xdg_wm_base_listener: Box<xdg_wm_base_listener>,

    // Null sub-backends for unsupported features
    null_taskbar: NullTaskbar,
    null_tray: NullNativeTray,
    null_notifications: NullNativeNotifications,
    null_drag_drop: NullDragDrop,
    display_backend: WaylandDisplayBackend,
    keymap_backend: WaylandKeymap,
}

// The Wayland display is used single-threaded; the struct is moved to
// the event-loop thread, but never shared concurrently.
unsafe impl Send for WaylandPlatform {}

impl WaylandPlatform {
    /// Connect to the Wayland display server and bind required globals.
    ///
    /// Returns `Err` if the connection cannot be established or if
    /// critical globals like `wl_compositor` or `xdg_wm_base` are missing.
    pub fn new() -> PlatformResult<Self> {
        unsafe {
            // Connect to the display (uses $WAYLAND_DISPLAY or "wayland-0").
            let display = wl_display_connect(ptr::null());
            if display.is_null() {
                return Err(PlatformError::Display(
                    "failed to connect to Wayland display".into(),
                ));
            }

            // Allocate shared state on the heap.
            let mut state = Box::new(WaylandState {
                compositor: ptr::null_mut(),
                shm: ptr::null_mut(),
                seat: ptr::null_mut(),
                xdg_wm_base: ptr::null_mut(),
                keyboard: ptr::null_mut(),
                pointer: ptr::null_mut(),
                outputs: Vec::new(),
                surface_to_handle: HashMap::new(),
                windows: HashMap::new(),
                next_handle: 1,
                event_queue: VecDeque::new(),
                current_modifiers: Modifiers::new(),
                keyboard_focus_surface: ptr::null_mut(),
                pointer_x: 0.0,
                pointer_y: 0.0,
                pointer_focus_surface: ptr::null_mut(),
                display,
            });

            // Get the registry.
            let registry = wl_proxy_marshal_flags(
                display,
                WL_DISPLAY_GET_REGISTRY,
                ptr::null(), // wl_registry has no static interface we need
                1,           // version
                0,           // flags
            );
            if registry.is_null() {
                wl_display_disconnect(display);
                return Err(PlatformError::Display(
                    "failed to get wl_registry".into(),
                ));
            }

            // Set up registry listener.
            let registry_listener = Box::new(wl_registry_listener {
                global: registry_global_handler,
                global_remove: registry_global_remove_handler,
            });
            let state_ptr: *mut c_void = &mut *state as *mut WaylandState as *mut c_void;
            wl_proxy_add_listener(
                registry,
                &*registry_listener as *const wl_registry_listener as *mut c_void,
                state_ptr,
            );

            // First roundtrip — discover globals.
            if wl_display_roundtrip(display) < 0 {
                wl_display_disconnect(display);
                return Err(PlatformError::Display(
                    "wl_display_roundtrip failed".into(),
                ));
            }

            // Verify required globals.
            if state.compositor.is_null() {
                wl_display_disconnect(display);
                return Err(PlatformError::Display(
                    "wl_compositor not available".into(),
                ));
            }
            if state.shm.is_null() {
                wl_display_disconnect(display);
                return Err(PlatformError::Display("wl_shm not available".into()));
            }
            if state.xdg_wm_base.is_null() {
                wl_display_disconnect(display);
                return Err(PlatformError::Display(
                    "xdg_wm_base not available".into(),
                ));
            }

            // Set up xdg_wm_base listener (for ping/pong).
            let xdg_wm_base_listener_box = Box::new(xdg_wm_base_listener {
                ping: xdg_wm_base_ping_handler,
            });
            wl_proxy_add_listener(
                state.xdg_wm_base,
                &*xdg_wm_base_listener_box as *const xdg_wm_base_listener as *mut c_void,
                state_ptr,
            );

            // Set up seat listener if a seat was bound.
            let seat_listener = Box::new(wl_seat_listener {
                capabilities: seat_capabilities_handler,
                name: seat_name_handler,
            });
            if !state.seat.is_null() {
                wl_proxy_add_listener(
                    state.seat,
                    &*seat_listener as *const wl_seat_listener as *mut c_void,
                    state_ptr,
                );
            }

            // Pre-allocate keyboard/pointer listeners (attached when caps arrive).
            let keyboard_listener = Box::new(wl_keyboard_listener {
                keymap: keyboard_keymap_handler,
                enter: keyboard_enter_handler,
                leave: keyboard_leave_handler,
                key: keyboard_key_handler,
                modifiers: keyboard_modifiers_handler,
                repeat_info: keyboard_repeat_info_handler,
            });
            let pointer_listener = Box::new(wl_pointer_listener {
                enter: pointer_enter_handler,
                leave: pointer_leave_handler,
                motion: pointer_motion_handler,
                button: pointer_button_handler,
                axis: pointer_axis_handler,
                frame: pointer_frame_handler,
                axis_source: pointer_axis_source_handler,
                axis_stop: pointer_axis_stop_handler,
                axis_discrete: pointer_axis_discrete_handler,
            });

            // Second roundtrip — receive seat capabilities and output info.
            wl_display_roundtrip(display);

            // If the seat emitted capabilities and bound keyboard/pointer,
            // attach listeners now.
            if !state.keyboard.is_null() {
                wl_proxy_add_listener(
                    state.keyboard,
                    &*keyboard_listener as *const wl_keyboard_listener as *mut c_void,
                    state_ptr,
                );
            }
            if !state.pointer.is_null() {
                wl_proxy_add_listener(
                    state.pointer,
                    &*pointer_listener as *const wl_pointer_listener as *mut c_void,
                    state_ptr,
                );
            }

            // Build the display backend snapshot from output info.
            let display_backend = WaylandDisplayBackend {
                monitors: state
                    .outputs
                    .iter()
                    .enumerate()
                    .map(|(i, o)| MonitorInfo {
                        id: o.name,
                        name: if o.model.is_empty() {
                            format!("output-{}", o.name)
                        } else {
                            o.model.clone()
                        },
                        geometry: Rect::new(
                            o.x as f32,
                            o.y as f32,
                            o.width as f32,
                            o.height as f32,
                        ),
                        work_area: Rect::new(
                            o.x as f32,
                            o.y as f32,
                            o.width as f32,
                            o.height as f32,
                        ),
                        dpi_scale: o.scale as f32,
                        primary: i == 0,
                        refresh_rate_hz: (o.refresh_mhz as u32).saturating_div(1000),
                    })
                    .collect(),
            };

            Ok(Self {
                display,
                registry,
                state,
                _registry_listener: registry_listener,
                _seat_listener: seat_listener,
                _keyboard_listener: keyboard_listener,
                _pointer_listener: pointer_listener,
                _xdg_wm_base_listener: xdg_wm_base_listener_box,
                null_taskbar: NullTaskbar,
                null_tray: NullNativeTray::new(),
                null_notifications: NullNativeNotifications::new(),
                null_drag_drop: NullDragDrop,
                display_backend,
                keymap_backend: WaylandKeymap,
            })
        }
    }

    /// Helper: create an SHM buffer of the given pixel dimensions.
    fn create_shm_buffer(
        shm: *mut wl_proxy,
        width: u32,
        height: u32,
    ) -> PlatformResult<ShmBuffer> {
        unsafe {
            let stride = width * 4; // 4 bytes per pixel (ARGB8888)
            let size = (stride * height) as usize;

            // Create an anonymous file with memfd_create.
            let name = CString::new("liquide-shm").unwrap();
            let fd = memfd_create(name.as_ptr(), MFD_CLOEXEC);
            if fd < 0 {
                return Err(PlatformError::Presentation(
                    "memfd_create failed".into(),
                ));
            }

            // Size the file.
            if ftruncate(fd, size as i64) < 0 {
                close(fd);
                return Err(PlatformError::Presentation("ftruncate failed".into()));
            }

            // Memory-map the file.
            let data = mmap(
                ptr::null_mut(),
                size,
                PROT_READ | PROT_WRITE,
                MAP_SHARED,
                fd,
                0,
            );
            if data == MAP_FAILED {
                close(fd);
                return Err(PlatformError::Presentation("mmap failed".into()));
            }

            // Create a wl_shm_pool.
            let pool = wl_proxy_marshal_flags(
                shm,
                WL_SHM_CREATE_POOL,
                ptr::null(),
                1,
                0,
                fd,
                size as i32,
            );
            if pool.is_null() {
                munmap(data, size);
                close(fd);
                return Err(PlatformError::Presentation(
                    "wl_shm.create_pool failed".into(),
                ));
            }

            // Create a wl_buffer from the pool.
            let buffer = wl_proxy_marshal_flags(
                pool,
                WL_SHM_POOL_CREATE_BUFFER,
                ptr::null(),
                1,
                0,
                0i32,             // offset
                width as i32,     // width
                height as i32,    // height
                stride as i32,    // stride
                WL_SHM_FORMAT_ARGB8888, // format
            );
            if buffer.is_null() {
                wl_proxy_marshal(pool, WL_SHM_POOL_DESTROY);
                wl_proxy_destroy(pool);
                munmap(data, size);
                close(fd);
                return Err(PlatformError::Presentation(
                    "wl_shm_pool.create_buffer failed".into(),
                ));
            }

            Ok(ShmBuffer {
                wl_buffer: buffer,
                wl_shm_pool: pool,
                data: data as *mut u8,
                size,
                fd,
                released: true,
            })
        }
    }

    /// Destroy an SHM buffer and release all associated resources.
    fn destroy_shm_buffer(buf: &ShmBuffer) {
        unsafe {
            wl_proxy_marshal(buf.wl_buffer, WL_BUFFER_DESTROY);
            wl_proxy_destroy(buf.wl_buffer);
            wl_proxy_marshal(buf.wl_shm_pool, WL_SHM_POOL_DESTROY);
            wl_proxy_destroy(buf.wl_shm_pool);
            munmap(buf.data as *mut c_void, buf.size);
            close(buf.fd);
        }
    }
}

impl Drop for WaylandPlatform {
    fn drop(&mut self) {
        unsafe {
            // Destroy all windows.
            let handles: Vec<u64> = self.state.windows.keys().copied().collect();
            for h in handles {
                if let Some(win) = self.state.windows.remove(&h) {
                    if let Some(ref buf) = win.buffer {
                        Self::destroy_shm_buffer(buf);
                    }
                    wl_proxy_marshal(win.xdg_toplevel, XDG_TOPLEVEL_DESTROY);
                    wl_proxy_destroy(win.xdg_toplevel);
                    wl_proxy_marshal(win.xdg_surface, XDG_SURFACE_DESTROY);
                    wl_proxy_destroy(win.xdg_surface);
                    wl_proxy_marshal(win.wl_surface, WL_SURFACE_DESTROY);
                    wl_proxy_destroy(win.wl_surface);
                }
            }

            // Destroy input devices.
            if !self.state.keyboard.is_null() {
                wl_proxy_destroy(self.state.keyboard);
            }
            if !self.state.pointer.is_null() {
                wl_proxy_destroy(self.state.pointer);
            }

            // Destroy bound globals.
            if !self.state.xdg_wm_base.is_null() {
                wl_proxy_destroy(self.state.xdg_wm_base);
            }
            if !self.state.seat.is_null() {
                wl_proxy_destroy(self.state.seat);
            }
            if !self.state.shm.is_null() {
                wl_proxy_destroy(self.state.shm);
            }
            if !self.state.compositor.is_null() {
                wl_proxy_destroy(self.state.compositor);
            }
            for o in &self.state.outputs {
                if !o.proxy.is_null() {
                    wl_proxy_destroy(o.proxy);
                }
            }

            // Destroy registry and disconnect.
            wl_proxy_destroy(self.registry);
            wl_display_disconnect(self.display);
        }
    }
}

// ── PlatformBackend implementation ──────────────────────────────────────

impl PlatformBackend for WaylandPlatform {
    fn display(&self) -> &dyn DisplayBackend {
        &self.display_backend
    }

    fn window_host(&mut self) -> &mut dyn NativeWindowHost {
        self
    }

    fn taskbar(&mut self) -> &mut dyn TaskbarIntegration {
        &mut self.null_taskbar
    }

    fn tray(&mut self) -> &mut dyn NativeTray {
        &mut self.null_tray
    }

    fn notifications(&mut self) -> &mut dyn NativeNotifications {
        &mut self.null_notifications
    }

    fn drag_drop(&mut self) -> &mut dyn NativeDragDrop {
        &mut self.null_drag_drop
    }

    fn keymap(&self) -> &dyn KeymapTranslator {
        &self.keymap_backend
    }

    fn platform_name(&self) -> &str {
        "wayland"
    }

    fn poll_event(&mut self) -> Option<PlatformEvent> {
        unsafe {
            // Flush outgoing requests and dispatch pending events without blocking.
            wl_display_flush(self.display);
            wl_display_dispatch_pending(self.display);
        }
        self.state.event_queue.pop_front()
    }

    fn wait_event(&mut self) -> PlatformEvent {
        unsafe {
            // Block until at least one event is dispatched.
            loop {
                wl_display_flush(self.display);
                if !self.state.event_queue.is_empty() {
                    return self.state.event_queue.pop_front().unwrap();
                }
                let ret = wl_display_dispatch(self.display);
                if ret < 0 {
                    return PlatformEvent::Quit;
                }
                if let Some(evt) = self.state.event_queue.pop_front() {
                    return evt;
                }
            }
        }
    }

    fn present_frame(
        &mut self,
        handle: NativeWindowHandle,
        pixels: &[u8],
        width: u32,
        height: u32,
        stride: u32,
        _format: PixelFormat,
    ) -> PlatformResult<()> {
        let shm = self.state.shm;
        let win = self
            .state
            .windows
            .get_mut(&handle.0)
            .ok_or_else(|| PlatformError::Presentation("unknown window handle".into()))?;

        // Re-create the buffer if the dimensions changed.
        let needs_new_buffer = match &win.buffer {
            Some(buf) => {
                let expected_size = (width * 4 * height) as usize;
                buf.size != expected_size
            }
            None => true,
        };

        if needs_new_buffer {
            if let Some(ref old) = win.buffer {
                Self::destroy_shm_buffer(old);
            }
            win.buffer = Some(Self::create_shm_buffer(shm, width, height)?);
        }

        let buf = win.buffer.as_mut().unwrap();

        // Copy pixel data into the SHM buffer.
        // Our BGRA8 format matches WL_SHM_FORMAT_ARGB8888 on little-endian.
        unsafe {
            let dst_stride = width * 4;
            let copy_bytes = std::cmp::min(stride, dst_stride) as usize;
            for row in 0..height as usize {
                let src_offset = row * stride as usize;
                let dst_offset = row * dst_stride as usize;
                if src_offset + copy_bytes <= pixels.len()
                    && dst_offset + copy_bytes <= buf.size
                {
                    memcpy(
                        buf.data.add(dst_offset) as *mut c_void,
                        pixels.as_ptr().add(src_offset) as *const c_void,
                        copy_bytes,
                    );
                }
            }

            // Attach buffer, damage, and commit.
            let surface = win.wl_surface;
            wl_proxy_marshal(surface, WL_SURFACE_ATTACH, buf.wl_buffer, 0i32, 0i32);
            wl_proxy_marshal(
                surface,
                WL_SURFACE_DAMAGE_BUFFER,
                0i32,
                0i32,
                width as i32,
                height as i32,
            );
            wl_proxy_marshal(surface, WL_SURFACE_COMMIT);
            wl_display_flush(self.display);
        }

        Ok(())
    }

    fn request_redraw(&mut self, handle: NativeWindowHandle) {
        if let Some(win) = self.state.windows.get(&handle.0) {
            unsafe {
                // Commit the surface to trigger a frame callback on the next
                // compositor repaint cycle.
                wl_proxy_marshal(win.wl_surface, WL_SURFACE_COMMIT);
                wl_display_flush(self.display);
            }
            self.state
                .event_queue
                .push_back(PlatformEvent::WindowRedraw { handle });
        }
    }
}

// ── NativeWindowHost implementation ─────────────────────────────────────

impl NativeWindowHost for WaylandPlatform {
    fn create_window(
        &mut self,
        params: NativeWindowParams,
    ) -> PlatformResult<NativeWindowHandle> {
        unsafe {
            let compositor = self.state.compositor;
            let xdg_wm_base = self.state.xdg_wm_base;
            let state_ptr: *mut c_void =
                &mut *self.state as *mut WaylandState as *mut c_void;

            // 1. Create wl_surface.
            let wl_surface = wl_proxy_marshal_flags(
                compositor,
                WL_COMPOSITOR_CREATE_SURFACE,
                ptr::null(),
                wl_proxy_get_version(compositor),
                0,
            );
            if wl_surface.is_null() {
                return Err(PlatformError::Window(
                    "wl_compositor.create_surface failed".into(),
                ));
            }

            // 2. Create xdg_surface.
            let xdg_surf = wl_proxy_marshal_flags(
                xdg_wm_base,
                XDG_WM_BASE_GET_XDG_SURFACE,
                ptr::null(),
                wl_proxy_get_version(xdg_wm_base),
                0,
                wl_surface,
            );
            if xdg_surf.is_null() {
                wl_proxy_marshal(wl_surface, WL_SURFACE_DESTROY);
                wl_proxy_destroy(wl_surface);
                return Err(PlatformError::Window(
                    "xdg_wm_base.get_xdg_surface failed".into(),
                ));
            }

            // 3. Create xdg_toplevel.
            let toplevel = wl_proxy_marshal_flags(
                xdg_surf,
                XDG_SURFACE_GET_TOPLEVEL,
                ptr::null(),
                wl_proxy_get_version(xdg_surf),
                0,
            );
            if toplevel.is_null() {
                wl_proxy_marshal(xdg_surf, XDG_SURFACE_DESTROY);
                wl_proxy_destroy(xdg_surf);
                wl_proxy_marshal(wl_surface, WL_SURFACE_DESTROY);
                wl_proxy_destroy(wl_surface);
                return Err(PlatformError::Window(
                    "xdg_surface.get_toplevel failed".into(),
                ));
            }

            // Assign a handle.
            let handle = NativeWindowHandle(self.state.next_handle);
            self.state.next_handle += 1;

            let width = if params.geometry.width > 0.0 {
                params.geometry.width as u32
            } else {
                800
            };
            let height = if params.geometry.height > 0.0 {
                params.geometry.height as u32
            } else {
                600
            };

            // Register the surface -> handle mapping.
            self.state
                .surface_to_handle
                .insert(wl_surface as usize, handle);

            let win = WaylandWindow {
                handle,
                wl_surface,
                xdg_surface: xdg_surf,
                xdg_toplevel: toplevel,
                width,
                height,
                configured: false,
                buffer: None,
            };
            self.state.windows.insert(handle.0, win);

            // Set up xdg_surface listener.
            // We allocate these on the heap so they outlive this function.
            let xdg_surface_listener_box = Box::new(xdg_surface_listener {
                configure: xdg_surface_configure_handler,
            });
            wl_proxy_add_listener(
                xdg_surf,
                Box::into_raw(xdg_surface_listener_box) as *mut c_void,
                state_ptr,
            );

            // Set up xdg_toplevel listener.
            let xdg_toplevel_listener_box = Box::new(xdg_toplevel_listener {
                configure: xdg_toplevel_configure_handler,
                close: xdg_toplevel_close_handler,
            });
            wl_proxy_add_listener(
                toplevel,
                Box::into_raw(xdg_toplevel_listener_box) as *mut c_void,
                state_ptr,
            );

            // Set title and app_id.
            let title_c = CString::new(params.title.as_str()).unwrap_or_default();
            wl_proxy_marshal(toplevel, XDG_TOPLEVEL_SET_TITLE, title_c.as_ptr());
            let app_id_c = CString::new(params.app_id.as_str()).unwrap_or_default();
            wl_proxy_marshal(toplevel, XDG_TOPLEVEL_SET_APP_ID, app_id_c.as_ptr());

            // Commit the surface so the compositor sends the initial configure.
            wl_proxy_marshal(wl_surface, WL_SURFACE_COMMIT);
            wl_display_flush(self.display);

            // Enqueue the creation event.
            self.state.event_queue.push_back(PlatformEvent::WindowCreated {
                handle,
                width,
                height,
            });

            Ok(handle)
        }
    }

    fn destroy_window(&mut self, handle: NativeWindowHandle) -> PlatformResult<()> {
        if let Some(win) = self.state.windows.remove(&handle.0) {
            self.state
                .surface_to_handle
                .remove(&(win.wl_surface as usize));

            if let Some(ref buf) = win.buffer {
                Self::destroy_shm_buffer(buf);
            }

            unsafe {
                wl_proxy_marshal(win.xdg_toplevel, XDG_TOPLEVEL_DESTROY);
                wl_proxy_destroy(win.xdg_toplevel);
                wl_proxy_marshal(win.xdg_surface, XDG_SURFACE_DESTROY);
                wl_proxy_destroy(win.xdg_surface);
                wl_proxy_marshal(win.wl_surface, WL_SURFACE_DESTROY);
                wl_proxy_destroy(win.wl_surface);
                wl_display_flush(self.display);
            }

            self.state
                .event_queue
                .push_back(PlatformEvent::WindowDestroyed { handle });
        }
        Ok(())
    }

    fn set_geometry(
        &mut self,
        _handle: NativeWindowHandle,
        _geometry: Rect,
    ) -> PlatformResult<()> {
        // Wayland does not allow clients to set their own position.
        // Size changes are typically driven by the compositor via configure events.
        Ok(())
    }

    fn set_title(
        &mut self,
        handle: NativeWindowHandle,
        title: &str,
    ) -> PlatformResult<()> {
        if let Some(win) = self.state.windows.get(&handle.0) {
            let title_c = CString::new(title).unwrap_or_default();
            unsafe {
                wl_proxy_marshal(
                    win.xdg_toplevel,
                    XDG_TOPLEVEL_SET_TITLE,
                    title_c.as_ptr(),
                );
                wl_display_flush(self.display);
            }
        }
        Ok(())
    }

    fn set_icon(
        &mut self,
        _handle: NativeWindowHandle,
        _icon_data: &[u8],
    ) -> PlatformResult<()> {
        // Wayland has no standard protocol for window icons.
        Ok(())
    }

    fn set_state(
        &mut self,
        handle: NativeWindowHandle,
        state: &str,
    ) -> PlatformResult<()> {
        if let Some(win) = self.state.windows.get(&handle.0) {
            unsafe {
                match state {
                    "maximized" => {
                        wl_proxy_marshal(win.xdg_toplevel, XDG_TOPLEVEL_SET_MAXIMIZED);
                    }
                    "minimized" => {
                        wl_proxy_marshal(win.xdg_toplevel, XDG_TOPLEVEL_SET_MINIMIZED);
                    }
                    "fullscreen" => {
                        wl_proxy_marshal(
                            win.xdg_toplevel,
                            XDG_TOPLEVEL_SET_FULLSCREEN,
                            ptr::null_mut::<c_void>(),
                        );
                    }
                    "normal" | "restored" => {
                        wl_proxy_marshal(win.xdg_toplevel, XDG_TOPLEVEL_UNSET_MAXIMIZED);
                        wl_proxy_marshal(win.xdg_toplevel, XDG_TOPLEVEL_UNSET_FULLSCREEN);
                    }
                    _ => {}
                }
                wl_display_flush(self.display);
            }
        }
        Ok(())
    }

    fn set_z_order(
        &mut self,
        _handle: NativeWindowHandle,
        _z_order: i32,
    ) -> PlatformResult<()> {
        // Wayland does not expose Z-order to clients.
        Ok(())
    }

    fn set_focus(&mut self, _handle: NativeWindowHandle) -> PlatformResult<()> {
        // Wayland does not allow clients to steal focus.
        Ok(())
    }
}

// ── Display backend ─────────────────────────────────────────────────────

/// Wayland display backend returning monitor info gathered from `wl_output`.
struct WaylandDisplayBackend {
    monitors: Vec<MonitorInfo>,
}

impl DisplayBackend for WaylandDisplayBackend {
    fn monitors(&self) -> Vec<MonitorInfo> {
        self.monitors.clone()
    }

    fn primary_monitor(&self) -> Option<MonitorInfo> {
        self.monitors.first().cloned()
    }

    fn virtual_screen_rect(&self) -> Rect {
        if self.monitors.is_empty() {
            return Rect::ZERO;
        }
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        for m in &self.monitors {
            min_x = min_x.min(m.geometry.x);
            min_y = min_y.min(m.geometry.y);
            max_x = max_x.max(m.geometry.x + m.geometry.width);
            max_y = max_y.max(m.geometry.y + m.geometry.height);
        }
        Rect::new(min_x, min_y, max_x - min_x, max_y - min_y)
    }
}

// SAFETY: The display backend holds only owned data.
unsafe impl Send for WaylandDisplayBackend {}

// ── Keymap translator ───────────────────────────────────────────────────

/// Wayland keymap translator using Linux scancodes.
struct WaylandKeymap;

impl KeymapTranslator for WaylandKeymap {
    fn translate_scancode(&self, scancode: u32) -> Option<KeyCode> {
        linux_scancode_to_keycode(scancode)
    }

    fn platform_name(&self) -> &str {
        "wayland"
    }
}

// SAFETY: Stateless.
unsafe impl Send for WaylandKeymap {}

// ── Wayland callback handlers ───────────────────────────────────────────
//
// These are `unsafe extern "C"` functions whose `data` parameter points
// to our `WaylandState`.  They are invoked by libwayland-client during
// `wl_display_dispatch*`.

/// `wl_registry::global` — a new global object appeared.
unsafe extern "C" fn registry_global_handler(
    data: *mut c_void,
    registry: *mut wl_proxy,
    name: u32,
    interface: *const c_char,
    version: u32,
) {
    let state = &mut *(data as *mut WaylandState);
    let iface = CStr::from_ptr(interface);

    match iface.to_bytes() {
        b"wl_compositor" => {
            state.compositor = wl_proxy_marshal_flags(
                registry,
                WL_REGISTRY_BIND,
                ptr::null(),
                version.min(5),
                0,
                name,
                interface,
                version.min(5) as c_int,
                ptr::null_mut::<c_void>(),
            );
        }
        b"wl_shm" => {
            state.shm = wl_proxy_marshal_flags(
                registry,
                WL_REGISTRY_BIND,
                ptr::null(),
                version.min(1),
                0,
                name,
                interface,
                version.min(1) as c_int,
                ptr::null_mut::<c_void>(),
            );
        }
        b"xdg_wm_base" => {
            state.xdg_wm_base = wl_proxy_marshal_flags(
                registry,
                WL_REGISTRY_BIND,
                ptr::null(),
                version.min(3),
                0,
                name,
                interface,
                version.min(3) as c_int,
                ptr::null_mut::<c_void>(),
            );
        }
        b"wl_seat" => {
            if state.seat.is_null() {
                state.seat = wl_proxy_marshal_flags(
                    registry,
                    WL_REGISTRY_BIND,
                    ptr::null(),
                    version.min(5),
                    0,
                    name,
                    interface,
                    version.min(5) as c_int,
                    ptr::null_mut::<c_void>(),
                );
            }
        }
        b"wl_output" => {
            let proxy = wl_proxy_marshal_flags(
                registry,
                WL_REGISTRY_BIND,
                ptr::null(),
                version.min(3),
                0,
                name,
                interface,
                version.min(3) as c_int,
                ptr::null_mut::<c_void>(),
            );
            if !proxy.is_null() {
                let mut info = OutputInfo::default();
                info.name = name;
                info.proxy = proxy;
                state.outputs.push(info);

                // Attach output listener.
                let listener = Box::new(wl_output_listener {
                    geometry: output_geometry_handler,
                    mode: output_mode_handler,
                    done: output_done_handler,
                    scale: output_scale_handler,
                });
                wl_proxy_add_listener(
                    proxy,
                    Box::into_raw(listener) as *mut c_void,
                    data,
                );
            }
        }
        _ => {}
    }
}

/// `wl_registry::global_remove` — a global was removed.
unsafe extern "C" fn registry_global_remove_handler(
    _data: *mut c_void,
    _registry: *mut wl_proxy,
    _name: u32,
) {
    // We could remove outputs here, but for simplicity we keep the
    // snapshot taken at startup.
}

/// `wl_seat::capabilities` — input device capabilities changed.
unsafe extern "C" fn seat_capabilities_handler(
    data: *mut c_void,
    seat: *mut wl_proxy,
    capabilities: u32,
) {
    let state = &mut *(data as *mut WaylandState);

    // Keyboard.
    if capabilities & WL_SEAT_CAPABILITY_KEYBOARD != 0 && state.keyboard.is_null() {
        state.keyboard = wl_proxy_marshal_flags(
            seat,
            WL_SEAT_GET_KEYBOARD,
            ptr::null(),
            wl_proxy_get_version(seat),
            0,
        );
    }

    // Pointer.
    if capabilities & WL_SEAT_CAPABILITY_POINTER != 0 && state.pointer.is_null() {
        state.pointer = wl_proxy_marshal_flags(
            seat,
            WL_SEAT_GET_POINTER,
            ptr::null(),
            wl_proxy_get_version(seat),
            0,
        );
    }
}

/// `wl_seat::name` — seat name (informational).
unsafe extern "C" fn seat_name_handler(
    _data: *mut c_void,
    _seat: *mut wl_proxy,
    _name: *const c_char,
) {
    // Informational only; ignored.
}

// ── Keyboard callbacks ──────────────────────────────────────────────────

unsafe extern "C" fn keyboard_keymap_handler(
    _data: *mut c_void,
    _keyboard: *mut wl_proxy,
    _format: u32,
    fd: i32,
    _size: u32,
) {
    // We receive the keymap as an fd.  For our scancode-based approach
    // we do not need to parse it; just close the fd.
    close(fd);
}

unsafe extern "C" fn keyboard_enter_handler(
    data: *mut c_void,
    _keyboard: *mut wl_proxy,
    _serial: u32,
    surface: *mut wl_proxy,
    _keys: *mut wl_array,
) {
    let state = &mut *(data as *mut WaylandState);
    state.keyboard_focus_surface = surface;
    if let Some(handle) = state.focused_window_handle(surface) {
        state
            .event_queue
            .push_back(PlatformEvent::FocusGained { handle });
    }
}

unsafe extern "C" fn keyboard_leave_handler(
    data: *mut c_void,
    _keyboard: *mut wl_proxy,
    _serial: u32,
    surface: *mut wl_proxy,
) {
    let state = &mut *(data as *mut WaylandState);
    if let Some(handle) = state.focused_window_handle(surface) {
        state
            .event_queue
            .push_back(PlatformEvent::FocusLost { handle });
    }
    state.keyboard_focus_surface = ptr::null_mut();
}

unsafe extern "C" fn keyboard_key_handler(
    data: *mut c_void,
    _keyboard: *mut wl_proxy,
    _serial: u32,
    time: u32,
    key: u32,
    key_state: u32,
) {
    let state = &mut *(data as *mut WaylandState);
    let surface = state.keyboard_focus_surface;
    if surface.is_null() {
        return;
    }
    let handle = match state.focused_window_handle(surface) {
        Some(h) => h,
        None => return,
    };

    let ks = if key_state == WL_KEYBOARD_KEY_STATE_PRESSED {
        KeyState::Pressed
    } else {
        KeyState::Released
    };

    // Linux scancodes in the key event are offset by 8 relative to
    // input-event-codes.h when coming from XKB.  However, Wayland's
    // wl_keyboard::key delivers the raw evdev scancode directly (not
    // XKB keycode), so we use it as-is.
    if let Some(key_code) = linux_scancode_to_keycode(key) {
        let event = KeyEvent::new(
            key_code,
            ks,
            state.current_modifiers,
            key,
            time as u64 * 1000, // ms -> us
        );
        state
            .event_queue
            .push_back(PlatformEvent::KeyInput { handle, event });
    }
}

unsafe extern "C" fn keyboard_modifiers_handler(
    data: *mut c_void,
    _keyboard: *mut wl_proxy,
    _serial: u32,
    mods_depressed: u32,
    _mods_latched: u32,
    mods_locked: u32,
    _group: u32,
) {
    let state = &mut *(data as *mut WaylandState);
    state.current_modifiers = wayland_modifiers_to_modifiers(mods_depressed, mods_locked);
}

unsafe extern "C" fn keyboard_repeat_info_handler(
    _data: *mut c_void,
    _keyboard: *mut wl_proxy,
    _rate: i32,
    _delay: i32,
) {
    // Key repeat is handled by the compositor; we could implement
    // client-side repeat here if needed.
}

// ── Pointer callbacks ───────────────────────────────────────────────────

unsafe extern "C" fn pointer_enter_handler(
    data: *mut c_void,
    _pointer: *mut wl_proxy,
    _serial: u32,
    surface: *mut wl_proxy,
    sx: i32,
    sy: i32,
) {
    let state = &mut *(data as *mut WaylandState);
    state.pointer_focus_surface = surface;
    state.pointer_x = wl_fixed_to_f32(sx);
    state.pointer_y = wl_fixed_to_f32(sy);
    if let Some(handle) = state.focused_window_handle(surface) {
        state.event_queue.push_back(PlatformEvent::MouseInput {
            handle,
            event: MouseEvent::Enter {
                x: state.pointer_x,
                y: state.pointer_y,
            },
        });
    }
}

unsafe extern "C" fn pointer_leave_handler(
    data: *mut c_void,
    _pointer: *mut wl_proxy,
    _serial: u32,
    surface: *mut wl_proxy,
) {
    let state = &mut *(data as *mut WaylandState);
    if let Some(handle) = state.focused_window_handle(surface) {
        state.event_queue.push_back(PlatformEvent::MouseInput {
            handle,
            event: MouseEvent::Leave,
        });
    }
    state.pointer_focus_surface = ptr::null_mut();
}

unsafe extern "C" fn pointer_motion_handler(
    data: *mut c_void,
    _pointer: *mut wl_proxy,
    _time: u32,
    sx: i32,
    sy: i32,
) {
    let state = &mut *(data as *mut WaylandState);
    state.pointer_x = wl_fixed_to_f32(sx);
    state.pointer_y = wl_fixed_to_f32(sy);
    let surface = state.pointer_focus_surface;
    if let Some(handle) = state.focused_window_handle(surface) {
        state.event_queue.push_back(PlatformEvent::MouseInput {
            handle,
            event: MouseEvent::Move {
                x: state.pointer_x,
                y: state.pointer_y,
            },
        });
    }
}

unsafe extern "C" fn pointer_button_handler(
    data: *mut c_void,
    _pointer: *mut wl_proxy,
    _serial: u32,
    _time: u32,
    button: u32,
    btn_state: u32,
) {
    let state = &mut *(data as *mut WaylandState);
    let surface = state.pointer_focus_surface;
    let handle = match state.focused_window_handle(surface) {
        Some(h) => h,
        None => return,
    };

    let mouse_button = match button {
        BTN_LEFT => MouseButton::Left,
        BTN_RIGHT => MouseButton::Right,
        BTN_MIDDLE => MouseButton::Middle,
        BTN_SIDE => MouseButton::Back,
        BTN_EXTRA => MouseButton::Forward,
        other => MouseButton::Other((other & 0xFF) as u8),
    };

    let button_state = if btn_state == 1 {
        ButtonState::Pressed
    } else {
        ButtonState::Released
    };

    state.event_queue.push_back(PlatformEvent::MouseInput {
        handle,
        event: MouseEvent::Button {
            button: mouse_button,
            state: button_state,
            x: state.pointer_x,
            y: state.pointer_y,
        },
    });
}

unsafe extern "C" fn pointer_axis_handler(
    data: *mut c_void,
    _pointer: *mut wl_proxy,
    _time: u32,
    axis: u32,
    value: i32,
) {
    let state = &mut *(data as *mut WaylandState);
    let surface = state.pointer_focus_surface;
    let handle = match state.focused_window_handle(surface) {
        Some(h) => h,
        None => return,
    };

    let scroll_axis = if axis == WL_POINTER_AXIS_HORIZONTAL_SCROLL {
        ScrollAxis::Horizontal
    } else {
        ScrollAxis::Vertical
    };

    // wl_fixed_t value: positive = scroll down/right.
    let delta = wl_fixed_to_f32(value);

    state.event_queue.push_back(PlatformEvent::MouseInput {
        handle,
        event: MouseEvent::Scroll {
            axis: scroll_axis,
            delta,
            x: state.pointer_x,
            y: state.pointer_y,
        },
    });
}

unsafe extern "C" fn pointer_frame_handler(
    _data: *mut c_void,
    _pointer: *mut wl_proxy,
) {
    // Frame boundary — we emit events individually above.
}

unsafe extern "C" fn pointer_axis_source_handler(
    _data: *mut c_void,
    _pointer: *mut wl_proxy,
    _axis_source: u32,
) {
}

unsafe extern "C" fn pointer_axis_stop_handler(
    _data: *mut c_void,
    _pointer: *mut wl_proxy,
    _time: u32,
    _axis: u32,
) {
}

unsafe extern "C" fn pointer_axis_discrete_handler(
    _data: *mut c_void,
    _pointer: *mut wl_proxy,
    _axis: u32,
    _discrete: i32,
) {
}

// ── Output callbacks ────────────────────────────────────────────────────

unsafe extern "C" fn output_geometry_handler(
    data: *mut c_void,
    output: *mut wl_proxy,
    x: i32,
    y: i32,
    _physical_width: i32,
    _physical_height: i32,
    _subpixel: i32,
    make: *const c_char,
    model: *const c_char,
    _transform: i32,
) {
    let state = &mut *(data as *mut WaylandState);
    if let Some(info) = state
        .outputs
        .iter_mut()
        .find(|o| o.proxy == output)
    {
        info.x = x;
        info.y = y;
        if !make.is_null() {
            info.make = CStr::from_ptr(make)
                .to_string_lossy()
                .into_owned();
        }
        if !model.is_null() {
            info.model = CStr::from_ptr(model)
                .to_string_lossy()
                .into_owned();
        }
    }
}

unsafe extern "C" fn output_mode_handler(
    data: *mut c_void,
    output: *mut wl_proxy,
    _flags: u32,
    width: i32,
    height: i32,
    refresh: i32,
) {
    let state = &mut *(data as *mut WaylandState);
    if let Some(info) = state
        .outputs
        .iter_mut()
        .find(|o| o.proxy == output)
    {
        info.width = width;
        info.height = height;
        info.refresh_mhz = refresh;
    }
}

unsafe extern "C" fn output_done_handler(
    _data: *mut c_void,
    _output: *mut wl_proxy,
) {
    // All output properties have been sent.
}

unsafe extern "C" fn output_scale_handler(
    data: *mut c_void,
    output: *mut wl_proxy,
    factor: i32,
) {
    let state = &mut *(data as *mut WaylandState);
    if let Some(info) = state
        .outputs
        .iter_mut()
        .find(|o| o.proxy == output)
    {
        info.scale = factor;
    }
}

// ── XDG shell callbacks ────────────────────────────────────────────────

/// `xdg_wm_base::ping` — compositor liveness check; must respond with pong.
unsafe extern "C" fn xdg_wm_base_ping_handler(
    data: *mut c_void,
    wm_base: *mut wl_proxy,
    serial: u32,
) {
    let state = &*(data as *mut WaylandState);
    wl_proxy_marshal(wm_base, XDG_WM_BASE_PONG, serial);
    wl_display_flush(state.display);
}

/// `xdg_surface::configure` — the compositor wants us to acknowledge a
/// configuration sequence.
unsafe extern "C" fn xdg_surface_configure_handler(
    data: *mut c_void,
    xdg_surf: *mut wl_proxy,
    serial: u32,
) {
    let state = &mut *(data as *mut WaylandState);

    // Acknowledge the configure.
    wl_proxy_marshal(xdg_surf, XDG_SURFACE_ACK_CONFIGURE, serial);

    // Find the window by xdg_surface pointer.
    if let Some(win) = state
        .windows
        .values_mut()
        .find(|w| w.xdg_surface == xdg_surf)
    {
        if !win.configured {
            win.configured = true;
            // Commit the surface after the first configure.
            wl_proxy_marshal(win.wl_surface, WL_SURFACE_COMMIT);

            state.event_queue.push_back(PlatformEvent::WindowRedraw {
                handle: win.handle,
            });
        }
    }

    wl_display_flush(state.display);
}

/// `xdg_toplevel::configure` — compositor suggests a new size.
unsafe extern "C" fn xdg_toplevel_configure_handler(
    data: *mut c_void,
    toplevel: *mut wl_proxy,
    width: i32,
    height: i32,
    _states: *mut wl_array,
) {
    let state = &mut *(data as *mut WaylandState);

    // width/height of 0 means the client can pick its own size.
    if width <= 0 || height <= 0 {
        return;
    }

    if let Some(win) = state
        .windows
        .values_mut()
        .find(|w| w.xdg_toplevel == toplevel)
    {
        let new_w = width as u32;
        let new_h = height as u32;
        if win.width != new_w || win.height != new_h {
            win.width = new_w;
            win.height = new_h;
            state.event_queue.push_back(PlatformEvent::WindowResized {
                handle: win.handle,
                width: new_w,
                height: new_h,
            });
        }
    }
}

/// `xdg_toplevel::close` — the user asked to close the window.
unsafe extern "C" fn xdg_toplevel_close_handler(
    data: *mut c_void,
    toplevel: *mut wl_proxy,
) {
    let state = &mut *(data as *mut WaylandState);
    if let Some(win) = state
        .windows
        .values()
        .find(|w| w.xdg_toplevel == toplevel)
    {
        state
            .event_queue
            .push_back(PlatformEvent::WindowCloseRequested {
                handle: win.handle,
            });
    }
}
