#![doc = "CSS parser and theming engine for the Liquide desktop shell."]
#![doc = ""]
#![doc = "Parses a subset of CSS used for theming Liquide UI widgets and the"]
#![doc = "desktop shell.  Provides `Theme`, `StyleSheet`, and `CssValue` types"]
#![doc = "consumed by the rendering pipeline."]

pub mod parser;
pub mod property;
pub mod theme;
pub mod value;

use thiserror::Error;

/// A parsed CSS value.
#[derive(Debug, Clone, PartialEq)]
pub enum CssValue {
    /// A length measurement (e.g. `12px`, `1.5em`).
    Length(f64, LengthUnit),
    /// A colour in RGBA.
    Color(Color),
    /// A plain string value (e.g. font family name).
    String(String),
    /// A numeric value without a unit.
    Number(f64),
    /// A percentage (0.0 .. 100.0).
    Percent(f64),
    /// The `inherit` keyword.
    Inherit,
    /// The `initial` keyword.
    Initial,
}

/// Length units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LengthUnit {
    /// Pixels.
    Px,
    /// Font-relative `em`.
    Em,
    /// Root-relative `rem`.
    Rem,
    /// Viewport width percentage.
    Vw,
    /// Viewport height percentage.
    Vh,
}

/// An RGBA colour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f32,
}

impl Color {
    /// Opaque white.
    pub const WHITE: Self = Self { r: 255, g: 255, b: 255, a: 1.0 };
    /// Opaque black.
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0, a: 1.0 };
    /// Fully transparent.
    pub const TRANSPARENT: Self = Self { r: 0, g: 0, b: 0, a: 0.0 };
}

/// A complete theme definition.
#[derive(Debug, Clone)]
pub struct Theme {
    /// Theme display name.
    pub name: String,
    /// The style sheets that compose this theme, in cascade order.
    pub sheets: Vec<StyleSheet>,
}

impl Theme {
    /// Create a new empty theme with the given name.
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            sheets: Vec::new(),
        }
    }
}

/// A parsed CSS style sheet.
#[derive(Debug, Clone)]
pub struct StyleSheet {
    /// Raw CSS source (kept for debugging / hot-reload diffing).
    pub source: String,
    /// Parsed rules.
    pub rules: Vec<StyleRule>,
}

/// A single CSS rule (selector + declarations).
#[derive(Debug, Clone)]
pub struct StyleRule {
    /// The selector string (e.g. `".button:hover"`).
    pub selector: String,
    /// Property declarations.
    pub declarations: Vec<(String, CssValue)>,
}

/// CSS parsing and theming errors.
#[derive(Debug, Error)]
pub enum CssError {
    /// The CSS source contains a syntax error.
    #[error("CSS syntax error at line {line}: {message}")]
    Syntax { line: usize, message: String },

    /// A property value could not be parsed.
    #[error("invalid value for property {property:?}: {value:?}")]
    InvalidValue { property: String, value: String },
}

/// Convenience result alias.
pub type Result<T> = std::result::Result<T, CssError>;
