//! Raw Cocoa / AppKit / Core Graphics FFI declarations.
//!
//! This module contains all the Objective-C runtime, AppKit, Foundation, and
//! Core Graphics type definitions, constants, and extern function declarations
//! needed by the macOS platform backend.  No external crate dependencies are
//! used -- we link directly to system frameworks and the Objective-C runtime
//! dylib.

#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]
#![allow(clippy::upper_case_acronyms)]
#![allow(dead_code)]

use std::ffi::{c_void, CString};
use std::os::raw::{c_char, c_int, c_uint};

// ---------------------------------------------------------------------------
// Fundamental Objective-C runtime types
// ---------------------------------------------------------------------------

/// An Objective-C object pointer (`id`).
pub type id = *mut c_void;
/// An Objective-C selector (`SEL`).
pub type SEL = *mut c_void;
/// An Objective-C class pointer (`Class`).
pub type Class = *mut c_void;
/// Boolean used by the Objective-C runtime.
pub type BOOL = i8;

pub const YES: BOOL = 1;
pub const NO: BOOL = 0;

/// A null `id`.
pub const NIL: id = std::ptr::null_mut();

// ---------------------------------------------------------------------------
// Core Graphics scalar and geometry types
// ---------------------------------------------------------------------------

/// CGFloat is `f64` on 64-bit macOS.
pub type CGFloat = f64;

/// Unsigned integer used by Foundation (NSUInteger).
pub type NSUInteger = usize;

/// Signed integer used by Foundation (NSInteger).
pub type NSInteger = isize;

/// Opaque Core Graphics color space reference.
pub type CGColorSpaceRef = *mut c_void;

/// Opaque Core Graphics bitmap context reference.
pub type CGContextRef = *mut c_void;

/// Opaque Core Graphics image reference.
pub type CGImageRef = *mut c_void;

/// A Core Graphics / AppKit point.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NSPoint {
    pub x: CGFloat,
    pub y: CGFloat,
}

/// A Core Graphics / AppKit size.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NSSize {
    pub width: CGFloat,
    pub height: CGFloat,
}

/// A Core Graphics / AppKit rectangle.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NSRect {
    pub origin: NSPoint,
    pub size: NSSize,
}

impl NSRect {
    /// Create a new rectangle.
    pub fn new(x: CGFloat, y: CGFloat, width: CGFloat, height: CGFloat) -> Self {
        Self {
            origin: NSPoint { x, y },
            size: NSSize { width, height },
        }
    }
}

/// A Core Graphics rectangle (same layout as NSRect on 64-bit).
pub type CGRect = NSRect;
/// A Core Graphics point.
pub type CGPoint = NSPoint;
/// A Core Graphics size.
pub type CGSize = NSSize;

// ---------------------------------------------------------------------------
// NSWindow style mask flags
// ---------------------------------------------------------------------------

/// NSWindowStyleMask values.
pub const NSWindowStyleMaskBorderless: NSUInteger = 0;
pub const NSWindowStyleMaskTitled: NSUInteger = 1 << 0;
pub const NSWindowStyleMaskClosable: NSUInteger = 1 << 1;
pub const NSWindowStyleMaskMiniaturizable: NSUInteger = 1 << 2;
pub const NSWindowStyleMaskResizable: NSUInteger = 1 << 3;
pub const NSWindowStyleMaskFullScreen: NSUInteger = 1 << 14;

/// Common style mask for a standard resizable window.
pub const NSWindowStyleMaskDefault: NSUInteger = NSWindowStyleMaskTitled
    | NSWindowStyleMaskClosable
    | NSWindowStyleMaskMiniaturizable
    | NSWindowStyleMaskResizable;

// ---------------------------------------------------------------------------
// NSBackingStoreType
// ---------------------------------------------------------------------------

pub const NSBackingStoreBuffered: NSUInteger = 2;

// ---------------------------------------------------------------------------
// NSApplicationActivationPolicy
// ---------------------------------------------------------------------------

pub const NSApplicationActivationPolicyRegular: NSInteger = 0;
pub const NSApplicationActivationPolicyAccessory: NSInteger = 1;
pub const NSApplicationActivationPolicyProhibited: NSInteger = 2;

