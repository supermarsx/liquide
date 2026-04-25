//! Tests for the input method framework.

use crate::candidates::{Candidate, compute_layout, hit_test_candidate};
use crate::compose::{ComposeResult, ComposeTable, default_compose_table};
use crate::emoji::{EmojiCategory, EmojiPicker};
use crate::engine::{InputAction, InputMethodEngine, KeyEvent};
use crate::state::{InputMethodState, InputMode, PreeditSegment, PreeditString, SegmentStyle};

// ===== PreeditString tests =====

#[test]
fn preedit_empty() {
    let p = PreeditString::empty();
    assert!(p.is_empty());
    assert_eq!(p.len(), 0);
    assert_eq!(p.cursor_pos, 0);
    assert!(p.segments.is_empty());
}

#[test]
fn preedit_new_with_text() {
    let p = PreeditString::new("hello");
    assert!(!p.is_empty());
    assert_eq!(p.text, "hello");
    assert_eq!(p.cursor_pos, 5);
    assert_eq!(p.segments.len(), 1);
    assert_eq!(p.segments[0].style, SegmentStyle::Underline);
    assert_eq!(p.segments[0].start, 0);
    assert_eq!(p.segments[0].end, 5);
}

#[test]
fn preedit_push_pop() {
    let mut p = PreeditString::empty();
    p.push('a');
    p.push('b');
    assert_eq!(p.text, "ab");
    assert_eq!(p.cursor_pos, 2);

    let ch = p.pop();
    assert_eq!(ch, Some('b'));
    assert_eq!(p.text, "a");
    assert_eq!(p.cursor_pos, 1);
}

#[test]
fn preedit_push_str_and_clear() {
    let mut p = PreeditString::empty();
    p.push_str("test");
    assert_eq!(p.len(), 4);
    p.clear();
    assert!(p.is_empty());
    assert!(p.segments.is_empty());
}

#[test]
fn preedit_set_text() {
    let mut p = PreeditString::new("old");
    p.set_text("new text");
    assert_eq!(p.text, "new text");
    assert_eq!(p.cursor_pos, 8);
}

// ===== PreeditSegment tests =====

#[test]
fn preedit_segment_styles() {
    let s = PreeditSegment::new(0, 5, SegmentStyle::Selected);
    assert_eq!(s.start, 0);
    assert_eq!(s.end, 5);
    assert_eq!(s.style, SegmentStyle::Selected);

    // All styles are distinct.
    assert_ne!(SegmentStyle::None, SegmentStyle::Underline);
    assert_ne!(SegmentStyle::Underline, SegmentStyle::ThickUnderline);
    assert_ne!(SegmentStyle::ThickUnderline, SegmentStyle::Selected);
}

// ===== InputMethodState tests =====

#[test]
fn state_default() {
    let s = InputMethodState::new();
    assert!(!s.active);
    assert!(!s.is_composing());
    assert!(!s.has_candidates());
    assert_eq!(s.mode, InputMode::Direct);
    assert!(s.selected().is_none());
}

#[test]
fn state_with_candidates() {
    let mut s = InputMethodState::new();
    s.candidates = vec![Candidate::new("A"), Candidate::new("B")];
    s.selected_candidate = 1;
    assert!(s.has_candidates());
    assert_eq!(s.selected().unwrap().text, "B");
}

#[test]
fn state_reset() {
    let mut s = InputMethodState::new();
    s.preedit.push_str("test");
    s.candidates = vec![Candidate::new("X")];
    s.selected_candidate = 0;
    s.reset();
    assert!(s.preedit.is_empty());
    assert!(s.candidates.is_empty());
    assert_eq!(s.selected_candidate, 0);
}

// ===== InputMode tests =====

#[test]
fn input_mode_labels() {
    assert_eq!(InputMode::Direct.label(), "A");
    assert_eq!(InputMode::Hiragana.label(), "\u{3042}");
    assert_eq!(InputMode::Katakana.label(), "\u{30A2}");
    assert_eq!(InputMode::Pinyin.label(), "\u{62FC}");
    assert_eq!(InputMode::Compose.label(), "Co");
    assert_eq!(InputMode::DeadKey.label(), "DK");
    assert_eq!(InputMode::Romaji.label(), "Ro");
}

// ===== ComposeTable tests =====

#[test]
fn compose_table_basic() {
    let mut t = ComposeTable::new();
    t.add_sequence(vec![0x01, 0x02], 'X');
    assert_eq!(t.len(), 1);
    assert!(!t.is_empty());
}

#[test]
fn compose_sequence_match() {
    let mut t =
        ComposeTable::from_sequences(vec![(vec![0x10, 0x20], 'A'), (vec![0x10, 0x30], 'B')]);

    assert_eq!(t.feed_key(0x10), ComposeResult::Composing);
    assert!(t.is_composing());
    assert_eq!(t.feed_key(0x20), ComposeResult::Committed('A'));
    assert!(!t.is_composing());
}

#[test]
fn compose_sequence_cancel() {
    let mut t = ComposeTable::from_sequences(vec![(vec![0x10, 0x20], 'A')]);

    assert_eq!(t.feed_key(0x10), ComposeResult::Composing);
    assert_eq!(t.feed_key(0xFF), ComposeResult::Cancelled);
    assert!(!t.is_composing());
}

#[test]
fn compose_reset() {
    let mut t = ComposeTable::from_sequences(vec![(vec![0x10, 0x20], 'A')]);
    t.feed_key(0x10);
    assert!(t.is_composing());
    t.reset();
    assert!(!t.is_composing());
}

