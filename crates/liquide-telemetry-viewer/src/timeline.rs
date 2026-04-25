//! Event timeline for debugging, with Chrome Trace Format export.
//!
//! Records timestamped events (instant marks and duration spans) into a
//! ring buffer and can export them in the Chrome `chrome://tracing` JSON
//! format for visualization in Chrome DevTools or Perfetto.

use std::collections::VecDeque;
use std::time::Instant;

/// Category of a timeline event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimelineCategory {
    Input,
    Layout,
    Paint,
    Composite,
    Network,
    IO,
    Animation,
    Script,
}

impl TimelineCategory {
    /// Short label for display and export.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Input => "Input",
            Self::Layout => "Layout",
            Self::Paint => "Paint",
            Self::Composite => "Composite",
            Self::Network => "Network",
            Self::IO => "IO",
            Self::Animation => "Animation",
            Self::Script => "Script",
        }
    }

    /// Color hint for Chrome Trace viewer (cname).
    pub fn trace_color(&self) -> &'static str {
        match self {
            Self::Input => "olive",
            Self::Layout => "rail_response",
            Self::Paint => "rail_animation",
            Self::Composite => "cq_build_passed",
            Self::Network => "thread_state_runnable",
            Self::IO => "thread_state_iowait",
            Self::Animation => "rail_idle",
            Self::Script => "generic_work",
        }
    }
}

/// A single event in the timeline.
#[derive(Debug, Clone)]
pub struct TimelineEvent {
    /// Microsecond timestamp relative to timeline creation.
    pub timestamp_us: u64,
    /// Event category.
    pub category: TimelineCategory,
    /// Human-readable event name.
    pub name: String,
    /// Duration in microseconds (0 for instant / mark events).
    pub duration_us: u64,
    /// Originating thread identifier.
    pub thread_id: u64,
    /// Optional key-value metadata for additional context.
    pub metadata: Vec<(String, String)>,
}

/// Default timeline ring buffer capacity.
const DEFAULT_TIMELINE_CAPACITY: usize = 4096;

/// Ring buffer of [`TimelineEvent`]s for debugging and export.
pub struct Timeline {
    events: VecDeque<TimelineEvent>,
    capacity: usize,
    /// Reference point for computing relative timestamps.
    epoch: Instant,
    /// Default thread id to use when none is specified.
    default_thread_id: u64,
}

