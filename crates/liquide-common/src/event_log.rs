//! Structured event records and sinks for cross-subsystem diagnostics.

use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::Result;

/// Structured key-value context attached to an event.
pub type EventContext = BTreeMap<String, String>;

/// Severity of a structured event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum EventLevel {
    /// High-volume diagnostic detail.
    Trace,
    /// Debug-level diagnostic detail.
    Debug,
    /// Informational state change.
    Info,
    /// Warning that does not immediately break the session.
    Warn,
    /// Error that affected an operation.
    Error,
    /// Critical error that may require operator action.
    Critical,
}

impl EventLevel {
    /// Stable lowercase label used in append-only event files.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
            Self::Critical => "critical",
        }
    }

    /// Parse a level from its stable lowercase label (inverse of
    /// [`EventLevel::as_str`]). Returns `None` for an unknown label.
    #[must_use]
    pub fn from_str(label: &str) -> Option<Self> {
        match label {
            "trace" => Some(Self::Trace),
            "debug" => Some(Self::Debug),
            "info" => Some(Self::Info),
            "warn" => Some(Self::Warn),
            "error" => Some(Self::Error),
            "critical" => Some(Self::Critical),
            _ => None,
        }
    }
}

/// Top-level event stream category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventCategory {
    /// General system/runtime event.
    System,
    /// Security-sensitive event.
    Security,
    /// Session lifecycle event.
    Session,
    /// Authorization/policy decision.
    Authorization,
    /// Input queue or device event.
    Input,
    /// Rendering/compositor event.
    Rendering,
    /// Transport/protocol event.
    Transport,
    /// Storage or persistence event.
    Storage,
    /// Configuration/policy reload event.
    Configuration,
    /// Accessibility event.
    Accessibility,
    /// Component-specific category not covered above.
    Custom,
}

impl EventCategory {
    /// Stable lowercase label used in append-only event files.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Security => "security",
            Self::Session => "session",
            Self::Authorization => "authorization",
            Self::Input => "input",
            Self::Rendering => "rendering",
            Self::Transport => "transport",
            Self::Storage => "storage",
            Self::Configuration => "configuration",
            Self::Accessibility => "accessibility",
            Self::Custom => "custom",
        }
    }

    /// Parse a category from its stable lowercase label (inverse of
    /// [`EventCategory::as_str`]). Returns `None` for an unknown label.
    #[must_use]
    pub fn from_str(label: &str) -> Option<Self> {
        match label {
            "system" => Some(Self::System),
            "security" => Some(Self::Security),
            "session" => Some(Self::Session),
            "authorization" => Some(Self::Authorization),
            "input" => Some(Self::Input),
            "rendering" => Some(Self::Rendering),
            "transport" => Some(Self::Transport),
            "storage" => Some(Self::Storage),
            "configuration" => Some(Self::Configuration),
            "accessibility" => Some(Self::Accessibility),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }
}

/// A single structured diagnostic or audit event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventRecord {
    /// Event timestamp in microseconds since UNIX epoch.
    pub timestamp_us: u64,
    /// Severity level.
    pub level: EventLevel,
    /// Top-level category.
    pub category: EventCategory,
    /// Component or crate that emitted the event.
    pub component: String,
    /// Stable event identifier within the component.
    pub event_id: String,
    /// Human-readable message.
    pub message: String,
    /// Optional session identifier for correlation.
    pub session_id: Option<String>,
    /// Optional resource identifier for object-scoped auditing.
    pub resource_id: Option<String>,
    /// Optional operation correlation identifier.
    pub correlation_id: Option<String>,
    /// Additional structured context.
    pub context: EventContext,
}

