//! Tests for the new keyboard modules: xkb, repeat_fsm, numpad, compose, accessibility.

#[cfg(test)]
mod tests {
    // ── XKB tests ───────────────────────────────────────────────────────

    mod xkb_tests {
        use crate::xkb::*;

        #[test]
        fn default_keymap_config() {
            let cfg = KeymapConfig::default();
            assert_eq!(cfg.rules, "evdev");
            assert_eq!(cfg.model, "pc105");
            assert_eq!(cfg.layout, "us");
            assert!(cfg.variant.is_empty());
            assert!(cfg.options.is_empty());
        }

        #[test]
        fn compile_keymap_produces_keys() {
            let km = compile_keymap(KeymapConfig::default());
            assert!(km.key_count() > 30, "should have 30+ keycodes mapped");
        }

        #[test]
        fn letter_keysym_lookup_base() {
            let km = compile_keymap(KeymapConfig::default());
            let sym = lookup_keysym(&km, 30, ModifierMask::empty()); // 'a'
            assert_eq!(sym, Some(XK_A));
        }

        #[test]
        fn letter_keysym_lookup_shifted() {
            let km = compile_keymap(KeymapConfig::default());
            let sym = lookup_keysym(&km, 30, ModifierMask::SHIFT); // 'A'
            assert_eq!(sym, Some(XK_A - 0x20)); // uppercase A = 0x41
        }

        #[test]
        fn digit_keysym_lookup() {
            let km = compile_keymap(KeymapConfig::default());
            let sym = lookup_keysym(&km, 2, ModifierMask::empty()); // '1'
            assert_eq!(sym, Some(XK_1));
            let sym_shifted = lookup_keysym(&km, 2, ModifierMask::SHIFT); // '!'
            assert_eq!(sym_shifted, Some(0x0021));
        }

        #[test]
        fn special_key_space() {
            let km = compile_keymap(KeymapConfig::default());
            let sym = lookup_keysym(&km, 57, ModifierMask::empty());
            assert_eq!(sym, Some(XK_SPACE));
        }

        #[test]
        fn special_key_return() {
            let km = compile_keymap(KeymapConfig::default());
            let sym = lookup_keysym(&km, 28, ModifierMask::empty());
            assert_eq!(sym, Some(XK_RETURN));
        }

        #[test]
        fn unknown_keycode_returns_none() {
            let km = compile_keymap(KeymapConfig::default());
            assert_eq!(lookup_keysym(&km, 999, ModifierMask::empty()), None);
        }

        #[test]
        fn modifier_keycodes_detected() {
            let km = compile_keymap(KeymapConfig::default());
            assert!(km.is_modifier(42)); // Left Shift
            assert!(km.is_modifier(54)); // Right Shift
            assert!(km.is_modifier(29)); // Left Ctrl
            assert!(km.is_modifier(56)); // Left Alt
            assert!(!km.is_modifier(30)); // 'a' is not a modifier
        }

        #[test]
        fn lock_modifier_detected() {
            let km = compile_keymap(KeymapConfig::default());
            assert!(km.is_lock_modifier(58)); // Caps Lock
            assert!(km.is_lock_modifier(69)); // Num Lock
            assert!(!km.is_lock_modifier(42)); // Shift is not lock
        }

        #[test]
        fn xkb_state_new_is_empty() {
            let state = XkbState::new();
            assert_eq!(state.effective_modifiers(), ModifierMask::empty());
            assert_eq!(state.depressed(), ModifierMask::empty());
            assert_eq!(state.latched(), ModifierMask::empty());
            assert_eq!(state.locked(), ModifierMask::empty());
        }

        #[test]
        fn xkb_state_shift_press_release() {
            let km = compile_keymap(KeymapConfig::default());
            let mut state = XkbState::new();

            let changes = state.update_key(42, true, &km); // Shift down
            assert_eq!(changes.len(), 1);
            assert_eq!(changes[0].modifier, ModifierMask::SHIFT);
            assert!(changes[0].active);
            assert_eq!(changes[0].kind, ModifierChangeKind::Depressed);
            assert!(state.effective_modifiers().contains(ModifierMask::SHIFT));

            let changes = state.update_key(42, false, &km); // Shift up
            assert_eq!(changes.len(), 1);
            assert!(!changes[0].active);
            assert!(!state.effective_modifiers().contains(ModifierMask::SHIFT));
        }

