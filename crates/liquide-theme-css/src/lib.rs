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

pub mod parser;
pub mod engine;
pub mod selector;
pub mod value;
pub mod error;
pub mod stylesheet;
pub mod property;
pub mod watcher;
pub mod cache;

pub use parser::ThemeParser;
pub use engine::ThemeEngine;
pub use stylesheet::StyleSheet;
pub use error::{ThemeError, Result};
pub use cache::{QueryCache, CacheStats};

pub mod prelude {
    pub use crate::parser::ThemeParser;
    pub use crate::engine::ThemeEngine;
    pub use crate::stylesheet::StyleSheet;
    pub use crate::error::{ThemeError, Result};
    pub use crate::value::{PropertyValue, Color};
    pub use crate::cache::{QueryCache, CacheStats};
}