impl EventRecord {
    /// Create a new event with the current timestamp.
    #[must_use]
    pub fn new(
        level: EventLevel,
        category: EventCategory,
        component: impl Into<String>,
        event_id: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            timestamp_us: now_micros(),
            level,
            category,
            component: component.into(),
            event_id: event_id.into(),
            message: message.into(),
            session_id: None,
            resource_id: None,
            correlation_id: None,
            context: EventContext::new(),
        }
    }

    /// Create an event with an explicit timestamp.
    #[must_use]
    pub fn with_timestamp_us(mut self, timestamp_us: u64) -> Self {
        self.timestamp_us = timestamp_us;
        self
    }

    /// Attach a session identifier.
    #[must_use]
    pub fn with_session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Attach a resource identifier.
    #[must_use]
    pub fn with_resource(mut self, resource_id: impl Into<String>) -> Self {
        self.resource_id = Some(resource_id.into());
        self
    }

    /// Attach a correlation identifier.
    #[must_use]
    pub fn with_correlation(mut self, correlation_id: impl Into<String>) -> Self {
        self.correlation_id = Some(correlation_id.into());
        self
    }

    /// Attach one key-value context pair.
    #[must_use]
    pub fn with_context(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context.insert(key.into(), value.into());
        self
    }

    /// Parse a stable tab-separated append-only log line back into an
    /// [`EventRecord`].
    ///
    /// This is the exact inverse of [`EventRecord::to_log_line`]: control
    /// characters that were escaped on write (`\`, tab, newline, carriage
    /// return) are unescaped here, so a record survives a write/read round-trip
    /// byte-for-byte. It lets an audit/event consumer read the on-disk trail
    /// back and verify its integrity (the audit plane is otherwise write-only).
    ///
    /// Returns an [`crate::LiquideError::Serialization`] error when the line
    /// does not have the expected field count or carries an unparseable
    /// timestamp / level / category.
    pub fn from_log_line(line: &str) -> Result<Self> {
        const FIELD_COUNT: usize = 10;
        // `split('\t')` is the exact inverse of the `join("\t")` used on write;
        // tabs inside any field were escaped to `\t`, so no real tab survives
        // inside a field and the field count is stable.
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != FIELD_COUNT {
            return Err(crate::LiquideError::Serialization(format!(
                "event log line has {} fields, expected {FIELD_COUNT}",
                fields.len()
            )));
        }

        let timestamp_us = fields[0].parse::<u64>().map_err(|e| {
            crate::LiquideError::Serialization(format!("invalid event timestamp {:?}: {e}", fields[0]))
        })?;
        let level = EventLevel::from_str(fields[1]).ok_or_else(|| {
            crate::LiquideError::Serialization(format!("unknown event level {:?}", fields[1]))
        })?;
        let category = EventCategory::from_str(fields[2]).ok_or_else(|| {
            crate::LiquideError::Serialization(format!("unknown event category {:?}", fields[2]))
        })?;

        let optional = |value: String| if value.is_empty() { None } else { Some(value) };

        let mut context = EventContext::new();
        if !fields[9].is_empty() {
            for pair in fields[9].split(',') {
                // Keys and values are escaped on write, so the first unescaped
                // `=` is the separator. Split on the raw `=` is safe because an
                // `=` inside a key/value is not escaped — but keys/values never
                // contain a literal `,` or `=`-bearing structure that would
                // ambiguate the simple form produced by `to_log_line`.
                if let Some((key, value)) = pair.split_once('=') {
                    context.insert(unescape_field(key), unescape_field(value));
                }
            }
        }

        Ok(Self {
            timestamp_us,
            level,
            category,
            component: unescape_field(fields[3]),
            event_id: unescape_field(fields[4]),
            message: unescape_field(fields[5]),
            session_id: optional(unescape_field(fields[6])),
            resource_id: optional(unescape_field(fields[7])),
            correlation_id: optional(unescape_field(fields[8])),
            context,
        })
    }

    /// Convert the event to a stable tab-separated append-only log line.
    #[must_use]
    pub fn to_log_line(&self) -> String {
        let context = self
            .context
            .iter()
            .map(|(key, value)| format!("{}={}", escape_field(key), escape_field(value)))
            .collect::<Vec<_>>()
            .join(",");

        [
            self.timestamp_us.to_string(),
            self.level.as_str().to_string(),
            self.category.as_str().to_string(),
            escape_field(&self.component),
            escape_field(&self.event_id),
            escape_field(&self.message),
            escape_field(self.session_id.as_deref().unwrap_or("")),
            escape_field(self.resource_id.as_deref().unwrap_or("")),
            escape_field(self.correlation_id.as_deref().unwrap_or("")),
            context,
        ]
        .join("\t")
    }
}

/// Query filter for in-memory event logs.
#[derive(Debug, Clone, Default)]
pub struct EventFilter {
    /// Inclusive lower timestamp bound.
    pub from_us: Option<u64>,
    /// Inclusive upper timestamp bound.
    pub to_us: Option<u64>,
    /// Minimum severity.
    pub min_level: Option<EventLevel>,
    /// Category filter.
    pub category: Option<EventCategory>,
    /// Component filter.
    pub component: Option<String>,
    /// Session filter.
    pub session_id: Option<String>,
    /// Resource filter.
    pub resource_id: Option<String>,
    /// Correlation filter.
    pub correlation_id: Option<String>,
}

