//! Raw X11 / Xlib FFI type definitions and extern declarations.
//!
//! This module defines the low-level C types and function signatures needed
//! to interact with the X Window System via `libX11` and `libXrandr`.
//! Only compiled on Linux (`#[cfg(target_os = "linux")]`).

#![allow(non_camel_case_types, non_upper_case_globals, dead_code, clippy::upper_case_acronyms)]

use std::ffi::c_void;
use std::os::raw::{c_char, c_int, c_long, c_uint, c_ulong};

// ── Core X11 types ──────────────────────────────────────────────────

/// Opaque display connection.  Always used as `*mut Display`.
pub type Display = c_void;

/// X window identifier.
pub type Window = c_ulong;

/// Pixmap identifier.
pub type Pixmap = c_ulong;

/// Graphics context identifier.
pub type GC = *mut c_void;

/// Colormap identifier.
pub type Colormap = c_ulong;

/// Atom (interned string identifier).
pub type Atom = c_ulong;

/// Cursor identifier.
pub type Cursor = c_ulong;

/// Keysym value (logical key symbol).
pub type KeySym = c_ulong;

/// Generic X resource identifier.
pub type XID = c_ulong;

/// Timestamp in server time (milliseconds).
pub type Time = c_ulong;

/// Xlib boolean (int-sized).
pub type Bool = c_int;

/// Status return code.
pub type Status = c_int;

// ── Visual / Screen / Depth ─────────────────────────────────────────

/// Visual description (opaque in practice).
#[repr(C)]
pub struct Visual {
    _private: [u8; 0],
}

/// Per-depth information.
#[repr(C)]
pub struct Depth {
    pub depth: c_int,
    pub nvisuals: c_int,
    pub visuals: *mut Visual,
}

/// Per-screen information.
#[repr(C)]
pub struct Screen {
    pub ext_data: *mut c_void,
    pub display: *mut Display,
    pub root: Window,
    pub width: c_int,
    pub height: c_int,
    pub mwidth: c_int,
    pub mheight: c_int,
    pub ndepths: c_int,
    pub depths: *mut Depth,
    pub root_depth: c_int,
    pub root_visual: *mut Visual,
    pub default_gc: GC,
    pub cmap: Colormap,
    pub white_pixel: c_ulong,
    pub black_pixel: c_ulong,
    pub max_maps: c_int,
    pub min_maps: c_int,
    pub backing_store: c_int,
    pub save_unders: Bool,
    pub root_input_mask: c_long,
}

// ── XImage ──────────────────────────────────────────────────────────

/// Image structure used by XCreateImage / XPutImage.
#[repr(C)]
pub struct XImage {
    pub width: c_int,
    pub height: c_int,
    pub xoffset: c_int,
    pub format: c_int,
    pub data: *mut c_char,
    pub byte_order: c_int,
    pub bitmap_unit: c_int,
    pub bitmap_bit_order: c_int,
    pub bitmap_pad: c_int,
    pub depth: c_int,
    pub bytes_per_line: c_int,
    pub bits_per_pixel: c_int,
    pub red_mask: c_ulong,
    pub green_mask: c_ulong,
    pub blue_mask: c_ulong,
    pub obdata: *mut c_char,
    // Function pointers — we use a padding block because their exact
    // layout is not needed (we never call them directly).
    pub funcs: XImageFuncs,
}

/// Internal function-pointer table inside XImage.
#[repr(C)]
pub struct XImageFuncs {
    pub create_image: *mut c_void,
    pub destroy_image: *mut c_void,
    pub get_pixel: *mut c_void,
    pub put_pixel: *mut c_void,
    pub sub_image: *mut c_void,
    pub add_pixel: *mut c_void,
}

// ── Event structures ────────────────────────────────────────────────

/// Key event (KeyPress / KeyRelease).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct XKeyEvent {
    pub type_: c_int,
    pub serial: c_ulong,
    pub send_event: Bool,
    pub display: *mut Display,
    pub window: Window,
    pub root: Window,
    pub subwindow: Window,
    pub time: Time,
    pub x: c_int,
    pub y: c_int,
    pub x_root: c_int,
    pub y_root: c_int,
    pub state: c_uint,
    pub keycode: c_uint,
    pub same_screen: Bool,
}

/// Button event (ButtonPress / ButtonRelease).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct XButtonEvent {
    pub type_: c_int,
    pub serial: c_ulong,
    pub send_event: Bool,
    pub display: *mut Display,
    pub window: Window,
    pub root: Window,
    pub subwindow: Window,
    pub time: Time,
    pub x: c_int,
    pub y: c_int,
    pub x_root: c_int,
    pub y_root: c_int,
    pub state: c_uint,
    pub button: c_uint,
    pub same_screen: Bool,
}

