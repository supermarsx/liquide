//! Theme engine for applying CSS styles

use crate::error::{Result, ThemeError};
use crate::property::PropertySet;
use crate::stylesheet::StyleSheet;
use crate::value::PropertyValue;

/// Theme engine for querying and applying styles
pub struct ThemeEngine {
    stylesheet: StyleSheet,
}

impl ThemeEngine {
    /// Create a new theme engine with a stylesheet
    pub fn new(stylesheet: StyleSheet) -> Self {
        Self { stylesheet }
    }
    
    /// Query styles for an element
    ///
    /// Returns the computed property set after applying CSS cascade rules
    pub fn query(
        &self,
        element: &str,
        classes: &[String],
        pseudo_classes: &[String],
    ) -> Result<PropertySet> {
        Ok(self.stylesheet.compute_styles(element, classes, None, pseudo_classes))
    }
    
    /// Query styles with ID
    pub fn query_with_id(
        &self,
        element: &str,
        id: Option<&str>,
        classes: &[String],
        pseudo_classes: &[String],
    ) -> Result<PropertySet> {
        Ok(self.stylesheet.compute_styles(element, classes, id, pseudo_classes))
    }
    
    /// Get a specific property value
    pub fn get_property(
        &self,
        element: &str,
        classes: &[String],
        pseudo_classes: &[String],
        property: &str,
    ) -> Result<Option<PropertyValue>> {
        let styles = self.query(element, classes, pseudo_classes)?;
        Ok(styles.get(property).cloned())
    }
    
    /// Check if element has a specific style
    pub fn has_property(
        &self,
        element: &str,
        classes: &[String],
        pseudo_classes: &[String],
        property: &str,
    ) -> bool {
        if let Ok(styles) = self.query(element, classes, pseudo_classes) {
            styles.has(property)
        } else {
            false
        }
    }
    
    /// Get a CSS variable
    pub fn get_variable(&self, name: &str) -> Option<&PropertyValue> {
        self.stylesheet.get_variable(name)
    }
    
    /// Get the underlying stylesheet
    pub fn stylesheet(&self) -> &StyleSheet {
        &self.stylesheet
    }
    
    /// Replace the stylesheet (for hot-reloading)
    pub fn set_stylesheet(&mut self, stylesheet: StyleSheet) {
        self.stylesheet = stylesheet;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ThemeParser;
    use crate::value::Color;
    
    #[test]
    fn test_query() {
        let css = r#"
            button {
                background: #ff0000;
                width: 100px;
            }
        "#;
        
        let parser = ThemeParser::new();
        let sheet = parser.parse_str(css).unwrap();
        let engine = ThemeEngine::new(sheet);
        
        let styles = engine.query("button", &[], &[]).unwrap();
        assert!(styles.has("background"));
        assert!(styles.has("width"));
    }
    
    #[test]
    fn test_get_property() {
        let css = r#"
            button {
                background: #ff0000;
            }
        "#;
        
        let parser = ThemeParser::new();
        let sheet = parser.parse_str(css).unwrap();
        let engine = ThemeEngine::new(sheet);
        
        let bg = engine.get_property("button", &[], &[], "background").unwrap();
        assert!(bg.is_some());
        
        if let Some(PropertyValue::Color(color)) = bg {
            assert_eq!(color.r, 255);
        } else {
            panic!("Expected color");
        }
    }
    
    #[test]
    fn test_cascade() {
        let css = r#"
            button {
                background: #ff0000;
            }
            
            button.primary {
                background: #00ff00;
            }
        "#;
        
        let parser = ThemeParser::new();
        let sheet = parser.parse_str(css).unwrap();
        let engine = ThemeEngine::new(sheet);
        
        // Without class = red
        let bg1 = engine.get_property("button", &[], &[], "background").unwrap().unwrap();
        if let PropertyValue::Color(color) = bg1 {
            assert_eq!(color.r, 255);
            assert_eq!(color.g, 0);
        }
        
        // With class = green (more specific)
        let bg2 = engine.get_property("button", &vec!["primary".to_string()], &[], "background").unwrap().unwrap();
        if let PropertyValue::Color(color) = bg2 {
            assert_eq!(color.r, 0);
            assert_eq!(color.g, 255);
        }
    }
}
