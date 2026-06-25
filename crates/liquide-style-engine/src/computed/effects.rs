//! Transform and effects enums.

use serde::{Deserialize, Serialize};

use crate::dimension::Dimension;

/// A transform-translate component: either an absolute length (px) or a
/// percentage that is resolved at apply time against the element's own box.
///
/// CSS: `translateX(%)` resolves against the element's border-box WIDTH and
/// `translateY(%)` against its HEIGHT. The percentage cannot be resolved at
/// compute time (the used box size isn't known until layout), so we carry it
/// here and resolve it in the painter (`painter/transforms.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum LengthPercent {
    /// Absolute length in CSS pixels.
    Px(f32),
    /// Percentage of the relevant box axis (0..100 == 0%..100%).
    Percent(f32),
}

impl LengthPercent {
    /// Zero-length convenience.
    pub const ZERO: LengthPercent = LengthPercent::Px(0.0);

    /// Resolve to pixels against the given axis length (element box size on the
    /// relevant axis). For `Px`, the axis length is ignored.
    pub fn resolve(self, axis_len: f32) -> f32 {
        match self {
            LengthPercent::Px(v) => v,
            LengthPercent::Percent(p) => axis_len * p / 100.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Transform {
    /// 2D translate. Each axis is a length-or-percent; X percentages resolve
    /// against the element width, Y against the element height.
    Translate(LengthPercent, LengthPercent),
    Scale(f32, f32),
    Rotate(f32),
    Skew(f32, f32),
    Matrix(f32, f32, f32, f32, f32, f32),
    // 3D transform functions
    /// 3D translate. X/Y are length-or-percent (resolved against element
    /// width/height); Z is always an absolute length per CSS (`translateZ(%)`
    /// is invalid).
    Translate3d(LengthPercent, LengthPercent, f32),
    Rotate3d(f32, f32, f32, f32),
    Scale3d(f32, f32, f32),
    Matrix3d([f32; 16]),
    PerspectiveFn(f32),
}

impl Transform {
    /// Returns `true` if this transform function requires 3D composition.
    pub fn is_3d(&self) -> bool {
        matches!(
            self,
            Transform::Translate3d(..)
                | Transform::Rotate3d(..)
                | Transform::Scale3d(..)
                | Transform::Matrix3d(..)
                | Transform::PerspectiveFn(..)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransformOrigin {
    pub x: Dimension,
    pub y: Dimension,
}

impl Default for TransformOrigin {
    fn default() -> Self {
        Self {
            x: Dimension::Percent(50.0),
            y: Dimension::Percent(50.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransformStyle {
    Flat,
    Preserve3d,
}

impl Default for TransformStyle {
    fn default() -> Self {
        TransformStyle::Flat
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransformBox {
    ContentBox,
    BorderBox,
    FillBox,
    StrokeBox,
    ViewBox,
}

impl Default for TransformBox {
    fn default() -> Self {
        TransformBox::ViewBox
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Perspective {
    None,
    Length(f32),
}

impl Default for Perspective {
    fn default() -> Self {
        Perspective::None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackfaceVisibility {
    Visible,
    Hidden,
}

impl Default for BackfaceVisibility {
    fn default() -> Self {
        BackfaceVisibility::Visible
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Isolation {
    Auto,
    Isolate,
}

impl Default for Isolation {
    fn default() -> Self {
        Isolation::Auto
    }
}