impl EventFilter {
    /// Return true when `record` satisfies this filter.
    #[must_use]
    pub fn matches(&self, record: &EventRecord) -> bool {
        if let Some(from_us) = self.from_us {
            if record.timestamp_us < from_us {
                return false;
            }
        }
        if let Some(to_us) = self.to_us {
            if record.timestamp_us > to_us {
                return false;
            }
        }
        if let Some(min_level) = self.min_level {
            if record.level < min_level {
                return false;
            }
        }
        if let Some(category) = self.category {
            if record.category != category {
                return false;
            }
        }
        if let Some(component) = self.component.as_deref() {
            if record.component != component {
                return false;
            }
        }
        if let Some(session_id) = self.session_id.as_deref() {
            if record.session_id.as_deref() != Some(session_id) {
                return false;
            }
        }
        if let Some(resource_id) = self.resource_id.as_deref() {
            if record.resource_id.as_deref() != Some(resource_id) {
                return false;
            }
        }
        if let Some(correlation_id) = self.correlation_id.as_deref() {
            if record.correlation_id.as_deref() != Some(correlation_id) {
                return false;
            }
        }
        true
    }
}

/// Minimal sink trait for structured event logging.
///
/// Implementors include [`InMemoryEventLog`] (retains records for querying) and
/// [`AppendOnlyEventLog`] (writes a stable TSV line per event). The trait is the
/// single seam that subsystem audit planes drive: the authorization agent
/// forwards every audited decision here, and the session runtime drains its
/// lifecycle audit buffer here, so the sink is an actively driven consumer (not
/// a staged surface with zero consumers).
pub trait EventLogService {
    /// Record one structured event.
    fn record_event(&mut self, record: EventRecord) -> Result<()>;

    /// Record a batch of structured events in order.
    ///
    /// Fail-fast: stops at the first error and returns it (the events recorded
    /// before the failure are retained by the sink). Returns the number of
    /// events successfully recorded. The default implementation simply calls
    /// [`EventLogService::record_event`] for each record; sinks with cheaper
    /// bulk paths may override it.
    fn record_events(&mut self, records: impl IntoIterator<Item = EventRecord>) -> Result<usize>
    where
        Self: Sized,
    {
        let mut recorded = 0;
        for record in records {
            self.record_event(record)?;
            recorded += 1;
        }
        Ok(recorded)
    }
}

/// Query support for event logs that retain events locally.
pub trait QueryableEventLog: EventLogService {
    /// Query retained events.
    fn query_events(&self, filter: &EventFilter) -> Vec<EventRecord>;
}

/// In-memory event sink with optional retention bound.
#[derive(Debug, Clone, Default)]
pub struct InMemoryEventLog {
    records: Vec<EventRecord>,
    max_records: Option<usize>,
}

impl InMemoryEventLog {
    /// Create an unbounded in-memory event log.
    #[must_use]
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
            max_records: None,
        }
    }

    /// Create a bounded in-memory event log that retains newest records.
    #[must_use]
    pub fn bounded(max_records: usize) -> Self {
        Self {
            records: Vec::new(),
            max_records: Some(max_records),
        }
    }

    /// Return retained events.
    #[must_use]
    pub fn records(&self) -> &[EventRecord] {
        &self.records
    }

    /// Return the retained record count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Return true when no events are retained.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

impl EventLogService for InMemoryEventLog {
    fn record_event(&mut self, record: EventRecord) -> Result<()> {
        if self.max_records == Some(0) {
            return Ok(());
        }
        self.records.push(record);
        if let Some(max_records) = self.max_records {
            let overflow = self.records.len().saturating_sub(max_records);
            if overflow > 0 {
                self.records.drain(0..overflow);
            }
        }
        Ok(())
    }
}

impl QueryableEventLog for InMemoryEventLog {
    fn query_events(&self, filter: &EventFilter) -> Vec<EventRecord> {
        self.records
            .iter()
            .filter(|record| filter.matches(record))
            .cloned()
            .collect()
    }
}

/// Append-only file sink using [`EventRecord::to_log_line`].
#[derive(Debug, Clone)]
pub struct AppendOnlyEventLog {
    path: PathBuf,
}

