use super::*;

#[test]
fn glyph_count() {
    assert_eq!(FONT_DATA.len(), 95);
}

#[test]
fn each_glyph_is_16_bytes() {
    for (i, glyph) in FONT_DATA.iter().enumerate() {
        assert_eq!(
            glyph.len(),
            16,
            "glyph at index {} (char {}) has wrong length",
            i,
            (i as u8 + 32) as char,
        );
    }
}

#[test]
fn space_is_blank() {
    let font = BitmapFont::new();
    let g = font.glyph(' ');
    assert!(g.iter().all(|&b| b == 0x00));
}

#[test]
fn fallback_is_not_a_solid_block() {
    let font = BitmapFont::new();
    // A genuinely-unknown, non-icon codepoint returns the `.notdef` fallback.
    // It MUST NOT be a fully-inked solid block (`[0xFF; 16]`) — that is the
    // worst possible notdef and is exactly the t167 "solid block" symptom.
    let g = font.glyph('\u{FFFF}');
    assert!(
        !g.iter().all(|&b| b == 0xFF),
        "fallback glyph must not be a fully-inked solid block"
    );
    // It must still be a *visible* glyph (the classic tofu box), so it has
    // both inked and non-inked rows — internal structure, not uniform.
    assert!(
        g.iter().any(|&b| b != 0x00),
        "fallback glyph must have visible ink (an outline box)"
    );
    assert!(
        g.iter().any(|&b| b == 0x00),
        "fallback glyph must have empty pixels (hollow, not solid)"
    );
}

#[test]
fn printable_ascii_not_blank() {
    let font = BitmapFont::new();
    // Every printable character except space should have at least one
    // non-zero byte (i.e. it actually has visible pixels).
    for code in 33u8..=126 {
        let ch = code as char;
        let g = font.glyph(ch);
        assert!(
            g.iter().any(|&b| b != 0x00),
            "glyph for '{}' (ASCII {}) is entirely blank",
            ch,
            code,
        );
    }
}

#[test]
fn measure_empty() {
    let font = BitmapFont::new();
    assert_eq!(font.measure_text(""), (0, 0));
}

#[test]
fn measure_single_char() {
    let font = BitmapFont::new();
    assert_eq!(font.measure_text("A"), (8, 16));
}

#[test]
fn measure_hello() {
    let font = BitmapFont::new();
    assert_eq!(font.measure_text("Hello"), (40, 16));
}

#[test]
fn measure_multiline() {
    let font = BitmapFont::new();
    // "AB\nCDE" => longest line is 3 chars = 24px, 2 lines = 32px
    assert_eq!(font.measure_text("AB\nCDE"), (24, 32));
}

#[test]
fn glyph_lookup_boundaries() {
    let font = BitmapFont::new();
    // First printable ASCII
    let _ = font.glyph(' ');
    // Last printable ASCII
    let _ = font.glyph('~');
    // Just below range — falls to the hollow `.notdef` box (not a solid block).
    let g = font.glyph('\x1F');
    assert_eq!(g, &FALLBACK_GLYPH, "below-range should be fallback");
    assert!(
        !g.iter().all(|&b| b == 0xFF),
        "below-range fallback must not be solid"
    );
    // Just above range — same.
    let g = font.glyph('\x7F');
    assert_eq!(g, &FALLBACK_GLYPH, "above-range should be fallback");
    assert!(
        !g.iter().all(|&b| b == 0xFF),
        "above-range fallback must not be solid"
    );
}

/// The devtools dingbat codepoints must resolve to recognizable icon glyphs —
/// NOT the `.notdef` fallback and, critically, NOT a uniform solid block.
///
/// This is the t167 bug-3 regression guard. Before the fix every one of these
/// codepoints fell through to `FALLBACK_GLYPH = [0xFF; 16]` (a solid filled
/// rectangle), so each devtools toolbar button / tree arrow painted as an
/// opaque block instead of an icon.
#[test]
fn devtools_icon_codepoints_render_as_structured_icons_not_blocks() {
    let font = BitmapFont::new();
    let icons = [
        ('\u{25B6}', "▶ tree-collapsed"),
        ('\u{25BC}', "▼ tree-expanded"),
        ('\u{2295}', "⊕ picker"),
        ('\u{25EB}', "◫ detach"),
        ('\u{22A5}', "⊥ dock-bottom"),
        ('\u{22A2}', "⊢ dock-right"),
        ('\u{2713}', "✓ applied"),
        ('\u{25CB}', "○ pending"),
    ];
    for (ch, label) in icons {
        let g = font.glyph(ch);
        // Not the notdef fallback.
        assert_ne!(
            g, &FALLBACK_GLYPH,
            "{label} must have a real icon glyph, not the .notdef fallback"
        );
        // Not a solid block (this is the teeth: the old [0xFF;16] path).
        assert!(
            !g.iter().all(|&b| b == 0xFF),
            "{label} must not be a solid filled block"
        );
        // Has real internal structure: a mix of inked and non-inked pixels.
        assert!(
            g.iter().any(|&b| b != 0x00),
            "{label} must have visible ink"
        );
        assert!(
            g.iter().any(|&b| b != 0xFF),
            "{label} must have non-inked (background) pixels for internal shape"
        );
    }
}

#[test]
fn digit_a_glyph_sanity() {
    let font = BitmapFont::new();
    // 'A' should have the crossbar row (0xFE) somewhere in its data.
    let g = font.glyph('A');
    assert!(
        g.iter().any(|&b| b == 0xFE),
        "'A' glyph should contain a crossbar row (0xFE)",
    );
}
