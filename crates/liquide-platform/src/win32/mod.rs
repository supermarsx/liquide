//! Win32 platform backend.
//!
//! Provides a complete `PlatformBackend` implementation using raw Win32 API
//! calls via FFI (no external crate dependencies). Links against user32.dll,
//! gdi32.dll, kernel32.dll, and shell32.dll at load time.

pub mod dxgi;
pub mod ffi;
pub mod input;
pub mod present_verify;

pub use dxgi::{DxgiPresentCapabilities, DxgiPresentMode};
pub use present_verify::{
    changed_pixels_in_region, compare_frames, encode_png_bgra, evaluate_partial_present,
    fill_dib_from_source, make_frame_with_cursor, make_test_pattern, FrameComparison,
    PartialPresentCheck, PixelRect, PresentPath, PresentVerifyMetrics,
};

use std::cell::UnsafeCell;
use std::collections::{HashMap, VecDeque};
use std::ffi::c_void;
use std::ptr;
use std::time::{SystemTime, UNIX_EPOCH};

use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::PixelFormat;
use liquide_input::keyboard::{KeyCode, KeyEvent, KeyState};
use liquide_input::mouse::{ButtonState, MouseButton, MouseEvent, ScrollAxis};

use crate::display::{DisplayBackend, MonitorInfo};
use crate::dnd::NullDragDrop;
use crate::event_loop::PlatformEvent;
use crate::keymap::KeymapTranslator;
use crate::notifications::{NativeNotificationParams, NativeNotifications};
use crate::taskbar::{JumpListItem, TaskbarIntegration};
use crate::tray::{NativeTray, NativeTrayHandle, NativeTrayParams, TrayUpdate};
use crate::window_host::{NativeWindowHandle, NativeWindowHost, NativeWindowParams};
use crate::{NativeDragDrop, PlatformBackend, PlatformError, PlatformResult, PresentFeedback};

// ---------------------------------------------------------------------------
// UTF-16 helper
// ---------------------------------------------------------------------------

/// Convert a Rust `&str` to a null-terminated UTF-16 vector for Win32 wide APIs.
fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// Decode a null-terminated UTF-16 slice into a `String`.
fn from_wide(s: &[u16]) -> String {
    let len = s.iter().position(|&c| c == 0).unwrap_or(s.len());
    String::from_utf16_lossy(&s[..len])
}

/// Return a microsecond timestamp (best-effort monotonic via `SystemTime`).
fn timestamp_us() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0)
}

fn timestamp_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

/// Monotonic millisecond clock for present-cadence decisions. Backed by a
/// process-lifetime `Instant` so it is immune to wall-clock jumps.
fn monotonic_ms() -> u64 {
    use std::sync::OnceLock;
    use std::time::Instant;
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u64
}

// ---------------------------------------------------------------------------
// Damage-rect normalization (partial present)
// ---------------------------------------------------------------------------

/// Convert a compositor-space damage rect (f32, top-left origin) into an
/// integer pixel rect clamped to the surface, *expanding* to whole pixels so a
/// fractional rect never under-covers a partially-touched pixel. Returns
/// `None` if the rect is fully outside the surface or collapses to empty.
fn damage_rect_to_pixels(r: &Rect, width: u32, height: u32) -> Option<PixelRect> {
    // Reject non-finite coordinates outright (NaN/inf would poison the floor/
    // ceil below and could produce a bogus rect).
    if !(r.x.is_finite() && r.y.is_finite() && r.width.is_finite() && r.height.is_finite()) {
        return None;
    }
    let x0 = r.x.floor().max(0.0);
    let y0 = r.y.floor().max(0.0);
    // Expand the far edge with ceil so any pixel the rect partially covers is
    // included, then clamp to the surface extent.
    let x1 = (r.x + r.width).ceil().min(width as f32);
    let y1 = (r.y + r.height).ceil().min(height as f32);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let x = x0 as u32;
    let y = y0 as u32;
    Some(PixelRect {
        x,
        y,
        w: (x1 as u32).saturating_sub(x),
        h: (y1 as u32).saturating_sub(y),
    }
    .clamped(width, height))
}

/// Normalize a slice of compositor damage rects into a clamped, de-duplicated,
/// coalesced set of integer pixel rects ready to blit.
///
/// - Out-of-bounds / empty / non-finite rects are dropped.
/// - Exact duplicates are removed.
/// - Rects that are fully contained in another rect are absorbed, and rects
///   that touch/overlap are greedily merged into their bounding box. This keeps
///   the BitBlt count small and guarantees every damaged pixel is covered
///   exactly once (no torn / double-blitted regions).
///
/// An empty input slice yields an empty result (caller treats this as
/// "present nothing changed").
fn coalesce_damage_rects(rects: &[Rect], width: u32, height: u32) -> Vec<PixelRect> {
    let mut out: Vec<PixelRect> = Vec::with_capacity(rects.len());
    for r in rects {
        let Some(pr) = damage_rect_to_pixels(r, width, height) else {
            continue;
        };
        merge_pixel_rect(&mut out, pr);
    }
    out
}

/// Right edge (exclusive) of a pixel rect.
fn pr_right(r: &PixelRect) -> u32 {
    r.x.saturating_add(r.w)
}

/// Bottom edge (exclusive) of a pixel rect.
fn pr_bottom(r: &PixelRect) -> u32 {
    r.y.saturating_add(r.h)
}

/// True when `a` fully contains `b`.
fn pr_contains(a: &PixelRect, b: &PixelRect) -> bool {
    b.x >= a.x && b.y >= a.y && pr_right(b) <= pr_right(a) && pr_bottom(b) <= pr_bottom(a)
}

/// True when `a` and `b` overlap or share an edge (touching rects are merged so
/// adjacent damage doesn't produce a hairline seam between two BitBlts).
fn pr_touches(a: &PixelRect, b: &PixelRect) -> bool {
    a.x <= pr_right(b) && b.x <= pr_right(a) && a.y <= pr_bottom(b) && b.y <= pr_bottom(a)
}

/// Bounding box of two pixel rects.
fn pr_union(a: &PixelRect, b: &PixelRect) -> PixelRect {
    let x = a.x.min(b.x);
    let y = a.y.min(b.y);
    let right = pr_right(a).max(pr_right(b));
    let bottom = pr_bottom(a).max(pr_bottom(b));
    PixelRect {
        x,
        y,
        w: right.saturating_sub(x),
        h: bottom.saturating_sub(y),
    }
}

/// Insert `pr` into `set`, absorbing duplicates/contained rects and greedily
/// merging anything it touches into its bounding box. Re-runs until no further
/// merge is possible so the result has no overlapping/touching members.
fn merge_pixel_rect(set: &mut Vec<PixelRect>, pr: PixelRect) {
    if pr.w == 0 || pr.h == 0 {
        return;
    }
    let mut acc = pr;
    loop {
        let mut merged_any = false;
        let mut i = 0;
        while i < set.len() {
            let existing = set[i];
            if pr_contains(&existing, &acc) {
                // Already covered — nothing to add.
                return;
            }
            if pr_touches(&acc, &existing) {
                acc = pr_union(&acc, &existing);
                set.swap_remove(i);
                merged_any = true;
                // Restart scan: the enlarged `acc` may now touch earlier rects.
            } else {
                i += 1;
            }
        }
        if !merged_any {
            break;
        }
    }
    set.push(acc);
}

/// Apply a present's pixels into the off-screen DIB back-buffer, copying only
/// what changed.
///
/// `dib` and `src` are both top-down packed BGRA8 of the same `stride` x
/// `height` layout. When `full` is true (a full present `None`, or the DIB was
/// just (re)allocated and has undefined contents) the whole frame is copied so
/// the DIB is a complete valid frame. Otherwise only the `rects` sub-regions are
/// copied; every other DIB pixel retains its prior (still-valid) content, so the
/// DIB *accumulates* damage across partial presents and a WM_PAINT full replay
/// never exposes a stale/torn region.
///
/// Rects must already be clamped to the surface; out-of-range rows/spans are
/// skipped defensively. Returns the number of pixels written into the DIB (a
/// bandwidth proxy the tests assert on — a partial present must write strictly
/// fewer than the whole frame).
fn apply_present_to_dib(
    dib: &mut [u8],
    src: &[u8],
    stride: u32,
    height: u32,
    rects: Option<&[PixelRect]>,
    full: bool,
) -> usize {
    let row_bytes = stride as usize;
    let total = row_bytes.saturating_mul(height as usize);
    let copyable = total.min(dib.len()).min(src.len());
    if full || rects.is_none() {
        dib[..copyable].copy_from_slice(&src[..copyable]);
        return copyable / 4;
    }
    let mut written = 0usize;
    if let Some(rects) = rects {
        for r in rects {
            if r.w == 0 || r.h == 0 {
                continue;
            }
            let span = (r.w as usize) * 4;
            for row in r.y..pr_bottom(r) {
                let off = (row as usize) * row_bytes + (r.x as usize) * 4;
                let end = off + span;
                if end > copyable {
                    continue;
                }
                dib[off..end].copy_from_slice(&src[off..end]);
                written += r.w as usize;
            }
        }
    }
    written
}

// ---------------------------------------------------------------------------
// RDP-aware present coalescing (remote cadence cap)
// ---------------------------------------------------------------------------

/// Default on-screen present cadence cap when a remote (RDP) session is
/// detected, in frames per second. RDP samples the desktop at ~30-60 Hz, so
/// presenting (BitBlt'ing) faster than this just burns CPU/channel bandwidth on
/// updates the remote client will never sample. Capping at 60 Hz keeps the
/// render thread free to run at full speed while the present layer coalesces
/// damage and flips at most this often.
const DEFAULT_REMOTE_PRESENT_HZ: u32 = 60;

/// Coalesces partial-present damage across multiple `present_frame_damaged`
/// calls so the on-screen BitBlt cadence can be capped (e.g. to the RDP sample
/// rate) without ever *dropping* damage.
///
/// This is a pure state machine with no Win32 dependency: the caller feeds it
/// each present's damage hint plus the current monotonic time in milliseconds,
/// and it decides whether to present *now* and, if so, returns the accumulated
/// damage to blit (the union of every coalesced present's damage since the last
/// flip). When it returns "not now", the damage is retained for the next flip —
/// nothing is discarded.
///
/// Correctness contract:
/// - A `None` (full) present is *sticky*: once any coalesced present was full,
///   the eventual flip is a full present (a full present subsumes every rect).
/// - `Some(rects)` accumulates the union of all rects across deferred presents.
/// - When `enabled` is false (local session, or cap disabled), every present is
///   emitted immediately with its own damage unchanged — zero added latency,
///   identical to the pre-coalescing local path.
struct RemotePresentCoalescer {
    /// When false, never coalesce — present every frame immediately (local).
    enabled: bool,
    /// Minimum milliseconds between on-screen flips while coalescing.
    min_interval_ms: u64,
    /// Monotonic time (ms) of the last emitted flip, or `None` before the first.
    last_flip_ms: Option<u64>,
    /// Accumulated, coalesced damage rects pending the next flip (held in the
    /// same normalized/merged form the blit path consumes). Empty +
    /// `!pending_full` means nothing is queued.
    pending: Vec<PixelRect>,
    /// True if any coalesced (deferred) present was a full present (`None`).
    pending_full: bool,
}

/// Outcome of feeding one present into the coalescer.
enum CoalesceDecision {
    /// Present now with this damage hint (`None` = full surface). The carried
    /// rects are already coalesced/merged and clamped, ready to BitBlt.
    PresentNow(Option<Vec<PixelRect>>),
    /// Defer: damage was accumulated; do not BitBlt to screen this call.
    Defer,
}

impl RemotePresentCoalescer {
    fn new(enabled: bool, cap_hz: u32) -> Self {
        let hz = cap_hz.max(1);
        Self {
            enabled,
            min_interval_ms: (1000 / hz as u64).max(1),
            last_flip_ms: None,
            pending: Vec::new(),
            pending_full: false,
        }
    }

    /// Merge a present's damage hint into the pending accumulator. Never drops a
    /// rect: rects are unioned via `merge_pixel_rect`; a full present sets the
    /// sticky `pending_full` flag (a full present subsumes every rect).
    fn accumulate(&mut self, damage: Option<&[PixelRect]>) {
        match damage {
            None => {
                // Full present subsumes everything — collapse to a full flip.
                self.pending_full = true;
                self.pending.clear();
            }
            Some(rects) => {
                if !self.pending_full {
                    for r in rects {
                        merge_pixel_rect(&mut self.pending, *r);
                    }
                }
            }
        }
    }

    /// Drain the pending accumulator into a damage hint for a flip.
    fn drain(&mut self) -> Option<Vec<PixelRect>> {
        if self.pending_full {
            self.pending.clear();
            self.pending_full = false;
            None
        } else {
            Some(std::mem::take(&mut self.pending))
        }
    }

    /// Decide what to do with a present arriving at monotonic time `now_ms`.
    ///
    /// Returns `PresentNow(damage)` when a flip should happen now (carrying the
    /// union of any previously-deferred damage plus this one), or `Defer` when
    /// the present was coalesced and will be flipped by a later call. Deferred
    /// damage is always retained — never discarded.
    fn on_present(&mut self, damage: Option<&[PixelRect]>, now_ms: u64) -> CoalesceDecision {
        if !self.enabled {
            // Local path: present immediately, no coalescing, no added latency.
            return CoalesceDecision::PresentNow(damage.map(|r| r.to_vec()));
        }

        self.accumulate(damage);

        let due = match self.last_flip_ms {
            None => true, // First present after enabling flips immediately.
            Some(last) => now_ms.saturating_sub(last) >= self.min_interval_ms,
        };

        if due {
            self.last_flip_ms = Some(now_ms);
            CoalesceDecision::PresentNow(self.drain())
        } else {
            CoalesceDecision::Defer
        }
    }
}

// ---------------------------------------------------------------------------
// Per-window data stored via GWLP_USERDATA
// ---------------------------------------------------------------------------