#[test]
fn default_compose_table_has_entries() {
    let t = default_compose_table();
    assert!(t.len() >= 50);
}

#[test]
fn compose_dead_acute_a() {
    let mut t = default_compose_table();
    // dead_acute (0xfe51) + 'a' (0x61) -> á
    assert_eq!(t.feed_key(0xfe51), ComposeResult::Composing);
    assert_eq!(t.feed_key(0x0061), ComposeResult::Committed('\u{00e1}'));
}

#[test]
fn compose_dead_diaeresis_u() {
    let mut t = default_compose_table();
    // dead_diaeresis (0xfe57) + 'u' (0x75) -> ü
    assert_eq!(t.feed_key(0xfe57), ComposeResult::Composing);
    assert_eq!(t.feed_key(0x0075), ComposeResult::Committed('\u{00fc}'));
}

#[test]
fn compose_dead_tilde_n() {
    let mut t = default_compose_table();
    // dead_tilde (0xfe53) + 'n' (0x6e) -> ñ
    assert_eq!(t.feed_key(0xfe53), ComposeResult::Composing);
    assert_eq!(t.feed_key(0x006e), ComposeResult::Committed('\u{00f1}'));
}

#[test]
fn compose_multi_key_euro() {
    let mut t = default_compose_table();
    // Multi_key (0xff20) + 'e' (0x65) + '=' (0x3d) -> €
    assert_eq!(t.feed_key(0xff20), ComposeResult::Composing);
    assert_eq!(t.feed_key(0x0065), ComposeResult::Composing);
    assert_eq!(t.feed_key(0x003d), ComposeResult::Committed('\u{20ac}'));
}

// ===== Candidate tests =====

#[test]
fn candidate_creation() {
    let c = Candidate::new("test");
    assert_eq!(c.text, "test");
    assert!(c.label.is_none());
    assert!(c.annotation.is_none());

    let c2 = Candidate::with_label("test", "1").annotated("meaning");
    assert_eq!(c2.label.as_deref(), Some("1"));
    assert_eq!(c2.annotation.as_deref(), Some("meaning"));
}

#[test]
fn candidate_layout_empty() {
    let layout = compute_layout(&[], 0, 100.0, 200.0, 10);
    assert!(layout.items.is_empty());
    assert_eq!(layout.width, 0.0);
}

#[test]
fn candidate_layout_basic() {
    let candidates = vec![
        Candidate::new("A"),
        Candidate::new("BB"),
        Candidate::new("CCC"),
    ];
    let layout = compute_layout(&candidates, 1, 50.0, 100.0, 10);
    assert_eq!(layout.items.len(), 3);
    assert_eq!(layout.x, 50.0);
    assert_eq!(layout.y, 100.0);
    assert!(layout.width > 0.0);
    assert!(layout.height > 0.0);

    // Second item should be selected.
    assert!(!layout.items[0].selected);
    assert!(layout.items[1].selected);
    assert!(!layout.items[2].selected);
}

#[test]
fn candidate_layout_max_visible() {
    let candidates: Vec<Candidate> = (0..20)
        .map(|i| Candidate::new(format!("item{}", i)))
        .collect();
    let layout = compute_layout(&candidates, 0, 0.0, 0.0, 5);
    assert_eq!(layout.items.len(), 5);
}

#[test]
fn candidate_hit_test() {
    let candidates = vec![
        Candidate::new("A"),
        Candidate::new("B"),
        Candidate::new("C"),
    ];
    let layout = compute_layout(&candidates, 0, 0.0, 0.0, 10);

    // Hit the first item.
    let item = &layout.items[0];
    let hit = hit_test_candidate(&layout, item.x + 1.0, item.y + 1.0);
    assert_eq!(hit, Some(0));

    // Hit the third item.
    let item2 = &layout.items[2];
    let hit2 = hit_test_candidate(&layout, item2.x + 1.0, item2.y + 1.0);
    assert_eq!(hit2, Some(2));

    // Miss entirely (below all items).
    let miss = hit_test_candidate(&layout, 0.0, layout.height + 100.0);
    assert_eq!(miss, None);
}

// ===== EmojiPicker tests =====

#[test]
fn emoji_picker_default_table() {
    let picker = EmojiPicker::new();
    assert!(picker.len() >= 100);
    assert!(!picker.is_empty());
}

#[test]
fn emoji_search_smile() {
    let picker = EmojiPicker::new();
    let results = picker.search("smile");
    assert!(!results.is_empty());
    // "smile" and "sweat smile" should both appear.
    assert!(results.iter().any(|e| e.name.contains("smile")));
}

#[test]
fn emoji_search_empty_returns_all() {
    let picker = EmojiPicker::new();
    let all = picker.search("");
    assert_eq!(all.len(), picker.len());
}

#[test]
fn emoji_search_no_match() {
    let picker = EmojiPicker::new();
    let results = picker.search("zzzznonexistent");
    assert!(results.is_empty());
}

#[test]
fn emoji_by_category() {
    let picker = EmojiPicker::new();
    let flags = picker.by_category(EmojiCategory::Flags);
    assert!(!flags.is_empty());
    assert!(flags.iter().all(|e| e.category == EmojiCategory::Flags));
}

#[test]
fn emoji_category_labels() {
    assert_eq!(EmojiCategory::Smileys.label(), "Smileys & Emotion");
    assert_eq!(EmojiCategory::Flags.label(), "Flags");
}

