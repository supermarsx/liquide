use crate::crtc::CrtcId;
use crate::device::DrmDevice;
use crate::error::{DrmError, Result};
#[cfg(target_os = "linux")]
use std::os::fd::RawFd;

const DRM_EVENT_VBLANK: u32 = 0x01;
const DRM_EVENT_FLIP_COMPLETE: u32 = 0x02;
const DRM_EVENT_HEADER_LEN: usize = 8;
const DRM_EVENT_VBLANK_LEN: usize = 32;
#[cfg(target_os = "linux")]
const DRM_EVENT_READ_CHUNK_SIZE: usize = 4096;

/// Bitflags for page flip requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageFlipFlags(u32);

impl PageFlipFlags {
    pub const EVENT: Self = Self(1 << 0);
    pub const ASYNC: Self = Self(1 << 1);

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

/// Requests a page flip on a CRTC to the given framebuffer.
#[cfg(target_os = "linux")]
pub fn request_page_flip(
    _device: &DrmDevice,
    _crtc: CrtcId,
    _fb_id: u32,
    _flags: PageFlipFlags,
) -> Result<()> {
    // TODO: implement via DRM_IOCTL_MODE_PAGE_FLIP
    Err(DrmError::PageFlip("not yet implemented".to_string()))
}

#[cfg(not(target_os = "linux"))]
pub fn request_page_flip(
    _device: &DrmDevice,
    _crtc: CrtcId,
    _fb_id: u32,
    _flags: PageFlipFlags,
) -> Result<()> {
    Err(DrmError::NoDevice)
}

/// Waits for the next vblank on the given CRTC.
#[cfg(target_os = "linux")]
pub fn wait_vblank(_device: &DrmDevice, _crtc: CrtcId) -> Result<VblankEvent> {
    // TODO: implement via DRM_IOCTL_WAIT_VBLANK
    Err(DrmError::VblankWait("not yet implemented".to_string()))
}

#[cfg(not(target_os = "linux"))]
pub fn wait_vblank(_device: &DrmDevice, _crtc: CrtcId) -> Result<VblankEvent> {
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
