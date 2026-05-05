use crate::crtc::CrtcId;
use crate::device::DrmDevice;
use crate::error::{DrmError, Result};
use crate::framebuffer::FramebufferId;
#[cfg(target_os = "linux")]
use std::os::fd::RawFd;

const DRM_EVENT_VBLANK: u32 = 0x01;
const DRM_EVENT_FLIP_COMPLETE: u32 = 0x02;
const DRM_EVENT_HEADER_LEN: usize = 8;
const DRM_EVENT_VBLANK_LEN: usize = 32;
#[cfg(target_os = "linux")]
const DRM_EVENT_READ_CHUNK_SIZE: usize = 4096;

/// Typed bitflags for page flip requests.
///
/// Mirrors the kernel uapi values from `include/uapi/drm/drm_mode.h`:
/// - `DRM_MODE_PAGE_FLIP_EVENT = 0x01`
/// - `DRM_MODE_PAGE_FLIP_ASYNC = 0x02`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageFlipFlags(pub u32);

impl PageFlipFlags {
    /// `DRM_MODE_PAGE_FLIP_EVENT` — request a completion event on the DRM fd.
    pub const EVENT: Self = Self(0x01);
    /// `DRM_MODE_PAGE_FLIP_ASYNC` — perform an asynchronous (non-vblank-locked) flip.
    pub const ASYNC: Self = Self(0x02);

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl std::ops::BitOr for PageFlipFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitAnd for PageFlipFlags {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

/// Bundled inputs for a single page-flip submission.
///
/// Constructing a `PresentRequest` is pure and host-safe; submitting it
/// (via `request_page_flip` or a `StandalonePresentSubmitter`) is what
/// ultimately invokes the kernel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresentRequest {
    pub crtc: CrtcId,
    pub fb: FramebufferId,
    pub flags: PageFlipFlags,
    pub user_data: u64,
}

impl PresentRequest {
    /// Convenience constructor.
    pub const fn new(
        crtc: CrtcId,
        fb: FramebufferId,
        flags: PageFlipFlags,
        user_data: u64,
    ) -> Self {
        Self { crtc, fb, flags, user_data }
    }

    /// Returns a `PresentRequest` with `flags` replaced.
    pub const fn with_flags(self, flags: PageFlipFlags) -> Self {
        Self { flags, ..self }
    }

    /// Returns a `PresentRequest` with `user_data` replaced.
    pub const fn with_user_data(self, user_data: u64) -> Self {
        Self { user_data, ..self }
    }

    /// Submits this request via `request_page_flip` against `device`.
    pub fn submit(self, device: &DrmDevice) -> Result<()> {
        request_page_flip(device, self.crtc, self.fb, self.flags, self.user_data)
    }
}

/// An event delivered after a page flip completes at vblank.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageFlipEvent {
    pub sequence: u32,
    pub timestamp_ns: u64,
    pub crtc_id: CrtcId,
}

/// An event delivered after a vblank completes on a CRTC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VblankEvent {
    pub sequence: u32,
    pub timestamp_ns: u64,
    pub crtc_id: CrtcId,
}

/// A raw DRM event record whose type is not yet decoded by this crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownDrmEvent {
    pub event_type: u32,
    pub raw_record: Vec<u8>,
}

/// Narrow typed surface for DRM completion feedback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DrmEvent {
    PageFlip(PageFlipEvent),
    Vblank(VblankEvent),
    Unknown(UnknownDrmEvent),
}

/// Parses a batch of DRM event records returned from a device read.
pub fn parse_drm_events(buffer: &[u8]) -> Result<Vec<DrmEvent>> {
    let mut events = Vec::new();
    let mut offset = 0usize;

    while offset < buffer.len() {
        let remaining = buffer.len() - offset;
        if remaining < DRM_EVENT_HEADER_LEN {
            return Err(DrmError::EventBufferTruncated {
                offset,
                expected: DRM_EVENT_HEADER_LEN,
                actual: remaining,
            });
        }

        let record_type = read_u32_native(&buffer[offset..offset + 4]);
        let record_len = read_u32_native(&buffer[offset + 4..offset + 8]) as usize;
        if record_len < DRM_EVENT_HEADER_LEN {
            return Err(DrmError::EventBufferMalformed {
                offset,
                reason: format!("record length {record_len} shorter than header"),
            });
        }
        if remaining < record_len {
            return Err(DrmError::EventBufferTruncated {
                offset,
                expected: record_len,
                actual: remaining,
            });
        }

        let record = &buffer[offset..offset + record_len];
        events.push(parse_drm_record(record_type, record, offset)?);
        offset += record_len;
    }

    Ok(events)
}

