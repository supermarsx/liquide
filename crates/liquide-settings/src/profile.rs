//! User preference profiles — named collections of settings that can be
//! saved, loaded, exported, imported, compared, and applied.

use crate::schema::SettingValue;
use std::collections::BTreeMap;
use std::fmt;

/// A named collection of setting key-value pairs representing a user profile.
#[derive(Debug, Clone)]
pub struct UserProfile {
    /// Profile name.
    pub name: String,
    /// Optional description.
    pub description: String,
    /// Settings stored in this profile (key -> serialized value).
    settings: BTreeMap<String, SettingValue>,
}

impl UserProfile {
    /// Create a new empty profile.
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            settings: BTreeMap::new(),
        }
    }

    /// Set a value in the profile.
    pub fn set(&mut self, key: &str, value: SettingValue) {
        self.settings.insert(key.to_string(), value);
    }

    /// Get a value from the profile.
    pub fn get(&self, key: &str) -> Option<&SettingValue> {
        self.settings.get(key)
    }

    /// Remove a key from the profile.
    pub fn remove(&mut self, key: &str) -> Option<SettingValue> {
        self.settings.remove(key)
    }

    /// Return all keys in the profile (sorted).
    pub fn keys(&self) -> Vec<&str> {
        self.settings.keys().map(|k| k.as_str()).collect()
    }

    /// Return the number of settings in this profile.
    pub fn len(&self) -> usize {
        self.settings.len()
    }

    /// Check if the profile has no settings.
    pub fn is_empty(&self) -> bool {
        self.settings.is_empty()
    }

    /// Iterate over all (key, value) pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &SettingValue)> {
        self.settings.iter().map(|(k, v)| (k.as_str(), v))
    }
}

/// Storage for multiple user profiles with save/load/export/import/diff support.
pub struct ProfileStore {
    /// Profiles indexed by name.
    profiles: BTreeMap<String, UserProfile>,
}

impl ProfileStore {
    /// Create a new empty profile store.
    pub fn new() -> Self {
        Self {
            profiles: BTreeMap::new(),
        }
    }

    /// Save (insert or update) a profile.
    pub fn save(&mut self, profile: UserProfile) {
        self.profiles.insert(profile.name.clone(), profile);
    }

    /// Load a profile by name.
    pub fn load(&self, name: &str) -> Option<&UserProfile> {
        self.profiles.get(name)
    }

    /// Delete a profile by name. Returns true if it existed.
    pub fn delete(&mut self, name: &str) -> bool {
        self.profiles.remove(name).is_some()
    }

    /// List all profile names (sorted).
    pub fn list(&self) -> Vec<&str> {
        self.profiles.keys().map(|k| k.as_str()).collect()
    }

    /// Return the number of stored profiles.
    pub fn count(&self) -> usize {
        self.profiles.len()
    }

    /// Export a profile to a text format for sharing / backup.
    /// Format:
    /// ```text
    /// [profile]
    /// name=My Profile
    /// description=A custom profile
    /// [settings]
    /// key1=type:value1
    /// key2=type:value2
    /// ```
    pub fn export_profile(&self, name: &str) -> Option<String> {
        let profile = self.profiles.get(name)?;

        let mut out = String::new();
        out.push_str("[profile]\n");
        out.push_str(&format!("name={}\n", profile.name));
        out.push_str(&format!("description={}\n", profile.description));
        out.push_str("[settings]\n");
        for (key, value) in &profile.settings {
            out.push_str(&format!("{}={}\n", key, value.serialize()));
        }
        Some(out)
    }

    /// Import a profile from the text export format.
    /// Returns the imported profile's name on success.
    pub fn import_profile(&mut self, data: &str) -> Result<String, ProfileError> {
        let mut name: Option<String> = None;
        let mut description = String::new();
        let mut settings = BTreeMap::new();
        let mut in_settings = false;

        for line in data.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line == "[profile]" {
                in_settings = false;
                continue;
            }
            if line == "[settings]" {
                in_settings = true;
                continue;
            }

            if let Some((key, val)) = line.split_once('=') {
                let key = key.trim();
                let val = val.trim();
                if in_settings {
                    if let Some(sv) = SettingValue::deserialize(val) {
                        settings.insert(key.to_string(), sv);
                    }
                } else {
                    match key {
                        "name" => name = Some(val.to_string()),
                        "description" => description = val.to_string(),
                        _ => {}
                    }
                }
            }
        }

        let name = name.ok_or(ProfileError::MissingName)?;
        if name.is_empty() {
            return Err(ProfileError::MissingName);
        }

