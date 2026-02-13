use crate::keymap::{DefaultKeymap, KeymapTranslator};

#[test]
fn default_keymap_returns_none_for_zero() {
    let keymap = DefaultKeymap;
    assert!(keymap.translate_scancode(0).is_none());
}

#[test]
fn default_keymap_returns_none_for_arbitrary_scancode() {
    let keymap = DefaultKeymap;
    assert!(keymap.translate_scancode(42).is_none());
}

#[test]
fn default_keymap_returns_none_for_large_scancode() {
    let keymap = DefaultKeymap;
    assert!(keymap.translate_scancode(u32::MAX).is_none());
}

#[test]
fn default_keymap_platform_name_is_null() {
    let keymap = DefaultKeymap;
    assert_eq!(keymap.platform_name(), "null");
}

#[test]
fn default_keymap_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<DefaultKeymap>();
}

#[test]
fn default_keymap_debug() {
    let keymap = DefaultKeymap;
    let debug = format!("{keymap:?}");
    assert!(debug.contains("DefaultKeymap"));
}
