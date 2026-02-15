//! Unicode Bidirectional Algorithm (UAX #9).
//!
//! Implements the Unicode Bidi Algorithm for proper rendering of mixed
//! left-to-right and right-to-left text (e.g., Arabic/Hebrew with Latin).
//!
//! The algorithm resolves *embedding levels* for each character:
//! - Even levels → left-to-right
//! - Odd levels → right-to-left
//!
//! Then reorders runs for visual display.

use serde::{Deserialize, Serialize};

/// Bidi embedding level (0–125). Even = LTR, odd = RTL.
pub type BidiLevel = u8;

/// Bidi character class (simplified from the full UAX #9 set).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BidiClass {
    /// Strong left-to-right (Latin, CJK, etc.)
    L,
    /// Strong right-to-left (Hebrew)
    R,
    /// Arabic letter
    AL,
    /// European number (0–9)
    EN,
    /// European separator (+ -)
    ES,
    /// European terminator (# $)
    ET,
    /// Arabic number
    AN,
    /// Common separator (, . /)
    CS,
    /// Non-spacing mark
    NSM,
    /// Boundary neutral
    BN,
    /// Paragraph separator
    B,
    /// Segment separator
    S,
    /// White space
    WS,
    /// Other neutral
    ON,
    // Explicit embedding/override markers
    /// Left-to-right embedding
    LRE,
    /// Right-to-left embedding
    RLE,
    /// Left-to-right override
    LRO,
    /// Right-to-left override
    RLO,
    /// Pop directional formatting
    PDF,
    /// Left-to-right isolate
    LRI,
    /// Right-to-left isolate
    RLI,
    /// First strong isolate
    FSI,
    /// Pop directional isolate
    PDI,
}

impl BidiClass {
    /// Classify a Unicode code point into its bidi class.
    #[must_use]
    pub fn from_char(ch: char) -> Self {
        let cp = ch as u32;
        match cp {
            // Paragraph separators
            0x000A | 0x000D | 0x001C..=0x001E | 0x0085 | 0x2029 => Self::B,
            // Segment separator
            0x0009 | 0x001F => Self::S,
            // White space
            0x000C | 0x0020 | 0x1680 | 0x2000..=0x200A | 0x2028 | 0x205F | 0x3000 => Self::WS,
            // Explicit formatting
            0x200E => Self::L,   // LRM
            0x200F => Self::R,   // RLM
            0x202A => Self::LRE,
            0x202B => Self::RLE,
            0x202C => Self::PDF,
            0x202D => Self::LRO,
            0x202E => Self::RLO,
            0x2066 => Self::LRI,
            0x2067 => Self::RLI,
            0x2068 => Self::FSI,
            0x2069 => Self::PDI,
            // Boundary neutral
            0x200B..=0x200D | 0xFEFF => Self::BN,
            // European numbers (must come before Arabic letters range)
            0x0030..=0x0039 | 0x00B2..=0x00B3 | 0x00B9 => Self::EN,
            // European separators
            0x002B | 0x002D | 0x207A | 0x207B | 0x208A | 0x208B | 0x2212 => Self::ES,
            // European terminators (must come before Arabic letters range)
            0x0023..=0x0025 | 0x00A2..=0x00A5 | 0x00B0..=0x00B1 | 0x058F |
            0x09F2..=0x09F3 | 0x20A0..=0x20CF => Self::ET,
            // Common separators (must come before Arabic letters range)
            0x002C | 0x002E | 0x002F | 0x003A | 0x00A0 | 0x202F => Self::CS,
            // Non-spacing marks (must come before Arabic letters and Hebrew)
            0x0300..=0x036F | 0x0483..=0x0489 |
            0xFE20..=0xFE2F => Self::NSM,
            // Arabic numbers (must come before Arabic letters range)
            0x0660..=0x0669 => Self::AN,
            // Arabic-specific: numbers, terminators, separators within Arabic block
            0x06F0..=0x06F9 => Self::EN,
            0x0609..=0x060A | 0x066A => Self::ET,
            0x060C => Self::CS,
            0x0591..=0x05BD => Self::NSM,
            0x0610..=0x061A | 0x064B..=0x065F | 0x0670 |
            0x06D6..=0x06DC | 0x06DF..=0x06E4 | 0x06E7..=0x06E8 => Self::NSM,
            // Arabic letters (general Arabic range, after specific subranges)
            0x0600..=0x06FF | 0x0750..=0x077F | 0x08A0..=0x08FF |
            0xFB50..=0xFDFF | 0xFE70..=0xFEFF => Self::AL,
            // Hebrew (after NSM subranges)
            0x0590..=0x05FF | 0xFB1D..=0xFB4F => Self::R,
            // Strong left-to-right: Latin, Greek, Cyrillic, CJK, most others
            0x0041..=0x005A | 0x0061..=0x007A | 0x00C0..=0x024F |
            0x0370..=0x03FF | 0x0400..=0x052F |
            0x1100..=0x11FF | 0x3040..=0x30FF | 0x3130..=0x318F |
            0x4E00..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF => Self::L,
            // Default: most characters are strong LTR or neutral
            _ => {
                if cp >= 0x0041 && cp <= 0x1FFFF {
                    Self::L
                } else {
                    Self::ON
                }
            }
        }
    }

