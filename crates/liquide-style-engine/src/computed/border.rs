//! Border enums.

use serde::{Deserialize, Serialize};

use liquide_compositor::pixel::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BorderLineStyle {
    None,
    Solid,
    Dashed,
    Dotted,
    Double,
    Groove,
    Ridge,
    Inset,
    Outset,
    Hidden,
}

impl Default for BorderLineStyle {
    fn default() -> Self {
        BorderLineStyle::None
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BorderSide {
    pub width: f32,
    pub style: BorderLineStyle,
    pub color: Color,
}

impl Default for BorderSide {
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
pub enum BorderCollapse {
    Separate,
    Collapse,
}

impl Default for BorderCollapse {
    fn default() -> Self {
        BorderCollapse::Separate
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TableLayout {
    Auto,
    Fixed,
}

impl Default for TableLayout {
    fn default() -> Self {
        TableLayout::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmptyCells {
    Show,
    Hide,
}

impl Default for EmptyCells {
    fn default() -> Self {
        EmptyCells::Show
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CaptionSide {
    Top,
    Bottom,
}

impl Default for CaptionSide {
    fn default() -> Self {
        CaptionSide::Top
    }
}
