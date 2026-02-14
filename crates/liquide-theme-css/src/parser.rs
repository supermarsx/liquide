//! CSS parser for themes using lightningcss

use crate::error::{Result, ThemeError};
use crate::property::PropertySet;
use crate::selector::Selector;
use crate::stylesheet::StyleSheet;
use crate::value::{Color, LengthUnit, PropertyValue};
use std::path::Path;

use lightningcss::printer::Printer;
use lightningcss::properties::Property;
use lightningcss::rules::CssRule;
use lightningcss::stylesheet::{ParserOptions, PrinterOptions, StyleSheet as LightningStyleSheet};
use lightningcss::traits::ToCss;
use lightningcss::values::color::CssColor;

/// CSS theme parser using lightningcss for full CSS3 support
pub struct ThemeParser {}

impl Default for ThemeParser {
    fn default() -> Self {
        Self::new()
    }
}

impl ThemeParser {
    /// Create a new theme parser
    pub fn new() -> Self {
        Self {}
    }

    /// Parse CSS from a string
    pub fn parse_str(&self, css: &str) -> Result<StyleSheet> {
        // Parse with lightningcss - use default options with static lifetime
        let options = ParserOptions::default();
        let lightning_sheet =
            LightningStyleSheet::parse(css, options).map_err(|e| ThemeError::ParseError {
                message: format!("lightningcss parse error: {:?}", e),
                location: "unknown".to_string(),
            })?;

        // Convert to our stylesheet format
        self.convert_stylesheet(lightning_sheet)
    }

    /// Parse CSS from a file
    pub fn parse_file<P: AsRef<Path>>(&self, path: P) -> Result<StyleSheet> {
        let css = std::fs::read_to_string(path)?;
        self.parse_str(&css)
    }

    /// Convert lightningcss StyleSheet to our StyleSheet format
    fn convert_stylesheet(&self, lightning: LightningStyleSheet) -> Result<StyleSheet> {
        let mut stylesheet = StyleSheet::new();

        // Process all rules
        for rule in lightning.rules.0.iter() {
            self.process_rule(rule, &mut stylesheet)?;
        }

        Ok(stylesheet)
    }

    /// Process a CSS rule recursively
    fn process_rule(&self, rule: &CssRule, stylesheet: &mut StyleSheet) -> Result<()> {
        match rule {
            CssRule::Style(style_rule) => {
                // Convert selector list to our format
                for selector in &style_rule.selectors.0 {
                    let selector_str = self.selector_to_string(selector)?;
                    let our_selector = Selector::parse(&selector_str)?;

                    // Convert declarations to properties
                    let properties = self.convert_declarations(&style_rule.declarations)?;

                    stylesheet.add_rule(our_selector, properties);
                }
            }
            CssRule::Media(media) => {
                // Process nested rules in media queries
                for nested_rule in &media.rules.0 {
                    self.process_rule(nested_rule, stylesheet)?;
                }
            }
            CssRule::Supports(supports) => {
                // Process nested rules in @supports
                for nested_rule in &supports.rules.0 {
                    self.process_rule(nested_rule, stylesheet)?;
                }
            }
            // Ignore other rule types (keyframes, font-face, import, etc.)
            _ => {}
        }

        Ok(())
    }

    /// Convert lightningcss selector to string
    fn selector_to_string(
        &self,
        selector: &lightningcss::selector::Selector<'_>,
    ) -> Result<String> {
        let mut css_string = String::new();
        let mut printer = Printer::new(&mut css_string, PrinterOptions::default());
        selector
            .to_css(&mut printer)
            .map_err(|e| ThemeError::ParseError {
                message: format!("Failed to serialize selector: {:?}", e),
                location: "selector".to_string(),
            })?;
        Ok(css_string)
    }

    /// Convert lightningcss declarations to our PropertySet
    fn convert_declarations(
        &self,
        decls: &lightningcss::declaration::DeclarationBlock,
    ) -> Result<PropertySet> {
        let mut properties = PropertySet::new();

        for decl in &decls.declarations {
            if let Some((name, value)) = self.convert_property(decl) {
                properties.insert(name, value);
            }
        }

        Ok(properties)
    }

    /// Convert a single property
    fn convert_property(&self, prop: &Property) -> Option<(String, PropertyValue)> {
        match prop {
            Property::BackgroundColor(color) => {
                Some(("background".to_string(), self.convert_color(color)?))
            }
            Property::Color(color) => Some(("color".to_string(), self.convert_color(color)?)),
            Property::BorderTopColor(color)
            | Property::BorderRightColor(color)
            | Property::BorderBottomColor(color)
            | Property::BorderLeftColor(color) => {
                Some(("border-color".to_string(), self.convert_color(color)?))
            }
            // Width/Height are Size types, just serialize them as strings for now
            Property::Width(size) | Property::Height(size) => {
                let mut size_str = String::new();
                let mut printer = Printer::new(&mut size_str, PrinterOptions::default());
                if size.to_css(&mut printer).is_ok() {
                    if let Some(px_value) = size_str.strip_suffix("px") {
                        if let Ok(num) = px_value.trim().parse::<f32>() {
                            let prop_name = if matches!(prop, Property::Width(_)) {
                                "width"
                            } else {
                                "height"
                            };
                            return Some((
                                prop_name.to_string(),
                                PropertyValue::Length(LengthUnit::Px(num)),
                            ));
                        }
                    }
                }
                None
            }
            Property::BorderTopWidth(width)
            | Property::BorderRightWidth(width)
            | Property::BorderBottomWidth(width)
            | Property::BorderLeftWidth(width) => Some((
                "border-width".to_string(),
                self.convert_border_width(width)?,
            )),
            // For now, skip unparsed properties
            // TODO: Handle more property types as needed
            _ => None,
        }
    }