        #[test]
        fn xkb_state_caps_lock_toggle() {
            let km = compile_keymap(KeymapConfig::default());
            let mut state = XkbState::new();

            // Press Caps Lock -> locked
            let changes = state.update_key(58, true, &km);
            assert_eq!(changes.len(), 1);
            assert!(changes[0].active);
            assert_eq!(changes[0].kind, ModifierChangeKind::Locked);
            assert!(state.locked().contains(ModifierMask::LOCK));

            // Release Caps Lock -> stays locked
            let changes = state.update_key(58, false, &km);
            assert!(changes.is_empty());
            assert!(state.locked().contains(ModifierMask::LOCK));

            // Press again -> unlocked
            let changes = state.update_key(58, true, &km);
            assert_eq!(changes.len(), 1);
            assert!(!changes[0].active);
            assert!(!state.locked().contains(ModifierMask::LOCK));
        }

        #[test]
        fn xkb_state_effective_combines_all() {
            let km = compile_keymap(KeymapConfig::default());
            let mut state = XkbState::new();

            state.update_key(58, true, &km); // Caps Lock on
            state.update_key(42, true, &km); // Shift down
            state.latch(ModifierMask::MOD1); // Latch Alt

            let eff = state.effective_modifiers();
            assert!(eff.contains(ModifierMask::LOCK));
            assert!(eff.contains(ModifierMask::SHIFT));
            assert!(eff.contains(ModifierMask::MOD1));
        }

        #[test]
        fn xkb_state_clear_latches() {
            let mut state = XkbState::new();
            state.latch(ModifierMask::SHIFT);
            assert!(state.latched().contains(ModifierMask::SHIFT));
            state.clear_latches();
            assert_eq!(state.latched(), ModifierMask::empty());
        }

        #[test]
        fn xkb_state_reset() {
            let km = compile_keymap(KeymapConfig::default());
            let mut state = XkbState::new();
            state.update_key(42, true, &km);
            state.update_key(58, true, &km);
            state.latch(ModifierMask::MOD1);
            state.reset();
            assert_eq!(state.effective_modifiers(), ModifierMask::empty());
        }

        #[test]
        fn non_modifier_key_produces_no_changes() {
            let km = compile_keymap(KeymapConfig::default());
            let mut state = XkbState::new();
            let changes = state.update_key(30, true, &km); // 'a'
            assert!(changes.is_empty());
        }

        #[test]
        fn keysym_with_caps_lock() {
            let km = compile_keymap(KeymapConfig::default());
            // LOCK modifier selects shifted level for letters.
            let sym = lookup_keysym(&km, 30, ModifierMask::LOCK);
            assert_eq!(sym, Some(XK_A - 0x20)); // Uppercase A
        }

        #[test]
        fn keymap_set_entry() {
            let mut km = compile_keymap(KeymapConfig::default());
            let count_before = km.key_count();
            km.set_entry(200, KeySymEntry { levels: [0x41, 0x42, 0x43, 0x44] });
            assert_eq!(km.key_count(), count_before + 1);
            assert_eq!(lookup_keysym(&km, 200, ModifierMask::empty()), Some(0x41));
        }
    }

    // ── Repeat FSM tests ────────────────────────────────────────────────

    mod repeat_fsm_tests {
        use crate::repeat_fsm::*;
        use crate::xkb::compile_keymap;
        use crate::xkb::KeymapConfig;

        #[test]
        fn default_config() {
            let cfg = RepeatConfig::default();
            assert_eq!(cfg.delay_ms, 500);
            assert_eq!(cfg.interval_ms, 33);
            assert!(!cfg.is_disabled());
        }

        #[test]
        fn disabled_config() {
            let cfg = RepeatConfig::new(500, 0);
            assert!(cfg.is_disabled());
        }

        #[test]
        fn idle_state_produces_no_events() {
            let mut state = RepeatState::new(RepeatConfig::default());
            assert!(state.is_idle());
            assert!(state.tick(1000).is_empty());
        }

