/// Bitflags describing display mode properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModeFlags(u32);

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

/// A display mode (resolution + refresh rate + timing info).
#[derive(Debug, Clone)]
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
}

/// Returns the first preferred mode, or `None` if no mode is preferred.
pub fn preferred_mode(modes: &[DrmMode]) -> Option<&DrmMode> {
    modes.iter().find(|m| m.is_preferred())
}
