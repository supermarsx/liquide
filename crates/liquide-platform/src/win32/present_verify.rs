//! Present-path verification facilities for the Win32 / RDP GDI present path.
//!
//! Background (t59/t62): over RDP there is no hardware DXGI swap-chain, so the
//! DE always takes the GDI fallback path. The flicker fix (t62 #1) replaced a
//! direct `SetDIBitsToDevice` to the visible window DC with an *off-screen
//! DIB-section back-buffer* + a single atomic `BitBlt` flip. That change cannot
//! be eyeballed by the coordinator and the offscreen render harness never
//! exercises the real present DC, so this module provides *automated* tooling to
//! verify it:
//!
//! 1. **Read-back assertion** — present a known frame through the off-screen
//!    DIB + BitBlt round-trip, read the ACTUAL presented pixels back, and assert
//!    they EQUAL the source (no partial / torn / missing rows). Proves the
//!    BitBlt is atomic and complete.
//! 2. **Self-test** — drive N distinct frames through the present round-trip and
//!    assert each read-back equals its source (catches tearing / stale-buffer
//!    regressions).
//! 3. **Live diagnostic** — used by the `present-verify` bin to capture each
//!    presented frame to PNG and write a short report.
//! 4. **Metrics** — present-path + frame-completeness counters.
//!
//! ## Headless vs live split
//!
//! - The *pure* functions ([`fill_dib_from_source`], [`compare_frames`],
//!   [`make_test_pattern`], [`encode_png_bgra`], [`PresentVerifyMetrics`]) have
//!   no Win32 dependency and are unit-tested on any platform (CI-able).
//! - The *live* functions ([`live`] sub-module, gated `#[cfg(windows)]`) create
//!   real GDI objects and a real window; they require a live Windows session
//!   (and, for the RDP-specific behaviour, an actual RDP session). They are
//!   driven by the `present-verify` bin and the `#[ignore]`d live tests.
//!
//! Clean-room: nothing here references any leaked source; all Win32 usage is
//! from public API documentation, mirrored from the existing `win32` backend.

#![allow(clippy::needless_range_loop)]

/// 4 bytes per pixel (BGRA8 / 32-bit BI_RGB).
pub const BYTES_PER_PIXEL: usize = 4;

// ---------------------------------------------------------------------------
// Metrics (headless)
// ---------------------------------------------------------------------------

/// Which present path a frame went through. Logged for the live report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentPath {
    /// Hardware DXGI swap-chain.
    Dxgi,
    /// GDI fallback (off-screen DIB-section + atomic BitBlt) — the RDP path.
    GdiOffscreenDib,
}

impl PresentPath {
    /// Human-readable label used in logs / reports.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            PresentPath::Dxgi => "DXGI hardware swap-chain",
            PresentPath::GdiOffscreenDib => "GDI fallback (off-screen DIB + BitBlt)",
        }
    }
}

/// Counters for the present path + frame completeness. Cheap to clone; meant to
/// be accumulated over a verification run and logged at the end.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PresentVerifyMetrics {
    /// Frames driven through the present round-trip.
    pub frames_presented: u64,
    /// Frames whose read-back EQUALLED the source exactly (complete, no tearing).
    pub frames_complete: u64,
    /// Frames whose read-back DIFFERED from the source (torn / partial / stale).
    pub frames_incomplete: u64,
    /// Presents that took the GDI off-screen DIB path (the RDP-relevant path).
    pub presents_via_gdi: u64,
    /// Presents that took the DXGI hardware path.
    pub presents_via_dxgi: u64,
}

impl PresentVerifyMetrics {
    /// Record one frame outcome.
    pub fn record(&mut self, path: PresentPath, complete: bool) {
        self.frames_presented = self.frames_presented.saturating_add(1);
        if complete {
            self.frames_complete = self.frames_complete.saturating_add(1);
        } else {
            self.frames_incomplete = self.frames_incomplete.saturating_add(1);
        }
        match path {
            PresentPath::Dxgi => self.presents_via_dxgi = self.presents_via_dxgi.saturating_add(1),
            PresentPath::GdiOffscreenDib => {
                self.presents_via_gdi = self.presents_via_gdi.saturating_add(1)
            }
        }
    }