#[test]
fn emoji_search_prefix_before_substring() {
    let picker = EmojiPicker::new();
    let results = picker.search("fire");
    assert!(!results.is_empty());
    // "fire" should come before entries that merely contain "fire" as a substring.
    assert_eq!(results[0].name, "fire");
}

// ===== InputMethodEngine tests =====

#[test]
fn engine_default_inactive() {
    let engine = InputMethodEngine::new();
    assert!(!engine.state().active);
    assert_eq!(engine.mode(), InputMode::Direct);
}

#[test]
fn engine_toggle() {
    let mut engine = InputMethodEngine::new();
    let action = engine.toggle();
    assert!(engine.state().active);
    assert!(matches!(action, InputAction::SwitchMode(_)));

    let action2 = engine.toggle();
    assert!(!engine.state().active);
    assert_eq!(action2, InputAction::HideCandidates);
}

#[test]
fn engine_direct_mode_forwards() {
    let mut engine = InputMethodEngine::new();
    engine.activate();
    let key = KeyEvent::new(0x0061, Some("a".to_string()), 0); // 'a'
    let action = engine.process_key(key);
    assert_eq!(action, InputAction::Forward);
}

#[test]
fn engine_dead_key_in_direct_mode() {
    let mut engine = InputMethodEngine::new();
    engine.activate();

    // Press dead_acute.
    let dead = KeyEvent::new(0xfe51, None, 0);
    let action = engine.process_key(dead);
    assert!(matches!(action, InputAction::UpdatePreedit(_)));
    assert_eq!(engine.mode(), InputMode::DeadKey);

    // Press 'a' -> should produce á.
    let a = KeyEvent::new(0x0061, Some("a".to_string()), 0);
    let action2 = engine.process_key(a);
    assert_eq!(action2, InputAction::Commit("\u{00e1}".to_string()));
    assert_eq!(engine.mode(), InputMode::Direct);
}

#[test]
fn engine_compose_mode() {
    let mut engine = InputMethodEngine::new();
    engine.set_mode(InputMode::Compose);

    // Multi_key + 'e' + '=' -> €
    let mk = KeyEvent::new(0xff20, None, 0);
    let a1 = engine.process_key(mk);
    assert!(matches!(a1, InputAction::UpdatePreedit(_)));

    let e = KeyEvent::new(0x0065, Some("e".to_string()), 0);
    let a2 = engine.process_key(e);
    assert!(matches!(a2, InputAction::UpdatePreedit(_)));

    let eq = KeyEvent::new(0x003d, Some("=".to_string()), 0);
    let a3 = engine.process_key(eq);
    assert_eq!(a3, InputAction::Commit("\u{20ac}".to_string()));
}

#[test]
fn engine_compose_escape_cancels() {
    let mut engine = InputMethodEngine::new();
    engine.set_mode(InputMode::Compose);

    let mk = KeyEvent::new(0xff20, None, 0);
    engine.process_key(mk);

    let esc = KeyEvent::new(0xff1b, None, 0); // Escape
    let action = engine.process_key(esc);
    assert_eq!(action, InputAction::HideCandidates);
}

#[test]
fn engine_hiragana_basic() {
    let mut engine = InputMethodEngine::new();
    engine.set_mode(InputMode::Hiragana);

    // Type "ka" -> should produce か
    let k = KeyEvent::new(0x006b, Some("k".to_string()), 0);
    let a1 = engine.process_key(k);
    assert!(matches!(a1, InputAction::UpdatePreedit(_)));

    let a = KeyEvent::new(0x0061, Some("a".to_string()), 0);
    let a2 = engine.process_key(a);
    if let InputAction::UpdatePreedit(ref preedit) = a2 {
        assert!(preedit.text.contains('\u{304B}')); // か
    } else {
        panic!("Expected UpdatePreedit, got {:?}", a2);
    }
}

#[test]
fn engine_katakana_basic() {
    let mut engine = InputMethodEngine::new();
    engine.set_mode(InputMode::Katakana);

    // Type "ka" -> should produce カ
    let k = KeyEvent::new(0x006b, Some("k".to_string()), 0);
    engine.process_key(k);

    let a = KeyEvent::new(0x0061, Some("a".to_string()), 0);
    let a2 = engine.process_key(a);
    if let InputAction::UpdatePreedit(ref preedit) = a2 {
        assert!(preedit.text.contains('\u{30AB}')); // カ
    } else {
        panic!("Expected UpdatePreedit, got {:?}", a2);
    }
}

#[test]
fn engine_commit() {
    let mut engine = InputMethodEngine::new();
    engine.set_mode(InputMode::Hiragana);

    // Type "a" -> あ
    let a = KeyEvent::new(0x0061, Some("a".to_string()), 0);
    engine.process_key(a);

    // Enter commits.
    let enter = KeyEvent::new(0xff0d, None, 0);
    let action = engine.process_key(enter);
    if let InputAction::Commit(ref text) = action {
        assert!(text.contains('\u{3042}')); // あ
    } else {
        panic!("Expected Commit, got {:?}", action);
    }
}

#[test]
fn engine_cancel() {
    let mut engine = InputMethodEngine::new();
    engine.set_mode(InputMode::Hiragana);

    let a = KeyEvent::new(0x0061, Some("a".to_string()), 0);
    engine.process_key(a);
    assert!(
        engine.state().is_composing() || !engine.state().preedit.is_empty() // The preedit might show in UpdatePreedit, check engine internal state.
    );

    let action = engine.cancel();
    assert_eq!(action, InputAction::HideCandidates);
}