/// Data attached to every window created by this backend.
///
/// A raw pointer to this struct is stored in the window's `GWLP_USERDATA`
/// slot so that the window procedure can push events into the shared queue.
struct WindowData {
    /// The `NativeWindowHandle` we assigned to this window.
    handle: NativeWindowHandle,
    /// Pointer back to the platform's event queue.
    /// The queue lives inside the `Win32Platform`, which owns all windows
    /// and outlives them.
    /// Wrapped in `UnsafeCell` to avoid creating `&mut` references in the
    /// reentrant wndproc (which would alias with other borrows).
    event_queue: *const UnsafeCell<VecDeque<PlatformEvent>>,
    /// Current hardware cursor handle. When non-null, the wndproc uses
    /// this for `WM_SETCURSOR` so the OS renders the cursor shape.
    cursor: std::sync::atomic::AtomicIsize,
    /// Current DPI scale (1.0 == 96 DPI) for this window, stored as the bit
    /// pattern of an `f32`. Written by the wndproc on `WM_DPICHANGED` (the
    /// wndproc has no access to the Rust-side `WindowInfo`, so the live scale
    /// lives here) and read back through `WindowInfo::dpi_scale`.
    dpi_scale_bits: std::sync::atomic::AtomicU32,
    /// Published handle to the live GDI off-screen back-buffer's memory DC, so
    /// `WM_PAINT` can BitBlt the authoritative last-presented frame instead of
    /// validating the update region without painting.
    ///
    /// The back-buffer is the single source of truth for the window's pixels
    /// (every present — full OR cursor-only partial damage — composites the
    /// whole frame into it and BitBlts it out). When the compositor / RDP client
    /// asks the window to repaint a region (occlusion, focus, remote refresh)
    /// between presents, replaying the back-buffer keeps that region correct;
    /// without it the region keeps whatever the compositor last sampled, which
    /// after a cursor-only present can be a stale cursor → smear/trail over RDP.
    ///
    /// Stored as the raw `HDC` value (an `isize`); `0` means "no back-buffer
    /// yet — fall through to the default validate-only behaviour". Width/height
    /// are published alongside so the paint blit copies the exact extent.
    ///
    /// SAFETY/threading: the message pump (wndproc) and the present path both
    /// run on the window's owning thread, so the `HDC` is only ever touched on
    /// that thread. The atomics exist solely to publish the value without
    /// constructing a `&mut WindowData` in the reentrant wndproc.
    backbuffer_dc: std::sync::atomic::AtomicIsize,
    /// Width of the published back-buffer (see [`backbuffer_dc`]).
    backbuffer_w: std::sync::atomic::AtomicU32,
    /// Height of the published back-buffer (see [`backbuffer_dc`]).
    backbuffer_h: std::sync::atomic::AtomicU32,
}

// ---------------------------------------------------------------------------
// Window procedure
// ---------------------------------------------------------------------------

/// The global window procedure registered with our WNDCLASSEXW.
///
/// # Safety
///
/// This function is called by the Win32 message dispatcher. It reads the
/// `WindowData` pointer stored in `GWLP_USERDATA`. That pointer is valid
/// for the lifetime of the window because `Win32Platform` owns both the
/// window data and the event queue, and `destroy_window` cleans up the
/// user-data before the window handle becomes invalid.
unsafe extern "system" fn wndproc(
    hwnd: ffi::HWND,
    msg: ffi::UINT,
    wp: ffi::WPARAM,
    lp: ffi::LPARAM,
) -> ffi::LRESULT {
    // During WM_CREATE the user-data is not yet set; fall through to default.
    // SAFETY: GetWindowLongPtrW is safe on any valid HWND; returns 0 if not yet set.
    let user_ptr = unsafe { ffi::GetWindowLongPtrW(hwnd, ffi::GWLP_USERDATA) };
    if user_ptr == 0 {
        // SAFETY: DefWindowProcW is safe to call with any valid window message.
        return unsafe { ffi::DefWindowProcW(hwnd, msg, wp, lp) };
    }

    // Safety: the pointer was set in `create_window` and points to a
    // heap-allocated `WindowData` that is valid until `destroy_window`.
    let wd = unsafe { &*(user_ptr as *const WindowData) };
    let handle = wd.handle;

    // Use a macro to push events via raw pointer operations to avoid
    // creating a `&mut` reference, which would be UB due to reentrancy.
    macro_rules! push_event {
        ($event:expr) => {
            // SAFETY: The event_queue pointer is valid for the window's lifetime.
            // We use raw pointer ops to avoid creating `&mut` references (UB due to reentrancy).
            unsafe { (*(*wd.event_queue).get()).push_back($event) }
        };
    }

    match msg {
        ffi::WM_CLOSE => {
            push_event!(PlatformEvent::WindowCloseRequested { handle });
            // Return 0 to prevent DefWindowProc from calling DestroyWindow.
            return 0;
        }

        ffi::WM_DESTROY => {
            push_event!(PlatformEvent::WindowDestroyed { handle });
        }

        ffi::WM_SIZE => {
            let width = ffi::loword(lp as usize) as u32;
            let height = ffi::hiword(lp as usize) as u32;
            match wp {
                ffi::SIZE_MINIMIZED => {
                    push_event!(PlatformEvent::WindowMinimized { handle });
                }
                ffi::SIZE_MAXIMIZED => {
                    push_event!(PlatformEvent::WindowMaximized { handle });
                    push_event!(PlatformEvent::WindowResized {
                        handle,
                        width,
                        height,
                    });
                }
                _ => {
                    push_event!(PlatformEvent::WindowResized {
                        handle,
                        width,
                        height,
                    });
                }
            }
        }

        ffi::WM_MOVE => {
            let x = ffi::get_x_lparam(lp);
            let y = ffi::get_y_lparam(lp);
            push_event!(PlatformEvent::WindowMoved { handle, x, y });
        }

        ffi::WM_PAINT => {
            // The off-screen GDI back-buffer is the authoritative copy of the
            // last presented frame. When the compositor / RDP client asks us to
            // repaint (occlusion, focus change, remote refresh) we MUST blit it
            // back, otherwise the update region keeps whatever the compositor
            // last sampled — which right after a cursor-only partial-damage
            // present is a stale cursor, producing the mouse-move smear/trail
            // over RDP. Replaying the whole back-buffer makes every repaint
            // atomic and residue-free regardless of which present produced it.
            let mut ps = ffi::PAINTSTRUCT::default();
            // SAFETY: BeginPaint/EndPaint are safe on a valid HWND during
            // WM_PAINT. The PAINTSTRUCT is stack-allocated and zero-initialized.
            unsafe {
                let paint_dc = ffi::BeginPaint(hwnd, &mut ps);
                // Read the published back-buffer (set by the present path on the
                // same thread). `0` means no frame presented yet → just validate.
                let bb_dc = wd
                    .backbuffer_dc
                    .load(std::sync::atomic::Ordering::Acquire)
                    as ffi::HDC;
                if !paint_dc.is_null() && !bb_dc.is_null() {
                    let w = wd
                        .backbuffer_w
                        .load(std::sync::atomic::Ordering::Relaxed);
                    let h = wd
                        .backbuffer_h
                        .load(std::sync::atomic::Ordering::Relaxed);
                    if w != 0 && h != 0 {
                        // One BitBlt of the whole authoritative frame: atomic
                        // from the compositor's view, no partial/stale region.
                        ffi::BitBlt(
                            paint_dc,
                            0,
                            0,
                            w as i32,
                            h as i32,
                            bb_dc,
                            0,
                            0,
                            ffi::SRCCOPY,
                        );
                    }
                }
                ffi::EndPaint(hwnd, &ps);
            }
            push_event!(PlatformEvent::WindowRedraw { handle });
            return 0;
        }

        ffi::WM_ERASEBKGND => {
            // Suppress background erase -- we paint the entire client area.
            return 1;
        }

        ffi::WM_SETCURSOR => {
            // Over the client area, use our stored cursor handle.
            // If the handle is non-null, show the hardware cursor.
            // If null, hide it (software cursor mode).
            if (lp & 0xFFFF) as i32 == ffi::HTCLIENT {
                let cursor_val = wd.cursor.load(std::sync::atomic::Ordering::Relaxed);
                // SAFETY: SetCursor accepts any cursor handle (or null to hide).
                unsafe {
                    ffi::SetCursor(cursor_val as ffi::HCURSOR);
                }
                return 1; // Handled
            }
        }

        ffi::WM_SETFOCUS => {
            push_event!(PlatformEvent::FocusGained { handle });
        }

        ffi::WM_KILLFOCUS => {
            push_event!(PlatformEvent::FocusLost { handle });
        }

        ffi::WM_KEYDOWN | ffi::WM_SYSKEYDOWN => {
            let vk = wp as u32;
            let scancode = ((lp as u32) >> 16) & 0x1FF; // bits 16-24
            let repeat = (lp & 0x40000000) != 0;
            if let Some(key) = input::vk_to_keycode(vk) {
                let state = if repeat {
                    KeyState::Repeat
                } else {
                    KeyState::Pressed
                };
                let mods = input::modifiers_from_state();
                let event = KeyEvent::new(key, state, mods, scancode, timestamp_us());
                push_event!(PlatformEvent::KeyInput { handle, event });
            }
            // Let DefWindowProc handle Alt+F4 etc. for SYSKEYDOWN.
            if msg == ffi::WM_SYSKEYDOWN {
                // SAFETY: DefWindowProcW is safe for default handling of system key messages.
                return unsafe { ffi::DefWindowProcW(hwnd, msg, wp, lp) };
            }
            return 0;
        }

        ffi::WM_KEYUP | ffi::WM_SYSKEYUP => {
            let vk = wp as u32;
            let scancode = ((lp as u32) >> 16) & 0x1FF;
            if let Some(key) = input::vk_to_keycode(vk) {
                let mods = input::modifiers_from_state();
                let event = KeyEvent::new(key, KeyState::Released, mods, scancode, timestamp_us());
                push_event!(PlatformEvent::KeyInput { handle, event });
            }
            if msg == ffi::WM_SYSKEYUP {
                // SAFETY: DefWindowProcW is safe for default handling of system key messages.
                return unsafe { ffi::DefWindowProcW(hwnd, msg, wp, lp) };
            }
            return 0;
        }

        ffi::WM_MOUSEMOVE => {
            let x = ffi::get_x_lparam(lp) as f32;
            let y = ffi::get_y_lparam(lp) as f32;
            push_event!(PlatformEvent::MouseInput {
                handle,
                event: MouseEvent::Move { x, y },
            });
            return 0;
        }

        ffi::WM_LBUTTONDOWN => {
            let x = ffi::get_x_lparam(lp) as f32;
            let y = ffi::get_y_lparam(lp) as f32;
            push_event!(PlatformEvent::MouseInput {
                handle,
                event: MouseEvent::Button {
                    button: MouseButton::Left,
                    state: ButtonState::Pressed,
                    x,
                    y,
                },
            });
            // SAFETY: SetCapture is safe on a valid HWND to capture mouse input.
            unsafe {
                ffi::SetCapture(hwnd);
            }
            return 0;
        }

        ffi::WM_LBUTTONUP => {
            let x = ffi::get_x_lparam(lp) as f32;
            let y = ffi::get_y_lparam(lp) as f32;
            push_event!(PlatformEvent::MouseInput {
                handle,
                event: MouseEvent::Button {
                    button: MouseButton::Left,
                    state: ButtonState::Released,
                    x,
                    y,
                },
            });
            // SAFETY: ReleaseCapture is always safe to call.
            unsafe {
                ffi::ReleaseCapture();
            }
            return 0;
        }

        ffi::WM_RBUTTONDOWN => {
            let x = ffi::get_x_lparam(lp) as f32;
            let y = ffi::get_y_lparam(lp) as f32;
            push_event!(PlatformEvent::MouseInput {
                handle,
                event: MouseEvent::Button {
                    button: MouseButton::Right,
                    state: ButtonState::Pressed,
                    x,
                    y,
                },
            });
            return 0;
        }

        ffi::WM_RBUTTONUP => {
            let x = ffi::get_x_lparam(lp) as f32;
            let y = ffi::get_y_lparam(lp) as f32;
            push_event!(PlatformEvent::MouseInput {
                handle,
                event: MouseEvent::Button {
                    button: MouseButton::Right,
                    state: ButtonState::Released,
                    x,
                    y,
                },
            });
            return 0;
        }

        ffi::WM_MBUTTONDOWN => {
            let x = ffi::get_x_lparam(lp) as f32;
            let y = ffi::get_y_lparam(lp) as f32;
            push_event!(PlatformEvent::MouseInput {
                handle,
                event: MouseEvent::Button {
                    button: MouseButton::Middle,
                    state: ButtonState::Pressed,
                    x,
                    y,
                },
            });
            return 0;
        }

        ffi::WM_MBUTTONUP => {
            let x = ffi::get_x_lparam(lp) as f32;
            let y = ffi::get_y_lparam(lp) as f32;
            push_event!(PlatformEvent::MouseInput {
                handle,
                event: MouseEvent::Button {
                    button: MouseButton::Middle,
                    state: ButtonState::Released,
                    x,
                    y,
                },
            });
            return 0;
        }

        ffi::WM_MOUSEWHEEL => {
            let delta = ffi::get_wheel_delta_wparam(wp) as f32 / ffi::WHEEL_DELTA as f32;
            // WM_MOUSEWHEEL coordinates are in screen space; convert to client.
            let mut pt = ffi::POINT {
                x: ffi::get_x_lparam(lp),
                y: ffi::get_y_lparam(lp),
            };
            // SAFETY: ScreenToClient is safe on a valid HWND with a valid POINT pointer.
            unsafe {
                ffi::ScreenToClient(hwnd, &mut pt);
            }
            push_event!(PlatformEvent::MouseInput {
                handle,
                event: MouseEvent::Scroll {
                    axis: ScrollAxis::Vertical,
                    delta,
                    x: pt.x as f32,
                    y: pt.y as f32,
                },
            });
            return 0;
        }

        ffi::WM_MOUSEHWHEEL => {
            let delta = ffi::get_wheel_delta_wparam(wp) as f32 / ffi::WHEEL_DELTA as f32;
            let mut pt = ffi::POINT {
                x: ffi::get_x_lparam(lp),
                y: ffi::get_y_lparam(lp),
            };
            // SAFETY: ScreenToClient is safe on a valid HWND with a valid POINT pointer.
            unsafe {
                ffi::ScreenToClient(hwnd, &mut pt);
            }
            push_event!(PlatformEvent::MouseInput {
                handle,
                event: MouseEvent::Scroll {
                    axis: ScrollAxis::Horizontal,
                    delta,
                    x: pt.x as f32,
                    y: pt.y as f32,
                },
            });
            return 0;
        }

        ffi::WM_DPICHANGED => {
            let new_dpi = ffi::loword(wp) as f32;
            let dpi_scale = new_dpi / 96.0;
            // Persist the live scale where window state lives so that any path
            // reading WindowInfo::dpi_scale (e.g. the present-path log) sees the
            // current value, and notify the session via the event.
            wd.dpi_scale_bits
                .store(dpi_scale.to_bits(), std::sync::atomic::Ordering::Relaxed);
            push_event!(PlatformEvent::DpiChanged { handle, dpi_scale });
            // Move / resize window to the suggested rectangle.
            if lp != 0 {
                // SAFETY: lp is non-zero, so it points to the suggested RECT
                // provided by the OS in the WM_DPICHANGED message.
                let suggested = unsafe { &*(lp as *const ffi::RECT) };
                // SAFETY: SetWindowPos is safe on a valid HWND with valid parameters.
                unsafe {
                    ffi::SetWindowPos(
                        hwnd,
                        ffi::HWND_TOP,
                        suggested.left,
                        suggested.top,
                        suggested.right - suggested.left,
                        suggested.bottom - suggested.top,
                        ffi::SWP_NOZORDER,
                    );
                }
            }
            return 0;
        }

        _ => {}
    }

    // SAFETY: DefWindowProcW handles any unprocessed message on a valid HWND.
    unsafe { ffi::DefWindowProcW(hwnd, msg, wp, lp) }
}