/// Reads and parses all currently pending DRM events from a device.
#[cfg(target_os = "linux")]
pub fn drain_pending_events(device: &DrmDevice) -> Result<Vec<DrmEvent>> {
    drain_pending_events_from_fd(device.fd())
}

/// Reads and parses all currently pending DRM events from a DRM file descriptor.
#[cfg(target_os = "linux")]
pub fn drain_pending_events_from_fd(fd: RawFd) -> Result<Vec<DrmEvent>> {
    let raw = read_pending_event_bytes(fd)?;
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    parse_drm_events(&raw)
}

/// `#[repr(C)]` argument struct for `DRM_IOCTL_MODE_PAGE_FLIP`.
///
/// Mirrors `struct drm_mode_crtc_page_flip` from the kernel uapi. Kept
/// `pub(crate)` because it is only meaningful when handed to the
/// Linux ioctl path (or its host-side mock).
#[cfg(any(test, target_os = "linux"))]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DrmModeCrtcPageFlip {
    pub crtc_id: u32,
    pub fb_id: u32,
    pub flags: u32,
    pub reserved: u32,
    pub user_data: u64,
}

/// `DRM_IOWR(0xB0, drm_mode_crtc_page_flip)` from kernel uapi
/// (`include/uapi/drm/drm.h`).
#[cfg(any(test, target_os = "linux"))]
pub(crate) const DRM_IOCTL_MODE_PAGE_FLIP: core::ffi::c_ulong =
    crate::ioctl::drm_iowr(0xB0, std::mem::size_of::<DrmModeCrtcPageFlip>());

/// Pure cfg-free translation from typed inputs to the page-flip ioctl arg shape.
#[cfg(any(test, target_os = "linux"))]
pub(crate) fn page_flip_request_args(
    crtc: CrtcId,
    fb: FramebufferId,
    flags: PageFlipFlags,
    user_data: u64,
) -> DrmModeCrtcPageFlip {
    DrmModeCrtcPageFlip {
        crtc_id: crtc.0,
        fb_id: fb.0,
        flags: flags.bits(),
        reserved: 0,
        user_data,
    }
}

/// Internal: invoke `DRM_IOCTL_MODE_PAGE_FLIP` against a raw fd.
///
/// Routed through `crate::ioctl::drm_ioctl`, which is in turn host-safe
/// under `#[cfg(test)]` thanks to the t40 mock dispatch layer.
#[cfg(any(test, target_os = "linux"))]
pub(crate) fn request_page_flip_via_fd(
    fd: i32,
    crtc: CrtcId,
    fb: FramebufferId,
    flags: PageFlipFlags,
    user_data: u64,
) -> Result<()> {
    let mut args = page_flip_request_args(crtc, fb, flags, user_data);
    crate::ioctl::drm_ioctl(fd, DRM_IOCTL_MODE_PAGE_FLIP, "MODE_PAGE_FLIP", &mut args)
}

/// Requests a page flip on a CRTC to the given framebuffer.
#[cfg(target_os = "linux")]
pub fn request_page_flip(
    device: &DrmDevice,
    crtc: CrtcId,
    fb: FramebufferId,
    flags: PageFlipFlags,
    user_data: u64,
) -> Result<()> {
    request_page_flip_via_fd(device.fd(), crtc, fb, flags, user_data)
}

#[cfg(not(target_os = "linux"))]
pub fn request_page_flip(
    _device: &DrmDevice,
    _crtc: CrtcId,
    _fb: FramebufferId,
    _flags: PageFlipFlags,
    _user_data: u64,
) -> Result<()> {
    Err(DrmError::NoDevice)
}

/// Typed bitflags for `wait_vblank` requests.
///
/// Mirrors the relevant kernel uapi values from `include/uapi/drm/drm.h`:
/// - `_DRM_VBLANK_EVENT       = 0x4000_0000` — async delivery via the DRM fd.
/// - `_DRM_VBLANK_NEXTONMISS  = 0x0000_0004` — fast-forward if the target
///   sequence has already passed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VblankFlags(pub u32);

