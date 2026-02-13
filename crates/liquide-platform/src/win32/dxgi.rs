//! DXGI / D3D11 swap chain presenter.
//!
//! Creates a D3D11 device and DXGI swap chain for a given HWND, then
//! presents CPU-rendered BGRA8 frames via a staging texture upload
//! instead of GDI's `SetDIBitsToDevice`.
//!
//! Benefits over GDI:
//! - DXGI flip-model swap effect for lower latency
//! - VSync support via `Present(1, 0)`
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
const D3D11_USAGE_STAGING: u32 = 3;
const D3D11_CPU_ACCESS_WRITE: u32 = 0x10000;
const D3D11_MAP_WRITE_DISCARD: u32 = 4;
const D3D11_CREATE_DEVICE_BGRA_SUPPORT: u32 = 0x20;

// ---------------------------------------------------------------------------
// DXGI / D3D11 structures
// ---------------------------------------------------------------------------

#[repr(C)]
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
struct DXGI_SAMPLE_DESC {
    count: u32,
    quality: u32,
}

#[repr(C)]
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

#[repr(C)]
struct D3D11_TEXTURE2D_DESC {
    width: u32,
    height: u32,
    mip_levels: u32,
    array_size: u32,
    format: u32,
    sample_desc: DXGI_SAMPLE_DESC,
    usage: u32,
    bind_flags: u32,
    cpu_access_flags: u32,
    misc_flags: u32,
}

#[repr(C)]
struct D3D11_MAPPED_SUBRESOURCE {
    data: *mut c_void,
    row_pitch: u32,
    depth_pitch: u32,
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

// ID3D11Device vtable: we only need CreateTexture2D (slot 5 after IUnknown)
//   IUnknown:   0-2
//   ID3D11Device: 3  CreateBuffer
//                 4  CreateTexture1D
//                 5  CreateTexture2D

// ID3D11DeviceContext vtable:
//   IUnknown: 0-2
//            ... many methods ...
//   Map      = slot 14
//   Unmap    = slot 15
//   ...
//   CopyResource = slot 47

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
/// On each frame, the caller's BGRA8 pixel buffer is uploaded to a
/// staging texture, copied to the swap chain back buffer, and presented.
pub struct DxgiPresenter {
    device: *mut c_void,        // ID3D11Device
    context: *mut c_void,       // ID3D11DeviceContext
    swap_chain: *mut c_void,    // IDXGISwapChain
    staging: *mut c_void,       // ID3D11Texture2D (staging, CPU-writable)
    width: u32,
    height: u32,
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
            return Err(format!("D3D11CreateDevice failed: 0x{:08X}", hr));
        }

        // 2. Create DXGI factory.
        let mut factory: *mut c_void = ptr::null_mut();
        let hr = unsafe { CreateDXGIFactory1(&IID_IDXGI_FACTORY1, &mut factory) };
        if hr != S_OK || factory.is_null() {
            unsafe { Self::release(device); Self::release(context); }
            return Err(format!("CreateDXGIFactory1 failed: 0x{:08X}", hr));
        }