/// Motion (pointer movement) event.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct XMotionEvent {
    pub type_: c_int,
    pub serial: c_ulong,
    pub send_event: Bool,
    pub display: *mut Display,
    pub window: Window,
    pub root: Window,
    pub subwindow: Window,
    pub time: Time,
    pub x: c_int,
    pub y: c_int,
    pub x_root: c_int,
    pub y_root: c_int,
    pub state: c_uint,
    pub is_hint: c_char,
    pub same_screen: Bool,
}

/// Crossing event (EnterNotify / LeaveNotify).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct XCrossingEvent {
    pub type_: c_int,
    pub serial: c_ulong,
    pub send_event: Bool,
    pub display: *mut Display,
    pub window: Window,
    pub root: Window,
    pub subwindow: Window,
    pub time: Time,
    pub x: c_int,
    pub y: c_int,
    pub x_root: c_int,
    pub y_root: c_int,
    pub mode: c_int,
    pub detail: c_int,
    pub same_screen: Bool,
    pub focus: Bool,
    pub state: c_uint,
}

/// Expose event.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct XExposeEvent {
    pub type_: c_int,
    pub serial: c_ulong,
    pub send_event: Bool,
    pub display: *mut Display,
    pub window: Window,
    pub x: c_int,
    pub y: c_int,
    pub width: c_int,
    pub height: c_int,
    pub count: c_int,
}

/// ConfigureNotify event.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct XConfigureEvent {
    pub type_: c_int,
    pub serial: c_ulong,
    pub send_event: Bool,
    pub display: *mut Display,
    pub event: Window,
    pub window: Window,
    pub x: c_int,
    pub y: c_int,
    pub width: c_int,
    pub height: c_int,
    pub border_width: c_int,
    pub above: Window,
    pub override_redirect: Bool,
}

/// Focus change event (FocusIn / FocusOut).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct XFocusChangeEvent {
    pub type_: c_int,
    pub serial: c_ulong,
    pub send_event: Bool,
    pub display: *mut Display,
    pub window: Window,
    pub mode: c_int,
    pub detail: c_int,
}

/// ClientMessage event.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct XClientMessageEvent {
    pub type_: c_int,
    pub serial: c_ulong,
    pub send_event: Bool,
    pub display: *mut Display,
    pub window: Window,
    pub message_type: Atom,
    pub format: c_int,
    pub data: ClientMessageData,
}

/// Data payload for XClientMessageEvent (union in C).
#[repr(C)]
#[derive(Clone, Copy)]
pub union ClientMessageData {
    pub b: [c_char; 20],
    pub s: [c_short; 10],
    pub l: [c_long; 5],
}

/// Short type alias used in the union above.
type c_short = i16;

/// PropertyNotify event.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct XPropertyEvent {
    pub type_: c_int,
    pub serial: c_ulong,
    pub send_event: Bool,
    pub display: *mut Display,
    pub window: Window,
    pub atom: Atom,
    pub time: Time,
    pub state: c_int,
}

/// MapNotify event.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct XMapEvent {
    pub type_: c_int,
    pub serial: c_ulong,
    pub send_event: Bool,
    pub display: *mut Display,
    pub event: Window,
    pub window: Window,
    pub override_redirect: Bool,
}

/// UnmapNotify event.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct XUnmapEvent {
    pub type_: c_int,
    pub serial: c_ulong,
    pub send_event: Bool,
    pub display: *mut Display,
    pub event: Window,
    pub window: Window,
    pub from_configure: Bool,
}

/// DestroyNotify event.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct XDestroyWindowEvent {
    pub type_: c_int,
    pub serial: c_ulong,
    pub send_event: Bool,
    pub display: *mut Display,
    pub event: Window,
    pub window: Window,
}

/// Generic event union — large enough to hold any X event.
///
/// Individual event types are accessed via the typed accessor methods
/// which reinterpret the raw bytes.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct XEvent {
    pub pad: [u8; 192],
}

impl XEvent {
    /// Get the event type (first c_int field of every event struct).
    pub fn event_type(&self) -> c_int {
        // SAFETY: The pad array is at least sizeof(c_int) bytes. Every
        // X event struct starts with a c_int type_ field, so this read
        // is always valid.
        unsafe { *(self.pad.as_ptr() as *const c_int) }
    }

    /// Reinterpret as a key event.
    pub fn as_key(&self) -> &XKeyEvent {
        // SAFETY: XEvent's pad is 192 bytes — large enough for any X event.
        // Caller checks event_type() before using this accessor.
        unsafe { &*(self.pad.as_ptr() as *const XKeyEvent) }
    }

    /// Reinterpret as a button event.
    pub fn as_button(&self) -> &XButtonEvent {
        // SAFETY: XEvent's pad is 192 bytes — large enough for XButtonEvent.
        // Caller checks event_type() before using this accessor.
        unsafe { &*(self.pad.as_ptr() as *const XButtonEvent) }
    }

