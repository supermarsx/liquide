/// DRM fourcc pixel format code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrmFourcc(pub u32);

impl DrmFourcc {
    pub const XRGB8888: Self = Self(0x34325258);
    pub const ARGB8888: Self = Self(0x34325241);
    pub const XBGR8888: Self = Self(0x34324258);
    pub const ABGR8888: Self = Self(0x34324241);
    pub const RGB888: Self = Self(0x34324752);

    pub fn new(value: u32) -> Self {
        Self(value)
    }

    pub fn name(&self) -> &str {
        match self.0 {
            0x34325258 => "XR24",
            0x34325241 => "AR24",
            0x34324258 => "XB24",
            0x34324241 => "AB24",
            0x34324752 => "RG24",
            _ => "????",
        }
    }
}

/// DRM format modifier (tiling layout, compression, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrmModifier(pub u64);

impl DrmModifier {
    pub const LINEAR: Self = Self(0);
}
