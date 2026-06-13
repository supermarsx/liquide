//! Caret movement: logical and visual cursor navigation.
//!
//! Implements cursor movement by:
//! - Grapheme cluster (character-level)
//! - Word boundary
//! - Line (home/end)
//! - Paragraph
//! - Document (Ctrl+Home/End)
//!
//! Handles bidi text correctly using visual ordering.

use serde::{Deserialize, Serialize};

use crate::cluster::grapheme_clusters;
use crate::paragraph::LayoutLine;
use crate::selection::TextOffset;

/// Caret position in the text with visual context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaretPosition {
    /// Byte offset in the source text.
    pub offset: TextOffset,
    /// Line index in the laid-out paragraph.
    pub line: usize,
    /// Preferred X position for vertical movement (sticky column).
    pub preferred_x: Option<i32>,
}

impl CaretPosition {
    #[must_use]
    pub fn new(offset: usize, line: usize) -> Self {
        Self {
            offset: TextOffset(offset),
            line,
            preferred_x: None,
        }
    }

    #[must_use]
    pub fn at_start() -> Self {
        Self::new(0, 0)
    }
}

/// Direction of caret movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MoveDirection {
    /// Move left (or previous in reading order).
    Left,
    /// Move right (or next in reading order).
    Right,
    /// Move up one line.
    Up,
    /// Move down one line.
    Down,
}

/// Granularity of caret movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MoveGranularity {
    /// Move by grapheme cluster (single "character").
    Grapheme,
    /// Move by word boundary.
    Word,
    /// Move to line boundary (home/end).
    Line,
    /// Move to paragraph boundary.
    Paragraph,
    /// Move to document boundary (Ctrl+Home/End).
    Document,
}

/// The caret navigator.
pub struct CaretNavigator;

impl CaretNavigator {
    /// Move the caret in the given direction and granularity.
    #[must_use]
    pub fn move_caret(
        text: &str,
        current: TextOffset,
        direction: MoveDirection,
        granularity: MoveGranularity,
        lines: &[LayoutLine],
    ) -> TextOffset {
        match (direction, granularity) {
            (MoveDirection::Left, MoveGranularity::Grapheme) => Self::prev_grapheme(text, current),
            (MoveDirection::Right, MoveGranularity::Grapheme) => Self::next_grapheme(text, current),
            (MoveDirection::Left, MoveGranularity::Word) => Self::prev_word(text, current),
            (MoveDirection::Right, MoveGranularity::Word) => Self::next_word(text, current),
            (MoveDirection::Left, MoveGranularity::Line) => Self::line_start(text, current),
            (MoveDirection::Right, MoveGranularity::Line) => Self::line_end(text, current),
            (MoveDirection::Up, MoveGranularity::Grapheme) => Self::prev_line(text, current, lines),
            (MoveDirection::Down, MoveGranularity::Grapheme) => {
                Self::next_line(text, current, lines)
            }
            (_, MoveGranularity::Document) => match direction {
                MoveDirection::Left | MoveDirection::Up => TextOffset(0),
                MoveDirection::Right | MoveDirection::Down => TextOffset(text.len()),
            },
            (MoveDirection::Up, MoveGranularity::Paragraph) => Self::prev_paragraph(text, current),
            (MoveDirection::Down, MoveGranularity::Paragraph) => {
                Self::next_paragraph(text, current)
            }
            _ => current,
        }
    }

    /// Move to the previous grapheme cluster.
    #[must_use]
    pub fn prev_grapheme(text: &str, current: TextOffset) -> TextOffset {
        if current.0 == 0 {
            return current;
        }
        let clusters = grapheme_clusters(text);
        // Find the cluster that contains or ends at current.
        for cluster in clusters.iter().rev() {
            if cluster.start < current.0 {
                return TextOffset(cluster.start);
            }
        }
        TextOffset(0)
    }

    /// Move to the next grapheme cluster.
    #[must_use]
    pub fn next_grapheme(text: &str, current: TextOffset) -> TextOffset {
        if current.0 >= text.len() {
            return current;
        }
        let clusters = grapheme_clusters(text);
        for cluster in &clusters {
            if cluster.start >= current.0 && cluster.end > current.0 {
                return TextOffset(cluster.end);
            }
        }
        TextOffset(text.len())
    }