#[test]
fn engine_candidate_navigation() {
    let mut engine = InputMethodEngine::new();
    engine.activate();
    // Manually inject candidates for testing.
    engine.state.candidates = vec![
        Candidate::new("A"),
        Candidate::new("B"),
        Candidate::new("C"),
    ];

    let a1 = engine.next_candidate();
    assert!(matches!(a1, InputAction::ShowCandidates(_)));
    assert_eq!(engine.state().selected_candidate, 1);

    let _a2 = engine.next_candidate();
    assert_eq!(engine.state().selected_candidate, 2);

    let _a3 = engine.next_candidate();
    assert_eq!(engine.state().selected_candidate, 0); // wraps

    let _a4 = engine.prev_candidate();
    assert_eq!(engine.state().selected_candidate, 2); // wraps back
}

#[test]
fn engine_select_candidate() {
    let mut engine = InputMethodEngine::new();
    engine.activate();
    engine.state.candidates = vec![Candidate::new("X"), Candidate::new("Y")];

    let action = engine.select_candidate(1);
    assert_eq!(action, InputAction::Commit("Y".to_string()));
    assert!(engine.state().candidates.is_empty());
}

#[test]
fn engine_inactive_forwards_non_hotkey() {
    let mut engine = InputMethodEngine::new();
    let key = KeyEvent::new(0x0061, Some("a".to_string()), 0);
    let action = engine.process_key(key);
    assert_eq!(action, InputAction::Forward);
}

#[test]
fn engine_ctrl_space_activates() {
    let mut engine = InputMethodEngine::new();
    // Ctrl+Space when inactive -> activate.
    let key = KeyEvent::new(0x0020, None, 2); // Ctrl
    let action = engine.process_key(key);
    assert!(matches!(action, InputAction::SwitchMode(_)));
    assert!(engine.state().active);
}

#[test]
fn engine_pinyin_basic() {
    let mut engine = InputMethodEngine::new();
    engine.set_mode(InputMode::Pinyin);

    // Type "ni" -> should show candidates for 你.
    let n = KeyEvent::new(0x006e, Some("n".to_string()), 0);
    engine.process_key(n);

    let i = KeyEvent::new(0x0069, Some("i".to_string()), 0);
    let action = engine.process_key(i);
    assert!(matches!(action, InputAction::UpdatePreedit(_)));
    assert!(!engine.state().candidates.is_empty());
    // First candidate should be 你.
    assert_eq!(engine.state().candidates[0].text, "\u{4F60}");
}

#[test]
fn engine_mode_switch_clears_state() {
    let mut engine = InputMethodEngine::new();
    engine.set_mode(InputMode::Hiragana);

    // Type something.
    let a = KeyEvent::new(0x0061, Some("a".to_string()), 0);
    engine.process_key(a);

    // Switch mode.
    engine.set_mode(InputMode::Pinyin);
    assert!(!engine.state().is_composing());
    assert_eq!(engine.mode(), InputMode::Pinyin);
}

#[test]
fn key_event_modifiers() {
    let key = KeyEvent::new(0x0061, None, 0b111); // Shift+Ctrl+Alt
    assert!(key.shift());
    assert!(key.ctrl());
    assert!(key.alt());

    let key2 = KeyEvent::new(0x0061, None, 0);
    assert!(!key2.shift());
    assert!(!key2.ctrl());
    assert!(!key2.alt());
}

// =====================================================================
// DeadKeyState tests
// =====================================================================

use crate::dead_keys::{
    ComposeResult as CharComposeResult, ComposeState, DeadKeyResult, DeadKeyState,
};

#[test]
fn dead_key_acute_e_produces_eacute() {
    let mut dk = DeadKeyState::new();
    // Feed acute accent (dead key).
    assert_eq!(dk.feed_key('\u{00B4}'), DeadKeyResult::Pending);
    assert_eq!(dk.pending(), Some('\u{00B4}'));
    // Feed 'e'.
    assert_eq!(dk.feed_key('e'), DeadKeyResult::Composed('\u{00E9}')); // e
    assert_eq!(dk.pending(), None);
}

#[test]
fn dead_key_umlaut_u_produces_udiaeresis() {
    let mut dk = DeadKeyState::new();
    assert_eq!(dk.feed_key('\u{00A8}'), DeadKeyResult::Pending);
    assert_eq!(dk.feed_key('u'), DeadKeyResult::Composed('\u{00FC}')); // u
}

#[test]
fn dead_key_tilde_n_produces_ntilde() {
    let mut dk = DeadKeyState::new();
    assert_eq!(dk.feed_key('~'), DeadKeyResult::Pending);
    assert_eq!(dk.feed_key('n'), DeadKeyResult::Composed('\u{00F1}')); // n
}

#[test]
fn dead_key_circumflex_a() {
    let mut dk = DeadKeyState::new();
    assert_eq!(dk.feed_key('^'), DeadKeyResult::Pending);
    assert_eq!(dk.feed_key('a'), DeadKeyResult::Composed('\u{00E2}')); // a
}

#[test]
fn dead_key_grave_e() {
    let mut dk = DeadKeyState::new();
    assert_eq!(dk.feed_key('`'), DeadKeyResult::Pending);
    assert_eq!(dk.feed_key('e'), DeadKeyResult::Composed('\u{00E8}')); // e
}

