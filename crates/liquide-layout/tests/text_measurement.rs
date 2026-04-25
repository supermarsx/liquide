//! Extensive text measurement tests for DefaultTextMeasurer.
//!
//! Covers: single-line, multi-line via \n, wrapping, text-indent,
//! white-space modes, letter-spacing, word-spacing, text-transform,
//! empty text, whitespace-only text, and edge cases.

use liquide_layout::{DefaultTextMeasurer, TextMeasurer, TextMetrics, TextProperties};
use liquide_style_engine::computed::{LineHeight, WhiteSpace};

// ── Helpers ──────────────────────────────────────────────────────────────

fn measurer() -> DefaultTextMeasurer {
    DefaultTextMeasurer
}

fn default_props() -> TextProperties {
    TextProperties::default()
}

fn pre_props() -> TextProperties {
    let mut p = TextProperties::default();
    p.white_space = WhiteSpace::Pre;
    p
}

fn pre_wrap_props() -> TextProperties {
    let mut p = TextProperties::default();
    p.white_space = WhiteSpace::PreWrap;
    p
}

fn pre_line_props() -> TextProperties {
    let mut p = TextProperties::default();
    p.white_space = WhiteSpace::PreLine;
    p
}

fn nowrap_props() -> TextProperties {
    let mut p = TextProperties::default();
    p.white_space = WhiteSpace::NoWrap;
    p
}

const FONT_SIZE: f32 = 16.0;
const NORMAL_FAMILIES: &[String] = &[];

// ── Basic single-line measurement ────────────────────────────────────────

#[test]
fn measure_empty_string() {
    let m = measurer();
    let result = m.measure("", FONT_SIZE, NORMAL_FAMILIES, 400, None, &default_props());
    assert_eq!(result.width, 0.0, "empty string should have zero width");
    assert_eq!(result.line_count, 1, "empty string is still 1 line");
    assert!(
        result.height > 0.0,
        "height should be positive (line height)"
    );
}

#[test]
fn measure_single_char() {
    let m = measurer();
    let result = m.measure("A", FONT_SIZE, NORMAL_FAMILIES, 400, None, &default_props());
    assert!(result.width > 0.0, "single char should have positive width");
    assert_eq!(result.line_count, 1);
}

#[test]
fn measure_longer_text_wider() {
    let m = measurer();
    let short = m.measure(
        "ab",
        FONT_SIZE,
        NORMAL_FAMILIES,
        400,
        None,
        &default_props(),
    );
    let long = m.measure(
        "abcdef",
        FONT_SIZE,
        NORMAL_FAMILIES,
        400,
        None,
        &default_props(),
    );
    assert!(
        long.width > short.width,
        "longer text should be wider: {} vs {}",
        long.width,
        short.width
    );
}

#[test]
fn measure_baseline_is_positive() {
    let m = measurer();
    let result = m.measure(
        "Hello",
        FONT_SIZE,
        NORMAL_FAMILIES,
        400,
        None,
        &default_props(),
    );
    assert!(result.baseline > 0.0);
}

// ── Line height ──────────────────────────────────────────────────────────

#[test]
fn measure_default_line_height() {
    let m = measurer();
    let result = m.measure(
        "Hello",
        FONT_SIZE,
        NORMAL_FAMILIES,
        400,
        None,
        &default_props(),
    );
    // Default line-height: normal → font_size * 1.2 = 19.2
    let expected = FONT_SIZE * 1.2;
    assert!(
        (result.height - expected).abs() < 0.1,
        "height should be ~{}, got {}",
        expected,
        result.height
    );
}

#[test]
fn measure_explicit_line_height_px() {
    let m = measurer();
    let mut props = default_props();
    props.line_height = LineHeight::Px(24.0);
    let result = m.measure("Hello", FONT_SIZE, NORMAL_FAMILIES, 400, None, &props);
    assert!(
        (result.height - 24.0).abs() < 0.1,
        "height should be 24.0, got {}",
        result.height
    );
}

#[test]
fn measure_line_height_number_multiplier() {
    let m = measurer();
    let mut props = default_props();
    props.line_height = LineHeight::Number(2.0);
    let result = m.measure("Hello", FONT_SIZE, NORMAL_FAMILIES, 400, None, &props);
    let expected = FONT_SIZE * 2.0;
    assert!(
        (result.height - expected).abs() < 0.1,
        "height should be ~{}, got {}",
        expected,
        result.height
    );
}

