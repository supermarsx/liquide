use crate::store::{SettingsError, SettingsStore};
use crate::value::SettingValue;
use std::collections::BTreeMap;
use std::path::Path;

/// Persistence layer for settings — saves/loads TOML files with overrides only.
pub struct SettingsPersistence;

impl SettingsPersistence {
    /// Save the store's overrides to a TOML file at `path`.
    /// Only non-default values are written.
    pub fn save(store: &SettingsStore, path: &Path) -> Result<(), SettingsError> {
        let content = Self::save_toml_string(store);

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| SettingsError::IoError(e.to_string()))?;
        }

        std::fs::write(path, content).map_err(|e| SettingsError::IoError(e.to_string()))
    }

    /// Load overrides from a TOML file into the store.
    /// Missing file is not an error (uses defaults).
    pub fn load(store: &mut SettingsStore, path: &Path) -> Result<(), SettingsError> {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(SettingsError::IoError(e.to_string())),
        };
        Self::load_from_toml_string(store, &content)
    }

    /// Serialize overrides to a TOML string. Categories become TOML sections.
    pub fn save_toml_string(store: &SettingsStore) -> String {
        let overrides = store.export_overrides();
        if overrides.is_empty() {
            return "# LiquiDE Settings\n# (all defaults)\n".to_string();
        }

        // Group by category (first dotted segment)
        let mut sections: BTreeMap<String, BTreeMap<String, SettingValue>> = BTreeMap::new();
        for (key, val) in &overrides {
            let (cat, rest) = key.split_once('.').unwrap_or(("misc", key));
            sections
                .entry(cat.to_string())
                .or_default()
                .insert(rest.to_string(), val.clone());
        }

        let mut out = String::from("# LiquiDE Settings\n\n");
        for (section, entries) in &sections {
            out.push_str(&format!("[{}]\n", section));
            for (key, val) in entries {
                out.push_str(&format!("{} = {}\n", key, value_to_toml(val)));
            }
            out.push('\n');
        }
        out
    }

    /// Load overrides from a TOML string.
    pub fn load_from_toml_string(
        store: &mut SettingsStore,
        toml_str: &str,
    ) -> Result<(), SettingsError> {
        let table: toml::Table =
            toml_str.parse().map_err(|e: toml::de::Error| SettingsError::IoError(e.to_string()))?;

        let mut overrides = std::collections::HashMap::new();

        for (section, value) in &table {
            match value {
                toml::Value::Table(entries) => {
                    for (key, val) in entries {
                        let full_key = format!("{}.{}", section, key);
                        if let Some(schema) = store.schema().get(&full_key) {
                            if let Some(sv) = toml_to_value(val, &schema.setting_type) {
                                overrides.insert(full_key, sv);
                            }
                        }
                    }
                }
                _ => {
                    // Top-level non-table entries: treat section as a flat key
                    if let Some(schema) = store.schema().get(section) {
                        if let Some(sv) = toml_to_value(value, &schema.setting_type) {
                            overrides.insert(section.clone(), sv);
                        }
                    }
                }
            }
        }

        store.import_overrides(overrides);
        Ok(())
    }
}