// ---------------------------------------------------------------------------
// NSEvent type constants
// ---------------------------------------------------------------------------

pub const NSEventTypeLeftMouseDown: NSUInteger = 1;
pub const NSEventTypeLeftMouseUp: NSUInteger = 2;
pub const NSEventTypeRightMouseDown: NSUInteger = 3;
pub const NSEventTypeRightMouseUp: NSUInteger = 4;
pub const NSEventTypeMouseMoved: NSUInteger = 5;
pub const NSEventTypeLeftMouseDragged: NSUInteger = 6;
pub const NSEventTypeRightMouseDragged: NSUInteger = 7;
pub const NSEventTypeKeyDown: NSUInteger = 10;
pub const NSEventTypeKeyUp: NSUInteger = 11;
pub const NSEventTypeFlagsChanged: NSUInteger = 12;
pub const NSEventTypeScrollWheel: NSUInteger = 22;
pub const NSEventTypeOtherMouseDown: NSUInteger = 25;
pub const NSEventTypeOtherMouseUp: NSUInteger = 26;
pub const NSEventTypeOtherMouseDragged: NSUInteger = 27;

// ---------------------------------------------------------------------------
// NSEventModifierFlags
// ---------------------------------------------------------------------------

pub const NSEventModifierFlagCapsLock: u64 = 1 << 16;
pub const NSEventModifierFlagShift: u64 = 1 << 17;
pub const NSEventModifierFlagControl: u64 = 1 << 18;
pub const NSEventModifierFlagOption: u64 = 1 << 19; // Alt
pub const NSEventModifierFlagCommand: u64 = 1 << 20; // Super / Cmd

// ---------------------------------------------------------------------------
// NSEvent matching mask (for nextEvent)
// ---------------------------------------------------------------------------

/// Match any event type.
pub const NSEventMaskAny: u64 = u64::MAX;

// ---------------------------------------------------------------------------
// Core Graphics bitmap info / alpha info constants
// ---------------------------------------------------------------------------

pub const kCGImageAlphaPremultipliedFirst: u32 = 2;
pub const kCGImageAlphaNoneSkipFirst: u32 = 6;
pub const kCGBitmapByteOrder32Little: u32 = 2 << 12;

/// Bitmap info for BGRA8 (little-endian 32-bit with alpha in the first byte).
pub const kCGBitmapInfoBGRA8: u32 =
    kCGImageAlphaPremultipliedFirst | kCGBitmapByteOrder32Little;

/// Bitmap info for BGRA8 with no alpha (skip first).
pub const kCGBitmapInfoBGRA8NoAlpha: u32 =
    kCGImageAlphaNoneSkipFirst | kCGBitmapByteOrder32Little;

// ---------------------------------------------------------------------------
// Objective-C runtime functions
// ---------------------------------------------------------------------------

#[link(name = "objc", kind = "dylib")]
unsafe extern "C" {
    /// Send a message to an Objective-C object.
    pub fn objc_msgSend(receiver: id, sel: SEL, ...) -> id;

    /// Look up a class by name.
    pub fn objc_getClass(name: *const c_char) -> Class;

    /// Register (intern) a selector by name.
    pub fn sel_registerName(name: *const c_char) -> SEL;

    /// Get the human-readable name of a class.
    pub fn class_getName(cls: Class) -> *const c_char;

    /// Allocate a new instance of a class.
    pub fn class_createInstance(cls: Class, extraBytes: usize) -> id;

    /// Get a selector's name.
    pub fn sel_getName(sel: SEL) -> *const c_char;
}

// ---------------------------------------------------------------------------
// Foundation framework functions
// ---------------------------------------------------------------------------

#[link(name = "Foundation", kind = "framework")]
unsafe extern "C" {
    /// Create an NSString from a UTF-8 C string (class method via msgSend).
    /// We declare NSLog here since it is in Foundation.
    pub fn NSLog(format: id, ...);
}

// ---------------------------------------------------------------------------
// AppKit framework functions
// ---------------------------------------------------------------------------

