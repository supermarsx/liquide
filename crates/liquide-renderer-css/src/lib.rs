//! CSS-to-renderer style translation middleware.
//!
//! This crate bridges the CSS theme system (`liquide-theme-css`) with the
//! renderer layer (`liquide-compositor`, `liquide-renderer-cpu/gpu`). It
//! translates CSS properties into renderer-friendly data structures like
//! colors, glass effects, shadows, borders, and transforms.
//!
//! # Architecture
//!
//! ```text
//! CSS Theme → ThemeEngine → StyleResolver → RenderStyle → Renderer
//! ```
//!
//! - **CSS Theme**: Raw CSS parsed by `liquide-theme-css`
//! - **ThemeEngine**: Query engine for CSS selectors
//! - **StyleResolver**: This crate - converts CSS to RenderStyle
//! - **RenderStyle**: Renderer-friendly style data structures
//! - **Renderer**: CPU/GPU renderers consume RenderStyle
//!
//! # Example
//!
//! ```rust,ignore
//! use liquide_renderer_css::{StyleResolver, RenderStyle};
//! use liquide_theme_css::ThemeEngine;
//!
//! let engine = ThemeEngine::from_css_file("theme.css")?;
//! let resolver = StyleResolver::new(engine);
//!
//! // Resolve styles for a window element
//! let style = resolver.resolve("window", &["focused"], &[], None)?;
//!
//! // Use style properties
//! let bg_color = style.background_color();
//! let glass_effect = style.glass();
//! let shadow = style.box_shadow();
//! ```

pub mod glass;
pub mod resolver;
pub mod shadow;
pub mod style;
pub mod transform;

pub use glass::GlassStyle;
pub use resolver::StyleResolver;
pub use shadow::ShadowStyle;
pub use style::{BorderStyle, RenderStyle};
pub use transform::TransformStyle;

use thiserror::Error;

/// Errors from CSS→Renderer style translation.
#[derive(Debug, Error)]
pub enum StyleError {
    /// Failed to query CSS theme.
    #[error("theme query failed: {0}")]
    ThemeQueryFailed(String),

    /// Invalid CSS property value.
    #[error("invalid property value: {property} = {value}")]
    InvalidPropertyValue { property: String, value: String },

    /// Required property missing.
    #[error("required property missing: {0}")]
    MissingProperty(String),

    /// CSS engine error.
    #[error("css engine error: {0}")]
    CssError(#[from] liquide_theme_css::error::ThemeError),
}

pub type Result<T> = std::result::Result<T, StyleError>;