        // 3. Create swap chain.
        let sc_desc = DXGI_SWAP_CHAIN_DESC {
            buffer_desc: DXGI_MODE_DESC {
                width,
                height,
                refresh_rate_numerator: 60,
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
            buffer_count: 2,
            output_window: hwnd,
            windowed: ffi::TRUE,
            swap_effect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
            flags: 0,
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

        // If flip-discard fails (older Windows), try classic discard.
        if hr != S_OK || swap_chain.is_null() {
            let mut sc_desc_fallback = sc_desc;
            sc_desc_fallback.swap_effect = DXGI_SWAP_EFFECT_DISCARD;
            sc_desc_fallback.buffer_count = 1;
            let hr2 = unsafe { create_sc(factory, device, &sc_desc_fallback, &mut swap_chain) };
            if hr2 != S_OK || swap_chain.is_null() {
                unsafe {
                    Self::release(factory);
                    Self::release(device);
                    Self::release(context);
                }
                return Err(format!("CreateSwapChain failed: 0x{:08X} / 0x{:08X}", hr, hr2));
            }
        }

        // Release factory (no longer needed).
        unsafe { Self::release(factory); }

        // 4. Create a staging texture for CPU → GPU uploads.
        let staging = unsafe { Self::create_staging_texture(device, width, height)? };

        Ok(Self {
            device,
            context,
            swap_chain,
            staging,
            width,
            height,
        })
    }

    /// Present a BGRA8 pixel buffer to the swap chain.
    ///
    /// Uploads `pixels` into the staging texture, copies to the back
    /// buffer, and calls `IDXGISwapChain::Present(0, 0)` (immediate, no
    /// vsync wait — the caller is responsible for frame rate limiting).
    pub fn present(&mut self, pixels: &[u8], width: u32, height: u32, stride: u32) -> Result<(), String> {
        if width != self.width || height != self.height {
            self.resize(width, height)?;
        }

        unsafe {
            // Map the staging texture for writing.
            let mut mapped = D3D11_MAPPED_SUBRESOURCE {
                data: ptr::null_mut(),
                row_pitch: 0,
                depth_pitch: 0,
            };

            // ID3D11DeviceContext::Map = vtable slot 14
            type MapFn = unsafe extern "system" fn(
                this: *mut c_void,
                resource: *mut c_void,
                subresource: u32,
                map_type: u32,
                map_flags: u32,
                mapped: *mut D3D11_MAPPED_SUBRESOURCE,
            ) -> HRESULT;
            let map_fn: MapFn = std::mem::transmute(vtable_fn(self.context, 14));
            let hr = map_fn(self.context, self.staging, 0, D3D11_MAP_WRITE_DISCARD, 0, &mut mapped);
            if hr != S_OK {
                return Err(format!("Map staging texture failed: 0x{:08X}", hr));
            }

            // Copy pixels into the staging texture.
            let src_stride = stride as usize;
            let dst_stride = mapped.row_pitch as usize;
            let row_bytes = (width as usize) * 4;
            let dst = mapped.data as *mut u8;
            let total_bytes = row_bytes * height as usize;

            if src_stride == dst_stride && src_stride == row_bytes {
                // Strides match and are tightly packed — single bulk copy.
                ptr::copy_nonoverlapping(
                    pixels.as_ptr(),
                    dst,
                    total_bytes,
                );
            } else {
                // Stride mismatch — copy row by row.
                for y in 0..height as usize {
                    let src_off = y * src_stride;
                    let dst_off = y * dst_stride;
                    ptr::copy_nonoverlapping(
                        pixels.as_ptr().add(src_off),
                        dst.add(dst_off),
                        row_bytes,
                    );
                }
            }

            // Unmap.
            // ID3D11DeviceContext::Unmap = vtable slot 15
            type UnmapFn = unsafe extern "system" fn(
                this: *mut c_void,
                resource: *mut c_void,
                subresource: u32,
            );
            let unmap_fn: UnmapFn = std::mem::transmute(vtable_fn(self.context, 15));
            unmap_fn(self.context, self.staging, 0);

            // Get the back buffer.
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
                return Err(format!("GetBuffer failed: 0x{:08X}", hr));
            }

            // CopyResource from staging to back buffer.
            // ID3D11DeviceContext::CopyResource = vtable slot 47
            type CopyResourceFn = unsafe extern "system" fn(
                this: *mut c_void,
                dst: *mut c_void,
                src: *mut c_void,
            );
            let copy_fn: CopyResourceFn = std::mem::transmute(vtable_fn(self.context, 47));
            copy_fn(self.context, back_buffer, self.staging);

            // Release back buffer reference.
            Self::release(back_buffer);

            // Present immediately (no vsync wait).
            // The desktop event loop already throttles to the target frame
            // rate, so blocking on vsync here just adds latency.
            // IDXGISwapChain::Present = vtable slot 8
            type PresentFn = unsafe extern "system" fn(
                this: *mut c_void,
                sync_interval: u32,
                flags: u32,
            ) -> HRESULT;
            let present: PresentFn = std::mem::transmute(vtable_fn(self.swap_chain, 8));
            let hr = present(self.swap_chain, 0, 0);
            if hr != S_OK {
                return Err(format!("Present failed: 0x{:08X}", hr));
            }
        }

        Ok(())
    }

    /// Resize the swap chain and staging texture.
    fn resize(&mut self, width: u32, height: u32) -> Result<(), String> {
        unsafe {
            // Release old staging texture.
            Self::release(self.staging);
            self.staging = ptr::null_mut();

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
            let hr = resize(self.swap_chain, 0, width, height, DXGI_FORMAT_B8G8R8A8_UNORM, 0);
            if hr != S_OK {
                return Err(format!("ResizeBuffers failed: 0x{:08X}", hr));
            }

            // Create new staging texture.
            self.staging = Self::create_staging_texture(self.device, width, height)?;
            self.width = width;
            self.height = height;
        }
        Ok(())
    }

    unsafe fn create_staging_texture(
        device: *mut c_void,
        width: u32,
        height: u32,
    ) -> Result<*mut c_void, String> {
        let desc = D3D11_TEXTURE2D_DESC {
            width,
            height,
            mip_levels: 1,
            array_size: 1,
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            sample_desc: DXGI_SAMPLE_DESC { count: 1, quality: 0 },
            usage: D3D11_USAGE_STAGING,
            bind_flags: 0,
            cpu_access_flags: D3D11_CPU_ACCESS_WRITE,
            misc_flags: 0,
        };

        let mut texture: *mut c_void = ptr::null_mut();

        // ID3D11Device::CreateTexture2D = vtable slot 5
        type CreateTexture2DFn = unsafe extern "system" fn(
            this: *mut c_void,
            desc: *const D3D11_TEXTURE2D_DESC,
            initial_data: *const c_void,
            texture: *mut *mut c_void,
        ) -> HRESULT;
        let create: CreateTexture2DFn = unsafe { std::mem::transmute(vtable_fn(device, 5)) };
        let hr = unsafe { create(device, &desc, ptr::null(), &mut texture) };
        if hr != S_OK || texture.is_null() {
            return Err(format!("CreateTexture2D (staging) failed: 0x{:08X}", hr));
        }
        Ok(texture)
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
            Self::release(self.staging);
            Self::release(self.swap_chain);
            Self::release(self.context);
            Self::release(self.device);
        }
    }
}
