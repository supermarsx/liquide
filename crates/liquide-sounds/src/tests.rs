#[cfg(test)]
mod tests {
    use crate::event::SoundEvent;
    use crate::format::{SoundFile, SoundFormat};
    use crate::manager::SoundManager;
    use crate::theme::{self, SoundTheme};
    use crate::wav;

    // -----------------------------------------------------------------------
    // SoundEvent tests
    // -----------------------------------------------------------------------

    #[test]
    fn event_all_returns_24_variants() {
        assert_eq!(SoundEvent::all().len(), 24);
    }

    #[test]
    fn event_roundtrip_str() {
        for &event in SoundEvent::all() {
            let s = event.as_str();
            let parsed = SoundEvent::from_str(s);
            assert_eq!(parsed, Some(event), "roundtrip failed for {}", s);
        }
    }

    #[test]
    fn event_from_str_unknown_returns_none() {
        assert_eq!(SoundEvent::from_str("nonexistent"), None);
        assert_eq!(SoundEvent::from_str(""), None);
    }

    #[test]
    fn event_display_matches_as_str() {
        for &event in SoundEvent::all() {
            assert_eq!(format!("{}", event), event.as_str());
        }
    }

    // -----------------------------------------------------------------------
    // SoundFormat tests
    // -----------------------------------------------------------------------

    #[test]
    fn format_extension_roundtrip() {
        for fmt in [SoundFormat::Wav, SoundFormat::Ogg, SoundFormat::Flac] {
            let ext = fmt.extension();
            let parsed = SoundFormat::from_extension(ext);
            assert_eq!(parsed, Some(fmt));
        }
    }

    #[test]
    fn format_from_extension_case_insensitive() {
        assert_eq!(SoundFormat::from_extension("WAV"), Some(SoundFormat::Wav));
        assert_eq!(SoundFormat::from_extension("Ogg"), Some(SoundFormat::Ogg));
        assert_eq!(SoundFormat::from_extension("FLAC"), Some(SoundFormat::Flac));
    }

    #[test]
    fn format_from_extension_aliases() {
        assert_eq!(SoundFormat::from_extension("wave"), Some(SoundFormat::Wav));
        assert_eq!(SoundFormat::from_extension("oga"), Some(SoundFormat::Ogg));
    }

    #[test]
    fn format_from_extension_unknown() {
        assert_eq!(SoundFormat::from_extension("mp3"), None);
        assert_eq!(SoundFormat::from_extension(""), None);
    }

    #[test]
    fn format_from_path() {
        assert_eq!(
            SoundFormat::from_path("sounds/click.wav"),
            Some(SoundFormat::Wav)
        );
        assert_eq!(
            SoundFormat::from_path("/usr/share/sounds/bell.ogg"),
            Some(SoundFormat::Ogg)
        );
        assert_eq!(
            SoundFormat::from_path("music.flac"),
            Some(SoundFormat::Flac)
        );
    }

    #[test]
    fn format_mime_types() {
        assert_eq!(SoundFormat::Wav.mime_type(), "audio/wav");
        assert_eq!(SoundFormat::Ogg.mime_type(), "audio/ogg");
        assert_eq!(SoundFormat::Flac.mime_type(), "audio/flac");
    }

    // -----------------------------------------------------------------------
    // SoundFile tests
    // -----------------------------------------------------------------------

    #[test]
    fn sound_file_auto_detect_format() {
        let f = SoundFile::new("path/to/sound.ogg");
        assert_eq!(f.format, SoundFormat::Ogg);
        assert_eq!(f.path, "path/to/sound.ogg");
    }

    #[test]
    fn sound_file_unknown_extension_defaults_to_wav() {
        let f = SoundFile::new("path/to/sound.xyz");
        assert_eq!(f.format, SoundFormat::Wav);
    }

    #[test]
    fn sound_file_explicit_format() {
        let f = SoundFile::with_format("sound.bin", SoundFormat::Flac);
        assert_eq!(f.format, SoundFormat::Flac);
    }

