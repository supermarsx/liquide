//! Pixel format definitions, color representation, and blend modes.

use serde::{Deserialize, Serialize};

/// Pixel format for frame buffers and tiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum PixelFormat {
    /// 32-bit BGRA, 8 bits per channel (default for SDR compositing).
    #[default]
    Bgra8,
    /// 32-bit RGBA, 8 bits per channel.
    Rgba8,
    /// 24-bit RGB, 8 bits per channel (no alpha).
    Rgb8,
    /// 16-bit RGB565 (low bandwidth).
    Rgb565,
    /// 32-bit, 10 bits per RGB + 2 pad (WCG/HDR).
    Rgb101010,
    /// 32-bit, 10 bits per RGB + 2-bit alpha (WCG/HDR).
    Rgba1010102,
}

impl PixelFormat {
    /// Bytes per pixel for this format.
    #[must_use]
    pub fn bytes_per_pixel(&self) -> u32 {
        match self {
            Self::Bgra8 | Self::Rgba8 => 4,
            Self::Rgb8 => 3,
            Self::Rgb565 => 2,
            Self::Rgb101010 | Self::Rgba1010102 => 4,
        }
    }

    /// Whether this format has an alpha channel.
    #[must_use]
    pub fn has_alpha(&self) -> bool {
        matches!(self, Self::Bgra8 | Self::Rgba8 | Self::Rgba1010102)
    }

    /// Return the wire name used in the protocol.
    #[must_use]
    pub fn wire_name(&self) -> &'static str {
        match self {
            Self::Bgra8 => "bgra8888",
            Self::Rgba8 => "rgba8888",
            Self::Rgb8 => "rgb888",
            Self::Rgb565 => "rgb565",
            Self::Rgb101010 => "rgb101010",
            Self::Rgba1010102 => "rgba1010102",
        }
    }

    /// Parse from protocol wire name.
    pub fn from_wire_name(name: &str) -> Option<Self> {
        match name {
            "bgra8888" => Some(Self::Bgra8),
            "rgba8888" => Some(Self::Rgba8),
            "rgb888" => Some(Self::Rgb8),
            "rgb565" => Some(Self::Rgb565),
            "rgb101010" => Some(Self::Rgb101010),
            "rgba1010102" => Some(Self::Rgba1010102),
            _ => None,
        }
    }
}

/// An RGBA color with 8 bits per channel.
///
/// Colors are stored in straight (non-premultiplied) form. Use
/// [`Color::premultiply`] to convert for compositing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const WHITE: Self = Self {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };
    pub const BLACK: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };
    pub const TRANSPARENT: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };

    /// Create a new color.
    #[must_use]
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Create from a packed RGBA u32 (R in highest byte).
    #[must_use]
    pub fn from_rgba_u32(rgba: u32) -> Self {
        Self {
            r: (rgba >> 24) as u8,
            g: (rgba >> 16) as u8,
            b: (rgba >> 8) as u8,
            a: rgba as u8,
        }
    }

    /// Pack into a RGBA u32 (R in highest byte).
    #[must_use]
    pub fn to_rgba_u32(&self) -> u32 {
        (self.r as u32) << 24 | (self.g as u32) << 16 | (self.b as u32) << 8 | self.a as u32
    }

    /// Convert to BGRA byte order (for `PixelFormat::Bgra8` frame buffers).
    #[must_use]
    pub fn to_bgra_bytes(&self) -> [u8; 4] {
        [self.b, self.g, self.r, self.a]
    }

    /// Create from BGRA bytes.
    #[must_use]
    pub fn from_bgra_bytes(bytes: [u8; 4]) -> Self {
        Self {
            r: bytes[2],
            g: bytes[1],
            b: bytes[0],
            a: bytes[3],
        }
    }

    /// Premultiply alpha: `channel = channel * alpha / 255`.
    #[must_use]
    pub fn premultiply(&self) -> Self {
        if self.a == 255 {
            return *self;
        }
        if self.a == 0 {
            return Self::TRANSPARENT;
        }
        let a = self.a as u16;
        Self {
            r: ((self.r as u16 * a + 127) / 255) as u8,
            g: ((self.g as u16 * a + 127) / 255) as u8,
            b: ((self.b as u16 * a + 127) / 255) as u8,
            a: self.a,
        }
    }

    /// Check if fully opaque.
    #[must_use]
    pub fn is_opaque(&self) -> bool {
        self.a == 255
    }

    /// Check if fully transparent.
    #[must_use]
    pub fn is_transparent(&self) -> bool {
        self.a == 0
    }
}

/// Porter-Duff compositing blend modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum BlendMode {
    /// Standard alpha compositing (default everywhere).
    #[default]
    SrcOver,
    /// Replace destination (used for opaque surface blit).
    Src,
    /// Multiply blend (used for color tint on glass surfaces).
    Multiply,
    /// Screen blend (used for specular highlights / inner glow).
    Screen,
    /// Source-atop (used for clip-to-shape effects).
    SrcAtop,
}