        #[test]
        fn key_down_starts_delay() {
            let mut state = RepeatState::new(RepeatConfig::default());
            let action = state.key_down(30, None);
            assert_eq!(action, Some(RepeatAction::StartDelay(30)));
            assert!(!state.is_idle());
            assert!(!state.is_repeating());
            assert_eq!(state.active_keycode(), Some(30));
        }

        #[test]
        fn key_up_cancels() {
            let mut state = RepeatState::new(RepeatConfig::default());
            state.key_down(30, None);
            let action = state.key_up(30);
            assert_eq!(action, Some(RepeatAction::Cancel));
            assert!(state.is_idle());
        }

        #[test]
        fn key_up_wrong_key_ignored() {
            let mut state = RepeatState::new(RepeatConfig::default());
            state.key_down(30, None);
            let action = state.key_up(31);
            assert_eq!(action, None);
            assert!(!state.is_idle());
        }

        #[test]
        fn tick_before_delay_no_repeat() {
            let mut state = RepeatState::new(RepeatConfig::new(100, 50));
            state.key_down(30, None);
            let actions = state.tick(50);
            assert!(actions.is_empty());
        }

        #[test]
        fn tick_past_delay_produces_repeat() {
            let mut state = RepeatState::new(RepeatConfig::new(100, 50));
            state.key_down(30, None);
            let actions = state.tick(110);
            assert_eq!(actions.len(), 1);
            assert_eq!(actions[0], RepeatAction::Repeat(30));
            assert!(state.is_repeating());
        }

        #[test]
        fn tick_produces_multiple_repeats() {
            let mut state = RepeatState::new(RepeatConfig::new(100, 50));
            state.key_down(30, None);
            // 300ms: 100ms delay + 200ms = 4 intervals of 50ms.
            // First repeat at 100ms, then 150, 200, 250, 300 = 1 + 4 = 5
            let actions = state.tick(300);
            assert_eq!(actions.len(), 5);
            for a in &actions {
                assert_eq!(*a, RepeatAction::Repeat(30));
            }
        }

        #[test]
        fn ongoing_repeat_ticks() {
            let mut state = RepeatState::new(RepeatConfig::new(100, 50));
            state.key_down(30, None);
            state.tick(110); // past delay, 1 repeat
            let actions = state.tick(100); // 100ms more = 2 repeats
            assert_eq!(actions.len(), 2);
        }

        #[test]
        fn modifier_keys_do_not_repeat() {
            let km = compile_keymap(KeymapConfig::default());
            let mut state = RepeatState::new(RepeatConfig::default());
            let action = state.key_down(42, Some(&km)); // Left Shift
            assert_eq!(action, None);
            assert!(state.is_idle());
        }

        #[test]
        fn new_key_replaces_old() {
            let mut state = RepeatState::new(RepeatConfig::new(100, 50));
            state.key_down(30, None);
            state.tick(60); // 60ms into delay for key 30
            state.key_down(31, None); // new key replaces
            assert_eq!(state.active_keycode(), Some(31));
            // 50ms tick — not past the 100ms delay for new key
            let actions = state.tick(50);
            assert!(actions.is_empty());
        }

        #[test]
        fn disabled_repeat_no_events() {
            let mut state = RepeatState::new(RepeatConfig::new(100, 0));
            let action = state.key_down(30, None);
            assert_eq!(action, None); // disabled
            let actions = state.tick(1000);
            assert!(actions.is_empty());
        }

        #[test]
        fn set_config_resets_state() {
            let mut state = RepeatState::new(RepeatConfig::new(100, 50));
            state.key_down(30, None);
            state.tick(200);
            state.set_config(RepeatConfig::new(200, 25));
            assert!(state.is_idle());
        }
    }

    // ── Numpad tests ────────────────────────────────────────────────────

    mod numpad_tests {
        use crate::numpad::*;

        #[test]
        fn numlock_on_produces_digits() {
            assert_eq!(numpad_translate(KP_0, true), NumpadOutput::Char('0'));
            assert_eq!(numpad_translate(KP_1, true), NumpadOutput::Char('1'));
            assert_eq!(numpad_translate(KP_5, true), NumpadOutput::Char('5'));
            assert_eq!(numpad_translate(KP_9, true), NumpadOutput::Char('9'));
            assert_eq!(numpad_translate(KP_DECIMAL, true), NumpadOutput::Char('.'));
        }

