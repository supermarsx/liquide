//! Grapheme cluster boundary detection (UAX #29).
//!
//! Determines grapheme cluster boundaries for correct cursor movement
//! and text selection. A grapheme cluster is the user-perceived "character"
//! (e.g., a base letter + combining marks, or an emoji sequence).

/// Whether a break can occur between two characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClusterBreak {
    /// No break — characters belong to the same cluster.
    NoBreak,
    /// Break allowed — characters are in different clusters.
    Break,
}

/// Grapheme cluster category for a character (simplified UAX #29).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphemeCategory {
    /// Control character, CR, LF.
    Control,
    CR,
    LF,
    /// Spacing combining mark.
    SpacingMark,
    /// Extending character (combining marks).
    Extend,
    /// Zero-width joiner.
    ZWJ,
    /// Regional indicator.
    RegionalIndicator,
    /// Hangul syllable types.
    HangulL,
    HangulV,
    HangulT,
    HangulLV,
    HangulLVT,
    /// Prepend character.
    Prepend,
    /// Extended Pictographic (emoji base).
    ExtendedPictographic,
    /// Any other character.
    Other,
}

impl GraphemeCategory {
    /// Classify a character into its grapheme cluster category.
    #[must_use]
    pub fn from_char(ch: char) -> Self {
        let cp = ch as u32;
        match cp {
            0x000D => Self::CR,
            0x000A => Self::LF,
            0x200D => Self::ZWJ,
            // Combining marks (general range)
            0x0300..=0x036F
            | 0x0483..=0x0489
            | 0x0591..=0x05BD
            | 0x05BF
            | 0x05C1..=0x05C2
            | 0x05C4..=0x05C5
            | 0x05C7
            | 0x0610..=0x061A
            | 0x064B..=0x065F
            | 0x0670
            | 0x06D6..=0x06DC
            | 0x06DF..=0x06E4
            | 0x06E7..=0x06E8
            | 0x06EA..=0x06ED
            | 0x0711
            | 0x0730..=0x074A
            | 0x0E31
            | 0x0E34..=0x0E3A
            | 0x0EB1
            | 0x0EB4..=0x0EBC
            | 0xFE00..=0xFE0F
            | 0xFE20..=0xFE2F
            | 0x20D0..=0x20FF
            | 0x1AB0..=0x1AFF
            | 0xE0100..=0xE01EF => Self::Extend,
            // Spacing combining marks (Indic, etc.)
            0x0903
            | 0x093B
            | 0x093E..=0x0940
            | 0x0949..=0x094C
            | 0x094E..=0x094F
            | 0x0982..=0x0983
            | 0x09BE..=0x09C0
            | 0x09C7..=0x09C8
            | 0x09CB..=0x09CC
            | 0x09D7
            | 0x0A03
            | 0x0A3E..=0x0A40
            | 0x0A83 => Self::SpacingMark,
            // Regional indicators (flag emoji)
            0x1F1E6..=0x1F1FF => Self::RegionalIndicator,
            // Extended pictographic (emoji) — simplified range
            0x1F600..=0x1F64F
            | 0x1F300..=0x1F5FF
            | 0x1F680..=0x1F6FF
            | 0x1F900..=0x1F9FF
            | 0x1FA00..=0x1FA6F
            | 0x1FA70..=0x1FAFF
            | 0x2600..=0x26FF
            | 0x2700..=0x27BF
            | 0x231A..=0x231B
            | 0x23E9..=0x23F3
            | 0x23F8..=0x23FA
            | 0x25AA..=0x25AB
            | 0x25B6
            | 0x25C0
            | 0x25FB..=0x25FE => Self::ExtendedPictographic,
            // Hangul Jamo
            0x1100..=0x115F | 0xA960..=0xA97C => Self::HangulL,
            0x1160..=0x11A7 | 0xD7B0..=0xD7C6 => Self::HangulV,
            0x11A8..=0x11FF | 0xD7CB..=0xD7FB => Self::HangulT,
            // Control characters
            0x0000..=0x0009
            | 0x000B..=0x000C
            | 0x000E..=0x001F
            | 0x007F..=0x009F
            | 0x00AD
            | 0x061C
            | 0x200B
            | 0x200E..=0x200F
            | 0x2028..=0x2029
            | 0x202A..=0x202E
            | 0x2060..=0x2064
            | 0x2066..=0x206F
            | 0xFEFF
            | 0xFFF0..=0xFFF8 => Self::Control,
            // Precomposed Hangul syllables (LV and LVT)
            cp if (0xAC00..=0xD7A3).contains(&cp) => {
                // LV syllables: those where (cp - 0xAC00) % 28 == 0
                if (cp - 0xAC00) % 28 == 0 {
                    Self::HangulLV
                } else {
                    Self::HangulLVT
                }
            }
            _ => Self::Other,
        }
    }
}

/// A grapheme cluster detected in text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphemeCluster {
    /// Byte offset of the start of this cluster.
    pub start: usize,
    /// Byte offset of the end (exclusive).
    pub end: usize,
}

