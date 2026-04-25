//! Unicode Line Breaking Algorithm (UAX #14).
//!
//! Determines legal line break opportunities in text for word wrapping
//! and paragraph layout.

/// Break action at a position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakAction {
    /// Line break is mandatory here (e.g., newline).
    Mandatory,
    /// Line break is allowed here (e.g., after space).
    Allowed,
    /// Line break is prohibited here.
    Prohibited,
}

/// A break opportunity at a specific byte offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BreakOpportunity {
    /// Byte offset in the source string.
    pub offset: usize,
    /// Whether the break is mandatory, allowed, or prohibited.
    pub action: BreakAction,
}

/// Line break class (simplified from full UAX #14).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum LineBreakClass {
    /// Mandatory break (BK)
    BK,
    /// Carriage return (CR)
    CR,
    /// Line feed (LF)
    LF,
    /// Space (SP)
    SP,
    /// Zero-width space (ZW)
    ZW,
    /// Opening punctuation (OP)
    OP,
    /// Closing punctuation (CL)
    CL,
    /// Quotation (QU)
    QU,
    /// Non-starter (NS) — e.g., Japanese small kana
    NS,
    /// Exclamation/Interrogation (EX)
    EX,
    /// Ideographic (ID) — CJK ideographs
    ID,
    /// Alphabetic (AL)
    AL,
    /// Numeric (NU)
    NU,
    /// Hyphen (HY)
    HY,
    /// Break after (BA)
    BA,
    /// Break before (BB)
    BB,
    /// Inseparable (IN)
    IN,
    /// Combining mark (CM)
    CM,
    /// Word joiner (WJ)
    WJ,
    /// Glue (GL)
    GL,
    /// Prefix (PR)
    PR,
    /// Postfix (PO)
    PO,
    /// Complex context (SA) — Thai, Lao, Khmer
    SA,
    /// Unknown / Other (XX)
    XX,
}

impl LineBreakClass {
    fn from_char(ch: char) -> Self {
        let cp = ch as u32;
        match cp {
            0x000A => Self::LF,
            0x000D => Self::CR,
            0x000B | 0x000C | 0x2028 | 0x2029 | 0x0085 => Self::BK,
            0x0020 => Self::SP,
            0x200B => Self::ZW,
            0x2060 | 0xFEFF => Self::WJ,
            // Opening
            0x0028 | 0x005B | 0x007B | 0x00AB | 0x2018 | 0x201C | 0x2039 | 0x3008 | 0x300A
            | 0x300C | 0x300E | 0x3010 | 0x3014 | 0x3016 | 0x3018 | 0x301A | 0xFF08 | 0xFF3B
            | 0xFF5B => Self::OP,
            // Closing
            0x0029 | 0x005D | 0x007D | 0x00BB | 0x2019 | 0x201D | 0x203A | 0x3009 | 0x300B
            | 0x300D | 0x300F | 0x3011 | 0x3015 | 0x3017 | 0x3019 | 0x301B | 0xFF09 | 0xFF3D
            | 0xFF5D => Self::CL,
            // Quotation
            0x0022 | 0x0027 => Self::QU,
            // Exclamation
            0x0021 | 0x003F | 0xFF01 | 0xFF1F => Self::EX,
            // Hyphen
            0x002D | 0x2010 | 0x2013 => Self::HY,
            // Non-starter (Japanese)
            0x3041
            | 0x3043
            | 0x3045
            | 0x3047
            | 0x3049
            | 0x3063
            | 0x3083
            | 0x3085
            | 0x3087
            | 0x308E
            | 0x3095..=0x3096
            | 0x30A1
            | 0x30A3
            | 0x30A5
            | 0x30A7
            | 0x30A9
            | 0x30C3
            | 0x30E3
            | 0x30E5
            | 0x30E7
            | 0x30EE
            | 0x30F5..=0x30F6
            | 0x3000..=0x3002
            | 0xFF0C
            | 0xFF0E
            | 0xFF1A
            | 0xFF1B => Self::NS,
            // Break after (comma, period in some contexts, etc.)
            0x0009 | 0x007C | 0x00AD | 0x058A | 0x2000..=0x200A | 0x2012 | 0x2014 => Self::BA,
            // CJK Ideographs
            0x4E00..=0x9FFF
            | 0x3400..=0x4DBF
            | 0x20000..=0x2A6DF
            | 0xF900..=0xFAFF
            | 0x2F800..=0x2FA1F => Self::ID,
            // CJK Fullwidth forms often act as ID
            0x3000..=0x303F => Self::ID,
            // Numerics
            0x0030..=0x0039 | 0x0660..=0x0669 | 0x06F0..=0x06F9 => Self::NU,
            // Combining marks
            0x0300..=0x036F | 0x0483..=0x0489 | 0x0591..=0x05BD | 0xFE20..=0xFE2F => Self::CM,
            // Thai, Lao
            0x0E01..=0x0E3A | 0x0E40..=0x0E5B | 0x0E81..=0x0EDF => Self::SA,
            // Prefix (currency before number)
            0x0024 | 0x00A3 | 0x00A5 | 0x20AC | 0x20A0..=0x20CF => Self::PR,
            // Postfix (percent after number)
            0x0025 | 0x00A2 | 0x00B0 | 0x2030..=0x2031 => Self::PO,
            // Glue (non-breaking space, etc.)
            0x00A0 | 0x202F => Self::GL,
            // Default: alphabetic
            _ if ch.is_alphabetic() => Self::AL,
            _ => Self::XX,
        }
    }
}