#[test]
fn dead_key_cedilla_c() {
    let mut dk = DeadKeyState::new();
    assert_eq!(dk.feed_key('\u{00B8}'), DeadKeyResult::Pending);
    assert_eq!(dk.feed_key('c'), DeadKeyResult::Composed('\u{00E7}')); // c
}

#[test]
fn dead_key_ring_a() {
    let mut dk = DeadKeyState::new();
    assert_eq!(dk.feed_key('\u{00B0}'), DeadKeyResult::Pending);
    assert_eq!(dk.feed_key('a'), DeadKeyResult::Composed('\u{00E5}')); // a
}

#[test]
fn dead_key_caron_s() {
    let mut dk = DeadKeyState::new();
    assert_eq!(dk.feed_key('\u{02C7}'), DeadKeyResult::Pending);
    assert_eq!(dk.feed_key('s'), DeadKeyResult::Composed('\u{0161}')); // s
}

#[test]
fn dead_key_macron_o() {
    let mut dk = DeadKeyState::new();
    assert_eq!(dk.feed_key('\u{00AF}'), DeadKeyResult::Pending);
    assert_eq!(dk.feed_key('o'), DeadKeyResult::Composed('\u{014D}')); // o
}

#[test]
fn dead_key_uppercase_acute_a() {
    let mut dk = DeadKeyState::new();
    assert_eq!(dk.feed_key('\u{00B4}'), DeadKeyResult::Pending);
    assert_eq!(dk.feed_key('A'), DeadKeyResult::Composed('\u{00C1}')); // A
}

#[test]
fn dead_key_no_match_passes_through() {
    let mut dk = DeadKeyState::new();
    assert_eq!(dk.feed_key('\u{00B4}'), DeadKeyResult::Pending);
    // 'z' has no acute composition in the default map.
    assert_eq!(dk.feed_key('z'), DeadKeyResult::PassThrough);
    assert_eq!(dk.pending(), None); // pending cleared
}

#[test]
fn dead_key_non_dead_char_passes_through() {
    let mut dk = DeadKeyState::new();
    assert_eq!(dk.feed_key('a'), DeadKeyResult::PassThrough);
    assert_eq!(dk.pending(), None);
}

#[test]
fn dead_key_reset() {
    let mut dk = DeadKeyState::new();
    dk.feed_key('\u{00B4}');
    assert!(dk.pending().is_some());
    dk.reset();
    assert_eq!(dk.pending(), None);
}

#[test]
fn dead_key_custom_map() {
    let dk = DeadKeyState::with_map(vec![('#', 'a', 'X'), ('#', 'b', 'Y')]);
    assert_eq!(dk.pending(), None);
    // '#' should be recognized as a dead key.
    let mut dk = dk;
    assert_eq!(dk.feed_key('#'), DeadKeyResult::Pending);
    assert_eq!(dk.feed_key('a'), DeadKeyResult::Composed('X'));
}

// =====================================================================
// ComposeState (character-level) tests
// =====================================================================

#[test]
fn compose_char_oc_copyright() {
    let mut cs = ComposeState::new();
    assert_eq!(cs.feed('o'), CharComposeResult::InProgress);
    assert!(cs.is_active());
    assert_eq!(cs.feed('c'), CharComposeResult::Composed("\u{00A9}".into())); // (c)
    assert!(!cs.is_active());
}

#[test]
fn compose_char_euro() {
    let mut cs = ComposeState::new();
    assert_eq!(cs.feed('e'), CharComposeResult::InProgress);
    assert_eq!(cs.feed('='), CharComposeResult::Composed("\u{20AC}".into())); // EUR
}

#[test]
fn compose_char_fraction_half() {
    let mut cs = ComposeState::new();
    assert_eq!(cs.feed('1'), CharComposeResult::InProgress);
    assert_eq!(cs.feed('2'), CharComposeResult::Composed("\u{00BD}".into())); // 1/2
}

#[test]
fn compose_char_fraction_quarter() {
    let mut cs = ComposeState::new();
    assert_eq!(cs.feed('1'), CharComposeResult::InProgress);
    assert_eq!(cs.feed('4'), CharComposeResult::Composed("\u{00BC}".into())); // 1/4
}

#[test]
fn compose_char_em_dash() {
    let mut cs = ComposeState::new();
    assert_eq!(cs.feed('-'), CharComposeResult::InProgress);
    assert_eq!(cs.feed('-'), CharComposeResult::Composed("\u{2014}".into())); // --
}

#[test]
fn compose_char_ellipsis() {
    let mut cs = ComposeState::new();
    assert_eq!(cs.feed('.'), CharComposeResult::InProgress);
    assert_eq!(cs.feed('.'), CharComposeResult::Composed("\u{2026}".into())); // ...
}

#[test]
fn compose_char_inverted_exclamation() {
    let mut cs = ComposeState::new();
    assert_eq!(cs.feed('!'), CharComposeResult::InProgress);
    assert_eq!(cs.feed('!'), CharComposeResult::Composed("\u{00A1}".into())); // !
}

#[test]
fn compose_char_pi() {
    let mut cs = ComposeState::new();
    assert_eq!(cs.feed('p'), CharComposeResult::InProgress);
    assert_eq!(cs.feed('i'), CharComposeResult::Composed("\u{03C0}".into())); // pi
}

#[test]
fn compose_char_no_match() {
    let mut cs = ComposeState::new();
    // 'z' does not start any default compose sequence, so first feed is NoMatch.
    assert_eq!(cs.feed('z'), CharComposeResult::NoMatch);
    assert!(!cs.is_active());
}

