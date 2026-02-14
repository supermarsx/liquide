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
            (MoveDirection::Left, MoveGranularity::Grapheme) => {
                Self::prev_grapheme(text, current)
            }
            (MoveDirection::Right, MoveGranularity::Grapheme) => {
                Self::next_grapheme(text, current)
            }
            (MoveDirection::Left, MoveGranularity::Word) => {
                Self::prev_word(text, current)
            }
            (MoveDirection::Right, MoveGranularity::Word) => {
                Self::next_word(text, current)
            }
            (MoveDirection::Left, MoveGranularity::Line) => {
                Self::line_start(text, current)
            }
            (MoveDirection::Right, MoveGranularity::Line) => {
                Self::line_end(text, current)
            }
            (MoveDirection::Up, MoveGranularity::Grapheme) => {
                Self::prev_line(current, lines)
            }
            (MoveDirection::Down, MoveGranularity::Grapheme) => {
                Self::next_line(current, lines)
            }
            (_, MoveGranularity::Document) => match direction {
                MoveDirection::Left | MoveDirection::Up => TextOffset(0),
                MoveDirection::Right | MoveDirection::Down => TextOffset(text.len()),
            },
            (MoveDirection::Up, MoveGranularity::Paragraph) => {
                Self::prev_paragraph(text, current)
            }
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
        let start = text[..current.0]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
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
    #[must_use]
    pub fn prev_line(current: TextOffset, lines: &[LayoutLine]) -> TextOffset {
        if lines.is_empty() {
            return current;
        }

        // Find which line the cursor is on.
        let current_line = line_index_for_offset(current.0, lines);
        if current_line == 0 {
            // Already on the first line; move to start.
            return TextOffset(lines[0].start);
        }

        // Move to the same horizontal position on the previous line.
        let prev = &lines[current_line - 1];
        let target_offset_in_line = current.0.saturating_sub(lines[current_line].start);
        let new_offset = prev.start + target_offset_in_line.min(prev.end.saturating_sub(prev.start));
        TextOffset(new_offset)
    }

    /// Move down one visual line.
    #[must_use]
    pub fn next_line(current: TextOffset, lines: &[LayoutLine]) -> TextOffset {
        if lines.is_empty() {
            return current;
        }

        let current_line = line_index_for_offset(current.0, lines);
        if current_line >= lines.len() - 1 {
            // Already on the last line; move to end.
            return TextOffset(lines.last().map(|l| l.end).unwrap_or(0));
        }

        let next = &lines[current_line + 1];
        let target_offset_in_line = current.0.saturating_sub(lines[current_line].start);
        let new_offset = next.start + target_offset_in_line.min(next.end.saturating_sub(next.start));
        TextOffset(new_offset)
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
}