impl Timeline {
    /// Create a new timeline with the default capacity (4096 events).
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_TIMELINE_CAPACITY)
    }

    /// Create a timeline with a specific capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            events: VecDeque::with_capacity(capacity),
            capacity,
            epoch: Instant::now(),
            default_thread_id: 0,
        }
    }

    /// Set the default thread id for events.
    pub fn set_thread_id(&mut self, thread_id: u64) {
        self.default_thread_id = thread_id;
    }

    /// Record an instant mark event (zero duration).
    pub fn mark(&mut self, name: &str, category: TimelineCategory) {
        self.push_event(TimelineEvent {
            timestamp_us: self.elapsed_us(),
            category,
            name: name.to_string(),
            duration_us: 0,
            thread_id: self.default_thread_id,
            metadata: Vec::new(),
        });
    }

    /// Record an instant mark with metadata.
    pub fn mark_with_meta(
        &mut self,
        name: &str,
        category: TimelineCategory,
        metadata: Vec<(String, String)>,
    ) {
        self.push_event(TimelineEvent {
            timestamp_us: self.elapsed_us(),
            category,
            name: name.to_string(),
            duration_us: 0,
            thread_id: self.default_thread_id,
            metadata,
        });
    }

    /// Measure the duration of a closure and record it as a timeline event.
    pub fn measure<F, R>(&mut self, name: &str, category: TimelineCategory, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let start = Instant::now();
        let start_us = self.elapsed_us();
        let result = f();
        let duration_us = start.elapsed().as_micros() as u64;
        self.push_event(TimelineEvent {
            timestamp_us: start_us,
            category,
            name: name.to_string(),
            duration_us,
            thread_id: self.default_thread_id,
            metadata: Vec::new(),
        });
        result
    }

    /// Record a pre-computed duration event.
    pub fn record_duration(
        &mut self,
        name: &str,
        category: TimelineCategory,
        timestamp_us: u64,
        duration_us: u64,
    ) {
        self.push_event(TimelineEvent {
            timestamp_us,
            category,
            name: name.to_string(),
            duration_us,
            thread_id: self.default_thread_id,
            metadata: Vec::new(),
        });
    }

    /// Record a duration event with metadata and thread id.
    pub fn record_full(
        &mut self,
        name: &str,
        category: TimelineCategory,
        timestamp_us: u64,
        duration_us: u64,
        thread_id: u64,
        metadata: Vec<(String, String)>,
    ) {
        self.push_event(TimelineEvent {
            timestamp_us,
            category,
            name: name.to_string(),
            duration_us,
            thread_id,
            metadata,
        });
    }

    /// Number of events currently stored.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Whether the timeline is empty.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Clear all events.
    pub fn clear(&mut self) {
        self.events.clear();
    }

    /// Iterate over events (oldest first).
    pub fn iter(&self) -> impl Iterator<Item = &TimelineEvent> {
        self.events.iter()
    }

    /// Filter events by category.
    pub fn events_by_category(&self, category: TimelineCategory) -> Vec<&TimelineEvent> {
        self.events
            .iter()
            .filter(|e| e.category == category)
            .collect()
    }

    /// Export all events in Chrome Trace Format (JSON).
    ///
    /// The output is compatible with `chrome://tracing` and Perfetto.
    /// Format reference: <https://docs.google.com/document/d/1CvAClvFfyA5R-PhYUmn5OOQtYMH4h6I0nSsKchNAySU/preview>
    pub fn to_chrome_trace_json(&self) -> String {
        let mut entries = Vec::with_capacity(self.events.len());

        for event in &self.events {
            let args = if event.metadata.is_empty() {
                "{}".to_string()
            } else {
                let pairs: Vec<String> = event
                    .metadata
                    .iter()
                    .map(|(k, v)| format!("\"{}\":\"{}\"", escape_json(k), escape_json(v)))
                    .collect();
                format!("{{{}}}", pairs.join(","))
            };

            if event.duration_us == 0 {
                // Instant event (mark)
                entries.push(format!(
                    "{{\"name\":\"{}\",\"cat\":\"{}\",\"ph\":\"i\",\"ts\":{},\"pid\":1,\"tid\":{},\"s\":\"g\",\"args\":{},\"cname\":\"{}\"}}",
                    escape_json(&event.name),
                    event.category.label(),
                    event.timestamp_us,
                    event.thread_id,
                    args,
                    event.category.trace_color(),
                ));
            } else {
                // Duration event (complete / "X")
                entries.push(format!(
                    "{{\"name\":\"{}\",\"cat\":\"{}\",\"ph\":\"X\",\"ts\":{},\"dur\":{},\"pid\":1,\"tid\":{},\"args\":{},\"cname\":\"{}\"}}",
                    escape_json(&event.name),
                    event.category.label(),
                    event.timestamp_us,
                    event.duration_us,
                    event.thread_id,
                    args,
                    event.category.trace_color(),
                ));
            }
        }

        format!("[{}]", entries.join(","))
    }

    // --- internal helpers ---

    fn push_event(&mut self, event: TimelineEvent) {
        if self.events.len() >= self.capacity {
            self.events.pop_front();
        }
        self.events.push_back(event);
    }

    fn elapsed_us(&self) -> u64 {
        self.epoch.elapsed().as_micros() as u64
    }
}

impl Default for Timeline {
    fn default() -> Self {
        Self::new()
    }
}