#[test]
fn compose_char_partial_then_no_match() {
    let mut cs = ComposeState::new();
    // 'o' starts valid sequences (oc, or, oo), so first feed is InProgress.
    assert_eq!(cs.feed('o'), CharComposeResult::InProgress);
    // 'z' does not continue any sequence starting with 'o'.
    assert_eq!(cs.feed('z'), CharComposeResult::NoMatch);
    assert!(!cs.is_active());
}

#[test]
fn compose_char_reset() {
    let mut cs = ComposeState::new();
    cs.feed('o');
    assert!(cs.is_active());
    cs.reset();
    assert!(!cs.is_active());
    assert!(cs.sequence().is_empty());
}

#[test]
fn compose_char_three_key_sequence() {
    let mut cs = ComposeState::new();
    // "inf" -> infinity
    assert_eq!(cs.feed('i'), CharComposeResult::InProgress);
    assert_eq!(cs.feed('n'), CharComposeResult::InProgress);
    assert_eq!(cs.feed('f'), CharComposeResult::Composed("\u{221E}".into()));
}

#[test]
fn compose_char_custom_sequences() {
    let mut cs = ComposeState::with_sequences(vec![(vec!['x', 'y'], "XY-RESULT".into())]);
    assert_eq!(cs.feed('x'), CharComposeResult::InProgress);
    assert_eq!(
        cs.feed('y'),
        CharComposeResult::Composed("XY-RESULT".into())
    );
}

// =====================================================================
// CandidateWindow tests
// =====================================================================

use crate::candidate_window::{CandidateEntry, CandidateWindow};

#[test]
fn candidate_window_empty() {
    let cw = CandidateWindow::new(5);
    assert!(cw.is_empty());
    assert_eq!(cw.total(), 0);
    assert_eq!(cw.total_pages(), 0);
    assert!(cw.visible_candidates().is_empty());
}

#[test]
fn candidate_window_set_candidates() {
    let mut cw = CandidateWindow::new(3);
    let entries: Vec<CandidateEntry> = (0..7)
        .map(|i| CandidateEntry::new(format!("item{}", i)))
        .collect();
    cw.set_candidates(entries);
    assert_eq!(cw.total(), 7);
    assert_eq!(cw.total_pages(), 3); // ceil(7/3) = 3
    assert_eq!(cw.current_page(), 0);
    assert_eq!(cw.selected_index(), 0);
}

#[test]
fn candidate_window_visible_candidates() {
    let mut cw = CandidateWindow::new(3);
    let entries: Vec<CandidateEntry> = (0..7)
        .map(|i| CandidateEntry::new(format!("item{}", i)))
        .collect();
    cw.set_candidates(entries);

    // Page 0: items 0, 1, 2.
    let vis = cw.visible_candidates();
    assert_eq!(vis.len(), 3);
    assert_eq!(vis[0].text, "item0");
    assert_eq!(vis[2].text, "item2");
}

#[test]
fn candidate_window_next_prev_candidate() {
    let mut cw = CandidateWindow::new(3);
    cw.set_candidates(vec![
        CandidateEntry::new("A"),
        CandidateEntry::new("B"),
        CandidateEntry::new("C"),
    ]);

    assert_eq!(cw.selected_index(), 0);
    cw.next_candidate();
    assert_eq!(cw.selected_index(), 1);
    cw.next_candidate();
    assert_eq!(cw.selected_index(), 2);
    cw.next_candidate();
    assert_eq!(cw.selected_index(), 0); // wraps

    cw.prev_candidate();
    assert_eq!(cw.selected_index(), 2); // wraps back
    cw.prev_candidate();
    assert_eq!(cw.selected_index(), 1);
}

#[test]
fn candidate_window_next_prev_page() {
    let mut cw = CandidateWindow::new(3);
    let entries: Vec<CandidateEntry> = (0..9)
        .map(|i| CandidateEntry::new(format!("item{}", i)))
        .collect();
    cw.set_candidates(entries);

    assert_eq!(cw.current_page(), 0);
    assert_eq!(cw.total_pages(), 3);

    cw.next_page();
    assert_eq!(cw.current_page(), 1);
    assert_eq!(cw.visible_candidates()[0].text, "item3");

    cw.next_page();
    assert_eq!(cw.current_page(), 2);
    assert_eq!(cw.visible_candidates()[0].text, "item6");

    cw.next_page();
    assert_eq!(cw.current_page(), 0); // wraps

    cw.prev_page();
    assert_eq!(cw.current_page(), 2); // wraps back
}

#[test]
fn candidate_window_select_by_number() {
    let mut cw = CandidateWindow::new(5);
    cw.set_candidates(vec![
        CandidateEntry::new("A"),
        CandidateEntry::new("B"),
        CandidateEntry::new("C"),
    ]);

    let entry = cw.select_by_number(1);
    assert!(entry.is_some());
    assert_eq!(entry.unwrap().text, "B");
    assert_eq!(cw.selected_index(), 1);

    // Out of range.
    let none = cw.select_by_number(10);
    assert!(none.is_none());
}

#[test]
fn candidate_window_confirm() {
    let mut cw = CandidateWindow::new(5);
    cw.set_candidates(vec![CandidateEntry::new("A"), CandidateEntry::new("B")]);
    cw.next_candidate(); // select "B"

    let confirmed = cw.confirm();
    assert!(confirmed.is_some());
    assert_eq!(confirmed.unwrap().text, "B");
    assert!(cw.is_empty()); // cleared after confirm
}