/// Line breaker that computes break opportunities for a given text.
pub struct LineBreaker;

impl LineBreaker {
    /// Compute all break opportunities in the given text.
    ///
    /// Returns a list of `BreakOpportunity` values indicating where
    /// line breaks may or must occur.
    #[must_use]
    pub fn break_opportunities(text: &str) -> Vec<BreakOpportunity> {
        if text.is_empty() {
            return Vec::new();
        }

        let chars: Vec<(usize, char)> = text.char_indices().collect();
        let classes: Vec<LineBreakClass> = chars
            .iter()
            .map(|&(_, ch)| LineBreakClass::from_char(ch))
            .collect();
        let n = chars.len();
        let mut opportunities = Vec::new();

        for i in 1..n {
            let prev = classes[i - 1];
            let curr = classes[i];
            let byte_offset = chars[i].0;

            let action = resolve_break(prev, curr);
            if action != BreakAction::Prohibited {
                opportunities.push(BreakOpportunity {
                    offset: byte_offset,
                    action,
                });
            }
        }

        // Always allow break at text end
        opportunities.push(BreakOpportunity {
            offset: text.len(),
            action: BreakAction::Allowed,
        });

        opportunities
    }

    /// Find the best break position to fit text within `max_width`.
    ///
    /// `char_widths` provides the advance width of each character (indexed
    /// by character position, not byte offset).
    ///
    /// Returns the byte offset where the line should break, or `text.len()`
    /// if the entire text fits.
    #[must_use]
    pub fn find_break(text: &str, char_widths: &[f32], max_width: f32) -> usize {
        let chars: Vec<(usize, char)> = text.char_indices().collect();
        let opportunities = Self::break_opportunities(text);

        let mut accumulated_width = 0.0;
        let mut last_valid_break = 0;
        let mut width_at_break = 0.0;

        // Build a byte-offset → char-index map
        let mut char_idx = 0;
        for opp in &opportunities {
            // Sum widths up to this break point
            while char_idx < chars.len() && chars[char_idx].0 < opp.offset {
                if char_idx < char_widths.len() {
                    accumulated_width += char_widths[char_idx];
                }
                char_idx += 1;
            }

            if accumulated_width <= max_width {
                last_valid_break = opp.offset;
                width_at_break = accumulated_width;
            } else {
                break;
            }

            if opp.action == BreakAction::Mandatory {
                return opp.offset;
            }
        }

        if accumulated_width <= max_width {
            return text.len();
        }

        let _ = width_at_break;

        if last_valid_break > 0 {
            last_valid_break
        } else {
            // Emergency break: no valid break found, break at first char that exceeds
            let mut w = 0.0;
            for (i, &cw) in char_widths.iter().enumerate() {
                w += cw;
                if w > max_width && i > 0 {
                    return chars.get(i).map_or(text.len(), |&(off, _)| off);
                }
            }
            text.len()
        }
    }
}

