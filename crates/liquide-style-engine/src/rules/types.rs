//! Core types for rule validation: severity levels, violations, and requirements.

use liquide_dom::NodeId;

use crate::computed::{
    AlignItems, ComputedStyle, Display, FlexDirection, JustifyContent, Position,
};

// ═══════════════════════════════════════════════════════════════════════════
// Rule Types
// ═══════════════════════════════════════════════════════════════════════════

/// Severity level for rule violations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Will cause visual glitches but may still render.
    Warning,
    /// Element will not function correctly.
    Error,
    /// Element will be completely broken/invisible.
    Critical,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Warning => write!(f, "WARN"),
            Severity::Error => write!(f, "ERROR"),
            Severity::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// A violation found during rule validation.
#[derive(Debug, Clone)]
pub struct Violation {
    /// The element tag or ID that has the violation.
    pub element: String,
    /// Node ID in the document.
    pub node_id: NodeId,
    /// The rule that was violated.
    pub rule_name: String,
    /// Human-readable description of the problem.
    pub message: String,
    /// How severe is this violation.
    pub severity: Severity,
    /// Suggested fix.
    pub suggestion: Option<String>,
}

/// A property requirement that an element must satisfy.
#[derive(Debug, Clone)]
pub enum PropertyRequirement {
    /// Display must be one of these values.
    DisplayOneOf(Vec<Display>),
    /// Position must be one of these values.
    PositionOneOf(Vec<Position>),
    /// Flex direction must be one of these values.
    FlexDirectionOneOf(Vec<FlexDirection>),
    /// Justify content must be one of these values.
    JustifyContentOneOf(Vec<JustifyContent>),
    /// Align items must be one of these values.
    AlignItemsOneOf(Vec<AlignItems>),
    /// Width must be set (not Auto).
    WidthDefined,
    /// Height must be set (not Auto).
    HeightDefined,
    /// Min width must be set.
    MinWidthDefined,
    /// Max width must be set.
    MaxWidthDefined,
    /// z-index must be set.
    ZIndexDefined,
    /// Border radius must be > 0.
    BorderRadiusPositive,
    /// Background color alpha must be > 0.
    BackgroundVisible,
    /// Blur radius must be > 0 (for glass effects).
    BlurRadiusPositive,
    /// Gap must be > 0 (for flex/grid containers).
    GapPositive,
    /// Padding must be set on at least one side.
    PaddingDefined,
    /// Font size must be in range.
    FontSizeInRange { min: f32, max: f32 },
    /// Color must be visible (alpha > 0).
    ColorVisible,
    /// Custom predicate with message.
    Custom {
        name: String,
        check: fn(&ComputedStyle) -> bool,
        message: String,
    },
}

/// A structural requirement for the DOM hierarchy.
#[derive(Debug, Clone)]
pub enum StructureRequirement {
    /// Element must be a direct child of elements with these tags.
    ParentTagOneOf(Vec<String>),
    /// Element must contain at least one child with this tag.
    MustContainChild(String),
    /// Element must have specific children in order.
    RequiredChildren(Vec<String>),
    /// Element must have an ID attribute.
    MustHaveId,
    /// Element must have specific attribute.
    MustHaveAttribute(String),
    /// Element must have specific class.
    MustHaveClass(String),
}
