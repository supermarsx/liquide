//! Display, position, and box model enums.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Display {
    Block,
    Inline,
    InlineBlock,
    Flex,
    InlineFlex,
    Grid,
    InlineGrid,
    Table,
    TableRow,
    TableCell,
    TableRowGroup,
    TableCaption,
    None,
    Contents,
    /// Establishes a new block formatting context (prevents margin collapse).
    FlowRoot,
    /// Block with a list marker (outside or inside).
    ListItem,
    /// Ruby annotation container.
    Ruby,
    /// Ruby text container.
    RubyText,
    /// Run-in box (collapses into following block if possible).
    RunIn,
    /// Table header group.
    TableHeaderGroup,
    /// Table footer group.
    TableFooterGroup,
    /// Table column.
    TableColumn,
    /// Table column group.
    TableColumnGroup,
}

impl Default for Display {
    fn default() -> Self {
        Display::Block
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Position {
    Static,
    Relative,
    Absolute,
    Fixed,
    Sticky,
}

impl Default for Position {
    fn default() -> Self {
        Position::Static
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoxSizing {
    ContentBox,
    BorderBox,
}

impl Default for BoxSizing {
    fn default() -> Self {
        BoxSizing::ContentBox
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Visibility {
    Visible,
    Hidden,
    Collapse,
}

impl Default for Visibility {
    fn default() -> Self {
        Visibility::Visible
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Float {
    None,
    Left,
    Right,
    InlineStart,
    InlineEnd,
}

impl Default for Float {
    fn default() -> Self {
        Float::None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Clear {
    None,
    Left,
    Right,
    Both,
    InlineStart,
    InlineEnd,
}

impl Default for Clear {
    fn default() -> Self {
        Clear::None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WritingMode {
    HorizontalTb,
    VerticalRl,
    VerticalLr,
    SidewaysRl,
    SidewaysLr,
}

impl Default for WritingMode {
    fn default() -> Self {
        WritingMode::HorizontalTb
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Ltr,
    Rtl,
}

impl Default for Direction {
    fn default() -> Self {
        Direction::Ltr
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnicodeBidi {
    Normal,
    Embed,
    Isolate,
    BidiOverride,
    IsolateOverride,
    Plaintext,
}

impl Default for UnicodeBidi {
    fn default() -> Self {
        UnicodeBidi::Normal
    }
}
