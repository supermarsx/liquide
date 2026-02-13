//! CSS selector matching

use crate::error::{Result, ThemeError};
use serde::{Deserialize, Serialize};
use std::fmt;

/// CSS selector
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Selector {
    /// Element name (e.g., "window", "button")
    pub element: String,
    
    /// Class names
    pub classes: Vec<String>,
    
    /// ID
    pub id: Option<String>,
    
    /// Pseudo-classes (e.g., "hover", "focus", "active")
    pub pseudo_classes: Vec<String>,
    
    /// Pseudo-elements (e.g., "::before", "::after")
    pub pseudo_element: Option<String>,
}

impl Selector {
    /// Create a simple element selector
    pub fn element(name: &str) -> Self {
        Self {
            element: name.to_string(),
            classes: Vec::new(),
            id: None,
            pseudo_classes: Vec::new(),
            pseudo_element: None,
        }
    }
    
    /// Add a class
    pub fn with_class(mut self, class: &str) -> Self {
        self.classes.push(class.to_string());
        self
    }
    
    /// Add an ID
    pub fn with_id(mut self, id: &str) -> Self {
        self.id = Some(id.to_string());
        self
    }
    
    /// Add a pseudo-class
    pub fn with_pseudo_class(mut self, pseudo: &str) -> Self {
        self.pseudo_classes.push(pseudo.to_string());
        self
    }
    
    /// Parse from CSS selector string
    pub fn parse(s: &str) -> Result<Self> {
        let s = s.trim();
        
        // Extract element name
        let mut element = String::new();
        let mut chars = s.chars().peekable();
        
        // Read element name
        while let Some(&ch) = chars.peek() {
            if ch.is_alphanumeric() || ch == '-' || ch == '_' {
                element.push(ch);
                chars.next();
            } else {
                break;
            }
        }
        
        if element.is_empty() {
            return Err(ThemeError::InvalidSelector(s.to_string()));
        }
        
        let mut selector = Selector::element(&element);
        
        // Parse modifiers
        while let Some(ch) = chars.next() {
            match ch {
                '.' => {
                    // Class
                    let mut class = String::new();
                    while let Some(&ch) = chars.peek() {
                        if ch.is_alphanumeric() || ch == '-' || ch == '_' {
                            class.push(ch);
                            chars.next();
                        } else {
                            break;
                        }
                    }
                    if !class.is_empty() {
                        selector.classes.push(class);
                    }
                }
                '#' => {
                    // ID
                    let mut id = String::new();
                    while let Some(&ch) = chars.peek() {
                        if ch.is_alphanumeric() || ch == '-' || ch == '_' {
                            id.push(ch);
                            chars.next();
                        } else {
                            break;
                        }
                    }
                    if !id.is_empty() {
                        selector.id = Some(id);
                    }
                }
                ':' => {
                    // Pseudo-class or pseudo-element
                    if chars.peek() == Some(&':') {
                        // Pseudo-element
                        chars.next();
                        let mut pseudo = String::new();
                        while let Some(&ch) = chars.peek() {
                            if ch.is_alphanumeric() || ch == '-' {
                                pseudo.push(ch);
                                chars.next();
                            } else {
                                break;
                            }
                        }
                        if !pseudo.is_empty() {
                            selector.pseudo_element = Some(pseudo);
                        }
                    } else {
                        // Pseudo-class
                        let mut pseudo = String::new();
                        while let Some(&ch) = chars.peek() {
                            if ch.is_alphanumeric() || ch == '-' {
                                pseudo.push(ch);
                                chars.next();
                            } else {
                                break;
                            }
                        }
                        if !pseudo.is_empty() {
                            selector.pseudo_classes.push(pseudo);
                        }
                    }
                }
                _ if ch.is_whitespace() => {
                    // Ignore whitespace
                    continue;
                }
                _ => {
                    return Err(ThemeError::InvalidSelector(format!(
                        "Unexpected character '{}' in selector '{}'",
                        ch, s
                    )));
                }
            }
        }
        
        Ok(selector)
    }
    
    /// Check if this selector matches an element with given properties
    pub fn matches(
        &self,
        element: &str,
        classes: &[String],
        id: Option<&str>,
        pseudo_classes: &[String],
    ) -> bool {
        // Check element
        if self.element != "*" && self.element != element {
            return false;
        }
        
        // Check ID
        if let Some(ref sel_id) = self.id {
            if id != Some(sel_id.as_str()) {
                return false;
            }
        }
        
        // Check classes (all must match)
        for class in &self.classes {
            if !classes.contains(class) {
                return false;
            }
        }
        
        // Check pseudo-classes (all must match)
        for pseudo in &self.pseudo_classes {
            if !pseudo_classes.contains(pseudo) {
                return false;
            }
        }
        
        true
    }
    
    /// Get specificity (for CSS cascade)
    /// Returns (id_count, class_count, element_count)
    pub fn specificity(&self) -> (u32, u32, u32) {
        let id_count = if self.id.is_some() { 1 } else { 0 };
        let class_count = (self.classes.len() + self.pseudo_classes.len()) as u32;
        let element_count = if self.element != "*" { 1 } else { 0 };
        
        (id_count, class_count, element_count)
    }
}

impl fmt::Display for Selector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.element)?;
        
        if let Some(ref id) = self.id {
            write!(f, "#{}", id)?;
        }
        
        for class in &self.classes {
            write!(f, ".{}", class)?;
        }
        
        for pseudo in &self.pseudo_classes {
            write!(f, ":{}", pseudo)?;
        }
        
        if let Some(ref pseudo_el) = self.pseudo_element {
            write!(f, "::{}", pseudo_el)?;
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_simple() {
        let sel = Selector::parse("button").unwrap();
        assert_eq!(sel.element, "button");
        assert!(sel.classes.is_empty());
    }
    
    #[test]
    fn test_parse_with_class() {
        let sel = Selector::parse("button.primary").unwrap();
        assert_eq!(sel.element, "button");
        assert_eq!(sel.classes, vec!["primary"]);
    }
    
    #[test]
    fn test_parse_with_id() {
        let sel = Selector::parse("window#main").unwrap();
        assert_eq!(sel.element, "window");
        assert_eq!(sel.id, Some("main".to_string()));
    }
    
    #[test]
    fn test_parse_complex() {
        let sel = Selector::parse("button.primary.large:hover").unwrap();
        assert_eq!(sel.element, "button");
        assert_eq!(sel.classes, vec!["primary", "large"]);
        assert_eq!(sel.pseudo_classes, vec!["hover"]);
    }
    
    #[test]
    fn test_matches() {
        let sel = Selector::parse("button.primary:hover").unwrap();
        
        assert!(sel.matches(
            "button",
            &vec!["primary".to_string()],
            None,
            &vec!["hover".to_string()],
        ));
        
        assert!(!sel.matches(
            "button",
            &vec!["secondary".to_string()],
            None,
            &vec!["hover".to_string()],
        ));
    }
    
    #[test]
    fn test_specificity() {
        let sel1 = Selector::parse("button").unwrap();
        assert_eq!(sel1.specificity(), (0, 0, 1));
        
        let sel2 = Selector::parse("button.primary").unwrap();
        assert_eq!(sel2.specificity(), (0, 1, 1));
        
        let sel3 = Selector::parse("#main").unwrap();
        assert_eq!(sel3.specificity(), (1, 0, 0));
    }
}
