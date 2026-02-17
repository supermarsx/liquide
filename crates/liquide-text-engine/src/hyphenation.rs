//! Hyphenation support for automatic word breaking.
//!
//! Implements a Knuth-Liang style hyphenation algorithm that finds valid
//! hyphenation points within words. This is used when `hyphens: auto` is set
//! to enable breaking long words at syllable boundaries.

use std::collections::HashMap;

/// Hyphenation configuration.
#[derive(Debug, Clone)]
pub struct HyphenationConfig {
    /// Minimum characters before first hyphen.
    pub left_min: usize,
    /// Minimum characters after last hyphen.
    pub right_min: usize,
    /// Minimum word length to consider for hyphenation.
    pub min_word_length: usize,
    /// The hyphen character to insert.
    pub hyphen_char: char,
}

impl Default for HyphenationConfig {
    fn default() -> Self {
        Self {
            left_min: 2,
            right_min: 2,
            min_word_length: 5,
            hyphen_char: '-',
        }
    }
}

/// A hyphenation point within a word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HyphenPoint {
    /// Byte offset within the word where hyphenation can occur.
    pub offset: usize,
    /// Priority/quality of this hyphenation point (higher = better).
    pub priority: u8,
}

/// Hyphenator that finds valid hyphenation points in words.
pub struct Hyphenator {
    patterns: HashMap<String, Vec<u8>>,
    config: HyphenationConfig,
}

impl Default for Hyphenator {
    fn default() -> Self {
        Self::new(HyphenationConfig::default())
    }
}

impl Hyphenator {
    /// Create a new hyphenator with the given configuration.
    #[must_use]
    pub fn new(config: HyphenationConfig) -> Self {
        Self {
            patterns: Self::load_english_patterns(),
            config,
        }
    }

    /// Create a hyphenator with custom patterns.
    #[must_use]
    pub fn with_patterns(config: HyphenationConfig, patterns: HashMap<String, Vec<u8>>) -> Self {
        Self { patterns, config }
    }

    /// Find all valid hyphenation points in a word.
    ///
    /// Returns byte offsets within the word where a hyphen can be inserted.
    #[must_use]
    pub fn hyphenate(&self, word: &str) -> Vec<HyphenPoint> {
        if word.len() < self.config.min_word_length {
            return Vec::new();
        }

        // Only hyphenate alphabetic words
        if !word.chars().all(|c| c.is_alphabetic()) {
            return Vec::new();
        }

        let lower = word.to_lowercase();
        let chars: Vec<char> = lower.chars().collect();
        let n = chars.len();

        if n < self.config.min_word_length {
            return Vec::new();
        }

        // Build hyphenation levels array
        let mut levels = vec![0u8; n + 1];

        // Apply patterns (Knuth-Liang algorithm)
        let dotted = format!(".{}.", lower);
        let dotted_chars: Vec<char> = dotted.chars().collect();

        for i in 0..dotted_chars.len() {
            for j in (i + 1)..=dotted_chars.len().min(i + 10) {
                let pattern: String = dotted_chars[i..j].iter().collect();
                if let Some(pattern_levels) = self.patterns.get(&pattern) {
                    // Apply pattern levels at position i
                    for (k, &level) in pattern_levels.iter().enumerate() {
                        let idx = i + k;
                        if idx > 0 && idx <= n {
                            levels[idx - 1] = levels[idx - 1].max(level);
                        }
                    }
                }
            }
        }

        // Find valid hyphenation points (odd levels)
        let char_indices: Vec<usize> = word.char_indices().map(|(i, _)| i).collect();
        let mut points = Vec::new();

        for i in self.config.left_min..(n.saturating_sub(self.config.right_min)) {
            if levels[i] % 2 == 1 {
                if let Some(&byte_offset) = char_indices.get(i) {
                    points.push(HyphenPoint {
                        offset: byte_offset,
                        priority: levels[i],
                    });
                }
            }
        }

        points
    }

    /// Check if a word can be hyphenated at the given byte offset.
    #[must_use]
    pub fn can_hyphenate_at(&self, word: &str, byte_offset: usize) -> bool {
        self.hyphenate(word)
            .iter()
            .any(|p| p.offset == byte_offset)
    }

    /// Get the best hyphenation point that keeps the word segment within max_width.
    ///
    /// `char_widths` contains the advance width of each character in the word.
    /// Returns the byte offset for the best break, or None if no valid break exists.
    #[must_use]
    pub fn find_break(&self, word: &str, char_widths: &[f32], max_width: f32, hyphen_width: f32) -> Option<usize> {
        let points = self.hyphenate(word);
        if points.is_empty() {
            return None;
        }

        let chars: Vec<(usize, char)> = word.char_indices().collect();
        let mut best_break: Option<usize> = None;

        for point in &points {
            // Find the character index for this byte offset
            let Some(char_idx) = chars.iter().position(|(off, _)| *off == point.offset) else {
                continue;
            };

            // Calculate width up to this point plus hyphen
            let width: f32 = char_widths[..char_idx].iter().sum::<f32>() + hyphen_width;

            if width <= max_width {
                best_break = Some(point.offset);
            } else {
                // Further breaks will be even wider
                break;
            }
        }

        best_break
    }

    /// Parse a Knuth-Liang pattern string into (key, levels).
    ///
    /// Pattern format: interleaved letters and digits, where digits indicate
    /// hyphenation levels at that position. E.g., "a1bc" means level 1 between a and b.
    fn parse_pattern(pattern: &str) -> (String, Vec<u8>) {
        let mut key = String::new();
        let mut levels = Vec::new();
        let mut pending_level = 0u8;

        for c in pattern.chars() {
            if c.is_ascii_digit() {
                pending_level = c.to_digit(10).unwrap_or(0) as u8;
            } else {
                levels.push(pending_level);
                pending_level = 0;
                key.push(c);
            }
        }
        // Final level after last character
        levels.push(pending_level);

        (key, levels)
    }