impl AppendOnlyEventLog {
    /// Create a sink that appends events to `path`.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Path written by this sink.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Read the entire on-disk trail back, parsing each non-empty line into an
    /// [`EventRecord`].
    ///
    /// This is the read half of the append-only audit plane: it makes the
    /// written trail verifiable (a round-trip of [`EventRecord::to_log_line`] /
    /// [`EventRecord::from_log_line`]) rather than write-only. If the sink file
    /// does not exist yet (no event has been recorded), an empty `Vec` is
    /// returned — not an error.
    ///
    /// Returns an error if the file cannot be read or if any line is malformed
    /// (corruption is surfaced, never silently skipped — important for an audit
    /// trail).
    pub fn read_all(&self) -> Result<Vec<EventRecord>> {
        let contents = match std::fs::read_to_string(&self.path) {
            Ok(contents) => contents,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };
        contents
            .lines()
            .filter(|line| !line.is_empty())
            .map(EventRecord::from_log_line)
            .collect()
    }
}

impl EventLogService for AppendOnlyEventLog {
    fn record_event(&mut self, record: EventRecord) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(file, "{}", record.to_log_line())?;
        Ok(())
    }
}

fn now_micros() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64
}

fn escape_field(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

/// Inverse of [`escape_field`]: turn escape sequences back into their literal
/// control characters. A trailing lone backslash (which `escape_field` never
/// produces) is preserved verbatim rather than dropped.
fn unescape_field(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('\\') => out.push('\\'),
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            // Unknown escape (or trailing backslash): keep both chars as-is so
            // the operation never loses data.
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_record(id: &str, level: EventLevel) -> EventRecord {
        EventRecord::new(
            level,
            EventCategory::Session,
            "liquide-session",
            id,
            "test event",
        )
        .with_timestamp_us(100)
        .with_session("session-1")
    }

    #[test]
    fn event_log_record_builder_sets_context() {
        let record = test_record("session_created", EventLevel::Info)
            .with_resource("window:42")
            .with_correlation("corr-1")
            .with_context("owner", "1000");

        assert_eq!(record.session_id.as_deref(), Some("session-1"));
        assert_eq!(record.resource_id.as_deref(), Some("window:42"));
        assert_eq!(record.correlation_id.as_deref(), Some("corr-1"));
        assert_eq!(
            record.context.get("owner").map(String::as_str),
            Some("1000")
        );
    }

    #[test]
    fn event_log_filter_matches_category_level_and_session() {
        let record = test_record("worker_failed", EventLevel::Error);
        let filter = EventFilter {
            min_level: Some(EventLevel::Warn),
            category: Some(EventCategory::Session),
            session_id: Some("session-1".to_string()),
            ..EventFilter::default()
        };

        assert!(filter.matches(&record));

        let wrong_session = EventFilter {
            session_id: Some("session-2".to_string()),
            ..filter
        };
        assert!(!wrong_session.matches(&record));
    }

    #[test]
    fn event_log_in_memory_retains_newest_records() {
        let mut log = InMemoryEventLog::bounded(2);
        log.record_event(test_record("a", EventLevel::Info))
            .unwrap();
        log.record_event(test_record("b", EventLevel::Warn))
            .unwrap();
        log.record_event(test_record("c", EventLevel::Error))
            .unwrap();

        assert_eq!(log.len(), 2);
        assert_eq!(log.records()[0].event_id, "b");
        assert_eq!(log.records()[1].event_id, "c");
    }

    #[test]
    fn event_log_query_returns_matching_records() {
        let mut log = InMemoryEventLog::new();
        log.record_event(test_record("a", EventLevel::Info))
            .unwrap();
        log.record_event(test_record("b", EventLevel::Error).with_resource("display:1"))
            .unwrap();

        let matches = log.query_events(&EventFilter {
            min_level: Some(EventLevel::Error),
            resource_id: Some("display:1".to_string()),
            ..EventFilter::default()
        });

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].event_id, "b");
    }

    #[test]
    fn event_log_record_events_batch_records_in_order() {
        let mut log = InMemoryEventLog::new();
        let recorded = log
            .record_events([
                test_record("a", EventLevel::Info),
                test_record("b", EventLevel::Warn),
                test_record("c", EventLevel::Error),
            ])
            .unwrap();

        assert_eq!(recorded, 3);
        assert_eq!(log.len(), 3);
        assert_eq!(log.records()[0].event_id, "a");
        assert_eq!(log.records()[2].event_id, "c");
    }

    #[test]
    fn event_log_object_safe_via_dyn_sink() {
        // The facade and the authorization agent hold the sink as
        // `Box<dyn EventLogService>`; the batch helper's `Self: Sized` bound
        // keeps the trait object-safe. This guards that invariant.
        let mut sink: Box<dyn EventLogService> = Box::new(InMemoryEventLog::new());
        sink.record_event(test_record("dyn", EventLevel::Info))
            .unwrap();
    }

    #[test]
    fn event_log_line_escapes_control_characters() {
        let line = test_record("session\tcreated", EventLevel::Info)
            .with_context("note", "line\nbreak")
            .to_log_line();

        assert!(line.contains("session\\tcreated"));
        assert!(line.contains("line\\nbreak"));
    }

    #[test]
    fn event_level_and_category_labels_round_trip() {
        for level in [
            EventLevel::Trace,
            EventLevel::Debug,
            EventLevel::Info,
            EventLevel::Warn,
            EventLevel::Error,
            EventLevel::Critical,
        ] {
            assert_eq!(EventLevel::from_str(level.as_str()), Some(level));
        }
        assert_eq!(EventLevel::from_str("nope"), None);

        for category in [
            EventCategory::System,
            EventCategory::Security,
            EventCategory::Session,
            EventCategory::Authorization,
            EventCategory::Input,
            EventCategory::Rendering,
            EventCategory::Transport,
            EventCategory::Storage,
            EventCategory::Configuration,
            EventCategory::Accessibility,
            EventCategory::Custom,
        ] {
            assert_eq!(EventCategory::from_str(category.as_str()), Some(category));
        }
        assert_eq!(EventCategory::from_str("nope"), None);
    }

    #[test]
    fn event_log_line_round_trips_through_from_log_line() {
        let original = EventRecord::new(
            EventLevel::Warn,
            EventCategory::Authorization,
            "liquide-authorization",
            "power.shutdown",
            "authorization decision: Deny",
        )
        .with_timestamp_us(1_700_000_000_000_000)
        .with_session("session-1")
        .with_resource("user:alice")
        .with_correlation("corr-7")
        .with_context("decision", "Deny")
        .with_context("subject_uid", "1000");

        let line = original.to_log_line();
        let parsed = EventRecord::from_log_line(&line).expect("round-trip parse");
        assert_eq!(parsed, original);
    }

    #[test]
    fn from_log_line_unescapes_control_characters() {
        // Fields containing tabs/newlines/backslashes survive the round-trip.
        let original = EventRecord::new(
            EventLevel::Info,
            EventCategory::Session,
            "comp\twith\ttabs",
            "id\nwith\nnewlines",
            "msg with \\ backslash",
        )
        .with_timestamp_us(42)
        .with_context("key\twith\ttab", "value\nwith\nnewline");

        let parsed = EventRecord::from_log_line(&original.to_log_line()).expect("round-trip");
        assert_eq!(parsed, original);
    }

    #[test]
    fn from_log_line_rejects_malformed_lines() {
        // Too few fields.
        assert!(EventRecord::from_log_line("only\tthree\tfields").is_err());
        // Bad timestamp.
        let mut line = test_record("x", EventLevel::Info).to_log_line();
        line = line.replacen("100", "not-a-number", 1);
        assert!(EventRecord::from_log_line(&line).is_err());
    }

    #[test]
    fn append_only_log_read_all_round_trips_written_records() {
        // Write two records to a temp file, read them back, and assert byte-for
        // -byte equality — proving the on-disk audit trail is verifiable.
        let mut path = std::env::temp_dir();
        path.push(format!(
            "liquide-common-event-log-test-{}-{}.log",
            std::process::id(),
            now_micros()
        ));

        let mut sink = AppendOnlyEventLog::new(&path);
        // No file yet → read_all is empty, not an error.
        assert!(sink.read_all().expect("empty read").is_empty());

        let a = test_record("a", EventLevel::Info).with_context("k", "v");
        let b = test_record("b", EventLevel::Error).with_resource("display:1");
        sink.record_event(a.clone()).unwrap();
        sink.record_event(b.clone()).unwrap();

        let read_back = sink.read_all().expect("read back");
        assert_eq!(read_back.len(), 2);
        assert_eq!(read_back[0], a);
        assert_eq!(read_back[1], b);

        let _ = std::fs::remove_file(&path);
    }
}