#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {
    /// NSApplication main entry point (rarely called directly; provided for
    /// completeness).
    pub fn NSApplicationMain(argc: c_int, argv: *const *const c_char) -> c_int;
}

// ---------------------------------------------------------------------------
// Core Graphics framework functions
// ---------------------------------------------------------------------------

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    pub fn CGColorSpaceCreateDeviceRGB() -> CGColorSpaceRef;
    pub fn CGColorSpaceRelease(space: CGColorSpaceRef);

    pub fn CGBitmapContextCreate(
        data: *mut c_void,
        width: usize,
        height: usize,
        bitsPerComponent: usize,
        bytesPerRow: usize,
        space: CGColorSpaceRef,
        bitmapInfo: u32,
    ) -> CGContextRef;

    pub fn CGBitmapContextCreateImage(context: CGContextRef) -> CGImageRef;

    pub fn CGContextRelease(context: CGContextRef);
    pub fn CGImageRelease(image: CGImageRef);

    pub fn CGContextDrawImage(context: CGContextRef, rect: CGRect, image: CGImageRef);
    pub fn CGContextFlush(context: CGContextRef);

    /// Get the main display ID.
    pub fn CGMainDisplayID() -> u32;
    /// Get the width in pixels of a display.
    pub fn CGDisplayPixelsWide(display: u32) -> usize;
    /// Get the height in pixels of a display.
    pub fn CGDisplayPixelsHigh(display: u32) -> usize;
}

// ---------------------------------------------------------------------------
// Helper: create a Rust-friendly selector
// ---------------------------------------------------------------------------

/// Register an Objective-C selector from a byte-string literal.
///
/// # Safety
///
/// The name must be a valid null-terminated C string. This is a thin
/// wrapper around `sel_registerName`.
#[inline]
pub unsafe fn sel(name: &[u8]) -> SEL {
    // SAFETY: The caller passes a null-terminated byte literal.
    // sel_registerName is an ObjC runtime function that returns a
    // valid SEL for any C string.
    unsafe { sel_registerName(name.as_ptr() as *const c_char) }
}

/// Look up an Objective-C class by name (null-terminated byte string).
///
/// # Safety
///
/// The name must be a valid null-terminated C string.
#[inline]
pub unsafe fn class(name: &[u8]) -> Class {
    // SAFETY: The caller passes a null-terminated byte literal.
    // objc_getClass is an ObjC runtime function that returns a valid
    // Class (or null if the class doesn't exist).
    unsafe { objc_getClass(name.as_ptr() as *const c_char) }
}

// ---------------------------------------------------------------------------
// Message send helpers (typed wrappers around objc_msgSend)
// ---------------------------------------------------------------------------
//
// The Objective-C runtime requires callers to cast `objc_msgSend` to the
// correct function pointer type for each call signature.  These helpers
// provide the most commonly used signatures.  More exotic signatures
// (e.g. returning NSRect by value) require `objc_msgSend_stret` on some
// architectures; on 64-bit macOS (both x86_64 and arm64), structs up to
// 2 registers are returned in registers so plain `objc_msgSend` suffices
// for NSRect.
//
// SAFETY (applies to all msg_send_* functions below):
// Each function transmutes `objc_msgSend` to a function pointer matching
// the ObjC method's C calling convention. The transmute is sound because:
//   1. objc_msgSend is defined as a variadic extern "C" trampoline.
//   2. The target signature matches the Objective-C method being invoked.
//   3. The caller is responsible for passing a valid receiver and selector.
// The resulting function pointer is immediately called with the provided
// arguments.

/// Send a message that returns `id` with no extra arguments.
#[inline]
pub unsafe fn msg_send_id(receiver: id, sel: SEL) -> id {
    // SAFETY: See section-level SAFETY comment above.
    let f: unsafe extern "C" fn(id, SEL) -> id =
        unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(receiver, sel) }
}

/// Send a message that returns `id` with one `id` argument.
#[inline]
pub unsafe fn msg_send_id_id(receiver: id, sel: SEL, arg: id) -> id {
    // SAFETY: See section-level SAFETY comment above.
    let f: unsafe extern "C" fn(id, SEL, id) -> id =
        unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(receiver, sel, arg) }
}

