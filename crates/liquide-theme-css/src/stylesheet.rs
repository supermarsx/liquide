//! CSS stylesheet representation

use crate::property::PropertySet;
use crate::selector::Selector;
use crate::value::PropertyValue;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A parsed CSS stylesheet
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StyleSheet {
    /// Rules indexed by selector
    rules: Vec<StyleRule>,
    
    /// CSS variables (custom properties)
    variables: HashMap<String, PropertyValue>,
}

/// A single style rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleRule {
    /// Selector for this rule
    pub selector: Selector,
    
    /// Properties in this rule
    pub properties: PropertySet,
    
    /// Rule specificity (cached)
    specificity: (u32, u32, u32),
}

impl StyleRule {
    /// Create a new style rule
    pub fn new(selector: Selector, properties: PropertySet) -> Self {
        let specificity = selector.specificity();
        Self {
            selector,
            properties,
            specificity,
        }
    }
    
    /// Get specificity
    pub fn specificity(&self) -> (u32, u32, u32) {
        self.specificity
    }
}

impl StyleSheet {
    /// Create a new empty stylesheet
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Add a rule
    pub fn add_rule(&mut self, selector: Selector, properties: PropertySet) {
        self.rules.push(StyleRule::new(selector, properties));
    }
    
    /// Set a CSS variable
    pub fn set_variable(&mut self, name: String, value: PropertyValue) {
        self.variables.insert(name, value);
    }
    
    /// Get a CSS variable
    pub fn get_variable(&self, name: &str) -> Option<&PropertyValue> {
        self.variables.get(name)
    }
    
    /// Find matching rules for an element
    pub fn find_matching_rules(
        &self,
        element: &str,
        classes: &[String],
        id: Option<&str>,
        pseudo_classes: &[String],
    ) -> Vec<&StyleRule> {
        let mut matching = Vec::new();
        
        for rule in &self.rules {
            if rule.selector.matches(element, classes, id, pseudo_classes) {
                matching.push(rule);
            }
        }
        
        // Sort by specificity (higher specificity = higher priority)
        matching.sort_by(|a, b| b.specificity.cmp(&a.specificity));
        
        matching
    }
    
    /// Compute final styles for an element (cascade resolution)
    pub fn compute_styles(
        &self,
        element: &str,
        classes: &[String],
        id: Option<&str>,
        pseudo_classes: &[String],
    ) -> PropertySet {
        let matching = self.find_matching_rules(element, classes, id, pseudo_classes);
        
        let mut final_properties = PropertySet::new();
        
        // Apply rules in reverse specificity order (lowest first)
        for rule in matching.iter().rev() {
            final_properties.merge(&rule.properties);
        }
        
        final_properties
    }
    
    /// Get all rules
    pub fn rules(&self) -> &[StyleRule] {
        &self.rules
    }
    
    /// Get number of rules
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
    
    /// Merge another stylesheet
    pub fn merge(&mut self, other: &StyleSheet) {
        self.rules.extend(other.rules.clone());
        self.variables.extend(other.variables.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Color;
    
    #[test]
    fn test_stylesheet() {
        let mut sheet = StyleSheet::new();
        
        let selector = Selector::element("button");
        let mut properties = PropertySet::new();
        properties.insert(
            "background".to_string(),
            PropertyValue::Color(Color::rgb(255, 0, 0)),
        );
        
        sheet.add_rule(selector, properties);
        
        assert_eq!(sheet.rule_count(), 1);
    }
    
    #[test]
    fn test_cascade() {
        let mut sheet = StyleSheet::new();
        
        // Less specific rule
        let selector1 = Selector::element("button");
        let mut props1 = PropertySet::new();
        props1.insert("background".to_string(), PropertyValue::Color(Color::rgb(255, 0, 0)));
        sheet.add_rule(selector1, props1);
        
        // More specific rule
        let selector2 = Selector::element("button").with_class("primary");
        let mut props2 = PropertySet::new();
        props2.insert("background".to_string(), PropertyValue::Color(Color::rgb(0, 255, 0)));
        sheet.add_rule(selector2, props2);
        
        // Query with class
        let styles = sheet.compute_styles(
            "button",
            &vec!["primary".to_string()],
            None,
            &[],
        );
        
        // Should get green background (more specific)
        let color = styles.get("background").unwrap().as_color().unwrap();
        assert_eq!(color.g, 255);
    }
}