    /// Convert lightningcss color to our Color type
    fn convert_color(&self, css_color: &CssColor) -> Option<PropertyValue> {
        match css_color {
            CssColor::RGBA(rgba) => Some(PropertyValue::Color(Color::new(
                rgba.red, rgba.green, rgba.blue, rgba.alpha,
            ))),
            _ => {
                // For other color types, try to serialize and parse
                let mut color_str = String::new();
                let mut printer = Printer::new(&mut color_str, PrinterOptions::default());
                if css_color.to_css(&mut printer).is_ok() {
                    if let Ok(color) = Color::from_hex(&color_str) {
                        return Some(PropertyValue::Color(color));
                    }
                }
                None
            }
        }
    }

    /// Convert lightningcss length value wrapper
    fn convert_length_value(
        &self,
        length: &lightningcss::values::length::Length,
    ) -> Option<PropertyValue> {
        // Try to serialize and parse
        let mut length_str = String::new();
        let mut printer = Printer::new(&mut length_str, PrinterOptions::default());
        if length.to_css(&mut printer).is_ok() {
            // Try to parse as px, em, etc.
            if let Some(px_value) = length_str.strip_suffix("px") {
                if let Ok(num) = px_value.trim().parse::<f32>() {
                    return Some(PropertyValue::Length(LengthUnit::Px(num)));
                }
            }
            if let Some(em_value) = length_str.strip_suffix("em") {
                if let Ok(num) = em_value.trim().parse::<f32>() {
                    return Some(PropertyValue::Length(LengthUnit::Em(num)));
                }
            }
        }
        None
    }

    /// Convert border width
    fn convert_border_width(
        &self,
        width: &lightningcss::properties::border::BorderSideWidth,
    ) -> Option<PropertyValue> {
        // Serialize and parse
        let mut width_str = String::new();
        let mut printer = Printer::new(&mut width_str, PrinterOptions::default());
        if width.to_css(&mut printer).is_ok() {
            match width_str.as_str() {
                "thin" => return Some(PropertyValue::Length(LengthUnit::Px(1.0))),
                "medium" => return Some(PropertyValue::Length(LengthUnit::Px(3.0))),
                "thick" => return Some(PropertyValue::Length(LengthUnit::Px(5.0))),
                _ => {
                    // Try to parse as length
                    if let Some(px_value) = width_str.strip_suffix("px") {
                        if let Ok(num) = px_value.trim().parse::<f32>() {
                            return Some(PropertyValue::Length(LengthUnit::Px(num)));
                        }
                    }
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let css = r#"
            button {
                background: #ff0000;
                width: 100px;
            }
        "#;

        let parser = ThemeParser::new();
        let sheet = parser.parse_str(css).unwrap();

        assert_eq!(sheet.rule_count(), 1);
    }

    #[test]
    fn test_parse_multiple_rules() {
        let css = r#"
            button {
                background: #ff0000;
            }
            
            window {
                border: 1px;
            }
        "#;

        let parser = ThemeParser::new();
        let sheet = parser.parse_str(css).unwrap();

        assert_eq!(sheet.rule_count(), 2);
    }

    #[test]
    fn test_parse_with_comments() {
        let css = r#"
            /* This is a comment */
            button {
                background: #ff0000;
                /* Another comment */
                width: 100px;
            }
        "#;

        let parser = ThemeParser::new();
        let sheet = parser.parse_str(css).unwrap();

        assert_eq!(sheet.rule_count(), 1);
    }

    #[test]
    fn test_parse_pseudo_classes() {
        let css = r#"
            button:hover {
                background: #00ff00;
            }
        "#;

        let parser = ThemeParser::new();
        let result = parser.parse_str(css);

        // Should parse successfully with lightningcss
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_rgba_colors() {
        let css = r#"
            window {
                background: rgba(255, 0, 0, 0.5);
            }
        "#;

        let parser = ThemeParser::new();
        let result = parser.parse_str(css);

        // Should parse successfully with lightningcss
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_css_variables() {
        let css = r#"
            :root {
                --primary: #5e81ac;
            }
            
            button {
                background: var(--primary);
            }
        "#;

        let parser = ThemeParser::new();
        let result = parser.parse_str(css);

        // Should parse successfully with lightningcss (full CSS3 support)
        assert!(result.is_ok());
    }
}