#[test]
fn candidate_window_confirm_empty() {
    let mut cw = CandidateWindow::new(5);
    assert!(cw.confirm().is_none());
}

#[test]
fn candidate_window_absolute_selected() {
    let mut cw = CandidateWindow::new(3);
    let entries: Vec<CandidateEntry> = (0..9)
        .map(|i| CandidateEntry::new(format!("item{}", i)))
        .collect();
    cw.set_candidates(entries);

    cw.next_page(); // page 1
    cw.next_candidate(); // index 1 on page 1
    assert_eq!(cw.absolute_selected(), 4); // 3*1 + 1
}

#[test]
fn candidate_window_last_page_partial() {
    let mut cw = CandidateWindow::new(3);
    let entries: Vec<CandidateEntry> = (0..5)
        .map(|i| CandidateEntry::new(format!("item{}", i)))
        .collect();
    cw.set_candidates(entries);

    assert_eq!(cw.total_pages(), 2);

    cw.next_page(); // page 1
    let vis = cw.visible_candidates();
    assert_eq!(vis.len(), 2); // only items 3,4
    assert_eq!(vis[0].text, "item3");
    assert_eq!(vis[1].text, "item4");
}

#[test]
fn candidate_window_clear() {
    let mut cw = CandidateWindow::new(5);
    cw.set_candidates(vec![CandidateEntry::new("A")]);
    assert!(!cw.is_empty());
    cw.clear();
    assert!(cw.is_empty());
}

#[test]
fn candidate_entry_builder() {
    let e = CandidateEntry::with_label("test", "1").annotated("note");
    assert_eq!(e.text, "test");
    assert_eq!(e.label.as_deref(), Some("1"));
    assert_eq!(e.annotation.as_deref(), Some("note"));
}

// =====================================================================
// KeywordEmojiPicker tests
// =====================================================================

use crate::emoji_picker::{
    EmojiCategory as KwEmojiCategory, EmojiEntry as KwEmojiEntry, EmojiPicker as KwEmojiPicker,
};

#[test]
fn kw_emoji_picker_default_table() {
    let picker = KwEmojiPicker::new();
    assert!(picker.len() >= 50);
    assert!(!picker.is_empty());
}

#[test]
fn kw_emoji_search_by_keyword() {
    let picker = KwEmojiPicker::new();
    // "laugh" is a keyword on "face with tears of joy".
    let results = picker.search("laugh");
    assert!(!results.is_empty());
    assert!(
        results
            .iter()
            .any(|e| e.keywords.contains(&"laugh".to_string()))
    );
}

#[test]
fn kw_emoji_search_by_name() {
    let picker = KwEmojiPicker::new();
    let results = picker.search("fire");
    assert!(!results.is_empty());
    assert_eq!(results[0].name, "fire");
}

#[test]
fn kw_emoji_search_case_insensitive() {
    let picker = KwEmojiPicker::new();
    let upper = picker.search("FIRE");
    let lower = picker.search("fire");
    assert_eq!(upper.len(), lower.len());
}

#[test]
fn kw_emoji_search_no_match() {
    let picker = KwEmojiPicker::new();
    let results = picker.search("zzzznonexistent");
    assert!(results.is_empty());
}

#[test]
fn kw_emoji_search_empty_returns_all() {
    let picker = KwEmojiPicker::new();
    let all = picker.search("");
    assert_eq!(all.len(), picker.len());
}

#[test]
fn kw_emoji_category_listing() {
    let picker = KwEmojiPicker::new();
    let smileys = picker.category_emojis(&KwEmojiCategory::SmileysEmotion);
    assert!(!smileys.is_empty());
    assert!(
        smileys
            .iter()
            .all(|e| e.category == KwEmojiCategory::SmileysEmotion)
    );
}

#[test]
fn kw_emoji_all_categories() {
    let picker = KwEmojiPicker::new();
    let cats = picker.categories();
    assert_eq!(cats.len(), 9);
    assert_eq!(cats[0], KwEmojiCategory::SmileysEmotion);
    assert_eq!(cats[8], KwEmojiCategory::Flags);
}

#[test]
fn kw_emoji_category_labels() {
    assert_eq!(KwEmojiCategory::SmileysEmotion.label(), "Smileys & Emotion");
    assert_eq!(KwEmojiCategory::PeopleBody.label(), "People & Body");
    assert_eq!(KwEmojiCategory::Flags.label(), "Flags");
}

#[test]
fn kw_emoji_recent_tracking() {
    let mut picker = KwEmojiPicker::new();
    assert!(picker.recent().is_empty());

    let entry = KwEmojiEntry::new(
        "\u{1F600}",
        "grin",
        vec!["happy"],
        KwEmojiCategory::SmileysEmotion,
    );
    picker.add_recent(entry.clone());
    assert_eq!(picker.recent().len(), 1);
    assert_eq!(picker.recent()[0].emoji, "\u{1F600}");

    // Adding the same emoji again moves it to front, no duplicates.
    let entry2 = KwEmojiEntry::new(
        "\u{1F602}",
        "joy",
        vec!["laugh"],
        KwEmojiCategory::SmileysEmotion,
    );
    picker.add_recent(entry2);
    picker.add_recent(entry.clone());
    assert_eq!(picker.recent().len(), 2);
    assert_eq!(picker.recent()[0].emoji, "\u{1F600}"); // most recent
}

