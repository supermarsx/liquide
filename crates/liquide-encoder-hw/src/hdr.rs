//! HDR metadata types and SEI/OBU packing stubs.

use std::fmt;

use serde::{Deserialize, Serialize};

/// HDR format identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HdrFormat {
    /// Static HDR10 (SMPTE ST 2084 PQ + ST 2086 mastering display).
    Hdr10,
    /// Dynamic HDR10+ (Samsung, per-frame metadata).
    Hdr10Plus,
    /// Hybrid Log-Gamma (BBC/NHK).
    Hlg,
}

impl fmt::Display for HdrFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hdr10 => write!(f, "HDR10"),
            Self::Hdr10Plus => write!(f, "HDR10+"),
            Self::Hlg => write!(f, "HLG"),
        }
    }
}

/// Colour primaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorPrimaries {
    /// ITU-R BT.709 (sRGB).
    Bt709,
    /// ITU-R BT.2020 (wide gamut).
    Bt2020,
    /// Display P3.
    DisplayP3,
}

impl fmt::Display for ColorPrimaries {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bt709 => write!(f, "BT.709"),
            Self::Bt2020 => write!(f, "BT.2020"),
            Self::DisplayP3 => write!(f, "Display P3"),
        }
    }
}

/// Electro-optical transfer function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferFunction {
    /// sRGB gamma (~2.2).
    Srgb,
    /// Perceptual Quantiser (SMPTE ST 2084).
    Pq,
    /// Hybrid Log-Gamma.
    Hlg,
}

impl fmt::Display for TransferFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Srgb => write!(f, "sRGB"),
            Self::Pq => write!(f, "PQ"),
            Self::Hlg => write!(f, "HLG"),
        }
    }
}

/// SMPTE ST 2086 mastering display colour volume.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasteringDisplay {
    /// Red primary (CIE x, y).
    pub red: (f32, f32),
    /// Green primary.
    pub green: (f32, f32),
    /// Blue primary.
    pub blue: (f32, f32),
    /// White point.
    pub white_point: (f32, f32),
    /// Maximum luminance (cd/m^2).
    pub max_luminance: f32,
    /// Minimum luminance (cd/m^2).
    pub min_luminance: f32,
}

/// HDR metadata carried alongside encoded frames.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HdrMetadata {
    /// HDR standard.
    pub format: HdrFormat,
    /// Colour primaries.
    pub primaries: ColorPrimaries,
    /// Transfer function.
    pub transfer: TransferFunction,
    /// Maximum content light level (cd/m^2).
    pub max_luminance: f32,
    /// Minimum content light level (cd/m^2).
    pub min_luminance: f32,
    /// Maximum Content Light Level.
    pub max_cll: u16,
    /// Maximum Frame-Average Light Level.
    pub max_fall: u16,
    /// Optional SMPTE ST 2086 mastering display metadata.
    pub mastering_display: Option<MasteringDisplay>,
    /// Optional per-frame dynamic metadata (HDR10+).
    pub dynamic_metadata: Option<Vec<u8>>,
}

impl HdrMetadata {
    /// Pack this metadata as an H.264/H.265 SEI NALU (stub).
    #[must_use]
    pub fn pack_sei_nalu(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(32);
        // SEI prefix (user data unregistered)
        buf.extend_from_slice(&[0x00, 0x00, 0x00, 0x01, 0x06]);
        buf.push(self.format as u8);
        buf.push(self.primaries as u8);
        buf.push(self.transfer as u8);
        buf.extend_from_slice(&self.max_cll.to_le_bytes());
        buf.extend_from_slice(&self.max_fall.to_le_bytes());
        buf
    }

    /// Pack this metadata as an AV1 OBU metadata block (stub).
    #[must_use]
    pub fn pack_obu_metadata(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(32);
        // OBU metadata header
        buf.extend_from_slice(&[0x2A, 0x01]);
        buf.push(self.format as u8);
        buf.push(self.primaries as u8);
        buf.push(self.transfer as u8);
        buf.extend_from_slice(&self.max_cll.to_le_bytes());
        buf.extend_from_slice(&self.max_fall.to_le_bytes());
        buf
    }
}

/// Tone-mapping operators for HDR-to-SDR conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToneMapOperator {
    /// Reinhard global operator.
    Reinhard,
    /// ITU-R BT.2390 reference OOTF.
    Bt2390,
    /// Hable (Uncharted 2) filmic curve.
    Hable,
    /// ACES filmic tone mapping.
    Aces,
}

impl fmt::Display for ToneMapOperator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reinhard => write!(f, "Reinhard"),
            Self::Bt2390 => write!(f, "BT.2390"),
            Self::Hable => write!(f, "Hable"),
            Self::Aces => write!(f, "ACES"),
        }
    }
}
