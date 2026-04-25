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
            0x200E => Self::L, // LRM
            0x200F => Self::R, // RLM
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
            0x0023..=0x0025
            | 0x00A2..=0x00A5
            | 0x00B0..=0x00B1
            | 0x058F
            | 0x09F2..=0x09F3
            | 0x20A0..=0x20CF => Self::ET,
            // Common separators (must come before Arabic letters range)
            0x002C | 0x002E | 0x002F | 0x003A | 0x00A0 | 0x202F => Self::CS,
            // Non-spacing marks (must come before Arabic letters and Hebrew)
            0x0300..=0x036F | 0x0483..=0x0489 | 0xFE20..=0xFE2F => Self::NSM,
            // Arabic numbers (must come before Arabic letters range)
            0x0660..=0x0669 => Self::AN,
            // Arabic-specific: numbers, terminators, separators within Arabic block
            0x06F0..=0x06F9 => Self::EN,
            0x0609..=0x060A | 0x066A => Self::ET,
            0x060C => Self::CS,
            0x0591..=0x05BD => Self::NSM,
            0x0610..=0x061A
            | 0x064B..=0x065F
            | 0x0670
            | 0x06D6..=0x06DC
            | 0x06DF..=0x06E4
            | 0x06E7..=0x06E8 => Self::NSM,
            // Arabic letters (general Arabic range, after specific subranges)
            0x0600..=0x06FF
            | 0x0750..=0x077F
            | 0x08A0..=0x08FF
            | 0xFB50..=0xFDFF
            | 0xFE70..=0xFEFF => Self::AL,
            // Hebrew (after NSM subranges)
            0x0590..=0x05FF | 0xFB1D..=0xFB4F => Self::R,
            // Strong left-to-right: Latin, Greek, Cyrillic, CJK, most others
            0x0041..=0x005A
            | 0x0061..=0x007A
            | 0x00C0..=0x024F
            | 0x0370..=0x03FF
            | 0x0400..=0x052F
            | 0x1100..=0x11FF
            | 0x3040..=0x30FF
            | 0x3130..=0x318F
            | 0x4E00..=0x9FFF
            | 0xAC00..=0xD7AF
            | 0xF900..=0xFAFF => Self::L,
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

/// Maximum embedding depth per UAX #9.
const MAX_DEPTH: BidiLevel = 125;

/// Override status for the directional status stack (X1-X8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OverrideStatus {
    Neutral,
    LeftToRight,
    RightToLeft,
}

/// Entry on the directional status stack (X1-X8, X5a-X5c).
#[derive(Debug, Clone, Copy)]
struct DirectionalStatus {
    level: BidiLevel,
    override_status: OverrideStatus,
    isolate_status: bool,
}

