//! CSS parser for themes using lightningcss.
//!
//! Split into focused sub-modules:
//! - [`serialize`] — CSS value serialization helpers
//! - [`rules`] — at-rule and style rule processing
//! - [`properties`] — property declaration conversion
//! - [`gradient`] — gradient parsing and conversion
//! - [`values`] — length, color, and keyword parsing
//! - [`math_expr`] — `calc()` / `min()` / `max()` / `clamp()` parsing

mod gradient;
mod math_expr;
mod properties;
mod rules;
mod serialize;
mod values;

use crate::error::{Result, ThemeError};
use crate::property::PropertySet;
use crate::stylesheet::StyleSheet;
use crate::value::PropertyValue;

use std::path::Path;

use lightningcss::stylesheet::{
    ParserFlags, ParserOptions, StyleSheet as LightningStyleSheet,
};

/// CSS theme parser using lightningcss for full CSS3 support.
pub struct ThemeParser {}

impl Default for ThemeParser {
    fn default() -> Self {
        Self::new()
    }
}

impl ThemeParser {
    /// Create a new theme parser.
    pub fn new() -> Self {
        Self {}
    }

    /// Parse CSS from a string.
    pub fn parse_str(&self, css: &str) -> Result<StyleSheet> {
        // Parse with lightningcss — enable CSS nesting support
        let options = ParserOptions {
            flags: ParserFlags::NESTING,
            ..ParserOptions::default()
        };
        let lightning_sheet =
            LightningStyleSheet::parse(css, options).map_err(|e| ThemeError::ParseError {
                message: format!("lightningcss parse error: {:?}", e),
                location: "unknown".to_string(),
            })?;

        // Convert to our stylesheet format
        self.convert_stylesheet(lightning_sheet)
    }

    /// Parse CSS from a file.
    pub fn parse_file<P: AsRef<Path>>(&self, path: P) -> Result<StyleSheet> {
        let css = std::fs::read_to_string(path)?;
        self.parse_str(&css)
    }

    /// Convert lightningcss `StyleSheet` to our `StyleSheet` format.
    fn convert_stylesheet(&self, lightning: LightningStyleSheet) -> Result<StyleSheet> {
        let mut stylesheet = StyleSheet::new();

        // Process all rules
        for rule in lightning.rules.0.iter() {
            self.process_rule(rule, &mut stylesheet)?;
        }

        Ok(stylesheet)
    }

    /// Convert lightningcss declarations to our `PropertySet`.
    pub(crate) fn convert_declarations(
        &self,
        decls: &lightningcss::declaration::DeclarationBlock,
    ) -> Result<PropertySet> {
        let mut properties = PropertySet::new();

        // Process normal declarations
        for decl in &decls.declarations {
            self.insert_property(decl, &mut properties);
        }

        // Process !important declarations into a temporary set so we can
        // track which property names came from the important list.
        let mut important_props = PropertySet::new();
        for decl in &decls.important_declarations {
            self.insert_property(decl, &mut important_props);
        }
        // Merge important properties and mark them
        for (key, value) in important_props.iter() {
            properties.insert(key.clone(), value.clone());
            properties.mark_important(key);
        }

        Ok(properties)
    }

    /// Convert declarations to (name, value) pairs — used for @keyframes.
    pub(crate) fn convert_declarations_to_pairs(
        &self,
        decls: &lightningcss::declaration::DeclarationBlock,
    ) -> Result<Vec<(String, PropertyValue)>> {
        let props = self.convert_declarations(decls)?;
        Ok(props
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect())
    }
}

#[cfg(test)]
#[path = "../tests/parser_tests.rs"]
mod tests;