// ---------------------------------------------------------------------------
// Window info stored on the Rust side
// ---------------------------------------------------------------------------

/// Read a window's effective DPI scale (1.0 == 96 DPI baseline).
///
/// Uses `GetDeviceCaps(GetDC(hwnd), LOGPIXELSX)`, which reflects the DPI of the
/// monitor the window is on under the process's DPI-awareness context. This is
/// the value the session layer needs to convert physical mouse coordinates back
/// into the logical (CSS-pixel) coordinate space the layout is authored in.
///
/// Returns `1.0` on any failure (e.g. `GetDC` returns null) so callers always
/// have a sane scale.
fn query_window_dpi_scale(hwnd: ffi::HWND) -> f32 {
    if hwnd.is_null() {
        return 1.0;
    }
    // SAFETY: GetDC/GetDeviceCaps/ReleaseDC are standard GDI calls on a valid
    // HWND. We release the DC immediately after reading the DPI.
    unsafe {
        let hdc = ffi::GetDC(hwnd);
        if hdc.is_null() {
            return 1.0;
        }
        let dpi = ffi::GetDeviceCaps(hdc, ffi::LOGPIXELSX);
        ffi::ReleaseDC(hwnd, hdc);
        if dpi > 0 { dpi as f32 / 96.0 } else { 1.0 }
    }
}

/// Metadata for a window managed by the platform backend.
#[allow(dead_code)]
/// Per-window off-screen GDI back-buffer used by the `SetDIBitsToDevice` /
/// `BitBlt` present path.
///
/// The GDI fallback path runs whenever there is no hardware DXGI swap chain —
/// which is *always* the case over RDP. Writing the framebuffer directly to the
/// visible window DC (the old behaviour) let the remote/DWM compositor sample
/// the DC mid-write, producing partial-frame tearpoints and visible flicker.
///
/// This buffer is a DIB section backed compatible DC: we copy the frame into the
/// off-screen DIB memory, then a *single* `BitBlt` flips it onto the window DC.
/// From the compositor's perspective the on-screen pixels change in one atomic
/// blit rather than row-by-row, eliminating the mid-write tearing.
///
/// The buffer is created on first present (or after a resize) and destroyed when
/// the window is destroyed (via `Drop`). It is recreated whenever the requested
/// frame size changes.
struct GdiBackBuffer {
    /// Memory DC compatible with the window, holding `bitmap` selected in.
    mem_dc: ffi::HDC,
    /// DIB section bitmap selected into `mem_dc`.
    bitmap: ffi::HBITMAP,
    /// Object originally selected in `mem_dc`, restored before deletion.
    old_bitmap: ffi::HGDIOBJ,
    /// Pointer to the DIB section's pixel memory (top-down BGRA8, packed).
    bits: *mut c_void,
    /// Width of the back-buffer in pixels.
    width: u32,
    /// Height of the back-buffer in pixels.
    height: u32,
}

impl GdiBackBuffer {
    /// Create an off-screen DIB-section back-buffer of `width` x `height`
    /// compatible with `window_hdc`. Returns `None` if any GDI allocation fails
    /// (the caller falls back to direct presentation for that frame).
    ///
    /// # Safety
    ///
    /// `window_hdc` must be a valid DC for the target window.
    unsafe fn create(window_hdc: ffi::HDC, width: u32, height: u32) -> Option<Self> {
        if width == 0 || height == 0 {
            return None;
        }

        // SAFETY: window_hdc is a valid DC supplied by the caller.
        let mem_dc = unsafe { ffi::CreateCompatibleDC(window_hdc) };
        if mem_dc.is_null() {
            return None;
        }

        let bmi = ffi::BITMAPINFO {
            bmiHeader: ffi::BITMAPINFOHEADER {
                biSize: std::mem::size_of::<ffi::BITMAPINFOHEADER>() as ffi::DWORD,
                biWidth: width as ffi::LONG,
                // Negative height → top-down DIB (row 0 is the top row), so our
                // top-down BGRA framebuffer copies in without a vertical flip.
                biHeight: -(height as ffi::LONG),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: ffi::BI_RGB,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [ffi::RGBQUAD::default()],
        };

        let mut bits: *mut c_void = ptr::null_mut();
        // SAFETY: mem_dc is valid; bmi is a fully-initialized BITMAPINFO; bits
        // receives the DIB memory pointer. hSection null → GDI owns the memory.
        let bitmap = unsafe {
            ffi::CreateDIBSection(
                mem_dc,
                &bmi,
                ffi::DIB_RGB_COLORS,
                &mut bits,
                ptr::null_mut(),
                0,
            )
        };
        if bitmap.is_null() || bits.is_null() {
            // SAFETY: mem_dc was created above and not yet selected into.
            unsafe {
                ffi::DeleteDC(mem_dc);
            }
            return None;
        }

        // SAFETY: both handles are valid; SelectObject returns the previously
        // selected bitmap so we can restore it before deleting the DC.
        let old_bitmap = unsafe { ffi::SelectObject(mem_dc, bitmap) };

        Some(GdiBackBuffer {
            mem_dc,
            bitmap,
            old_bitmap,
            bits,
            width,
            height,
        })
    }
}

impl Drop for GdiBackBuffer {
    fn drop(&mut self) {
        // SAFETY: restore the original bitmap before deleting ours, then delete
        // both the bitmap and the memory DC. All handles were created by us and
        // are not used elsewhere.
        unsafe {
            if !self.mem_dc.is_null() {
                if !self.old_bitmap.is_null() {
                    ffi::SelectObject(self.mem_dc, self.old_bitmap);
                }
                if !self.bitmap.is_null() {
                    ffi::DeleteObject(self.bitmap);
                }
                ffi::DeleteDC(self.mem_dc);
            }
        }
    }
}

struct WindowInfo {
    hwnd: ffi::HWND,
    handle: NativeWindowHandle,
    _data: Box<WindowData>,
    /// DXGI swap-chain presenter (lazily initialized on first present).
    dxgi: Option<dxgi::DxgiPresenter>,
    /// Off-screen GDI back-buffer for the `SetDIBitsToDevice`/`BitBlt` present
    /// path (created on first GDI present, recreated on resize). `None` until
    /// the first GDI present or when the DXGI path is active.
    gdi_back_buffer: Option<GdiBackBuffer>,
    /// Set once the first present path (DXGI vs GDI/WARP fallback) has been
    /// logged, so the diagnostic line is emitted exactly once per window.
    present_path_logged: bool,
    /// RDP-aware present-cadence coalescer. Lazily created on the first GDI
    /// present once the remote-session state and cap are known. `None` means a
    /// full per-frame present (local default) until the first GDI present
    /// initializes it.
    remote_coalescer: Option<RemotePresentCoalescer>,
}

impl WindowInfo {
    /// Current DPI scale for this window (1.0 == 96 DPI). Initialized from the
    /// window's actual DPI at creation and updated on `WM_DPICHANGED` (the
    /// wndproc writes the live value into `WindowData::dpi_scale_bits`). The
    /// session layer also tracks this via the `DpiChanged` event to map
    /// physical mouse coordinates into logical layout space.
    #[allow(dead_code)]
    fn dpi_scale(&self) -> f32 {
        f32::from_bits(
            self._data
                .dpi_scale_bits
                .load(std::sync::atomic::Ordering::Relaxed),
        )
    }
}

// ---------------------------------------------------------------------------
// Win32 display backend
// ---------------------------------------------------------------------------

/// Display backend that queries monitor information via Win32 APIs.
struct Win32DisplayBackend;

impl Win32DisplayBackend {
    fn enumerate_monitors(&self) -> Vec<MonitorInfo> {
        let mut result: Vec<MonitorInfo> = Vec::new();

        // Safety: EnumDisplayMonitors calls our callback for each monitor.
        // The callback pushes a `MonitorInfo` for each monitor into the Vec
        // via the LPARAM pointer.
        unsafe {
            ffi::EnumDisplayMonitors(
                ptr::null_mut(),
                ptr::null(),
                Some(monitor_enum_callback),
                &mut result as *mut Vec<MonitorInfo> as ffi::LPARAM,
            );
        }
        result
    }
}

/// Callback invoked by `EnumDisplayMonitors` for each connected monitor.
///
/// # Safety
///
/// `lparam` must point to a valid `Vec<MonitorInfo>` whose lifetime spans the
/// entire `EnumDisplayMonitors` call. This is guaranteed by `enumerate_monitors`.
unsafe extern "system" fn monitor_enum_callback(
    hmonitor: ffi::HMONITOR,
    _hdc: ffi::HDC,
    _lprc: *mut ffi::RECT,
    lparam: ffi::LPARAM,
) -> ffi::BOOL {
    // SAFETY: lparam is a valid pointer to Vec<MonitorInfo>, guaranteed by the caller.
    let monitors = unsafe { &mut *(lparam as *mut Vec<MonitorInfo>) };

    let mut mi = ffi::MONITORINFOEXW::default();
    mi.base.cbSize = std::mem::size_of::<ffi::MONITORINFOEXW>() as ffi::DWORD;

    // SAFETY: GetMonitorInfoW is safe with a valid HMONITOR and properly sized struct.
    if unsafe { ffi::GetMonitorInfoW(hmonitor, &mut mi) } == 0 {
        return ffi::TRUE;
    }

    let id = monitors.len() as u32;
    let name = from_wide(&mi.szDevice);
    let primary = (mi.base.dwFlags & ffi::MONITORINFOF_PRIMARY) != 0;
    let refresh_rate_hz = query_refresh_rate_hz(&mi.szDevice).unwrap_or(60);
    let dpi_scale = query_monitor_dpi_scale(hmonitor);

    let geometry = Rect::new(
        mi.base.rcMonitor.left as f32,
        mi.base.rcMonitor.top as f32,
        (mi.base.rcMonitor.right - mi.base.rcMonitor.left) as f32,
        (mi.base.rcMonitor.bottom - mi.base.rcMonitor.top) as f32,
    );
    let work_area = Rect::new(
        mi.base.rcWork.left as f32,
        mi.base.rcWork.top as f32,
        (mi.base.rcWork.right - mi.base.rcWork.left) as f32,
        (mi.base.rcWork.bottom - mi.base.rcWork.top) as f32,
    );

    monitors.push(MonitorInfo {
        id,
        name,
        geometry,
        work_area,
        dpi_scale,
        primary,
        refresh_rate_hz,
    });

    ffi::TRUE
}

/// Query a monitor's effective DPI scale via `GetDpiForMonitor` (shcore).
///
/// Returns the scale relative to the 96-DPI baseline (1.0 == 100%, 1.5 == 150%,
/// 2.0 == 200%). Falls back to `1.0` if the call fails — e.g. on a Windows
/// version without per-monitor DPI, or for a monitor that has been disconnected
/// between enumeration and the query. This is the per-monitor scale the session
/// needs to convert physical coordinates into logical layout space on each
/// display independently.
fn query_monitor_dpi_scale(hmonitor: ffi::HMONITOR) -> f32 {
    if hmonitor.is_null() {
        return 1.0;
    }
    let mut dpi_x: ffi::UINT = 0;
    let mut dpi_y: ffi::UINT = 0;
    // SAFETY: GetDpiForMonitor accepts a valid HMONITOR and two writable
    // UINT out-pointers. On failure it returns a non-zero HRESULT and we ignore
    // the (untouched) outputs.
    let hr = unsafe {
        ffi::GetDpiForMonitor(hmonitor, ffi::MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y)
    };
    if hr == 0 && dpi_x > 0 {
        dpi_x as f32 / 96.0
    } else {
        1.0
    }
}

impl DisplayBackend for Win32DisplayBackend {
    fn monitors(&self) -> Vec<MonitorInfo> {
        self.enumerate_monitors()
    }

    fn primary_monitor(&self) -> Option<MonitorInfo> {
        self.enumerate_monitors().into_iter().find(|m| m.primary)
    }

    fn virtual_screen_rect(&self) -> Rect {
        // The virtual screen is the union of ALL monitors (which can start at a
        // negative origin when a secondary monitor sits left of / above the
        // primary), not just the primary monitor's resolution. Using
        // SM_*VIRTUALSCREEN keeps point→monitor hit-testing and multi-monitor
        // window placement correct.
        // SAFETY: GetSystemMetrics is always safe to call.
        let x = unsafe { ffi::GetSystemMetrics(ffi::SM_XVIRTUALSCREEN) };
        let y = unsafe { ffi::GetSystemMetrics(ffi::SM_YVIRTUALSCREEN) };
        let w = unsafe { ffi::GetSystemMetrics(ffi::SM_CXVIRTUALSCREEN) };
        let h = unsafe { ffi::GetSystemMetrics(ffi::SM_CYVIRTUALSCREEN) };
        if w > 0 && h > 0 {
            Rect::new(x as f32, y as f32, w as f32, h as f32)
        } else {
            // Fallback for the rare case the virtual-screen metrics are
            // unavailable: use the primary monitor's resolution.
            let pw = unsafe { ffi::GetSystemMetrics(ffi::SM_CXSCREEN) };
            let ph = unsafe { ffi::GetSystemMetrics(ffi::SM_CYSCREEN) };
            Rect::new(0.0, 0.0, pw as f32, ph as f32)
        }
    }
}

// Safety: Win32DisplayBackend has no mutable state; all queries call
// thread-safe Win32 APIs.
unsafe impl Send for Win32DisplayBackend {}

pub fn refresh_rate_hz_from_devmode_frequency(frequency: u32) -> Option<u32> {
    (frequency > 1).then_some(frequency)
}

fn query_refresh_rate_hz(device_name: &[u16; ffi::CCHDEVICENAME]) -> Option<u32> {
    let mut dev_mode = ffi::DEVMODEW::default();
    dev_mode.dmSize = std::mem::size_of::<ffi::DEVMODEW>() as ffi::WORD;

    // SAFETY: device_name is a valid monitor device string returned by
    // GetMonitorInfoW and dev_mode is a properly sized output buffer.
    let result = unsafe {
        ffi::EnumDisplaySettingsW(
            device_name.as_ptr(),
            ffi::ENUM_CURRENT_SETTINGS,
            &mut dev_mode,
        )
    };

    (result != 0)
        .then_some(dev_mode.dmDisplayFrequency)
        .and_then(refresh_rate_hz_from_devmode_frequency)
}

#[derive(Debug, Default)]
struct Win32PresentFeedbackState {
    submitted_present_count: u64,
    feedback_queue: VecDeque<PresentFeedback>,
}

impl Win32PresentFeedbackState {
    fn record_accepted_present(&mut self, timestamp_ns: u64) {
        self.submitted_present_count = self.submitted_present_count.saturating_add(1);
        let acknowledged_present_count = self.submitted_present_count;

        self.feedback_queue.push_back(PresentFeedback {
            acknowledged_present_count,
            sequence: (acknowledged_present_count <= u32::MAX as u64)
                .then_some(acknowledged_present_count as u32),
            timestamp_ns: Some(timestamp_ns),
            crtc_id: None,
        });
    }