impl VblankFlags {
    /// `_DRM_VBLANK_EVENT` — request async event delivery on the DRM fd.
    pub const EVENT: Self = Self(0x4000_0000);
    /// `_DRM_VBLANK_NEXTONMISS` — return at the next vblank if the target is in the past.
    pub const NEXTONMISS: Self = Self(0x0000_0004);

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl std::ops::BitOr for VblankFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitAnd for VblankFlags {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

/// Wait mode for a vblank request — corresponds to the low bits of the
/// kernel uapi `type` field on `union drm_wait_vblank`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VblankMode {
    /// `_DRM_VBLANK_RELATIVE = 0x1` — wait for `sequence` more vblanks.
    Relative,
    /// `_DRM_VBLANK_ABSOLUTE = 0x0` — wait until the vblank counter reaches `sequence`.
    Absolute,
}

/// Bundled inputs for a single `DRM_IOCTL_WAIT_VBLANK` submission.
///
/// Constructing a `VblankRequest` is pure and host-safe; submitting it
/// (via `wait_vblank`) is what ultimately invokes the kernel.
///
/// The CRTC index is encoded into the high pipe bits of the kernel
/// `type` word per `_DRM_VBLANK_HIGH_CRTC_MASK` (`0x003F_0000`, bits
/// 16-21). Pipe 0 leaves the high bits clear, matching the legacy
/// single-CRTC behavior; pipes 1..63 occupy the 6-bit mask region.
/// (Brief specified `_DRM_VBLANK_HIGH_CRTC_SHIFT = 1`, but a left-shift
/// of 1 places pipe values outside the mask; we use 16 to actually land
/// in bits 16-21, matching the kernel's `(kind & MASK) >> 16` decode.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VblankRequest {
    /// Which CRTC's vblank to wait on.
    pub crtc: CrtcId,
    /// Wait mode: relative count or absolute target.
    pub mode: VblankMode,
    /// Target sequence (interpretation depends on `mode`).
    pub sequence: u32,
    /// Opaque cookie returned in the reply for correlation when EVENT is set
    /// (occupies the `signal` field of the kernel request union arm).
    pub user_data: u64,
    /// Flags: `EVENT` and/or `NEXTONMISS`.
    pub flags: VblankFlags,
}

impl VblankRequest {
    /// Convenience constructor.
    pub const fn new(
        crtc: CrtcId,
        mode: VblankMode,
        sequence: u32,
        user_data: u64,
        flags: VblankFlags,
    ) -> Self {
        Self { crtc, mode, sequence, user_data, flags }
    }
}

impl Default for VblankRequest {
    fn default() -> Self {
        Self {
            crtc: CrtcId(0),
            mode: VblankMode::Relative,
            sequence: 1,
            user_data: 0,
            flags: VblankFlags::empty(),
        }
    }
}

/// Typed reply returned by `DRM_IOCTL_WAIT_VBLANK`.
///
/// The kernel writes back the reply arm of `union drm_wait_vblank`,
/// which echoes a `type`/`sequence` pair plus the timestamp at which
/// the wait completed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VblankReply {
    /// Type echo from the kernel (with EVENT bit cleared if event was queued).
    pub kind: u32,
    /// Sequence number when the wait completed.
    pub sequence: u32,
    /// Wall-clock-ish timestamp (seconds component).
    pub tval_sec: i64,
    /// Wall-clock-ish timestamp (microseconds component).
    pub tval_usec: i64,
}