    /// Move to the previous word boundary.
    #[must_use]
    pub fn prev_word(text: &str, current: TextOffset) -> TextOffset {
        let bytes = text.as_bytes();
        let mut pos = current.0;

        // Skip trailing spaces.
        while pos > 0 && is_whitespace(bytes[pos - 1]) {
            pos -= 1;
        }

        if pos == 0 {
            return TextOffset(0);
        }

        // Determine the category of the character before pos.
        let cat = char_category(bytes[pos - 1]);

        // Move back through characters of the same category.
        while pos > 0 && char_category(bytes[pos - 1]) == cat {
            pos -= 1;
        }

        TextOffset(pos)
    }

    /// Move to the next word boundary.
    #[must_use]
    pub fn next_word(text: &str, current: TextOffset) -> TextOffset {
        let bytes = text.as_bytes();
        let len = bytes.len();
        let mut pos = current.0;

        if pos >= len {
            return TextOffset(len);
        }

        // Determine the category of the character at pos.
        let cat = char_category(bytes[pos]);

        // Move forward through characters of the same category.
        while pos < len && char_category(bytes[pos]) == cat {
            pos += 1;
        }

        // Skip trailing spaces.
        while pos < len && is_whitespace(bytes[pos]) {
            pos += 1;
        }

        TextOffset(pos)
    }

    /// Move to the start of the current line.
    #[must_use]
    pub fn line_start(text: &str, current: TextOffset) -> TextOffset {
        let start = text[..current.0].rfind('\n').map(|i| i + 1).unwrap_or(0);
        TextOffset(start)
    }

    /// Move to the end of the current line.
    #[must_use]
    pub fn line_end(text: &str, current: TextOffset) -> TextOffset {
        let end = text[current.0..]
            .find('\n')
            .map(|i| current.0 + i)
            .unwrap_or(text.len());
        TextOffset(end)
    }

    /// Move up one visual line.
    ///
    /// Uses x-coordinate-based column targeting: the caret's horizontal
    /// position on the current line is computed, then the nearest valid
    /// caret offset at that x on the previous line is chosen. The result is
    /// always snapped to a grapheme-cluster boundary so that subsequent
    /// editing slices never land mid-codepoint.
    #[must_use]
    pub fn prev_line(text: &str, current: TextOffset, lines: &[LayoutLine]) -> TextOffset {
        if lines.is_empty() {
            return snap_to_boundary(text, current.0);
        }

        // Find which line the cursor is on.
        let current_line = line_index_for_offset(current.0, lines);
        if current_line == 0 {
            // Already on the first line; move to start.
            return TextOffset(snap_to_boundary(text, lines[0].start).0);
        }

        let target_x = caret_x_on_line(&lines[current_line], current.0);
        let prev = &lines[current_line - 1];
        offset_at_x_on_line(text, prev, target_x)
    }

    /// Move down one visual line.
    ///
    /// Mirror of [`Self::prev_line`]; see its documentation for the
    /// boundary-safety guarantee.
    #[must_use]
    pub fn next_line(text: &str, current: TextOffset, lines: &[LayoutLine]) -> TextOffset {
        if lines.is_empty() {
            return snap_to_boundary(text, current.0);
        }

        let current_line = line_index_for_offset(current.0, lines);
        if current_line >= lines.len() - 1 {
            // Already on the last line; move to end.
            let end = lines.last().map(|l| l.end).unwrap_or(0);
            return snap_to_boundary(text, end);
        }

        let target_x = caret_x_on_line(&lines[current_line], current.0);
        let next = &lines[current_line + 1];
        offset_at_x_on_line(text, next, target_x)
    }

    /// Move to the previous paragraph boundary (before the preceding blank line).
    #[must_use]
    pub fn prev_paragraph(text: &str, current: TextOffset) -> TextOffset {
        let mut pos = current.0;

        // Skip current newlines.
        while pos > 0 && text.as_bytes()[pos - 1] == b'\n' {
            pos -= 1;
        }

        // Find previous newline.
        if let Some(nl) = text[..pos].rfind('\n') {
            TextOffset(nl + 1)
        } else {
            TextOffset(0)
        }
    }

