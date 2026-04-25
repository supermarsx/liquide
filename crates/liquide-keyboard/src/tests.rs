#[cfg(test)]
mod tests {
    use crate::builtin::*;
    use crate::layout::*;
    use crate::manager::*;
    use crate::osk::*;
    use crate::repeat::*;
    use std::collections::HashMap;

    // ── Layout basics ───────────────────────────────────────────────────

    #[test]
    fn us_qwerty_has_at_least_47_keys() {
        let layout = layout_us_qwerty();
        assert!(
            layout.key_count() >= 47,
            "US QWERTY should have at least 47 keys, got {}",
            layout.key_count()
        );
    }

    #[test]
    fn uk_qwerty_has_at_least_47_keys() {
        let layout = layout_uk_qwerty();
        assert!(layout.key_count() >= 47);
    }

    #[test]
    fn de_qwertz_has_at_least_47_keys() {
        let layout = layout_de_qwertz();
        assert!(layout.key_count() >= 47);
    }

    #[test]
    fn fr_azerty_has_at_least_47_keys() {
        let layout = layout_fr_azerty();
        assert!(layout.key_count() >= 47);
    }

    #[test]
    fn dvorak_has_at_least_47_keys() {
        let layout = layout_us_dvorak();
        assert!(layout.key_count() >= 47);
    }

    #[test]
    fn us_qwerty_letter_a() {
        let layout = layout_us_qwerty();
        let mapping = layout.get(0x1E).expect("scancode 0x1E should be mapped");
        assert_eq!(mapping.normal, 'a');
        assert_eq!(mapping.shift, Some('A'));
    }

    #[test]
    fn de_qwertz_z_y_swap() {
        let layout = layout_de_qwertz();
        // SC_Y (0x15) should produce 'z' in QWERTZ
        let y_pos = layout.get(0x15).unwrap();
        assert_eq!(y_pos.normal, 'z');
        // SC_Z (0x2C) should produce 'y' in QWERTZ
        let z_pos = layout.get(0x2C).unwrap();
        assert_eq!(z_pos.normal, 'y');
    }

    #[test]
    fn fr_azerty_number_row_shifted() {
        let layout = layout_fr_azerty();
        // In AZERTY, Shift+1 = '1' (digits need shift)
        let key_1 = layout.get(0x02).unwrap();
        assert_eq!(key_1.normal, '&');
        assert_eq!(key_1.shift, Some('1'));
    }

    #[test]
    fn dvorak_home_row() {
        let layout = layout_us_dvorak();
        // Dvorak home row: a o e u i d h t n s
        let expected = [
            (0x1E, 'a'), // A position
            (0x1F, 'o'), // S position
            (0x20, 'e'), // D position
            (0x21, 'u'), // F position
            (0x22, 'i'), // G position
        ];
        for (sc, ch) in expected {
            assert_eq!(layout.get(sc).unwrap().normal, ch);
        }
    }

    // ── Dead key composition ────────────────────────────────────────────

    #[test]
    fn dead_key_compose() {
        let dk = DeadKey {
            id: 1,
            base_char: '^',
            combinations: {
                let mut m = HashMap::new();
                m.insert('a', '\u{00e2}');
                m.insert('e', '\u{00ea}');
                m
            },
            fallback: '^',
        };
        assert_eq!(dk.compose('a'), '\u{00e2}');
        assert_eq!(dk.compose('e'), '\u{00ea}');
        assert_eq!(dk.compose('x'), '^'); // fallback
    }

    #[test]
    fn dead_key_has_combination() {
        let dk = DeadKey {
            id: 1,
            base_char: '`',
            combinations: {
                let mut m = HashMap::new();
                m.insert('a', '\u{00e0}');
                m
            },
            fallback: '`',
        };
        assert!(dk.has_combination('a'));
        assert!(!dk.has_combination('z'));
    }