        #[test]
        fn numlock_off_produces_nav() {
            assert_eq!(numpad_translate(KP_0, false), NumpadOutput::NavigationKey(NavKey::Insert));
            assert_eq!(numpad_translate(KP_1, false), NumpadOutput::NavigationKey(NavKey::End));
            assert_eq!(numpad_translate(KP_2, false), NumpadOutput::NavigationKey(NavKey::Down));
            assert_eq!(numpad_translate(KP_3, false), NumpadOutput::NavigationKey(NavKey::PageDown));
            assert_eq!(numpad_translate(KP_4, false), NumpadOutput::NavigationKey(NavKey::Left));
            assert_eq!(numpad_translate(KP_6, false), NumpadOutput::NavigationKey(NavKey::Right));
            assert_eq!(numpad_translate(KP_7, false), NumpadOutput::NavigationKey(NavKey::Home));
            assert_eq!(numpad_translate(KP_8, false), NumpadOutput::NavigationKey(NavKey::Up));
            assert_eq!(numpad_translate(KP_9, false), NumpadOutput::NavigationKey(NavKey::PageUp));
            assert_eq!(numpad_translate(KP_DECIMAL, false), NumpadOutput::NavigationKey(NavKey::Delete));
        }

        #[test]
        fn kp5_numlock_off_is_none() {
            assert_eq!(numpad_translate(KP_5, false), NumpadOutput::None);
        }

        #[test]
        fn operator_keys_always_chars() {
            // Regardless of NumLock state.
            assert_eq!(numpad_translate(KP_ADD, true), NumpadOutput::Char('+'));
            assert_eq!(numpad_translate(KP_ADD, false), NumpadOutput::Char('+'));
            assert_eq!(numpad_translate(KP_SUBTRACT, true), NumpadOutput::Char('-'));
            assert_eq!(numpad_translate(KP_SUBTRACT, false), NumpadOutput::Char('-'));
            assert_eq!(numpad_translate(KP_MULTIPLY, true), NumpadOutput::Char('*'));
            assert_eq!(numpad_translate(KP_DIVIDE, false), NumpadOutput::Char('/'));
            assert_eq!(numpad_translate(KP_ENTER, true), NumpadOutput::Char('\n'));
        }

        #[test]
        fn non_numpad_key_is_none() {
            assert_eq!(numpad_translate(30, true), NumpadOutput::None);
            assert_eq!(numpad_translate(30, false), NumpadOutput::None);
        }

        #[test]
        fn numpad_state_default_numlock_on() {
            let state = NumpadState::new();
            assert!(state.num_lock);
            assert_eq!(state.translate(KP_7), NumpadOutput::Char('7'));
        }

        #[test]
        fn numpad_state_toggle() {
            let mut state = NumpadState::new();
            state.toggle_num_lock();
            assert!(!state.num_lock);
            assert_eq!(state.translate(KP_7), NumpadOutput::NavigationKey(NavKey::Home));
            state.toggle_num_lock();
            assert!(state.num_lock);
            assert_eq!(state.translate(KP_7), NumpadOutput::Char('7'));
        }

        #[test]
        fn numpad_state_with_numlock() {
            let state = NumpadState::with_num_lock(false);
            assert!(!state.num_lock);
        }
    }

    // ── Compose tests ───────────────────────────────────────────────────

    mod compose_tests {
        use crate::compose::*;

        #[test]
        fn empty_table() {
            let table = ComposeTable::new();
            assert_eq!(table.sequence_count(), 0);
        }

        #[test]
        fn default_table_has_sequences() {
            let table = ComposeTable::with_defaults();
            assert!(table.sequence_count() >= 50, "should have 50+ sequences, got {}", table.sequence_count());
        }

        #[test]
        fn add_custom_sequence() {
            let mut table = ComposeTable::new();
            table.add_sequence(&[0x41, 0x42], "custom");
            assert_eq!(table.sequence_count(), 1);
        }

        #[test]
        fn feed_nothing_when_no_match() {
            let mut state = ComposeState::new(ComposeTable::new());
            assert_eq!(state.feed(0x41), ComposeStatus::Nothing);
            assert!(!state.is_composing());
        }

