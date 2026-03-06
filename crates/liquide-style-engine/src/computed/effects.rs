//! Transform and effects enums.

use serde::{Deserialize, Serialize};

use crate::dimension::Dimension;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Transform {
    Translate(f32, f32),
    Scale(f32, f32),
    Rotate(f32),
    Skew(f32, f32),
    Matrix(f32, f32, f32, f32, f32, f32),
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