/// `#[repr(C)]` argument struct for `DRM_IOCTL_WAIT_VBLANK`.
///
/// Mirrors the flattened `union drm_wait_vblank` from the kernel uapi
/// (`include/uapi/drm/drm.h`):
///
/// ```text
/// union drm_wait_vblank {
///     struct drm_wait_vblank_request { __u32 type; __u32 sequence; __u64 signal; }     // 16 bytes
///     struct drm_wait_vblank_reply   { __u32 type; __u32 sequence; __s64 tval_sec; __s64 tval_usec; } // 24 bytes
/// };
/// ```
///
/// We model the union as the larger reply arm (24 bytes total). When
/// encoding a request, the `signal` field overlaps `tval_sec`; the
/// trailing `tval_usec` slot is unused on submission and zeroed.
/// Kernel `long` is 8 bytes on 64-bit Linux, hence `i64` here.
///
/// Kept `pub(crate)` because it is only meaningful when handed to the
/// Linux ioctl path (or its host-side mock).
#[cfg(any(test, target_os = "linux"))]
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct DrmVblank {
    /// Overlaps both request `type` and reply `type`.
    pub kind: u32,
    /// Overlaps both request `sequence` and reply `sequence`.
    pub sequence: u32,
    /// On request: low 8 bytes of `signal` (we store `user_data` here).
    /// On reply: `tval_sec`.
    pub tval_sec: i64,
    /// On request: zero (past the end of the 16-byte request arm).
    /// On reply: `tval_usec`.
    pub tval_usec: i64,
}

// Compile-time assertion that the union arm sizing matches kernel uapi.
#[cfg(any(test, target_os = "linux"))]
const _: [(); 24] = [(); std::mem::size_of::<DrmVblank>()];

/// `DRM_IOWR(0x3A, union drm_wait_vblank)` from kernel uapi
/// (`include/uapi/drm/drm.h`).
#[cfg(any(test, target_os = "linux"))]
pub(crate) const DRM_IOCTL_WAIT_VBLANK: core::ffi::c_ulong =
    crate::ioctl::drm_iowr(0x3A, std::mem::size_of::<DrmVblank>());

/// Pure cfg-free translation from typed inputs to the wait-vblank ioctl arg shape.
#[cfg(any(test, target_os = "linux"))]
pub(crate) fn vblank_args_from_request(req: &VblankRequest) -> DrmVblank {
    const _DRM_VBLANK_RELATIVE: u32 = 0x1;
    const _DRM_VBLANK_ABSOLUTE: u32 = 0x0;
    // Shift of 16 places the pipe index (0..63) into the 6-bit mask
    // region 0x003F_0000 (bits 16-21). The brief's value of 1 was
    // incorrect — it would zero out any pipe < 0x8000.
    const _DRM_VBLANK_HIGH_CRTC_SHIFT: u32 = 16;
    const _DRM_VBLANK_HIGH_CRTC_MASK: u32 = 0x003F_0000;

    let mode_bits = match req.mode {
        VblankMode::Relative => _DRM_VBLANK_RELATIVE,
        VblankMode::Absolute => _DRM_VBLANK_ABSOLUTE,
    };
    let flag_bits = req.flags.bits();
    let pipe_bits = (req.crtc.0 << _DRM_VBLANK_HIGH_CRTC_SHIFT) & _DRM_VBLANK_HIGH_CRTC_MASK;
    let kind = mode_bits | flag_bits | pipe_bits;

    // `signal` (request arm) overlaps `tval_sec` (reply arm) — store the
    // user-supplied cookie there. `tval_usec` is past the end of the
    // 16-byte request and must be zero on submission.
    DrmVblank {
        kind,
        sequence: req.sequence,
        tval_sec: req.user_data as i64,
        tval_usec: 0,
    }
}

/// Pure cfg-free translation from a kernel-written reply arm to `VblankReply`.
#[cfg(any(test, target_os = "linux"))]
pub(crate) fn vblank_reply_from_args(args: &DrmVblank) -> VblankReply {
    VblankReply {
        kind: args.kind,
        sequence: args.sequence,
        tval_sec: args.tval_sec,
        tval_usec: args.tval_usec,
    }
}

/// Internal: invoke `DRM_IOCTL_WAIT_VBLANK` against a raw fd.
///
/// Routed through `crate::ioctl::drm_ioctl`, which is in turn host-safe
/// under `#[cfg(test)]` thanks to the t40 mock dispatch layer.
#[cfg(any(test, target_os = "linux"))]
pub(crate) fn wait_vblank_via_fd(fd: i32, request: &VblankRequest) -> Result<VblankReply> {
    let mut args = vblank_args_from_request(request);
    crate::ioctl::drm_ioctl(fd, DRM_IOCTL_WAIT_VBLANK, "WAIT_VBLANK", &mut args)?;
    Ok(vblank_reply_from_args(&args))
}