    /// Move to the next paragraph boundary.
    #[must_use]
    pub fn next_paragraph(text: &str, current: TextOffset) -> TextOffset {
        let mut pos = current.0;

        // Skip to end of current line.
        while pos < text.len() && text.as_bytes()[pos] != b'\n' {
            pos += 1;
        }

        // Skip newlines.
        while pos < text.len() && text.as_bytes()[pos] == b'\n' {
            pos += 1;
        }

        TextOffset(pos)
    }
}

/// Find which layout line contains the given byte offset.
fn line_index_for_offset(offset: usize, lines: &[LayoutLine]) -> usize {
    for (i, line) in lines.iter().enumerate() {
        if offset >= line.start && offset <= line.end {
            return i;
        }
    }
    lines.len().saturating_sub(1)
}

/// Snap a byte offset to the nearest enclosing grapheme-cluster boundary
/// (rounding down to the start of the cluster that contains `offset`).
///
/// The returned offset is guaranteed to be a valid char boundary, so callers
/// can slice the text at it without panicking. Snapping to grapheme clusters
/// (rather than bare char boundaries) keeps vertical movement consistent with
/// the crate's grapheme-granularity horizontal navigation.
fn snap_to_boundary(text: &str, offset: usize) -> TextOffset {
    let offset = offset.min(text.len());
    if offset == 0 || offset == text.len() || text.is_char_boundary(offset) {
        // Already on a char boundary; align to the enclosing grapheme start.
        return TextOffset(snap_to_grapheme_start(text, offset));
    }
    // Mid-codepoint: round down to the previous char boundary first, then to
    // the enclosing grapheme start.
    let mut pos = offset;
    while pos > 0 && !text.is_char_boundary(pos) {
        pos -= 1;
    }
    TextOffset(snap_to_grapheme_start(text, pos))
}

/// Given a byte offset that is already on a char boundary, return the start of
/// the grapheme cluster that contains it (or the offset itself if it is a
/// cluster boundary or the text end).
fn snap_to_grapheme_start(text: &str, offset: usize) -> usize {
    if offset == 0 || offset >= text.len() {
        return offset.min(text.len());
    }
    let clusters = grapheme_clusters(text);
    for cluster in &clusters {
        if offset == cluster.start || offset == cluster.end {
            return offset;
        }
        if offset > cluster.start && offset < cluster.end {
            // Mid-cluster: round down to the cluster start.
            return cluster.start;
        }
    }
    offset
}

/// Compute the caret's horizontal x position on `line` for the given global
/// byte `offset`.
///
/// The position is derived from the glyphs' recorded x coordinates. This is a
/// best-effort horizontal estimate used only to choose a target column on the
/// adjacent line; correctness of the final offset is guaranteed independently
/// by [`snap_to_boundary`], so any imprecision here can never produce an
/// invalid (panicking) offset.
fn caret_x_on_line(line: &LayoutLine, offset: usize) -> f32 {
    let local = offset.saturating_sub(line.start);
    if line.glyphs.is_empty() {
        return 0.0;
    }
    // Find the first glyph whose cluster is at or after the target column; the
    // caret sits at that glyph's left edge. If none, the caret is past the last
    // glyph: use the line width.
    for glyph in &line.glyphs {
        if (glyph.cluster as usize) >= local {
            return glyph.x;
        }
    }
    // Caret is after the last glyph on the line.
    line.glyphs
        .last()
        .map(|g| g.x.max(line.width))
        .unwrap_or(line.width)
}

/// Choose the caret offset on `line` nearest to horizontal position
/// `target_x`, then snap it to a grapheme boundary within the line's byte
/// range.
///
/// The candidate offsets are the grapheme-cluster boundaries that fall inside
/// the line, each evaluated at the x of the glyph that begins it. The result
/// is always a valid char boundary within `[line.start, line.end]`.
fn offset_at_x_on_line(text: &str, line: &LayoutLine, target_x: f32) -> TextOffset {
    let line_start = line.start.min(text.len());
    let line_end = line.end.min(text.len());
    if line_start >= line_end {
        return snap_to_boundary(text, line_start);
    }

    // Build the set of candidate caret offsets: every grapheme boundary within
    // the line, paired with the x of the glyph that begins at that boundary.
    let slice = &text[line_start..line_end];
    let clusters = grapheme_clusters(slice);

    let mut best_offset = line_start;
    let mut best_dist = f32::INFINITY;

    // Helper to evaluate a candidate global offset at a given x.
    let consider = |global_offset: usize, x: f32, best_offset: &mut usize, best_dist: &mut f32| {
        let dist = (x - target_x).abs();
        if dist < *best_dist {
            *best_dist = dist;
            *best_offset = global_offset;
        }
    };

    // Candidate at the start of each cluster.
    for cluster in &clusters {
        let global = line_start + cluster.start;
        let local = cluster.start;
        let x = line
            .glyphs
            .iter()
            .find(|g| (g.cluster as usize) >= local)
            .map(|g| g.x)
            .unwrap_or(line.width);
        consider(global, x, &mut best_offset, &mut best_dist);
    }
    // Candidate at the end of the line (after the last cluster).
    consider(line_end, line.width, &mut best_offset, &mut best_dist);

    snap_to_boundary(text, best_offset)
}

