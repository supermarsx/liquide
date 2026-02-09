//! CSS source parser.

use crate::{CssError, StyleRule, StyleSheet};

/// Parse a CSS source string into a [`StyleSheet`].
///
/// # Errors
///
/// Returns [`CssError::Syntax`] if the input contains invalid CSS.
pub fn parse(source: &str) -> crate::Result<StyleSheet> {
    // Stub — real implementation would tokenize and parse the CSS.
    let _ = source;
    Ok(StyleSheet {
        source: source.to_string(),
        rules: Vec::new(),
    })
}