/// Escape special characters for JSON string values.
fn escape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_timeline_is_empty() {
        let tl = Timeline::new();
        assert!(tl.is_empty());
        assert_eq!(tl.len(), 0);
    }

    #[test]
    fn mark_adds_event() {
        let mut tl = Timeline::new();
        tl.mark("frame_start", TimelineCategory::Layout);
        assert_eq!(tl.len(), 1);
        let event = tl.iter().next().unwrap();
        assert_eq!(event.name, "frame_start");
        assert_eq!(event.category, TimelineCategory::Layout);
        assert_eq!(event.duration_us, 0);
    }

    #[test]
    fn mark_with_metadata() {
        let mut tl = Timeline::new();
        tl.mark_with_meta(
            "click",
            TimelineCategory::Input,
            vec![("x".into(), "100".into()), ("y".into(), "200".into())],
        );
        let event = tl.iter().next().unwrap();
        assert_eq!(event.metadata.len(), 2);
        assert_eq!(event.metadata[0], ("x".into(), "100".into()));
    }

    #[test]
    fn measure_records_duration() {
        let mut tl = Timeline::new();
        let result = tl.measure("work", TimelineCategory::Paint, || {
            let mut sum = 0u64;
            for i in 0..1000 {
                sum += i;
            }
            sum
        });
        assert_eq!(result, 499_500);
        assert_eq!(tl.len(), 1);
        let event = tl.iter().next().unwrap();
        assert_eq!(event.name, "work");
        assert_eq!(event.category, TimelineCategory::Paint);
        // Duration should be > 0 (may be very small)
    }

    #[test]
    fn record_duration_direct() {
        let mut tl = Timeline::new();
        tl.record_duration("rasterize", TimelineCategory::Composite, 1000, 5000);
        let event = tl.iter().next().unwrap();
        assert_eq!(event.timestamp_us, 1000);
        assert_eq!(event.duration_us, 5000);
        assert_eq!(event.name, "rasterize");
    }

    #[test]
    fn record_full() {
        let mut tl = Timeline::new();
        tl.record_full(
            "fetch",
            TimelineCategory::Network,
            500,
            2000,
            42,
            vec![("url".into(), "http://example.com".into())],
        );
        let event = tl.iter().next().unwrap();
        assert_eq!(event.thread_id, 42);
        assert_eq!(event.metadata.len(), 1);
    }

    #[test]
    fn ring_buffer_eviction() {
        let mut tl = Timeline::with_capacity(3);
        tl.mark("a", TimelineCategory::Layout);
        tl.mark("b", TimelineCategory::Layout);
        tl.mark("c", TimelineCategory::Layout);
        tl.mark("d", TimelineCategory::Layout);
        assert_eq!(tl.len(), 3);
        let names: Vec<&str> = tl.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["b", "c", "d"]);
    }

    #[test]
    fn clear_empties_timeline() {
        let mut tl = Timeline::new();
        tl.mark("x", TimelineCategory::IO);
        tl.mark("y", TimelineCategory::IO);
        tl.clear();
        assert!(tl.is_empty());
    }

    #[test]
    fn events_by_category() {
        let mut tl = Timeline::new();
        tl.mark("layout1", TimelineCategory::Layout);
        tl.mark("paint1", TimelineCategory::Paint);
        tl.mark("layout2", TimelineCategory::Layout);
        tl.mark("input1", TimelineCategory::Input);

        let layout_events = tl.events_by_category(TimelineCategory::Layout);
        assert_eq!(layout_events.len(), 2);
        assert_eq!(layout_events[0].name, "layout1");
        assert_eq!(layout_events[1].name, "layout2");

        let input_events = tl.events_by_category(TimelineCategory::Input);
        assert_eq!(input_events.len(), 1);

        let network_events = tl.events_by_category(TimelineCategory::Network);
        assert!(network_events.is_empty());
    }

    #[test]
    fn chrome_trace_empty() {
        let tl = Timeline::new();
        assert_eq!(tl.to_chrome_trace_json(), "[]");
    }

    #[test]
    fn chrome_trace_instant_event() {
        let mut tl = Timeline::new();
        tl.record_full("click", TimelineCategory::Input, 100, 0, 1, Vec::new());
        let json = tl.to_chrome_trace_json();
        assert!(json.contains("\"ph\":\"i\""));
        assert!(json.contains("\"name\":\"click\""));
        assert!(json.contains("\"cat\":\"Input\""));
        assert!(json.contains("\"ts\":100"));
    }

    #[test]
    fn chrome_trace_duration_event() {
        let mut tl = Timeline::new();
        tl.record_full("layout", TimelineCategory::Layout, 200, 5000, 1, Vec::new());
        let json = tl.to_chrome_trace_json();
        assert!(json.contains("\"ph\":\"X\""));
        assert!(json.contains("\"dur\":5000"));
    }

    #[test]
    fn chrome_trace_with_metadata() {
        let mut tl = Timeline::new();
        tl.record_full(
            "fetch",
            TimelineCategory::Network,
            0,
            100,
            1,
            vec![("url".into(), "http://test.com".into())],
        );
        let json = tl.to_chrome_trace_json();
        assert!(json.contains("\"url\":\"http://test.com\""));
    }

    #[test]
    fn chrome_trace_escapes_special_chars() {
        let mut tl = Timeline::new();
        tl.record_full(
            "test \"event\"",
            TimelineCategory::Script,
            0,
            100,
            1,
            vec![("data".into(), "line1\nline2".into())],
        );
        let json = tl.to_chrome_trace_json();
        assert!(json.contains("test \\\"event\\\""));
        assert!(json.contains("line1\\nline2"));
    }

    #[test]
    fn set_thread_id() {
        let mut tl = Timeline::new();
        tl.set_thread_id(7);
        tl.mark("evt", TimelineCategory::Animation);
        assert_eq!(tl.iter().next().unwrap().thread_id, 7);
    }

    #[test]
    fn category_labels() {
        assert_eq!(TimelineCategory::Input.label(), "Input");
        assert_eq!(TimelineCategory::Layout.label(), "Layout");
        assert_eq!(TimelineCategory::Paint.label(), "Paint");
        assert_eq!(TimelineCategory::Composite.label(), "Composite");
        assert_eq!(TimelineCategory::Network.label(), "Network");
        assert_eq!(TimelineCategory::IO.label(), "IO");
        assert_eq!(TimelineCategory::Animation.label(), "Animation");
        assert_eq!(TimelineCategory::Script.label(), "Script");
    }

    #[test]
    fn category_trace_colors() {
        // Ensure all categories have a non-empty trace color
        let cats = [
            TimelineCategory::Input,
            TimelineCategory::Layout,
            TimelineCategory::Paint,
            TimelineCategory::Composite,
            TimelineCategory::Network,
            TimelineCategory::IO,
            TimelineCategory::Animation,
            TimelineCategory::Script,
        ];
        for cat in &cats {
            assert!(!cat.trace_color().is_empty());
        }
    }

    #[test]
    fn default_timeline() {
        let tl = Timeline::default();
        assert!(tl.is_empty());
    }

    #[test]
    fn escape_json_basic() {
        assert_eq!(escape_json("hello"), "hello");
        assert_eq!(escape_json("a\"b"), "a\\\"b");
        assert_eq!(escape_json("a\\b"), "a\\\\b");
        assert_eq!(escape_json("a\nb"), "a\\nb");
        assert_eq!(escape_json("a\rb"), "a\\rb");
        assert_eq!(escape_json("a\tb"), "a\\tb");
    }

    #[test]
    fn multiple_events_in_trace() {
        let mut tl = Timeline::new();
        tl.record_duration("a", TimelineCategory::Layout, 0, 100);
        tl.record_duration("b", TimelineCategory::Paint, 100, 200);
        let json = tl.to_chrome_trace_json();
        // Should have two entries separated by comma
        assert!(json.starts_with('['));
        assert!(json.ends_with(']'));
        let count = json.matches("\"name\"").count();
        assert_eq!(count, 2);
    }
}
