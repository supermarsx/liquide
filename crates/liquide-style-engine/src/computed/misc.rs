//! Remaining types.

use serde::{Deserialize, Serialize};

use liquide_compositor::pixel::Color;

use super::border::BorderLineStyle;

/// CSS contain property (bitflags style).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contain {
    pub size: bool,
    pub layout: bool,
    pub style: bool,
    pub paint: bool,
    pub inline_size: bool,
}

impl Default for Contain {
    fn default() -> Self {
        Self {
            size: false,
            layout: false,
            style: false,
            paint: false,
            inline_size: false,
        }
    }
}

impl Contain {
    pub fn none() -> Self {
        Self::default()
    }
    pub fn strict() -> Self {
        Self {
            size: true,
            layout: true,
            style: true,
            paint: true,
            inline_size: false,
        }
    }
    pub fn content() -> Self {
        Self {
            size: false,
            layout: true,
            style: true,
            paint: true,
            inline_size: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TouchAction {
    pub pan_x: bool,
    pub pan_y: bool,
    pub pinch_zoom: bool,
    pub manipulation: bool,
    pub none: bool,
}

impl Default for TouchAction {
    fn default() -> Self {
        Self {
            pan_x: true,
            pan_y: true,
            pinch_zoom: true,
            manipulation: false,
            none: false,
        }
    }
}

impl TouchAction {
    pub fn auto() -> Self {
        Self::default()
    }
    pub fn none_val() -> Self {
        Self {
            pan_x: false,
            pan_y: false,
            pinch_zoom: false,
            manipulation: false,
            none: true,
        }
    }
    pub fn manipulation_val() -> Self {
        Self {
            pan_x: true,
            pan_y: true,
            pinch_zoom: true,
            manipulation: true,
            none: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColumnRule {
    pub width: f32,
    pub style: BorderLineStyle,
    pub color: Color,
}

impl Default for ColumnRule {
    fn default() -> Self {
        Self {
            width: 0.0,
            style: BorderLineStyle::None,
            color: Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColumnFill {
    Balance,
    Auto,
}

impl Default for ColumnFill {
    fn default() -> Self {
        ColumnFill::Balance
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColumnSpan {
    None,
    All,
}

impl Default for ColumnSpan {
    fn default() -> Self {
        ColumnSpan::None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoxDecorationBreak {
    Slice,
    Clone,
}

impl Default for BoxDecorationBreak {
    fn default() -> Self {
        BoxDecorationBreak::Slice
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BreakValue {
    Auto,
    Avoid,
    AvoidPage,
    AvoidColumn,
    AvoidRegion,
    Page,
    Column,
    Region,
    Left,
    Right,
    Recto,
    Verso,
    Always,
}

impl Default for BreakValue {
    fn default() -> Self {
        BreakValue::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackgroundAttachment {
    Scroll,
    Fixed,
    Local,
}

impl Default for BackgroundAttachment {
    fn default() -> Self {
        BackgroundAttachment::Scroll
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackgroundClip {
    BorderBox,
    PaddingBox,
    ContentBox,
    Text,
}

impl Default for BackgroundClip {
    fn default() -> Self {
        BackgroundClip::BorderBox
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackgroundOrigin {
    BorderBox,
    PaddingBox,
    ContentBox,
}

impl Default for BackgroundOrigin {
    fn default() -> Self {
        BackgroundOrigin::PaddingBox
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListStyleType {
    None,
    Disc,
    Circle,
    Square,
    Decimal,
    DecimalLeadingZero,
    LowerRoman,
    UpperRoman,
    LowerAlpha,
    UpperAlpha,
    LowerLatin,
    UpperLatin,
}

impl Default for ListStyleType {
    fn default() -> Self {
        ListStyleType::Disc
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListStylePosition {
    Outside,
    Inside,
}

impl Default for ListStylePosition {
    fn default() -> Self {
        ListStylePosition::Outside
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RubyPosition {
    Over,
    Under,
    AlternateOver,
    AlternateUnder,
}
impl Default for RubyPosition {
    fn default() -> Self {
        RubyPosition::Over
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RubyAlign {
    SpaceAround,
    Center,
    Start,
    SpaceBetween,
}
impl Default for RubyAlign {
    fn default() -> Self {
        RubyAlign::SpaceAround
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShapeOutside {
    None,
    MarginBox,
    BorderBox,
    PaddingBox,
    ContentBox,
}
impl Default for ShapeOutside {
    fn default() -> Self {
        ShapeOutside::None
    }
}