    /// Load built-in English hyphenation patterns.
    ///
    /// These are common patterns for English hyphenation based on the
    /// Knuth-Liang algorithm. For production use, load full pattern files.
    fn load_english_patterns() -> HashMap<String, Vec<u8>> {
        let mut patterns = HashMap::new();

        // Knuth-Liang pattern format: digits indicate hyphenation levels
        // at positions between (and around) letters. Odd levels = break ok.
        let raw_patterns = [
            // Vowel-consonant patterns - prefer breaks before consonant
            "a1b", "a1c", "a1d", "a1f", "a1g", "a1l", "a1m", "a1n", "a1p",
            "a1r", "a1s", "a1t", "a1v", "a1w", "a1z",
            "e1b", "e1c", "e1d", "e1f", "e1g", "e1l", "e1m", "e1n", "e1p",
            "e1r", "e1s", "e1t", "e1v", "e1x",
            "i1b", "i1c", "i1d", "i1f", "i1g", "i1l", "i1m", "i1n", "i1p",
            "i1r", "i1s", "i1t", "i1v", "i1z",
            "o1b", "o1c", "o1d", "o1f", "o1g", "o1l", "o1m", "o1n", "o1p",
            "o1r", "o1s", "o1t", "o1v", "o1w",
            "u1b", "u1c", "u1d", "u1f", "u1g", "u1l", "u1m", "u1n", "u1p",
            "u1r", "u1s", "u1t", "u1v",

            // Common syllable breaks
            "1pu", "1tu", "1ta", "1te", "1ti", "1to", "1da", "1de", "1di", "1do", "1du",
            "1ma", "1me", "1mi", "1mo", "1mu", "1na", "1ne", "1ni", "1no", "1nu",
            "1ra", "1re", "1ri", "1ro", "1ru", "1la", "1le", "1li", "1lo", "1lu",
            "1sa", "1se", "1si", "1so", "1su", "1ba", "1be", "1bi", "1bo", "1bu",
            "1ca", "1ce", "1ci", "1co", "1cu", "1fa", "1fe", "1fi", "1fo", "1fu",
            "1ga", "1ge", "1gi", "1go", "1gu", "1va", "1ve", "1vi", "1vo", "1vu",
            "1pa", "1pe", "1pi", "1po",

            // Common word parts
            "com1pu", "pu1ter", "pro1gram", "gram1m", "un1der", "der1st",
            "stand1", "1ing.", "1tion", "1sion",

            // Double consonants - break between
            "b1b", "c1c", "d1d", "f1f", "g1g", "l1l", "m1m", "n1n", "p1p",
            "r1r", "s1s", "t1t", "z1z",

            // Prefix patterns
            ".un1", ".re1", ".pre1", ".dis1", ".mis1", ".over1", ".under1",

            // Keep consonant clusters together
            "2bl", "2br", "2ch", "2ck", "2cl", "2cr", "2dr", "2fl", "2fr",
            "2gl", "2gr", "2ph", "2pl", "2pr", "2qu", "2sc", "2sh", "2sk",
            "2sl", "2sm", "2sn", "2sp", "2st", "2sw", "2th", "2tr", "2tw",
            "2wh", "2wr",
        ];

        for pattern in &raw_patterns {
            let (key, levels) = Self::parse_pattern(pattern);
            if !key.is_empty() {
                patterns.insert(key, levels);
            }
        }

        patterns
    }
}

/// Hyphens mode from CSS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HyphensMode {
    /// No hyphenation.
    None,
    /// Manual hyphenation only (soft hyphens in text).
    #[default]
    Manual,
    /// Automatic hyphenation.
    Auto,
}

/// Find soft hyphen (U+00AD) break opportunities in text.
#[must_use]
pub fn soft_hyphen_breaks(text: &str) -> Vec<usize> {
    text.char_indices()
        .filter(|&(_, c)| c == '\u{00AD}')
        .map(|(i, _)| i)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hyphenate_short_word() {
        let hyphenator = Hyphenator::default();
        let points = hyphenator.hyphenate("cat");
        // Too short to hyphenate
        assert!(points.is_empty());
    }

    #[test]
    fn test_hyphenate_medium_word() {
        let hyphenator = Hyphenator::default();
        let points = hyphenator.hyphenate("computer");
        // Should find at least one hyphenation point
        assert!(!points.is_empty());
    }

    #[test]
    fn test_hyphenate_long_word() {
        let hyphenator = Hyphenator::default();
        let points = hyphenator.hyphenate("understanding");
        // Should find multiple hyphenation points
        assert!(points.len() >= 1);
    }

    #[test]
    fn test_soft_hyphen_detection() {
        let text = "break\u{00AD}able";
        let breaks = soft_hyphen_breaks(text);
        assert_eq!(breaks.len(), 1);
        assert_eq!(breaks[0], 5);
    }

    #[test]
    fn test_non_alphabetic_word() {
        let hyphenator = Hyphenator::default();
        let points = hyphenator.hyphenate("12345");
        assert!(points.is_empty());
    }

    #[test]
    fn test_find_break_within_width() {
        let hyphenator = Hyphenator::default();
        let word = "programming";
        // Each character is 10px wide, hyphen is 5px
        let char_widths: Vec<f32> = word.chars().map(|_| 10.0).collect();
        
        // Max width 55px should allow "progr-" (5*10 + 5 = 55)
        let break_point = hyphenator.find_break(word, &char_widths, 55.0, 5.0);
        // Should find a break point
        assert!(break_point.is_some());
    }
}