    // -----------------------------------------------------------------------
    // SoundTheme tests
    // -----------------------------------------------------------------------

    #[test]
    fn theme_new_is_empty() {
        let t = SoundTheme::new("test", "Test");
        assert!(t.is_empty());
        assert_eq!(t.len(), 0);
        assert_eq!(t.id, "test");
        assert_eq!(t.name, "Test");
        assert!(t.parent.is_none());
    }

    #[test]
    fn theme_with_parent() {
        let t = SoundTheme::new("child", "Child").with_parent("parent_id");
        assert_eq!(t.parent.as_deref(), Some("parent_id"));
        assert_eq!(t.inherits_from.as_deref(), Some("parent_id"));
    }

    #[test]
    fn theme_insert_and_get() {
        let mut t = SoundTheme::new("t", "T");
        t.insert(SoundEvent::Login, SoundFile::new("login.wav"));
        assert!(t.has_sound(SoundEvent::Login));
        assert!(!t.has_sound(SoundEvent::Logout));
        assert_eq!(t.len(), 1);

        let f = t.get(SoundEvent::Login).unwrap();
        assert_eq!(f.path, "login.wav");
    }

    // -----------------------------------------------------------------------
    // Built-in themes
    // -----------------------------------------------------------------------

    #[test]
    fn default_theme_has_all_events() {
        let t = theme::default_theme();
        assert_eq!(t.id, "default");
        for &event in SoundEvent::all() {
            assert!(
                t.has_sound(event),
                "default theme missing sound for {:?}",
                event
            );
        }
        assert_eq!(t.len(), SoundEvent::all().len());
    }

    #[test]
    fn silent_theme_is_empty() {
        let t = theme::silent_theme();
        assert_eq!(t.id, "silent");
        assert!(t.is_empty());
    }

    #[test]
    fn minimal_theme_inherits_from_default() {
        let t = theme::minimal_theme();
        assert_eq!(t.id, "minimal");
        assert_eq!(t.parent.as_deref(), Some("default"));
        // Should have a subset of events.
        assert!(t.len() > 0);
        assert!(t.len() < SoundEvent::all().len());
        // Must have notification.
        assert!(t.has_sound(SoundEvent::NotificationDefault));
    }

    #[test]
    fn retro_theme_has_ogg_files() {
        let t = theme::retro_theme();
        assert_eq!(t.id, "retro");
        for &event in SoundEvent::all() {
            let f = t.get(event).unwrap();
            assert_eq!(f.format, SoundFormat::Ogg);
            assert!(f.path.ends_with(".ogg"));
        }
    }

    // -----------------------------------------------------------------------
    // WAV generation tests
    // -----------------------------------------------------------------------

    #[test]
    fn wav_beep_has_valid_header() {
        let data = wav::generate_beep(440.0, 100, 0.5);
        assert!(data.len() >= 44);
        assert_eq!(&data[0..4], b"RIFF");
        assert_eq!(&data[8..12], b"WAVE");
        assert_eq!(&data[12..16], b"fmt ");
        assert_eq!(&data[36..40], b"data");
    }

    #[test]
    fn wav_beep_correct_size() {
        let data = wav::generate_beep(440.0, 100, 1.0);
        // 44100 samples/sec * 0.1 sec * 2 bytes/sample = 8820 bytes of PCM
        let expected_pcm = 8820usize;
        assert_eq!(data.len(), 44 + expected_pcm);

        // Verify data sub-chunk size in header
        let data_size = u32::from_le_bytes([data[40], data[41], data[42], data[43]]);
        assert_eq!(data_size as usize, expected_pcm);
    }

    #[test]
    fn wav_beep_zero_volume_is_silent() {
        let data = wav::generate_beep(440.0, 50, 0.0);
        // All PCM samples should be zero.
        for chunk in data[44..].chunks(2) {
            let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
            assert_eq!(sample, 0);
        }
    }

    #[test]
    fn wav_chime_valid() {
        let data = wav::generate_chime(&[440.0, 554.0, 659.0], 200);
        assert!(data.len() >= 44);
        assert_eq!(&data[0..4], b"RIFF");

        // Should be 200ms of data
        let expected_pcm = (44100 * 200 / 1000) * 2;
        assert_eq!(data.len(), 44 + expected_pcm);
    }

