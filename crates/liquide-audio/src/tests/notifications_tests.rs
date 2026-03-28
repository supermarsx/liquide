//! Tests for the sound event / notification system.

use std::path::PathBuf;

use crate::notifications::*;

// ── SoundEvent ────────────────────────────────────────────────────────

#[test]
fn sound_event_all_variants() {
    assert_eq!(SoundEvent::ALL.len(), 15);
}

#[test]
fn sound_event_freedesktop_names() {
    assert_eq!(SoundEvent::NotificationPopup.freedesktop_name(), "message-new-instant");
    assert_eq!(SoundEvent::Error.freedesktop_name(), "dialog-error");
    assert_eq!(SoundEvent::VolumeChange.freedesktop_name(), "audio-volume-change");
    assert_eq!(SoundEvent::Screenshot.freedesktop_name(), "screen-capture");
    assert_eq!(SoundEvent::Trash.freedesktop_name(), "trash-empty");
}

#[test]
fn sound_event_display() {
    let s = format!("{}", SoundEvent::NotificationPopup);
    assert!(s.contains("message-new-instant"));
}

#[test]
fn sound_event_equality() {
    assert_eq!(SoundEvent::Error, SoundEvent::Error);
    assert_ne!(SoundEvent::Error, SoundEvent::Warning);
}

#[test]
fn sound_event_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(SoundEvent::Error);
    set.insert(SoundEvent::Warning);
    set.insert(SoundEvent::Error);
    assert_eq!(set.len(), 2);
}

// ── SoundTheme ────────────────────────────────────────────────────────

#[test]
fn sound_theme_freedesktop() {
    let theme = SoundTheme::freedesktop();
    assert_eq!(theme.name(), "freedesktop");
    assert_eq!(theme.base_dir(), std::path::Path::new("/usr/share/sounds"));
}

#[test]
fn sound_theme_lookup() {
    let theme = SoundTheme::freedesktop();
    let path = theme.lookup(SoundEvent::Error);
    assert_eq!(
        path,
        PathBuf::from("/usr/share/sounds/freedesktop/stereo/dialog-error.oga")
    );
}

#[test]
fn sound_theme_lookup_notification() {
    let theme = SoundTheme::freedesktop();
    let path = theme.lookup(SoundEvent::NotificationPopup);
    assert!(path.to_string_lossy().contains("message-new-instant"));
}

#[test]
fn sound_theme_override() {
    let mut theme = SoundTheme::freedesktop();
    let custom_path = PathBuf::from("/home/user/sounds/custom-error.wav");
    theme.set_override(SoundEvent::Error, custom_path.clone());

    let path = theme.lookup(SoundEvent::Error);
    assert_eq!(path, custom_path);

    // Non-overridden event still uses theme default.
    let path2 = theme.lookup(SoundEvent::Warning);
    assert!(path2.to_string_lossy().contains("dialog-warning"));
}

#[test]
fn sound_theme_remove_override() {
    let mut theme = SoundTheme::freedesktop();
    theme.set_override(SoundEvent::Error, PathBuf::from("/custom.wav"));
    theme.remove_override(SoundEvent::Error);

    let path = theme.lookup(SoundEvent::Error);
    assert!(path.to_string_lossy().contains("dialog-error"));
}

#[test]
fn sound_theme_display() {
    let theme = SoundTheme::freedesktop();
    let s = format!("{theme}");
    assert!(s.contains("freedesktop"));
    assert!(s.contains("0 overrides"));
}

#[test]
fn sound_theme_custom() {
    let theme = SoundTheme::new("Yaru".into(), PathBuf::from("/usr/share/sounds"));
    assert_eq!(theme.name(), "Yaru");
    let path = theme.lookup(SoundEvent::Login);
    assert!(path.to_string_lossy().contains("Yaru"));
}

// ── SoundConfig ───────────────────────────────────────────────────────

#[test]
fn sound_config_defaults() {
    let config = SoundConfig::new();
    assert!((config.global_volume - 1.0).abs() < f32::EPSILON);
    assert!(config.events_enabled);
    assert!(config.is_event_enabled(SoundEvent::Error));
}

#[test]
fn sound_config_disable_event() {
    let mut config = SoundConfig::new();
    config.set_event_enabled(SoundEvent::Error, false);
    assert!(!config.is_event_enabled(SoundEvent::Error));
    assert!(config.is_event_enabled(SoundEvent::Warning)); // Others still enabled
}