    #[test]
    fn de_qwertz_dead_key_circumflex() {
        let layout = layout_de_qwertz();
        let dk = layout.get_dead_key(1).expect("dead key 1 should exist");
        assert_eq!(dk.base_char, '^');
        assert_eq!(dk.compose('a'), '\u{00e2}');
        assert_eq!(dk.compose('o'), '\u{00f4}');
    }

    // ── Manager ─────────────────────────────────────────────────────────

    #[test]
    fn manager_default_has_builtin_layouts() {
        let mgr = KeyboardLayoutManager::new();
        assert_eq!(mgr.layout_count(), 5);
        assert_eq!(mgr.active_layout().id, "us");
    }

    #[test]
    fn manager_set_layout() {
        let mut mgr = KeyboardLayoutManager::new();
        assert!(mgr.set_layout("de"));
        assert_eq!(mgr.active_layout().id, "de");
        assert!(!mgr.set_layout("nonexistent"));
        assert_eq!(mgr.active_layout().id, "de"); // unchanged
    }

    #[test]
    fn manager_next_layout_cycles() {
        let mut mgr = KeyboardLayoutManager::new();
        let ids: Vec<String> = mgr
            .available_layouts()
            .iter()
            .map(|l| l.id.clone())
            .collect();

        for i in 0..ids.len() + 1 {
            assert_eq!(mgr.active_layout().id, ids[i % ids.len()]);
            mgr.next_layout();
        }
    }

    #[test]
    fn manager_add_layout() {
        let mut mgr = KeyboardLayoutManager::empty();
        assert_eq!(mgr.layout_count(), 0);
        mgr.add_layout(layout_us_qwerty());
        assert_eq!(mgr.layout_count(), 1);
        // Adding same ID replaces
        mgr.add_layout(layout_us_qwerty());
        assert_eq!(mgr.layout_count(), 1);
        // Different ID adds
        mgr.add_layout(layout_de_qwertz());
        assert_eq!(mgr.layout_count(), 2);
    }

    #[test]
    fn translate_basic_char() {
        let mut mgr = KeyboardLayoutManager::new();
        // 'a' unshifted
        let out = mgr.translate_scancode(0x1E, Modifiers::empty());
        assert_eq!(out, KeyOutput::Char('a'));
    }

    #[test]
    fn translate_shifted_char() {
        let mut mgr = KeyboardLayoutManager::new();
        let out = mgr.translate_scancode(0x1E, Modifiers::SHIFT);
        assert_eq!(out, KeyOutput::Char('A'));
    }

    #[test]
    fn translate_caps_lock() {
        let mut mgr = KeyboardLayoutManager::new();
        // CapsLock without Shift -> uppercase
        let out = mgr.translate_scancode(0x1E, Modifiers::CAPS_LOCK);
        assert_eq!(out, KeyOutput::Char('A'));
        // CapsLock + Shift -> lowercase (they cancel)
        let out = mgr.translate_scancode(0x1E, Modifiers::SHIFT | Modifiers::CAPS_LOCK);
        assert_eq!(out, KeyOutput::Char('a'));
    }

    #[test]
    fn translate_caps_lock_does_not_affect_numbers() {
        let mut mgr = KeyboardLayoutManager::new();
        // CapsLock should NOT shift numbers
        let out = mgr.translate_scancode(0x02, Modifiers::CAPS_LOCK);
        assert_eq!(out, KeyOutput::Char('1'));
        let out = mgr.translate_scancode(0x02, Modifiers::SHIFT);
        assert_eq!(out, KeyOutput::Char('!'));
    }

    #[test]
    fn translate_unknown_scancode() {
        let mut mgr = KeyboardLayoutManager::new();
        let out = mgr.translate_scancode(0xFF, Modifiers::empty());
        assert_eq!(out, KeyOutput::None);
    }

