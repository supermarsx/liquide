//! Tests for the settings runtime coordinator.

use crate::category::Category;
use crate::config::SettingsConfig;
use crate::entry::SettingValue;
use crate::policy::PolicyConstraint;
use crate::runtime::SettingsRuntime;

#[test]
fn test_runtime_new() {
    let rt = SettingsRuntime::new(SettingsConfig::default());
    assert!(rt.total_entries() > 0);
    assert_eq!(rt.active_category(), Category::Display);
}

#[test]
fn test_runtime_set_category() {
    let mut rt = SettingsRuntime::new(SettingsConfig::default());
    rt.set_category(Category::Audio);
    assert_eq!(rt.active_category(), Category::Audio);
}

#[test]
fn test_runtime_get_value() {
    let rt = SettingsRuntime::new(SettingsConfig::default());
    let value = rt.value("display.resolution");
    assert!(value.is_some());
}

#[test]
fn test_runtime_set_value() {
    let mut rt = SettingsRuntime::new(SettingsConfig::default());
    rt.set_value("audio.mute", SettingValue::Bool(true))
        .unwrap();
    assert_eq!(rt.value("audio.mute"), Some(&SettingValue::Bool(true)));
}

#[test]
fn test_runtime_set_value_invalid() {
    let mut rt = SettingsRuntime::new(SettingsConfig::default());
    let result = rt.set_value("audio.volume", SettingValue::Number(200.0));
    assert!(result.is_err());
}

#[test]
fn test_runtime_set_value_unknown_key() {
    let mut rt = SettingsRuntime::new(SettingsConfig::default());
    let result = rt.set_value("nonexistent", SettingValue::Bool(true));
    assert!(result.is_err());
}

#[test]
fn test_runtime_reset_to_default() {
    let mut rt = SettingsRuntime::new(SettingsConfig::default());
    rt.set_value("audio.mute", SettingValue::Bool(true))
        .unwrap();
    rt.reset_to_default("audio.mute").unwrap();
    assert_eq!(rt.value("audio.mute"), Some(&SettingValue::Bool(false)));
}

#[test]
fn test_runtime_undo_redo() {
    let mut rt = SettingsRuntime::new(SettingsConfig::default());
    rt.set_value("audio.mute", SettingValue::Bool(true))
        .unwrap();
    assert!(rt.can_undo());

    rt.undo().unwrap();
    assert_eq!(rt.value("audio.mute"), Some(&SettingValue::Bool(false)));
    assert!(rt.can_redo());

    rt.redo().unwrap();
    assert_eq!(rt.value("audio.mute"), Some(&SettingValue::Bool(true)));
}

#[test]
fn test_runtime_undo_empty() {
    let mut rt = SettingsRuntime::new(SettingsConfig::default());
    assert!(rt.undo().is_err());
}

#[test]
fn test_runtime_policy_locked() {
    let mut rt = SettingsRuntime::new(SettingsConfig::default());
    rt.policy_mut()
        .set_constraint("audio.mute", PolicyConstraint::Locked);
    let result = rt.set_value("audio.mute", SettingValue::Bool(true));
    assert!(result.is_err());
}

#[test]
fn test_runtime_visible_entries() {
    let mut rt = SettingsRuntime::new(SettingsConfig::default());
    rt.set_category(Category::Audio);
    let visible = rt.visible_entries();
    assert!(!visible.is_empty());
}

#[test]
fn test_runtime_entries_for() {
    let rt = SettingsRuntime::new(SettingsConfig::default());
    let display_entries = rt.entries_for(Category::Display);
    assert!(display_entries.len() >= 3);
}

#[test]
fn test_runtime_search() {
    let mut rt = SettingsRuntime::new(SettingsConfig::default());
    rt.search("volume");
    assert!(!rt.search_results().is_empty());
}

#[test]
fn test_runtime_search_clear() {
    let mut rt = SettingsRuntime::new(SettingsConfig::default());
    rt.search("volume");
    rt.clear_search();
    assert!(rt.search_results().is_empty());
}

#[test]
fn test_runtime_category_infos() {
    let rt = SettingsRuntime::new(SettingsConfig::default());
    let infos = rt.category_infos();
    assert_eq!(infos.len(), 8);
    assert!(
        infos
            .iter()
            .all(|i| i.entry_count > 0 || i.category == Category::Users)
    );
}

#[test]
fn test_runtime_notifications() {
    let mut rt = SettingsRuntime::new(SettingsConfig::default());
    rt.set_value("audio.mute", SettingValue::Bool(true))
        .unwrap();
    let notifs = rt.drain_notifications();
    assert_eq!(notifs.len(), 1);
    assert_eq!(notifs[0].key, "audio.mute");
}

#[test]
fn test_runtime_page() {
    let rt = SettingsRuntime::new(SettingsConfig::default());
    let page = rt.page(Category::Display);
    assert!(page.is_some());
    assert!(page.unwrap().entry_count() > 0);
}

#[test]
fn test_runtime_config() {
    let config = SettingsConfig {
        window_width: 1200,
        ..SettingsConfig::default()
    };
    let rt = SettingsRuntime::new(config);
    assert_eq!(rt.config().window_width, 1200);
}

// ---- Persistence tests ----