#[test]
fn sound_config_disable_all() {
    let mut config = SoundConfig::new();
    config.disable_all();
    assert!(!config.is_event_enabled(SoundEvent::Error));
    assert!(!config.is_event_enabled(SoundEvent::Login));
}

#[test]
fn sound_config_enable_all() {
    let mut config = SoundConfig::new();
    config.set_event_enabled(SoundEvent::Error, false);
    config.enable_all();
    assert!(config.is_event_enabled(SoundEvent::Error));
}

#[test]
fn sound_config_set_volume() {
    let mut config = SoundConfig::new();
    config.set_global_volume(0.5);
    assert!((config.global_volume - 0.5).abs() < f32::EPSILON);
}

#[test]
fn sound_config_set_volume_clamps() {
    let mut config = SoundConfig::new();
    config.set_global_volume(2.0);
    assert!((config.global_volume - 1.0).abs() < f32::EPSILON);
    config.set_global_volume(-0.5);
    assert!((config.global_volume - 0.0).abs() < f32::EPSILON);
}

#[test]
fn sound_config_display() {
    let config = SoundConfig::new();
    let s = format!("{config}");
    assert!(s.contains("SoundConfig"));
    assert!(s.contains("enabled"));
}

// ── SoundPlayer ───────────────────────────────────────────────────────

#[test]
fn player_play_event() {
    let mut player = SoundPlayer::with_defaults();
    let path = player.play_sound_event(SoundEvent::Error);
    assert!(path.is_some());
    assert!(path.unwrap().to_string_lossy().contains("dialog-error"));
}

#[test]
fn player_play_disabled_event() {
    let mut config = SoundConfig::new();
    config.set_event_enabled(SoundEvent::Error, false);
    let mut player = SoundPlayer::new(SoundTheme::freedesktop(), config);
    let path = player.play_sound_event(SoundEvent::Error);
    assert!(path.is_none());
}

#[test]
fn player_play_zero_volume() {
    let mut config = SoundConfig::new();
    config.set_global_volume(0.0);
    let mut player = SoundPlayer::new(SoundTheme::freedesktop(), config);
    let path = player.play_sound_event(SoundEvent::Error);
    assert!(path.is_none());
}

#[test]
fn player_play_log() {
    let mut player = SoundPlayer::with_defaults();
    player.play_sound_event(SoundEvent::Error);
    player.play_sound_event(SoundEvent::Warning);
    assert_eq!(player.play_log().len(), 2);
    assert_eq!(player.play_log()[0].0, SoundEvent::Error);
    assert_eq!(player.play_log()[1].0, SoundEvent::Warning);
}

#[test]
fn player_clear_log() {
    let mut player = SoundPlayer::with_defaults();
    player.play_sound_event(SoundEvent::Error);
    player.clear_log();
    assert!(player.play_log().is_empty());
}

#[test]
fn player_theme_access() {
    let player = SoundPlayer::with_defaults();
    assert_eq!(player.theme().name(), "freedesktop");
}

#[test]
fn player_config_access() {
    let player = SoundPlayer::with_defaults();
    assert!(player.config().events_enabled);
}

#[test]
fn player_config_mut() {
    let mut player = SoundPlayer::with_defaults();
    player.config_mut().set_global_volume(0.5);
    assert!((player.config().global_volume - 0.5).abs() < f32::EPSILON);
}

#[test]
fn player_theme_mut_override() {
    let mut player = SoundPlayer::with_defaults();
    player
        .theme_mut()
        .set_override(SoundEvent::Error, PathBuf::from("/custom.wav"));
    let path = player.play_sound_event(SoundEvent::Error);
    assert_eq!(path, Some(PathBuf::from("/custom.wav")));
}

#[test]
fn player_display() {
    let player = SoundPlayer::with_defaults();
    let s = format!("{player}");
    assert!(s.contains("SoundPlayer"));
}

#[test]
fn player_all_events_playable() {
    let mut player = SoundPlayer::with_defaults();
    for &event in SoundEvent::ALL {
        let path = player.play_sound_event(event);
        assert!(path.is_some(), "event {:?} should produce a path", event);
    }
    assert_eq!(player.play_log().len(), 15);
}