#[test]
fn kw_emoji_recent_max_limit() {
    let mut picker = KwEmojiPicker::new();
    // Add 35 unique entries; max_recent is 30.
    for i in 0..35 {
        let entry = KwEmojiEntry::new(
            format!("E{}", i),
            format!("emoji{}", i),
            vec![],
            KwEmojiCategory::Objects,
        );
        picker.add_recent(entry);
    }
    assert_eq!(picker.recent().len(), 30);
    // Most recent is E34.
    assert_eq!(picker.recent()[0].emoji, "E34");
}

#[test]
fn kw_emoji_search_prioritizes_name_over_keyword() {
    let picker = KwEmojiPicker::new();
    // "pizza" is a name match, not just a keyword match.
    let results = picker.search("pizza");
    assert!(!results.is_empty());
    // The first result should be the one named "pizza".
    assert!(results[0].name.contains("pizza"));
}

// =====================================================================
// InputMethodSwitcher tests
// =====================================================================

use crate::switcher::{InputMethodInfo, InputMethodSwitcher};

#[test]
fn switcher_empty() {
    let sw = InputMethodSwitcher::new();
    assert!(sw.is_empty());
    assert_eq!(sw.len(), 0);
    assert!(sw.active().is_none());
}

#[test]
fn switcher_add_methods() {
    let mut sw = InputMethodSwitcher::new();
    sw.add_method(InputMethodInfo::new("en-us", "English (US)", "en"));
    sw.add_method(InputMethodInfo::new("ja-romaji", "Japanese - Romaji", "ja"));
    assert_eq!(sw.len(), 2);
    assert!(!sw.is_empty());
    assert_eq!(sw.active().unwrap().id, "en-us");
}

#[test]
fn switcher_switch_next_cycles() {
    let mut sw = InputMethodSwitcher::new();
    sw.add_method(InputMethodInfo::new("en", "English", "en"));
    sw.add_method(InputMethodInfo::new("ja", "Japanese", "ja"));
    sw.add_method(InputMethodInfo::new("zh", "Chinese", "zh"));

    assert_eq!(sw.active_index(), 0);

    let m = sw.switch_next().unwrap();
    assert_eq!(m.id, "ja");
    assert_eq!(sw.active_index(), 1);

    let m = sw.switch_next().unwrap();
    assert_eq!(m.id, "zh");

    let m = sw.switch_next().unwrap();
    assert_eq!(m.id, "en"); // wraps around
    assert_eq!(sw.active_index(), 0);
}

#[test]
fn switcher_switch_to() {
    let mut sw = InputMethodSwitcher::new();
    sw.add_method(InputMethodInfo::new("en", "English", "en"));
    sw.add_method(InputMethodInfo::new("ja", "Japanese", "ja"));

    let m = sw.switch_to(1);
    assert!(m.is_some());
    assert_eq!(m.unwrap().id, "ja");
    assert_eq!(sw.active_index(), 1);

    // Out of bounds.
    let none = sw.switch_to(99);
    assert!(none.is_none());
    assert_eq!(sw.active_index(), 1); // unchanged
}

#[test]
fn switcher_per_window_state() {
    let mut sw = InputMethodSwitcher::new();
    sw.add_method(InputMethodInfo::new("en", "English", "en"));
    sw.add_method(InputMethodInfo::new("ja", "Japanese", "ja"));
    sw.add_method(InputMethodInfo::new("zh", "Chinese", "zh"));

    // Global is "en" (index 0).
    assert_eq!(sw.get_for_window(100).unwrap().id, "en");

    // Set window 100 to "zh" (index 2).
    sw.set_for_window(100, 2);
    assert_eq!(sw.get_for_window(100).unwrap().id, "zh");

    // Window 200 still uses global.
    assert_eq!(sw.get_for_window(200).unwrap().id, "en");

    // Clear window override.
    sw.clear_for_window(100);
    assert_eq!(sw.get_for_window(100).unwrap().id, "en");
}

#[test]
fn switcher_per_window_follows_global_change() {
    let mut sw = InputMethodSwitcher::new();
    sw.add_method(InputMethodInfo::new("en", "English", "en"));
    sw.add_method(InputMethodInfo::new("ja", "Japanese", "ja"));

    // Window 50 has no override -> follows global.
    sw.switch_to(1);
    assert_eq!(sw.get_for_window(50).unwrap().id, "ja");
}

#[test]
fn switcher_methods_list() {
    let mut sw = InputMethodSwitcher::new();
    sw.add_method(InputMethodInfo::new("en", "English", "en"));
    sw.add_method(InputMethodInfo::new("ja", "Japanese", "ja"));

    let methods = sw.methods();
    assert_eq!(methods.len(), 2);
    assert_eq!(methods[0].id, "en");
    assert_eq!(methods[1].language, "ja");
}

#[test]
fn switcher_switch_next_empty() {
    let mut sw = InputMethodSwitcher::new();
    assert!(sw.switch_next().is_none());
}

#[test]
fn switcher_info_with_icon() {
    let info = InputMethodInfo::new("en", "English", "en").with_icon("keyboard-en.svg");
    assert_eq!(info.icon.as_deref(), Some("keyboard-en.svg"));
}

#[test]
fn switcher_set_for_window_invalid_index_ignored() {
    let mut sw = InputMethodSwitcher::new();
    sw.add_method(InputMethodInfo::new("en", "English", "en"));
    // Index 5 is out of bounds (only 1 method).
    sw.set_for_window(100, 5);
    // Should still use global default.
    assert_eq!(sw.get_for_window(100).unwrap().id, "en");
}