    /// True when every presented frame read back complete.
    #[must_use]
    pub fn all_complete(&self) -> bool {
        self.frames_presented > 0 && self.frames_incomplete == 0
    }

    /// One-line summary suitable for a log / report.
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "frames_presented={} complete={} incomplete={} gdi={} dxgi={} all_complete={}",
            self.frames_presented,
            self.frames_complete,
            self.frames_incomplete,
            self.presents_via_gdi,
            self.presents_via_dxgi,
            self.all_complete(),
        )
    }
}

// ---------------------------------------------------------------------------
// Frame comparison (headless) — the read-back assertion core
// ---------------------------------------------------------------------------

/// Result of comparing a presented (read-back) frame against its source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameComparison {
    /// Total rows in the frame.
    pub rows: u32,
    /// Number of rows that matched the source byte-for-byte.
    pub matching_rows: u32,
    /// Indices of the first few mismatching rows (for diagnostics), capped.
    pub first_mismatched_rows: Vec<u32>,
    /// Total bytes that differed across the whole frame.
    pub mismatched_bytes: u64,
}

impl FrameComparison {
    /// True when EVERY row matched: the presented frame is the complete source
    /// with no partial / torn / missing rows.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.rows > 0 && self.matching_rows == self.rows && self.mismatched_bytes == 0
    }
}

/// Compare a read-back frame against the source frame, row by row.
///
/// Both buffers are interpreted as top-down packed BGRA8 of `width` x `height`
/// (stride == `width * 4`). This is the exact layout the GDI off-screen DIB and
/// the source framebuffer share, so a complete atomic BitBlt yields an exact
/// match. A torn frame (compositor sampled mid-write) or a stale back-buffer
/// shows up as one or more mismatching rows.
///
/// Returns a per-row breakdown so a test can assert completeness and a report
/// can point at the first torn rows.
#[must_use]
pub fn compare_frames(source: &[u8], readback: &[u8], width: u32, height: u32) -> FrameComparison {
    let row_bytes = (width as usize) * BYTES_PER_PIXEL;
    let mut matching_rows = 0u32;
    let mut first_mismatched_rows = Vec::new();
    let mut mismatched_bytes = 0u64;

    for row in 0..height as usize {
        let start = row * row_bytes;
        let end = start + row_bytes;
        let src_row = source.get(start..end);
        let rb_row = readback.get(start..end);
        match (src_row, rb_row) {
            (Some(s), Some(r)) if s == r => {
                matching_rows += 1;
            }
            (Some(s), Some(r)) => {
                if first_mismatched_rows.len() < 16 {
                    first_mismatched_rows.push(row as u32);
                }
                mismatched_bytes += s
                    .iter()
                    .zip(r.iter())
                    .filter(|(a, b)| a != b)
                    .count() as u64;
            }
            _ => {
                // A row that is missing entirely (buffer too short) counts as a
                // fully-mismatched row — exactly the "missing rows" failure mode
                // the assertion must catch.
                if first_mismatched_rows.len() < 16 {
                    first_mismatched_rows.push(row as u32);
                }
                mismatched_bytes += row_bytes as u64;
            }
        }
    }

    FrameComparison {
        rows: height,
        matching_rows,
        first_mismatched_rows,
        mismatched_bytes,
    }
}

/// Mirror of the production GDI present copy: fill a destination DIB buffer from
/// a packed top-down BGRA8 source via a single contiguous copy.
///
/// This is the *exact* operation `present_frame`'s GDI path performs
/// (`ptr::copy_nonoverlapping(pixels -> dib.bits, required)`), lifted into a
/// pure, testable function. `dst` must be at least `width * height * 4` bytes.
/// Returns the number of bytes copied, or `None` if `dst` or `src` is too small
/// (the production path returns a `Presentation` error in that case).
#[must_use]
pub fn fill_dib_from_source(dst: &mut [u8], src: &[u8], width: u32, height: u32) -> Option<usize> {
    let required = (width as usize)
        .checked_mul(BYTES_PER_PIXEL)?
        .checked_mul(height as usize)?;
    if src.len() < required || dst.len() < required {
        return None;
    }
    dst[..required].copy_from_slice(&src[..required]);
    Some(required)
}