#[test]
fn test_save_and_load_round_trip() {
    let dir = std::env::temp_dir().join("liquide_test_roundtrip");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("settings.json");

    let mut rt = SettingsRuntime::new(SettingsConfig::default());
    rt.set_value("audio.mute", SettingValue::Bool(true))
        .unwrap();
    rt.set_value("audio.volume", SettingValue::Number(75.0))
        .unwrap();
    rt.save_to_path(&path).unwrap();

    // Load into a fresh runtime
    let mut rt2 = SettingsRuntime::new(SettingsConfig::default());
    rt2.load_from_path(&path).unwrap();
    assert_eq!(rt2.value("audio.mute"), Some(&SettingValue::Bool(true)));
    assert_eq!(rt2.value("audio.volume"), Some(&SettingValue::Number(75.0)));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_load_nonexistent_file_is_ok() {
    let path = std::env::temp_dir().join("liquide_test_nofile/settings.json");
    let mut rt = SettingsRuntime::new(SettingsConfig::default());
    assert!(rt.load_from_path(&path).is_ok());
}

#[test]
fn test_load_invalid_json() {
    use std::io::Write;
    let dir = std::env::temp_dir().join("liquide_test_badjson");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("settings.json");
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(b"not valid json").unwrap();

    let mut rt = SettingsRuntime::new(SettingsConfig::default());
    assert!(rt.load_from_path(&path).is_err());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_load_ignores_unknown_keys() {
    let dir = std::env::temp_dir().join("liquide_test_unknown_keys");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("settings.json");
    std::fs::write(&path, r#"{"unknown.key": true, "audio.mute": true}"#).unwrap();

    let mut rt = SettingsRuntime::new(SettingsConfig::default());
    rt.load_from_path(&path).unwrap();
    assert_eq!(rt.value("audio.mute"), Some(&SettingValue::Bool(true)));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_load_skips_invalid_values() {
    let dir = std::env::temp_dir().join("liquide_test_invalid_val");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("settings.json");
    // volume out of range [0, 100]
    std::fs::write(&path, r#"{"audio.volume": 999.0}"#).unwrap();

    let mut rt = SettingsRuntime::new(SettingsConfig::default());
    let original = rt.value("audio.volume").cloned();
    rt.load_from_path(&path).unwrap();
    // Value should remain at default since 999 is out of range
    assert_eq!(rt.value("audio.volume").cloned(), original);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_handle_change() {
    let mut rt = SettingsRuntime::new(SettingsConfig::default());
    let result = rt.handle_change("audio.mute", SettingValue::Bool(true));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), true);
    assert_eq!(rt.value("audio.mute"), Some(&SettingValue::Bool(true)));
}

#[test]
fn test_handle_change_locked() {
    let mut rt = SettingsRuntime::new(SettingsConfig::default());
    rt.policy_mut()
        .set_constraint("audio.mute", PolicyConstraint::Locked);
    let result = rt.handle_change("audio.mute", SettingValue::Bool(true));
    assert!(result.is_err());
}

#[test]
fn test_apply_changes_saves() {
    let dir = std::env::temp_dir().join("liquide_test_apply");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("settings.json");

    // We test save_to_path directly since apply_changes uses settings_file()
    let mut rt = SettingsRuntime::new(SettingsConfig::default());
    rt.set_value("audio.mute", SettingValue::Bool(true))
        .unwrap();
    rt.save_to_path(&path).unwrap();
    assert!(path.exists());

    let content = std::fs::read_to_string(&path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(parsed["audio.mute"], serde_json::Value::Bool(true));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_revert_changes() {
    let mut rt = SettingsRuntime::new(SettingsConfig::default());
    let original = rt.value("audio.mute").cloned().unwrap();
    rt.set_value("audio.mute", SettingValue::Bool(true))
        .unwrap();
    rt.revert_changes();
    assert_eq!(rt.value("audio.mute"), Some(&original));
}

#[test]
fn test_category_settings_display() {
    let rt = SettingsRuntime::new(SettingsConfig::default());
    let displays = rt.category_settings(Category::Audio);
    assert!(!displays.is_empty());
    for d in &displays {
        assert_eq!(d.category, Category::Audio);
        assert!(!d.label.is_empty());
        assert!(!d.locked);
    }
}

#[test]
fn test_category_settings_locked() {
    let mut rt = SettingsRuntime::new(SettingsConfig::default());
    rt.policy_mut()
        .set_constraint("audio.mute", PolicyConstraint::Locked);
    let displays = rt.category_settings(Category::Audio);
    let mute = displays.iter().find(|d| d.key == "audio.mute");
    assert!(mute.is_some());
    assert!(mute.unwrap().locked);
}

#[test]
fn test_category_settings_hidden() {
    let mut rt = SettingsRuntime::new(SettingsConfig::default());
    rt.policy_mut()
        .set_constraint("audio.mute", PolicyConstraint::Hidden);
    let displays = rt.category_settings(Category::Audio);
    assert!(displays.iter().all(|d| d.key != "audio.mute"));
}

#[test]
fn test_active_category_settings() {
    let mut rt = SettingsRuntime::new(SettingsConfig::default());
    rt.set_category(Category::Privacy);
    let displays = rt.active_category_settings();
    assert!(!displays.is_empty());
    assert!(displays.iter().all(|d| d.category == Category::Privacy));
}

#[test]
fn test_all_entries_as_json() {
    let rt = SettingsRuntime::new(SettingsConfig::default());
    let json = rt.all_entries_as_json();
    assert!(json.is_object());
    let map = json.as_object().unwrap();
    assert!(map.contains_key("audio.volume"));
    assert!(map.contains_key("display.resolution"));
}

#[test]
fn test_settings_dir_not_empty() {
    let dir = crate::runtime::settings_dir();
    assert!(!dir.as_os_str().is_empty());
}

#[test]
fn test_settings_file_ends_with_json() {
    let file = crate::runtime::settings_file();
    assert!(file.to_string_lossy().ends_with("settings.json"));
}