/// Bracket type for N0 paired bracket algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BracketType {
    Open,
    Close,
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
        if self.level % 2 == 0 {
            Direction::Ltr
        } else {
            Direction::Rtl
        }
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
    /// Implements rules P2–P3 (base direction), X1–X8 (explicit embeddings),
    /// X5a–X5c (isolates), W1–W7 (weak types), N0 (paired brackets),
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

        let mut classes: Vec<BidiClass> =
            chars.iter().map(|&ch| BidiClass::from_char(ch)).collect();

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

        // X1-X8, X5a-X5c: Apply explicit embedding/override/isolate rules
        apply_explicit_levels(&chars, &mut classes, &mut levels, base_level);

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
            if matches!(
                cls,
                BidiClass::ES | BidiClass::ET | BidiClass::CS | BidiClass::BN
            ) {
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

        // N0: Paired bracket algorithm
        apply_bracket_pairs(&chars, &mut classes, &levels);

        // N1/N2: Neutral types between same-direction runs get that direction;
        // otherwise get embedding direction.
        for i in 0..n {
            if matches!(
                classes[i],
                BidiClass::ON | BidiClass::WS | BidiClass::S | BidiClass::B
            ) {
                let embed_dir = if levels[i] % 2 == 0 {
                    Direction::Ltr
                } else {
                    Direction::Rtl
                };
                let prev_strong = find_prev_strong(&classes, i, embed_dir);
                let next_strong = find_next_strong(&classes, i, n, embed_dir);
                if prev_strong == next_strong {
                    classes[i] = prev_strong;
                } else {
                    classes[i] = base_dir_to_class(embed_dir);
                }
            }
        }

        // I1/I2: Assign implicit levels (using per-character embedding level)
        for i in 0..n {
            let level = levels[i];
            match classes[i] {
                BidiClass::L => {
                    if level % 2 == 1 {
                        levels[i] = level + 1;
                    }
                }
                BidiClass::R => {
                    if level % 2 == 0 {
                        levels[i] = level + 1;
                    }
                }
                BidiClass::AN | BidiClass::EN => {
                    if level % 2 == 0 {
                        levels[i] = level + 2;
                    } else {
                        levels[i] = level + 1;
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
                let end_byte = if i < char_indices.len() {
                    char_indices[i]
                } else {
                    text_len
                };
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

/// Compute the least even embedding level greater than `level`.
fn next_even_level(level: BidiLevel) -> BidiLevel {
    (level | 1) + 1
}

/// Compute the least odd embedding level greater than `level`.
fn next_odd_level(level: BidiLevel) -> BidiLevel {
    (level + 1) | 1
}

/// Apply explicit embedding/override/isolate rules (X1-X8, X5a-X5c).
///
/// Sets per-character embedding levels and resolves directional overrides.
/// Explicit formatting characters are set to BN class.
fn apply_explicit_levels(
    chars: &[char],
    classes: &mut [BidiClass],
    levels: &mut [BidiLevel],
    base_level: BidiLevel,
) {
    let n = chars.len();
    if n == 0 {
        return;
    }

    // X1: Initialize directional status stack
    let mut stack = Vec::with_capacity(MAX_DEPTH as usize + 2);
    stack.push(DirectionalStatus {
        level: base_level,
        override_status: OverrideStatus::Neutral,
        isolate_status: false,
    });

    let mut overflow_isolate_count: u32 = 0;
    let mut overflow_embedding_count: u32 = 0;
    let mut valid_isolate_count: u32 = 0;

    for i in 0..n {
        let cls = classes[i];
        let current = *stack.last().unwrap();

        match cls {
            // X2: Left-to-Right Embedding
            BidiClass::LRE => {
                let new_level = next_even_level(current.level);
                if new_level <= MAX_DEPTH
                    && overflow_isolate_count == 0
                    && overflow_embedding_count == 0
                {
                    stack.push(DirectionalStatus {
                        level: new_level,
                        override_status: OverrideStatus::Neutral,
                        isolate_status: false,
                    });
                } else if overflow_isolate_count == 0 {
                    overflow_embedding_count += 1;
                }
                levels[i] = current.level;
                classes[i] = BidiClass::BN;
            }

            // X3: Right-to-Left Embedding
            BidiClass::RLE => {
                let new_level = next_odd_level(current.level);
                if new_level <= MAX_DEPTH
                    && overflow_isolate_count == 0
                    && overflow_embedding_count == 0
                {
                    stack.push(DirectionalStatus {
                        level: new_level,
                        override_status: OverrideStatus::Neutral,
                        isolate_status: false,
                    });
                } else if overflow_isolate_count == 0 {
                    overflow_embedding_count += 1;
                }
                levels[i] = current.level;
                classes[i] = BidiClass::BN;
            }

            // X4: Left-to-Right Override
            BidiClass::LRO => {
                let new_level = next_even_level(current.level);
                if new_level <= MAX_DEPTH
                    && overflow_isolate_count == 0
                    && overflow_embedding_count == 0
                {
                    stack.push(DirectionalStatus {
                        level: new_level,
                        override_status: OverrideStatus::LeftToRight,
                        isolate_status: false,
                    });
                } else if overflow_isolate_count == 0 {
                    overflow_embedding_count += 1;
                }
                levels[i] = current.level;
                classes[i] = BidiClass::BN;
            }

            // X5: Right-to-Left Override
            BidiClass::RLO => {
                let new_level = next_odd_level(current.level);
                if new_level <= MAX_DEPTH
                    && overflow_isolate_count == 0
                    && overflow_embedding_count == 0
                {
                    stack.push(DirectionalStatus {
                        level: new_level,
                        override_status: OverrideStatus::RightToLeft,
                        isolate_status: false,
                    });
                } else if overflow_isolate_count == 0 {
                    overflow_embedding_count += 1;
                }
                levels[i] = current.level;
                classes[i] = BidiClass::BN;
            }

            // X5a: Left-to-Right Isolate
            BidiClass::LRI => {
                levels[i] = current.level;
                if current.override_status == OverrideStatus::LeftToRight {
                    classes[i] = BidiClass::L;
                } else if current.override_status == OverrideStatus::RightToLeft {
                    classes[i] = BidiClass::R;
                }
                let new_level = next_even_level(current.level);
                if new_level <= MAX_DEPTH
                    && overflow_isolate_count == 0
                    && overflow_embedding_count == 0
                {
                    valid_isolate_count += 1;
                    stack.push(DirectionalStatus {
                        level: new_level,
                        override_status: OverrideStatus::Neutral,
                        isolate_status: true,
                    });
                } else {
                    overflow_isolate_count += 1;
                }
            }

            // X5b: Right-to-Left Isolate
            BidiClass::RLI => {
                levels[i] = current.level;
                if current.override_status == OverrideStatus::LeftToRight {
                    classes[i] = BidiClass::L;
                } else if current.override_status == OverrideStatus::RightToLeft {
                    classes[i] = BidiClass::R;
                }
                let new_level = next_odd_level(current.level);
                if new_level <= MAX_DEPTH
                    && overflow_isolate_count == 0
                    && overflow_embedding_count == 0
                {
                    valid_isolate_count += 1;
                    stack.push(DirectionalStatus {
                        level: new_level,
                        override_status: OverrideStatus::Neutral,
                        isolate_status: true,
                    });
                } else {
                    overflow_isolate_count += 1;
                }
            }

            // X5c: First Strong Isolate
            BidiClass::FSI => {
                let dir = determine_isolate_direction(chars, classes, i + 1);
                levels[i] = current.level;
                if current.override_status == OverrideStatus::LeftToRight {
                    classes[i] = BidiClass::L;
                } else if current.override_status == OverrideStatus::RightToLeft {
                    classes[i] = BidiClass::R;
                }
                let new_level = if dir == Direction::Rtl {
                    next_odd_level(current.level)
                } else {
                    next_even_level(current.level)
                };
                if new_level <= MAX_DEPTH
                    && overflow_isolate_count == 0
                    && overflow_embedding_count == 0
                {
                    valid_isolate_count += 1;
                    stack.push(DirectionalStatus {
                        level: new_level,
                        override_status: OverrideStatus::Neutral,
                        isolate_status: true,
                    });
                } else {
                    overflow_isolate_count += 1;
                }
            }

            // PDI: Pop Directional Isolate
            BidiClass::PDI => {
                if overflow_isolate_count > 0 {
                    overflow_isolate_count -= 1;
                } else if valid_isolate_count > 0 {
                    overflow_embedding_count = 0;
                    // Pop until we find an isolate entry
                    while stack.len() > 1 {
                        if stack.last().unwrap().isolate_status {
                            stack.pop();
                            break;
                        }
                        stack.pop();
                    }
                    valid_isolate_count -= 1;
                }
                let current = *stack.last().unwrap();
                levels[i] = current.level;
                if current.override_status == OverrideStatus::LeftToRight {
                    classes[i] = BidiClass::L;
                } else if current.override_status == OverrideStatus::RightToLeft {
                    classes[i] = BidiClass::R;
                }
            }

            // X7: Pop Directional Formatting (PDF)
            BidiClass::PDF => {
                if overflow_isolate_count > 0 {
                    // Do nothing — PDF is ignored within overflow isolates
                } else if overflow_embedding_count > 0 {
                    overflow_embedding_count -= 1;
                } else if stack.len() >= 2 && !stack.last().unwrap().isolate_status {
                    stack.pop();
                }
                levels[i] = stack.last().unwrap().level;
                classes[i] = BidiClass::BN;
            }

            // X8: Paragraph separator — reset stack
            BidiClass::B => {
                levels[i] = base_level;
                stack.clear();
                stack.push(DirectionalStatus {
                    level: base_level,
                    override_status: OverrideStatus::Neutral,
                    isolate_status: false,
                });
                overflow_isolate_count = 0;
                overflow_embedding_count = 0;
                valid_isolate_count = 0;
            }

            // X6: All other characters — apply current embedding level and override
            _ => {
                levels[i] = current.level;
                match current.override_status {
                    OverrideStatus::LeftToRight => classes[i] = BidiClass::L,
                    OverrideStatus::RightToLeft => classes[i] = BidiClass::R,
                    OverrideStatus::Neutral => {}
                }
            }
        }
    }
}

/// Determine the paragraph direction of an isolate content (for FSI / X5c).
/// Scans from `start` looking for the first strong type, respecting nested isolates.
fn determine_isolate_direction(chars: &[char], classes: &[BidiClass], start: usize) -> Direction {
    let mut isolate_depth: u32 = 0;
    for i in start..chars.len() {
        match classes[i] {
            BidiClass::LRI | BidiClass::RLI | BidiClass::FSI => {
                isolate_depth += 1;
            }
            BidiClass::PDI => {
                if isolate_depth == 0 {
                    break;
                }
                isolate_depth -= 1;
            }
            _ if isolate_depth == 0 => match classes[i] {
                BidiClass::L => return Direction::Ltr,
                BidiClass::R | BidiClass::AL => return Direction::Rtl,
                _ => {}
            },
            _ => {}
        }
    }
    Direction::Ltr
}

/// N0: Paired bracket algorithm.
///
/// For each bracket pair, determines whether to resolve the brackets to L or R
/// based on the strong types between them and the embedding context.
fn apply_bracket_pairs(chars: &[char], classes: &mut [BidiClass], levels: &[BidiLevel]) {
    let pairs = find_bracket_pairs(chars);

    for (open_pos, close_pos) in pairs {
        let embedding_level = levels[open_pos];
        let embedding_dir = if embedding_level % 2 == 0 {
            BidiClass::L
        } else {
            BidiClass::R
        };
        let opposite_dir = if embedding_dir == BidiClass::L {
            BidiClass::R
        } else {
            BidiClass::L
        };

        let mut found_embedding_dir = false;
        let mut found_opposite = false;

        for i in (open_pos + 1)..close_pos {
            let cls = classes[i];
            if cls == embedding_dir {
                found_embedding_dir = true;
            } else if cls == opposite_dir {
                found_opposite = true;
            } else if cls == BidiClass::EN || cls == BidiClass::AN {
                // EN and AN are treated as R for bracket resolution
                if embedding_dir == BidiClass::R {
                    found_embedding_dir = true;
                } else {
                    found_opposite = true;
                }
            }
        }

        let resolved = if found_embedding_dir {
            // N0b: strong type matching embedding direction
            embedding_dir
        } else if found_opposite {
            // N0c: check context before opening bracket
            let context = find_strong_context_before(classes, open_pos, embedding_dir);
            context
        } else {
            // N0d: no strong types between brackets — leave as-is
            continue;
        };

        classes[open_pos] = resolved;
        classes[close_pos] = resolved;
    }
}

/// Find matched bracket pairs in the text.
///
/// Returns pairs as (open_position, close_position) sorted by open position.
/// Maximum stack depth of 63 per UAX #9.
fn find_bracket_pairs(chars: &[char]) -> Vec<(usize, usize)> {
    let mut pairs = Vec::new();
    let mut stack: Vec<(usize, char)> = Vec::new();

    for (i, &ch) in chars.iter().enumerate() {
        if let Some((paired, btype)) = bracket_info(ch) {
            match btype {
                BracketType::Open => {
                    if stack.len() < 63 {
                        stack.push((i, paired));
                    }
                }
                BracketType::Close => {
                    for j in (0..stack.len()).rev() {
                        if stack[j].1 == ch {
                            pairs.push((stack[j].0, i));
                            stack.truncate(j);
                            break;
                        }
                    }
                }
            }
        }
    }

    pairs.sort_by_key(|&(open, _)| open);
    pairs
}

/// Look up bracket pairing information for a character.
///
/// Returns `(paired_character, bracket_type)` if the character is a Unicode
/// paired bracket (subset of BidiBrackets.txt covering common brackets).
fn bracket_info(ch: char) -> Option<(char, BracketType)> {
    match ch {
        '(' => Some((')', BracketType::Open)),
        ')' => Some(('(', BracketType::Close)),
        '[' => Some((']', BracketType::Open)),
        ']' => Some(('[', BracketType::Close)),
        '{' => Some(('}', BracketType::Open)),
        '}' => Some(('{', BracketType::Close)),
        '\u{0F3A}' => Some(('\u{0F3B}', BracketType::Open)),
        '\u{0F3B}' => Some(('\u{0F3A}', BracketType::Close)),
        '\u{0F3C}' => Some(('\u{0F3D}', BracketType::Open)),
        '\u{0F3D}' => Some(('\u{0F3C}', BracketType::Close)),
        '\u{169B}' => Some(('\u{169C}', BracketType::Open)),
        '\u{169C}' => Some(('\u{169B}', BracketType::Close)),
        '\u{2045}' => Some(('\u{2046}', BracketType::Open)),
        '\u{2046}' => Some(('\u{2045}', BracketType::Close)),
        '\u{207D}' => Some(('\u{207E}', BracketType::Open)),
        '\u{207E}' => Some(('\u{207D}', BracketType::Close)),
        '\u{208D}' => Some(('\u{208E}', BracketType::Open)),
        '\u{208E}' => Some(('\u{208D}', BracketType::Close)),
        '\u{2308}' => Some(('\u{2309}', BracketType::Open)),
        '\u{2309}' => Some(('\u{2308}', BracketType::Close)),
        '\u{230A}' => Some(('\u{230B}', BracketType::Open)),
        '\u{230B}' => Some(('\u{230A}', BracketType::Close)),
        '\u{2329}' => Some(('\u{232A}', BracketType::Open)),
        '\u{232A}' => Some(('\u{2329}', BracketType::Close)),
        '\u{27C5}' => Some(('\u{27C6}', BracketType::Open)),
        '\u{27C6}' => Some(('\u{27C5}', BracketType::Close)),
        '\u{27E6}' => Some(('\u{27E7}', BracketType::Open)),
        '\u{27E7}' => Some(('\u{27E6}', BracketType::Close)),
        '\u{27E8}' => Some(('\u{27E9}', BracketType::Open)),
        '\u{27E9}' => Some(('\u{27E8}', BracketType::Close)),
        '\u{27EA}' => Some(('\u{27EB}', BracketType::Open)),
        '\u{27EB}' => Some(('\u{27EA}', BracketType::Close)),
        '\u{27EC}' => Some(('\u{27ED}', BracketType::Open)),
        '\u{27ED}' => Some(('\u{27EC}', BracketType::Close)),
        '\u{27EE}' => Some(('\u{27EF}', BracketType::Open)),
        '\u{27EF}' => Some(('\u{27EE}', BracketType::Close)),
        '\u{2983}' => Some(('\u{2984}', BracketType::Open)),
        '\u{2984}' => Some(('\u{2983}', BracketType::Close)),
        '\u{2985}' => Some(('\u{2986}', BracketType::Open)),
        '\u{2986}' => Some(('\u{2985}', BracketType::Close)),
        '\u{2987}' => Some(('\u{2988}', BracketType::Open)),
        '\u{2988}' => Some(('\u{2987}', BracketType::Close)),
        '\u{2989}' => Some(('\u{298A}', BracketType::Open)),
        '\u{298A}' => Some(('\u{2989}', BracketType::Close)),
        '\u{298B}' => Some(('\u{298C}', BracketType::Open)),
        '\u{298C}' => Some(('\u{298B}', BracketType::Close)),
        '\u{298D}' => Some(('\u{2990}', BracketType::Open)),
        '\u{2990}' => Some(('\u{298D}', BracketType::Close)),
        '\u{298F}' => Some(('\u{298E}', BracketType::Open)),
        '\u{298E}' => Some(('\u{298F}', BracketType::Close)),
        '\u{2991}' => Some(('\u{2992}', BracketType::Open)),
        '\u{2992}' => Some(('\u{2991}', BracketType::Close)),
        '\u{2993}' => Some(('\u{2994}', BracketType::Open)),
        '\u{2994}' => Some(('\u{2993}', BracketType::Close)),
        '\u{2995}' => Some(('\u{2996}', BracketType::Open)),
        '\u{2996}' => Some(('\u{2995}', BracketType::Close)),
        '\u{2997}' => Some(('\u{2998}', BracketType::Open)),
        '\u{2998}' => Some(('\u{2997}', BracketType::Close)),
        '\u{29D8}' => Some(('\u{29D9}', BracketType::Open)),
        '\u{29D9}' => Some(('\u{29D8}', BracketType::Close)),
        '\u{29DA}' => Some(('\u{29DB}', BracketType::Open)),
        '\u{29DB}' => Some(('\u{29DA}', BracketType::Close)),
        '\u{29FC}' => Some(('\u{29FD}', BracketType::Open)),
        '\u{29FD}' => Some(('\u{29FC}', BracketType::Close)),
        '\u{3008}' => Some(('\u{3009}', BracketType::Open)),
        '\u{3009}' => Some(('\u{3008}', BracketType::Close)),
        '\u{300A}' => Some(('\u{300B}', BracketType::Open)),
        '\u{300B}' => Some(('\u{300A}', BracketType::Close)),
        '\u{300C}' => Some(('\u{300D}', BracketType::Open)),
        '\u{300D}' => Some(('\u{300C}', BracketType::Close)),
        '\u{300E}' => Some(('\u{300F}', BracketType::Open)),
        '\u{300F}' => Some(('\u{300E}', BracketType::Close)),
        '\u{3010}' => Some(('\u{3011}', BracketType::Open)),
        '\u{3011}' => Some(('\u{3010}', BracketType::Close)),
        '\u{3014}' => Some(('\u{3015}', BracketType::Open)),
        '\u{3015}' => Some(('\u{3014}', BracketType::Close)),
        '\u{3016}' => Some(('\u{3017}', BracketType::Open)),
        '\u{3017}' => Some(('\u{3016}', BracketType::Close)),
        '\u{3018}' => Some(('\u{3019}', BracketType::Open)),
        '\u{3019}' => Some(('\u{3018}', BracketType::Close)),
        '\u{301A}' => Some(('\u{301B}', BracketType::Open)),
        '\u{301B}' => Some(('\u{301A}', BracketType::Close)),
        '\u{FE59}' => Some(('\u{FE5A}', BracketType::Open)),
        '\u{FE5A}' => Some(('\u{FE59}', BracketType::Close)),
        '\u{FE5B}' => Some(('\u{FE5C}', BracketType::Open)),
        '\u{FE5C}' => Some(('\u{FE5B}', BracketType::Close)),
        '\u{FE5D}' => Some(('\u{FE5E}', BracketType::Open)),
        '\u{FE5E}' => Some(('\u{FE5D}', BracketType::Close)),
        '\u{FF08}' => Some(('\u{FF09}', BracketType::Open)),
        '\u{FF09}' => Some(('\u{FF08}', BracketType::Close)),
        '\u{FF3B}' => Some(('\u{FF3D}', BracketType::Open)),
        '\u{FF3D}' => Some(('\u{FF3B}', BracketType::Close)),
        '\u{FF5B}' => Some(('\u{FF5D}', BracketType::Open)),
        '\u{FF5D}' => Some(('\u{FF5B}', BracketType::Close)),
        '\u{FF5F}' => Some(('\u{FF60}', BracketType::Open)),
        '\u{FF60}' => Some(('\u{FF5F}', BracketType::Close)),
        _ => None,
    }
}

/// Find the strong type before a bracket for N0c context resolution.
fn find_strong_context_before(
    classes: &[BidiClass],
    open_pos: usize,
    embedding_dir: BidiClass,
) -> BidiClass {
    for i in (0..open_pos).rev() {
        match classes[i] {
            BidiClass::L | BidiClass::R => return classes[i],
            BidiClass::EN | BidiClass::AN => return BidiClass::R,
            _ => continue,
        }
    }
    embedding_dir
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
        let run_ltr = BidiRun {
            start: 0,
            end: 5,
            level: 0,
        };
        assert_eq!(run_ltr.direction(), Direction::Ltr);

        let run_rtl = BidiRun {
            start: 0,
            end: 5,
            level: 1,
        };
        assert_eq!(run_rtl.direction(), Direction::Rtl);
    }

    #[test]
    fn test_visual_reorder() {
        let runs = vec![
            BidiRun {
                start: 0,
                end: 5,
                level: 0,
            },
            BidiRun {
                start: 5,
                end: 10,
                level: 1,
            },
            BidiRun {
                start: 10,
                end: 15,
                level: 0,
            },
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

    // === X Rules: Explicit Embeddings ===

    #[test]
    fn test_x2_lre_embedding() {
        // LRE (U+202A) creates an LTR embedding inside RTL context
        let text = format!("אבג{}Hello{}דהו", '\u{202A}', '\u{202C}');
        let result = BidiResolver::resolve(&text, Some(Direction::Rtl));
        assert_eq!(result.base_direction, Direction::Rtl);
        // "Hello" should be at even (LTR) level within RTL context
        let chars: Vec<char> = text.chars().collect();
        let hello_start = chars.iter().position(|&c| c == 'H').unwrap();
        assert!(
            result.levels[hello_start] % 2 == 0,
            "LRE should create even embedding level"
        );
        assert!(
            result.levels[hello_start] >= 2,
            "LRE level should be >= 2 in RTL context"
        );
    }

    #[test]
    fn test_x3_rle_embedding() {
        // RLE (U+202B) creates an RTL embedding inside LTR context
        let text = format!("Hello {}אבג{} World", '\u{202B}', '\u{202C}');
        let result = BidiResolver::resolve(&text, Some(Direction::Ltr));
        let chars: Vec<char> = text.chars().collect();
        let alef_pos = chars.iter().position(|&c| c == 'א').unwrap();
        assert!(
            result.levels[alef_pos] % 2 == 1,
            "RLE should create odd embedding level"
        );
    }

    #[test]
    fn test_x4_lro_override() {
        // LRO (U+202D) forces LTR override — even RTL chars become L
        let text = format!("{}אבג{}", '\u{202D}', '\u{202C}');
        let result = BidiResolver::resolve(&text, Some(Direction::Ltr));
        // The Hebrew chars under LRO should be at even level (overridden to L)
        // LRO sets override=L, so X6 changes classes to L
        for i in 1..4 {
            assert!(
                result.levels[i] % 2 == 0,
                "LRO override should make chars LTR at position {}",
                i
            );
        }
    }

    #[test]
    fn test_x5_rlo_override() {
        // RLO (U+202E) forces RTL override — even LTR chars become R
        let text = format!("{}Hello{}", '\u{202E}', '\u{202C}');
        let result = BidiResolver::resolve(&text, Some(Direction::Ltr));
        let chars: Vec<char> = text.chars().collect();
        let h_pos = chars.iter().position(|&c| c == 'H').unwrap();
        assert!(
            result.levels[h_pos] % 2 == 1,
            "RLO override should make Latin chars RTL"
        );
    }

    #[test]
    fn test_x7_pdf_pops_embedding() {
        // PDF (U+202C) should pop the embedding stack
        let text = format!("A{}B{}C", '\u{202B}', '\u{202C}');
        let result = BidiResolver::resolve(&text, Some(Direction::Ltr));
        let chars: Vec<char> = text.chars().collect();
        let a_pos = chars.iter().position(|&c| c == 'A').unwrap();
        let c_pos = chars.iter().position(|&c| c == 'C').unwrap();
        // A and C should be at base level (0)
        assert_eq!(result.levels[a_pos], 0, "A should be at base level");
        // After PDF, C should return to base level
        assert_eq!(
            result.levels[c_pos], 0,
            "C should return to base level after PDF"
        );
    }

    #[test]
    fn test_x8_paragraph_separator_resets() {
        // Paragraph separator should reset the embedding stack
        let text = format!("{}A\u{2029}B", '\u{202B}');
        let result = BidiResolver::resolve(&text, Some(Direction::Ltr));
        let chars: Vec<char> = text.chars().collect();
        let b_pos = chars.iter().position(|&c| c == 'B').unwrap();
        // After paragraph separator, B should be at base level
        assert_eq!(
            result.levels[b_pos], 0,
            "B should be at base level after paragraph separator"
        );
    }

    #[test]
    fn test_max_depth_overflow() {
        // Exceeding MAX_DEPTH (125) should not crash; overflow embeddings are ignored
        let mut text = String::new();
        for _ in 0..130 {
            text.push('\u{202A}'); // LRE
        }
        text.push('A');
        for _ in 0..130 {
            text.push('\u{202C}'); // PDF
        }
        let result = BidiResolver::resolve(&text, Some(Direction::Ltr));
        // Should not panic; levels should be capped
        assert!(result.levels.iter().all(|&l| l <= MAX_DEPTH + 2));
    }

    // === Isolate Controls (X5a-X5c) ===

    #[test]
    fn test_x5a_lri_isolate() {
        // LRI (U+2066) creates an LTR isolate
        let text = format!("אבג{}Hello{}", '\u{2066}', '\u{2069}');
        let result = BidiResolver::resolve(&text, Some(Direction::Rtl));
        let chars: Vec<char> = text.chars().collect();
        let h_pos = chars.iter().position(|&c| c == 'H').unwrap();
        assert!(
            result.levels[h_pos] % 2 == 0,
            "LRI should create LTR embedding for 'Hello'"
        );
    }

    #[test]
    fn test_x5b_rli_isolate() {
        // RLI (U+2067) creates an RTL isolate
        let text = format!("Hello {}אבג{} World", '\u{2067}', '\u{2069}');
        let result = BidiResolver::resolve(&text, Some(Direction::Ltr));
        let chars: Vec<char> = text.chars().collect();
        let alef_pos = chars.iter().position(|&c| c == 'א').unwrap();
        assert!(
            result.levels[alef_pos] % 2 == 1,
            "RLI should create RTL embedding for Hebrew"
        );
    }

    #[test]
    fn test_x5c_fsi_auto_ltr() {
        // FSI (U+2068) should auto-detect direction — LTR content → acts as LRI
        let text = format!("אבג{}Hello{}", '\u{2068}', '\u{2069}');
        let result = BidiResolver::resolve(&text, Some(Direction::Rtl));
        let chars: Vec<char> = text.chars().collect();
        let h_pos = chars.iter().position(|&c| c == 'H').unwrap();
        assert!(
            result.levels[h_pos] % 2 == 0,
            "FSI with LTR content should act as LRI"
        );
    }

    #[test]
    fn test_x5c_fsi_auto_rtl() {
        // FSI (U+2068) with RTL first strong → acts as RLI
        let text = format!("Hello {}אבג{} World", '\u{2068}', '\u{2069}');
        let result = BidiResolver::resolve(&text, Some(Direction::Ltr));
        let chars: Vec<char> = text.chars().collect();
        let alef_pos = chars.iter().position(|&c| c == 'א').unwrap();
        assert!(
            result.levels[alef_pos] % 2 == 1,
            "FSI with RTL content should act as RLI"
        );
    }

    #[test]
    fn test_pdi_pops_isolate() {
        // PDI (U+2069) should close the isolate, returning to parent level
        let text = format!("A{}B{}C", '\u{2066}', '\u{2069}');
        let result = BidiResolver::resolve(&text, Some(Direction::Ltr));
        let chars: Vec<char> = text.chars().collect();
        let a_pos = chars.iter().position(|&c| c == 'A').unwrap();
        let c_pos = chars.iter().position(|&c| c == 'C').unwrap();
        assert_eq!(
            result.levels[a_pos], result.levels[c_pos],
            "After PDI, should return to pre-isolate level"
        );
    }

    #[test]
    fn test_nested_isolates() {
        // Nested isolates: LRI inside RLI
        let text = format!(
            "Hello {}אבג{}World{}{} End",
            '\u{2067}', // RLI
            '\u{2066}', // LRI
            '\u{2069}', // PDI (closes LRI)
            '\u{2069}', // PDI (closes RLI)
        );
        let result = BidiResolver::resolve(&text, Some(Direction::Ltr));
        let chars: Vec<char> = text.chars().collect();
        let alef_pos = chars.iter().position(|&c| c == 'א').unwrap();
        let w_pos = chars.iter().position(|&c| c == 'W').unwrap();
        // Hebrew in RLI should be RTL
        assert!(result.levels[alef_pos] % 2 == 1);
        // "World" in nested LRI should be LTR
        assert!(result.levels[w_pos] % 2 == 0);
    }

    #[test]
    fn test_overflow_isolate() {
        // Overflow isolates should be handled gracefully
        let mut text = String::new();
        for _ in 0..130 {
            text.push('\u{2066}'); // LRI
        }
        text.push('A');
        for _ in 0..130 {
            text.push('\u{2069}'); // PDI
        }
        let result = BidiResolver::resolve(&text, Some(Direction::Ltr));
        assert!(!result.levels.is_empty());
    }

    // === N0: Paired Bracket Algorithm ===

    #[test]
    fn test_n0_brackets_ltr_context() {
        // Brackets containing LTR content in LTR context → resolve to L
        let text = "(Hello)";
        let result = BidiResolver::resolve(text, Some(Direction::Ltr));
        // All chars should be LTR
        assert!(result.levels.iter().all(|&l| l % 2 == 0));
    }

    #[test]
    fn test_n0_brackets_rtl_content_in_rtl() {
        // Brackets containing RTL content in RTL context → resolve to R
        let text = format!("(אבג)");
        let result = BidiResolver::resolve(&text, Some(Direction::Rtl));
        // Brackets should resolve according to RTL embedding
        let chars: Vec<char> = text.chars().collect();
        let alef_pos = chars.iter().position(|&c| c == 'א').unwrap();
        assert!(
            result.levels[alef_pos] % 2 == 1,
            "Hebrew in brackets should be RTL"
        );
    }

    #[test]
    fn test_n0_nested_brackets() {
        // Nested brackets: ([Hello])
        let text = "([Hello])";
        let result = BidiResolver::resolve(text, Some(Direction::Ltr));
        assert!(result.levels.iter().all(|&l| l % 2 == 0));
    }

    #[test]
    fn test_bracket_info_common() {
        assert_eq!(bracket_info('('), Some((')', BracketType::Open)));
        assert_eq!(bracket_info(')'), Some(('(', BracketType::Close)));
        assert_eq!(bracket_info('['), Some((']', BracketType::Open)));
        assert_eq!(bracket_info(']'), Some(('[', BracketType::Close)));
        assert_eq!(bracket_info('{'), Some(('}', BracketType::Open)));
        assert_eq!(bracket_info('}'), Some(('{', BracketType::Close)));
        assert_eq!(bracket_info('A'), None);
    }

    #[test]
    fn test_find_bracket_pairs_simple() {
        let chars: Vec<char> = "(AB)".chars().collect();
        let pairs = find_bracket_pairs(&chars);
        assert_eq!(pairs, vec![(0, 3)]);
    }

    #[test]
    fn test_find_bracket_pairs_nested() {
        let chars: Vec<char> = "([A])".chars().collect();
        let pairs = find_bracket_pairs(&chars);
        assert_eq!(pairs.len(), 2);
        // Inner pair first by open position: '[' at 1, ']' at 3
        // Outer pair: '(' at 0, ')' at 4
        assert!(pairs.contains(&(0, 4)));
        assert!(pairs.contains(&(1, 3)));
    }

    #[test]
    fn test_find_bracket_pairs_unmatched() {
        let chars: Vec<char> = "(A[B)".chars().collect();
        let pairs = find_bracket_pairs(&chars);
        // '[' at 2 is not closed before ')' at 4 — implementation truncates stack
        // so '(' at 0 matches ')' at 4, and '[' is unmatched
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0], (0, 4));
    }

    // === Helper function tests ===

    #[test]
    fn test_next_even_level() {
        assert_eq!(next_even_level(0), 2);
        assert_eq!(next_even_level(1), 2);
        assert_eq!(next_even_level(2), 4);
        assert_eq!(next_even_level(3), 4);
        assert_eq!(next_even_level(124), 126);
    }

    #[test]
    fn test_next_odd_level() {
        assert_eq!(next_odd_level(0), 1);
        assert_eq!(next_odd_level(1), 3);
        assert_eq!(next_odd_level(2), 3);
        assert_eq!(next_odd_level(3), 5);
        assert_eq!(next_odd_level(124), 125);
    }

    #[test]
    fn test_determine_isolate_direction_ltr() {
        let chars: Vec<char> = "Hello".chars().collect();
        let classes: Vec<BidiClass> = chars.iter().map(|&c| BidiClass::from_char(c)).collect();
        assert_eq!(
            determine_isolate_direction(&chars, &classes, 0),
            Direction::Ltr
        );
    }

    #[test]
    fn test_determine_isolate_direction_rtl() {
        let chars: Vec<char> = "אבג".chars().collect();
        let classes: Vec<BidiClass> = chars.iter().map(|&c| BidiClass::from_char(c)).collect();
        assert_eq!(
            determine_isolate_direction(&chars, &classes, 0),
            Direction::Rtl
        );
    }

    #[test]
    fn test_determine_isolate_direction_nested() {
        // FSI with nested isolate: should skip nested content
        let text = format!("{} אבג {} Hello", '\u{2066}', '\u{2069}');
        let chars: Vec<char> = text.chars().collect();
        let classes: Vec<BidiClass> = chars.iter().map(|&c| BidiClass::from_char(c)).collect();
        // Looking from position 0: LRI (isolate), should skip nested, find 'Hello'
        // Actually the LRI at 0 starts a nested isolate, so searching from 0 skips until PDI
        // then finds 'Hello' → LTR
        let dir = determine_isolate_direction(&chars, &classes, 0);
        // The LRI opens an isolate, Hebrew is inside it (skipped), then 'Hello' after PDI
        assert_eq!(dir, Direction::Ltr);
    }

    // === Integration: existing behavior preserved ===

    #[test]
    fn test_pure_ltr_unchanged() {
        let result = BidiResolver::resolve("Hello world", None);
        assert_eq!(result.base_direction, Direction::Ltr);
        assert!(result.levels.iter().all(|&l| l == 0));
    }

    #[test]
    fn test_pure_rtl_unchanged() {
        let result = BidiResolver::resolve("مرحبا", None);
        assert_eq!(result.base_direction, Direction::Rtl);
        assert!(result.levels.iter().all(|&l| l % 2 == 1));
    }

    #[test]
    fn test_mixed_unchanged() {
        let result = BidiResolver::resolve("Hello مرحبا world", None);
        assert_eq!(result.base_direction, Direction::Ltr);
        assert!(result.runs.len() >= 2);
    }
}
