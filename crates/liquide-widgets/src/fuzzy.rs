//! Tiny, dependency-free fuzzy subsequence matcher + scorer.
//!
//! Used by the command palette to filter + rank commands by a typed query. The
//! contract is intentionally small and self-contained (no external fuzzy crate):
//!
//! - [`matches`] — case-insensitive **subsequence** test: every character of the
//!   query appears in the haystack, in order (not necessarily contiguously).
//! - [`score`] — `Some(score)` when it matches, else `None`. A higher score is a
//!   better match. The score rewards:
//!     * **contiguity** — adjacent matched characters score extra (so "doc"
//!       prefers "**doc**ument" over "**d**ark m**o**de **c**lose"),
//!     * **word-boundary / prefix** hits — a query char that lands at the start of
//!       the haystack or right after a separator (space/`-`/`_`/`/`/`.`) or on a
//!       camelCase boundary scores extra (so "om" prefers "**O**pen **M**ap" over
//!       "rand**om**"),
//!     * an empty query trivially matches with score 0 (used for the "show all"
//!       state — callers usually special-case the empty query upstream anyway).
//!
//! Matching is greedy-leftmost, which is cheap and good enough for a palette of
//! human-readable command titles; it is deterministic so tests are stable.

/// Bonus for a matched character immediately following a previous match
/// (contiguous run).
const CONTIGUOUS_BONUS: i32 = 8;
/// Bonus for a matched character that starts a word (haystack start, after a
/// separator, or a camelCase upper-after-lower boundary).
const BOUNDARY_BONUS: i32 = 12;
/// Base score for any matched character.
const MATCH_BASE: i32 = 2;

/// Whether `query` is a case-insensitive subsequence of `haystack`.
///
/// An empty query always matches. Greedy-leftmost: each query char consumes the
/// next equal haystack char.
pub fn matches(query: &str, haystack: &str) -> bool {
    score(query, haystack).is_some()
}

/// Score `haystack` against `query`, or `None` if it is not a subsequence.
///
/// See the module docs for the scoring model. Deterministic + greedy-leftmost.
pub fn score(query: &str, haystack: &str) -> Option<i32> {
    let q: Vec<char> = query.chars().flat_map(char::to_lowercase).collect();
    if q.is_empty() {
        return Some(0);
    }
    let hay: Vec<char> = haystack.chars().collect();
    let hay_lower: Vec<char> = haystack.chars().flat_map(char::to_lowercase).collect();
    // `to_lowercase` can change length (rare for command titles); fall back to a
    // 1:1 lower map when lengths diverge so index math stays sound.
    let lower: Vec<char> = if hay_lower.len() == hay.len() {
        hay_lower
    } else {
        hay.iter()
            .map(|c| c.to_lowercase().next().unwrap_or(*c))
            .collect()
    };

    let mut total = 0i32;
    let mut qi = 0usize;
    let mut prev_match: Option<usize> = None;

    for (hi, &lc) in lower.iter().enumerate() {
        if qi >= q.len() {
            break;
        }
        if lc == q[qi] {
            let mut s = MATCH_BASE;
            if prev_match == Some(hi.wrapping_sub(1)) && hi > 0 {
                s += CONTIGUOUS_BONUS;
            }
            if is_boundary(&hay, hi) {
                s += BOUNDARY_BONUS;
            }
            total += s;
            prev_match = Some(hi);
            qi += 1;
        }
    }

    if qi == q.len() {
        Some(total)
    } else {
        None
    }
}

/// Whether the haystack character at `i` starts a "word" for scoring purposes:
/// index 0, the char after a separator, or a camelCase upper-after-lower.
fn is_boundary(hay: &[char], i: usize) -> bool {
    if i == 0 {
        return true;
    }
    let prev = hay[i - 1];
    if matches!(prev, ' ' | '-' | '_' | '/' | '.' | ':') {
        return true;
    }
    let cur = hay[i];
    cur.is_uppercase() && prev.is_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_query_matches_everything_with_zero_score() {
        assert_eq!(score("", "anything"), Some(0));
        assert!(matches("", "anything"));
    }

    #[test]
    fn subsequence_matches_in_order_only() {
        assert!(matches("doc", "Open Document"));
        assert!(matches("opm", "Open Map"));
        // Out of order does NOT match.
        assert!(!matches("cod", "Open Document"));
        // A char not present at all fails.
        assert!(!matches("xyz", "Open Document"));
    }

    #[test]
    fn contiguity_beats_scattered() {
        // "doc" contiguous in "Document" should outrank the scattered hit in
        // "Dark mode Off c…".
        let contiguous = score("doc", "Document").unwrap();
        let scattered = score("doc", "Dark mode close").unwrap();
        assert!(
            contiguous > scattered,
            "contiguous {contiguous} should beat scattered {scattered}"
        );
    }

    #[test]
    fn word_boundary_beats_midword() {
        // "om" as the start of two words ("Open Map") should beat "om" buried in
        // "random".
        let boundary = score("om", "Open Map").unwrap();
        let midword = score("om", "random").unwrap();
        assert!(
            boundary > midword,
            "boundary {boundary} should beat midword {midword}"
        );
    }

    #[test]
    fn case_insensitive() {
        assert!(matches("OPEN", "open file"));
        assert!(matches("open", "OPEN FILE"));
    }
}
