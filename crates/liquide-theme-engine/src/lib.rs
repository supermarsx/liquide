//! Comprehensive theming framework for the LiquiDE desktop environment.
//!
//! Provides theme creation, variant support, inheritance resolution,
//! CSS variable generation, smooth theme transitions, built-in themes,
//! and a simple TOML-like theme file parser.

pub mod color;
pub mod palette;
pub mod definition;
pub mod manager;
pub mod transition;
pub mod builtin;
pub mod parser;

#[cfg(test)]
mod tests;

// Re-export primary types at crate root for convenience.
pub use color::Color;
pub use palette::ColorPalette;
pub use definition::{
    ThemeVariant, ThemeMetadata, ThemeDefinition,
    WindowTheme, StatusBarTheme, DockTheme, MenuTheme,
    TooltipTheme, NotificationTheme, GlassParams,
};
pub use manager::{ThemeManager, ThemeError};
pub use transition::ThemeTransition;
pub use parser::{parse_theme, ParseError};
pub use builtin::{builtin_night, builtin_midday, builtin_sunset, builtin_liquid_glass};
