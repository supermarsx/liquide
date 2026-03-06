//! Visual and interaction enums.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Cursor {
    Auto,
    Default,
    Pointer,
    Text,
    Move,
    Crosshair,
    Wait,
    Help,
    NotAllowed,
    Grab,
    Grabbing,
    ColResize,
    RowResize,
    EResize,
    WResize,
    NResize,
    SResize,
    NeResize,
    NwResize,
    SeResize,
    SwResize,
}

impl Default for Cursor {
    fn default() -> Self {
        Cursor::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PointerEvents {
    Auto,
    None,
}

impl Default for PointerEvents {
    fn default() -> Self {
        PointerEvents::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectFit {
    Fill,
    Contain,
    Cover,
    None,
    ScaleDown,
}

impl Default for ObjectFit {
    fn default() -> Self {
        ObjectFit::Fill
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContentVisibility {
    Visible,
    Auto,
    Hidden,
}

impl Default for ContentVisibility {
    fn default() -> Self {
        ContentVisibility::Visible
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AspectRatio {
    Auto,
    Ratio(f32, f32),
}

impl Default for AspectRatio {
    fn default() -> Self {
        AspectRatio::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Resize {
    None,
    Both,
    Horizontal,
    Vertical,
    Block,
    Inline,
}

impl Default for Resize {
    fn default() -> Self {
        Resize::None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserSelect {
    Auto,
    None,
    Text,
    All,
    Contain,
}

impl Default for UserSelect {
    fn default() -> Self {
        UserSelect::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Appearance {
    None,
    Auto,
}

impl Default for Appearance {
    fn default() -> Self {
        Appearance::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScrollBehavior {
    Auto,
    Smooth,
}

impl Default for ScrollBehavior {
    fn default() -> Self {
        ScrollBehavior::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OverscrollBehavior {
    Auto,
    Contain,
    None,
}

impl Default for OverscrollBehavior {
    fn default() -> Self {
        OverscrollBehavior::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorScheme {
    Normal,
    Light,
    Dark,
    LightDark,
}

impl Default for ColorScheme {
    fn default() -> Self {
        ColorScheme::Normal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ForcedColorAdjust {
    Auto,
    None,
}

impl Default for ForcedColorAdjust {
    fn default() -> Self {
        ForcedColorAdjust::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrintColorAdjust {
    Economy,
    Exact,
}

impl Default for PrintColorAdjust {
    fn default() -> Self {
        PrintColorAdjust::Economy
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageOrientation {
    FromImage,
    None,
}
impl Default for ImageOrientation {
    fn default() -> Self {
        ImageOrientation::FromImage
    }
}
