//! Style property management

use crate::value::PropertyValue;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::HashSet;

/// A set of CSS properties
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PropertySet {
    properties: HashMap<String, PropertyValue>,
    /// Properties that were declared with `!important`.
    #[serde(default)]
    important_keys: HashSet<String>,
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

    /// Mark a property as `!important`.
    pub fn mark_important(&mut self, name: &str) {
        self.important_keys.insert(name.to_string());
    }

    /// Check whether a property was declared with `!important`.
    pub fn is_important(&self, name: &str) -> bool {
        self.important_keys.contains(name)
    }

    /// Merge another property set (other takes precedence)
    pub fn merge(&mut self, other: &PropertySet) {
        for (key, value) in &other.properties {
            self.properties.insert(key.clone(), value.clone());
            if other.is_important(key) {
                self.important_keys.insert(key.clone());
            } else {
                self.important_keys.remove(key);
            }
        }
    }

    /// Iterate over properties
    pub fn iter(&self) -> impl Iterator<Item = (&String, &PropertyValue)> {
        self.properties.iter()
    }
}

impl From<HashMap<String, PropertyValue>> for PropertySet {
    fn from(map: HashMap<String, PropertyValue>) -> Self {
        Self {
            properties: map,
            important_keys: HashSet::new(),
        }
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
        set1.insert(
            "color".to_string(),
            PropertyValue::Color(Color::rgb(255, 0, 0)),
        );

        let mut set2 = PropertySet::new();
        set2.insert(
            "color".to_string(),
            PropertyValue::Color(Color::rgb(0, 255, 0)),
        );
        set2.insert(
            "background".to_string(),
            PropertyValue::Color(Color::rgb(0, 0, 255)),
        );

        set1.merge(&set2);

        // set2's color should override set1's
        let color = set1.get("color").unwrap().as_color().unwrap();
        assert_eq!(color.g, 255);

        // background should be added
        assert!(set1.has("background"));
    }

    #[test]
    fn test_merge_clears_stale_important_flags() {
        let mut set1 = PropertySet::new();
        set1.insert(
            "color".to_string(),
            PropertyValue::Color(Color::rgb(255, 0, 0)),
        );
        set1.mark_important("color");

        let mut set2 = PropertySet::new();
        set2.insert(
            "color".to_string(),
            PropertyValue::Color(Color::rgb(0, 255, 0)),
        );

        set1.merge(&set2);

        assert!(!set1.is_important("color"));
    }

    #[test]
    fn test_merge_preserves_new_important_flags() {
        let mut set1 = PropertySet::new();
        set1.insert(
            "color".to_string(),
            PropertyValue::Color(Color::rgb(255, 0, 0)),
        );

        let mut set2 = PropertySet::new();
        set2.insert(
            "color".to_string(),
            PropertyValue::Color(Color::rgb(0, 255, 0)),
        );
        set2.mark_important("color");

        set1.merge(&set2);

        assert!(set1.is_important("color"));
    }
}
