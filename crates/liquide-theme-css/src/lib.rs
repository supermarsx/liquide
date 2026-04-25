//! CSS-based theme system for Liquide compositor
//!
//! This crate provides a full-featured CSS parser and theme engine,
//! allowing desktop themes to be defined using standard CSS syntax.
//!
//! # Features
//!
//! - Complete CSS 3 parser
//! - Selector matching (class, ID, pseudo-classes)
//! - CSS variables (custom properties)
//! - Color manipulation and gradients
//! - Animation support
//! - Hot-reloading themes
//!
//! # CSS Theme Format
//!
//! Themes use standard CSS with custom selectors for desktop elements:
//!
//! ```css
//! /* Window styling */
//! window {
//!     background: #2e3440;
//!     border: 1px solid #4c566a;
//!     border-radius: 8px;
//!     box-shadow: 0 4px 16px rgba(0, 0, 0, 0.3);
//! }
//!
//! window.focused {
//!     border-color: #5e81ac;
//! }
//!
//! /* Titlebar */
//! titlebar {
//!     background: linear-gradient(180deg, #3b4252 0%, #2e3440 100%);
//!     height: 32px;
//!     color: #eceff4;
//! }
//!
//! /* Buttons */
//! button.close {
//!     background: #bf616a;
//!     border-radius: 50%;
//! }
//!
//! button.close:hover {
//!     background: #d08770;
//! }
//! ```
//!
//! # Example
//!
//! ```rust
//! use liquide_theme_css::{ThemeParser, ThemeEngine};
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Parse theme from CSS
//! let css = r#"
//!     window {
//!         background: #2e3440;
//!         border-radius: 8px;
//!     }
//! "#;
//!
//! let parser = ThemeParser::new();
//! let theme = parser.parse_str(css)?;
//!
//! // Query styles
//! let engine = ThemeEngine::new(theme);
//! let styles = engine.query("window", &[], &[])?;
//!
//! println!("Background: {:?}", styles.get("background"));
//! # Ok(())
//! # }
//! ```

pub mod cache;
pub mod engine;
pub mod error;
pub mod parser;
pub mod property;
pub mod selector;
pub mod stylesheet;
pub mod value;
pub mod watcher;

pub use cache::{CacheStats, QueryCache};
pub use engine::ThemeEngine;
pub use error::{Result, ThemeError};
pub use parser::ThemeParser;
pub use stylesheet::{QueryEnvironment, StyleSheet};

pub mod prelude {
    pub use crate::cache::{CacheStats, QueryCache};
    pub use crate::engine::ThemeEngine;
    pub use crate::error::{Result, ThemeError};
    pub use crate::parser::ThemeParser;
    pub use crate::stylesheet::{QueryEnvironment, StyleSheet};
    pub use crate::value::{Color, PropertyValue};
}

#[cfg(test)]
#[path = "tests/css_conformance_fixtures.rs"]
mod css_conformance_fixtures;
