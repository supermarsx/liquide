//! SVG presentation enums.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FillRule {
    NonZero,
    EvenOdd,
}
impl Default for FillRule {
    fn default() -> Self {
        FillRule::NonZero
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StrokeLinecap {
    Butt,
    Round,
    Square,
}
impl Default for StrokeLinecap {
    fn default() -> Self {
        StrokeLinecap::Butt
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StrokeLinejoin {
    Miter,
    Round,
    Bevel,
}
impl Default for StrokeLinejoin {
    fn default() -> Self {
        StrokeLinejoin::Miter
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorInterpolation {
    Auto,
    SRGB,
    LinearRGB,
}
impl Default for ColorInterpolation {
    fn default() -> Self {
        ColorInterpolation::SRGB
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DominantBaseline {
    Auto,
    TextBottom,
    Alphabetic,
    Ideographic,
    Middle,
    Central,
    Mathematical,
    Hanging,
    TextTop,
}
impl Default for DominantBaseline {
    fn default() -> Self {
        DominantBaseline::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlignmentBaseline {
    Auto,
    Baseline,
    TextBottom,
    Alphabetic,
    Ideographic,
    Middle,
    Central,
    Mathematical,
    TextTop,
}
impl Default for AlignmentBaseline {
    fn default() -> Self {
        AlignmentBaseline::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClipRule {
    NonZero,
    EvenOdd,
}
impl Default for ClipRule {
    fn default() -> Self {
        ClipRule::NonZero
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShapeRendering {
    Auto,
    OptimizeSpeed,
    CrispEdges,
    GeometricPrecision,
}
impl Default for ShapeRendering {
    fn default() -> Self {
        ShapeRendering::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextAnchor {
    Start,
    Middle,
    End,
}
impl Default for TextAnchor {
    fn default() -> Self {
        TextAnchor::Start
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VectorEffect {
    None,
    NonScalingStroke,
}
impl Default for VectorEffect {
    fn default() -> Self {
        VectorEffect::None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaintOrder {
    Normal,
    Fill,
    Stroke,
    Markers,
}

impl Default for PaintOrder {
    fn default() -> Self {
        PaintOrder::Normal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaskType {
    Luminance,
    Alpha,
}
impl Default for MaskType {
    fn default() -> Self {
        MaskType::Luminance
    }
}