// ── Newlines in pre/pre-wrap/pre-line ────────────────────────────────────

#[test]
fn measure_newline_in_pre_mode() {
    let m = measurer();
    let result = m.measure(
        "Hello\nWorld",
        FONT_SIZE,
        NORMAL_FAMILIES,
        400,
        None,
        &pre_props(),
    );
    assert_eq!(
        result.line_count, 2,
        "\\n in pre mode should produce 2 lines"
    );
    let line_h = FONT_SIZE * 1.2;
    assert!(
        (result.height - 2.0 * line_h).abs() < 0.1,
        "height should be 2 × line_h = {}, got {}",
        2.0 * line_h,
        result.height
    );
}

#[test]
fn measure_multiple_newlines_pre() {
    let m = measurer();
    let result = m.measure(
        "a\nb\nc\nd",
        FONT_SIZE,
        NORMAL_FAMILIES,
        400,
        None,
        &pre_props(),
    );
    assert_eq!(result.line_count, 4, "3 \\n should produce 4 lines");
}

#[test]
fn measure_trailing_newline_pre() {
    let m = measurer();
    let result = m.measure(
        "Hello\n",
        FONT_SIZE,
        NORMAL_FAMILIES,
        400,
        None,
        &pre_props(),
    );
    // "Hello\n" splits into ["Hello", ""] → 2 lines
    assert_eq!(
        result.line_count, 2,
        "trailing \\n should add an extra line"
    );
}

#[test]
fn measure_leading_newline_pre() {
    let m = measurer();
    let result = m.measure(
        "\nHello",
        FONT_SIZE,
        NORMAL_FAMILIES,
        400,
        None,
        &pre_props(),
    );
    // "\nHello" → ["", "Hello"] → 2 lines
    assert_eq!(result.line_count, 2, "leading \\n should add a line");
}

#[test]
fn measure_only_newlines_pre() {
    let m = measurer();
    let result = m.measure(
        "\n\n\n",
        FONT_SIZE,
        NORMAL_FAMILIES,
        400,
        None,
        &pre_props(),
    );
    // "\n\n\n" → ["", "", "", ""] → 4 lines
    assert_eq!(result.line_count, 4);
}

#[test]
fn measure_newline_in_pre_wrap() {
    let m = measurer();
    let result = m.measure(
        "Hello\nWorld",
        FONT_SIZE,
        NORMAL_FAMILIES,
        400,
        None,
        &pre_wrap_props(),
    );
    assert_eq!(result.line_count, 2);
}

#[test]
fn measure_newline_in_pre_line() {
    let m = measurer();
    let result = m.measure(
        "Hello\nWorld",
        FONT_SIZE,
        NORMAL_FAMILIES,
        400,
        None,
        &pre_line_props(),
    );
    assert_eq!(result.line_count, 2);
}

#[test]
fn measure_newline_in_normal_mode_not_preserved() {
    let m = measurer();
    // In normal white-space mode, \n should be treated as a space, not a line break.
    let result = m.measure(
        "Hello\nWorld",
        FONT_SIZE,
        NORMAL_FAMILIES,
        400,
        None,
        &default_props(),
    );
    // Should be single-line (newlines collapsed in normal mode)
    assert_eq!(
        result.line_count, 1,
        "\\n in white-space:normal should not create line breaks"
    );
}

#[test]
fn measure_newline_in_nowrap_not_preserved() {
    let m = measurer();
    let result = m.measure(
        "Hello\nWorld",
        FONT_SIZE,
        NORMAL_FAMILIES,
        400,
        None,
        &nowrap_props(),
    );
    assert_eq!(
        result.line_count, 1,
        "\\n in white-space:nowrap should not create line breaks"
    );
}

// ── Wrapping with max_width ──────────────────────────────────────────────

#[test]
fn measure_wraps_when_exceeds_max_width() {
    let m = measurer();
    let text = "The quick brown fox jumps over the lazy dog";
    let result = m.measure(
        text,
        FONT_SIZE,
        NORMAL_FAMILIES,
        400,
        Some(100.0),
        &default_props(),
    );
    assert!(result.line_count > 1, "long text should wrap within 100px");
}

