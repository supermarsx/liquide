//! Style property management

use crate::value::PropertyValue;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A set of CSS properties
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PropertySet {
    properties: HashMap<String, PropertyValue>,
}

impl PropertySet {
    /// Create a new empty property set
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Insert a property
    pub fn insert(&mut self, name: String, value: PropertyValue) {
        self.properties.insert(name, value);
    }
    
    /// Get a property value
    pub fn get(&self, name: &str) -> Option<&PropertyValue> {
        self.properties.get(name)
    }
    
    /// Check if property exists
    pub fn has(&self, name: &str) -> bool {
        self.properties.contains_key(name)
    }
    
    /// Remove a property
    pub fn remove(&mut self, name: &str) -> Option<PropertyValue> {
        self.properties.remove(name)
    }
    
    /// Get all property names
    pub fn keys(&self) -> Vec<&String> {
        self.properties.keys().collect()
    }
    
    /// Merge another property set (other takes precedence)
    pub fn merge(&mut self, other: &PropertySet) {
        for (key, value) in &other.properties {
            self.properties.insert(key.clone(), value.clone());
        }
    }
    
    /// Iterate over properties
    pub fn iter(&self) -> impl Iterator<Item = (&String, &PropertyValue)> {
        self.properties.iter()
    }
}

impl From<HashMap<String, PropertyValue>> for PropertySet {
    fn from(map: HashMap<String, PropertyValue>) -> Self {
        Self { properties: map }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Color;
    
    #[test]
    fn test_property_set() {
        let mut set = PropertySet::new();
        
        set.insert(
            "background".to_string(),
            PropertyValue::Color(Color::rgb(255, 0, 0)),
        );
        
        assert!(set.has("background"));
        assert!(set.get("background").is_some());
        
        let removed = set.remove("background");
        assert!(removed.is_some());
        assert!(!set.has("background"));
    }
    
    #[test]
    fn test_merge() {
        let mut set1 = PropertySet::new();
        set1.insert("color".to_string(), PropertyValue::Color(Color::rgb(255, 0, 0)));
        
        let mut set2 = PropertySet::new();
        set2.insert("color".to_string(), PropertyValue::Color(Color::rgb(0, 255, 0)));
        set2.insert("background".to_string(), PropertyValue::Color(Color::rgb(0, 0, 255)));
        
        set1.merge(&set2);
        
        // set2's color should override set1's
        let color = set1.get("color").unwrap().as_color().unwrap();
        assert_eq!(color.g, 255);
        
        // background should be added
        assert!(set1.has("background"));
    }
}