        #[test]
        fn feed_composing_then_composed() {
            let mut table = ComposeTable::new();
            table.add_sequence(&[0x41, 0x42], "result");
            let mut state = ComposeState::new(table);

            let s1 = state.feed(0x41);
            assert_eq!(s1, ComposeStatus::Composing);
            assert!(state.is_composing());

            let s2 = state.feed(0x42);
            assert_eq!(s2, ComposeStatus::Composed("result".to_string()));
            assert!(!state.is_composing());
        }

        #[test]
        fn feed_cancelled_on_bad_continuation() {
            let mut table = ComposeTable::new();
            table.add_sequence(&[0x41, 0x42], "result");
            let mut state = ComposeState::new(table);

            state.feed(0x41); // start sequence
            let s = state.feed(0x99); // invalid continuation
            assert_eq!(s, ComposeStatus::Cancelled);
            assert!(!state.is_composing());
        }

        #[test]
        fn compose_acute_e() {
            let mut state = ComposeState::with_defaults();
            // Compose ' + e = é
            let s1 = state.feed(0x0027); // '
            assert_eq!(s1, ComposeStatus::Composing);
            let s2 = state.feed(0x0065); // e
            assert_eq!(s2, ComposeStatus::Composed("\u{00e9}".to_string()));
        }

        #[test]
        fn compose_grave_a() {
            let mut state = ComposeState::with_defaults();
            let s1 = state.feed(0x0060); // `
            assert_eq!(s1, ComposeStatus::Composing);
            let s2 = state.feed(0x0061); // a
            assert_eq!(s2, ComposeStatus::Composed("\u{00e0}".to_string()));
        }

        #[test]
        fn compose_circumflex_o() {
            let mut state = ComposeState::with_defaults();
            state.feed(0x005e); // ^
            let s = state.feed(0x006f); // o
            assert_eq!(s, ComposeStatus::Composed("\u{00f4}".to_string()));
        }

        #[test]
        fn compose_tilde_n() {
            let mut state = ComposeState::with_defaults();
            state.feed(0x007e); // ~
            let s = state.feed(0x006e); // n
            assert_eq!(s, ComposeStatus::Composed("\u{00f1}".to_string()));
        }

        #[test]
        fn compose_umlaut_u() {
            let mut state = ComposeState::with_defaults();
            state.feed(0x0022); // "
            let s = state.feed(0x0075); // u
            assert_eq!(s, ComposeStatus::Composed("\u{00fc}".to_string()));
        }

        #[test]
        fn compose_cedilla_c() {
            let mut state = ComposeState::with_defaults();
            state.feed(0x002c); // ,
            let s = state.feed(0x0063); // c
            assert_eq!(s, ComposeStatus::Composed("\u{00e7}".to_string()));
        }

        #[test]
        fn compose_euro_sign() {
            let mut state = ComposeState::with_defaults();
            state.feed(0x003d); // =
            let s = state.feed(0x0065); // e
            assert_eq!(s, ComposeStatus::Composed("\u{20ac}".to_string()));
        }

        #[test]
        fn compose_ellipsis() {
            let mut state = ComposeState::with_defaults();
            state.feed(0x002e); // .
            let s = state.feed(0x002e); // .
            assert_eq!(s, ComposeStatus::Composed("\u{2026}".to_string()));
        }

        #[test]
        fn compose_reset() {
            let mut state = ComposeState::with_defaults();
            state.feed(0x0027); // start composing
            assert!(state.is_composing());
            state.reset();
            assert!(!state.is_composing());
        }

        #[test]
        fn compose_table_replace_sequence() {
            let mut table = ComposeTable::new();
            table.add_sequence(&[0x41, 0x42], "first");
            table.add_sequence(&[0x41, 0x42], "replaced");
            assert_eq!(table.sequence_count(), 1);
            let mut state = ComposeState::new(table);
            state.feed(0x41);
            let s = state.feed(0x42);
            assert_eq!(s, ComposeStatus::Composed("replaced".to_string()));
        }

        #[test]
        fn compose_empty_sequence_ignored() {
            let mut table = ComposeTable::new();
            table.add_sequence(&[], "nope");
            assert_eq!(table.sequence_count(), 0);
        }

