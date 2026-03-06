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
fn fallback_is_filled() {
    let font = BitmapFont::new();
    // Non-ASCII character should return the filled-block fallback.
    let g = font.glyph('\u{FFFF}');
    assert!(g.iter().all(|&b| b == 0xFF));
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
    // Just below range
    let g = font.glyph('\x1F');
    assert!(
        g.iter().all(|&b| b == 0xFF),
        "below-range should be fallback"
    );
    // Just above range
    let g = font.glyph('\x7F');
    assert!(
        g.iter().all(|&b| b == 0xFF),
        "above-range should be fallback"
    );
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