/// Character categories for word boundary detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CharCategory {
    Word,
    Punctuation,
    Space,
    Other,
}

fn char_category(b: u8) -> CharCategory {
    if b.is_ascii_alphanumeric() || b == b'_' {
        CharCategory::Word
    } else if b.is_ascii_punctuation() {
        CharCategory::Punctuation
    } else if is_whitespace(b) {
        CharCategory::Space
    } else {
        CharCategory::Other
    }
}

fn is_whitespace(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_next_grapheme_ascii() {
        let offset = CaretNavigator::next_grapheme("Hello", TextOffset(0));
        assert_eq!(offset.0, 1);
    }

    #[test]
    fn test_prev_grapheme_ascii() {
        let offset = CaretNavigator::prev_grapheme("Hello", TextOffset(3));
        assert_eq!(offset.0, 2);
    }

    #[test]
    fn test_prev_grapheme_at_start() {
        let offset = CaretNavigator::prev_grapheme("Hello", TextOffset(0));
        assert_eq!(offset.0, 0);
    }

    #[test]
    fn test_next_grapheme_at_end() {
        let offset = CaretNavigator::next_grapheme("Hello", TextOffset(5));
        assert_eq!(offset.0, 5);
    }

    #[test]
    fn test_next_word() {
        let offset = CaretNavigator::next_word("Hello World", TextOffset(0));
        assert_eq!(offset.0, 6); // After "Hello " to "W"
    }

    #[test]
    fn test_prev_word() {
        let offset = CaretNavigator::prev_word("Hello World", TextOffset(11));
        assert_eq!(offset.0, 6); // Start of "World"
    }

    #[test]
    fn test_line_start() {
        let text = "Line 1\nLine 2\nLine 3";
        let offset = CaretNavigator::line_start(text, TextOffset(10));
        assert_eq!(offset.0, 7); // Start of "Line 2"
    }

    #[test]
    fn test_line_end() {
        let text = "Line 1\nLine 2\nLine 3";
        let offset = CaretNavigator::line_end(text, TextOffset(10));
        assert_eq!(offset.0, 13); // End of "Line 2"
    }

    #[test]
    fn test_next_paragraph() {
        let text = "Para 1\n\nPara 2";
        let offset = CaretNavigator::next_paragraph(text, TextOffset(0));
        assert_eq!(offset.0, 8); // Start of "Para 2"
    }

    #[test]
    fn test_prev_paragraph() {
        let text = "Para 1\n\nPara 2";
        let offset = CaretNavigator::prev_paragraph(text, TextOffset(10));
        assert_eq!(offset.0, 8); // Start of "Para 2"
    }

    #[test]
    fn test_document_start_end() {
        let text = "Hello World";
        let start = CaretNavigator::move_caret(
            text,
            TextOffset(5),
            MoveDirection::Left,
            MoveGranularity::Document,
            &[],
        );
        assert_eq!(start.0, 0);

        let end = CaretNavigator::move_caret(
            text,
            TextOffset(5),
            MoveDirection::Right,
            MoveGranularity::Document,
            &[],
        );
        assert_eq!(end.0, text.len());
    }

    #[test]
    fn test_word_boundary_punctuation() {
        let text = "hello.world";
        let offset = CaretNavigator::next_word(text, TextOffset(0));
        // "hello" is a word, then "." is punctuation, then "world"
        assert_eq!(offset.0, 5); // After "hello"
    }

    // ─── Vertical-movement boundary safety (regression: t49-e3-F13) ──────

    use crate::font_fallback::FontId;
    use crate::paragraph::PositionedGlyph;

    /// Build a `LayoutLine` whose glyphs carry the given (line-local cluster,
    /// x) pairs, spanning the global byte range `[start, end)`.
    fn line(start: usize, end: usize, width: f32, glyphs: &[(u32, f32)]) -> LayoutLine {
        LayoutLine {
            glyphs: glyphs
                .iter()
                .map(|&(cluster, x)| PositionedGlyph {
                    glyph_id: 0,
                    font_id: FontId(1),
                    size: 16.0,
                    x,
                    y: 0.0,
                    cluster,
                })
                .collect(),
            start,
            end,
            baseline_y: 0.0,
            ascent: 12.0,
            descent: 4.0,
            width,
            hard_break: true,
        }
    }

    /// Vertical movement over multi-byte (emoji / CJK / combining) text must
    /// never land on a byte offset that is not a char boundary — otherwise a
    /// subsequent edit slices the string mid-codepoint and panics.
    #[test]
    fn test_vertical_movement_lands_on_char_boundary() {
        // Two lines of mixed-width multi-byte text.
        //   line 0: "a😀b"   bytes: a(1) 😀(4) b(1) → 6 bytes, indices 0,1,5,6
        //   line 1: "日本語" bytes: 3×3 = 9, indices 0,3,6,9
        let l0 = "a😀b"; // 6 bytes
        let l1 = "日本語"; // 9 bytes
        let text = format!("{l0}\n{l1}");
        let l0_len = l0.len();
        let l1_start = l0_len + 1; // skip '\n'

        let lines = [
            line(0, l0_len, 30.0, &[(0, 0.0), (1, 10.0), (5, 20.0)]),
            line(
                l1_start,
                l1_start + l1.len(),
                45.0,
                &[(0, 0.0), (3, 15.0), (6, 30.0)],
            ),
        ];

        // From every char boundary on line 0, move down; result must be a valid
        // char boundary inside the text.
        for off in [0usize, 1, 5, 6] {
            let down = CaretNavigator::next_line(&text, TextOffset(off), &lines);
            assert!(
                text.is_char_boundary(down.0),
                "down from {off} landed at {} (not a char boundary)",
                down.0
            );
            // And it must be on the next line's byte range.
            assert!(down.0 >= l1_start && down.0 <= l1_start + l1.len());
        }

        // From every char boundary on line 1, move up; result must be valid.
        for off in [l1_start, l1_start + 3, l1_start + 6, l1_start + 9] {
            let up = CaretNavigator::prev_line(&text, TextOffset(off), &lines);
            assert!(
                text.is_char_boundary(up.0),
                "up from {off} landed at {} (not a char boundary)",
                up.0
            );
            assert!(up.0 <= l0_len);
        }
    }

    /// Even when handed a deliberately bogus mid-codepoint offset, vertical
    /// movement snaps the result to a valid boundary.
    #[test]
    fn test_vertical_movement_snaps_bogus_offset() {
        let l0 = "😀😀"; // 8 bytes
        let l1 = "xy"; // 2 bytes
        let text = format!("{l0}\n{l1}");
        let l1_start = l0.len() + 1;
        let lines = [
            line(0, l0.len(), 20.0, &[(0, 0.0), (4, 10.0)]),
            line(l1_start, l1_start + l1.len(), 20.0, &[(0, 0.0), (1, 10.0)]),
        ];

        // Offset 2 is mid-emoji on line 0; moving down must still produce a
        // valid char boundary.
        let down = CaretNavigator::next_line(&text, TextOffset(2), &lines);
        assert!(text.is_char_boundary(down.0));

        // A mid-emoji offset that resolves onto line 0 via prev_line bounds.
        let up = CaretNavigator::prev_line(&text, TextOffset(l1_start + 1), &lines);
        assert!(text.is_char_boundary(up.0));
        assert!(up.0 <= l0.len());
    }

    #[test]
    fn test_vertical_movement_empty_lines_is_safe() {
        let text = "café"; // 5 bytes, é = 2 bytes at 3..5
        let up = CaretNavigator::prev_line(text, TextOffset(4), &[]);
        assert!(text.is_char_boundary(up.0));
        let down = CaretNavigator::next_line(text, TextOffset(4), &[]);
        assert!(text.is_char_boundary(down.0));
    }
}
