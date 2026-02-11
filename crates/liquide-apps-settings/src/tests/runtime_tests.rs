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
    rt.set_value("audio.mute", SettingValue::Bool(true)).unwrap();
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
    rt.set_value("audio.mute", SettingValue::Bool(true)).unwrap();
    rt.reset_to_default("audio.mute").unwrap();
    assert_eq!(rt.value("audio.mute"), Some(&SettingValue::Bool(false)));
}

#[test]
fn test_runtime_undo_redo() {
    let mut rt = SettingsRuntime::new(SettingsConfig::default());
    rt.set_value("audio.mute", SettingValue::Bool(true)).unwrap();
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
    rt.policy_mut().set_constraint("audio.mute", PolicyConstraint::Locked);
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
    assert!(infos.iter().all(|i| i.entry_count > 0 || i.category == Category::Users));
}

#[test]
fn test_runtime_notifications() {
    let mut rt = SettingsRuntime::new(SettingsConfig::default());
    rt.set_value("audio.mute", SettingValue::Bool(true)).unwrap();
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
    let config = SettingsConfig { window_width: 1200, ..SettingsConfig::default() };
    let rt = SettingsRuntime::new(config);
    assert_eq!(rt.config().window_width, 1200);
}