        #[test]
        fn table_mut_add_custom() {
            let mut state = ComposeState::with_defaults();
            let count_before = state.table().sequence_count();
            state.table_mut().add_sequence(&[0xAA, 0xBB], "custom");
            assert_eq!(state.table().sequence_count(), count_before + 1);
        }
    }

    // ── Accessibility tests ─────────────────────────────────────────────

    mod accessibility_tests {
        use crate::accessibility::*;
        use crate::xkb::ModifierMask;

        // -- StickyKeys --

        #[test]
        fn sticky_keys_initial_state() {
            let sk = StickyKeys::new();
            assert_eq!(sk.latched(), ModifierMask::empty());
        }

        #[test]
        fn sticky_keys_standalone_modifier_latches() {
            let mut sk = StickyKeys::new();
            sk.modifier_down(ModifierMask::SHIFT);
            let result = sk.modifier_up(ModifierMask::SHIFT);
            assert_eq!(result, Some(ModifierMask::SHIFT));
            assert!(sk.latched().contains(ModifierMask::SHIFT));
        }

        #[test]
        fn sticky_keys_consumed_on_key() {
            let mut sk = StickyKeys::new();
            sk.modifier_down(ModifierMask::SHIFT);
            sk.modifier_up(ModifierMask::SHIFT);
            let consumed = sk.consume_on_key();
            assert!(consumed.contains(ModifierMask::SHIFT));
            assert_eq!(sk.latched(), ModifierMask::empty());
        }

        #[test]
        fn sticky_keys_modifier_used_with_key_does_not_latch() {
            let mut sk = StickyKeys::new();
            sk.modifier_down(ModifierMask::SHIFT);
            sk.consume_on_key(); // key pressed while modifier held
            let result = sk.modifier_up(ModifierMask::SHIFT);
            assert_eq!(result, None); // was used, not standalone
        }

        #[test]
        fn sticky_keys_double_tap_unlatch() {
            let mut sk = StickyKeys::new();
            // First tap: latches.
            sk.modifier_down(ModifierMask::SHIFT);
            sk.modifier_up(ModifierMask::SHIFT);
            assert!(sk.latched().contains(ModifierMask::SHIFT));
            // Second tap: unlatches.
            sk.modifier_down(ModifierMask::SHIFT);
            let result = sk.modifier_up(ModifierMask::SHIFT);
            assert_eq!(result, None);
            assert_eq!(sk.latched(), ModifierMask::empty());
        }

        #[test]
        fn sticky_keys_reset() {
            let mut sk = StickyKeys::new();
            sk.modifier_down(ModifierMask::CONTROL);
            sk.modifier_up(ModifierMask::CONTROL);
            sk.reset();
            assert_eq!(sk.latched(), ModifierMask::empty());
        }

        // -- SlowKeys --

        #[test]
        fn slow_keys_zero_threshold_accepts() {
            let mut sk = SlowKeys::new(0);
            assert_eq!(sk.key_down(30), KeyDecision::Accept);
        }

        #[test]
        fn slow_keys_nonzero_delays() {
            let mut sk = SlowKeys::new(200);
            assert_eq!(sk.key_down(30), KeyDecision::Delay(200));
        }

        #[test]
        fn slow_keys_tick_accepts_after_threshold() {
            let mut sk = SlowKeys::new(200);
            sk.key_down(30);
            let accepted = sk.tick(100);
            assert!(accepted.is_empty()); // not yet
            let accepted = sk.tick(150);
            assert_eq!(accepted, vec![30]); // past 200ms
        }

        #[test]
        fn slow_keys_release_before_threshold_rejects() {
            let mut sk = SlowKeys::new(200);
            sk.key_down(30);
            sk.tick(100);
            let decision = sk.key_up(30);
            assert_eq!(decision, KeyDecision::Reject);
        }

        #[test]
        fn slow_keys_release_after_accept() {
            let mut sk = SlowKeys::new(200);
            sk.key_down(30);
            sk.tick(250);
            let decision = sk.key_up(30);
            assert_eq!(decision, KeyDecision::Accept);
        }

        #[test]
        fn slow_keys_reset() {
            let mut sk = SlowKeys::new(200);
            sk.key_down(30);
            sk.reset();
            // After reset, key_up should reject (not in accepted set).
            assert_eq!(sk.key_up(30), KeyDecision::Reject);
        }

