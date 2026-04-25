use crate::layout::WindowInfo;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FoldedSegment {
    folded_start: usize,
    folded_end: usize,
    original_start: usize,
    original_end: usize,
}

/// Type-ahead search filter for the overview.
pub struct OverviewSearch {
    query: String,
}

impl OverviewSearch {
    pub fn new() -> Self {
        Self {
            query: String::new(),
        }
    }

    /// Append a character to the search query.
    pub fn push_char(&mut self, c: char) {
        self.query.push(c);
    }

    /// Remove the last character from the search query.
    pub fn pop_char(&mut self) {
        self.query.pop();
    }

    /// Clear the search query entirely.
    pub fn clear(&mut self) {
        self.query.clear();
    }

    /// The current query string.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Whether the search is active (non-empty query).
    pub fn is_active(&self) -> bool {
        !self.query.is_empty()
    }

    /// Filter windows whose title contains the query as a case-insensitive
    /// substring. An empty query matches all windows.
    pub fn filter_windows<'a>(&self, windows: &'a [WindowInfo]) -> Vec<&'a WindowInfo> {
        if self.query.is_empty() {
            return windows.iter().collect();
        }
        windows
            .iter()
            .filter(|w| !match_ranges(&w.title, &self.query).is_empty())
            .collect()
    }

    /// Compute byte-offset ranges in `title` that match the query, for
    /// highlighting. Returns non-overlapping `(start, end)` ranges (byte
    /// offsets, suitable for slicing).
    pub fn highlight_ranges(&self, title: &str) -> Vec<(usize, usize)> {
        match_ranges(title, &self.query)
    }
}

fn match_ranges(title: &str, query: &str) -> Vec<(usize, usize)> {
    if query.is_empty() {
        return Vec::new();
    }

    let (folded_title, segments) = build_folded_segments(title);
    let folded_query = fold_case(query);
    if folded_query.is_empty() {
        return Vec::new();
    }

    let mut ranges = Vec::new();
    let mut search_start = 0usize;

    while let Some(pos) = folded_title[search_start..].find(&folded_query) {
        let match_start = search_start + pos;
        let match_end = match_start + folded_query.len();

        if let Some(range) = map_folded_range(&segments, match_start, match_end) {
            if ranges.last().copied() != Some(range) {
                ranges.push(range);
            }
        }

        search_start = match_end;
    }

    ranges
}

fn fold_case(text: &str) -> String {
    text.chars().flat_map(|ch| ch.to_lowercase()).collect()
}

fn build_folded_segments(title: &str) -> (String, Vec<FoldedSegment>) {
    let mut folded = String::new();
    let mut segments = Vec::new();

    let mut chars = title.char_indices().peekable();
    let mut cluster_start: Option<usize> = None;
    let mut cluster_end = 0usize;
    let mut cluster_folded = String::new();
    let mut prev_was_zwj = false;

    while let Some((idx, ch)) = chars.next() {
        let next_idx = chars.peek().map(|(next, _)| *next).unwrap_or(title.len());
        let continues_cluster = cluster_start.is_some() && (prev_was_zwj || is_cluster_tail(ch));

        if !continues_cluster {
            push_segment(
                &mut folded,
                &mut segments,
                cluster_start,
                cluster_end,
                &cluster_folded,
            );
            cluster_start = Some(idx);
            cluster_folded.clear();
        }

        cluster_end = next_idx;
        cluster_folded.extend(ch.to_lowercase());
        prev_was_zwj = ch == '\u{200D}';
    }

    push_segment(
        &mut folded,
        &mut segments,
        cluster_start,
        cluster_end,
        &cluster_folded,
    );

    (folded, segments)
}

fn push_segment(
    folded: &mut String,
    segments: &mut Vec<FoldedSegment>,
    cluster_start: Option<usize>,
    cluster_end: usize,
    cluster_folded: &str,
) {
    let Some(original_start) = cluster_start else {
        return;
    };

    let folded_start = folded.len();
    folded.push_str(cluster_folded);
    let folded_end = folded.len();
    segments.push(FoldedSegment {
        folded_start,
        folded_end,
        original_start,
        original_end: cluster_end,
    });
}

fn map_folded_range(
    segments: &[FoldedSegment],
    folded_start: usize,
    folded_end: usize,
) -> Option<(usize, usize)> {
    let first = segments.iter().find(|segment| segment.folded_end > folded_start)?;
    let last = segments
        .iter()
        .rfind(|segment| segment.folded_start < folded_end)?;
    Some((first.original_start, last.original_end))
}

