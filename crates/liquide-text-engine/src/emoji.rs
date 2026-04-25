//! Emoji grapheme helpers — ZWJ sequences + presentation selectors.
//!
//! Minimal, dependency-free detection suitable for segmentation inside
//! shaping. Not a full Unicode TR #51 implementation; covers the cases
//! that matter for shaping/line-break/cluster boundaries:
//!
//! * **Default-emoji codepoints** — common BMP + SMP emoji blocks.
//! * **Variation selectors** — `U+FE0E` text presentation, `U+FE0F` emoji
//!   presentation — must stay in the same cluster as the preceding
//!   codepoint.
//! * **Zero-Width Joiner (`U+200D`)** — joins adjacent emoji codepoints
//!   into a single grapheme cluster (e.g. 👨‍👩‍👧).
//! * **Regional Indicator Symbols (`U+1F1E6`..=`U+1F1FF`)** — pairs form
//!   flag sequences.
//!
//! Callers typically iterate `emoji_cluster_boundaries(text)` to obtain
//! byte offsets where a new grapheme cluster starts after ZWJ-joining.

/// Variation selectors.
pub const VS_TEXT: char = '\u{FE0E}';
pub const VS_EMOJI: char = '\u{FE0F}';
/// Zero-Width Joiner.
pub const ZWJ: char = '\u{200D}';

/// Whether a scalar is a regional indicator (flag component).
#[must_use]
pub fn is_regional_indicator(c: char) -> bool {
    let cp = c as u32;
    (0x1F1E6..=0x1F1FF).contains(&cp)
}

/// Whether a scalar is a variation selector used for emoji/text.
#[must_use]
pub fn is_variation_selector(c: char) -> bool {
    c == VS_TEXT || c == VS_EMOJI
}

/// Broad-brush "is emoji-presentation default" test covering the blocks
/// that matter for shaping. Not exhaustive; erring on the side of
/// grouping ZWJ sequences correctly rather than strict TR #51
/// conformance.
#[must_use]
pub fn is_emoji_codepoint(c: char) -> bool {
    let cp = c as u32;
    matches!(
        cp,
        // Misc Symbols & Pictographs, Emoticons, Transport & Map,
        // Supplemental Symbols & Pictographs, Symbols & Pictographs Ext-A,
        // Dingbats (subset), Misc technical
        0x2600..=0x26FF
        | 0x2700..=0x27BF
        | 0x1F300..=0x1F5FF
        | 0x1F600..=0x1F64F
        | 0x1F680..=0x1F6FF
        | 0x1F700..=0x1F77F
        | 0x1F900..=0x1F9FF
        | 0x1FA70..=0x1FAFF
        | 0x1F1E6..=0x1F1FF  // regional indicators
    )
}

/// Compute grapheme cluster boundaries that respect emoji ZWJ sequences
/// and variation selectors.
///
/// Returns a `Vec<usize>` of byte offsets where a new cluster starts
/// (always includes `0` if text is non-empty; never includes
/// `text.len()` as a boundary — use it as exclusive upper bound).
///
/// Rules applied (in order):
///   1. Variation selectors attach to the preceding scalar.
///   2. `ZWJ` between two emoji-like scalars fuses them into one cluster.
///   3. Two consecutive regional indicators form one flag cluster.
///   4. Otherwise, each scalar starts a new cluster.
#[must_use]
pub fn emoji_cluster_boundaries(text: &str) -> Vec<usize> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut boundaries = vec![0usize];
    let mut chars: Vec<(usize, char)> = text.char_indices().collect();
    chars.push((text.len(), '\0')); // sentinel so look-ahead stays in bounds
    let mut i = 0usize;
    let mut last_was_ri_pair_start = false;
    while i + 1 < chars.len() {
        let (_byte, ch) = chars[i];
        let (_nbyte, nch) = chars[i + 1];
        if is_variation_selector(nch) {
            // VS attaches to current: no boundary before i+1.
            i += 1;
            continue;
        }
        if nch == ZWJ {
            // ZWJ absorbs into current cluster; also swallow the
            // following scalar if present.
            i += 1;
            if i + 1 < chars.len() {
                // Don't emit a boundary before this next scalar.
                i += 1;
            }
            continue;
        }
        // Regional indicator pairing.
        if is_regional_indicator(ch) && is_regional_indicator(nch) && !last_was_ri_pair_start {
            // Pair: no boundary between. Consume both, mark paired.
            i += 1;
            last_was_ri_pair_start = true;
            continue;
        }
        last_was_ri_pair_start = false;
        i += 1;
        let (byte_i, _) = chars[i];
        if byte_i < text.len() {
            boundaries.push(byte_i);
        }
    }
    boundaries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_ascii() {
        let b = emoji_cluster_boundaries("abc");
        assert_eq!(b, vec![0, 1, 2]);
    }

    #[test]
    fn emoji_with_vs16() {
        // heart ❤ + VS16 = one cluster
        let s = "❤\u{FE0F}";
        let b = emoji_cluster_boundaries(s);
        assert_eq!(b, vec![0]);
    }

    #[test]
    fn zwj_family() {
        // man + ZWJ + woman + ZWJ + girl → one cluster
        let s = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
        let b = emoji_cluster_boundaries(s);
        assert_eq!(b, vec![0]);
    }

    #[test]
    fn flag_pair() {
        // US flag: two regional indicators → one cluster
        let s = "\u{1F1FA}\u{1F1F8}";
        let b = emoji_cluster_boundaries(s);
        assert_eq!(b, vec![0]);
    }

    #[test]
    fn two_emoji_no_zwj() {
        let s = "\u{1F600}\u{1F601}";
        let b = emoji_cluster_boundaries(s);
        assert_eq!(b.len(), 2);
    }
}