        let profile = UserProfile {
            name: name.clone(),
            description,
            settings,
        };
        self.profiles.insert(name.clone(), profile);
        Ok(name)
    }

    /// Compare two profiles and return a list of differences.
    /// Each difference is (key, value_in_a, value_in_b) where values are
    /// the Display representation (or "<absent>" if the key is missing).
    pub fn diff_profiles(
        &self,
        name_a: &str,
        name_b: &str,
    ) -> Result<Vec<(String, String, String)>, ProfileError> {
        let a = self
            .profiles
            .get(name_a)
            .ok_or(ProfileError::NotFound(name_a.to_string()))?;
        let b = self
            .profiles
            .get(name_b)
            .ok_or(ProfileError::NotFound(name_b.to_string()))?;

        diff_user_profiles(a, b)
    }
}

/// Compare two `UserProfile` references directly and return differences.
pub fn diff_user_profiles(
    a: &UserProfile,
    b: &UserProfile,
) -> Result<Vec<(String, String, String)>, ProfileError> {
    let mut diffs = Vec::new();

    // Keys in a
    for (key, val_a) in a.iter() {
        match b.get(key) {
            Some(val_b) => {
                if val_a != val_b {
                    diffs.push((key.to_string(), format!("{}", val_a), format!("{}", val_b)));
                }
            }
            None => {
                diffs.push((
                    key.to_string(),
                    format!("{}", val_a),
                    "<absent>".to_string(),
                ));
            }
        }
    }

    // Keys only in b
    for (key, val_b) in b.iter() {
        if a.get(key).is_none() {
            diffs.push((
                key.to_string(),
                "<absent>".to_string(),
                format!("{}", val_b),
            ));
        }
    }

    diffs.sort_by(|x, y| x.0.cmp(&y.0));
    Ok(diffs)
}

/// Errors from profile operations.
#[derive(Debug, Clone)]
pub enum ProfileError {
    /// Profile not found by name.
    NotFound(String),
    /// Import data is missing a profile name.
    MissingName,
}

impl fmt::Display for ProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(name) => write!(f, "profile not found: {}", name),
            Self::MissingName => write!(f, "profile data is missing a name"),
        }
    }
}

