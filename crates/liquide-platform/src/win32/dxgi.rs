//! DXGI / D3D11 swap chain presenter.
//!
//! Creates a D3D11 device and DXGI swap chain for a given HWND, then
//! presents CPU-rendered BGRA8 frames via `UpdateSubresource` on the
//! swap-chain back buffer.
//!
//! Benefits over GDI:
//! - DXGI flip-model swap effect for lower latency
//! - Optional tearing support for immediate presentation
//! - Foundation for eventual GPU-accelerated rendering
//!
//! Falls back gracefully if D3D11/DXGI is unavailable (returns `Err`
//! from `DxgiPresenter::new` so the caller can fall back to GDI).

use std::ffi::c_void;
use std::ptr;

use super::ffi;

// ---------------------------------------------------------------------------
// COM helpers
// ---------------------------------------------------------------------------

type HRESULT = i32;
type REFIID = *const GUID;

#[repr(C)]
#[derive(Clone, Copy)]
struct GUID {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

const S_OK: HRESULT = 0;

macro_rules! guid {
    ($d1:expr, $d2:expr, $d3:expr, $d4:expr) => {
        GUID {
            data1: $d1,
            data2: $d2,
            data3: $d3,
            data4: $d4,
        }
    };
}

/// IID_IDXGIFactory1 {770AAE78-F26F-4DBA-A829-253C83D1B387}
const IID_IDXGI_FACTORY1: GUID =
    guid!(0x770aae78, 0xf26f, 0x4dba, [0xa8, 0x29, 0x25, 0x3c, 0x83, 0xd1, 0xb3, 0x87]);

/// IID_ID3D11Texture2D {6F15AAF2-D208-4E89-9AB4-489535D34F9C}
const IID_ID3D11_TEXTURE2D: GUID =
    guid!(0x6f15aaf2, 0xd208, 0x4e89, [0x9a, 0xb4, 0x48, 0x95, 0x35, 0xd3, 0x4f, 0x9c]);

// ---------------------------------------------------------------------------
// DXGI / D3D11 constants
// ---------------------------------------------------------------------------

const DXGI_FORMAT_B8G8R8A8_UNORM: u32 = 87;
const DXGI_USAGE_RENDER_TARGET_OUTPUT: u32 = 1 << 5; // 0x20
const DXGI_SWAP_EFFECT_FLIP_DISCARD: u32 = 4;
const DXGI_SWAP_EFFECT_DISCARD: u32 = 0;
const D3D_DRIVER_TYPE_HARDWARE: u32 = 1;
const D3D_FEATURE_LEVEL_11_0: u32 = 0xb000;
const D3D11_SDK_VERSION: u32 = 7;
const D3D11_CREATE_DEVICE_BGRA_SUPPORT: u32 = 0x20;
const DXGI_SWAP_CHAIN_FLAG_ALLOW_TEARING: u32 = 0x00000080;
const DXGI_PRESENT_ALLOW_TEARING: u32 = 0x00000200;

// ---------------------------------------------------------------------------
// DXGI / D3D11 structures
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Clone, Copy)]
struct DXGI_MODE_DESC {
    width: u32,
    height: u32,
    refresh_rate_numerator: u32,
    refresh_rate_denominator: u32,
    format: u32,
    scanline_ordering: u32,
    scaling: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct DXGI_SAMPLE_DESC {
    count: u32,
    quality: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct DXGI_SWAP_CHAIN_DESC {
    buffer_desc: DXGI_MODE_DESC,
    sample_desc: DXGI_SAMPLE_DESC,
    buffer_usage: u32,
    buffer_count: u32,
    output_window: ffi::HWND,
    windowed: ffi::BOOL,
    swap_effect: u32,
    flags: u32,
}

// ---------------------------------------------------------------------------
// COM vtable interfaces (minimal — only functions we use)
// ---------------------------------------------------------------------------

/// IUnknown vtable (first 3 entries of every COM interface).
#[repr(C)]
#[allow(dead_code)]
struct IUnknownVtbl {
    query_interface:
        unsafe extern "system" fn(*mut c_void, REFIID, *mut *mut c_void) -> HRESULT,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
}

// We access COM objects through raw pointers and vtable offsets.
// Each "interface" is a `*mut c_void` whose first `usize` points to
// the vtable. We index past the IUnknown entries to reach each
// interface's own methods.

// IDXGIFactory1 vtable layout (after IUnknown + IDXGIObject + IDXGIFactory):
//   IUnknown:       0  QueryInterface
//                   1  AddRef
//                   2  Release
//   IDXGIObject:    3  SetPrivateData
//                   4  SetPrivateDataInterface
//                   5  GetPrivateData
//                   6  GetParent
//   IDXGIFactory:   7  EnumAdapters
//                   8  MakeWindowAssociation
//                   9  GetWindowAssociation
//                  10  CreateSwapChain
//                  11  CreateSoftwareAdapter
//   IDXGIFactory1: 12  EnumAdapters1
//                  13  IsCurrent

// IDXGISwapChain vtable layout (after IUnknown + IDXGIObject + IDXGIDeviceSubObject):
//   IUnknown:              0-2
//   IDXGIObject:           3-6
//   IDXGIDeviceSubObject:  7  GetDevice
//   IDXGISwapChain:        8  Present
//                          9  GetBuffer
//                         10  SetFullscreenState
//                         11  GetFullscreenState
//                         12  GetDesc
//                         13  ResizeBuffers

// ID3D11DeviceContext vtable:
//   IUnknown: 0-2
//   ID3D11DeviceChild: 3-6
//   ...
//   UpdateSubresource = slot 48

// ---------------------------------------------------------------------------
// Helper: call a vtable slot with a given signature
// ---------------------------------------------------------------------------

/// Read the vtable pointer from a COM object and return the function at `slot`.
unsafe fn vtable_fn(obj: *mut c_void, slot: usize) -> *const c_void {
    unsafe {
        let vtbl = *(obj as *const *const *const c_void);
        *vtbl.add(slot)
    }
}

// ---------------------------------------------------------------------------
// External functions
// ---------------------------------------------------------------------------

#[link(name = "d3d11")]
unsafe extern "system" {
    fn D3D11CreateDevice(
        adapter: *mut c_void,           // IDXGIAdapter*
        driver_type: u32,               // D3D_DRIVER_TYPE
        software: *mut c_void,          // HMODULE
        flags: u32,                     // D3D11_CREATE_DEVICE_FLAG
        feature_levels: *const u32,     // D3D_FEATURE_LEVEL*
        num_feature_levels: u32,
        sdk_version: u32,
        device: *mut *mut c_void,       // ID3D11Device**
        feature_level: *mut u32,        // D3D_FEATURE_LEVEL*
        context: *mut *mut c_void,      // ID3D11DeviceContext**
    ) -> HRESULT;
}

#[link(name = "dxgi")]
unsafe extern "system" {
    fn CreateDXGIFactory1(
        riid: REFIID,
        factory: *mut *mut c_void,
    ) -> HRESULT;
}

// ---------------------------------------------------------------------------
// DxgiPresenter
// ---------------------------------------------------------------------------

/// Owns a DXGI swap chain and D3D11 device/context for frame presentation.
///
/// On each frame the caller's BGRA8 pixel buffer is uploaded directly
/// to the swap chain back buffer via `UpdateSubresource` and then
/// presented.  No staging texture is needed.
pub struct DxgiPresenter {
    #[allow(dead_code)]
    device: *mut c_void,        // ID3D11Device
    context: *mut c_void,       // ID3D11DeviceContext
    swap_chain: *mut c_void,    // IDXGISwapChain
    width: u32,
    height: u32,
    tearing: bool,              // swap chain supports ALLOW_TEARING
}

// Safety: the COM pointers are only accessed from the thread that created
// the device (the message-loop thread), which is the same thread that
// calls present(). We enforce this structurally.
unsafe impl Send for DxgiPresenter {}

impl DxgiPresenter {
    /// Create a new DXGI presenter for the given HWND and dimensions.
    ///
    /// Returns `Err` if D3D11/DXGI initialization fails (the caller
    /// should fall back to GDI presentation).
    pub fn new(hwnd: ffi::HWND, width: u32, height: u32) -> Result<Self, String> {
        unsafe { Self::init(hwnd, width, height) }
    }

    unsafe fn init(hwnd: ffi::HWND, width: u32, height: u32) -> Result<Self, String> {
        // 1. Create D3D11 device and immediate context.
        let mut device: *mut c_void = ptr::null_mut();
        let mut context: *mut c_void = ptr::null_mut();
        let mut feature_level: u32 = 0;
        let levels = [D3D_FEATURE_LEVEL_11_0];

        let hr = unsafe {
            D3D11CreateDevice(
                ptr::null_mut(),            // default adapter
                D3D_DRIVER_TYPE_HARDWARE,
                ptr::null_mut(),            // no software rasterizer
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                levels.as_ptr(),
                levels.len() as u32,
                D3D11_SDK_VERSION,
                &mut device,
                &mut feature_level,
                &mut context,
            )
        };
        if hr != S_OK || device.is_null() {
            return Err(format!("D3D11CreateDevice failed: 0x{hr:08X}"));
        }

        // 2. Create DXGI factory.
        let mut factory: *mut c_void = ptr::null_mut();
        let hr = unsafe { CreateDXGIFactory1(&IID_IDXGI_FACTORY1, &mut factory) };
        if hr != S_OK || factory.is_null() {
            unsafe { Self::release(device); Self::release(context); }
            return Err(format!("CreateDXGIFactory1 failed: 0x{hr:08X}"));
        }

        // 3. Create swap chain.
        //    Triple-buffer with FLIP_DISCARD and tearing support for
        //    immediate, non-blocking presentation.
        let sc_desc = DXGI_SWAP_CHAIN_DESC {
            buffer_desc: DXGI_MODE_DESC {
                width,
                height,
                refresh_rate_numerator: 0, // let driver choose
                refresh_rate_denominator: 1,
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                scanline_ordering: 0,
                scaling: 0,
            },
            sample_desc: DXGI_SAMPLE_DESC {
                count: 1,
                quality: 0,
            },
            buffer_usage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            buffer_count: 3,  // triple-buffer to avoid Present blocking on DWM
            output_window: hwnd,
            windowed: ffi::TRUE,
            swap_effect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
            flags: DXGI_SWAP_CHAIN_FLAG_ALLOW_TEARING,
        };

        let mut swap_chain: *mut c_void = ptr::null_mut();

        // IDXGIFactory::CreateSwapChain is vtable slot 10.
        type CreateSwapChainFn = unsafe extern "system" fn(
            this: *mut c_void,
            device: *mut c_void,
            desc: *const DXGI_SWAP_CHAIN_DESC,
            swap_chain: *mut *mut c_void,
        ) -> HRESULT;
        let create_sc: CreateSwapChainFn =
            unsafe { std::mem::transmute(vtable_fn(factory, 10)) };
        let hr = unsafe { create_sc(factory, device, &sc_desc, &mut swap_chain) };

        // If flip-discard + tearing fails, try without tearing flag.
        if hr != S_OK || swap_chain.is_null() {
            let mut sc_desc_no_tear = sc_desc;
            sc_desc_no_tear.flags = 0;
            let hr2 = unsafe { create_sc(factory, device, &sc_desc_no_tear, &mut swap_chain) };

            // If that also fails, try classic discard model.
            if hr2 != S_OK || swap_chain.is_null() {
                let mut sc_desc_fallback = sc_desc;
                sc_desc_fallback.swap_effect = DXGI_SWAP_EFFECT_DISCARD;
                sc_desc_fallback.buffer_count = 1;
                sc_desc_fallback.flags = 0;
                let hr3 = unsafe { create_sc(factory, device, &sc_desc_fallback, &mut swap_chain) };
                if hr3 != S_OK || swap_chain.is_null() {
                    unsafe {
                        Self::release(factory);
                        Self::release(device);
                        Self::release(context);
                    }
                    return Err(format!(
                        "CreateSwapChain failed: 0x{hr:08X} / 0x{hr2:08X} / 0x{hr3:08X}"
                    ));
                }
            }
        }

        // Determine whether the swap chain was created with tearing support.
        let tearing = hr == S_OK && !swap_chain.is_null();

        // Release factory (no longer needed).
        unsafe { Self::release(factory); }

        Ok(Self {
            device,
            context,
            swap_chain,
            width,
            height,
            tearing,
        })
    }

    /// Present a BGRA8 pixel buffer to the swap chain.
    ///
    /// Uploads `pixels` directly into the swap-chain back buffer via
    /// `UpdateSubresource`, then calls `Present(0, flags)` for immediate
    /// presentation (no vsync — the caller handles frame pacing).
    pub fn present(&mut self, pixels: &[u8], width: u32, height: u32, stride: u32) -> Result<(), String> {
        if width != self.width || height != self.height {
            self.resize(width, height)?;
        }

        unsafe {
            // 1. Acquire the current back buffer.
            let mut back_buffer: *mut c_void = ptr::null_mut();
            // IDXGISwapChain::GetBuffer = vtable slot 9
            type GetBufferFn = unsafe extern "system" fn(
                this: *mut c_void,
                buffer: u32,
                riid: REFIID,
                surface: *mut *mut c_void,
            ) -> HRESULT;
            let get_buffer: GetBufferFn = std::mem::transmute(vtable_fn(self.swap_chain, 9));
            let hr = get_buffer(self.swap_chain, 0, &IID_ID3D11_TEXTURE2D, &mut back_buffer);
            if hr != S_OK || back_buffer.is_null() {
                return Err(format!("GetBuffer failed: 0x{hr:08X}"));
            }

            // 2. Upload CPU pixels into the back buffer.
            //    UpdateSubresource copies from system memory to GPU memory
            //    in a single call — no staging texture needed.
            // ID3D11DeviceContext::UpdateSubresource = vtable slot 48
            type UpdateSubresourceFn = unsafe extern "system" fn(
                this: *mut c_void,
                dst_resource: *mut c_void,
                dst_subresource: u32,
                dst_box: *const c_void,    // NULL = entire resource
                src_data: *const c_void,
                src_row_pitch: u32,
                src_depth_pitch: u32,
            );
            let update: UpdateSubresourceFn =
                std::mem::transmute(vtable_fn(self.context, 48));
            update(
                self.context,
                back_buffer,
                0,
                ptr::null(),
                pixels.as_ptr() as *const c_void,
                stride,
                0,
            );

            // 3. Release back buffer reference before presenting.
            Self::release(back_buffer);

            // 4. Present immediately (no vsync wait).
            //    With tearing support, use DXGI_PRESENT_ALLOW_TEARING for
            //    truly immediate presentation; otherwise present with
            //    sync_interval = 0 which still skips vsync.
            // IDXGISwapChain::Present = vtable slot 8
            type PresentFn = unsafe extern "system" fn(
                this: *mut c_void,
                sync_interval: u32,
                flags: u32,
            ) -> HRESULT;
            let present: PresentFn = std::mem::transmute(vtable_fn(self.swap_chain, 8));
            let present_flags = if self.tearing { DXGI_PRESENT_ALLOW_TEARING } else { 0 };
            let hr = present(self.swap_chain, 0, present_flags);
            if hr != S_OK {
                return Err(format!("Present failed: 0x{hr:08X}"));
            }
        }

        Ok(())
    }

    /// Resize the swap chain buffers.
    fn resize(&mut self, width: u32, height: u32) -> Result<(), String> {
        unsafe {
            // IDXGISwapChain::ResizeBuffers = vtable slot 13
            type ResizeBuffersFn = unsafe extern "system" fn(
                this: *mut c_void,
                buffer_count: u32,
                width: u32,
                height: u32,
                format: u32,
                flags: u32,
            ) -> HRESULT;
            let resize: ResizeBuffersFn = std::mem::transmute(vtable_fn(self.swap_chain, 13));
            let flags = if self.tearing { DXGI_SWAP_CHAIN_FLAG_ALLOW_TEARING } else { 0 };
            let hr = resize(self.swap_chain, 0, width, height, DXGI_FORMAT_B8G8R8A8_UNORM, flags);
            if hr != S_OK {
                return Err(format!("ResizeBuffers failed: 0x{hr:08X}"));
            }

            self.width = width;
            self.height = height;
        }
        Ok(())
    }

    /// Release a COM object.
    unsafe fn release(obj: *mut c_void) {
        if !obj.is_null() {
            unsafe {
                let vtbl = *(obj as *const *const *const c_void);
                let release_fn: unsafe extern "system" fn(*mut c_void) -> u32 =
                    std::mem::transmute(*vtbl.add(2));
                release_fn(obj);
            }
        }
    }
}

impl Drop for DxgiPresenter {
    fn drop(&mut self) {
        unsafe {
            Self::release(self.swap_chain);
            Self::release(self.context);
            Self::release(self.device);
        }
    }
}