/// Resolve the break action between two adjacent characters.
fn resolve_break(prev: LineBreakClass, curr: LineBreakClass) -> BreakAction {
    use LineBreakClass::*;

    // LB4: Always break after BK
    if prev == BK {
        return BreakAction::Mandatory;
    }

    // LB5: CR + LF → no break; isolated CR/LF → mandatory
    if prev == CR && curr == LF {
        return BreakAction::Prohibited;
    }
    if prev == CR || prev == LF {
        return BreakAction::Mandatory;
    }

    // LB6: Do not break before BK, CR, LF
    if matches!(curr, BK | CR | LF) {
        return BreakAction::Prohibited;
    }

    // LB7: Do not break before SP or ZW
    if matches!(curr, SP | ZW) {
        return BreakAction::Prohibited;
    }

    // LB8: Break after ZW
    if prev == ZW {
        return BreakAction::Allowed;
    }

    // LB8a: Do not break after ZWJ (for emoji sequences)
    // (handled implicitly via CM rule)

    // LB11: Do not break before or after WJ
    if prev == WJ || curr == WJ {
        return BreakAction::Prohibited;
    }

    // LB12: Do not break after GL
    if prev == GL {
        return BreakAction::Prohibited;
    }

    // LB13: Do not break before CL, EX, NS
    if matches!(curr, CL | EX | NS) {
        return BreakAction::Prohibited;
    }

    // LB14: Do not break after OP
    if prev == OP {
        return BreakAction::Prohibited;
    }

    // LB15: Do not break between QU and OP
    if prev == QU && curr == OP {
        return BreakAction::Prohibited;
    }

    // LB16: Do not break between CL and NS
    if prev == CL && curr == NS {
        return BreakAction::Prohibited;
    }

    // LB18: Break after SP
    if prev == SP {
        return BreakAction::Allowed;
    }

    // LB19: Do not break before or after QU
    if prev == QU || curr == QU {
        return BreakAction::Prohibited;
    }

    // LB20: Break before and after BA, HY
    if curr == BA || curr == HY {
        return BreakAction::Allowed;
    }
    if prev == BA || prev == HY {
        return BreakAction::Allowed;
    }

    // LB21: Break before BB
    if curr == BB {
        return BreakAction::Allowed;
    }

    // LB23: Do not break between digits
    if prev == NU && curr == NU {
        return BreakAction::Prohibited;
    }
    if prev == AL && curr == NU {
        return BreakAction::Prohibited;
    }
    if prev == NU && curr == AL {
        return BreakAction::Prohibited;
    }

    // LB24: Do not break between prefix/postfix and numbers
    if prev == PR && curr == NU {
        return BreakAction::Prohibited;
    }
    if prev == NU && curr == PO {
        return BreakAction::Prohibited;
    }

    // LB25: Number sequences
    if prev == PR && curr == AL {
        return BreakAction::Prohibited;
    }

    // LB26–LB28: Hangul syllable sequences
    if matches!(prev, LineBreakClass::AL) && curr == AL {
        return BreakAction::Prohibited;
    }

    // LB28a: Do not break between alphabetics
    // (already covered above)

    // LB29: ID × ID — allow break between ideographs
    if prev == ID || curr == ID {
        return BreakAction::Allowed;
    }

    // LB30: CM does not cause break
    if curr == CM {
        return BreakAction::Prohibited;
    }

    // LB31: Otherwise, allow break
    BreakAction::Allowed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_words() {
        let breaks = LineBreaker::break_opportunities("hello world");
        // Should have a break opportunity at the space between words
        let space_break = breaks.iter().find(|b| b.offset == 6);
        assert!(space_break.is_some());
        assert_eq!(space_break.unwrap().action, BreakAction::Allowed);
    }

    #[test]
    fn test_mandatory_break() {
        let breaks = LineBreaker::break_opportunities("line1\nline2");
        let nl = breaks.iter().find(|b| b.action == BreakAction::Mandatory);
        assert!(nl.is_some());
    }

    #[test]
    fn test_crlf_no_break() {
        let breaks = LineBreaker::break_opportunities("line1\r\nline2");
        // Should not break between CR and LF
        let crlf_inner = breaks.iter().find(|b| b.offset == 6);
        assert!(crlf_inner.is_none());
        // CR at index 5, LF at index 6 — break prohibited between them
        // But mandatory break at offset 7 (after LF)
        let after_crlf = breaks
            .iter()
            .find(|b| b.offset == 7 && b.action == BreakAction::Mandatory);
        assert!(after_crlf.is_some());
    }

    #[test]
    fn test_cjk_breaks() {
        let breaks = LineBreaker::break_opportunities("日本語");
        // CJK ideographs allow break between each
        assert!(breaks.iter().any(|b| b.action == BreakAction::Allowed));
    }

    #[test]
    fn test_empty() {
        assert!(LineBreaker::break_opportunities("").is_empty());
    }

    #[test]
    fn test_find_break_fits() {
        let text = "ab cd";
        let widths = vec![10.0, 10.0, 5.0, 10.0, 10.0];
        let pos = LineBreaker::find_break(text, &widths, 100.0);
        assert_eq!(pos, text.len()); // everything fits
    }

    #[test]
    fn test_find_break_word_wrap() {
        let text = "hello world";
        let widths: Vec<f32> = text.chars().map(|_| 10.0).collect();
        // Total width = 110, max = 60 → should break at space
        let pos = LineBreaker::find_break(text, &widths, 60.0);
        assert!(pos <= 6); // "hello " = 60px
    }

    #[test]
    fn test_no_break_in_number() {
        let breaks = LineBreaker::break_opportunities("123456");
        // Should not break within digits
        let digit_breaks: Vec<_> = breaks
            .iter()
            .filter(|b| b.offset > 0 && b.offset < 6 && b.action == BreakAction::Allowed)
            .collect();
        assert!(digit_breaks.is_empty());
    }
}
