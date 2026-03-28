use crate::layout::WindowInfo;

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
        let lower_query = self.query.to_lowercase();
        windows
            .iter()
            .filter(|w| w.title.to_lowercase().contains(&lower_query))
            .collect()
    }

    /// Compute byte-offset ranges in `title` that match the query, for
    /// highlighting. Returns non-overlapping `(start, end)` ranges (byte
    /// offsets, suitable for slicing).
    pub fn highlight_ranges(&self, title: &str) -> Vec<(usize, usize)> {
        if self.query.is_empty() {
            return Vec::new();
        }
        let lower_title = title.to_lowercase();
        let lower_query = self.query.to_lowercase();
        let qlen = lower_query.len();
        let mut ranges = Vec::new();
        let mut start = 0usize;
        while let Some(pos) = lower_title[start..].find(&lower_query) {
            let abs = start + pos;
            ranges.push((abs, abs + qlen));
            start = abs + qlen;
        }
        ranges
    }
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
}
