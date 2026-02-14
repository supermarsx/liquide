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

/// Porter-Duff compositing and CSS `mix-blend-mode` values.
///
/// The first five are Porter-Duff operators used by the compositor itself.
/// The remaining modes correspond to the CSS Compositing and Blending Level 1
/// specification and are needed for full CSS3 parity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum BlendMode {
    // -- Porter-Duff operators (compositor) --
    /// Standard alpha compositing (default everywhere).
    #[default]
    SrcOver,
    /// Replace destination (used for opaque surface blit).
    Src,
    /// Source-atop (used for clip-to-shape effects).
    SrcAtop,

    // -- CSS separable blend modes --
    /// `multiply` — `out = src * dst` per channel.
    Multiply,
    /// `screen` — `out = src + dst - src * dst`.
    Screen,
    /// `overlay` — Multiply or Screen depending on dst luminance.
    Overlay,
    /// `darken` — `out = min(src, dst)` per channel.
    Darken,
    /// `lighten` — `out = max(src, dst)` per channel.
    Lighten,
    /// `color-dodge` — brightens dst to reflect src.
    ColorDodge,
    /// `color-burn` — darkens dst to reflect src.
    ColorBurn,
    /// `hard-light` — Multiply or Screen depending on src luminance.
    HardLight,
    /// `soft-light` — softer version of hard-light.
    SoftLight,
    /// `difference` — `out = |src - dst|`.
    Difference,
    /// `exclusion` — lower contrast difference.
    Exclusion,

    // -- CSS non-separable blend modes --
    /// `hue` — hue from src, saturation+luminosity from dst.
    Hue,
    /// `saturation` — saturation from src, hue+luminosity from dst.
    Saturation,
    /// `color` — hue+saturation from src, luminosity from dst.
    ColorBlend,
    /// `luminosity` — luminosity from src, hue+saturation from dst.
    Luminosity,
}

impl BlendMode {
    /// CSS keyword name for this blend mode.
    #[must_use]
    pub fn css_name(&self) -> &'static str {
        match self {
            Self::SrcOver => "normal",
            Self::Src => "normal",
            Self::SrcAtop => "normal",
            Self::Multiply => "multiply",
            Self::Screen => "screen",
            Self::Overlay => "overlay",
            Self::Darken => "darken",
            Self::Lighten => "lighten",
            Self::ColorDodge => "color-dodge",
            Self::ColorBurn => "color-burn",
            Self::HardLight => "hard-light",
            Self::SoftLight => "soft-light",
            Self::Difference => "difference",
            Self::Exclusion => "exclusion",
            Self::Hue => "hue",
            Self::Saturation => "saturation",
            Self::ColorBlend => "color",
            Self::Luminosity => "luminosity",
        }
    }

    /// Parse from CSS keyword.
    pub fn from_css_name(name: &str) -> Option<Self> {
        match name.trim().to_lowercase().as_str() {
            "normal" => Some(Self::SrcOver),
            "multiply" => Some(Self::Multiply),
            "screen" => Some(Self::Screen),
            "overlay" => Some(Self::Overlay),
            "darken" => Some(Self::Darken),
            "lighten" => Some(Self::Lighten),
            "color-dodge" => Some(Self::ColorDodge),
            "color-burn" => Some(Self::ColorBurn),
            "hard-light" => Some(Self::HardLight),
            "soft-light" => Some(Self::SoftLight),
            "difference" => Some(Self::Difference),
            "exclusion" => Some(Self::Exclusion),
            "hue" => Some(Self::Hue),
            "saturation" => Some(Self::Saturation),
            "color" => Some(Self::ColorBlend),
            "luminosity" => Some(Self::Luminosity),
            _ => None,
        }
    }

    /// Whether this is a separable blend mode (can be computed per-channel).
    #[must_use]
    pub fn is_separable(&self) -> bool {
        !matches!(
            self,
            Self::Hue | Self::Saturation | Self::ColorBlend | Self::Luminosity
        )
    }
}
