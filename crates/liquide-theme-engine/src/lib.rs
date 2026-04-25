//! Comprehensive theming framework for the LiquiDE desktop environment.
//!
//! Provides theme creation, variant support, inheritance resolution,
//! CSS variable generation, smooth theme transitions, built-in themes,
//! and a simple TOML-like theme file parser.

pub mod builtin;
pub mod color;
pub mod definition;
pub mod manager;
pub mod palette;
pub mod parser;
pub mod transition;

#[cfg(test)]
mod tests;

// Re-export primary types at crate root for convenience.
pub use builtin::{builtin_liquid_glass, builtin_midday, builtin_night, builtin_sunset};
pub use color::Color;
pub use definition::{
    DockTheme, GlassParams, MenuTheme, NotificationTheme, StatusBarTheme, ThemeDefinition,
    ThemeMetadata, ThemeVariant, TooltipTheme, WindowTheme,
};
pub use manager::{ThemeError, ThemeManager};
pub use palette::ColorPalette;
pub use parser::{ParseError, ParsedTheme, parse_theme, parse_theme_source};
pub use transition::ThemeTransition;