/// Send a message that returns `id` with one `BOOL` argument.
#[inline]
pub unsafe fn msg_send_id_bool(receiver: id, sel: SEL, arg: BOOL) -> id {
    // SAFETY: See section-level SAFETY comment above.
    let f: unsafe extern "C" fn(id, SEL, BOOL) -> id =
        unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(receiver, sel, arg) }
}

/// Send a message that returns nothing with no arguments.
#[inline]
pub unsafe fn msg_send_void(receiver: id, sel: SEL) {
    // SAFETY: See section-level SAFETY comment above.
    let f: unsafe extern "C" fn(id, SEL) =
        unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(receiver, sel) }
}

/// Send a message that returns nothing with one `BOOL` argument.
#[inline]
pub unsafe fn msg_send_void_bool(receiver: id, sel: SEL, arg: BOOL) {
    // SAFETY: See section-level SAFETY comment above.
    let f: unsafe extern "C" fn(id, SEL, BOOL) =
        unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(receiver, sel, arg) }
}

/// Send a message that returns nothing with one `NSInteger` argument.
#[inline]
pub unsafe fn msg_send_void_nsinteger(receiver: id, sel: SEL, arg: NSInteger) {
    // SAFETY: See section-level SAFETY comment above.
    let f: unsafe extern "C" fn(id, SEL, NSInteger) =
        unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(receiver, sel, arg) }
}

/// Send a message that returns nothing with one `id` argument.
#[inline]
pub unsafe fn msg_send_void_id(receiver: id, sel: SEL, arg: id) {
    // SAFETY: See section-level SAFETY comment above.
    let f: unsafe extern "C" fn(id, SEL, id) =
        unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(receiver, sel, arg) }
}

/// Send a message that returns a `BOOL` with no extra arguments.
#[inline]
pub unsafe fn msg_send_bool(receiver: id, sel: SEL) -> BOOL {
    // SAFETY: See section-level SAFETY comment above.
    let f: unsafe extern "C" fn(id, SEL) -> BOOL =
        unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(receiver, sel) }
}

/// Send a message that returns an `NSUInteger` with no extra arguments.
#[inline]
pub unsafe fn msg_send_nsuinteger(receiver: id, sel: SEL) -> NSUInteger {
    // SAFETY: See section-level SAFETY comment above.
    let f: unsafe extern "C" fn(id, SEL) -> NSUInteger =
        unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(receiver, sel) }
}

/// Send a message that returns an `NSInteger` with no extra arguments.
#[inline]
pub unsafe fn msg_send_nsinteger(receiver: id, sel: SEL) -> NSInteger {
    // SAFETY: See section-level SAFETY comment above.
    let f: unsafe extern "C" fn(id, SEL) -> NSInteger =
        unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(receiver, sel) }
}

/// Send a message that returns a `u16` with no extra arguments.
#[inline]
pub unsafe fn msg_send_u16(receiver: id, sel: SEL) -> u16 {
    // SAFETY: See section-level SAFETY comment above.
    let f: unsafe extern "C" fn(id, SEL) -> u16 =
        unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(receiver, sel) }
}

/// Send a message that returns a `u64` with no extra arguments.
#[inline]
pub unsafe fn msg_send_u64(receiver: id, sel: SEL) -> u64 {
    // SAFETY: See section-level SAFETY comment above.
    let f: unsafe extern "C" fn(id, SEL) -> u64 =
        unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(receiver, sel) }
}

/// Send a message that returns a `CGFloat` with no extra arguments.
#[inline]
pub unsafe fn msg_send_cgfloat(receiver: id, sel: SEL) -> CGFloat {
    // SAFETY: See section-level SAFETY comment above.
    let f: unsafe extern "C" fn(id, SEL) -> CGFloat =
        unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(receiver, sel) }
}

/// Send a message that returns an NSPoint with no extra arguments.
#[inline]
pub unsafe fn msg_send_nspoint(receiver: id, sel: SEL) -> NSPoint {
    // SAFETY: See section-level SAFETY comment above.
    let f: unsafe extern "C" fn(id, SEL) -> NSPoint =
        unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(receiver, sel) }
}

