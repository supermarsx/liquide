use crate::panels::*;
use crate::schema::*;
use crate::store::*;

// --- Schema tests ---

#[test]
fn category_all_returns_14_standard_categories() {
    let all = SettingCategory::all();
    assert_eq!(all.len(), 14);
    // First three in display order
    assert_eq!(all[0], SettingCategory::Appearance);
    assert_eq!(all[1], SettingCategory::Desktop);
    assert_eq!(all[2], SettingCategory::Display);
    // Last one
    assert_eq!(all[13], SettingCategory::About);
}

#[test]
fn category_labels_and_icons() {
    assert_eq!(SettingCategory::Appearance.label(), "Appearance");
    assert_eq!(SettingCategory::Privacy.label(), "Privacy & Security");
    assert_eq!(SettingCategory::Custom("Foo".into()).label(), "Foo");

    assert_eq!(
        SettingCategory::Appearance.icon(),
        "preferences-desktop-theme"
    );
    assert_eq!(
        SettingCategory::Custom("x".into()).icon(),
        "preferences-other"
    );
}

#[test]
fn setting_key_category_extraction() {
    let key = SettingKey::new("appearance.theme");
    assert_eq!(key.category(), "appearance");

    let key2 = SettingKey::new("wm.tiling_gap");
    assert_eq!(key2.category(), "wm");

    let key3 = SettingKey::new("standalone");
    assert_eq!(key3.category(), "standalone");
}

#[test]
fn setting_key_display() {
    let key = SettingKey::new("input.mouse_speed");
    assert_eq!(format!("{}", key), "input.mouse_speed");
}

#[test]
fn setting_value_accessors() {
    assert_eq!(SettingValue::Bool(true).as_bool(), Some(true));
    assert_eq!(SettingValue::Bool(false).as_bool(), Some(false));
    assert_eq!(SettingValue::Int(42).as_bool(), None);

    assert_eq!(SettingValue::Int(42).as_int(), Some(42));
    assert_eq!(SettingValue::Bool(true).as_int(), None);

    assert_eq!(SettingValue::Float(3.14).as_float(), Some(3.14));
    assert_eq!(SettingValue::Int(1).as_float(), None);

    assert_eq!(SettingValue::String("hello".into()).as_str(), Some("hello"));
    assert_eq!(SettingValue::Choice("night".into()).as_str(), Some("night"));
    assert_eq!(
        SettingValue::KeyBinding("Ctrl+A".into()).as_str(),
        Some("Ctrl+A")
    );
    assert_eq!(
        SettingValue::FilePath("/tmp/bg.png".into()).as_str(),
        Some("/tmp/bg.png")
    );
    assert_eq!(SettingValue::Bool(true).as_str(), None);
    assert_eq!(
        SettingValue::Color {
            r: 0,
            g: 0,
            b: 0,
            a: 255
        }
        .as_str(),
        None
    );
}

// --- Store tests ---

#[test]
fn store_register_defaults_populates_settings() {
    let store = SettingsStore::new();
    // Should have at least the settings we defined
    assert!(store.setting_count() >= 20);
}

#[test]
fn store_get_bool() {
    let store = SettingsStore::new();
    assert_eq!(store.get_bool("desktop.show_icons"), Some(true));
    assert_eq!(store.get_bool("input.natural_scroll"), Some(false));
    assert_eq!(store.get_bool("nonexistent.key"), None);
}

#[test]
fn store_get_int() {
    let store = SettingsStore::new();
    assert_eq!(store.get_int("appearance.font_size"), Some(14));
    assert_eq!(store.get_int("wm.tiling_gap"), Some(8));
    assert_eq!(store.get_int("input.key_repeat_delay"), Some(400));
}

#[test]
fn store_get_float() {
    let store = SettingsStore::new();
    assert_eq!(store.get_float("input.mouse_speed"), Some(1.0));
}

#[test]
fn store_get_string() {
    let store = SettingsStore::new();
    assert_eq!(store.get_string("appearance.theme"), Some("liquid_glass"));
    assert_eq!(store.get_string("wm.focus_policy"), Some("click"));
}

#[test]
fn store_set_valid_value() {
    let mut store = SettingsStore::new();
    let key = SettingKey::new("appearance.font_size");
    assert!(store.set(&key, SettingValue::Int(18)).is_ok());
    assert_eq!(store.get_int("appearance.font_size"), Some(18));
    assert!(store.is_dirty());
}

#[test]
fn store_set_out_of_range_fails() {
    let mut store = SettingsStore::new();
    let key = SettingKey::new("appearance.font_size");

    let result = store.set(&key, SettingValue::Int(100));
    assert!(result.is_err());
    match result.unwrap_err() {
        SettingsError::OutOfRange(_) => {}
        other => panic!("expected OutOfRange, got {:?}", other),
    }
    // Value should be unchanged
    assert_eq!(store.get_int("appearance.font_size"), Some(14));
}