    #[test]
    fn translate_dead_key_composition() {
        let mut mgr = KeyboardLayoutManager::new();
        mgr.set_layout("de");
        // Grave accent key (SC_GRAVE = 0x29) is dead circumflex in DE
        let out = mgr.translate_scancode(0x29, Modifiers::empty());
        assert_eq!(out, KeyOutput::DeadKey(1)); // circumflex dead key
        assert!(mgr.is_composing());

        // Now press 'a' (SC_A = 0x1E)
        let out = mgr.translate_scancode(0x1E, Modifiers::empty());
        assert_eq!(out, KeyOutput::Composed('\u{00e2}')); // â
        assert!(!mgr.is_composing());
    }

    #[test]
    fn translate_dead_key_no_match() {
        let mut mgr = KeyboardLayoutManager::new();
        mgr.set_layout("de");
        // Start circumflex dead key
        let _out = mgr.translate_scancode(0x29, Modifiers::empty());
        // Press 'x' — no composition for x with circumflex
        let out = mgr.translate_scancode(0x2D, Modifiers::empty());
        // Should produce 'x' (the pressed key)
        assert_eq!(out, KeyOutput::Char('x'));
        assert!(!mgr.is_composing());
    }

    #[test]
    fn translate_alt_gr() {
        let mut mgr = KeyboardLayoutManager::new();
        mgr.set_layout("de");
        // AltGr+Q = @ on German layout
        let out = mgr.translate_scancode(0x10, Modifiers::ALT_GR);
        assert_eq!(out, KeyOutput::Char('@'));
    }

    // ── OSK ─────────────────────────────────────────────────────────────

    #[test]
    fn osk_layout_has_5_rows() {
        let layout = layout_us_qwerty();
        let osk = compute_osk_layout(&layout, 800.0, 300.0);
        assert_eq!(osk.rows.len(), 5);
    }

    #[test]
    fn osk_layout_key_count() {
        let layout = layout_us_qwerty();
        let osk = compute_osk_layout(&layout, 800.0, 300.0);
        // 14 + 14 + 13 + 12 + 7 = 60
        assert_eq!(osk.key_count(), 60);
    }

    #[test]
    fn osk_layout_labels_from_layout() {
        let layout = layout_de_qwertz();
        let osk = compute_osk_layout(&layout, 800.0, 300.0);
        // In QWERTZ, the key at SC_Y position (row 1, index 6) should show 'Z'
        let row1 = &osk.rows[1];
        let y_key = row1.keys.iter().find(|k| k.scancode == 0x15).unwrap();
        assert_eq!(y_key.label, "Z");
    }

    #[test]
    fn osk_hit_test() {
        let layout = layout_us_qwerty();
        let osk = compute_osk_layout(&layout, 800.0, 300.0);
        // First key in first row should be hittable near (0, 0)
        let hit = osk.hit_test(1.0, 1.0);
        assert!(hit.is_some());
        // Far outside should miss
        let miss = osk.hit_test(900.0, 400.0);
        assert!(miss.is_none());
    }

    #[test]
    fn osk_keys_within_bounds() {
        let layout = layout_us_qwerty();
        let osk = compute_osk_layout(&layout, 800.0, 300.0);
        for row in &osk.rows {
            for key in &row.keys {
                assert!(key.x >= 0.0, "key x={} should be >= 0", key.x);
                assert!(key.y >= 0.0, "key y={} should be >= 0", key.y);
                assert!(
                    key.x + key.w <= 801.0, // allow 1px rounding
                    "key right edge {} should be <= width",
                    key.x + key.w
                );
                assert!(
                    key.y + key.h <= 301.0,
                    "key bottom edge {} should be <= height",
                    key.y + key.h
                );
            }
        }
    }

    // ── Key repeat ──────────────────────────────────────────────────────

    #[test]
    fn key_repeat_default() {
        let kr = KeyRepeat::default();
        assert_eq!(kr.delay_ms, 500);
        assert_eq!(kr.rate_ms, 33);
        assert!(!kr.is_disabled());
        // ~30 keys/sec
        assert!((kr.rate_hz() - 30.3).abs() < 1.0);
    }