/// Send a message that returns an NSRect with no extra arguments.
///
/// On 64-bit macOS (both x86_64 and arm64), NSRect (4 doubles = 32 bytes)
/// is returned in registers, so plain `objc_msgSend` is correct.
#[inline]
pub unsafe fn msg_send_nsrect(receiver: id, sel: SEL) -> NSRect {
    // SAFETY: See section-level SAFETY comment above. On 64-bit macOS,
    // NSRect (4 doubles = 32 bytes) is returned in registers.
    let f: unsafe extern "C" fn(id, SEL) -> NSRect =
        unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(receiver, sel) }
}

/// NSWindow initWithContentRect:styleMask:backing:defer:
///
/// Signature: `- (instancetype)initWithContentRect:(NSRect)contentRect
///   styleMask:(NSWindowStyleMask)style backing:(NSBackingStoreType)backingStoreType
///   defer:(BOOL)flag;`
#[inline]
pub unsafe fn msg_send_init_window(
    receiver: id,
    sel: SEL,
    content_rect: NSRect,
    style_mask: NSUInteger,
    backing: NSUInteger,
    defer: BOOL,
) -> id {
    // SAFETY: See section-level SAFETY comment. This signature matches
    // NSWindow's initWithContentRect:styleMask:backing:defer: exactly.
    let f: unsafe extern "C" fn(id, SEL, NSRect, NSUInteger, NSUInteger, BOOL) -> id =
        unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(receiver, sel, content_rect, style_mask, backing, defer) }
}

/// nextEventMatchingMask:untilDate:inMode:dequeue:
///
/// Signature: `- (NSEvent *)nextEventMatchingMask:(NSEventMask)mask
///   untilDate:(NSDate *)expiration inMode:(NSRunLoopMode)mode
///   dequeue:(BOOL)deqFlag;`
#[inline]
pub unsafe fn msg_send_next_event(
    receiver: id,
    sel: SEL,
    mask: u64,
    until_date: id,
    mode: id,
    dequeue: BOOL,
) -> id {
    // SAFETY: See section-level SAFETY comment. This signature matches
    // NSApplication's nextEventMatchingMask:untilDate:inMode:dequeue:.
    let f: unsafe extern "C" fn(id, SEL, u64, id, id, BOOL) -> id =
        unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(receiver, sel, mask, until_date, mode, dequeue) }
}

/// Send a message that returns nothing with one NSRect argument.
#[inline]
pub unsafe fn msg_send_void_nsrect(receiver: id, sel: SEL, rect: NSRect) {
    // SAFETY: See section-level SAFETY comment above.
    let f: unsafe extern "C" fn(id, SEL, NSRect) =
        unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(receiver, sel, rect) }
}

/// Send a message that returns `id` with one `*const c_char` (UTF-8) argument.
#[inline]
pub unsafe fn msg_send_id_cstr(receiver: id, sel: SEL, arg: *const c_char) -> id {
    // SAFETY: See section-level SAFETY comment above.
    let f: unsafe extern "C" fn(id, SEL, *const c_char) -> id =
        unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(receiver, sel, arg) }
}

// ---------------------------------------------------------------------------
// Convenience: create an NSString from a Rust &str
// ---------------------------------------------------------------------------

/// Create an autoreleased `NSString` from a Rust `&str`.
///
/// # Safety
///
/// Must be called within an active `@autoreleasepool` (i.e. an
/// `NSAutoreleasePool` must be on the stack).
#[inline]
pub unsafe fn nsstring(s: &str) -> id {
    // SAFETY: ObjC runtime calls to alloc and initWithUTF8String:.
    // The CString ensures a valid null-terminated buffer. Must be called
    // within an active @autoreleasepool.
    let cls = unsafe { class(b"NSString\0") };
    let alloc = unsafe { msg_send_id(cls, sel(b"alloc\0")) };
    let c_str = CString::new(s).expect("interior NUL in string passed to nsstring()");
    unsafe {
        msg_send_id_cstr(
            alloc,
            sel(b"initWithUTF8String:\0"),
            c_str.as_ptr(),
        )
    }
}