    /// Whether this class represents a strong direction.
    #[must_use]
    pub fn is_strong(&self) -> bool {
        matches!(self, Self::L | Self::R | Self::AL)
    }
}

/// Direction for a paragraph or run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    /// Left-to-right.
    Ltr,
    /// Right-to-left.
    Rtl,
}

/// A resolved bidi run: a contiguous range of characters at the same level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BidiRun {
    /// Byte offset of the start in the source string.
    pub start: usize,
    /// Byte offset of the end (exclusive).
    pub end: usize,
    /// Resolved embedding level.
    pub level: BidiLevel,
}

impl BidiRun {
    /// The visual direction of this run.
    #[must_use]
    pub fn direction(&self) -> Direction {
        if self.level % 2 == 0 { Direction::Ltr } else { Direction::Rtl }
    }
}

/// Result of bidi analysis for a paragraph.
#[derive(Debug, Clone)]
pub struct BidiParagraph {
    /// The resolved embedding level for each character (indexed by char position).
    pub levels: Vec<BidiLevel>,
    /// The base paragraph direction.
    pub base_direction: Direction,
    /// Bidi runs (contiguous ranges at the same level).
    pub runs: Vec<BidiRun>,
}

/// Bidi resolver that implements a simplified Unicode Bidi Algorithm.
pub struct BidiResolver;