    /// Reinterpret as a motion event.
    pub fn as_motion(&self) -> &XMotionEvent {
        // SAFETY: XEvent's pad is 192 bytes — large enough for XMotionEvent.
        // Caller checks event_type() before using this accessor.
        unsafe { &*(self.pad.as_ptr() as *const XMotionEvent) }
    }

    /// Reinterpret as a crossing event.
    pub fn as_crossing(&self) -> &XCrossingEvent {
        // SAFETY: XEvent's pad is 192 bytes — large enough for XCrossingEvent.
        // Caller checks event_type() before using this accessor.
        unsafe { &*(self.pad.as_ptr() as *const XCrossingEvent) }
    }

    /// Reinterpret as an expose event.
    pub fn as_expose(&self) -> &XExposeEvent {
        // SAFETY: XEvent's pad is 192 bytes — large enough for XExposeEvent.
        // Caller checks event_type() before using this accessor.
        unsafe { &*(self.pad.as_ptr() as *const XExposeEvent) }
    }

    /// Reinterpret as a configure event.
    pub fn as_configure(&self) -> &XConfigureEvent {
        // SAFETY: XEvent's pad is 192 bytes — large enough for XConfigureEvent.
        // Caller checks event_type() before using this accessor.
        unsafe { &*(self.pad.as_ptr() as *const XConfigureEvent) }
    }

    /// Reinterpret as a focus change event.
    pub fn as_focus_change(&self) -> &XFocusChangeEvent {
        // SAFETY: XEvent's pad is 192 bytes — large enough for XFocusChangeEvent.
        // Caller checks event_type() before using this accessor.
        unsafe { &*(self.pad.as_ptr() as *const XFocusChangeEvent) }
    }

    /// Reinterpret as a client message event.
    pub fn as_client_message(&self) -> &XClientMessageEvent {
        // SAFETY: XEvent's pad is 192 bytes — large enough for XClientMessageEvent.
        // Caller checks event_type() before using this accessor.
        unsafe { &*(self.pad.as_ptr() as *const XClientMessageEvent) }
    }

    /// Reinterpret as a property event.
    pub fn as_property(&self) -> &XPropertyEvent {
        // SAFETY: XEvent's pad is 192 bytes — large enough for XPropertyEvent.
        // Caller checks event_type() before using this accessor.
        unsafe { &*(self.pad.as_ptr() as *const XPropertyEvent) }
    }

    /// Reinterpret as a map event.
    pub fn as_map(&self) -> &XMapEvent {
        // SAFETY: XEvent's pad is 192 bytes — large enough for XMapEvent.
        // Caller checks event_type() before using this accessor.
        unsafe { &*(self.pad.as_ptr() as *const XMapEvent) }
    }

    /// Reinterpret as an unmap event.
    pub fn as_unmap(&self) -> &XUnmapEvent {
        // SAFETY: XEvent's pad is 192 bytes — large enough for XUnmapEvent.
        // Caller checks event_type() before using this accessor.
        unsafe { &*(self.pad.as_ptr() as *const XUnmapEvent) }
    }

    /// Reinterpret as a destroy window event.
    pub fn as_destroy(&self) -> &XDestroyWindowEvent {
        // SAFETY: XEvent's pad is 192 bytes — large enough for XDestroyWindowEvent.
        // Caller checks event_type() before using this accessor.
        unsafe { &*(self.pad.as_ptr() as *const XDestroyWindowEvent) }
    }

    /// Get the window field (present at offset of the `window` field in
    /// the common event header — this is the 5th pointer-sized field for
    /// most event types, after type, serial, send_event, display).
    pub fn window(&self) -> Window {
        // We use XKeyEvent layout since most events share the same
        // initial field ordering with a `window` field at offset 32 (on
        // 64-bit) or 20 (on 32-bit).
        self.as_key().window
    }
}

// ── Window attributes / hints ───────────────────────────────────────

/// Returned by XGetWindowAttributes.
#[repr(C)]
pub struct XWindowAttributes {
    pub x: c_int,
    pub y: c_int,
    pub width: c_int,
    pub height: c_int,
    pub border_width: c_int,
    pub depth: c_int,
    pub visual: *mut Visual,
    pub root: Window,
    pub class: c_int,
    pub bit_gravity: c_int,
    pub win_gravity: c_int,
    pub backing_store: c_int,
    pub backing_planes: c_ulong,
    pub backing_pixel: c_ulong,
    pub save_under: Bool,
    pub colormap: Colormap,
    pub map_installed: Bool,
    pub map_state: c_int,
    pub all_event_masks: c_long,
    pub your_event_mask: c_long,
    pub do_not_propagate_mask: c_long,
    pub override_redirect: Bool,
    pub screen: *mut Screen,
}

