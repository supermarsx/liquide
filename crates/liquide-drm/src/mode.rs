/// Bitflags describing display mode properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModeFlags(u32);

#[cfg(any(test, target_os = "linux"))]
const DRM_DISPLAY_MODE_NAME_LEN: usize = 32;
#[cfg(any(test, target_os = "linux"))]
const DRM_MODE_FLAG_INTERLACE: u32 = 1 << 4;
#[cfg(any(test, target_os = "linux"))]
const DRM_MODE_FLAG_DBLSCAN: u32 = 1 << 5;
#[cfg(any(test, target_os = "linux"))]
const DRM_MODE_TYPE_CLOCK_C: u32 = 1 << 1;
#[cfg(any(test, target_os = "linux"))]
const DRM_MODE_TYPE_CRTC_C: u32 = 1 << 2;
#[cfg(any(test, target_os = "linux"))]
const DRM_MODE_TYPE_PREFERRED: u32 = 1 << 3;

impl ModeFlags {
    pub const PREFERRED: Self = Self(1 << 0);
    pub const CURRENT: Self = Self(1 << 1);
    pub const INTERLACE: Self = Self(1 << 2);

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

impl std::ops::BitOr for ModeFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitAnd for ModeFlags {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

/// Internal mirror of `drm_mode_modeinfo` used for safe synthetic tests and
/// Linux connector enumeration.
#[cfg(any(test, target_os = "linux"))]
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct RawDrmModeInfo {
    pub clock: u32,
    pub hdisplay: u16,
    pub hsync_start: u16,
    pub hsync_end: u16,
    pub htotal: u16,
    pub hskew: u16,
    pub vdisplay: u16,
    pub vsync_start: u16,
    pub vsync_end: u16,
    pub vtotal: u16,
    pub vscan: u16,
    pub vrefresh: u32,
    pub flags: u32,
    pub mode_type: u32,
    pub name: [u8; DRM_DISPLAY_MODE_NAME_LEN],
}

/// A display mode (resolution + refresh rate + timing info).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrmMode {
    pub width: u32,
    pub height: u32,
    pub refresh_hz: u32,
    pub clock_khz: u32,
    pub flags: ModeFlags,
    pub name: String,
}

impl DrmMode {
    /// Returns `true` if this mode is marked as preferred by the display.
    pub fn is_preferred(&self) -> bool {
        self.flags.contains(ModeFlags::PREFERRED)
    }

    /// Returns `true` if this mode describes the connector's current timing.
    pub fn is_current(&self) -> bool {
        self.flags.contains(ModeFlags::CURRENT)
    }