        #[test]
        fn slow_keys_set_threshold() {
            let mut sk = SlowKeys::new(200);
            sk.set_threshold(400);
            assert_eq!(sk.threshold_ms(), 400);
        }

        // -- BounceKeys --

        #[test]
        fn bounce_keys_first_press_accepted() {
            let mut bk = BounceKeys::new(100);
            assert_eq!(bk.key_down(30), KeyDecision::Accept);
        }

        #[test]
        fn bounce_keys_rapid_repress_rejected() {
            let mut bk = BounceKeys::new(100);
            bk.key_down(30);
            bk.key_up(30); // release at time 0
            bk.tick(50);   // 50ms later
            assert_eq!(bk.key_down(30), KeyDecision::Reject); // too soon
        }

        #[test]
        fn bounce_keys_slow_repress_accepted() {
            let mut bk = BounceKeys::new(100);
            bk.key_down(30);
            bk.key_up(30);
            bk.tick(150); // past debounce window
            assert_eq!(bk.key_down(30), KeyDecision::Accept);
        }

        #[test]
        fn bounce_keys_different_key_accepted() {
            let mut bk = BounceKeys::new(100);
            bk.key_down(30);
            bk.key_up(30);
            bk.tick(10);
            assert_eq!(bk.key_down(31), KeyDecision::Accept); // different key
        }

        #[test]
        fn bounce_keys_reset() {
            let mut bk = BounceKeys::new(100);
            bk.key_down(30);
            bk.key_up(30);
            bk.reset();
            assert_eq!(bk.key_down(30), KeyDecision::Accept); // no history
        }

        #[test]
        fn bounce_keys_set_interval() {
            let mut bk = BounceKeys::new(100);
            bk.set_interval(200);
            assert_eq!(bk.interval_ms(), 200);
        }

        // -- MouseKeys --

        #[test]
        fn mouse_keys_direction_move() {
            let mut mk = MouseKeys::new(10, 40, 20);
            let action = mk.key_down(72); // KP_8 = up
            assert_eq!(action, MouseKeyAction::Move(0, -10));
        }

        #[test]
        fn mouse_keys_diagonal() {
            let mut mk = MouseKeys::new(10, 40, 20);
            let action = mk.key_down(73); // KP_9 = up-right
            assert_eq!(action, MouseKeyAction::Move(10, -10));
        }

        #[test]
        fn mouse_keys_click() {
            let mut mk = MouseKeys::new(10, 40, 20);
            let action = mk.key_down(76); // KP_5 = click
            assert_eq!(action, MouseKeyAction::ButtonPress(MouseButton::Left));
            let action = mk.key_up(76);
            assert_eq!(action, MouseKeyAction::ButtonRelease(MouseButton::Left));
        }

        #[test]
        fn mouse_keys_button_cycle() {
            let mut mk = MouseKeys::new(10, 40, 20);
            assert_eq!(mk.selected_button(), MouseButton::Left);
            mk.key_down(82); // KP_0 = cycle button
            assert_eq!(mk.selected_button(), MouseButton::Middle);
            mk.key_down(82);
            assert_eq!(mk.selected_button(), MouseButton::Right);
            mk.key_down(82);
            assert_eq!(mk.selected_button(), MouseButton::Left);
        }

        #[test]
        fn mouse_keys_tick_acceleration() {
            let mut mk = MouseKeys::new(10, 40, 20);
            mk.key_down(77); // KP_6 = right
            // First tick: speed should be around step (10).
            let delta = mk.tick();
            assert!(delta.is_some());
            let (dx, _dy) = delta.unwrap();
            assert!(dx > 0);
        }

        #[test]
        fn mouse_keys_tick_no_keys_returns_none() {
            let mut mk = MouseKeys::new(10, 40, 20);
            assert_eq!(mk.tick(), None);
        }

        #[test]
        fn mouse_keys_release_stops_movement() {
            let mut mk = MouseKeys::new(10, 40, 20);
            mk.key_down(77); // right
            mk.key_up(77);
            assert_eq!(mk.tick(), None);
        }

        #[test]
        fn mouse_keys_is_mouse_key() {
            assert!(MouseKeys::is_mouse_key(72)); // KP_8
            assert!(MouseKeys::is_mouse_key(76)); // KP_5
            assert!(MouseKeys::is_mouse_key(82)); // KP_0
            assert!(!MouseKeys::is_mouse_key(30)); // 'a'
        }

