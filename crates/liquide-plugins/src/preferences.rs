use std::collections::HashMap;

/// Per-plugin key-value preferences store.
#[derive(Debug, Clone)]
pub struct PluginPreferences {
    values: HashMap<String, String>,
}

impl PluginPreferences {
    /// Create an empty preferences store.
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    /// Get the value for a key, if present.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(|s| s.as_str())
    }

    /// Set a key-value pair. Overwrites any existing value.
    pub fn set(&mut self, key: &str, value: String) {
        self.values.insert(key.to_string(), value);
    }

    /// Remove a key-value pair. Returns `true` if the key was present.
    pub fn remove(&mut self, key: &str) -> bool {
        self.values.remove(key).is_some()
    }

    /// Return all keys in arbitrary order.
    pub fn keys(&self) -> Vec<&str> {
        self.values.keys().map(|s| s.as_str()).collect()
    }

    /// Return the number of stored preferences.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Check whether the preferences store is empty.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Serialize preferences to a simple "key=value\n" text format.
    pub fn serialize(&self) -> String {
        let mut lines: Vec<String> = self
            .values
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();
        lines.sort(); // deterministic output
        lines.join("\n")
    }

    /// Deserialize preferences from the "key=value\n" text format.
    pub fn deserialize(s: &str) -> Self {
        let mut values = HashMap::new();
        for line in s.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                values.insert(key.to_string(), value.to_string());
            }
        }
        Self { values }
    }
}

impl Default for PluginPreferences {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_is_empty() {
        let prefs = PluginPreferences::new();
        assert!(prefs.is_empty());
        assert_eq!(prefs.len(), 0);
    }

    #[test]
    fn set_and_get() {
        let mut prefs = PluginPreferences::new();
        prefs.set("color", "blue".into());
        assert_eq!(prefs.get("color"), Some("blue"));
    }

    #[test]
    fn get_missing_key() {
        let prefs = PluginPreferences::new();
        assert_eq!(prefs.get("nonexistent"), None);
    }

    #[test]
    fn set_overwrites() {
        let mut prefs = PluginPreferences::new();
        prefs.set("size", "10".into());
        prefs.set("size", "20".into());
        assert_eq!(prefs.get("size"), Some("20"));
        assert_eq!(prefs.len(), 1);
    }

    #[test]
    fn remove_existing() {
        let mut prefs = PluginPreferences::new();
        prefs.set("key", "val".into());
        assert!(prefs.remove("key"));
        assert_eq!(prefs.get("key"), None);
        assert!(prefs.is_empty());
    }

    #[test]
    fn remove_nonexistent() {
        let mut prefs = PluginPreferences::new();
        assert!(!prefs.remove("ghost"));
    }

    #[test]
    fn keys_returns_all() {
        let mut prefs = PluginPreferences::new();
        prefs.set("a", "1".into());
        prefs.set("b", "2".into());
        prefs.set("c", "3".into());
        let mut keys = prefs.keys();
        keys.sort();
        assert_eq!(keys, vec!["a", "b", "c"]);
    }

    #[test]
    fn serialize_empty() {
        let prefs = PluginPreferences::new();
        assert_eq!(prefs.serialize(), "");
    }

    #[test]
    fn serialize_single() {
        let mut prefs = PluginPreferences::new();
        prefs.set("theme", "dark".into());
        assert_eq!(prefs.serialize(), "theme=dark");
    }

    #[test]
    fn serialize_multiple_sorted() {
        let mut prefs = PluginPreferences::new();
        prefs.set("z_last", "1".into());
        prefs.set("a_first", "2".into());
        prefs.set("m_mid", "3".into());
        let serialized = prefs.serialize();
        assert_eq!(serialized, "a_first=2\nm_mid=3\nz_last=1");
    }

    #[test]
    fn deserialize_empty() {
        let prefs = PluginPreferences::deserialize("");
        assert!(prefs.is_empty());
    }

    #[test]
    fn deserialize_single() {
        let prefs = PluginPreferences::deserialize("theme=dark");
        assert_eq!(prefs.get("theme"), Some("dark"));
        assert_eq!(prefs.len(), 1);
    }

    #[test]
    fn deserialize_multiple() {
        let prefs = PluginPreferences::deserialize("a=1\nb=2\nc=3");
        assert_eq!(prefs.get("a"), Some("1"));
        assert_eq!(prefs.get("b"), Some("2"));
        assert_eq!(prefs.get("c"), Some("3"));
        assert_eq!(prefs.len(), 3);
    }

    #[test]
    fn deserialize_ignores_comments() {
        let prefs = PluginPreferences::deserialize("# comment\nkey=val\n# another");
        assert_eq!(prefs.get("key"), Some("val"));
        assert_eq!(prefs.len(), 1);
    }

    #[test]
    fn deserialize_ignores_blank_lines() {
        let prefs = PluginPreferences::deserialize("\n\na=1\n\nb=2\n\n");
        assert_eq!(prefs.len(), 2);
    }

    #[test]
    fn deserialize_ignores_malformed_lines() {
        let prefs = PluginPreferences::deserialize("good=yes\nno_equals_here\nalso_good=yep");
        assert_eq!(prefs.len(), 2);
        assert_eq!(prefs.get("good"), Some("yes"));
        assert_eq!(prefs.get("also_good"), Some("yep"));
    }

    #[test]
    fn deserialize_value_with_equals() {
        // Value itself contains '=' — split_once only splits on the first '='
        let prefs = PluginPreferences::deserialize("formula=a=b+c");
        assert_eq!(prefs.get("formula"), Some("a=b+c"));
    }

    #[test]
    fn round_trip() {
        let mut original = PluginPreferences::new();
        original.set("name", "Weather Widget".into());
        original.set("refresh_seconds", "300".into());
        original.set("location", "Berlin".into());

        let serialized = original.serialize();
        let restored = PluginPreferences::deserialize(&serialized);

        assert_eq!(restored.get("name"), Some("Weather Widget"));
        assert_eq!(restored.get("refresh_seconds"), Some("300"));
        assert_eq!(restored.get("location"), Some("Berlin"));
        assert_eq!(restored.len(), 3);
    }

    #[test]
    fn round_trip_empty() {
        let original = PluginPreferences::new();
        let serialized = original.serialize();
        let restored = PluginPreferences::deserialize(&serialized);
        assert!(restored.is_empty());
    }

    #[test]
    fn default_is_empty() {
        let prefs = PluginPreferences::default();
        assert!(prefs.is_empty());
    }
}