// ---------------------------------------------------------------------------
// Test patterns (headless)
// ---------------------------------------------------------------------------

/// Generate a distinct, deterministic BGRA8 test pattern for frame index `n`.
///
/// Each frame is visually distinct (so a stale back-buffer that presents an old
/// frame is detectable) and has per-row variation (so a torn frame — some rows
/// from a previous frame — is detectable). Layout: top-down packed BGRA8.
#[must_use]
pub fn make_test_pattern(width: u32, height: u32, n: u32) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    let mut buf = vec![0u8; w * h * BYTES_PER_PIXEL];
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * BYTES_PER_PIXEL;
            // Vary by frame index, row, and column so any swap/tear is visible.
            let b = ((x.wrapping_add(n as usize)) & 0xFF) as u8;
            let g = ((y.wrapping_add((n as usize).wrapping_mul(3))) & 0xFF) as u8;
            let r = ((x ^ y).wrapping_add(n as usize * 7) & 0xFF) as u8;
            buf[i] = b; // B
            buf[i + 1] = g; // G
            buf[i + 2] = r; // R
            buf[i + 3] = 0xFF; // A
        }
    }
    buf
}

// ---------------------------------------------------------------------------
// Minimal PNG encoder (headless, no external deps)
// ---------------------------------------------------------------------------

/// Encode a top-down packed BGRA8 buffer as an RGBA PNG (uncompressed / stored
/// zlib blocks — no external crate). Returns the PNG file bytes.
///
/// Used by the live diagnostic bin to dump each presented frame to `target/`.
#[must_use]
pub fn encode_png_bgra(bgra: &[u8], width: u32, height: u32) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;

    // 1. Raw image data: filter byte (0 = None) + RGBA row, top-down.
    let mut raw = Vec::with_capacity(h * (1 + w * 4));
    for y in 0..h {
        raw.push(0u8); // filter type: None
        for x in 0..w {
            let i = (y * w + x) * 4;
            // BGRA -> RGBA
            raw.push(bgra.get(i + 2).copied().unwrap_or(0)); // R
            raw.push(bgra.get(i + 1).copied().unwrap_or(0)); // G
            raw.push(bgra.get(i).copied().unwrap_or(0)); // B
            raw.push(bgra.get(i + 3).copied().unwrap_or(0xFF)); // A
        }
    }

    // 2. zlib stream wrapping stored (uncompressed) DEFLATE blocks.
    let zlib = zlib_store(&raw);

    // 3. PNG container.
    let mut png = Vec::new();
    png.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);

    // IHDR
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.push(8); // bit depth
    ihdr.push(6); // color type: RGBA
    ihdr.push(0); // compression
    ihdr.push(0); // filter
    ihdr.push(0); // interlace
    write_chunk(&mut png, b"IHDR", &ihdr);

    // IDAT
    write_chunk(&mut png, b"IDAT", &zlib);

    // IEND
    write_chunk(&mut png, b"IEND", &[]);

    png
}

fn write_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc_input = Vec::with_capacity(4 + data.len());
    crc_input.extend_from_slice(kind);
    crc_input.extend_from_slice(data);
    out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
}