#[test]
fn measure_no_wrap_when_fits() {
    let m = measurer();
    let result = m.measure(
        "Hi",
        FONT_SIZE,
        NORMAL_FAMILIES,
        400,
        Some(500.0),
        &default_props(),
    );
    assert_eq!(result.line_count, 1, "short text should not wrap");
}

#[test]
fn measure_nowrap_ignores_max_width() {
    let m = measurer();
    let text = "The quick brown fox jumps over the lazy dog over and over again";
    let result = m.measure(
        text,
        FONT_SIZE,
        NORMAL_FAMILIES,
        400,
        Some(50.0),
        &nowrap_props(),
    );
    assert_eq!(
        result.line_count, 1,
        "white-space:nowrap should prevent wrapping"
    );
}

#[test]
fn measure_pre_does_not_soft_wrap() {
    let m = measurer();
    let text = "a very long line that should not wrap in pre mode at all";
    let result = m.measure(
        text,
        FONT_SIZE,
        NORMAL_FAMILIES,
        400,
        Some(50.0),
        &pre_props(),
    );
    assert_eq!(result.line_count, 1, "white-space:pre should not soft-wrap");
}

#[test]
fn measure_pre_wrap_does_soft_wrap() {
    let m = measurer();
    let text = "This is a moderately long line that should wrap in pre-wrap";
    let result = m.measure(
        text,
        FONT_SIZE,
        NORMAL_FAMILIES,
        400,
        Some(80.0),
        &pre_wrap_props(),
    );
    assert!(
        result.line_count > 1,
        "white-space:pre-wrap should soft-wrap"
    );
}

// ── Text-indent (first line only) ────────────────────────────────────────

#[test]
fn measure_text_indent_only_affects_first_line() {
    let m = measurer();
    let text = "The quick brown fox jumps over the lazy dog and other things";

    let mut props_no_indent = default_props();
    let result_no_indent = m.measure(
        text,
        FONT_SIZE,
        NORMAL_FAMILIES,
        400,
        Some(200.0),
        &props_no_indent,
    );

    let mut props_indent = default_props();
    props_indent.text_indent = 50.0;
    let result_indent = m.measure(
        text,
        FONT_SIZE,
        NORMAL_FAMILIES,
        400,
        Some(200.0),
        &props_indent,
    );

    // With text-indent, the first line has less space, so more lines total
    assert!(
        result_indent.line_count >= result_no_indent.line_count,
        "text-indent should increase or maintain line count: {} vs {}",
        result_indent.line_count,
        result_no_indent.line_count
    );
}

#[test]
fn measure_text_indent_with_newlines_pre() {
    let m = measurer();
    let mut props = pre_props();
    props.text_indent = 40.0;

    let result = m.measure(
        "Hello\nWorld",
        FONT_SIZE,
        NORMAL_FAMILIES,
        400,
        None,
        &props,
    );
    assert_eq!(result.line_count, 2);
    // First line should be wider due to indent
    // We can't directly check line widths, but overall width should include indent
}

#[test]
fn measure_negative_text_indent_is_allowed() {
    let m = measurer();
    let mut props = default_props();
    props.text_indent = -20.0;

    // Should not crash and should still produce valid results
    let result = m.measure(
        "Hello World",
        FONT_SIZE,
        NORMAL_FAMILIES,
        400,
        Some(200.0),
        &props,
    );
    assert!(result.line_count >= 1);
    assert!(result.height > 0.0);
}

// ── Letter spacing ───────────────────────────────────────────────────────

#[test]
fn measure_letter_spacing_increases_width() {
    let m = measurer();
    let text = "Hello";

    let no_spacing = m.measure(
        text,
        FONT_SIZE,
        NORMAL_FAMILIES,
        400,
        None,
        &default_props(),
    );

    let mut props = default_props();
    props.letter_spacing = 5.0;
    let with_spacing = m.measure(text, FONT_SIZE, NORMAL_FAMILIES, 400, None, &props);

    assert!(
        with_spacing.width > no_spacing.width,
        "letter-spacing should increase width: {} vs {}",
        with_spacing.width,
        no_spacing.width
    );
}

// ── Word spacing ─────────────────────────────────────────────────────────