/// Window attributes for XCreateWindow / XChangeWindowAttributes.
#[repr(C)]
pub struct XSetWindowAttributes {
    pub background_pixmap: Pixmap,
    pub background_pixel: c_ulong,
    pub border_pixmap: Pixmap,
    pub border_pixel: c_ulong,
    pub bit_gravity: c_int,
    pub win_gravity: c_int,
    pub backing_store: c_int,
    pub backing_planes: c_ulong,
    pub backing_pixel: c_ulong,
    pub save_under: Bool,
    pub event_mask: c_long,
    pub do_not_propagate_mask: c_long,
    pub override_redirect: Bool,
    pub colormap: Colormap,
    pub cursor: Cursor,
}

/// Size hints for XSetWMNormalHints.
#[repr(C)]
pub struct XSizeHints {
    pub flags: c_long,
    pub x: c_int,
    pub y: c_int,
    pub width: c_int,
    pub height: c_int,
    pub min_width: c_int,
    pub min_height: c_int,
    pub max_width: c_int,
    pub max_height: c_int,
    pub width_inc: c_int,
    pub height_inc: c_int,
    pub min_aspect_x: c_int,
    pub min_aspect_y: c_int,
    pub max_aspect_x: c_int,
    pub max_aspect_y: c_int,
    pub base_width: c_int,
    pub base_height: c_int,
    pub win_gravity: c_int,
}

/// XSizeHints flag: user specified position.
pub const PPosition: c_long = 1 << 2;
/// XSizeHints flag: user specified size.
pub const PSize: c_long = 1 << 3;
/// XSizeHints flag: program specified minimum size.
pub const PMinSize: c_long = 1 << 4;
/// XSizeHints flag: program specified maximum size.
pub const PMaxSize: c_long = 1 << 5;

/// Class hint for XSetClassHint.
#[repr(C)]
pub struct XClassHint {
    pub res_name: *mut c_char,
    pub res_class: *mut c_char,
}

/// Text property for XSetWMName etc.
#[repr(C)]
pub struct XTextProperty {
    pub value: *mut c_char,
    pub encoding: Atom,
    pub format: c_int,
    pub nitems: c_ulong,
}

/// GC values for XCreateGC.
#[repr(C)]
pub struct XGCValues {
    pub function: c_int,
    pub plane_mask: c_ulong,
    pub foreground: c_ulong,
    pub background: c_ulong,
    pub line_width: c_int,
    pub line_style: c_int,
    pub cap_style: c_int,
    pub join_style: c_int,
    pub fill_style: c_int,
    pub fill_rule: c_int,
    pub arc_mode: c_int,
    pub tile: Pixmap,
    pub stipple: Pixmap,
    pub ts_x_origin: c_int,
    pub ts_y_origin: c_int,
    pub font: XID,
    pub subwindow_mode: c_int,
    pub graphics_exposures: Bool,
    pub clip_x_origin: c_int,
    pub clip_y_origin: c_int,
    pub clip_mask: Pixmap,
    pub dash_offset: c_int,
    pub dashes: c_char,
}

// ── XRandR types ────────────────────────────────────────────────────

/// Screen resources returned by XRRGetScreenResourcesCurrent.
#[repr(C)]
pub struct XRRScreenResources {
    pub timestamp: Time,
    pub config_timestamp: Time,
    pub ncrtc: c_int,
    pub crtcs: *mut XID,
    pub noutput: c_int,
    pub outputs: *mut XID,
    pub nmode: c_int,
    pub modes: *mut XRRModeInfo,
}

/// Mode info (used inside XRRScreenResources).
#[repr(C)]
pub struct XRRModeInfo {
    pub id: XID,
    pub width: c_uint,
    pub height: c_uint,
    pub dot_clock: c_ulong,
    pub h_sync_start: c_uint,
    pub h_sync_end: c_uint,
    pub h_total: c_uint,
    pub h_skew: c_uint,
    pub v_sync_start: c_uint,
    pub v_sync_end: c_uint,
    pub v_total: c_uint,
    pub name: *mut c_char,
    pub name_length: c_uint,
    pub mode_flags: c_ulong,
}

/// Output info returned by XRRGetOutputInfo.
#[repr(C)]
pub struct XRROutputInfo {
    pub timestamp: Time,
    pub crtc: XID,
    pub name: *mut c_char,
    pub name_len: c_int,
    pub mm_width: c_ulong,
    pub mm_height: c_ulong,
    pub connection: u16,
    pub subpixel_order: u16,
    pub ncrtc: c_int,
    pub crtcs: *mut XID,
    pub nclone: c_int,
    pub clones: *mut XID,
    pub nmode: c_int,
    pub npreferred: c_int,
    pub modes: *mut XID,
}

/// CRTC info returned by XRRGetCrtcInfo.
#[repr(C)]
pub struct XRRCrtcInfo {
    pub timestamp: Time,
    pub x: c_int,
    pub y: c_int,
    pub width: c_uint,
    pub height: c_uint,
    pub mode: XID,
    pub rotation: u16,
    pub noutput: c_int,
    pub outputs: *mut XID,
    pub rotations: u16,
    pub npossible: c_int,
    pub possible: *mut XID,
}