        #[test]
        fn mouse_keys_non_mouse_key_returns_none() {
            let mut mk = MouseKeys::new(10, 40, 20);
            assert_eq!(mk.key_down(30), MouseKeyAction::None);
        }

        #[test]
        fn mouse_keys_reset() {
            let mut mk = MouseKeys::new(10, 40, 20);
            mk.key_down(82); // cycle to middle
            mk.key_down(77); // hold right
            mk.reset();
            assert_eq!(mk.selected_button(), MouseButton::Left);
            assert_eq!(mk.tick(), None);
        }

        // -- process_key integration --

        #[test]
        fn process_key_all_disabled_accepts() {
            let config = AccessibilityConfig::default();
            let mut sticky = StickyKeys::new();
            let mut slow = SlowKeys::new(config.slow_keys_threshold_ms);
            let mut bounce = BounceKeys::new(config.bounce_keys_interval_ms);

            let d = process_key(30, true, false, None, &config, &mut sticky, &mut slow, &mut bounce);
            assert_eq!(d, KeyDecision::Accept);
        }

        #[test]
        fn process_key_bounce_rejects() {
            let mut config = AccessibilityConfig::default();
            config.bounce_keys_enabled = true;
            config.bounce_keys_interval_ms = 100;
            let mut sticky = StickyKeys::new();
            let mut slow = SlowKeys::new(config.slow_keys_threshold_ms);
            let mut bounce = BounceKeys::new(config.bounce_keys_interval_ms);

            // First press accepted.
            let d = process_key(30, true, false, None, &config, &mut sticky, &mut slow, &mut bounce);
            assert_eq!(d, KeyDecision::Accept);
            // Release.
            process_key(30, false, false, None, &config, &mut sticky, &mut slow, &mut bounce);
            bounce.tick(10);
            // Rapid repress rejected.
            let d = process_key(30, true, false, None, &config, &mut sticky, &mut slow, &mut bounce);
            assert_eq!(d, KeyDecision::Reject);
        }

        #[test]
        fn process_key_slow_delays() {
            let mut config = AccessibilityConfig::default();
            config.slow_keys_enabled = true;
            config.slow_keys_threshold_ms = 200;
            let mut sticky = StickyKeys::new();
            let mut slow = SlowKeys::new(config.slow_keys_threshold_ms);
            let mut bounce = BounceKeys::new(config.bounce_keys_interval_ms);

            let d = process_key(30, true, false, None, &config, &mut sticky, &mut slow, &mut bounce);
            assert_eq!(d, KeyDecision::Delay(200));
        }

        #[test]
        fn process_key_sticky_latches_modifier() {
            let mut config = AccessibilityConfig::default();
            config.sticky_keys_enabled = true;
            let mut sticky = StickyKeys::new();
            let mut slow = SlowKeys::new(config.slow_keys_threshold_ms);
            let mut bounce = BounceKeys::new(config.bounce_keys_interval_ms);

            // Modifier down.
            process_key(42, true, true, Some(ModifierMask::SHIFT), &config, &mut sticky, &mut slow, &mut bounce);
            // Modifier up (standalone).
            let d = process_key(42, false, true, Some(ModifierMask::SHIFT), &config, &mut sticky, &mut slow, &mut bounce);
            assert_eq!(d, KeyDecision::ModifierSticky(ModifierMask::SHIFT));
            // Non-modifier key consumes.
            let d = process_key(30, true, false, None, &config, &mut sticky, &mut slow, &mut bounce);
            assert_eq!(d, KeyDecision::ModifierSticky(ModifierMask::SHIFT));
        }

        #[test]
        fn accessibility_config_default() {
            let cfg = AccessibilityConfig::default();
            assert!(!cfg.sticky_keys_enabled);
            assert!(!cfg.slow_keys_enabled);
            assert_eq!(cfg.slow_keys_threshold_ms, 300);
            assert!(!cfg.bounce_keys_enabled);
            assert_eq!(cfg.bounce_keys_interval_ms, 300);
            assert!(!cfg.mouse_keys_enabled);
            assert_eq!(cfg.mouse_keys_step, 10);
        }
    }
}