#[test]
fn measure_word_spacing_increases_width() {
    let m = measurer();
    let text = "Hello World";

    let no_spacing = m.measure(
        text,
        FONT_SIZE,
        NORMAL_FAMILIES,
        400,
        None,
        &default_props(),
    );

    let mut props = default_props();
    props.word_spacing = 10.0;
    let with_spacing = m.measure(text, FONT_SIZE, NORMAL_FAMILIES, 400, None, &props);

    assert!(
        with_spacing.width > no_spacing.width,
        "word-spacing should increase width for text with spaces"
    );
}

#[test]
fn measure_word_spacing_no_effect_without_spaces() {
    let m = measurer();
    let text = "HelloWorld"; // no spaces

    let no_spacing = m.measure(
        text,
        FONT_SIZE,
        NORMAL_FAMILIES,
        400,
        None,
        &default_props(),
    );

    let mut props = default_props();
    props.word_spacing = 10.0;
    let with_spacing = m.measure(text, FONT_SIZE, NORMAL_FAMILIES, 400, None, &props);

    assert!(
        (with_spacing.width - no_spacing.width).abs() < 0.01,
        "word-spacing should not affect text without spaces"
    );
}

// ── Multi-line height calculations ───────────────────────────────────────

#[test]
fn measure_wrapped_height_is_lines_times_line_height() {
    let m = measurer();
    let text = "The quick brown fox jumps over the lazy dog and many more words to wrap";

    let result = m.measure(
        text,
        FONT_SIZE,
        NORMAL_FAMILIES,
        400,
        Some(100.0),
        &default_props(),
    );

    let line_h = FONT_SIZE * 1.2;
    let expected_height = result.line_count as f32 * line_h;
    assert!(
        (result.height - expected_height).abs() < 0.1,
        "height should be line_count × line_height: expected {}, got {}",
        expected_height,
        result.height
    );
}

#[test]
fn measure_pre_newline_height_is_lines_times_line_height() {
    let m = measurer();
    let result = m.measure(
        "A\nB\nC",
        FONT_SIZE,
        NORMAL_FAMILIES,
        400,
        None,
        &pre_props(),
    );

    let line_h = FONT_SIZE * 1.2;
    let expected = 3.0 * line_h;
    assert!(
        (result.height - expected).abs() < 0.1,
        "3 lines at {} each = {}, got {}",
        line_h,
        expected,
        result.height
    );
}

// ── Whitespace-only text ─────────────────────────────────────────────────

#[test]
fn measure_whitespace_only() {
    let m = measurer();
    let result = m.measure(
        "   ",
        FONT_SIZE,
        NORMAL_FAMILIES,
        400,
        None,
        &default_props(),
    );
    assert!(result.width > 0.0, "spaces should have positive width");
    assert_eq!(result.line_count, 1);
}

// ── Large font size ──────────────────────────────────────────────────────

#[test]
fn measure_large_font_size_scales_proportionally() {
    let m = measurer();
    let text = "Hello";

    let small = m.measure(text, 12.0, NORMAL_FAMILIES, 400, None, &default_props());
    let large = m.measure(text, 48.0, NORMAL_FAMILIES, 400, None, &default_props());

    // Width should scale roughly proportionally with font size
    let ratio = large.width / small.width;
    assert!(
        ratio > 3.0 && ratio < 5.0,
        "48/12 = 4x size → width ratio should be ~4, got {}",
        ratio
    );
}

// ── Pre-wrap + newlines + wrapping combined ──────────────────────────────

#[test]
fn measure_pre_wrap_newlines_and_wrapping() {
    let m = measurer();
    // First hard line fits, second is very long and should wrap
    let text = "Short\nThis is a very very very very very long line that exceeds max width";
    let result = m.measure(
        text,
        FONT_SIZE,
        NORMAL_FAMILIES,
        400,
        Some(100.0),
        &pre_wrap_props(),
    );
    assert!(
        result.line_count > 2,
        "should have more than 2 lines (1 short + wrapped long): got {}",
        result.line_count
    );
}

// ── Width is capped at max_width ─────────────────────────────────────────

#[test]
fn measure_width_does_not_exceed_max_width() {
    let m = measurer();
    let text = "The quick brown fox jumps over the lazy dog";
    let max_w = 100.0;
    let result = m.measure(
        text,
        FONT_SIZE,
        NORMAL_FAMILIES,
        400,
        Some(max_w),
        &default_props(),
    );
    assert!(
        result.width <= max_w + 0.01,
        "width {} should not exceed max_width {}",
        result.width,
        max_w
    );
}