/// Monitor info returned by XRRGetMonitors.
#[repr(C)]
pub struct XRRMonitorInfo {
    pub name: Atom,
    pub primary: Bool,
    pub automatic: Bool,
    pub noutput: c_int,
    pub x: c_int,
    pub y: c_int,
    pub width: c_int,
    pub height: c_int,
    pub mwidth: c_int,
    pub mheight: c_int,
    pub outputs: *mut XID,
}

// ── Constants: event types ──────────────────────────────────────────

pub const KeyPress: c_int = 2;
pub const KeyRelease: c_int = 3;
pub const ButtonPress: c_int = 4;
pub const ButtonRelease: c_int = 5;
pub const MotionNotify: c_int = 6;
pub const EnterNotify: c_int = 7;
pub const LeaveNotify: c_int = 8;
pub const FocusIn: c_int = 9;
pub const FocusOut: c_int = 10;
pub const Expose: c_int = 12;
pub const DestroyNotify: c_int = 17;
pub const UnmapNotify: c_int = 18;
pub const MapNotify: c_int = 19;
pub const ConfigureNotify: c_int = 22;
pub const PropertyNotify: c_int = 28;
pub const ClientMessage: c_int = 33;

// ── Constants: event masks ──────────────────────────────────────────

pub const KeyPressMask: c_long = 1 << 0;
pub const KeyReleaseMask: c_long = 1 << 1;
pub const ButtonPressMask: c_long = 1 << 2;
pub const ButtonReleaseMask: c_long = 1 << 3;
pub const EnterWindowMask: c_long = 1 << 4;
pub const LeaveWindowMask: c_long = 1 << 5;
pub const PointerMotionMask: c_long = 1 << 6;
pub const ExposureMask: c_long = 1 << 15;
pub const StructureNotifyMask: c_long = 1 << 17;
pub const SubstructureNotifyMask: c_long = 1 << 19;
pub const SubstructureRedirectMask: c_long = 1 << 20;
pub const FocusChangeMask: c_long = 1 << 21;

// ── Constants: button identifiers ───────────────────────────────────

pub const Button1: c_uint = 1;
pub const Button2: c_uint = 2;
pub const Button3: c_uint = 3;
pub const Button4: c_uint = 4; // scroll up
pub const Button5: c_uint = 5; // scroll down

// ── Constants: modifier masks ───────────────────────────────────────

pub const ShiftMask: c_uint = 1 << 0;
pub const LockMask: c_uint = 1 << 1;
pub const ControlMask: c_uint = 1 << 2;
pub const Mod1Mask: c_uint = 1 << 3; // Alt
pub const Mod2Mask: c_uint = 1 << 4; // Num Lock
pub const Mod3Mask: c_uint = 1 << 5;
pub const Mod4Mask: c_uint = 1 << 6; // Super
pub const Mod5Mask: c_uint = 1 << 7;

// ── Constants: window attribute value masks ─────────────────────────

pub const CWBackPixmap: c_ulong = 1 << 0;
pub const CWBackPixel: c_ulong = 1 << 1;
pub const CWBorderPixmap: c_ulong = 1 << 2;
pub const CWBorderPixel: c_ulong = 1 << 3;
pub const CWBitGravity: c_ulong = 1 << 4;
pub const CWWinGravity: c_ulong = 1 << 5;
pub const CWBackingStore: c_ulong = 1 << 6;
pub const CWBackingPlanes: c_ulong = 1 << 7;
pub const CWBackingPixel: c_ulong = 1 << 8;
pub const CWOverrideRedirect: c_ulong = 1 << 9;
pub const CWSaveUnder: c_ulong = 1 << 10;
pub const CWEventMask: c_ulong = 1 << 11;
pub const CWDontPropagate: c_ulong = 1 << 12;
pub const CWColormap: c_ulong = 1 << 13;
pub const CWCursor: c_ulong = 1 << 14;

// ── Constants: window class / image format ──────────────────────────

pub const InputOutput: c_uint = 1;
pub const InputOnly: c_uint = 2;

pub const XYBitmap: c_int = 0;
pub const XYPixmap: c_int = 1;
pub const ZPixmap: c_int = 2;

pub const TrueColor: c_int = 4;

// ── Constants: misc ─────────────────────────────────────────────────

pub const None_: c_ulong = 0;
pub const RevertToParent: c_int = 2;
pub const CurrentTime: Time = 0;
pub const PropModeReplace: c_int = 0;
pub const LSBFirst: c_int = 0;
pub const MSBFirst: c_int = 1;
pub const XA_ATOM: Atom = 4;
pub const XA_CARDINAL: Atom = 6;
pub const XA_STRING: Atom = 31;
pub const XA_WM_NAME: Atom = 39;

/// RandR connection state: connected.
pub const RR_Connected: u16 = 0;

// ── Keysym constants ────────────────────────────────────────────────

