//! CSS parser for themes

use crate::error::{Result, ThemeError};
use crate::property::PropertySet;
use crate::selector::Selector;
use crate::stylesheet::StyleSheet;
use crate::value::{BorderStyle, BoxShadow, Color, ColorStop, Gradient, LengthUnit, PropertyValue};
use std::path::Path;

/// CSS theme parser
pub struct ThemeParser {
    // Parser state
}

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
        let mut stylesheet = StyleSheet::new();
        
        // Simple parser (in production, use lightningcss)
        let rules = self.parse_rules(css)?;
        
        for (selector_str, properties) in rules {
            let selector = Selector::parse(&selector_str)?;
            stylesheet.add_rule(selector, properties);
        }
        
        Ok(stylesheet)
    }
    
    /// Parse CSS from a file
    pub fn parse_file<P: AsRef<Path>>(&self, path: P) -> Result<StyleSheet> {
        let css = std::fs::read_to_string(path)?;
        self.parse_str(&css)
    }
    
    /// Parse CSS rules (simplified parser)
    fn parse_rules(&self, css: &str) -> Result<Vec<(String, PropertySet)>> {
        let mut rules = Vec::new();
        let mut current_pos = 0;
        let chars: Vec<char> = css.chars().collect();
        
        while current_pos < chars.len() {
            // Skip whitespace and comments
            current_pos = self.skip_whitespace_and_comments(&chars, current_pos);
            
            if current_pos >= chars.len() {
                break;
            }
            
            // Parse selector
            let (selector, new_pos) = self.parse_selector(&chars, current_pos)?;
            current_pos = new_pos;
            
            // Expect '{'
            current_pos = self.skip_whitespace(&chars, current_pos);
            if current_pos >= chars.len() || chars[current_pos] != '{' {
                return Err(ThemeError::ParseError {
                    message: "Expected '{'".to_string(),
                    location: format!("position {}", current_pos),
                });
            }
            current_pos += 1;
            
            // Parse properties
            let (properties, new_pos) = self.parse_properties(&chars, current_pos)?;
            current_pos = new_pos;
            
            // Expect '}'
            current_pos = self.skip_whitespace(&chars, current_pos);
            if current_pos >= chars.len() || chars[current_pos] != '}' {
                return Err(ThemeError::ParseError {
                    message: "Expected '}'".to_string(),
                    location: format!("position {}", current_pos),
                });
            }
            current_pos += 1;
            
            rules.push((selector, properties));
        }
        
        Ok(rules)
    }
    
    fn parse_selector(&self, chars: &[char], start: usize) -> Result<(String, usize)> {
        let mut pos = start;
        let mut selector = String::new();
        
        while pos < chars.len() && chars[pos] != '{' {
            selector.push(chars[pos]);
            pos += 1;
        }
        
        Ok((selector.trim().to_string(), pos))
    }
    
    fn parse_properties(&self, chars: &[char], start: usize) -> Result<(PropertySet, usize)> {
        let mut pos = start;
        let mut properties = PropertySet::new();
        
        while pos < chars.len() && chars[pos] != '}' {
            pos = self.skip_whitespace(&chars, pos);
            
            if pos >= chars.len() || chars[pos] == '}' {
                break;
            }
            
            // Parse property name
            let (name, new_pos) = self.parse_identifier(&chars, pos)?;
            pos = new_pos;
            
            // Expect ':'
            pos = self.skip_whitespace(&chars, pos);
            if pos >= chars.len() || chars[pos] != ':' {
                return Err(ThemeError::ParseError {
                    message: "Expected ':'".to_string(),
                    location: format!("position {}", pos),
                });
            }
            pos += 1;
            
            // Parse property value
            let (value_str, new_pos) = self.parse_value(&chars, pos)?;
            pos = new_pos;
            
            // Parse value into PropertyValue
            let value = self.parse_property_value(&name, &value_str)?;
            properties.insert(name, value);
            
            // Expect ';'
            pos = self.skip_whitespace(&chars, pos);
            if pos < chars.len() && chars[pos] == ';' {
                pos += 1;
            }
        }
        
        Ok((properties, pos))
    }
    
    fn parse_identifier(&self, chars: &[char], start: usize) -> Result<(String, usize)> {
        let mut pos = start;
        let mut ident = String::new();
        
        while pos < chars.len() {
            let ch = chars[pos];
            if ch.is_alphanumeric() || ch == '-' || ch == '_' {
                ident.push(ch);
                pos += 1;
            } else {
                break;
            }
        }
        
        Ok((ident, pos))
    }
    
    fn parse_value(&self, chars: &[char], start: usize) -> Result<(String, usize)> {
        let mut pos = start;
        pos = self.skip_whitespace(&chars, pos);
        
        let mut value = String::new();
        let mut depth = 0;
        
        while pos < chars.len() {
            let ch = chars[pos];
            
            if ch == '(' {
                depth += 1;
            } else if ch == ')' {
                depth -= 1;
            }
            
            if (ch == ';' || ch == '}') && depth == 0 {
                break;
            }
            
            value.push(ch);
            pos += 1;
        }
        
        Ok((value.trim().to_string(), pos))
    }
    
    fn parse_property_value(&self, name: &str, value: &str) -> Result<PropertyValue> {
        let value = value.trim();
        
        // Try to parse as color
        if let Ok(color) = Color::from_hex(value) {
            return Ok(PropertyValue::Color(color));
        }
        
        // Try to parse as length
        if let Some(px_value) = value.strip_suffix("px") {
            if let Ok(num) = px_value.trim().parse::<f32>() {
                return Ok(PropertyValue::Length(LengthUnit::Px(num)));
            }
        }
        
        if let Some(em_value) = value.strip_suffix("em") {
            if let Ok(num) = em_value.trim().parse::<f32>() {
                return Ok(PropertyValue::Length(LengthUnit::Em(num)));
            }
        }
        
        if let Some(pct_value) = value.strip_suffix('%') {
            if let Ok(num) = pct_value.trim().parse::<f32>() {
                return Ok(PropertyValue::Length(LengthUnit::Percent(num)));
            }
        }
        
        // Try to parse as number
        if let Ok(num) = value.parse::<f32>() {
            return Ok(PropertyValue::Number(num));
        }
        
        // Border styles
        if name.contains("border-style") {
            let style = match value.to_lowercase().as_str() {
                "solid" => BorderStyle::Solid,
                "dashed" => BorderStyle::Dashed,
                "dotted" => BorderStyle::Dotted,
                "double" => BorderStyle::Double,
                _ => BorderStyle::None,
            };
            return Ok(PropertyValue::BorderStyle(style));
        }
        
        // Default to keyword/string
        if value.starts_with('"') && value.ends_with('"') {
            Ok(PropertyValue::String(value[1..value.len()-1].to_string()))
        } else {
            Ok(PropertyValue::Keyword(value.to_string()))
        }
    }
    
    fn skip_whitespace(&self, chars: &[char], start: usize) -> usize {
        let mut pos = start;
        while pos < chars.len() && chars[pos].is_whitespace() {
            pos += 1;
        }
        pos
    }
    
    fn skip_whitespace_and_comments(&self, chars: &[char], start: usize) -> usize {
        let mut pos = start;
        
        loop {
            pos = self.skip_whitespace(&chars, pos);
            
            // Check for comments
            if pos + 1 < chars.len() && chars[pos] == '/' && chars[pos + 1] == '*' {
                // Skip until */
                pos += 2;
                while pos + 1 < chars.len() {
                    if chars[pos] == '*' && chars[pos + 1] == '/' {
                        pos += 2;
                        break;
                    }
                    pos += 1;
                }
            } else {
                break;
            }
        }
        
        pos
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
}