    #[test]
    fn wav_chime_empty_frequencies() {
        let data = wav::generate_chime(&[], 100);
        // Should still produce valid WAV (silent beep fallback).
        assert!(data.len() >= 44);
        assert_eq!(&data[0..4], b"RIFF");
    }

    #[test]
    fn wav_click_valid() {
        let data = wav::generate_click(10);
        assert!(data.len() >= 44);
        assert_eq!(&data[0..4], b"RIFF");

        let expected_pcm = (44100 * 10 / 1000) * 2;
        assert_eq!(data.len(), 44 + expected_pcm);
    }

    #[test]
    fn wav_click_deterministic() {
        let a = wav::generate_click(10);
        let b = wav::generate_click(10);
        assert_eq!(a, b, "click generation should be deterministic");
    }

    #[test]
    fn wav_sweep_valid() {
        let data = wav::generate_sweep(200.0, 800.0, 150, 0.7);
        assert!(data.len() >= 44);
        assert_eq!(&data[0..4], b"RIFF");

        let expected_pcm = (44100 * 150 / 1000) * 2;
        assert_eq!(data.len(), 44 + expected_pcm);
    }

    #[test]
    fn wav_beep_samples_within_range() {
        let data = wav::generate_beep(1000.0, 50, 1.0);
        for chunk in data[44..].chunks(2) {
            let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
            assert!(
                sample >= i16::MIN && sample <= i16::MAX,
                "sample out of i16 range: {}",
                sample
            );
        }
    }

    #[test]
    fn wav_header_format_fields() {
        let data = wav::generate_beep(440.0, 10, 0.5);

        // PCM format = 1
        let audio_format = u16::from_le_bytes([data[20], data[21]]);
        assert_eq!(audio_format, 1);

        // Mono = 1 channel
        let channels = u16::from_le_bytes([data[22], data[23]]);
        assert_eq!(channels, 1);

        // Sample rate = 44100
        let sample_rate = u32::from_le_bytes([data[24], data[25], data[26], data[27]]);
        assert_eq!(sample_rate, 44100);

        // Bits per sample = 16
        let bps = u16::from_le_bytes([data[34], data[35]]);
        assert_eq!(bps, 16);

        // Byte rate = 44100 * 1 * 2 = 88200
        let byte_rate = u32::from_le_bytes([data[28], data[29], data[30], data[31]]);
        assert_eq!(byte_rate, 88200);

        // Block align = 1 * 2 = 2
        let block_align = u16::from_le_bytes([data[32], data[33]]);
        assert_eq!(block_align, 2);
    }

    // -----------------------------------------------------------------------
    // SoundManager tests
    // -----------------------------------------------------------------------

    #[test]
    fn manager_default_has_four_themes() {
        let m = SoundManager::new();
        assert_eq!(m.theme_count(), 4);
        assert_eq!(m.active_theme_id(), "default");
    }

    #[test]
    fn manager_set_theme() {
        let mut m = SoundManager::new();
        assert!(m.set_theme("silent"));
        assert_eq!(m.active_theme_id(), "silent");

        assert!(!m.set_theme("nonexistent"));
        // Still silent after failed switch.
        assert_eq!(m.active_theme_id(), "silent");
    }

    #[test]
    fn manager_active_theme_reference() {
        let m = SoundManager::new();
        let t = m.active_theme();
        assert_eq!(t.id, "default");
        assert_eq!(t.name, "LiquiDE Default");
    }

    #[test]
    fn manager_enabled_toggle() {
        let mut m = SoundManager::new();
        assert!(m.is_enabled());
        m.set_enabled(false);
        assert!(!m.is_enabled());
        m.set_enabled(true);
        assert!(m.is_enabled());
    }