/// Detect grapheme cluster boundaries in text.
///
/// Returns a list of `GraphemeCluster` values, each representing one
/// user-perceived character.
#[must_use]
pub fn grapheme_clusters(text: &str) -> Vec<GraphemeCluster> {
    if text.is_empty() {
        return Vec::new();
    }

    let chars: Vec<(usize, char)> = text.char_indices().collect();
    if chars.len() == 1 {
        return vec![GraphemeCluster {
            start: 0,
            end: text.len(),
        }];
    }

    let mut clusters = Vec::new();
    let mut cluster_start = chars[0].0;

    for i in 1..chars.len() {
        let prev_cat = GraphemeCategory::from_char(chars[i - 1].1);
        let curr_cat = GraphemeCategory::from_char(chars[i].1);

        let should_break = should_break_between(prev_cat, curr_cat);

        if should_break {
            clusters.push(GraphemeCluster {
                start: cluster_start,
                end: chars[i].0,
            });
            cluster_start = chars[i].0;
        }
    }

    // Final cluster
    clusters.push(GraphemeCluster {
        start: cluster_start,
        end: text.len(),
    });

    clusters
}

/// Determine if a grapheme cluster break occurs between two categories.
///
/// Implements a simplified version of the UAX #29 rules.
fn should_break_between(prev: GraphemeCategory, curr: GraphemeCategory) -> bool {
    use GraphemeCategory::*;

    // GB3: Do not break between CR and LF
    if prev == CR && curr == LF {
        return false;
    }

    // GB4: Break after Control, CR, LF
    if matches!(prev, Control | CR | LF) {
        return true;
    }

    // GB5: Break before Control, CR, LF
    if matches!(curr, Control | CR | LF) {
        return true;
    }

    // GB6: Do not break Hangul syllable sequences
    if prev == HangulL && matches!(curr, HangulL | HangulV | HangulLV | HangulLVT) {
        return false;
    }

    // GB7
    if matches!(prev, HangulLV | HangulV) && matches!(curr, HangulV | HangulT) {
        return false;
    }

    // GB8
    if matches!(prev, HangulLVT | HangulT) && curr == HangulT {
        return false;
    }

    // GB9: Do not break before Extend or ZWJ
    if matches!(curr, Extend | ZWJ) {
        return false;
    }

    // GB9a: Do not break before SpacingMark
    if curr == SpacingMark {
        return false;
    }

    // GB9b: Do not break after Prepend
    if prev == Prepend {
        return false;
    }

    // GB11: ZWJ + ExtendedPictographic (emoji ZWJ sequence)
    if prev == ZWJ && curr == ExtendedPictographic {
        return false;
    }

    // GB12/GB13: Regional indicator pairs
    if prev == RegionalIndicator && curr == RegionalIndicator {
        return false;
    }

    // GB999: Otherwise, break
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ascii() {
        let clusters = grapheme_clusters("abc");
        assert_eq!(clusters.len(), 3);
        assert_eq!(clusters[0], GraphemeCluster { start: 0, end: 1 });
        assert_eq!(clusters[1], GraphemeCluster { start: 1, end: 2 });
        assert_eq!(clusters[2], GraphemeCluster { start: 2, end: 3 });
    }

    #[test]
    fn test_crlf() {
        let clusters = grapheme_clusters("a\r\nb");
        assert_eq!(clusters.len(), 3);
        // \r\n should be a single cluster
        assert_eq!(clusters[1], GraphemeCluster { start: 1, end: 3 });
    }

    #[test]
    fn test_combining_mark() {
        // 'e' + combining acute accent = single cluster
        let clusters = grapheme_clusters("e\u{0301}");
        assert_eq!(clusters.len(), 1);
    }

    #[test]
    fn test_hangul() {
        // Precomposed Hangul syllable
        let clusters = grapheme_clusters("한");
        assert_eq!(clusters.len(), 1);
    }

    #[test]
    fn test_empty() {
        assert!(grapheme_clusters("").is_empty());
    }

    #[test]
    fn test_single_char() {
        let clusters = grapheme_clusters("x");
        assert_eq!(clusters.len(), 1);
    }

    #[test]
    fn test_emoji() {
        // Simple emoji
        let clusters = grapheme_clusters("😀");
        assert_eq!(clusters.len(), 1);
    }

    #[test]
    fn test_multibyte_utf8() {
        let clusters = grapheme_clusters("日本語");
        assert_eq!(clusters.len(), 3);
    }

    #[test]
    fn test_categories() {
        assert_eq!(GraphemeCategory::from_char('\r'), GraphemeCategory::CR);
        assert_eq!(GraphemeCategory::from_char('\n'), GraphemeCategory::LF);
        assert_eq!(
            GraphemeCategory::from_char('\u{200D}'),
            GraphemeCategory::ZWJ
        );
        assert_eq!(GraphemeCategory::from_char('A'), GraphemeCategory::Other);
    }
}
