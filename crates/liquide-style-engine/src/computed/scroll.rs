//! Scroll and containment enums.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScrollSnapType {
    None,
    X(ScrollSnapStrictness),
    Y(ScrollSnapStrictness),
    Block(ScrollSnapStrictness),
    Inline(ScrollSnapStrictness),
    Both(ScrollSnapStrictness),
}

impl Default for ScrollSnapType {
    fn default() -> Self {
        ScrollSnapType::None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScrollSnapStrictness {
    Mandatory,
    Proximity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScrollSnapAlign {
    None,
    Start,
    End,
    Center,
}

impl Default for ScrollSnapAlign {
    fn default() -> Self {
        ScrollSnapAlign::None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScrollSnapStop {
    Normal,
    Always,
}

impl Default for ScrollSnapStop {
    fn default() -> Self {
        ScrollSnapStop::Normal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OverflowAnchor {
    Auto,
    None,
}
impl Default for OverflowAnchor {
    fn default() -> Self {
        OverflowAnchor::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScrollbarWidth {
    Auto,
    Thin,
    None,
}
impl Default for ScrollbarWidth {
    fn default() -> Self {
        ScrollbarWidth::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScrollbarGutter {
    Auto,
    Stable,
    StableBothEdges,
}
impl Default for ScrollbarGutter {
    fn default() -> Self {
        ScrollbarGutter::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContainerType {
    Normal,
    InlineSize,
    Size,
}
impl Default for ContainerType {
    fn default() -> Self {
        ContainerType::Normal
    }
}