fn is_cluster_tail(ch: char) -> bool {
    matches!(ch, '\u{200D}' | '\u{FE0E}' | '\u{FE0F}')
        || matches!(ch as u32, 0x1F3FB..=0x1F3FF)
        || matches!(ch as u32, 0x0300..=0x036F)
        || matches!(ch as u32, 0x1AB0..=0x1AFF)
        || matches!(ch as u32, 0x1DC0..=0x1DFF)
        || matches!(ch as u32, 0x20D0..=0x20FF)
        || matches!(ch as u32, 0xFE20..=0xFE2F)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::OverviewRect;

    fn make_windows() -> Vec<WindowInfo> {
        vec![
            WindowInfo {
                id: 1,
                title: "Firefox".into(),
                original: OverviewRect::new(0.0, 0.0, 800.0, 600.0),
                workspace: 0,
                monitor: 0,
            },
            WindowInfo {
                id: 2,
                title: "Terminal".into(),
                original: OverviewRect::new(0.0, 0.0, 800.0, 600.0),
                workspace: 0,
                monitor: 0,
            },
            WindowInfo {
                id: 3,
                title: "Files".into(),
                original: OverviewRect::new(0.0, 0.0, 800.0, 600.0),
                workspace: 0,
                monitor: 0,
            },
            WindowInfo {
                id: 4,
                title: "Text Editor".into(),
                original: OverviewRect::new(0.0, 0.0, 800.0, 600.0),
                workspace: 0,
                monitor: 0,
            },
        ]
    }

    #[test]
    fn empty_query_matches_all() {
        let search = OverviewSearch::new();
        let wins = make_windows();
        let filtered = search.filter_windows(&wins);
        assert_eq!(filtered.len(), 4);
    }

    #[test]
    fn single_char_filter() {
        let mut search = OverviewSearch::new();
        search.push_char('f');
        let wins = make_windows();
        let filtered = search.filter_windows(&wins);
        // "Firefox" and "Files" contain 'f'.
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn multi_char_filter() {
        let mut search = OverviewSearch::new();
        search.push_char('t');
        search.push_char('e');
        search.push_char('r');
        let wins = make_windows();
        let filtered = search.filter_windows(&wins);
        // "Terminal" and "Text Editor" contain "ter" (case-insensitive).
        assert_eq!(filtered.len(), 1); // only "Terminal" has "ter"
    }

    #[test]
    fn case_insensitive() {
        let mut search = OverviewSearch::new();
        search.push_char('F');
        search.push_char('I');
        let wins = make_windows();
        let filtered = search.filter_windows(&wins);
        // "Firefox" and "Files" contain "fi" (case-insensitive).
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn no_matches() {
        let mut search = OverviewSearch::new();
        search.push_char('z');
        search.push_char('z');
        search.push_char('z');
        let wins = make_windows();
        let filtered = search.filter_windows(&wins);
        assert!(filtered.is_empty());
    }

    #[test]
    fn pop_char_removes_last() {
        let mut search = OverviewSearch::new();
        search.push_char('a');
        search.push_char('b');
        search.pop_char();
        assert_eq!(search.query(), "a");
    }

    #[test]
    fn pop_char_on_empty_is_safe() {
        let mut search = OverviewSearch::new();
        search.pop_char();
        assert_eq!(search.query(), "");
    }

    #[test]
    fn clear_resets_query() {
        let mut search = OverviewSearch::new();
        search.push_char('x');
        search.push_char('y');
        search.clear();
        assert_eq!(search.query(), "");
    }

    #[test]
    fn highlight_ranges_empty_query() {
        let search = OverviewSearch::new();
        assert!(search.highlight_ranges("Firefox").is_empty());
    }

    #[test]
    fn highlight_ranges_single_match() {
        let mut search = OverviewSearch::new();
        search.push_char('f');
        search.push_char('i');
        let ranges = search.highlight_ranges("Firefox");
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0], (0, 2));
    }

    #[test]
    fn highlight_ranges_multiple_matches() {
        let mut search = OverviewSearch::new();
        search.push_char('a');
        let ranges = search.highlight_ranges("abracadabra");
        // "a" appears at indices 0, 3, 5, 7, 10.
        assert_eq!(ranges.len(), 5);
    }

    #[test]
    fn is_active_tracks_query() {
        let mut search = OverviewSearch::new();
        assert!(!search.is_active());
        search.push_char('x');
        assert!(search.is_active());
        search.clear();
        assert!(!search.is_active());
    }

    #[test]
    fn highlight_ranges_non_ascii_use_original_boundaries() {
        let mut search = OverviewSearch::new();
        search.push_char('i');

        let ranges = search.highlight_ranges("İstanbul");

        assert_eq!(ranges, vec![(0, "İ".len())]);
    }
}