fn value_to_toml(val: &SettingValue) -> String {
    match val {
        SettingValue::Bool(b) => b.to_string(),
        SettingValue::Int(i) => i.to_string(),
        SettingValue::Float(f) => {
            let s = format!("{:.2}", f);
            // Ensure there's a dot for TOML float syntax
            if s.contains('.') { s } else { format!("{}.0", s) }
        }
        SettingValue::Str(s) | SettingValue::Path(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
        SettingValue::Color(r, g, b, a) => format!("\"#{:02x}{:02x}{:02x}{:02x}\"", r, g, b, a),
        SettingValue::KeyBinding(keys) => {
            let inner: Vec<String> = keys.iter().map(|k| format!("\"{}\"", k)).collect();
            format!("[{}]", inner.join(", "))
        }
        SettingValue::Null => "\"null\"".to_string(),
    }
}

fn toml_to_value(
    val: &toml::Value,
    typ: &crate::schema::SettingType,
) -> Option<SettingValue> {
    use crate::schema::SettingType;
    match typ {
        SettingType::Bool => val.as_bool().map(SettingValue::Bool),
        SettingType::Int => val.as_integer().map(SettingValue::Int),
        SettingType::Float => val.as_float().map(SettingValue::Float),
        SettingType::String | SettingType::Enum(_) => {
            val.as_str().map(|s| SettingValue::Str(s.to_string()))
        }
        SettingType::Path => val.as_str().map(|s| SettingValue::Path(s.to_string())),
        SettingType::IntRange(_, _) => val.as_integer().map(SettingValue::Int),
        SettingType::FloatRange(_, _) => {
            val.as_float()
                .or_else(|| val.as_integer().map(|i| i as f64))
                .map(SettingValue::Float)
        }
        SettingType::Color => {
            val.as_str().and_then(|s| parse_color_hex(s))
        }
        SettingType::KeyBinding => {
            val.as_array().map(|arr| {
                SettingValue::KeyBinding(
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect(),
                )
            })
        }
    }
}

fn parse_color_hex(s: &str) -> Option<SettingValue> {
    let hex = s.trim_start_matches('#');
    if hex.len() >= 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        let a = if hex.len() >= 8 {
            u8::from_str_radix(&hex[6..8], 16).unwrap_or(255)
        } else {
            255
        };
        Some(SettingValue::Color(r, g, b, a))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{SchemaRegistry, SettingSchema, SettingType};

    fn test_schema() -> SchemaRegistry {
        let mut reg = SchemaRegistry::new();
        reg.register(SettingSchema {
            key: "appearance.theme".into(),
            display_name: "Theme".into(),
            description: String::new(),
            setting_type: SettingType::Enum(vec![
                "liquid-glass".into(), "night".into(), "midday".into(),
            ]),
            default: SettingValue::Str("liquid-glass".into()),
            category: "appearance".into(),
            subcategory: String::new(),
        });
        reg.register(SettingSchema {
            key: "appearance.font-size".into(),
            display_name: "Font Size".into(),
            description: String::new(),
            setting_type: SettingType::FloatRange(8.0, 32.0),
            default: SettingValue::Float(14.0),
            category: "appearance".into(),
            subcategory: String::new(),
        });
        reg.register(SettingSchema {
            key: "appearance.accent-color".into(),
            display_name: "Accent Color".into(),
            description: String::new(),
            setting_type: SettingType::Color,
            default: SettingValue::Color(0, 122, 255, 255),
            category: "appearance".into(),
            subcategory: String::new(),
        });
        reg.register(SettingSchema {
            key: "desktop.show-icons".into(),
            display_name: "Show Desktop Icons".into(),
            description: String::new(),
            setting_type: SettingType::Bool,
            default: SettingValue::Bool(true),
            category: "desktop".into(),
            subcategory: String::new(),
        });
        reg.register(SettingSchema {
            key: "input.double-click-ms".into(),
            display_name: "Double Click Time".into(),
            description: String::new(),
            setting_type: SettingType::IntRange(200, 1000),
            default: SettingValue::Int(400),
            category: "input".into(),
            subcategory: String::new(),
        });
        reg.register(SettingSchema {
            key: "input.hotkey".into(),
            display_name: "Global Hotkey".into(),
            description: String::new(),
            setting_type: SettingType::KeyBinding,
            default: SettingValue::KeyBinding(vec!["Ctrl".into(), "Space".into()]),
            category: "input".into(),
            subcategory: String::new(),
        });
        reg.register(SettingSchema {
            key: "desktop.wallpaper-path".into(),
            display_name: "Wallpaper Path".into(),
            description: String::new(),
            setting_type: SettingType::Path,
            default: SettingValue::Path(String::new()),
            category: "desktop".into(),
            subcategory: String::new(),
        });
        reg
    }

    #[test]
    fn save_toml_string_empty_overrides() {
        let store = SettingsStore::new(test_schema());
        let toml = SettingsPersistence::save_toml_string(&store);
        assert!(toml.contains("all defaults"));
    }

    #[test]
    fn save_toml_string_with_overrides() {
        let mut store = SettingsStore::new(test_schema());
        store.set("appearance.theme", SettingValue::Str("night".into())).unwrap();
        store.set("desktop.show-icons", SettingValue::Bool(false)).unwrap();

        let toml = SettingsPersistence::save_toml_string(&store);
        assert!(toml.contains("[appearance]"));
        assert!(toml.contains("theme = \"night\""));
        assert!(toml.contains("[desktop]"));
        assert!(toml.contains("show-icons = false"));
    }

    #[test]
    fn roundtrip_toml_string() {
        let mut store = SettingsStore::new(test_schema());
        store.set("appearance.theme", SettingValue::Str("night".into())).unwrap();
        store.set("appearance.font-size", SettingValue::Float(18.0)).unwrap();
        store.set("desktop.show-icons", SettingValue::Bool(false)).unwrap();
        store.set("input.double-click-ms", SettingValue::Int(300)).unwrap();

        let toml = SettingsPersistence::save_toml_string(&store);

        let mut store2 = SettingsStore::new(test_schema());
        SettingsPersistence::load_from_toml_string(&mut store2, &toml).unwrap();

        assert_eq!(store2.get("appearance.theme"), SettingValue::Str("night".into()));
        assert_eq!(store2.get("appearance.font-size"), SettingValue::Float(18.0));
        assert_eq!(store2.get("desktop.show-icons"), SettingValue::Bool(false));
        assert_eq!(store2.get("input.double-click-ms"), SettingValue::Int(300));
    }

    #[test]
    fn roundtrip_color() {
        let mut store = SettingsStore::new(test_schema());
        store.set("appearance.accent-color", SettingValue::Color(255, 0, 128, 200)).unwrap();

        let toml = SettingsPersistence::save_toml_string(&store);
        assert!(toml.contains("#ff0080c8"));

        let mut store2 = SettingsStore::new(test_schema());
        SettingsPersistence::load_from_toml_string(&mut store2, &toml).unwrap();
        assert_eq!(store2.get("appearance.accent-color"), SettingValue::Color(255, 0, 128, 200));
    }

    #[test]
    fn roundtrip_keybinding() {
        let mut store = SettingsStore::new(test_schema());
        store.set("input.hotkey", SettingValue::KeyBinding(vec!["Alt".into(), "F2".into()])).unwrap();

        let toml = SettingsPersistence::save_toml_string(&store);
        assert!(toml.contains("hotkey = [\"Alt\", \"F2\"]"));

        let mut store2 = SettingsStore::new(test_schema());
        SettingsPersistence::load_from_toml_string(&mut store2, &toml).unwrap();
        assert_eq!(
            store2.get("input.hotkey"),
            SettingValue::KeyBinding(vec!["Alt".into(), "F2".into()])
        );
    }

    #[test]
    fn roundtrip_path() {
        let mut store = SettingsStore::new(test_schema());
        store.set("desktop.wallpaper-path", SettingValue::Path("/home/user/bg.png".into())).unwrap();

        let toml = SettingsPersistence::save_toml_string(&store);

        let mut store2 = SettingsStore::new(test_schema());
        SettingsPersistence::load_from_toml_string(&mut store2, &toml).unwrap();
        assert_eq!(
            store2.get("desktop.wallpaper-path"),
            SettingValue::Path("/home/user/bg.png".into())
        );
    }

    #[test]
    fn save_load_file_roundtrip() {
        let dir = std::env::temp_dir().join("liquide_persist_test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("settings.toml");

        let mut store = SettingsStore::new(test_schema());
        store.set("appearance.theme", SettingValue::Str("midday".into())).unwrap();
        store.set("input.double-click-ms", SettingValue::Int(600)).unwrap();
        SettingsPersistence::save(&store, &path).unwrap();

        let mut store2 = SettingsStore::new(test_schema());
        SettingsPersistence::load(&mut store2, &path).unwrap();

        assert_eq!(store2.get("appearance.theme"), SettingValue::Str("midday".into()));
        assert_eq!(store2.get("input.double-click-ms"), SettingValue::Int(600));
        // Unchanged keys remain at defaults
        assert_eq!(store2.get("desktop.show-icons"), SettingValue::Bool(true));

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn load_nonexistent_file_is_ok() {
        let path = std::env::temp_dir().join("liquide_nonexistent_settings.toml");
        let _ = std::fs::remove_file(&path);

        let mut store = SettingsStore::new(test_schema());
        assert!(SettingsPersistence::load(&mut store, &path).is_ok());
    }

    #[test]
    fn load_invalid_toml_returns_error() {
        let mut store = SettingsStore::new(test_schema());
        let result = SettingsPersistence::load_from_toml_string(&mut store, "{{not valid toml}}");
        assert!(result.is_err());
    }

    #[test]
    fn load_unknown_keys_ignored() {
        let mut store = SettingsStore::new(test_schema());
        let toml_str = r#"
[unknown]
foo = "bar"

[appearance]
theme = "night"
unknown-key = "value"
"#;
        SettingsPersistence::load_from_toml_string(&mut store, toml_str).unwrap();
        assert_eq!(store.get("appearance.theme"), SettingValue::Str("night".into()));
    }

    #[test]
    fn load_invalid_value_skipped() {
        let mut store = SettingsStore::new(test_schema());
        let toml_str = r#"
[input]
double-click-ms = 99999
"#;
        // 99999 is out of range for IntRange(200, 1000), so import_overrides
        // validates and skips it
        SettingsPersistence::load_from_toml_string(&mut store, toml_str).unwrap();
        // Should still be default
        assert_eq!(store.get("input.double-click-ms"), SettingValue::Int(400));
    }

    #[test]
    fn only_overrides_are_saved() {
        let store = SettingsStore::new(test_schema());
        let toml = SettingsPersistence::save_toml_string(&store);
        // No sections should appear since nothing is overridden
        assert!(!toml.contains("[appearance]"));
        assert!(!toml.contains("[desktop]"));
    }
}