    #[test]
    fn manager_volume_clamp() {
        let mut m = SoundManager::new();
        m.set_volume(0.5);
        assert!((m.volume() - 0.5).abs() < f32::EPSILON);

        m.set_volume(-1.0);
        assert!((m.volume() - 0.0).abs() < f32::EPSILON);

        m.set_volume(5.0);
        assert!((m.volume() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn manager_resolve_sound_default_theme() {
        let m = SoundManager::new();
        let sound = m.resolve_sound(SoundEvent::Login);
        assert!(sound.is_some());
        let f = sound.unwrap();
        assert!(f.path.contains("login"));
        assert_eq!(f.format, SoundFormat::Wav);
    }

    #[test]
    fn manager_resolve_sound_silent_theme() {
        let mut m = SoundManager::new();
        m.set_theme("silent");
        // Silent theme has no sounds and no parent — everything resolves to None.
        assert!(m.resolve_sound(SoundEvent::Login).is_none());
    }

    #[test]
    fn manager_resolve_sound_with_inheritance() {
        let mut m = SoundManager::new();
        m.set_theme("minimal");

        // Minimal has NotificationDefault mapped.
        let direct = m.resolve_sound(SoundEvent::NotificationDefault);
        assert!(direct.is_some());
        assert!(direct.unwrap().path.contains("minimal"));

        // Minimal does NOT have WindowOpen, but its parent (default) does.
        let inherited = m.resolve_sound(SoundEvent::WindowOpen);
        assert!(inherited.is_some());
        assert!(inherited.unwrap().path.contains("default"));
    }

    #[test]
    fn manager_resolve_path() {
        let m = SoundManager::new();
        let path = m.resolve_path(SoundEvent::Error);
        assert!(path.is_some());
        assert!(path.unwrap().contains("error"));
    }

    #[test]
    fn manager_register_custom_theme() {
        let mut m = SoundManager::new();
        let mut custom = SoundTheme::new("custom", "My Custom");
        custom.insert(SoundEvent::Login, SoundFile::new("my_login.wav"));
        m.register_theme(custom);

        assert_eq!(m.theme_count(), 5);
        assert!(m.set_theme("custom"));
        let s = m.resolve_sound(SoundEvent::Login).unwrap();
        assert_eq!(s.path, "my_login.wav");
    }

    #[test]
    fn manager_remove_theme() {
        let mut m = SoundManager::new();
        // Cannot remove active theme.
        assert!(m.remove_theme("default").is_none());

        // Can remove non-active theme.
        let removed = m.remove_theme("retro");
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().id, "retro");
        assert_eq!(m.theme_count(), 3);
    }

    #[test]
    fn manager_theme_ids() {
        let m = SoundManager::new();
        let mut ids = m.theme_ids();
        ids.sort();
        assert_eq!(ids, vec!["default", "minimal", "retro", "silent"]);
    }

    #[test]
    fn manager_inheritance_depth_limit() {
        // Create a chain of themes: a -> b -> c -> ... that exceeds depth limit.
        let mut m = SoundManager::new();
        for i in 0..12 {
            let parent = if i == 0 {
                None
            } else {
                Some(format!("chain_{}", i - 1))
            };
            let mut t = SoundTheme::new(format!("chain_{}", i), format!("Chain {}", i));
            if let Some(p) = parent {
                t = t.with_parent(p);
            }
            // Only the root theme has a sound.
            if i == 0 {
                t.insert(SoundEvent::Login, SoundFile::new("deep.wav"));
            }
            m.register_theme(t);
        }

        // Set to the deepest theme (chain_11).
        m.set_theme("chain_11");

        // Depth limit is 8, so chain_11 -> chain_10 -> ... -> chain_3 (depth 8)
        // won't reach chain_0. Should return None.
        assert!(m.resolve_sound(SoundEvent::Login).is_none());

        // But chain_7 -> chain_0 is depth 7, which should work.
        m.set_theme("chain_7");
        assert!(m.resolve_sound(SoundEvent::Login).is_some());
    }

    #[test]
    fn manager_default_impl() {
        let m = SoundManager::default();
        assert_eq!(m.active_theme_id(), "default");
        assert!(m.is_enabled());
    }
}