    /// Returns `true` if this mode carries enough geometry for launch planning.
    pub fn is_usable(&self) -> bool {
        self.width > 0 && self.height > 0
    }
}

/// Returns the first preferred mode, or `None` if no mode is preferred.
pub fn preferred_mode(modes: &[DrmMode]) -> Option<&DrmMode> {
    modes.iter().find(|m| m.is_preferred())
}

/// Returns the first current mode, or `None` if no mode is marked current.
pub fn current_mode(modes: &[DrmMode]) -> Option<&DrmMode> {
    modes.iter().find(|m| m.is_current())
}

/// Returns the best usable mode for standalone launch planning.
pub fn launchable_mode(modes: &[DrmMode]) -> Option<&DrmMode> {
    current_mode(modes)
        .filter(|mode| mode.is_usable())
        .or_else(|| preferred_mode(modes).filter(|mode| mode.is_usable()))
        .or_else(|| modes.iter().find(|mode| mode.is_usable()))
}

/// Returns the first usable mode whose `width × height` exactly matches the
/// requested dimensions. Returns `None` if no usable mode matches.
pub fn match_mode_by_dimensions(modes: &[DrmMode], width: u32, height: u32) -> Option<&DrmMode> {
    modes
        .iter()
        .find(|m| m.is_usable() && m.width == width && m.height == height)
}

/// Returns the usable mode with the highest pixel area (`width * height`),
/// breaking ties by higher `refresh_hz`. Returns `None` if no mode is usable.
pub fn highest_resolution_mode(modes: &[DrmMode]) -> Option<&DrmMode> {
    modes
        .iter()
        .filter(|m| m.is_usable())
        .max_by(|a, b| {
            let area_a = (a.width as u64) * (a.height as u64);
            let area_b = (b.width as u64) * (b.height as u64);
            area_a
                .cmp(&area_b)
                .then_with(|| a.refresh_hz.cmp(&b.refresh_hz))
        })
}

/// Returns the usable mode whose `refresh_hz` is closest to `target_hz`.
/// Ties are broken by higher refresh. Returns `None` if no mode is usable.
pub fn closest_refresh_mode(modes: &[DrmMode], target_hz: u32) -> Option<&DrmMode> {
    modes
        .iter()
        .filter(|m| m.is_usable())
        .min_by_key(|m| {
            let delta = (m.refresh_hz as i64 - target_hz as i64).abs();
            // Tie-break by negative refresh so that on equal delta the higher
            // refresh wins (smaller min_by_key key when negated).
            (delta, -(m.refresh_hz as i64))
        })
}

#[cfg(any(test, target_os = "linux"))]
pub(crate) fn from_raw_mode_info(raw: &RawDrmModeInfo) -> Option<DrmMode> {
    let width = u32::from(raw.hdisplay);
    let height = u32::from(raw.vdisplay);
    if width == 0 || height == 0 {
        return None;
    }

    let refresh_hz = raw_refresh_hz(raw);
    let name = raw_mode_name(raw, width, height, refresh_hz);

    Some(DrmMode {
        width,
        height,
        refresh_hz,
        clock_khz: raw.clock,
        flags: raw_mode_flags(raw),
        name,
    })
}

#[cfg(any(test, target_os = "linux"))]
fn raw_mode_flags(raw: &RawDrmModeInfo) -> ModeFlags {
    let mut flags = ModeFlags::empty();
    if (raw.mode_type & DRM_MODE_TYPE_PREFERRED) != 0 {
        flags = flags | ModeFlags::PREFERRED;
    }
    if (raw.mode_type & (DRM_MODE_TYPE_CLOCK_C | DRM_MODE_TYPE_CRTC_C)) != 0 {
        flags = flags | ModeFlags::CURRENT;
    }
    if (raw.flags & DRM_MODE_FLAG_INTERLACE) != 0 {
        flags = flags | ModeFlags::INTERLACE;
    }
    flags
}

#[cfg(any(test, target_os = "linux"))]
fn raw_refresh_hz(raw: &RawDrmModeInfo) -> u32 {
    if raw.vrefresh > 0 {
        return raw.vrefresh;
    }

    let clock_hz = u64::from(raw.clock).saturating_mul(1_000);
    let htotal = u64::from(raw.htotal);
    let vtotal = u64::from(raw.vtotal);
    if clock_hz == 0 || htotal == 0 || vtotal == 0 {
        return 0;
    }

    let mut numerator = clock_hz;
    let mut denominator = htotal.saturating_mul(vtotal);
    if (raw.flags & DRM_MODE_FLAG_INTERLACE) != 0 {
        numerator = numerator.saturating_mul(2);
    }
    if (raw.flags & DRM_MODE_FLAG_DBLSCAN) != 0 {
        denominator = denominator.saturating_mul(2);
    }
    if raw.vscan > 1 {
        denominator = denominator.saturating_mul(u64::from(raw.vscan));
    }
    if denominator == 0 {
        return 0;
    }

    let rounded = numerator
        .saturating_add(denominator / 2)
        .saturating_div(denominator);
    rounded.min(u64::from(u32::MAX)) as u32
}

#[cfg(any(test, target_os = "linux"))]
fn raw_mode_name(raw: &RawDrmModeInfo, width: u32, height: u32, refresh_hz: u32) -> String {
    let end = raw
        .name
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(raw.name.len());
    if end > 0 {
        let bytes = &raw.name[..end];
        let value = String::from_utf8_lossy(bytes).trim().to_string();
        if !value.is_empty() {
            return value;
        }
    }

    format!("{width}x{height}@{refresh_hz}")
}