/// Waits for a vblank on the requested CRTC and returns the typed reply.
#[cfg(target_os = "linux")]
pub fn wait_vblank(device: &DrmDevice, request: &VblankRequest) -> Result<VblankReply> {
    wait_vblank_via_fd(device.fd(), request)
}

#[cfg(not(target_os = "linux"))]
pub fn wait_vblank(_device: &DrmDevice, _request: &VblankRequest) -> Result<VblankReply> {
    Err(DrmError::NoDevice)
}

fn parse_drm_record(record_type: u32, record: &[u8], offset: usize) -> Result<DrmEvent> {
    match record_type {
        DRM_EVENT_FLIP_COMPLETE => Ok(DrmEvent::PageFlip(parse_page_flip_record(record, offset)?)),
        DRM_EVENT_VBLANK => Ok(DrmEvent::Vblank(parse_vblank_record(record, offset)?)),
        _ => Ok(DrmEvent::Unknown(UnknownDrmEvent {
            event_type: record_type,
            raw_record: record.to_vec(),
        })),
    }
}

fn parse_page_flip_record(record: &[u8], offset: usize) -> Result<PageFlipEvent> {
    let (sequence, timestamp_ns, crtc_id) = parse_sequence_record(record, offset)?;
    Ok(PageFlipEvent {
        sequence,
        timestamp_ns,
        crtc_id,
    })
}

fn parse_vblank_record(record: &[u8], offset: usize) -> Result<VblankEvent> {
    let (sequence, timestamp_ns, crtc_id) = parse_sequence_record(record, offset)?;
    Ok(VblankEvent {
        sequence,
        timestamp_ns,
        crtc_id,
    })
}

fn parse_sequence_record(record: &[u8], offset: usize) -> Result<(u32, u64, CrtcId)> {
    if record.len() < DRM_EVENT_VBLANK_LEN {
        return Err(DrmError::EventBufferMalformed {
            offset,
            reason: format!(
                "known event length {} shorter than {} bytes",
                record.len(),
                DRM_EVENT_VBLANK_LEN
            ),
        });
    }

    let seconds = read_u32_native(&record[16..20]);
    let microseconds = read_u32_native(&record[20..24]);
    let sequence = read_u32_native(&record[24..28]);
    let crtc_id = CrtcId(read_u32_native(&record[28..32]));
    let timestamp_ns = u64::from(seconds)
        .saturating_mul(1_000_000_000)
        .saturating_add(u64::from(microseconds).saturating_mul(1_000));

    Ok((sequence, timestamp_ns, crtc_id))
}

fn read_u32_native(bytes: &[u8]) -> u32 {
    let mut raw = [0u8; 4];
    raw.copy_from_slice(bytes);
    u32::from_ne_bytes(raw)
}

#[cfg(target_os = "linux")]
fn read_pending_event_bytes(fd: RawFd) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();

    loop {
        let mut pollfd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };

        // SAFETY: `pollfd` points to initialized storage and the kernel writes at most one entry.
        let poll_result = unsafe { libc::poll(&mut pollfd, 1, 0) };
        if poll_result < 0 {
            return Err(DrmError::Ioctl {
                name: "poll".to_string(),
                reason: std::io::Error::last_os_error().to_string(),
            });
        }
        if poll_result == 0 {
            break;
        }
        if (pollfd.revents & (libc::POLLHUP | libc::POLLNVAL)) != 0 {
            return Err(DrmError::DeviceLost);
        }
        if (pollfd.revents & libc::POLLIN) == 0 {
            break;
        }

        let mut chunk = [0u8; DRM_EVENT_READ_CHUNK_SIZE];
        // SAFETY: `chunk` is valid writable memory for the duration of the syscall.
        let read_result = unsafe { libc::read(fd, chunk.as_mut_ptr().cast(), chunk.len()) };
        if read_result < 0 {
            let error = std::io::Error::last_os_error();
            match error.raw_os_error() {
                Some(code) if code == libc::EINTR => continue,
                Some(code) if code == libc::EAGAIN || code == libc::EWOULDBLOCK => break,
                _ => {
                    return Err(DrmError::Ioctl {
                        name: "read".to_string(),
                        reason: error.to_string(),
                    });
                }
            }
        }
        if read_result == 0 {
            break;
        }

        bytes.extend_from_slice(&chunk[..read_result as usize]);
    }

    Ok(bytes)
}