impl std::error::Error for ProfileError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_profile() -> UserProfile {
        let mut p = UserProfile::new("work", "Work environment");
        p.set("appearance.theme", SettingValue::String("night".into()));
        p.set("appearance.font_size", SettingValue::Int(16));
        p.set("desktop.show_icons", SettingValue::Bool(false));
        p
    }

    #[test]
    fn profile_new_empty() {
        let p = UserProfile::new("test", "A test profile");
        assert_eq!(p.name, "test");
        assert_eq!(p.description, "A test profile");
        assert!(p.is_empty());
        assert_eq!(p.len(), 0);
    }

    #[test]
    fn profile_set_get() {
        let mut p = UserProfile::new("test", "");
        p.set("key", SettingValue::Bool(true));
        assert_eq!(p.get("key"), Some(&SettingValue::Bool(true)));
        assert_eq!(p.get("missing"), None);
    }

    #[test]
    fn profile_remove() {
        let mut p = UserProfile::new("test", "");
        p.set("key", SettingValue::Int(42));
        assert!(p.remove("key").is_some());
        assert!(p.get("key").is_none());
        assert!(p.remove("key").is_none());
    }

    #[test]
    fn profile_keys_sorted() {
        let p = sample_profile();
        let keys = p.keys();
        assert_eq!(
            keys,
            vec![
                "appearance.font_size",
                "appearance.theme",
                "desktop.show_icons"
            ]
        );
    }

    #[test]
    fn profile_len() {
        let p = sample_profile();
        assert_eq!(p.len(), 3);
        assert!(!p.is_empty());
    }

    #[test]
    fn profile_iter() {
        let p = sample_profile();
        let pairs: Vec<_> = p.iter().collect();
        assert_eq!(pairs.len(), 3);
    }

    // ── ProfileStore tests ─────────────────────────────────────────

    #[test]
    fn store_save_load() {
        let mut store = ProfileStore::new();
        store.save(sample_profile());

        let loaded = store.load("work");
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().name, "work");
    }

    #[test]
    fn store_delete() {
        let mut store = ProfileStore::new();
        store.save(sample_profile());
        assert!(store.delete("work"));
        assert!(!store.delete("work"));
        assert!(store.load("work").is_none());
    }

    #[test]
    fn store_list() {
        let mut store = ProfileStore::new();
        store.save(UserProfile::new("b-profile", ""));
        store.save(UserProfile::new("a-profile", ""));
        store.save(UserProfile::new("c-profile", ""));

        let names = store.list();
        assert_eq!(names, vec!["a-profile", "b-profile", "c-profile"]);
    }

    #[test]
    fn store_count() {
        let mut store = ProfileStore::new();
        assert_eq!(store.count(), 0);
        store.save(sample_profile());
        assert_eq!(store.count(), 1);
    }

    #[test]
    fn export_import_roundtrip() {
        let mut store = ProfileStore::new();
        store.save(sample_profile());

        let exported = store.export_profile("work").unwrap();
        assert!(exported.contains("[profile]"));
        assert!(exported.contains("name=work"));
        assert!(exported.contains("[settings]"));

        let mut store2 = ProfileStore::new();
        let name = store2.import_profile(&exported).unwrap();
        assert_eq!(name, "work");

        let imported = store2.load("work").unwrap();
        assert_eq!(imported.name, "work");
        assert_eq!(imported.description, "Work environment");
        assert_eq!(
            imported.get("appearance.theme"),
            Some(&SettingValue::String("night".into()))
        );
        assert_eq!(
            imported.get("appearance.font_size"),
            Some(&SettingValue::Int(16))
        );
        assert_eq!(
            imported.get("desktop.show_icons"),
            Some(&SettingValue::Bool(false))
        );
    }

    #[test]
    fn import_missing_name_fails() {
        let mut store = ProfileStore::new();
        let data = "[profile]\ndescription=no name\n[settings]\n";
        assert!(store.import_profile(data).is_err());
    }

    #[test]
    fn import_empty_name_fails() {
        let mut store = ProfileStore::new();
        let data = "[profile]\nname=\ndescription=empty\n[settings]\n";
        assert!(store.import_profile(data).is_err());
    }

    #[test]
    fn diff_identical_profiles() {
        let mut store = ProfileStore::new();
        store.save(sample_profile());
        let mut p2 = sample_profile();
        p2.name = "work2".to_string();
        store.save(p2);

        let diffs = store.diff_profiles("work", "work2").unwrap();
        assert!(diffs.is_empty());
    }

    #[test]
    fn diff_different_values() {
        let mut store = ProfileStore::new();
        let mut p1 = UserProfile::new("a", "");
        p1.set("theme", SettingValue::String("night".into()));
        p1.set("size", SettingValue::Int(14));

        let mut p2 = UserProfile::new("b", "");
        p2.set("theme", SettingValue::String("midday".into()));
        p2.set("size", SettingValue::Int(14));

        store.save(p1);
        store.save(p2);

        let diffs = store.diff_profiles("a", "b").unwrap();
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].0, "theme");
        assert_eq!(diffs[0].1, "night");
        assert_eq!(diffs[0].2, "midday");
    }

    #[test]
    fn diff_absent_keys() {
        let mut store = ProfileStore::new();
        let mut p1 = UserProfile::new("a", "");
        p1.set("only-in-a", SettingValue::Bool(true));
        p1.set("shared", SettingValue::Int(1));

        let mut p2 = UserProfile::new("b", "");
        p2.set("only-in-b", SettingValue::String("hello".into()));
        p2.set("shared", SettingValue::Int(1));

        store.save(p1);
        store.save(p2);

        let diffs = store.diff_profiles("a", "b").unwrap();
        assert_eq!(diffs.len(), 2);

        // Sorted by key
        assert_eq!(diffs[0].0, "only-in-a");
        assert_eq!(diffs[0].2, "<absent>");

        assert_eq!(diffs[1].0, "only-in-b");
        assert_eq!(diffs[1].1, "<absent>");
    }

    #[test]
    fn diff_nonexistent_profile_fails() {
        let mut store = ProfileStore::new();
        store.save(sample_profile());
        assert!(store.diff_profiles("work", "nonexistent").is_err());
        assert!(store.diff_profiles("nonexistent", "work").is_err());
    }

    #[test]
    fn export_nonexistent_returns_none() {
        let store = ProfileStore::new();
        assert!(store.export_profile("nonexistent").is_none());
    }

    #[test]
    fn profile_error_display() {
        let err = ProfileError::NotFound("missing".into());
        assert!(format!("{}", err).contains("missing"));

        let err = ProfileError::MissingName;
        assert!(format!("{}", err).contains("name"));
    }

    #[test]
    fn diff_user_profiles_direct() {
        let mut a = UserProfile::new("a", "");
        a.set("x", SettingValue::Int(1));
        a.set("y", SettingValue::Int(2));

        let mut b = UserProfile::new("b", "");
        b.set("x", SettingValue::Int(1));
        b.set("y", SettingValue::Int(3));
        b.set("z", SettingValue::Int(4));

        let diffs = diff_user_profiles(&a, &b).unwrap();
        assert_eq!(diffs.len(), 2); // y differs, z absent in a
    }

    #[test]
    fn import_with_comments_and_blanks() {
        let mut store = ProfileStore::new();
        let data = "# A profile export\n\n[profile]\nname=test\ndescription=Test\n\n# Settings follow\n[settings]\nkey1=bool:true\nkey2=int:42\n";
        let name = store.import_profile(data).unwrap();
        assert_eq!(name, "test");
        let profile = store.load("test").unwrap();
        assert_eq!(profile.len(), 2);
    }
}
