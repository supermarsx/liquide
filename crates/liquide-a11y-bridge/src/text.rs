//! Text accessibility for the bridge layer.
//!
//! Implements the WAI-ARIA text interface: character / word / line / sentence
//! / paragraph boundary queries, caret and selection tracking, and text
//! attribute runs.

// ---------------------------------------------------------------------------
// Text boundary
// ---------------------------------------------------------------------------

/// Boundary type for text range queries (aligned with ATK / AT-SPI
/// `AtkTextBoundary`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextBoundary {
    Char,
    Word,
    Line,
    Sentence,
    Paragraph,
    All,
}

// ---------------------------------------------------------------------------
// Text attribute
// ---------------------------------------------------------------------------

/// A text attribute describing the appearance of a run of text.
#[derive(Debug, Clone, PartialEq)]
pub struct TextAttribute {
    pub font_family: String,
    pub font_size: f64,
    pub font_weight: u16,
    pub foreground_color: (u8, u8, u8, u8),
    pub background_color: (u8, u8, u8, u8),
    pub underline: bool,
    pub strikethrough: bool,
    pub italic: bool,
}

impl TextAttribute {
    /// Create a default text attribute set.
    #[must_use]
    pub fn default_attrs() -> Self {
        Self {
            font_family: "sans-serif".to_string(),
            font_size: 14.0,
            font_weight: 400,
            foreground_color: (0, 0, 0, 255),
            background_color: (255, 255, 255, 255),
            underline: false,
            strikethrough: false,
            italic: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Accessible text trait
// ---------------------------------------------------------------------------

/// Trait for nodes that expose accessible text content (AT-SPI `Text`
/// interface).
pub trait AccessibleText {
    /// The full text content of the node.
    fn text_content(&self) -> &str;

    /// The current caret (cursor) offset within the text.
    fn caret_offset(&self) -> usize;

    /// The current selection range `(start, end)`.  Returns `None` if
    /// nothing is selected.
    fn selection_range(&self) -> Option<(usize, usize)>;

    /// Return the character at byte offset `offset` (as a `char`).
    fn char_at(&self, offset: usize) -> Option<char>;

    /// Return the word containing the character at `offset`.
    fn word_at(&self, offset: usize) -> Option<String>;

    /// Return the line containing the character at `offset`.
    fn line_at(&self, offset: usize) -> Option<String>;

    /// Return the paragraph containing the character at `offset`.
    fn paragraph_at(&self, offset: usize) -> Option<String>;
}

// ---------------------------------------------------------------------------
// get_text_at_offset  (free function)
// ---------------------------------------------------------------------------

/// Return the text segment at `offset` for the given `boundary`, together
/// with the start and end byte offsets of that segment.
///
/// Returns `("", 0, 0)` if the offset is out of range.
#[must_use]
pub fn get_text_at_offset(
    text: &str,
    offset: usize,
    boundary: TextBoundary,
) -> (String, usize, usize) {
    if offset >= text.len() {
        return (String::new(), 0, 0);
    }

    match boundary {
        TextBoundary::Char => {
            if let Some(ch) = text[offset..].chars().next() {
                let end = offset + ch.len_utf8();
                (ch.to_string(), offset, end)
            } else {
                (String::new(), 0, 0)
            }
        }
        TextBoundary::Word => extract_segment(text, offset, is_word_boundary),
        TextBoundary::Line => extract_segment(text, offset, is_line_boundary),
        TextBoundary::Sentence => extract_segment(text, offset, is_sentence_boundary),
        TextBoundary::Paragraph => extract_segment(text, offset, is_paragraph_boundary),
        TextBoundary::All => (text.to_string(), 0, text.len()),
    }
}

/// Generic helper: find the segment around `offset` where `is_boundary`
/// returns `true` for boundary positions.
fn extract_segment(
    text: &str,
    offset: usize,
    is_boundary: fn(&str, usize) -> bool,
) -> (String, usize, usize) {
    // Walk backwards to find the segment start.
    let mut start = offset;
    while start > 0 && !is_boundary(text, start) {
        start -= 1;
        // Re-align to char boundary.
        while start > 0 && !text.is_char_boundary(start) {
            start -= 1;
        }
    }

    // Walk forwards to find the segment end.
    let mut end = offset + 1;
    while end < text.len() && !is_boundary(text, end) {
        end += 1;
        while end < text.len() && !text.is_char_boundary(end) {
            end += 1;
        }
    }

    let segment = &text[start..end];
    (segment.to_string(), start, end)
}

fn is_word_boundary(text: &str, pos: usize) -> bool {
    if pos == 0 || pos >= text.len() {
        return true;
    }
    let bytes = text.as_bytes();
    let cur = bytes[pos];
    let prev = bytes[pos - 1];
    let cur_alnum = cur.is_ascii_alphanumeric() || cur == b'_';
    let prev_alnum = prev.is_ascii_alphanumeric() || prev == b'_';
    cur_alnum != prev_alnum
}

fn is_line_boundary(text: &str, pos: usize) -> bool {
    if pos == 0 || pos >= text.len() {
        return true;
    }
    text.as_bytes()[pos - 1] == b'\n'
}

fn is_sentence_boundary(text: &str, pos: usize) -> bool {
    if pos == 0 || pos >= text.len() {
        return true;
    }
    let prev = text.as_bytes()[pos - 1];
    (prev == b'.' || prev == b'!' || prev == b'?')
        && (pos >= text.len() || text.as_bytes()[pos] == b' ' || text.as_bytes()[pos] == b'\n')
}

fn is_paragraph_boundary(text: &str, pos: usize) -> bool {
    if pos == 0 || pos >= text.len() {
        return true;
    }
    // A paragraph boundary is a blank line (two consecutive newlines).
    if text.as_bytes()[pos - 1] == b'\n' {
        if pos >= 2 && text.as_bytes()[pos - 2] == b'\n' {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// SimpleAccessibleText — concrete implementation for testing / simple cases
// ---------------------------------------------------------------------------

/// A simple concrete implementation of [`AccessibleText`].
#[derive(Debug, Clone)]
pub struct SimpleAccessibleText {
    pub text: String,
    pub caret: usize,
    pub selection: Option<(usize, usize)>,
}

impl SimpleAccessibleText {
    #[must_use]
    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
            caret: 0,
            selection: None,
        }
    }

    /// Set the caret offset.
    pub fn set_caret(&mut self, offset: usize) {
        self.caret = offset.min(self.text.len());
    }

    /// Set the selection range.
    pub fn set_selection(&mut self, start: usize, end: usize) {
        self.selection = Some((start.min(self.text.len()), end.min(self.text.len())));
    }

    /// Clear the selection.
    pub fn clear_selection(&mut self) {
        self.selection = None;
    }
}

impl AccessibleText for SimpleAccessibleText {
    fn text_content(&self) -> &str {
        &self.text
    }

    fn caret_offset(&self) -> usize {
        self.caret
    }

    fn selection_range(&self) -> Option<(usize, usize)> {
        self.selection
    }

    fn char_at(&self, offset: usize) -> Option<char> {
        self.text[offset..].chars().next()
    }

    fn word_at(&self, offset: usize) -> Option<String> {
        let (word, _, _) = get_text_at_offset(&self.text, offset, TextBoundary::Word);
        if word.is_empty() { None } else { Some(word) }
    }

    fn line_at(&self, offset: usize) -> Option<String> {
        let (line, _, _) = get_text_at_offset(&self.text, offset, TextBoundary::Line);
        if line.is_empty() { None } else { Some(line) }
    }

    fn paragraph_at(&self, offset: usize) -> Option<String> {
        let (para, _, _) = get_text_at_offset(&self.text, offset, TextBoundary::Paragraph);
        if para.is_empty() { None } else { Some(para) }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn char_boundary() {
        let (s, start, end) = get_text_at_offset("hello", 1, TextBoundary::Char);
        assert_eq!(s, "e");
        assert_eq!(start, 1);
        assert_eq!(end, 2);
    }

    #[test]
    fn char_boundary_first() {
        let (s, start, end) = get_text_at_offset("hello", 0, TextBoundary::Char);
        assert_eq!(s, "h");
        assert_eq!(start, 0);
        assert_eq!(end, 1);
    }

    #[test]
    fn char_boundary_out_of_range() {
        let (s, _, _) = get_text_at_offset("hello", 100, TextBoundary::Char);
        assert!(s.is_empty());
    }

    #[test]
    fn word_boundary_simple() {
        let (word, start, end) = get_text_at_offset("hello world", 2, TextBoundary::Word);
        assert_eq!(word, "hello");
        assert_eq!(start, 0);
        assert_eq!(end, 5);
    }

    #[test]
    fn word_boundary_second_word() {
        let (word, start, end) = get_text_at_offset("hello world", 7, TextBoundary::Word);
        assert_eq!(word, "world");
        assert_eq!(start, 6);
        assert_eq!(end, 11);
    }

    #[test]
    fn line_boundary() {
        let text = "line one\nline two\nline three";
        let (line, start, _end) = get_text_at_offset(text, 10, TextBoundary::Line);
        assert_eq!(start, 9);
        assert!(line.starts_with("line two"));
    }

    #[test]
    fn sentence_boundary() {
        let text = "First sentence. Second sentence.";
        let (sent, start, _end) = get_text_at_offset(text, 17, TextBoundary::Sentence);
        // The sentence boundary is after ". " — the boundary detector
        // places it at byte 16 (space after period).  The segment start
        // is where the boundary function returns true.
        assert!(start <= 17);
        assert!(sent.contains("Second"));
    }

    #[test]
    fn all_boundary() {
        let text = "full text here";
        let (s, start, end) = get_text_at_offset(text, 5, TextBoundary::All);
        assert_eq!(s, text);
        assert_eq!(start, 0);
        assert_eq!(end, text.len());
    }

    #[test]
    fn simple_text_char_at() {
        let t = SimpleAccessibleText::new("abc");
        assert_eq!(t.char_at(0), Some('a'));
        assert_eq!(t.char_at(1), Some('b'));
        assert_eq!(t.char_at(2), Some('c'));
        assert_eq!(t.char_at(3), None);
    }

    #[test]
    fn simple_text_caret() {
        let mut t = SimpleAccessibleText::new("hello");
        assert_eq!(t.caret_offset(), 0);
        t.set_caret(3);
        assert_eq!(t.caret_offset(), 3);
        t.set_caret(999);
        assert_eq!(t.caret_offset(), 5); // clamped
    }

    #[test]
    fn simple_text_selection() {
        let mut t = SimpleAccessibleText::new("hello world");
        assert!(t.selection_range().is_none());
        t.set_selection(0, 5);
        assert_eq!(t.selection_range(), Some((0, 5)));
        t.clear_selection();
        assert!(t.selection_range().is_none());
    }

    #[test]
    fn simple_text_word_at() {
        let t = SimpleAccessibleText::new("hello world");
        assert_eq!(t.word_at(0), Some("hello".to_string()));
        assert_eq!(t.word_at(7), Some("world".to_string()));
    }

    #[test]
    fn simple_text_line_at() {
        let t = SimpleAccessibleText::new("line1\nline2");
        let line = t.line_at(7).unwrap();
        assert!(line.contains("line2"));
    }

    #[test]
    fn text_attribute_default() {
        let a = TextAttribute::default_attrs();
        assert_eq!(a.font_size, 14.0);
        assert_eq!(a.font_weight, 400);
        assert!(!a.underline);
        assert!(!a.strikethrough);
        assert!(!a.italic);
        assert_eq!(a.font_family, "sans-serif");
    }

    #[test]
    fn text_attribute_custom() {
        let mut a = TextAttribute::default_attrs();
        a.font_family = "monospace".to_string();
        a.font_size = 16.0;
        a.font_weight = 700;
        a.underline = true;
        a.italic = true;
        assert_eq!(a.font_family, "monospace");
        assert_eq!(a.font_size, 16.0);
        assert_eq!(a.font_weight, 700);
        assert!(a.underline);
        assert!(a.italic);
    }

    #[test]
    fn text_content_trait() {
        let t = SimpleAccessibleText::new("hello");
        assert_eq!(t.text_content(), "hello");
    }

    #[test]
    fn empty_text() {
        let t = SimpleAccessibleText::new("");
        assert_eq!(t.text_content(), "");
        assert_eq!(t.char_at(0), None);
        assert!(t.word_at(0).is_none());
    }
}