#[test]
fn store_set_invalid_choice_fails() {
    let mut store = SettingsStore::new();
    let key = SettingKey::new("appearance.theme");
    let result = store.set(&key, SettingValue::Choice("nonexistent_theme".into()));
    assert!(result.is_err());
    match result.unwrap_err() {
        SettingsError::InvalidChoice(_) => {}
        other => panic!("expected InvalidChoice, got {:?}", other),
    }
}

#[test]
fn store_set_type_mismatch_fails() {
    let mut store = SettingsStore::new();
    let key = SettingKey::new("appearance.font_size");
    let result = store.set(&key, SettingValue::Bool(true));
    assert!(result.is_err());
    match result.unwrap_err() {
        SettingsError::TypeMismatch => {}
        other => panic!("expected TypeMismatch, got {:?}", other),
    }
}

#[test]
fn store_set_not_found_fails() {
    let mut store = SettingsStore::new();
    let key = SettingKey::new("nonexistent.setting");
    let result = store.set(&key, SettingValue::Bool(true));
    assert!(result.is_err());
    match result.unwrap_err() {
        SettingsError::NotFound(_) => {}
        other => panic!("expected NotFound, got {:?}", other),
    }
}

#[test]
fn store_reset_to_default() {
    let mut store = SettingsStore::new();
    let key = SettingKey::new("appearance.font_size");

    store.set(&key, SettingValue::Int(20)).unwrap();
    assert_eq!(store.get_int("appearance.font_size"), Some(20));

    store.reset(&key).unwrap();
    assert_eq!(store.get_int("appearance.font_size"), Some(14));
}

#[test]
fn store_reset_all() {
    let mut store = SettingsStore::new();

    store
        .set(
            &SettingKey::new("appearance.font_size"),
            SettingValue::Int(20),
        )
        .unwrap();
    store
        .set(
            &SettingKey::new("desktop.show_icons"),
            SettingValue::Bool(false),
        )
        .unwrap();
    store
        .set(&SettingKey::new("wm.tiling_gap"), SettingValue::Int(16))
        .unwrap();

    store.reset_all();

    assert_eq!(store.get_int("appearance.font_size"), Some(14));
    assert_eq!(store.get_bool("desktop.show_icons"), Some(true));
    assert_eq!(store.get_int("wm.tiling_gap"), Some(8));
}

#[test]
fn store_category_settings() {
    let store = SettingsStore::new();
    let appearance = store.category_settings(&SettingCategory::Appearance);
    assert_eq!(appearance.len(), 3); // theme, font_size, accent_color

    let wm = store.category_settings(&SettingCategory::WindowManagement);
    assert_eq!(wm.len(), 3); // focus_policy, tiling_gap, snap_enabled
}

#[test]
fn store_categories_returns_populated_categories() {
    let store = SettingsStore::new();
    let cats = store.categories();
    assert!(cats.contains(&SettingCategory::Appearance));
    assert!(cats.contains(&SettingCategory::Desktop));
    assert!(cats.contains(&SettingCategory::WindowManagement));
    assert!(cats.contains(&SettingCategory::Input));
    assert!(cats.contains(&SettingCategory::Display));
    assert!(cats.contains(&SettingCategory::Power));
    assert!(cats.contains(&SettingCategory::Notifications));
    assert!(cats.contains(&SettingCategory::Accessibility));
    assert!(cats.contains(&SettingCategory::Privacy));
    // Categories with no registered settings should not appear
    assert!(!cats.contains(&SettingCategory::Sound));
    assert!(!cats.contains(&SettingCategory::Network));
}

#[test]
fn store_slider_validation() {
    let mut store = SettingsStore::new();
    let key = SettingKey::new("input.mouse_speed");

    // Valid
    assert!(store.set(&key, SettingValue::Float(2.5)).is_ok());
    assert_eq!(store.get_float("input.mouse_speed"), Some(2.5));

    // Out of range (max is 3.0)
    let result = store.set(&key, SettingValue::Float(5.0));
    assert!(result.is_err());
}

#[test]
fn store_color_setting() {
    let store = SettingsStore::new();
    let key = SettingKey::new("appearance.accent_color");
    let val = store.get(&key);
    assert!(val.is_some());
    match val.unwrap() {
        SettingValue::Color { r, g, b, a } => {
            assert_eq!(*r, 0);
            assert_eq!(*g, 122);
            assert_eq!(*b, 255);
            assert_eq!(*a, 255);
        }
        other => panic!("expected Color, got {:?}", other),
    }
}

// --- Save/Load round-trip ---