impl BidiResolver {
    /// Resolve bidi levels for a paragraph of text.
    ///
    /// Implements rules P2–P3 (base direction), W1–W7 (weak types),
    /// N1–N2 (neutral types), and I1–I2 (implicit levels).
    #[must_use]
    pub fn resolve(text: &str, base_direction: Option<Direction>) -> BidiParagraph {
        let chars: Vec<char> = text.chars().collect();
        let n = chars.len();

        if n == 0 {
            return BidiParagraph {
                levels: Vec::new(),
                base_direction: base_direction.unwrap_or(Direction::Ltr),
                runs: Vec::new(),
            };
        }

        let mut classes: Vec<BidiClass> = chars.iter().map(|&ch| BidiClass::from_char(ch)).collect();

        // P2/P3: Determine base direction from first strong character
        let base_dir = base_direction.unwrap_or_else(|| {
            for &cls in &classes {
                match cls {
                    BidiClass::L => return Direction::Ltr,
                    BidiClass::R | BidiClass::AL => return Direction::Rtl,
                    _ => continue,
                }
            }
            Direction::Ltr
        });

        let base_level: BidiLevel = if base_dir == Direction::Ltr { 0 } else { 1 };
        let mut levels = vec![base_level; n];

        // W1: NSM inherits class from previous character
        for i in 0..n {
            if classes[i] == BidiClass::NSM {
                classes[i] = if i > 0 { classes[i - 1] } else { BidiClass::ON };
            }
        }

        // W2: EN after AL becomes AN
        {
            let mut last_strong = base_dir_to_class(base_dir);
            for i in 0..n {
                if classes[i].is_strong() {
                    last_strong = classes[i];
                } else if classes[i] == BidiClass::EN && last_strong == BidiClass::AL {
                    classes[i] = BidiClass::AN;
                }
            }
        }

        // W3: AL → R
        for cls in &mut classes {
            if *cls == BidiClass::AL {
                *cls = BidiClass::R;
            }
        }

        // W4: Single ES/CS between matching number types resolves
        for i in 1..n.saturating_sub(1) {
            if classes[i] == BidiClass::ES
                && classes[i - 1] == BidiClass::EN
                && classes[i + 1] == BidiClass::EN
            {
                classes[i] = BidiClass::EN;
            }
            if classes[i] == BidiClass::CS {
                if classes[i - 1] == BidiClass::EN && classes[i + 1] == BidiClass::EN {
                    classes[i] = BidiClass::EN;
                } else if classes[i - 1] == BidiClass::AN && classes[i + 1] == BidiClass::AN {
                    classes[i] = BidiClass::AN;
                }
            }
        }

        // W5: ET adjacent to EN becomes EN
        for i in 0..n {
            if classes[i] == BidiClass::ET {
                let adjacent_en = (i > 0 && classes[i - 1] == BidiClass::EN)
                    || (i + 1 < n && classes[i + 1] == BidiClass::EN);
                if adjacent_en {
                    classes[i] = BidiClass::EN;
                }
            }
        }

        // W6: Remaining ES, ET, CS, BN → ON
        for cls in &mut classes {
            if matches!(cls, BidiClass::ES | BidiClass::ET | BidiClass::CS | BidiClass::BN) {
                *cls = BidiClass::ON;
            }
        }

        // W7: EN with LTR context → L
        {
            let mut last_strong = base_dir_to_class(base_dir);
            for i in 0..n {
                if classes[i] == BidiClass::L || classes[i] == BidiClass::R {
                    last_strong = classes[i];
                } else if classes[i] == BidiClass::EN && last_strong == BidiClass::L {
                    classes[i] = BidiClass::L;
                }
            }
        }

        // N1/N2: Neutral types between same-direction runs get that direction;
        // otherwise get base direction.
        for i in 0..n {
            if matches!(classes[i], BidiClass::ON | BidiClass::WS | BidiClass::S | BidiClass::B) {
                let prev_strong = find_prev_strong(&classes, i, base_dir);
                let next_strong = find_next_strong(&classes, i, n, base_dir);
                if prev_strong == next_strong {
                    classes[i] = prev_strong;
                } else {
                    classes[i] = base_dir_to_class(base_dir);
                }
            }
        }

        // I1/I2: Assign implicit levels
        for i in 0..n {
            match classes[i] {
                BidiClass::L => {
                    if base_level % 2 == 1 {
                        levels[i] = base_level + 1;
                    }
                }
                BidiClass::R => {
                    if base_level % 2 == 0 {
                        levels[i] = base_level + 1;
                    }
                }
                BidiClass::AN | BidiClass::EN => {
                    if base_level % 2 == 0 {
                        levels[i] = base_level + 2;
                    } else {
                        levels[i] = base_level + 1;
                    }
                }
                _ => {}
            }
        }

        // Build runs from levels
        let runs = Self::build_runs(text, &levels);

        BidiParagraph {
            levels,
            base_direction: base_dir,
            runs,
        }
    }

    /// Build bidi runs from resolved levels.
    fn build_runs(text: &str, levels: &[BidiLevel]) -> Vec<BidiRun> {
        if levels.is_empty() {
            return Vec::new();
        }

        let mut runs = Vec::new();
        let char_indices: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
        let text_len = text.len();

        let mut run_start_char = 0;
        let mut current_level = levels[0];

        for i in 1..levels.len() {
            if levels[i] != current_level {
                let start_byte = char_indices[run_start_char];
                let end_byte = if i < char_indices.len() { char_indices[i] } else { text_len };
                runs.push(BidiRun {
                    start: start_byte,
                    end: end_byte,
                    level: current_level,
                });
                run_start_char = i;
                current_level = levels[i];
            }
        }

        // Final run
        let start_byte = char_indices[run_start_char];
        runs.push(BidiRun {
            start: start_byte,
            end: text_len,
            level: current_level,
        });

        runs
    }

