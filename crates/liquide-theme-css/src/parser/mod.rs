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

use lightningcss::stylesheet::{ParserFlags, ParserOptions, StyleSheet as LightningStyleSheet};

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
        self.parse_str_with_filename(css, "<inline>".to_string())
    }

    /// Parse a single CSS declaration (`name: value`) and return the converted
    /// property set.
    ///
    /// This runs the *full* lightningcss property parser on the declaration, so
    /// multi-token shorthands (e.g. `box-shadow: 0 8px 32px rgba(0,0,0,.5)`) are
    /// expanded into their structured `PropertyValue` (e.g.
    /// [`PropertyValue::BoxShadow`]) and longhands — exactly as if the
    /// declaration had appeared in a stylesheet. This is what the style engine
    /// uses to re-parse `var()`-substituted shorthand values, which the
    /// single-value inline parser cannot handle.
    ///
    /// Returns `None` if the declaration does not parse or yields no recognized
    /// property for `name`.
    pub fn parse_declaration(&self, name: &str, value: &str) -> Option<PropertySet> {
        // Wrap in a trivial rule so lightningcss applies its real property
        // grammar (not our single-value inline fallback).
        let css = format!("x{{{}:{}}}", name, value);
        let sheet = self.parse_str(&css).ok()?;
        let props = sheet.rules().first().map(|rule| rule.properties.clone())?;
        if props.iter().next().is_none() {
            return None;
        }
        Some(props)
    }

    /// Parse CSS from a file.
    pub fn parse_file<P: AsRef<Path>>(&self, path: P) -> Result<StyleSheet> {
        let path = path.as_ref();
        let css = std::fs::read_to_string(path)?;
        self.parse_str_with_filename(&css, path.to_string_lossy().into_owned())
    }

    fn parse_str_with_filename(&self, css: &str, filename: String) -> Result<StyleSheet> {
        let options = ParserOptions {
            filename: filename.clone(),
            flags: ParserFlags::NESTING,
            ..ParserOptions::default()
        };
        let lightning_sheet =
            LightningStyleSheet::parse(css, options).map_err(|error| ThemeError::ParseError {
                message: format!("lightningcss parse error: {}", error.kind),
                location: error
                    .loc
                    .as_ref()
                    .map(Self::format_parse_error_location)
                    .unwrap_or(filename),
            })?;

        self.convert_stylesheet(lightning_sheet)
    }

    fn format_parse_error_location(location: &lightningcss::error::ErrorLocation) -> String {
        format!(
            "{}:{}:{}",
            location.filename,
            location.line.saturating_add(1),
            location.column
        )
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