#[test]
fn store_save_load_roundtrip() {
    let dir = std::env::temp_dir().join("liquide_settings_test");
    let _ = std::fs::create_dir_all(&dir);
    let config_path = dir.join("test_settings.conf");

    // Save with modified values
    {
        let mut store = SettingsStore::new().with_config_path(config_path.clone());

        store
            .set(
                &SettingKey::new("appearance.font_size"),
                SettingValue::Int(18),
            )
            .unwrap();
        store
            .set(
                &SettingKey::new("desktop.show_icons"),
                SettingValue::Bool(false),
            )
            .unwrap();
        store
            .set(
                &SettingKey::new("appearance.theme"),
                SettingValue::Choice("night".into()),
            )
            .unwrap();
        store
            .set(
                &SettingKey::new("input.mouse_speed"),
                SettingValue::Float(2.0),
            )
            .unwrap();

        store.save().unwrap();
        assert!(!store.is_dirty());
    }

    // Load into a fresh store
    {
        let mut store = SettingsStore::new().with_config_path(config_path.clone());

        store.load().unwrap();

        assert_eq!(store.get_int("appearance.font_size"), Some(18));
        assert_eq!(store.get_bool("desktop.show_icons"), Some(false));
        assert_eq!(store.get_string("appearance.theme"), Some("night"));
        assert_eq!(store.get_float("input.mouse_speed"), Some(2.0));

        // Unchanged settings should still have defaults
        assert_eq!(store.get_int("wm.tiling_gap"), Some(8));
        assert_eq!(store.get_bool("wm.snap_enabled"), Some(true));
    }

    // Cleanup
    let _ = std::fs::remove_file(&config_path);
    let _ = std::fs::remove_dir(&dir);
}

#[test]
fn store_load_nonexistent_file_uses_defaults() {
    let config_path = std::env::temp_dir().join("liquide_settings_nonexistent.conf");
    let _ = std::fs::remove_file(&config_path); // ensure it doesn't exist

    let mut store = SettingsStore::new().with_config_path(config_path);

    assert!(store.load().is_ok());
    assert_eq!(store.get_int("appearance.font_size"), Some(14));
}

#[test]
fn store_save_without_config_path_fails() {
    let mut store = SettingsStore::new();
    let result = store.save();
    assert!(result.is_err());
}

// --- Panels tests ---

#[test]
fn default_panels_covers_categories() {
    let panels = default_panels();
    assert!(panels.len() >= 9);

    let panel_categories: Vec<_> = panels.iter().map(|p| &p.category).collect();
    assert!(panel_categories.contains(&&SettingCategory::Appearance));
    assert!(panel_categories.contains(&&SettingCategory::Desktop));
    assert!(panel_categories.contains(&&SettingCategory::WindowManagement));
    assert!(panel_categories.contains(&&SettingCategory::Input));
    assert!(panel_categories.contains(&&SettingCategory::Display));
    assert!(panel_categories.contains(&&SettingCategory::Power));
    assert!(panel_categories.contains(&&SettingCategory::Notifications));
    assert!(panel_categories.contains(&&SettingCategory::Accessibility));
    assert!(panel_categories.contains(&&SettingCategory::Privacy));
}

#[test]
fn default_panels_have_sections() {
    let panels = default_panels();
    for panel in &panels {
        assert!(
            !panel.sections.is_empty(),
            "panel {:?} has no sections",
            panel.category
        );
        for section in &panel.sections {
            assert!(!section.title.is_empty());
            assert!(
                !section.setting_keys.is_empty(),
                "section '{}' has no keys",
                section.title
            );
        }
    }
}

#[test]
fn default_panels_keys_exist_in_store() {
    let store = SettingsStore::new();
    let panels = default_panels();

    for panel in &panels {
        for section in &panel.sections {
            for key_str in &section.setting_keys {
                let key = SettingKey::new(key_str);
                assert!(
                    store.get(&key).is_some(),
                    "panel key '{}' not found in store (panel: {:?}, section: '{}')",
                    key_str,
                    panel.category,
                    section.title
                );
            }
        }
    }
}

// --- Error Display ---

#[test]
fn settings_error_display() {
    let err = SettingsError::NotFound(SettingKey::new("foo.bar"));
    assert!(format!("{}", err).contains("foo.bar"));

    let err = SettingsError::TypeMismatch;
    assert_eq!(format!("{}", err), "type mismatch");

    let err = SettingsError::OutOfRange("value too high".into());
    assert!(format!("{}", err).contains("value too high"));

    let err = SettingsError::InvalidChoice("bad".into());
    assert!(format!("{}", err).contains("bad"));

    let err = SettingsError::IoError("disk full".into());
    assert!(format!("{}", err).contains("disk full"));
}