// Latin-1 / miscellaneous
pub const XK_BackSpace: KeySym = 0xff08;
pub const XK_Tab: KeySym = 0xff09;
pub const XK_Return: KeySym = 0xff0d;
pub const XK_Escape: KeySym = 0xff1b;
pub const XK_Delete: KeySym = 0xffff;

// Cursor control & motion
pub const XK_Home: KeySym = 0xff50;
pub const XK_Left: KeySym = 0xff51;
pub const XK_Up: KeySym = 0xff52;
pub const XK_Right: KeySym = 0xff53;
pub const XK_Down: KeySym = 0xff54;
pub const XK_Page_Up: KeySym = 0xff55;
pub const XK_Page_Down: KeySym = 0xff56;
pub const XK_End: KeySym = 0xff57;
pub const XK_Insert: KeySym = 0xff63;

// Misc functions
pub const XK_Print: KeySym = 0xff61;
pub const XK_Pause: KeySym = 0xff13;
pub const XK_Scroll_Lock: KeySym = 0xff14;
pub const XK_Menu: KeySym = 0xff67;

// Modifier keys
pub const XK_Shift_L: KeySym = 0xffe1;
pub const XK_Shift_R: KeySym = 0xffe2;
pub const XK_Control_L: KeySym = 0xffe3;
pub const XK_Control_R: KeySym = 0xffe4;
pub const XK_Caps_Lock: KeySym = 0xffe5;
pub const XK_Alt_L: KeySym = 0xffe9;
pub const XK_Alt_R: KeySym = 0xffea;
pub const XK_Super_L: KeySym = 0xffeb;
pub const XK_Super_R: KeySym = 0xffec;
pub const XK_Num_Lock: KeySym = 0xff7f;

// Function keys
pub const XK_F1: KeySym = 0xffbe;
pub const XK_F2: KeySym = 0xffbf;
pub const XK_F3: KeySym = 0xffc0;
pub const XK_F4: KeySym = 0xffc1;
pub const XK_F5: KeySym = 0xffc2;
pub const XK_F6: KeySym = 0xffc3;
pub const XK_F7: KeySym = 0xffc4;
pub const XK_F8: KeySym = 0xffc5;
pub const XK_F9: KeySym = 0xffc6;
pub const XK_F10: KeySym = 0xffc7;
pub const XK_F11: KeySym = 0xffc8;
pub const XK_F12: KeySym = 0xffc9;

// ASCII-mapped keys (space, digits, lowercase letter names)
pub const XK_space: KeySym = 0x0020;

pub const XK_0: KeySym = 0x0030;
pub const XK_1: KeySym = 0x0031;
pub const XK_2: KeySym = 0x0032;
pub const XK_3: KeySym = 0x0033;
pub const XK_4: KeySym = 0x0034;
pub const XK_5: KeySym = 0x0035;
pub const XK_6: KeySym = 0x0036;
pub const XK_7: KeySym = 0x0037;
pub const XK_8: KeySym = 0x0038;
pub const XK_9: KeySym = 0x0039;

pub const XK_a: KeySym = 0x0061;
pub const XK_b: KeySym = 0x0062;
pub const XK_c: KeySym = 0x0063;
pub const XK_d: KeySym = 0x0064;
pub const XK_e: KeySym = 0x0065;
pub const XK_f: KeySym = 0x0066;
pub const XK_g: KeySym = 0x0067;
pub const XK_h: KeySym = 0x0068;
pub const XK_i: KeySym = 0x0069;
pub const XK_j: KeySym = 0x006a;
pub const XK_k: KeySym = 0x006b;
pub const XK_l: KeySym = 0x006c;
pub const XK_m: KeySym = 0x006d;
pub const XK_n: KeySym = 0x006e;
pub const XK_o: KeySym = 0x006f;
pub const XK_p: KeySym = 0x0070;
pub const XK_q: KeySym = 0x0071;
pub const XK_r: KeySym = 0x0072;
pub const XK_s: KeySym = 0x0073;
pub const XK_t: KeySym = 0x0074;
pub const XK_u: KeySym = 0x0075;
pub const XK_v: KeySym = 0x0076;
pub const XK_w: KeySym = 0x0077;
pub const XK_x: KeySym = 0x0078;
pub const XK_y: KeySym = 0x0079;
pub const XK_z: KeySym = 0x007a;