/// Wrap `data` in a zlib stream using stored (uncompressed) DEFLATE blocks.
fn zlib_store(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    // zlib header: CMF=0x78 (deflate, 32K window), FLG=0x01 (no dict, check ok).
    out.push(0x78);
    out.push(0x01);

    // Stored DEFLATE blocks: max 65535 bytes each.
    let mut offset = 0usize;
    while offset < data.len() || data.is_empty() {
        let remaining = data.len() - offset;
        let block = remaining.min(0xFFFF);
        let is_final = offset + block >= data.len();
        out.push(if is_final { 1 } else { 0 }); // BFINAL, BTYPE=00 (stored)
        let len = block as u16;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(&data[offset..offset + block]);
        offset += block;
        if data.is_empty() {
            break;
        }
    }

    // Adler-32 checksum of the uncompressed data.
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

fn adler32(data: &[u8]) -> u32 {
    const MOD: u32 = 65521;
    let mut a = 1u32;
    let mut b = 0u32;
    for &byte in data {
        a = (a + byte as u32) % MOD;
        b = (b + a) % MOD;
    }
    (b << 16) | a
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

// ---------------------------------------------------------------------------
// Live (Windows-only) present round-trip
// ---------------------------------------------------------------------------

#[cfg(windows)]
pub mod live {
    //! Live GDI present round-trip: present a frame through a real off-screen
    //! DIB-section + atomic `BitBlt` to a target DC, then read the *actual*
    //! presented pixels back via a second `BitBlt` into a read-back DIB. This
    //! exercises the same GDI mechanism the production `present_frame` GDI path
    //! uses, so the read-back proves the BitBlt is atomic and complete.
    //!
    //! Requires a live Windows session. The RDP-specific behaviour
    //! ([`is_remote_session`]) only reports `true` under an actual RDP session.

    use super::{compare_frames, FrameComparison, BYTES_PER_PIXEL};
    use crate::win32::ffi;
    use std::ffi::c_void;
    use std::ptr;

    /// True when running inside a Remote Desktop (RDP) session.
    #[must_use]
    pub fn is_remote_session() -> bool {
        // SAFETY: GetSystemMetrics is a pure query with no preconditions.
        unsafe { ffi::GetSystemMetrics(ffi::SM_REMOTESESSION) != 0 }
    }

    /// An off-screen DIB section bound to a memory DC, exposing its pixel bytes.
    ///
    /// Mirrors the production `GdiBackBuffer` but additionally lets a verifier
    /// READ the bits back (the production buffer is write-only from outside).
    struct DibSurface {
        mem_dc: ffi::HDC,
        bitmap: ffi::HBITMAP,
        old_bitmap: ffi::HGDIOBJ,
        bits: *mut c_void,
        width: u32,
        height: u32,
    }

    impl DibSurface {
        /// Create a top-down BGRA8 DIB surface compatible with `reference_dc`.
        unsafe fn create(reference_dc: ffi::HDC, width: u32, height: u32) -> Option<Self> {
            if width == 0 || height == 0 {
                return None;
            }
            let mem_dc = unsafe { ffi::CreateCompatibleDC(reference_dc) };
            if mem_dc.is_null() {
                return None;
            }
            let bmi = ffi::BITMAPINFO {
                bmiHeader: ffi::BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<ffi::BITMAPINFOHEADER>() as ffi::DWORD,
                    biWidth: width as ffi::LONG,
                    biHeight: -(height as ffi::LONG), // top-down
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
                unsafe { ffi::DeleteDC(mem_dc) };
                return None;
            }
            let old_bitmap = unsafe { ffi::SelectObject(mem_dc, bitmap) };
            Some(DibSurface {
                mem_dc,
                bitmap,
                old_bitmap,
                bits,
                width,
                height,
            })
        }

        fn byte_len(&self) -> usize {
            (self.width as usize) * (self.height as usize) * BYTES_PER_PIXEL
        }

        /// Copy `src` (packed top-down BGRA8) into this surface's DIB memory.
        /// Returns false if `src` is too small.
        unsafe fn write_pixels(&mut self, src: &[u8]) -> bool {
            let n = self.byte_len();
            if src.len() < n {
                return false;
            }
            // GDI may have buffered drawing into the DIB; flush before we touch
            // the shared memory directly.
            unsafe { ffi::GdiFlush() };
            unsafe { ptr::copy_nonoverlapping(src.as_ptr(), self.bits as *mut u8, n) };
            true
        }

        /// Read this surface's DIB memory out into a Vec (packed top-down BGRA8).
        unsafe fn read_pixels(&self) -> Vec<u8> {
            let n = self.byte_len();
            // Flush any pending GDI ops that targeted this DIB before reading.
            unsafe { ffi::GdiFlush() };
            let mut out = vec![0u8; n];
            unsafe { ptr::copy_nonoverlapping(self.bits as *const u8, out.as_mut_ptr(), n) };
            out
        }
    }

    impl Drop for DibSurface {
        fn drop(&mut self) {
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

    /// Outcome of one live present round-trip.
    #[derive(Debug, Clone)]
    pub struct RoundTrip {
        /// The pixels read back from the destination DC after the present.
        pub readback: Vec<u8>,
        /// Per-row comparison against the source.
        pub comparison: FrameComparison,
    }

    /// Present `source` through an off-screen DIB + atomic BitBlt onto
    /// `target_dc`, then read the presented pixels back from `target_dc` and
    /// compare to the source.
    ///
    /// This is the real GDI mechanism the production present path uses. When
    /// `target_dc` is a window DC this presents to a real (possibly RDP-remote)
    /// window; when it is a memory DC the round-trip is fully self-contained and
    /// can run on any Windows host (including headless CI runners) — see
    /// [`present_roundtrip_offscreen`].
    ///
    /// # Safety
    /// `target_dc` must be a valid DC at least `width` x `height` in extent.
    pub unsafe fn present_and_readback(
        target_dc: ffi::HDC,
        source: &[u8],
        width: u32,
        height: u32,
    ) -> Option<RoundTrip> {
        // 1. Off-screen back-buffer (the production "GdiBackBuffer").
        let mut back = unsafe { DibSurface::create(target_dc, width, height) }?;
        unsafe { back.write_pixels(source) };

        // 2. Atomic flip: single BitBlt from the off-screen DIB to the target.
        let ok = unsafe {
            ffi::BitBlt(
                target_dc,
                0,
                0,
                width as i32,
                height as i32,
                back.mem_dc,
                0,
                0,
                ffi::SRCCOPY,
            )
        };
        if ok == ffi::FALSE {
            return None;
        }

        // 3. Read back the ACTUAL presented pixels: BitBlt target -> readback DIB.
        let readback_surface = unsafe { DibSurface::create(target_dc, width, height) }?;
        let ok = unsafe {
            ffi::BitBlt(
                readback_surface.mem_dc,
                0,
                0,
                width as i32,
                height as i32,
                target_dc,
                0,
                0,
                ffi::SRCCOPY,
            )
        };
        if ok == ffi::FALSE {
            return None;
        }
        let readback = unsafe { readback_surface.read_pixels() };
        let comparison = compare_frames(source, &readback, width, height);
        Some(RoundTrip {
            readback,
            comparison,
        })
    }

    /// Fully self-contained present round-trip against an off-screen memory DC
    /// (no window). Creates a destination memory DIB, then runs the same
    /// off-screen-DIB + BitBlt + read-back as [`present_and_readback`].
    ///
    /// This needs only GDI (no visible window, no message pump), so it runs on
    /// any Windows host — it is the live read-back assertion that CI on a
    /// Windows runner can execute. It proves the BitBlt copy itself is atomic
    /// and complete; the truly-remote (RDP compositor sampling) behaviour still
    /// requires a real RDP window, exercised by the `present-verify` bin.
    #[must_use]
    pub fn present_roundtrip_offscreen(source: &[u8], width: u32, height: u32) -> Option<RoundTrip> {
        unsafe {
            // A screen-compatible DC to base the destination surface on.
            let screen_dc = ffi::GetDC(ptr::null_mut());
            if screen_dc.is_null() {
                return None;
            }
            let dest = DibSurface::create(screen_dc, width, height);
            ffi::ReleaseDC(ptr::null_mut(), screen_dc);
            let dest = dest?;
            present_and_readback(dest.mem_dc, source, width, height)
        }
    }

    /// Outcome of one frame in a live windowed capture.
    #[derive(Debug, Clone)]
    pub struct WindowedFrame {
        /// Frame index (0-based).
        pub index: u32,
        /// The pixels actually presented (read back from the window DC).
        pub readback: Vec<u8>,
        /// Per-row comparison against the source frame.
        pub comparison: FrameComparison,
    }

    /// A minimal visible window for live present verification. Registers its own
    /// window class on creation and tears everything down on drop.
    pub struct VerifyWindow {
        hwnd: ffi::HWND,
        class_atom: ffi::ATOM,
        hinstance: ffi::HINSTANCE,
        class_name: Vec<u16>,
    }

    unsafe extern "system" fn verify_wndproc(
        hwnd: ffi::HWND,
        msg: ffi::UINT,
        wparam: ffi::WPARAM,
        lparam: ffi::LPARAM,
    ) -> ffi::LRESULT {
        // We suppress WM_ERASEBKGND (return 1) so the background flash the
        // production backend also avoids does not contaminate the read-back, and
        // post a quit on destroy. Everything else is default-handled.
        const WM_ERASEBKGND: ffi::UINT = 0x0014;
        match msg {
            WM_ERASEBKGND => 1,
            ffi::WM_DESTROY => {
                unsafe { ffi::PostQuitMessage(0) };
                0
            }
            _ => unsafe { ffi::DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }

    impl VerifyWindow {
        /// Create and show a `width` x `height` (client area) verification window.
        pub fn create(width: u32, height: u32, title: &str) -> Option<Self> {
            let class_name: Vec<u16> = "LiquidePresentVerifyClass\0".encode_utf16().collect();
            let mut title_w: Vec<u16> = title.encode_utf16().collect();
            title_w.push(0);

            unsafe {
                let hinstance = ffi::GetModuleHandleW(ptr::null());
                let wc = ffi::WNDCLASSEXW {
                    cbSize: std::mem::size_of::<ffi::WNDCLASSEXW>() as ffi::UINT,
                    style: 0,
                    lpfnWndProc: Some(verify_wndproc),
                    cbClsExtra: 0,
                    cbWndExtra: 0,
                    hInstance: hinstance,
                    hIcon: ptr::null_mut(),
                    hCursor: ffi::LoadCursorW(ptr::null_mut(), ffi::IDC_ARROW),
                    hbrBackground: ptr::null_mut(),
                    lpszMenuName: ptr::null(),
                    lpszClassName: class_name.as_ptr(),
                    hIconSm: ptr::null_mut(),
                };
                let class_atom = ffi::RegisterClassExW(&wc);
                if class_atom == 0 {
                    return None;
                }

                let hwnd = ffi::CreateWindowExW(
                    0,
                    class_name.as_ptr(),
                    title_w.as_ptr(),
                    ffi::WS_OVERLAPPEDWINDOW | ffi::WS_VISIBLE,
                    ffi::CW_USEDEFAULT,
                    ffi::CW_USEDEFAULT,
                    width as i32,
                    height as i32,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    hinstance,
                    ptr::null_mut(),
                );
                if hwnd.is_null() {
                    ffi::UnregisterClassW(class_name.as_ptr(), hinstance);
                    return None;
                }
                ffi::ShowWindow(hwnd, ffi::SW_SHOW);
                ffi::UpdateWindow(hwnd);

                Some(VerifyWindow {
                    hwnd,
                    class_atom,
                    hinstance,
                    class_name,
                })
            }
        }

        /// The window's client-area size in pixels.
        #[must_use]
        pub fn client_size(&self) -> (u32, u32) {
            let mut rc = ffi::RECT::default();
            // SAFETY: hwnd is valid for the window's lifetime.
            unsafe { ffi::GetClientRect(self.hwnd, &mut rc) };
            (
                (rc.right - rc.left).max(0) as u32,
                (rc.bottom - rc.top).max(0) as u32,
            )
        }

        /// Drain pending window messages (lets the window paint / the compositor
        /// observe the present).
        pub fn pump_messages(&self) {
            unsafe {
                let mut msg = ffi::MSG::default();
                while ffi::PeekMessageW(&mut msg, ptr::null_mut(), 0, 0, ffi::PM_REMOVE)
                    != ffi::FALSE
                {
                    ffi::TranslateMessage(&msg);
                    ffi::DispatchMessageW(&msg);
                }
            }
        }

        /// Present `source` to this window via the off-screen-DIB + atomic BitBlt
        /// path, then read back the ACTUAL presented window pixels and compare.
        pub fn present_and_readback(&self, source: &[u8], width: u32, height: u32) -> Option<RoundTrip> {
            unsafe {
                let dc = ffi::GetDC(self.hwnd);
                if dc.is_null() {
                    return None;
                }
                let result = present_and_readback(dc, source, width, height);
                ffi::ReleaseDC(self.hwnd, dc);
                result
            }
        }
    }

    impl Drop for VerifyWindow {
        fn drop(&mut self) {
            unsafe {
                if !self.hwnd.is_null() {
                    ffi::DestroyWindow(self.hwnd);
                }
                if self.class_atom != 0 {
                    ffi::UnregisterClassW(self.class_name.as_ptr(), self.hinstance);
                }
            }
        }
    }

    /// Drive `frames` distinct test-pattern frames through a real visible window
    /// (the production-equivalent GDI present), reading each presented frame back
    /// from the window DC and comparing to the source.
    ///
    /// Returns one [`WindowedFrame`] per frame. Requires a live Windows session;
    /// the truly-remote behaviour only manifests when this process is in an RDP
    /// session ([`is_remote_session`] == true). This is what the `present-verify`
    /// bin uses for the user's live RDP check.
    #[must_use]
    pub fn run_windowed_capture(
        width: u32,
        height: u32,
        frames: u32,
    ) -> Option<Vec<WindowedFrame>> {
        let window = VerifyWindow::create(width, height, "liquide present-verify")?;
        // Use the actual client size (CreateWindowExW size includes borders).
        let (cw, ch) = window.client_size();
        let (w, h) = if cw == 0 || ch == 0 {
            (width, height)
        } else {
            (cw, ch)
        };

        let mut out = Vec::with_capacity(frames as usize);
        for n in 0..frames {
            window.pump_messages();
            let source = super::make_test_pattern(w, h, n);
            let rt = window.present_and_readback(&source, w, h)?;
            window.pump_messages();
            out.push(WindowedFrame {
                index: n,
                readback: rt.readback,
                comparison: rt.comparison,
            });
        }
        Some(out)
    }
}

// ---------------------------------------------------------------------------
// Tests (headless)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_dib_is_exact_contiguous_copy() {
        let src = make_test_pattern(8, 4, 1);
        let mut dst = vec![0u8; src.len()];
        let n = fill_dib_from_source(&mut dst, &src, 8, 4).expect("copy");
        assert_eq!(n, 8 * 4 * 4);
        assert_eq!(dst, src);
    }

    #[test]
    fn fill_dib_rejects_undersized_buffers() {
        let src = vec![0u8; 8 * 4 * 4];
        let mut small = vec![0u8; 8 * 4 * 4 - 1];
        assert_eq!(fill_dib_from_source(&mut small, &src, 8, 4), None);
        let mut ok = vec![0u8; 8 * 4 * 4];
        let short_src = vec![0u8; 8 * 4 * 4 - 1];
        assert_eq!(fill_dib_from_source(&mut ok, &short_src, 8, 4), None);
    }

    #[test]
    fn readback_equal_to_source_is_complete() {
        // Models a complete, atomic present: read-back == source.
        let src = make_test_pattern(16, 10, 3);
        let cmp = compare_frames(&src, &src, 16, 10);
        assert!(cmp.is_complete());
        assert_eq!(cmp.rows, 10);
        assert_eq!(cmp.matching_rows, 10);
        assert_eq!(cmp.mismatched_bytes, 0);
        assert!(cmp.first_mismatched_rows.is_empty());
    }

    #[test]
    fn torn_frame_is_detected_as_incomplete() {
        // Models tearing: rows 0..5 are frame N, rows 5..10 are an older frame.
        let new = make_test_pattern(16, 10, 5);
        let old = make_test_pattern(16, 10, 4);
        let row_bytes = 16 * 4;
        let mut torn = new.clone();
        for row in 5..10 {
            let s = row * row_bytes;
            let e = s + row_bytes;
            torn[s..e].copy_from_slice(&old[s..e]);
        }
        let cmp = compare_frames(&new, &torn, 16, 10);
        assert!(!cmp.is_complete());
        assert_eq!(cmp.matching_rows, 5);
        assert_eq!(cmp.first_mismatched_rows, vec![5, 6, 7, 8, 9]);
        assert!(cmp.mismatched_bytes > 0);
    }

    #[test]
    fn stale_frame_is_detected_as_incomplete() {
        // Models a stale back-buffer: presented an entirely older frame.
        let new = make_test_pattern(16, 10, 9);
        let stale = make_test_pattern(16, 10, 8);
        let cmp = compare_frames(&new, &stale, 16, 10);
        assert!(!cmp.is_complete());
        assert_eq!(cmp.matching_rows, 0);
    }

    #[test]
    fn missing_rows_are_detected_as_incomplete() {
        // Read-back buffer truncated (missing bottom rows).
        let new = make_test_pattern(16, 10, 2);
        let short = new[..16 * 4 * 6].to_vec();
        let cmp = compare_frames(&new, &short, 16, 10);
        assert!(!cmp.is_complete());
        assert_eq!(cmp.matching_rows, 6);
        assert_eq!(cmp.first_mismatched_rows, vec![6, 7, 8, 9]);
    }

    #[test]
    fn test_patterns_are_distinct_per_frame() {
        let a = make_test_pattern(32, 32, 0);
        let b = make_test_pattern(32, 32, 1);
        assert_ne!(a, b, "consecutive frames must differ so staleness is visible");
    }

    #[test]
    fn metrics_track_completeness_and_path() {
        let mut m = PresentVerifyMetrics::default();
        m.record(PresentPath::GdiOffscreenDib, true);
        m.record(PresentPath::GdiOffscreenDib, true);
        m.record(PresentPath::GdiOffscreenDib, false);
        assert_eq!(m.frames_presented, 3);
        assert_eq!(m.frames_complete, 2);
        assert_eq!(m.frames_incomplete, 1);
        assert_eq!(m.presents_via_gdi, 3);
        assert!(!m.all_complete());

        let mut clean = PresentVerifyMetrics::default();
        clean.record(PresentPath::GdiOffscreenDib, true);
        assert!(clean.all_complete());
    }

    #[test]
    fn png_encodes_valid_signature_and_dimensions() {
        let bgra = make_test_pattern(4, 3, 0);
        let png = encode_png_bgra(&bgra, 4, 3);
        // PNG signature.
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        // IHDR length (13) + "IHDR".
        assert_eq!(&png[8..12], &[0, 0, 0, 13]);
        assert_eq!(&png[12..16], b"IHDR");
        // Width/height big-endian in IHDR data (bytes 16..24).
        assert_eq!(&png[16..20], &4u32.to_be_bytes());
        assert_eq!(&png[20..24], &3u32.to_be_bytes());
        // Ends with IEND chunk.
        assert_eq!(&png[png.len() - 8..png.len() - 4], b"IEND");
    }

    #[test]
    fn adler_and_crc_match_known_values() {
        // Adler-32 of "abc" is 0x024D0127; CRC-32 (IEEE) of "abc" is 0x352441C2.
        assert_eq!(adler32(b"abc"), 0x024D_0127);
        assert_eq!(crc32(b"abc"), 0x3524_41C2);
    }
}