    fn take_feedback(&mut self) -> Option<PresentFeedback> {
        self.feedback_queue.pop_front()
    }
}

// ---------------------------------------------------------------------------
// Win32 window host
// ---------------------------------------------------------------------------

/// Convert a desired CLIENT-area size into the OUTER window size that
/// `CreateWindowExW` needs so that the resulting client area matches the
/// request, accounting for the window's title bar / borders.
///
/// Uses `AdjustWindowRectEx`; on failure (it should not fail for valid styles)
/// it falls back to the requested size unchanged. Inputs are clamped to be at
/// least 1×1. Returns `(window_width, window_height)`.
fn client_size_to_window_size(
    client_width: i32,
    client_height: i32,
    style: ffi::DWORD,
    ex_style: ffi::DWORD,
) -> (i32, i32) {
    let cw = client_width.max(1);
    let ch = client_height.max(1);
    let mut rect = ffi::RECT {
        left: 0,
        top: 0,
        right: cw,
        bottom: ch,
    };
    // SAFETY: AdjustWindowRectEx only reads/writes the provided RECT and takes
    // the style flags by value. `rect` is a valid stack-allocated RECT.
    let ok = unsafe { ffi::AdjustWindowRectEx(&mut rect, style, ffi::FALSE, ex_style) };
    if ok == 0 {
        // Should not happen for valid styles; keep the requested size so the
        // window is at least created.
        return (cw, ch);
    }
    let w = (rect.right - rect.left).max(1);
    let h = (rect.bottom - rect.top).max(1);
    (w, h)
}

/// Window host implementation backed by `CreateWindowExW`.
struct Win32WindowHost {
    /// Map from our `NativeWindowHandle` to the Win32 HWND and metadata.
    windows: HashMap<u64, WindowInfo>,
    /// Monotonically increasing counter for handle generation.
    next_handle: u64,
    /// HINSTANCE used for window creation.
    hinstance: ffi::HINSTANCE,
    /// Class name (wide) kept alive for `CreateWindowExW`.
    class_name: Vec<u16>,
    /// Shared event queue (raw pointer because `WindowData` needs it).
    event_queue: *const UnsafeCell<VecDeque<PlatformEvent>>,
}

// Safety: Win32WindowHost is !Send by default due to raw pointers. However,
// the struct (and the platform that owns it) is only accessed from the thread
// that created the message loop, which is the same thread that created the
// windows. We enforce this structurally.
unsafe impl Send for Win32WindowHost {}

impl NativeWindowHost for Win32WindowHost {
    fn create_window(&mut self, params: NativeWindowParams) -> PlatformResult<NativeWindowHandle> {
        let handle = NativeWindowHandle(self.next_handle);
        self.next_handle += 1;

        let title = to_wide(&params.title);

        // Choose window style based on window_type:
        //   "desktop" → borderless popup covering the full screen
        //   anything else → normal overlapped window
        let is_desktop = params.window_type == "desktop";

        let (style, ex_style) = if is_desktop {
            (ffi::WS_POPUP | ffi::WS_VISIBLE, ffi::WS_EX_APPWINDOW)
        } else {
            (ffi::WS_OVERLAPPEDWINDOW, ffi::WS_EX_APPWINDOW)
        };

        let (x, y, w, h) = if is_desktop {
            // Full primary screen dimensions.
            // SAFETY: GetSystemMetrics is always safe to call.
            let sw = unsafe { ffi::GetSystemMetrics(ffi::SM_CXSCREEN) };
            let sh = unsafe { ffi::GetSystemMetrics(ffi::SM_CYSCREEN) };
            (0, 0, sw, sh)
        } else {
            let x = if params.geometry.x == 0.0 && params.geometry.y == 0.0 {
                ffi::CW_USEDEFAULT
            } else {
                params.geometry.x as i32
            };
            let y = if params.geometry.x == 0.0 && params.geometry.y == 0.0 {
                ffi::CW_USEDEFAULT
            } else {
                params.geometry.y as i32
            };
            // The geometry width/height the caller requests is the desired
            // CLIENT area (the surface the compositor renders into). Win32's
            // CreateWindowExW takes the OUTER window size, so for a normal
            // overlapped window the title bar + borders would eat into the
            // client area, leaving it smaller than requested. DXGI would then
            // stretch the rendered frame into the smaller client rect, which
            // shimmers on every present (t55 flicker fix, H2). Expand the
            // requested client size to the matching outer window size via
            // AdjustWindowRectEx so the client area == the requested size.
            let (w, h) = if params.geometry.width > 0.0 && params.geometry.height > 0.0 {
                client_size_to_window_size(
                    params.geometry.width as i32,
                    params.geometry.height as i32,
                    style,
                    ex_style,
                )
            } else {
                (ffi::CW_USEDEFAULT, ffi::CW_USEDEFAULT)
            };
            (x, y, w, h)
        };

        let parent = match params.parent {
            Some(p) => {
                // Look up the HWND of the parent.
                self.windows
                    .get(&p.0)
                    .map(|info| info.hwnd)
                    .unwrap_or(ptr::null_mut())
            }
            None => ptr::null_mut(),
        };

        // Allocate per-window data on the heap.
        // Load the default arrow cursor for the hardware cursor.
        // SAFETY: LoadCursorW with null hInstance loads a system cursor.
        let default_cursor = unsafe { ffi::LoadCursorW(ptr::null_mut(), ffi::IDC_ARROW) };
        let data = Box::new(WindowData {
            handle,
            event_queue: self.event_queue,
            cursor: std::sync::atomic::AtomicIsize::new(default_cursor as isize),
            // Seeded with the actual DPI just below (after the HWND exists).
            dpi_scale_bits: std::sync::atomic::AtomicU32::new(1.0f32.to_bits()),
            // No back-buffer until the first GDI present publishes one.
            backbuffer_dc: std::sync::atomic::AtomicIsize::new(0),
            backbuffer_w: std::sync::atomic::AtomicU32::new(0),
            backbuffer_h: std::sync::atomic::AtomicU32::new(0),
        });

        // Safety: CreateWindowExW creates a native Win32 window. All
        // parameters are valid: the class was registered in `new()`, the
        // hinstance is the process module handle, and the title is a
        // null-terminated wide string.
        let hwnd = unsafe {
            ffi::CreateWindowExW(
                ex_style,
                self.class_name.as_ptr(),
                title.as_ptr(),
                style,
                x,
                y,
                w,
                h,
                parent,
                ptr::null_mut(),
                self.hinstance,
                ptr::null_mut(),
            )
        };

        if hwnd.is_null() {
            return Err(PlatformError::Window(format!(
                "CreateWindowExW failed (error {})",
                unsafe { ffi::GetLastError() }
            )));
        }

        // Store the WindowData pointer in the window's user-data slot so that
        // wndproc can access the event queue.
        // Safety: `data` is heap-allocated and will remain valid until
        // `destroy_window` deallocates it.
        let data_ptr = &*data as *const WindowData;
        unsafe {
            ffi::SetWindowLongPtrW(hwnd, ffi::GWLP_USERDATA, data_ptr as ffi::LONG_PTR);
        }

        // Show the window.
        // SAFETY: ShowWindow and UpdateWindow are safe on a valid HWND.
        unsafe {
            ffi::ShowWindow(hwnd, ffi::SW_SHOW);
            ffi::UpdateWindow(hwnd);
        }

        // Read the window's actual DPI at creation so the session starts with
        // the correct coordinate scale rather than assuming 96 DPI. Win32 does
        // not reliably send WM_DPICHANGED for the initial DPI, so we seed it
        // here (into WindowData, which the wndproc updates) and emit a
        // DpiChanged event below.
        let dpi_scale = query_window_dpi_scale(hwnd);
        data.dpi_scale_bits
            .store(dpi_scale.to_bits(), std::sync::atomic::Ordering::Relaxed);

        let info = WindowInfo {
            hwnd,
            handle,
            _data: data,
            dxgi: None,
            gdi_back_buffer: None,
            present_path_logged: false,
            remote_coalescer: None,
        };
        self.windows.insert(handle.0, info);

        // Push a WindowCreated event.
        let mut rc = ffi::RECT::default();
        // SAFETY: GetClientRect is safe on a valid HWND with a valid RECT pointer.
        unsafe {
            ffi::GetClientRect(hwnd, &mut rc);
        }
        let width = (rc.right - rc.left) as u32;
        let height = (rc.bottom - rc.top) as u32;
        // Safety: the event_queue pointer is valid for the lifetime of
        // the platform.
        unsafe {
            (*(*self.event_queue).get()).push_back(PlatformEvent::WindowCreated {
                handle,
                width,
                height,
            });
            // Seed the session's DPI scale from the window's actual DPI. On a
            // scaled display this is != 1.0; without it the first frames of
            // input would be unscaled and clicks would miss (e6 DPI input bug).
            if (dpi_scale - 1.0).abs() > f32::EPSILON {
                (*(*self.event_queue).get())
                    .push_back(PlatformEvent::DpiChanged { handle, dpi_scale });
            }
        }

        Ok(handle)
    }

    fn destroy_window(&mut self, handle: NativeWindowHandle) -> PlatformResult<()> {
        if let Some(info) = self.windows.remove(&handle.0) {
            // Clear user-data before destroying so wndproc won't
            // dereference a dangling pointer during teardown messages.
            // SAFETY: SetWindowLongPtrW and DestroyWindow are safe on a valid HWND.
            // We clear user-data first to prevent the wndproc from using stale pointers.
            unsafe {
                ffi::SetWindowLongPtrW(info.hwnd, ffi::GWLP_USERDATA, 0);
                ffi::DestroyWindow(info.hwnd);
            }
            // `info._data` (the Box<WindowData>) is dropped here.
        }
        Ok(())
    }

    fn set_geometry(&mut self, handle: NativeWindowHandle, geometry: Rect) -> PlatformResult<()> {
        if let Some(info) = self.windows.get(&handle.0) {
            // Safety: MoveWindow is safe to call on a valid HWND.
            unsafe {
                ffi::MoveWindow(
                    info.hwnd,
                    geometry.x as i32,
                    geometry.y as i32,
                    geometry.width as i32,
                    geometry.height as i32,
                    ffi::TRUE,
                );
            }
        }
        Ok(())
    }

    fn set_title(&mut self, handle: NativeWindowHandle, title: &str) -> PlatformResult<()> {
        if let Some(info) = self.windows.get(&handle.0) {
            let wide = to_wide(title);
            // Safety: SetWindowTextW is safe on a valid HWND with a
            // null-terminated wide string.
            unsafe {
                ffi::SetWindowTextW(info.hwnd, wide.as_ptr());
            }
        }
        Ok(())
    }

    fn set_icon(&mut self, _handle: NativeWindowHandle, _icon_data: &[u8]) -> PlatformResult<()> {
        // Setting a window icon from raw pixel data requires creating an
        // HICON via CreateIconIndirect, which is non-trivial. For now we
        // accept the call but do nothing.
        Ok(())
    }

    fn set_state(&mut self, handle: NativeWindowHandle, state: &str) -> PlatformResult<()> {
        if let Some(info) = self.windows.get(&handle.0) {
            let cmd = match state {
                "maximized" => ffi::SW_MAXIMIZE,
                "minimized" => ffi::SW_MINIMIZE,
                "restored" | "normal" => ffi::SW_RESTORE,
                "hidden" => ffi::SW_HIDE,
                _ => ffi::SW_SHOW,
            };
            // SAFETY: ShowWindow is safe on a valid HWND.
            unsafe {
                ffi::ShowWindow(info.hwnd, cmd);
            }
        }
        Ok(())
    }

    fn set_z_order(&mut self, handle: NativeWindowHandle, z_order: i32) -> PlatformResult<()> {
        if let Some(info) = self.windows.get(&handle.0) {
            let insert_after = if z_order > 0 {
                ffi::HWND_TOPMOST
            } else {
                ffi::HWND_TOP
            };
            // SAFETY: SetWindowPos is safe on a valid HWND. SWP_NOMOVE | SWP_NOSIZE
            // means only the Z-order is changed.
            unsafe {
                ffi::SetWindowPos(
                    info.hwnd,
                    insert_after,
                    0,
                    0,
                    0,
                    0,
                    ffi::SWP_NOMOVE | ffi::SWP_NOSIZE,
                );
            }
        }
        Ok(())
    }

