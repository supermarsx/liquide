//! Attribute storage for DOM nodes.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Attribute map optimized for the common case of few attributes.
///
/// Most DOM nodes in a desktop environment have 0–4 attributes.
/// We use a small inline vec for ≤8 attributes and spill to a HashMap.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AttributeMap {
    /// For small counts we use a flat vec of (key, value) pairs.
    /// When the count exceeds `INLINE_THRESHOLD`, we migrate to the map.
    entries: Vec<(String, String)>,
    /// Overflow map for nodes with many attributes.
    #[serde(skip_serializing_if = "Option::is_none")]
    overflow: Option<HashMap<String, String>>,
}

const INLINE_THRESHOLD: usize = 8;

impl AttributeMap {
    /// Create an empty attribute map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set an attribute. Overwrites if it already exists.
    pub fn set(&mut self, key: &str, value: &str) {
        // Check overflow map first
        if let Some(ref mut map) = self.overflow {
            map.insert(key.to_string(), value.to_string());
            return;
        }

        // Check inline entries
        for entry in &mut self.entries {
            if entry.0 == key {
                entry.1 = value.to_string();
                return;
            }
        }

        // Insert new
        if self.entries.len() < INLINE_THRESHOLD {
            self.entries.push((key.to_string(), value.to_string()));
        } else {
            // Migrate to HashMap
            let mut map: HashMap<String, String> = self.entries.drain(..).collect();
            map.insert(key.to_string(), value.to_string());
            self.overflow = Some(map);
        }
    }

    /// Get an attribute value.
    pub fn get(&self, key: &str) -> Option<&str> {
        if let Some(ref map) = self.overflow {
            return map.get(key).map(String::as_str);
        }
        for entry in &self.entries {
            if entry.0 == key {
                return Some(&entry.1);
            }
        }
        None
    }

    /// Remove an attribute. Returns the previous value if it existed.
    pub fn remove(&mut self, key: &str) -> Option<String> {
        if let Some(ref mut map) = self.overflow {
            return map.remove(key);
        }
        if let Some(pos) = self.entries.iter().position(|e| e.0 == key) {
            return Some(self.entries.swap_remove(pos).1);
        }
        None
    }

    /// Check if an attribute exists.
    pub fn contains(&self, key: &str) -> bool {
        if let Some(ref map) = self.overflow {
            return map.contains_key(key);
        }
        self.entries.iter().any(|e| e.0 == key)
    }

    /// Number of attributes.
    pub fn len(&self) -> usize {
        if let Some(ref map) = self.overflow {
            return map.len();
        }
        self.entries.len()
    }

    /// Is the map empty?
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Iterate over all (key, value) pairs.
    pub fn iter(&self) -> AttributeIter<'_> {
        AttributeIter {
            inline: self.entries.iter(),
            overflow: self.overflow.as_ref().map(|m| m.iter()),
        }
    }
}

/// Iterator over attribute (key, value) pairs.
pub struct AttributeIter<'a> {
    inline: std::slice::Iter<'a, (String, String)>,
    overflow: Option<std::collections::hash_map::Iter<'a, String, String>>,
}

impl<'a> Iterator for AttributeIter<'a> {
    type Item = (&'a str, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(ref mut ov) = self.overflow {
            return ov.next().map(|(k, v)| (k.as_str(), v.as_str()));
        }
        self.inline
            .next()
            .map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_set_get() {
        let mut attrs = AttributeMap::new();
        attrs.set("role", "button");
        assert_eq!(attrs.get("role"), Some("button"));
        assert_eq!(attrs.len(), 1);
    }

    #[test]
    fn overwrite() {
        let mut attrs = AttributeMap::new();
        attrs.set("title", "old");
        attrs.set("title", "new");
        assert_eq!(attrs.get("title"), Some("new"));
        assert_eq!(attrs.len(), 1);
    }

    #[test]
    fn remove_returns_value() {
        let mut attrs = AttributeMap::new();
        attrs.set("x", "1");
        assert_eq!(attrs.remove("x"), Some("1".to_string()));
        assert!(attrs.is_empty());
    }

    #[test]
    fn overflow_to_hashmap() {
        let mut attrs = AttributeMap::new();
        for i in 0..=INLINE_THRESHOLD {
            attrs.set(&format!("key{}", i), &format!("val{}", i));
        }
        assert_eq!(attrs.len(), INLINE_THRESHOLD + 1);
        assert_eq!(attrs.get("key0"), Some("val0"));
        assert_eq!(
            attrs.get(&format!("key{}", INLINE_THRESHOLD)),
            Some(format!("val{}", INLINE_THRESHOLD).as_str())
        );
    }
}