// Uppercase letter keysyms (used for Shift+letter)
pub const XK_A: KeySym = 0x0041;
pub const XK_B: KeySym = 0x0042;
pub const XK_C: KeySym = 0x0043;
pub const XK_D: KeySym = 0x0044;
pub const XK_E: KeySym = 0x0045;
pub const XK_F: KeySym = 0x0046;
pub const XK_G: KeySym = 0x0047;
pub const XK_H: KeySym = 0x0048;
pub const XK_I: KeySym = 0x0049;
pub const XK_J: KeySym = 0x004a;
pub const XK_K: KeySym = 0x004b;
pub const XK_L: KeySym = 0x004c;
pub const XK_M: KeySym = 0x004d;
pub const XK_N: KeySym = 0x004e;
pub const XK_O: KeySym = 0x004f;
pub const XK_P: KeySym = 0x0050;
pub const XK_Q: KeySym = 0x0051;
pub const XK_R: KeySym = 0x0052;
pub const XK_S: KeySym = 0x0053;
pub const XK_T: KeySym = 0x0054;
pub const XK_U: KeySym = 0x0055;
pub const XK_V: KeySym = 0x0056;
pub const XK_W: KeySym = 0x0057;
pub const XK_X: KeySym = 0x0058;
pub const XK_Y: KeySym = 0x0059;
pub const XK_Z: KeySym = 0x005a;

// Punctuation / symbol keys
pub const XK_comma: KeySym = 0x002c;
pub const XK_period: KeySym = 0x002e;
pub const XK_slash: KeySym = 0x002f;
pub const XK_semicolon: KeySym = 0x003b;
pub const XK_apostrophe: KeySym = 0x0027;
pub const XK_bracketleft: KeySym = 0x005b;
pub const XK_bracketright: KeySym = 0x005d;
pub const XK_backslash: KeySym = 0x005c;
pub const XK_minus: KeySym = 0x002d;
pub const XK_equal: KeySym = 0x003d;
pub const XK_grave: KeySym = 0x0060;

// ── extern "C" — libX11 ────────────────────────────────────────────