    fn set_focus(&mut self, handle: NativeWindowHandle) -> PlatformResult<()> {
        if let Some(info) = self.windows.get(&handle.0) {
            // SAFETY: SetForegroundWindow is safe on a valid HWND.
            unsafe {
                ffi::SetForegroundWindow(info.hwnd);
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Win32 taskbar integration (minimal stub)
// ---------------------------------------------------------------------------

/// Minimal taskbar integration.
///
/// Full ITaskbarList3 integration requires COM; this implementation
/// silently accepts all calls.
struct Win32Taskbar;

impl TaskbarIntegration for Win32Taskbar {
    fn set_progress(&mut self, _handle: u64, _progress: f64) -> PlatformResult<()> {
        // Requires ITaskbarList3 (COM). Stubbed.
        Ok(())
    }

    fn set_overlay_icon(&mut self, _handle: u64, _icon_data: &[u8]) -> PlatformResult<()> {
        Ok(())
    }

    fn set_badge_count(&mut self, _count: u32) -> PlatformResult<()> {
        Ok(())
    }

    fn add_jump_list_item(&mut self, _item: JumpListItem) -> PlatformResult<()> {
        Ok(())
    }
}

// SAFETY: Win32Taskbar is stateless and all methods are no-ops.
unsafe impl Send for Win32Taskbar {}

// ---------------------------------------------------------------------------
// Win32 tray icon backend
// ---------------------------------------------------------------------------

/// System tray backend using `Shell_NotifyIconW`.
struct Win32Tray {
    /// Monotonically increasing tray icon ID.
    next_id: u64,
    /// Active tray icons keyed by our handle ID.
    icons: HashMap<u64, TrayIconInfo>,
    /// Hidden message-only window that receives tray callbacks.
    msg_hwnd: ffi::HWND,
}

struct TrayIconInfo {
    uid: u32,
}

// Safety: the struct is only used from the platform's owning thread.
unsafe impl Send for Win32Tray {}

impl Win32Tray {
    fn new(msg_hwnd: ffi::HWND) -> Self {
        Self {
            next_id: 1,
            icons: HashMap::new(),
            msg_hwnd,
        }
    }
}

impl NativeTray for Win32Tray {
    fn add_icon(&mut self, params: NativeTrayParams) -> PlatformResult<NativeTrayHandle> {
        let handle_id = self.next_id;
        self.next_id += 1;
        let uid = handle_id as u32;

        let mut nid = ffi::NOTIFYICONDATAW::default();
        nid.cbSize = std::mem::size_of::<ffi::NOTIFYICONDATAW>() as ffi::DWORD;
        nid.hWnd = self.msg_hwnd;
        nid.uID = uid;
        nid.uFlags = ffi::NIF_MESSAGE | ffi::NIF_TIP;
        nid.uCallbackMessage = ffi::WM_USER + 1;

        // Copy tooltip (up to 127 chars).
        let tip = to_wide(&params.tooltip);
        let copy_len = tip.len().min(127);
        nid.szTip[..copy_len].copy_from_slice(&tip[..copy_len]);

        // Safety: Shell_NotifyIconW with NIM_ADD adds a tray icon.
        let ok = unsafe { ffi::Shell_NotifyIconW(ffi::NIM_ADD, &mut nid) };
        if ok == 0 {
            return Err(PlatformError::Tray(
                "Shell_NotifyIconW NIM_ADD failed".into(),
            ));
        }

        self.icons.insert(handle_id, TrayIconInfo { uid });
        Ok(NativeTrayHandle(handle_id))
    }

    fn update_icon(&mut self, handle: NativeTrayHandle, update: TrayUpdate) -> PlatformResult<()> {
        let info = self
            .icons
            .get(&handle.0)
            .ok_or_else(|| PlatformError::Tray("unknown tray handle".into()))?;

        let mut nid = ffi::NOTIFYICONDATAW::default();
        nid.cbSize = std::mem::size_of::<ffi::NOTIFYICONDATAW>() as ffi::DWORD;
        nid.hWnd = self.msg_hwnd;
        nid.uID = info.uid;

        if let Some(ref tooltip) = update.tooltip {
            nid.uFlags |= ffi::NIF_TIP;
            let tip = to_wide(tooltip);
            let copy_len = tip.len().min(127);
            nid.szTip[..copy_len].copy_from_slice(&tip[..copy_len]);
        }

        // SAFETY: Shell_NotifyIconW with NIM_MODIFY updates an existing tray icon.
        // The NOTIFYICONDATAW struct is properly initialized.
        unsafe {
            ffi::Shell_NotifyIconW(ffi::NIM_MODIFY, &mut nid);
        }
        Ok(())
    }

    fn remove_icon(&mut self, handle: NativeTrayHandle) -> PlatformResult<()> {
        if let Some(info) = self.icons.remove(&handle.0) {
            let mut nid = ffi::NOTIFYICONDATAW::default();
            nid.cbSize = std::mem::size_of::<ffi::NOTIFYICONDATAW>() as ffi::DWORD;
            nid.hWnd = self.msg_hwnd;
            nid.uID = info.uid;
            // SAFETY: Shell_NotifyIconW with NIM_DELETE removes a tray icon.
            unsafe {
                ffi::Shell_NotifyIconW(ffi::NIM_DELETE, &mut nid);
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Win32 notifications (basic implementation)
// ---------------------------------------------------------------------------

/// Desktop notification backend.
///
/// Uses the balloon tip feature of `Shell_NotifyIconW` for a minimal
/// implementation. Full toast notifications require COM (Windows Runtime).
struct Win32Notifications {
    next_id: u32,
}

// SAFETY: Win32Notifications is only used from the platform's owning thread.
unsafe impl Send for Win32Notifications {}

impl Win32Notifications {
    fn new() -> Self {
        Self { next_id: 1 }
    }
}

impl NativeNotifications for Win32Notifications {
    fn show(&mut self, _params: NativeNotificationParams) -> PlatformResult<u32> {
        let id = self.next_id;
        self.next_id += 1;
        // Full toast notification requires WinRT / COM; return a unique ID
        // and silently drop the notification content.
        Ok(id)
    }

    fn dismiss(&mut self, _id: u32) -> PlatformResult<()> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Win32 keymap translator
// ---------------------------------------------------------------------------

/// Keymap translator that delegates to `input::scancode_to_keycode`.
struct Win32Keymap;

// SAFETY: Win32Keymap is stateless — safe to send between threads.
unsafe impl Send for Win32Keymap {}

impl KeymapTranslator for Win32Keymap {
    fn translate_scancode(&self, scancode: u32) -> Option<KeyCode> {
        input::scancode_to_keycode(scancode)
    }

    fn platform_name(&self) -> &str {
        "win32"
    }
}

// ---------------------------------------------------------------------------
// Win32Platform — the top-level backend
// ---------------------------------------------------------------------------

/// Win32 platform backend.
///
/// Manages the window class, event queue, and all sub-backends for the
/// Windows desktop.
pub struct Win32Platform {
    // Sub-backends
    display: Win32DisplayBackend,
    window_host: Win32WindowHost,
    taskbar: Win32Taskbar,
    tray: Win32Tray,
    notifications: Win32Notifications,
    drag_drop: NullDragDrop,
    keymap: Win32Keymap,

    /// Shared event queue. Kept in a `Box` so that its heap address is
    /// stable for `WindowData` pointers. Wrapped in `UnsafeCell` to
    /// allow the wndproc to push events without creating `&mut` references.
    event_queue: Box<UnsafeCell<VecDeque<PlatformEvent>>>,

    /// ATOM from `RegisterClassExW` (needed for unregistration).
    class_atom: ffi::ATOM,

    /// Process instance handle.
    hinstance: ffi::HINSTANCE,

    /// Wide class name (kept alive so the ATOM remains valid).
    class_name_wide: Vec<u16>,

    /// Hidden message-only window for tray callbacks, etc.
    msg_hwnd: ffi::HWND,

    /// Requested presentation behavior for lazily-created DXGI presenters.
    present_mode: dxgi::DxgiPresentMode,

    /// Accepted-present metadata surfaced through PlatformBackend feedback.
    present_feedback: Win32PresentFeedbackState,

    /// On-screen present cadence cap (Hz) applied when a remote (RDP) session is
    /// detected. The render thread keeps running at full speed; the GDI present
    /// layer coalesces damage and flips at most this often so it never wastes
    /// BitBlts the RDP client will not sample. `None` disables the cap entirely
    /// (always present every frame, even remote).
    remote_present_cap_hz: Option<u32>,

    /// Test/override hook for remote-session detection. `None` means "ask the OS
    /// via `GetSystemMetrics(SM_REMOTESESSION)`"; `Some(b)` forces the answer so
    /// the coalescing path can be exercised without a real RDP session.
    remote_session_override: Option<bool>,
}

// Safety: Win32Platform owns all raw handles and is designed to be used
// from a single thread (the one that runs the message loop). The `Send`
// bound on `PlatformBackend` is satisfied by structural guarantees.
unsafe impl Send for Win32Platform {}

impl Win32Platform {
    /// Create and initialise a new Win32 platform backend.
    ///
    /// Registers a window class and creates a hidden message-only window for
    /// tray icon callbacks. Fails if `RegisterClassExW` fails.
    pub fn new() -> PlatformResult<Self> {
        // Safety: GetModuleHandleW(null) returns the HINSTANCE of the
        // running executable, which is always valid.
        let hinstance = unsafe { ffi::GetModuleHandleW(ptr::null()) };
        if hinstance.is_null() {
            return Err(PlatformError::Other(
                "GetModuleHandleW returned null".into(),
            ));
        }

        let class_name = "LiquiDE_Win32_Window";
        let class_name_wide = to_wide(class_name);

        let wc = ffi::WNDCLASSEXW {
            cbSize: std::mem::size_of::<ffi::WNDCLASSEXW>() as ffi::UINT,
            style: ffi::CS_HREDRAW | ffi::CS_VREDRAW | ffi::CS_OWNDC,
            lpfnWndProc: Some(wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: ptr::null_mut(),
            hCursor: // SAFETY: LoadCursorW with null hInstance loads systems cursors.
                     unsafe { ffi::LoadCursorW(ptr::null_mut(), ffi::IDC_ARROW) },
            hbrBackground: ptr::null_mut(),
            lpszMenuName: ptr::null(),
            lpszClassName: class_name_wide.as_ptr(),
            hIconSm: ptr::null_mut(),
        };

        // Safety: RegisterClassExW registers the window class. The struct is
        // fully initialised above.
        let class_atom = unsafe { ffi::RegisterClassExW(&wc) };
        if class_atom == 0 {
            // ERROR_CLASS_ALREADY_EXISTS (1410) means a previous Win32Platform
            // in this process already registered the (process-global) class.
            // That registration is still valid for CreateWindowExW, so tolerate
            // it rather than failing — this lets a second backend (and the unit
            // tests) construct without error.
            const ERROR_CLASS_ALREADY_EXISTS: ffi::DWORD = 1410;
            let err = unsafe { ffi::GetLastError() };
            if err != ERROR_CLASS_ALREADY_EXISTS {
                return Err(PlatformError::Other(format!(
                    "RegisterClassExW failed (error {err})"
                )));
            }
        }

        // Create a hidden message-only window for tray icon callbacks.
        let msg_class = "LiquiDE_MsgOnly";
        let msg_class_wide = to_wide(msg_class);

        let msg_wc = ffi::WNDCLASSEXW {
            cbSize: std::mem::size_of::<ffi::WNDCLASSEXW>() as ffi::UINT,
            style: 0,
            lpfnWndProc: Some(wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: ptr::null_mut(),
            hCursor: ptr::null_mut(),
            hbrBackground: ptr::null_mut(),
            lpszMenuName: ptr::null(),
            lpszClassName: msg_class_wide.as_ptr(),
            hIconSm: ptr::null_mut(),
        };
        // SAFETY: RegisterClassExW registers the message-only window class.
        // The struct is fully initialised above.
        unsafe {
            ffi::RegisterClassExW(&msg_wc);
        }

        let msg_title = to_wide("");
        // SAFETY: CreateWindowExW creates a hidden message-only window.
        // All parameters are valid and the class was just registered.
        let msg_hwnd = unsafe {
            ffi::CreateWindowExW(
                0,
                msg_class_wide.as_ptr(),
                msg_title.as_ptr(),
                0, // not visible
                0,
                0,
                0,
                0,
                ptr::null_mut(),
                ptr::null_mut(),
                hinstance,
                ptr::null_mut(),
            )
        };

        // Allocate a stable event queue on the heap.  The Box provides a
        // stable heap address that survives moves of the containing struct.
        // Wrapped in UnsafeCell so the wndproc can push events without
        // creating &mut references that would alias.
        let event_queue = Box::new(UnsafeCell::new(VecDeque::<PlatformEvent>::with_capacity(
            256,
        )));

        // Safety: obtain a raw pointer to the UnsafeCell. The temporary borrow
        // ends at the semicolon so it does not conflict with later accesses.
        let eq_ptr: *const UnsafeCell<VecDeque<PlatformEvent>> = &*event_queue;

        let window_host = Win32WindowHost {
            windows: HashMap::new(),
            next_handle: 1,
            hinstance,
            class_name: class_name_wide.clone(),
            event_queue: eq_ptr,
        };

        let tray = Win32Tray::new(msg_hwnd);

        Ok(Self {
            display: Win32DisplayBackend,
            window_host,
            taskbar: Win32Taskbar,
            tray,
            notifications: Win32Notifications::new(),
            drag_drop: NullDragDrop,
            keymap: Win32Keymap,
            event_queue,
            class_atom,
            hinstance,
            class_name_wide,
            msg_hwnd,
            // Default to vsync (RefreshSync) for the windowed/dev CPU present
            // path. The CPU renderer uploads a full BGRA frame each present via
            // UpdateSubresource; presenting that with no vsync + ALLOW_TEARING
            // races DWM composition and reads as flicker (t55 flicker fix, H1).
            // RefreshSync presents with sync_interval = 1 and never combines
            // with ALLOW_TEARING, so DWM composites whole frames. Callers that
            // genuinely need no-vsync immediate presentation (e.g. specialized
            // non-windowed paths) can opt in via `new_with_present_mode`.
            present_mode: dxgi::DxgiPresentMode::RefreshSync,
            present_feedback: Win32PresentFeedbackState::default(),
            remote_present_cap_hz: Some(DEFAULT_REMOTE_PRESENT_HZ),
            remote_session_override: None,
        })
    }

    /// Create a Win32 backend with an explicit DXGI present mode.
    pub fn new_with_present_mode(present_mode: dxgi::DxgiPresentMode) -> PlatformResult<Self> {
        let mut platform = Self::new()?;
        platform.present_mode = present_mode;
        Ok(platform)
    }

    /// Return the requested DXGI present mode for newly-created presenters.
    pub fn present_mode(&self) -> dxgi::DxgiPresentMode {
        self.present_mode
    }

    /// Set the remote (RDP) on-screen present cadence cap in Hz, or `None` to
    /// disable coalescing entirely (always present every frame, even remote).
    ///
    /// Takes effect for windows whose coalescer has not yet been initialized
    /// (i.e. before their first GDI present). The local path is never affected —
    /// when no remote session is detected, every present is flipped immediately
    /// regardless of this value.
    pub fn set_remote_present_cap_hz(&mut self, cap_hz: Option<u32>) {
        self.remote_present_cap_hz = cap_hz;
    }

    /// Current remote present cadence cap (Hz), or `None` if disabled.
    pub fn remote_present_cap_hz(&self) -> Option<u32> {
        self.remote_present_cap_hz
    }

    /// Force the remote-session detection result for testing, or `None` to use
    /// the real OS query (`GetSystemMetrics(SM_REMOTESESSION)`). Lets the
    /// coalescing present path be exercised without a live RDP session.
    pub fn set_remote_session_override(&mut self, remote: Option<bool>) {
        self.remote_session_override = remote;
    }

    /// True when the current session should be treated as remote (RDP): the
    /// test override if set, else the live `SM_REMOTESESSION` system metric.
    fn is_remote_session(&self) -> bool {
        if let Some(forced) = self.remote_session_override {
            return forced;
        }
        // SAFETY: GetSystemMetrics is a pure query with no preconditions.
        unsafe { ffi::GetSystemMetrics(ffi::SM_REMOTESESSION) != 0 }
    }

    /// Pump all pending Win32 messages and dispatch them through the wndproc,
    /// which pushes `PlatformEvent`s into `self.event_queue`.
    fn pump_messages(&mut self) {
        let mut msg = ffi::MSG::default();
        // Safety: PeekMessageW retrieves messages from the calling thread's
        // queue. The MSG struct is valid.
        while unsafe { ffi::PeekMessageW(&mut msg, ptr::null_mut(), 0, 0, ffi::PM_REMOVE) != 0 } {
            if msg.message == ffi::WM_QUIT {
                // SAFETY: The event_queue UnsafeCell is only accessed from
                // the message-loop thread.  No concurrent access is possible.
                unsafe { (*self.event_queue.get()).push_back(PlatformEvent::Quit) };
                return;
            }
            // Safety: TranslateMessage and DispatchMessageW are standard
            // message loop calls and are safe with a valid MSG.
            unsafe {
                ffi::TranslateMessage(&msg);
                ffi::DispatchMessageW(&msg);
            }
        }
    }
}

impl Drop for Win32Platform {
    fn drop(&mut self) {
        // Destroy all tracked windows.
        let handles: Vec<u64> = self.window_host.windows.keys().copied().collect();
        for h in handles {
            if let Some(info) = self.window_host.windows.remove(&h) {
                // SAFETY: Clearing user-data and destroying windows during teardown.
                // All HWNDs are valid because they were created by this platform.
                unsafe {
                    ffi::SetWindowLongPtrW(info.hwnd, ffi::GWLP_USERDATA, 0);
                    ffi::DestroyWindow(info.hwnd);
                }
            }
        }

        // Remove all tray icons.
        let tray_ids: Vec<u64> = self.tray.icons.keys().copied().collect();
        for id in tray_ids {
            let _ = self.tray.remove_icon(NativeTrayHandle(id));
        }

        // Destroy the hidden message window.
        if !self.msg_hwnd.is_null() {
            // SAFETY: msg_hwnd is a valid HWND created in `new()`.
            unsafe {
                ffi::DestroyWindow(self.msg_hwnd);
            }
        }

        // Unregister the window class.
        if self.class_atom != 0 {
            // SAFETY: UnregisterClassW is safe with the class name and hinstance
            // that were used to register the class.
            unsafe {
                ffi::UnregisterClassW(self.class_name_wide.as_ptr(), self.hinstance);
            }
        }
    }
}

impl PlatformBackend for Win32Platform {
    fn display(&self) -> &dyn DisplayBackend {
        &self.display
    }

    fn window_host(&mut self) -> &mut dyn NativeWindowHost {
        &mut self.window_host
    }

    fn taskbar(&mut self) -> &mut dyn TaskbarIntegration {
        &mut self.taskbar
    }

    fn tray(&mut self) -> &mut dyn NativeTray {
        &mut self.tray
    }

    fn notifications(&mut self) -> &mut dyn NativeNotifications {
        &mut self.notifications
    }

    fn drag_drop(&mut self) -> &mut dyn NativeDragDrop {
        &mut self.drag_drop
    }

    fn keymap(&self) -> &dyn KeymapTranslator {
        &self.keymap
    }

    fn platform_name(&self) -> &str {
        "win32"
    }

    // ── Event loop ───────────────────────────────────────────────────

    fn poll_event(&mut self) -> Option<PlatformEvent> {
        // First drain any already-queued events.
        // SAFETY: we have &mut self, so no other Rust code can access
        // the event_queue concurrently. The wndproc only runs during
        // pump_messages below (same thread).
        let queue = unsafe { &mut *self.event_queue.get() };
        if let Some(ev) = queue.pop_front() {
            return Some(ev);
        }
        // Pump the Win32 message queue.
        self.pump_messages();
        // Return the next event, if any.
        // SAFETY: same reasoning as above — exclusive &mut self access.
        let queue = unsafe { &mut *self.event_queue.get() };
        queue.pop_front()
    }

    fn wait_event(&mut self) -> PlatformEvent {
        // Block until a real platform event is available.
        // Some Win32 messages (WM_TIMER, WM_SETFOCUS, internal painting
        // messages, etc.) don't produce a PlatformEvent, so we loop until
        // GetMessageW delivers one that does — matching the Wayland
        // backend's looping behaviour.
        loop {
            // Drain any already-queued events first.
            // SAFETY: exclusive &mut self access; no concurrent queue mutation.
            let queue = unsafe { &mut *self.event_queue.get() };
            if let Some(ev) = queue.pop_front() {
                return ev;
            }

            // Block until a message arrives.
            let mut msg = ffi::MSG::default();
            // SAFETY: GetMessageW blocks until a message is available.
            // Return value 0 = WM_QUIT, -1 = error.
            let ret = unsafe { ffi::GetMessageW(&mut msg, ptr::null_mut(), 0, 0) };

            if ret <= 0 {
                return PlatformEvent::Quit;
            }

            // SAFETY: TranslateMessage and DispatchMessageW are standard
            // message-loop calls; the MSG struct is valid.
            unsafe {
                ffi::TranslateMessage(&msg);
                ffi::DispatchMessageW(&msg);
            }

            // Drain any additional pending messages.
            self.pump_messages();
            // Loop back to check if any PlatformEvents were produced.
        }
    }

    fn take_present_feedback(&mut self) -> Option<PresentFeedback> {
        self.present_feedback.take_feedback()
    }

    // ── Frame presentation ───────────────────────────────────────────

    fn present_frame(
        &mut self,
        handle: NativeWindowHandle,
        pixels: &[u8],
        width: u32,
        height: u32,
        stride: u32,
        format: PixelFormat,
    ) -> PlatformResult<()> {
        // Full present (damage hint = None).
        self.present_frame_impl(handle, pixels, width, height, stride, format, None)
    }

    fn present_frame_damaged(
        &mut self,
        handle: NativeWindowHandle,
        pixels: &[u8],
        width: u32,
        height: u32,
        stride: u32,
        format: PixelFormat,
        damage: Option<&[Rect]>,
    ) -> PlatformResult<()> {
        // Normalize the compositor damage rects into clamped/coalesced integer
        // pixel rects up front. `None` stays `None` (full present); `Some(&[])`
        // (nothing changed) stays an empty Vec (the impl refreshes the
        // back-buffer for WM_PAINT but skips the on-screen blit).
        let coalesced = damage.map(|rects| coalesce_damage_rects(rects, width, height));
        self.present_frame_impl(
            handle,
            pixels,
            width,
            height,
            stride,
            format,
            coalesced.as_deref(),
        )
    }

    fn request_redraw(&mut self, handle: NativeWindowHandle) {
        if let Some(info) = self.window_host.windows.get(&handle.0) {
            // SAFETY: InvalidateRect with a null RECT invalidates the entire
            // client area, causing a WM_PAINT message to be posted.
            // The HWND is valid because it's in our window map.
            unsafe {
                ffi::InvalidateRect(info.hwnd, ptr::null(), ffi::FALSE);
            }
        }
    }


    fn set_cursor_shape(&mut self, handle: NativeWindowHandle, shape: &str) -> bool {
        let cursor_id = match shape {
            "default" | "arrow" => ffi::IDC_ARROW,
            "pointer" | "hand" => ffi::IDC_HAND,
            "text" | "ibeam" => ffi::IDC_IBEAM,
            "crosshair" => ffi::IDC_CROSS,
            "move" | "all-scroll" => ffi::IDC_SIZEALL,
            "not-allowed" | "no-drop" => ffi::IDC_NO,
            "wait" => ffi::IDC_WAIT,
            "progress" => ffi::IDC_APPSTARTING,
            "help" => ffi::IDC_HELP,
            "ns-resize" | "row-resize" => ffi::IDC_SIZENS,
            "ew-resize" | "col-resize" => ffi::IDC_SIZEWE,
            "nwse-resize" => ffi::IDC_SIZENWSE,
            "nesw-resize" => ffi::IDC_SIZENESW,
            "none" | "hidden" => ptr::null(),
            _ => ffi::IDC_ARROW,
        };

        if let Some(info) = self.window_host.windows.get(&handle.0) {
            // SAFETY: LoadCursorW with null hInstance loads a system cursor.
            let hcursor = unsafe { ffi::LoadCursorW(ptr::null_mut(), cursor_id) };
            // Store the cursor handle in the WindowData so the wndproc
            // uses it on WM_SETCURSOR.
            info._data
                .cursor
                .store(hcursor as isize, std::sync::atomic::Ordering::Relaxed);
            // Also set it immediately (in case we're already in the client area).
            // SAFETY: SetCursor is safe with any cursor handle.
            unsafe {
                ffi::SetCursor(hcursor);
            }
            true
        } else {
            false
        }
    }

    fn hide_cursor(&mut self, handle: NativeWindowHandle) {
        if let Some(info) = self.window_host.windows.get(&handle.0) {
            info._data
                .cursor
                .store(0, std::sync::atomic::Ordering::Relaxed);
            // SAFETY: SetCursor(null) hides the cursor. Always safe to call.
            unsafe {
                ffi::SetCursor(ptr::null_mut());
            }
        }
    }

    fn show_cursor(&mut self, handle: NativeWindowHandle) {
        if let Some(info) = self.window_host.windows.get(&handle.0) {
            // SAFETY: LoadCursorW with null hInstance loads a system cursor.
            let hcursor = unsafe { ffi::LoadCursorW(ptr::null_mut(), ffi::IDC_ARROW) };
            info._data
                .cursor
                .store(hcursor as isize, std::sync::atomic::Ordering::Relaxed);
            // SAFETY: SetCursor is safe with any cursor handle.
            unsafe {
                ffi::SetCursor(hcursor);
            }
        }
    }
}

impl Win32Platform {
    /// Core present path shared by the full and damage-aware entry points.
    ///
    /// `damage`:
    /// - `None` — present the WHOLE surface (DXGI present or full-surface BitBlt).
    /// - `Some(rects)` — the GDI path still refreshes the *entire* off-screen
    ///   back-buffer (so a subsequent WM_PAINT replays the authoritative full
    ///   frame) but only BitBlts the given sub-rectangles to the visible DC.
    ///   An empty slice presents nothing to screen (frame unchanged). The DXGI
    ///   path cannot do a sub-rect present with the current swap-chain present
    ///   model, so it ignores the hint and presents the full surface (still
    ///   correct, just not bandwidth-optimal — RDP never takes the DXGI path).
    #[allow(clippy::too_many_arguments)]
    fn present_frame_impl(
        &mut self,
        handle: NativeWindowHandle,
        pixels: &[u8],
        width: u32,
        height: u32,
        stride: u32,
        format: PixelFormat,
        damage: Option<&[PixelRect]>,
    ) -> PlatformResult<()> {
        // We only support BGRA8, which maps directly to Win32's 32-bit
        // BI_RGB (which is actually BGRA in memory).
        if format != PixelFormat::Bgra8 {
            return Err(PlatformError::Presentation(format!(
                "unsupported pixel format {:?}; only Bgra8 is supported",
                format
            )));
        }

        let present_mode = self.present_mode;
        // Capture remote-cadence config before borrowing `info` (the
        // remote-session query borrows `&self`). `cap_hz == None` disables
        // coalescing; a non-remote session always presents every frame.
        let remote_session = self.is_remote_session();
        let remote_cap_hz = self.remote_present_cap_hz;
        let info = self
            .window_host
            .windows
            .get_mut(&handle.0)
            .ok_or_else(|| PlatformError::Presentation("unknown window handle".into()))?;

        let hwnd = info.hwnd;

        // Try DXGI presentation first.
        // Lazily initialize the DXGI presenter on first use.
        if info.dxgi.is_none() {
            match dxgi::DxgiPresenter::new_with_present_mode(hwnd, width, height, present_mode) {
                Ok(presenter) => {
                    info.dxgi = Some(presenter);
                }
                Err(_) => {
                    // DXGI unavailable; will fall through to GDI below.
                }
            }
        }

        if let Some(ref mut presenter) = info.dxgi {
            match presenter.present(pixels, width, height, stride) {
                Ok(()) => {
                    // One-time present-path diagnostic. Over RDP, hardware DXGI
                    // commonly falls back to GDI/WARP; this confirms the active
                    // path without spamming a per-frame line. Read the DPI from
                    // the disjoint `_data` field (not via `dpi_scale()`, which
                    // would borrow all of `info` while `presenter` holds
                    // `info.dxgi`).
                    if !info.present_path_logged {
                        info.present_path_logged = true;
                        let dpi_scale = f32::from_bits(
                            info._data
                                .dpi_scale_bits
                                .load(std::sync::atomic::Ordering::Relaxed),
                        );
                        eprintln!(
                            "[liquide][win32] present path: hardware DXGI swap-chain \
                             (window {}, dpi_scale {:.2})",
                            handle.0, dpi_scale
                        );
                    }
                    self.present_feedback
                        .record_accepted_present(timestamp_ns());
                    return Ok(());
                }
                Err(_) => {
                    // DXGI present failed (device lost, etc.); drop the
                    // presenter and fall through to GDI for this frame.
                    info.dxgi = None;
                }
            }
        }

        // GDI fallback: off-screen DIB-section back-buffer + atomic BitBlt.
        //
        // This is the path hardware-less / remote (RDP) sessions take. The old
        // implementation wrote the framebuffer directly to the visible window DC
        // via `SetDIBitsToDevice`, which let the DWM/RDP compositor sample the DC
        // mid-write → partial-frame tearpoints/flicker. Instead we copy the
        // frame into an off-screen DIB section and then perform a *single*
        // `BitBlt` to the window DC, which the compositor observes as one atomic
        // pixel update (t62 flicker fix #1).
        //
        // The framebuffer rows must be packed (stride == width * 4) so the BGRA
        // memcpy into the DIB lines up with the DIB's natural stride. A padded
        // framebuffer would otherwise be read incorrectly and shear; guard it
        // here (carried over from t55 flicker fix, H3 hardening).
        let packed_stride = width.saturating_mul(4);
        if stride != packed_stride {
            return Err(PlatformError::Presentation(format!(
                "GDI fallback requires packed BGRA rows (stride {stride} != {packed_stride} for width {width})"
            )));
        }
        // Validate the buffer is large enough for the full frame before copying.
        let required = (packed_stride as usize).saturating_mul(height as usize);
        if pixels.len() < required {
            return Err(PlatformError::Presentation(format!(
                "GDI fallback pixel buffer too small: {} < {} for {}x{}",
                pixels.len(),
                required,
                width,
                height
            )));
        }

        // Re-borrow the window to manage its off-screen back-buffer lifecycle.
        let info = self
            .window_host
            .windows
            .get_mut(&handle.0)
            .ok_or_else(|| PlatformError::Presentation("unknown window handle".into()))?;

        // SAFETY: All GDI calls below operate on a valid HWND/DC. The pixel
        // buffer was bounds-checked above. The back-buffer's DIB memory is
        // `width * height * 4` bytes (matching the bitmap we created).
        unsafe {
            let window_hdc = ffi::GetDC(hwnd);
            if window_hdc.is_null() {
                return Err(PlatformError::Presentation("GetDC returned null".into()));
            }

            // (Re)create the off-screen buffer if missing or the frame size
            // changed (window resize). Dropping the old buffer frees its DC and
            // bitmap before the new one is allocated.
            let needs_realloc = match &info.gdi_back_buffer {
                Some(bb) => bb.width != width || bb.height != height,
                None => true,
            };
            if needs_realloc {
                // Un-publish the stale DC first so a concurrent WM_PAINT on this
                // thread can never blit from a back-buffer we're about to drop.
                info._data
                    .backbuffer_dc
                    .store(0, std::sync::atomic::Ordering::Release);
                info.gdi_back_buffer = None;
                info.gdi_back_buffer = GdiBackBuffer::create(window_hdc, width, height);
            }

            let Some(back_buffer) = info.gdi_back_buffer.as_ref() else {
                info._data
                    .backbuffer_dc
                    .store(0, std::sync::atomic::Ordering::Release);
                ffi::ReleaseDC(hwnd, window_hdc);
                return Err(PlatformError::Presentation(
                    "failed to create GDI off-screen back-buffer".into(),
                ));
            };

            // Publish the back-buffer so WM_PAINT can replay the authoritative
            // frame. Width/height are stored before the DC (release ordering on
            // the DC publishes them) so a WM_PAINT that observes the DC also sees
            // the matching extent.
            info._data
                .backbuffer_w
                .store(back_buffer.width, std::sync::atomic::Ordering::Relaxed);
            info._data
                .backbuffer_h
                .store(back_buffer.height, std::sync::atomic::Ordering::Relaxed);
            info._data.backbuffer_dc.store(
                back_buffer.mem_dc as isize,
                std::sync::atomic::Ordering::Release,
            );

            // Copy out the back-buffer's DC + memory pointer (both `Copy` raw
            // handles) so the immutable `info.gdi_back_buffer` borrow is released
            // before we mutably borrow `info.remote_coalescer` below. The handles
            // stay valid: nothing reallocs the back-buffer after this point.
            let bb_mem_dc = back_buffer.mem_dc;
            let bb_bits = back_buffer.bits;
            let _ = back_buffer;

            // Update the off-screen DIB memory with THIS present's pixels.
            //
            // The DIB is the authoritative back-buffer that WM_PAINT replays in
            // full, so it must always hold a complete, current frame — but it
            // *accumulates* across presents: we only overwrite the regions that
            // actually changed this present, and unchanged regions retain their
            // prior (still-valid) content. So a partial present only copies its
            // damaged sub-rects into the DIB, not the whole 8 MB frame.
            //
            //   - `None` (full present) / first present / resize realloc → copy
            //     the WHOLE frame (a single contiguous `required`-byte memcpy).
            //     A realloc'd DIB has undefined contents, so it MUST be fully
            //     populated regardless of the damage hint.
            //   - `Some(rects)` on an existing DIB → copy only those sub-rects
            //     row-by-row. Source and destination share identical top-down
            //     packed BGRA8 layout, so each rect copies at the same offset.
            //
            // This is local memory only — it costs no RDP bandwidth; the RDP
            // cost is the BitBlt to the visible DC below. Limiting it avoids the
            // ~1-3 ms whole-frame memcpy on a tiny (e.g. cursor) partial present.
            let dib_full = damage.is_none() || needs_realloc;
            // SAFETY: `bb_bits` points at the DIB section's `required`-byte
            // (width*height*4) memory, exclusively owned by this back-buffer and
            // not aliased while we hold the DC. `apply_present_to_dib` only
            // writes within `[0, required)`.
            let dib_slice = std::slice::from_raw_parts_mut(bb_bits as *mut u8, required);
            apply_present_to_dib(dib_slice, pixels, packed_stride, height, damage, dib_full);

            // Decide which sub-rectangles to BitBlt to the visible window DC.
            // This is where the RDP cadence cap applies: the coalescer may DEFER
            // the on-screen flip (accumulating damage) so we don't waste BitBlts
            // the RDP client will never sample. The DIB was already updated above
            // with this present's pixels, so a later coalesced flip blits the
            // freshest content for the accumulated rects. A realloc forces a full
            // immediate flip (prior on-screen content is invalid).
            //
            // The coalescer is created lazily on first GDI present so it reflects
            // the live remote-session state and the configured cap. `enabled` is
            // false for a local session or when the cap is disabled → it returns
            // PresentNow with the present's own damage (no coalescing, no added
            // latency: identical to the prior per-frame local path).
            let coalescer = info.remote_coalescer.get_or_insert_with(|| {
                let enabled = remote_session && remote_cap_hz.is_some();
                RemotePresentCoalescer::new(enabled, remote_cap_hz.unwrap_or(DEFAULT_REMOTE_PRESENT_HZ))
            });

            // A realloc invalidates any prior on-screen content → force a full
            // flip now (feed `None` so accumulated partial damage is subsumed).
            let blit_decision = if needs_realloc {
                coalescer.on_present(None, monotonic_ms())
            } else {
                coalescer.on_present(damage, monotonic_ms())
            };

            let mut blit_ok = true;
            match blit_decision {
                CoalesceDecision::Defer => {
                    // Damage retained for a later flip; nothing hits the screen.
                }
                CoalesceDecision::PresentNow(None) => {
                    // Full flip: one BitBlt of the whole frame onto the window DC.
                    let ok = ffi::BitBlt(
                        window_hdc,
                        0,
                        0,
                        width as i32,
                        height as i32,
                        bb_mem_dc,
                        0,
                        0,
                        ffi::SRCCOPY,
                    );
                    blit_ok = ok != ffi::FALSE;
                }
                CoalesceDecision::PresentNow(Some(rects)) => {
                    // Partial flip: one BitBlt per accumulated damage rect. Rects
                    // are clamped to the surface, so dst/src extents are in-bounds
                    // for both the window DC and the DIB.
                    for r in &rects {
                        if r.w == 0 || r.h == 0 {
                            continue;
                        }
                        let ok = ffi::BitBlt(
                            window_hdc,
                            r.x as i32,
                            r.y as i32,
                            r.w as i32,
                            r.h as i32,
                            bb_mem_dc,
                            r.x as i32,
                            r.y as i32,
                            ffi::SRCCOPY,
                        );
                        if ok == ffi::FALSE {
                            blit_ok = false;
                            break;
                        }
                    }
                }
            }

            ffi::ReleaseDC(hwnd, window_hdc);

            if !blit_ok {
                return Err(PlatformError::Presentation(
                    "BitBlt failed (GDI back-buffer present failed)".into(),
                ));
            }
        }

        // One-time present-path diagnostic for the GDI/WARP fallback. This is
        // the path hardware-less / remote (RDP) sessions usually take, so the
        // line confirms the DE is blitting via GDI rather than DXGI, and whether
        // it detected a Remote Desktop session.
        if !info.present_path_logged {
            info.present_path_logged = true;
            // SAFETY: GetSystemMetrics is a pure query with no preconditions.
            let remote = unsafe { ffi::GetSystemMetrics(ffi::SM_REMOTESESSION) } != 0;
            eprintln!(
                "[liquide][win32] present path: GDI fallback (off-screen DIB + BitBlt) \
                 — no hardware DXGI swap-chain (window {}, dpi_scale {:.2}, remote_session {})",
                handle.0,
                info.dpi_scale(),
                remote
            );
        }

        self.present_feedback
            .record_accepted_present(timestamp_ns());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_wide_ascii_string() {
        let wide = to_wide("ABC");
        // 'A'=0x41, 'B'=0x42, 'C'=0x43, null=0x00
        assert_eq!(wide, vec![0x41, 0x42, 0x43, 0x00]);
    }

    #[test]
    fn to_wide_empty_string() {
        let wide = to_wide("");
        assert_eq!(wide, vec![0x00]);
    }

    #[test]
    fn to_wide_unicode_string() {
        // '€' is U+20AC → single UTF-16 code unit 0x20AC
        let wide = to_wide("€");
        assert_eq!(wide, vec![0x20AC, 0x00]);
    }

    #[test]
    fn from_wide_ascii() {
        let wide = [0x48u16, 0x69, 0x00]; // "Hi\0"
        assert_eq!(from_wide(&wide), "Hi");
    }

    #[test]
    fn from_wide_stops_at_null() {
        let wide = [0x41u16, 0x42, 0x00, 0x43, 0x44];
        assert_eq!(from_wide(&wide), "AB");
    }

    #[test]
    fn from_wide_no_null_terminator() {
        let wide = [0x58u16, 0x59, 0x5A]; // "XYZ" with no null
        assert_eq!(from_wide(&wide), "XYZ");
    }

    #[test]
    fn to_wide_roundtrip() {
        let original = "Hello Platform";
        let wide = to_wide(original);
        let back = from_wide(&wide);
        assert_eq!(back, original);
    }

    #[test]
    fn timestamp_us_is_nonzero() {
        let ts = timestamp_us();
        assert!(ts > 0);
    }

    #[test]
    fn refresh_rate_metadata_filters_windows_default_values() {
        assert_eq!(refresh_rate_hz_from_devmode_frequency(0), None);
        assert_eq!(refresh_rate_hz_from_devmode_frequency(1), None);
        assert_eq!(refresh_rate_hz_from_devmode_frequency(60), Some(60));
        assert_eq!(refresh_rate_hz_from_devmode_frequency(144), Some(144));
    }

    /// The default Win32 present mode must be vsync (RefreshSync), so the
    /// windowed/dev CPU present path does not tear/flicker (t55 flicker fix,
    /// H1). This guards the wiring decision point without rendering.
    #[test]
    fn default_present_mode_is_refresh_sync() {
        let platform = Win32Platform::new().expect("create Win32 platform");
        assert_eq!(platform.present_mode(), dxgi::DxgiPresentMode::RefreshSync);
    }

    /// An explicit present mode is still honored (e.g. Immediate for
    /// non-windowed paths), so the default change does not lock out callers.
    #[test]
    fn explicit_present_mode_overrides_default() {
        let platform = Win32Platform::new_with_present_mode(dxgi::DxgiPresentMode::Immediate)
            .expect("create Win32 platform");
        assert_eq!(platform.present_mode(), dxgi::DxgiPresentMode::Immediate);
    }

    /// For a normal overlapped window, AdjustWindowRectEx must grow the
    /// requested client size to a strictly larger outer window size (title bar
    /// + borders), so the client area ends up == the requested size and DXGI
    /// does not stretch the frame (t55 flicker fix, H2).
    #[test]
    fn client_sizing_grows_overlapped_window() {
        let (w, h) =
            client_size_to_window_size(1270, 768, ffi::WS_OVERLAPPEDWINDOW, ffi::WS_EX_APPWINDOW);
        assert!(
            w >= 1270 && h >= 768,
            "outer window must be at least the requested client size, got {w}x{h}"
        );
        assert!(
            w > 1270 || h > 768,
            "overlapped window has borders/title bar, so outer must exceed client; got {w}x{h}"
        );
    }

    /// A borderless popup ("desktop") has no non-client area, so the outer
    /// window size equals the requested client size.
    #[test]
    fn client_sizing_borderless_popup_is_identity() {
        let (w, h) = client_size_to_window_size(800, 600, ffi::WS_POPUP, ffi::WS_EX_APPWINDOW);
        assert_eq!((w, h), (800, 600));
    }

    /// Degenerate (zero/negative) client sizes are clamped to at least 1×1.
    #[test]
    fn client_sizing_clamps_degenerate_sizes() {
        let (w, h) = client_size_to_window_size(0, -5, ffi::WS_POPUP, ffi::WS_EX_APPWINDOW);
        assert!(w >= 1 && h >= 1, "must clamp to >= 1x1, got {w}x{h}");
    }

    #[test]
    fn present_feedback_state_records_accepted_presents_in_order() {
        let mut state = Win32PresentFeedbackState::default();
        state.record_accepted_present(10);
        state.record_accepted_present(20);

        let first = state.take_feedback().unwrap();
        let second = state.take_feedback().unwrap();

        assert_eq!(first.acknowledged_present_count, 1);
        assert_eq!(first.sequence, Some(1));
        assert_eq!(first.timestamp_ns, Some(10));
        assert_eq!(second.acknowledged_present_count, 2);
        assert_eq!(second.sequence, Some(2));
        assert_eq!(second.timestamp_ns, Some(20));
        assert!(state.take_feedback().is_none());
    }

    // ── Damage-rect normalization (partial present) ──────────────────

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::new(x, y, w, h)
    }

    #[test]
    fn damage_rect_to_pixels_clamps_to_surface() {
        // A rect that overhangs the right/bottom edge is clamped to the surface.
        let pr = damage_rect_to_pixels(&rect(90.0, 90.0, 50.0, 50.0), 100, 100).unwrap();
        assert_eq!((pr.x, pr.y, pr.w, pr.h), (90, 90, 10, 10));
    }

    #[test]
    fn damage_rect_to_pixels_rejects_out_of_bounds_and_nonfinite() {
        // Fully outside the surface → dropped.
        assert!(damage_rect_to_pixels(&rect(200.0, 200.0, 10.0, 10.0), 100, 100).is_none());
        // Zero area → dropped.
        assert!(damage_rect_to_pixels(&rect(10.0, 10.0, 0.0, 10.0), 100, 100).is_none());
        // Non-finite → dropped (never produces a bogus rect).
        assert!(damage_rect_to_pixels(&rect(f32::NAN, 0.0, 10.0, 10.0), 100, 100).is_none());
        assert!(
            damage_rect_to_pixels(&rect(0.0, 0.0, f32::INFINITY, 10.0), 100, 100).is_none()
        );
    }

    #[test]
    fn damage_rect_to_pixels_expands_fractional_to_whole_pixels() {
        // A fractional rect must cover every partially-touched pixel.
        let pr = damage_rect_to_pixels(&rect(10.5, 10.5, 1.0, 1.0), 100, 100).unwrap();
        assert_eq!((pr.x, pr.y), (10, 10));
        // (10.5 .. 11.5) → floor 10 .. ceil 12 → width 2.
        assert_eq!((pr.w, pr.h), (2, 2));
    }

    #[test]
    fn coalesce_dedupes_identical_rects() {
        let r = rect(10.0, 10.0, 20.0, 20.0);
        let out = coalesce_damage_rects(&[r, r, r], 100, 100);
        assert_eq!(out.len(), 1);
        assert_eq!((out[0].x, out[0].y, out[0].w, out[0].h), (10, 10, 20, 20));
    }

    #[test]
    fn coalesce_absorbs_contained_rect() {
        let big = rect(0.0, 0.0, 50.0, 50.0);
        let small = rect(10.0, 10.0, 5.0, 5.0);
        let out = coalesce_damage_rects(&[big, small], 100, 100);
        assert_eq!(out.len(), 1);
        assert_eq!((out[0].x, out[0].y, out[0].w, out[0].h), (0, 0, 50, 50));
        // Order-independent.
        let out2 = coalesce_damage_rects(&[small, big], 100, 100);
        assert_eq!(out2.len(), 1);
        assert_eq!((out2[0].w, out2[0].h), (50, 50));
    }

    #[test]
    fn coalesce_merges_overlapping_into_bounding_box() {
        let a = rect(0.0, 0.0, 30.0, 30.0);
        let b = rect(20.0, 20.0, 30.0, 30.0);
        let out = coalesce_damage_rects(&[a, b], 100, 100);
        assert_eq!(out.len(), 1);
        // Union bounding box (0,0)-(50,50).
        assert_eq!((out[0].x, out[0].y, out[0].w, out[0].h), (0, 0, 50, 50));
    }

    #[test]
    fn coalesce_keeps_disjoint_rects_separate() {
        let a = rect(0.0, 0.0, 10.0, 10.0);
        let b = rect(80.0, 80.0, 10.0, 10.0);
        let out = coalesce_damage_rects(&[a, b], 100, 100);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn coalesce_chains_transitive_merges() {
        // a touches b, b touches c, but a does NOT touch c directly.
        // The greedy restart must still merge all three into one box.
        let a = rect(0.0, 0.0, 20.0, 10.0);
        let b = rect(20.0, 0.0, 20.0, 10.0);
        let c = rect(40.0, 0.0, 20.0, 10.0);
        // Insert in an order where the bridging rect comes last.
        let out = coalesce_damage_rects(&[a, c, b], 100, 100);
        assert_eq!(out.len(), 1);
        assert_eq!((out[0].x, out[0].w), (0, 60));
    }

    #[test]
    fn coalesce_empty_input_is_empty() {
        let out = coalesce_damage_rects(&[], 100, 100);
        assert!(out.is_empty());
    }

    #[test]
    fn coalesce_drops_out_of_bounds_but_keeps_valid() {
        let inside = rect(10.0, 10.0, 5.0, 5.0);
        let outside = rect(500.0, 500.0, 5.0, 5.0);
        let out = coalesce_damage_rects(&[outside, inside], 100, 100);
        assert_eq!(out.len(), 1);
        assert_eq!((out[0].x, out[0].y), (10, 10));
    }

    // ── Damage-limited DIB copy (Fix 1) ──────────────────────────────
    //
    // These prove `apply_present_to_dib` (the byte-copy the Win32 GDI present
    // uses to refresh its off-screen DIB) copies ONLY the damaged sub-rects on a
    // partial present and the WHOLE frame on a full present — and that the DIB
    // remains a complete valid frame after several partial presents (no stale /
    // torn region a WM_PAINT could replay). A regression that copied the whole
    // frame for a partial present would FAIL the pixel-count assertions; one that
    // left a damaged region stale would FAIL the accumulation assertions.

    const DIBW: u32 = 8;
    const DIBH: u32 = 8;

    fn dib_solid(byte: u8) -> Vec<u8> {
        vec![byte; (DIBW * DIBH * 4) as usize]
    }

    fn dib_px(dib: &[u8], x: u32, y: u32) -> [u8; 4] {
        let i = ((y * DIBW + x) * 4) as usize;
        [dib[i], dib[i + 1], dib[i + 2], dib[i + 3]]
    }

    #[test]
    fn dib_full_present_copies_whole_frame() {
        let mut dib = dib_solid(0x00);
        let src = dib_solid(0x11);
        // `None` (full present) → whole-frame copy. Written px == W*H.
        let written = apply_present_to_dib(&mut dib, &src, DIBW * 4, DIBH, None, true);
        assert_eq!(written, (DIBW * DIBH) as usize, "full present must copy the whole frame");
        assert_eq!(dib_px(&dib, 0, 0), [0x11; 4]);
        assert_eq!(dib_px(&dib, DIBW - 1, DIBH - 1), [0x11; 4]);
    }

    #[test]
    fn dib_partial_present_copies_only_damaged_rects() {
        let mut dib = dib_solid(0x11); // existing valid frame
        let src = dib_solid(0x22); // new frame, but only a 2x2 tile damaged
        let dmg = [PixelRect { x: 1, y: 1, w: 2, h: 2 }];
        let written = apply_present_to_dib(&mut dib, &src, DIBW * 4, DIBH, Some(&dmg), false);
        // ONLY 4 px must be written — a "copy whole frame on Some" regression
        // would write W*H here and fail this assertion (the core teeth of Fix 1).
        assert_eq!(written, 4, "partial present must copy only the damaged tile into the DIB");
        // Inside the damage rect → new content.
        assert_eq!(dib_px(&dib, 1, 1), [0x22; 4]);
        assert_eq!(dib_px(&dib, 2, 2), [0x22; 4]);
        // Outside → retains prior valid content (not overwritten, not stale-zero).
        assert_eq!(dib_px(&dib, 0, 0), [0x11; 4]);
        assert_eq!(dib_px(&dib, DIBW - 1, DIBH - 1), [0x11; 4]);
    }

    #[test]
    fn dib_accumulates_across_partial_presents_no_stale_region() {
        // Start from a full present so the DIB is a known complete frame.
        let mut dib = dib_solid(0x00);
        apply_present_to_dib(&mut dib, &dib_solid(0xAA), DIBW * 4, DIBH, None, true);

        // Partial present 1: change the top-left 2x2 to 0xBB.
        let mut f1 = dib_solid(0xAA);
        for y in 0..2 {
            for x in 0..2 {
                let i = ((y * DIBW + x) * 4) as usize;
                f1[i..i + 4].copy_from_slice(&[0xBB; 4]);
            }
        }
        apply_present_to_dib(
            &mut dib,
            &f1,
            DIBW * 4,
            DIBH,
            Some(&[PixelRect { x: 0, y: 0, w: 2, h: 2 }]),
            false,
        );

        // Partial present 2: change the bottom-right 2x2 to 0xCC.
        let mut f2 = dib_solid(0xAA);
        for y in (DIBH - 2)..DIBH {
            for x in (DIBW - 2)..DIBW {
                let i = ((y * DIBW + x) * 4) as usize;
                f2[i..i + 4].copy_from_slice(&[0xCC; 4]);
            }
        }
        apply_present_to_dib(
            &mut dib,
            &f2,
            DIBW * 4,
            DIBH,
            Some(&[PixelRect { x: DIBW - 2, y: DIBH - 2, w: 2, h: 2 }]),
            false,
        );

        // The DIB is now a COMPLETE valid frame: present-1 region, present-2
        // region, AND the untouched majority all hold the right pixels — no
        // region was left stale (zeroed) or torn.
        assert_eq!(dib_px(&dib, 0, 0), [0xBB; 4], "present-1 region present");
        assert_eq!(dib_px(&dib, DIBW - 1, DIBH - 1), [0xCC; 4], "present-2 region present");
        assert_eq!(dib_px(&dib, 4, 4), [0xAA; 4], "untouched center retained from full present");
        assert_eq!(dib_px(&dib, 0, DIBH - 1), [0xAA; 4], "untouched corner retained");
        // Whole-buffer sanity: no zero (stale/uninitialized) pixels remain.
        assert!(dib.iter().all(|&b| b != 0x00), "no stale/uninitialized DIB region");
    }

    #[test]
    fn dib_empty_damage_copies_nothing() {
        let mut dib = dib_solid(0x11);
        let src = dib_solid(0x22);
        let written = apply_present_to_dib(&mut dib, &src, DIBW * 4, DIBH, Some(&[]), false);
        assert_eq!(written, 0, "empty damage must not write into the DIB");
        assert_eq!(dib_px(&dib, 0, 0), [0x11; 4]);
        assert_eq!(dib_px(&dib, DIBW - 1, DIBH - 1), [0x11; 4]);
    }

    // ── RDP-aware present coalescing (Fix 2) ─────────────────────────
    //
    // These drive `RemotePresentCoalescer` directly with an injected monotonic
    // clock and an injected `enabled` flag (standing in for SM_REMOTESESSION),
    // so no real RDP session is needed. They prove that while coalescing, damage
    // is UNIONED across deferred presents and NEVER dropped, that a full present
    // subsumes partial damage, and that the local (disabled) path presents every
    // frame immediately with no added latency.

    fn pr(x: u32, y: u32, w: u32, h: u32) -> PixelRect {
        PixelRect { x, y, w, h }
    }

    fn present_now_rects(d: CoalesceDecision) -> Option<Vec<PixelRect>> {
        match d {
            CoalesceDecision::PresentNow(r) => Some(r.unwrap_or_default()),
            CoalesceDecision::Defer => None,
        }
    }

    #[test]
    fn coalescer_local_presents_every_frame_immediately() {
        // enabled=false → every present flips now with its OWN damage, unchanged.
        let mut c = RemotePresentCoalescer::new(false, 60);
        let d0 = c.on_present(Some(&[pr(0, 0, 4, 4)]), 0);
        let d1 = c.on_present(Some(&[pr(4, 4, 4, 4)]), 1); // 1ms later, well under cap
        let r0 = present_now_rects(d0).expect("local present must flip immediately");
        let r1 = present_now_rects(d1).expect("local present must flip immediately");
        assert_eq!(r0, vec![pr(0, 0, 4, 4)]);
        assert_eq!(r1, vec![pr(4, 4, 4, 4)], "local path must not coalesce / add latency");
    }

    #[test]
    fn coalescer_remote_unions_deferred_damage_never_drops() {
        // 60Hz cap → ~16ms min interval. First present flips; the next few within
        // the interval DEFER and accumulate; the flip at/after the interval blits
        // the UNION of every deferred rect — nothing dropped.
        let mut c = RemotePresentCoalescer::new(true, 60);

        // t=0: first present flips immediately (establishes cadence).
        let _ = present_now_rects(c.on_present(Some(&[pr(0, 0, 2, 2)]), 0))
            .expect("first remote present flips");

        // t=2,4,6 ms: three presents inside the interval → all DEFER.
        assert!(matches!(c.on_present(Some(&[pr(10, 10, 2, 2)]), 2), CoalesceDecision::Defer));
        assert!(matches!(c.on_present(Some(&[pr(20, 20, 2, 2)]), 4), CoalesceDecision::Defer));
        assert!(matches!(c.on_present(Some(&[pr(30, 30, 2, 2)]), 6), CoalesceDecision::Defer));

        // t=20 ms (>= 16ms interval): flips the UNION of the 3 deferred rects.
        let flipped = present_now_rects(c.on_present(Some(&[pr(40, 40, 2, 2)]), 20))
            .expect("present past the cap interval must flip");
        // All four damaged regions must be covered (disjoint here → 4 rects).
        // A coalescer that DROPPED deferred damage would miss some of these.
        for want in [pr(10, 10, 2, 2), pr(20, 20, 2, 2), pr(30, 30, 2, 2), pr(40, 40, 2, 2)] {
            assert!(
                flipped.iter().any(|g| pr_contains(g, &want)),
                "deferred damage {want:?} must be covered by the coalesced flip {flipped:?}"
            );
        }
        // After the flip, the accumulator is empty (no leftover damage).
        assert!(matches!(c.on_present(Some(&[pr(0, 0, 2, 2)]), 21), CoalesceDecision::Defer));
    }

    #[test]
    fn coalescer_full_present_subsumes_partial_damage() {
        let mut c = RemotePresentCoalescer::new(true, 60);
        // First flips.
        let _ = c.on_present(Some(&[pr(0, 0, 2, 2)]), 0);
        // Defer a partial, then a FULL present arrives (still within interval).
        assert!(matches!(c.on_present(Some(&[pr(10, 10, 2, 2)]), 2), CoalesceDecision::Defer));
        assert!(matches!(c.on_present(None, 4), CoalesceDecision::Defer));
        // The flip past the interval must be a FULL present (None), because a
        // full present subsumes every accumulated rect.
        match c.on_present(Some(&[pr(20, 20, 2, 2)]), 20) {
            CoalesceDecision::PresentNow(None) => {}
            other => panic!("a coalesced full present must flip the WHOLE surface, got {:?}",
                match other { CoalesceDecision::PresentNow(Some(_)) => "PresentNow(partial)",
                              CoalesceDecision::PresentNow(None) => "PresentNow(full)",
                              CoalesceDecision::Defer => "Defer" }),
        }
    }

    #[test]
    fn coalescer_disabled_when_cap_none_via_platform_flag() {
        // The platform treats `remote_present_cap_hz == None` as "coalescing
        // disabled" — even a forced-remote session presents every frame. This
        // guards the configurable disable knob.
        let mut platform = Win32Platform::new().expect("create Win32 platform");
        platform.set_remote_session_override(Some(true));
        platform.set_remote_present_cap_hz(None);
        assert!(platform.is_remote_session(), "override must force remote=true");
        assert_eq!(platform.remote_present_cap_hz(), None, "cap disabled");

        // And the override can force a non-remote answer regardless of host.
        platform.set_remote_session_override(Some(false));
        assert!(!platform.is_remote_session());
    }
}