    #[test]
    fn key_repeat_presets() {
        let slow = KeyRepeat::slow();
        assert_eq!(slow.delay_ms, 660);
        assert_eq!(slow.rate_ms, 50);

        let fast = KeyRepeat::fast();
        assert_eq!(fast.delay_ms, 300);
        assert_eq!(fast.rate_ms, 20);

        let disabled = KeyRepeat::disabled();
        assert!(disabled.is_disabled());
        assert_eq!(disabled.rate_hz(), 0.0);
    }

    #[test]
    fn key_repeat_tracker_basic() {
        let mut tracker = KeyRepeatTracker::new(KeyRepeat::new(100, 50));

        // No key held — tick produces nothing.
        assert_eq!(tracker.tick(200_000), 0);

        // Press a key.
        tracker.key_down(0x1E);
        assert_eq!(tracker.held_scancode(), Some(0x1E));

        // 50ms — still in delay period.
        assert_eq!(tracker.tick(50_000), 0);
        assert!(!tracker.is_repeating());

        // 60ms more (total 110ms) — past 100ms delay, should emit 1 repeat.
        let repeats = tracker.tick(60_000);
        assert_eq!(repeats, 1);
        assert!(tracker.is_repeating());
    }

    #[test]
    fn key_repeat_tracker_multiple_repeats() {
        let mut tracker = KeyRepeatTracker::new(KeyRepeat::new(100, 50));
        tracker.key_down(0x1E);

        // Jump 300ms in one tick: 100ms delay + 200ms = 4 rate intervals
        // First repeat at 100ms, then at 150, 200, 250, 300 = 1 + 4 = 5
        let repeats = tracker.tick(300_000);
        assert_eq!(repeats, 5);
    }

    #[test]
    fn key_repeat_tracker_release() {
        let mut tracker = KeyRepeatTracker::new(KeyRepeat::new(100, 50));
        tracker.key_down(0x1E);
        tracker.tick(200_000);
        tracker.key_up(0x1E);
        assert_eq!(tracker.held_scancode(), None);
        assert_eq!(tracker.tick(200_000), 0);
    }

    #[test]
    fn key_repeat_tracker_disabled() {
        let mut tracker = KeyRepeatTracker::new(KeyRepeat::disabled());
        tracker.key_down(0x1E);
        assert_eq!(tracker.tick(1_000_000), 0);
    }

    // ── All builtins ────────────────────────────────────────────────────

    #[test]
    fn all_builtin_layouts_returns_five() {
        let all = all_builtin_layouts();
        assert_eq!(all.len(), 5);
        let ids: Vec<&str> = all.iter().map(|l| l.id.as_str()).collect();
        assert!(ids.contains(&"us"));
        assert!(ids.contains(&"uk"));
        assert!(ids.contains(&"de"));
        assert!(ids.contains(&"fr"));
        assert!(ids.contains(&"us-dvorak"));
    }

    #[test]
    fn layout_metadata() {
        let us = layout_us_qwerty();
        assert_eq!(us.language, "en");
        assert_eq!(us.variant, None);

        let dvorak = layout_us_dvorak();
        assert_eq!(dvorak.variant.as_deref(), Some("dvorak"));
        assert_eq!(dvorak.language, "en");

        let de = layout_de_qwertz();
        assert_eq!(de.language, "de");

        let fr = layout_fr_azerty();
        assert_eq!(fr.language, "fr");
    }

    #[test]
    fn uk_qwerty_pound_sign() {
        let layout = layout_uk_qwerty();
        // Shift+3 = £ on UK keyboard
        let key_3 = layout.get(0x04).unwrap();
        assert_eq!(key_3.shift, Some('\u{00a3}'));
        // AltGr+3 = #
        assert_eq!(key_3.alt_gr, Some('#'));
    }

    #[test]
    fn manager_cancel_composition() {
        let mut mgr = KeyboardLayoutManager::new();
        mgr.set_layout("de");
        // Start dead key
        let _ = mgr.translate_scancode(0x29, Modifiers::empty());
        assert!(mgr.is_composing());
        mgr.cancel_composition();
        assert!(!mgr.is_composing());
    }
}