#[link(name = "X11")]
unsafe extern "C" {
    // Display management
    pub fn XOpenDisplay(display_name: *const c_char) -> *mut Display;
    pub fn XCloseDisplay(display: *mut Display) -> c_int;
    pub fn XDefaultScreen(display: *mut Display) -> c_int;
    pub fn XDefaultRootWindow(display: *mut Display) -> Window;
    pub fn XConnectionNumber(display: *mut Display) -> c_int;

    // Screen queries
    pub fn XScreenCount(display: *mut Display) -> c_int;
    pub fn XScreenOfDisplay(display: *mut Display, screen_number: c_int) -> *mut Screen;
    pub fn XWidthOfScreen(screen: *mut Screen) -> c_int;
    pub fn XHeightOfScreen(screen: *mut Screen) -> c_int;

    // Window creation / destruction
    pub fn XCreateWindow(
        display: *mut Display,
        parent: Window,
        x: c_int,
        y: c_int,
        width: c_uint,
        height: c_uint,
        border_width: c_uint,
        depth: c_int,
        class: c_uint,
        visual: *mut Visual,
        valuemask: c_ulong,
        attributes: *mut XSetWindowAttributes,
    ) -> Window;
    pub fn XDestroyWindow(display: *mut Display, window: Window) -> c_int;
    pub fn XMapWindow(display: *mut Display, window: Window) -> c_int;
    pub fn XUnmapWindow(display: *mut Display, window: Window) -> c_int;

    // Window manipulation
    pub fn XMoveResizeWindow(
        display: *mut Display,
        window: Window,
        x: c_int,
        y: c_int,
        width: c_uint,
        height: c_uint,
    ) -> c_int;
    pub fn XRaiseWindow(display: *mut Display, window: Window) -> c_int;
    pub fn XLowerWindow(display: *mut Display, window: Window) -> c_int;
    pub fn XSetInputFocus(
        display: *mut Display,
        window: Window,
        revert_to: c_int,
        time: Time,
    ) -> c_int;
    pub fn XStoreName(display: *mut Display, window: Window, name: *const c_char) -> c_int;
    pub fn XSelectInput(display: *mut Display, window: Window, event_mask: c_long) -> c_int;
    pub fn XGetWindowAttributes(
        display: *mut Display,
        window: Window,
        attributes: *mut XWindowAttributes,
    ) -> Status;

    // Size hints
    pub fn XSetWMNormalHints(
        display: *mut Display,
        window: Window,
        hints: *mut XSizeHints,
    ) -> c_int;
    pub fn XAllocSizeHints() -> *mut XSizeHints;

    // Event handling
    pub fn XNextEvent(display: *mut Display, event: *mut XEvent) -> c_int;
    pub fn XPending(display: *mut Display) -> c_int;
    pub fn XCheckMaskEvent(
        display: *mut Display,
        event_mask: c_long,
        event: *mut XEvent,
    ) -> Bool;
    pub fn XSendEvent(
        display: *mut Display,
        window: Window,
        propagate: Bool,
        event_mask: c_long,
        event: *mut XEvent,
    ) -> Status;
    pub fn XFlush(display: *mut Display) -> c_int;
    pub fn XSync(display: *mut Display, discard: Bool) -> c_int;

    // Atoms / properties
    pub fn XInternAtom(
        display: *mut Display,
        atom_name: *const c_char,
        only_if_exists: Bool,
    ) -> Atom;
    pub fn XChangeProperty(
        display: *mut Display,
        window: Window,
        property: Atom,
        type_: Atom,
        format: c_int,
        mode: c_int,
        data: *const u8,
        nelements: c_int,
    ) -> c_int;
    pub fn XSetWMProtocols(
        display: *mut Display,
        window: Window,
        protocols: *mut Atom,
        count: c_int,
    ) -> Status;
    pub fn XGetAtomName(display: *mut Display, atom: Atom) -> *mut c_char;

    // Graphics context
    pub fn XCreateGC(
        display: *mut Display,
        drawable: Window,
        valuemask: c_ulong,
        values: *mut XGCValues,
    ) -> GC;
    pub fn XFreeGC(display: *mut Display, gc: GC) -> c_int;

    // Image handling
    pub fn XCreateImage(
        display: *mut Display,
        visual: *mut Visual,
        depth: c_uint,
        format: c_int,
        offset: c_int,
        data: *mut c_char,
        width: c_uint,
        height: c_uint,
        bitmap_pad: c_int,
        bytes_per_line: c_int,
    ) -> *mut XImage;
    pub fn XPutImage(
        display: *mut Display,
        drawable: Window,
        gc: GC,
        image: *mut XImage,
        src_x: c_int,
        src_y: c_int,
        dest_x: c_int,
        dest_y: c_int,
        width: c_uint,
        height: c_uint,
    ) -> c_int;
    pub fn XDestroyImage(image: *mut XImage) -> c_int;

    // Visual / depth / colormap
    pub fn XDefaultVisual(display: *mut Display, screen_number: c_int) -> *mut Visual;
    pub fn XDefaultDepth(display: *mut Display, screen_number: c_int) -> c_int;
    pub fn XDefaultColormap(display: *mut Display, screen_number: c_int) -> Colormap;

    // Pixmap
    pub fn XCreatePixmap(
        display: *mut Display,
        drawable: Window,
        width: c_uint,
        height: c_uint,
        depth: c_uint,
    ) -> Pixmap;
    pub fn XFreePixmap(display: *mut Display, pixmap: Pixmap) -> c_int;
    pub fn XCopyArea(
        display: *mut Display,
        src: Window,
        dest: Window,
        gc: GC,
        src_x: c_int,
        src_y: c_int,
        width: c_uint,
        height: c_uint,
        dest_x: c_int,
        dest_y: c_int,
    ) -> c_int;

    // Keyboard
    pub fn XLookupKeysym(event: *const XKeyEvent, index: c_int) -> KeySym;
    pub fn XLookupString(
        event: *const XKeyEvent,
        buffer: *mut c_char,
        buffer_len: c_int,
        keysym: *mut KeySym,
        compose: *mut c_void,
    ) -> c_int;
    pub fn XKeysymToKeycode(display: *mut Display, keysym: KeySym) -> c_uint;

    // Pointer
    pub fn XGrabPointer(
        display: *mut Display,
        grab_window: Window,
        owner_events: Bool,
        event_mask: c_uint,
        pointer_mode: c_int,
        keyboard_mode: c_int,
        confine_to: Window,
        cursor: Cursor,
        time: Time,
    ) -> c_int;
    pub fn XUngrabPointer(display: *mut Display, time: Time) -> c_int;
    pub fn XWarpPointer(
        display: *mut Display,
        src_window: Window,
        dest_window: Window,
        src_x: c_int,
        src_y: c_int,
        src_width: c_uint,
        src_height: c_uint,
        dest_x: c_int,
        dest_y: c_int,
    ) -> c_int;

    // Memory
    pub fn XFree(data: *mut c_void) -> c_int;
}

// ── extern "C" — libXrandr ──────────────────────────────────────────

#[link(name = "Xrandr")]
unsafe extern "C" {
    pub fn XRRGetScreenResourcesCurrent(
        display: *mut Display,
        window: Window,
    ) -> *mut XRRScreenResources;
    pub fn XRRGetOutputInfo(
        display: *mut Display,
        resources: *mut XRRScreenResources,
        output: XID,
    ) -> *mut XRROutputInfo;
    pub fn XRRGetCrtcInfo(
        display: *mut Display,
        resources: *mut XRRScreenResources,
        crtc: XID,
    ) -> *mut XRRCrtcInfo;
    pub fn XRRFreeScreenResources(resources: *mut XRRScreenResources);
    pub fn XRRFreeOutputInfo(output_info: *mut XRROutputInfo);
    pub fn XRRFreeCrtcInfo(crtc_info: *mut XRRCrtcInfo);
    pub fn XRRGetMonitors(
        display: *mut Display,
        window: Window,
        get_active: Bool,
        nmonitors: *mut c_int,
    ) -> *mut XRRMonitorInfo;
    pub fn XRRFreeMonitors(monitors: *mut XRRMonitorInfo);
}
