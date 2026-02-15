//! Colour mode negotiation, tone-mapping, and gamut management.

use std::fmt;

/// Active colour mode of the pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    SdrSrgb,
    WcgSdr,
    Hdr,
}

impl fmt::Display for ColorMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::SdrSrgb => "SDR/sRGB",
            Self::WcgSdr => "WCG-SDR",
            Self::Hdr => "HDR",
        };
        f.write_str(label)
    }
}

/// Tone-mapping operator selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToneMapper {
    Reinhard,
    Bt2390,
    Hable,
    Aces,
}

impl fmt::Display for ToneMapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Reinhard => "Reinhard",
            Self::Bt2390 => "BT.2390",
            Self::Hable => "Hable",
            Self::Aces => "ACES",
        };
        f.write_str(label)
    }
}

/// Display gamut capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayGamut {
    Srgb,
    P3,
    Bt2020,
}

/// Describes the colour capabilities of the local display for negotiation
/// with the server.
#[derive(Debug, Clone)]
pub struct ColorNegotiation {
    pub supported_modes: Vec<ColorMode>,
    pub display_gamut: DisplayGamut,
    pub hdr_support: bool,
    pub max_luminance_nits: u32,
    pub preferred_bit_depth: u8,
}

impl ColorNegotiation {
    /// Create a new negotiation descriptor with reasonable defaults.
    #[must_use]
    pub fn new() -> Self {
        Self {
            supported_modes: vec![ColorMode::SdrSrgb],
            display_gamut: DisplayGamut::Srgb,
            hdr_support: false,
            max_luminance_nits: 400,
            preferred_bit_depth: 8,
        }
    }

    /// Whether this display advertises HDR support.
    #[must_use]
    pub fn supports_hdr(&self) -> bool {
        self.hdr_support && self.supported_modes.contains(&ColorMode::Hdr)
    }

    /// Select the best colour mode from the set of supported modes.
    #[must_use]
    pub fn best_mode(&self) -> ColorMode {
        if self.supports_hdr() {
            return ColorMode::Hdr;
        }
        if self.supported_modes.contains(&ColorMode::WcgSdr) {
            return ColorMode::WcgSdr;
        }
        ColorMode::SdrSrgb
    }
}

impl Default for ColorNegotiation {
    fn default() -> Self {
        Self::new()
    }
}

/// Manages the active colour pipeline for the client session.
pub struct ColorPipeline {
    active_mode: ColorMode,
    #[allow(dead_code)]
    tone_mapper: ToneMapper,
    gamut: DisplayGamut,
}

impl ColorPipeline {
    /// Create a new pipeline with SDR/sRGB defaults.
    #[must_use]
    pub fn new() -> Self {
        Self {
            active_mode: ColorMode::SdrSrgb,
            tone_mapper: ToneMapper::Reinhard,
            gamut: DisplayGamut::Srgb,
        }
    }

    /// Active colour mode.
    #[must_use]
    pub fn active_mode(&self) -> ColorMode {
        self.active_mode
    }

    /// Switch to a different colour mode.
    pub fn set_mode(&mut self, mode: ColorMode) {
        self.active_mode = mode;
    }

    /// Whether tone mapping is required (HDR content on non-HDR display).
    #[must_use]
    pub fn needs_tone_mapping(&self) -> bool {
        self.active_mode == ColorMode::Hdr
    }

    /// Whether gamut mapping is required (wider gamut than sRGB).
    #[must_use]
    pub fn needs_gamut_mapping(&self) -> bool {
        self.gamut != DisplayGamut::Srgb
            || self.active_mode == ColorMode::WcgSdr
            || self.active_mode == ColorMode::Hdr
    }
}

impl Default for ColorPipeline {
    fn default() -> Self {
        Self::new()
    }
}
