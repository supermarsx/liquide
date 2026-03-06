//! Grid enums and types.

use serde::{Deserialize, Serialize};

/// Repeat mode for grid track definitions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RepeatMode {
    /// Fixed count: repeat(3, 100px)
    Count(u32),
    /// Auto-fill: repeat as many tracks as fit, keeping empty tracks
    AutoFill,
    /// Auto-fit: repeat as many tracks as fit, collapsing empty tracks
    AutoFit,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TrackSize {
    Px(f32),
    Percent(f32),
    Fr(f32),
    MinContent,
    MaxContent,
    Auto,
    MinMax(Box<TrackSize>, Box<TrackSize>),
    FitContent(f32),
    /// CSS Subgrid — inherits tracks from parent grid.
    Subgrid,
    /// CSS repeat() function for track repetition.
    Repeat {
        mode: RepeatMode,
        tracks: Vec<TrackSize>,
    },
}

impl Default for TrackSize {
    fn default() -> Self {
        TrackSize::Auto
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GridPlacement {
    pub start: GridLine,
    pub end: GridLine,
}

impl Default for GridPlacement {
    fn default() -> Self {
        Self {
            start: GridLine::Auto,
            end: GridLine::Auto,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GridLine {
    Auto,
    Line(i32),
    Span(u32),
    /// Named grid line or grid-area name (e.g. `grid-area: header`).
    Named(String),
}

impl Default for GridLine {
    fn default() -> Self {
        GridLine::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GridAutoFlow {
    Row,
    Column,
    RowDense,
    ColumnDense,
}

impl Default for GridAutoFlow {
    fn default() -> Self {
        GridAutoFlow::Row
    }
}