    /// Reorder runs for visual display (L2 rule: reverse runs at each level).
    #[must_use]
    pub fn visual_reorder(runs: &[BidiRun]) -> Vec<BidiRun> {
        if runs.is_empty() {
            return Vec::new();
        }

        let max_level = runs.iter().map(|r| r.level).max().unwrap_or(0);
        let mut result: Vec<BidiRun> = runs.to_vec();

        // Reverse contiguous sequences of runs at each level from max down to 1
        for level in (1..=max_level).rev() {
            let mut i = 0;
            while i < result.len() {
                if result[i].level >= level {
                    let start = i;
                    while i < result.len() && result[i].level >= level {
                        i += 1;
                    }
                    result[start..i].reverse();
                } else {
                    i += 1;
                }
            }
        }

        result
    }
}

fn base_dir_to_class(dir: Direction) -> BidiClass {
    match dir {
        Direction::Ltr => BidiClass::L,
        Direction::Rtl => BidiClass::R,
    }
}

fn find_prev_strong(classes: &[BidiClass], pos: usize, base_dir: Direction) -> BidiClass {
    for i in (0..pos).rev() {
        if classes[i] == BidiClass::L || classes[i] == BidiClass::R {
            return classes[i];
        }
    }
    base_dir_to_class(base_dir)
}

fn find_next_strong(classes: &[BidiClass], pos: usize, n: usize, base_dir: Direction) -> BidiClass {
    for i in (pos + 1)..n {
        if classes[i] == BidiClass::L || classes[i] == BidiClass::R {
            return classes[i];
        }
    }
    base_dir_to_class(base_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pure_ltr() {
        let result = BidiResolver::resolve("Hello world", None);
        assert_eq!(result.base_direction, Direction::Ltr);
        assert!(result.levels.iter().all(|&l| l == 0));
    }

    #[test]
    fn test_pure_rtl() {
        let result = BidiResolver::resolve("مرحبا", None);
        assert_eq!(result.base_direction, Direction::Rtl);
        assert!(result.levels.iter().all(|&l| l % 2 == 1));
    }

    #[test]
    fn test_mixed_ltr_rtl() {
        let result = BidiResolver::resolve("Hello مرحبا world", None);
        assert_eq!(result.base_direction, Direction::Ltr);
        // The Arabic portion should have odd level
        assert!(result.runs.len() >= 2);
    }

    #[test]
    fn test_empty_text() {
        let result = BidiResolver::resolve("", None);
        assert!(result.levels.is_empty());
        assert!(result.runs.is_empty());
    }

    #[test]
    fn test_explicit_base_direction() {
        let result = BidiResolver::resolve("Hello", Some(Direction::Rtl));
        assert_eq!(result.base_direction, Direction::Rtl);
    }

    #[test]
    fn test_bidi_class_detection() {
        assert_eq!(BidiClass::from_char('A'), BidiClass::L);
        assert_eq!(BidiClass::from_char('ع'), BidiClass::AL);
        assert_eq!(BidiClass::from_char('א'), BidiClass::R);
        assert_eq!(BidiClass::from_char('5'), BidiClass::EN);
        assert_eq!(BidiClass::from_char(' '), BidiClass::WS);
    }

    #[test]
    fn test_run_direction() {
        let run_ltr = BidiRun { start: 0, end: 5, level: 0 };
        assert_eq!(run_ltr.direction(), Direction::Ltr);

        let run_rtl = BidiRun { start: 0, end: 5, level: 1 };
        assert_eq!(run_rtl.direction(), Direction::Rtl);
    }

    #[test]
    fn test_visual_reorder() {
        let runs = vec![
            BidiRun { start: 0, end: 5, level: 0 },
            BidiRun { start: 5, end: 10, level: 1 },
            BidiRun { start: 10, end: 15, level: 0 },
        ];
        let reordered = BidiResolver::visual_reorder(&runs);
        assert_eq!(reordered.len(), 3);
        // Middle RTL run should be reversed
    }

    #[test]
    fn test_numbers_in_rtl() {
        let result = BidiResolver::resolve("العدد 42", None);
        assert_eq!(result.base_direction, Direction::Rtl);
        // Numbers should have level 2 in RTL context
    }
}
