//! System Event Viewer types for browsing OS-level event logs.
//!
//! Provides structured access to Windows Event Log, Linux journald,
//! and macOS Unified Logging entries. Modelled after the Windows
//! Event Viewer with Application, System, Security, and custom log
//! sources.

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// EventLogSource
// ---------------------------------------------------------------------------

/// Source log from which a system event originates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventLogSource {
    /// Application-level events (crashes, warnings, informational).
    Application,
    /// Core OS / kernel events (driver errors, service state changes).
    System,
    /// Authentication, access-control, and audit events.
    Security,
    /// OS and component install / update events.
    Setup,
    /// Events forwarded from remote machines.
    ForwardedEvents,
    /// Hardware-specific events (WHEA, disk, SMART).
    Hardware,
    /// User-defined or third-party log.
    Custom,
}

impl EventLogSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Application => "Application",
            Self::System => "System",
            Self::Security => "Security",
            Self::Setup => "Setup",
            Self::ForwardedEvents => "Forwarded Events",
            Self::Hardware => "Hardware",
            Self::Custom => "Custom",
        }
    }
}

impl fmt::Display for EventLogSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// EventLevel
// ---------------------------------------------------------------------------

/// Severity level of a system event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventLevel {
    /// Diagnostic trace-level information.
    Verbose,
    /// Informational message.
    Information,
    /// Potential issue that may require attention.
    Warning,
    /// A significant failure occurred.
    Error,
    /// A fatal or unrecoverable failure.
    Critical,
}

impl EventLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Verbose => "Verbose",
            Self::Information => "Information",
            Self::Warning => "Warning",
            Self::Error => "Error",
            Self::Critical => "Critical",
        }
    }

    /// Numeric severity (higher = more severe).
    pub fn severity(&self) -> u8 {
        match self {
            Self::Verbose => 0,
            Self::Information => 1,
            Self::Warning => 2,
            Self::Error => 3,
            Self::Critical => 4,
        }
    }
}

impl fmt::Display for EventLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// EventCategory
// ---------------------------------------------------------------------------

/// Broad category of a system event for filtering and grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventCategory {
    /// No category assigned.
    None,
    /// Disk / storage events.
    Disk,
    /// Network adapter or connectivity events.
    Network,
    /// Printer / scanner events.
    Printer,
    /// Security and audit events.
    Security,
    /// Service control manager events.
    ServiceControl,
    /// Shell and desktop events.
    Shell,
    /// Power management events.
    Power,
    /// Driver installation or failure events.
    Driver,
    /// Windows Update or package management events.
    Update,
    /// Application crash or hang events.
    ApplicationError,
}

impl EventCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Disk => "Disk",
            Self::Network => "Network",
            Self::Printer => "Printer",
            Self::Security => "Security",
            Self::ServiceControl => "Service Control",
            Self::Shell => "Shell",
            Self::Power => "Power",
            Self::Driver => "Driver",
            Self::Update => "Update",
            Self::ApplicationError => "Application Error",
        }
    }
}

impl fmt::Display for EventCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// SystemEvent
// ---------------------------------------------------------------------------

/// A single entry from the system event log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemEvent {
    /// Unique record number within the log.
    pub record_id: u64,
    /// ISO-8601 timestamp of the event.
    pub timestamp: String,
    /// Source log (Application, System, Security, etc.).
    pub source: EventLogSource,
    /// Provider or application that generated the event.
    pub provider: String,
    /// Numeric event identifier defined by the provider.
    pub event_id: u32,
    /// Severity level.
    pub level: EventLevel,
    /// Broad event category.
    pub category: EventCategory,
    /// Short task or operation name (e.g. "Logon", "ServiceStart").
    pub task: Option<String>,
    /// Human-readable event message.
    pub message: String,
    /// Structured XML or JSON event data (if available).
    pub raw_data: Option<String>,
    /// User account associated with the event (SID or username).
    pub user: Option<String>,
    /// Machine name that generated the event.
    pub computer: String,
    /// Process ID that generated the event.
    pub process_id: Option<u32>,
    /// Thread ID that generated the event.
    pub thread_id: Option<u32>,
    /// Keywords or tags associated with the event.
    pub keywords: Vec<String>,
    /// Correlation activity ID for grouped events.
    pub activity_id: Option<String>,
}

impl Default for SystemEvent {
    fn default() -> Self {
        Self {
            record_id: 0,
            timestamp: String::new(),
            source: EventLogSource::Application,
            provider: String::new(),
            event_id: 0,
            level: EventLevel::Information,
            category: EventCategory::None,
            task: None,
            message: String::new(),
            raw_data: None,
            user: None,
            computer: String::new(),
            process_id: None,
            thread_id: None,
            keywords: Vec::new(),
            activity_id: None,
        }
    }
}

// ---------------------------------------------------------------------------
// EventLogFilter
// ---------------------------------------------------------------------------

/// Filter criteria for querying system events.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EventLogFilter {
    /// Only include events from these sources.
    pub sources: Option<Vec<EventLogSource>>,
    /// Only include events at or above these severity levels.
    pub levels: Option<Vec<EventLevel>>,
    /// Only include events matching these event IDs.
    pub event_ids: Option<Vec<u32>>,
    /// Only include events from these providers.
    pub providers: Option<Vec<String>>,
    /// Only include events in these categories.
    pub categories: Option<Vec<EventCategory>>,
    /// Only include events after this ISO-8601 timestamp.
    pub from_time: Option<String>,
    /// Only include events before this ISO-8601 timestamp.
    pub to_time: Option<String>,
    /// Free-text search within the event message.
    pub keyword_search: Option<String>,
    /// Only include events generated by this process ID.
    pub pid: Option<u32>,
    /// Maximum number of events to return.
    pub max_results: Option<u32>,
}

// ---------------------------------------------------------------------------
// EventLogStats
// ---------------------------------------------------------------------------

/// Summary statistics for the system event log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventLogStats {
    /// Total number of events in the log.
    pub total_events: u64,
    /// Number of critical events.
    pub critical_count: u64,
    /// Number of error events.
    pub error_count: u64,
    /// Number of warning events.
    pub warning_count: u64,
    /// Number of informational events.
    pub information_count: u64,
    /// Number of verbose events.
    pub verbose_count: u64,
    /// ISO-8601 timestamp of the oldest event.
    pub oldest_event: Option<String>,
    /// ISO-8601 timestamp of the newest event.
    pub newest_event: Option<String>,
    /// Total log size in bytes.
    pub log_size_bytes: u64,
    /// Maximum log size in bytes.
    pub max_log_size_bytes: u64,
}

// ---------------------------------------------------------------------------
// EventLogView
// ---------------------------------------------------------------------------

/// View mode for the event viewer tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventLogView {
    /// Show all events in a flat list.
    All,
    /// Filter to Application log only.
    Application,
    /// Filter to System log only.
    System,
    /// Filter to Security log only.
    Security,
    /// Filter to Setup log only.
    Setup,
    /// Filter to Hardware log only.
    Hardware,
    /// Show summary statistics and charts.
    Summary,
    /// Show saved / bookmarked events.
    Bookmarks,
    /// Custom filter view.
    CustomFilter,
}

impl EventLogView {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::All => "All Events",
            Self::Application => "Application",
            Self::System => "System",
            Self::Security => "Security",
            Self::Setup => "Setup",
            Self::Hardware => "Hardware",
            Self::Summary => "Summary",
            Self::Bookmarks => "Bookmarks",
            Self::CustomFilter => "Custom Filter",
        }
    }
}

impl fmt::Display for EventLogView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// EventBookmark
// ---------------------------------------------------------------------------

/// A user-saved bookmark referencing a specific event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventBookmark {
    /// The bookmarked event record ID.
    pub record_id: u64,
    /// The log source where the event lives.
    pub source: EventLogSource,
    /// User-supplied note about the bookmark.
    pub note: Option<String>,
    /// ISO-8601 timestamp when the bookmark was created.
    pub created_at: String,
}

// ---------------------------------------------------------------------------
// EventExportOptions
// ---------------------------------------------------------------------------

/// Options for exporting event log data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventExportOptions {
    /// Export format.
    pub format: EventExportFormat,
    /// Filter to apply before export.
    pub filter: EventLogFilter,
    /// Whether to include raw XML/JSON data.
    pub include_raw_data: bool,
}

/// Format for event log export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventExportFormat {
    Csv,
    Json,
    Xml,
    Evtx,
    Html,
}

impl EventExportFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Csv => "CSV",
            Self::Json => "JSON",
            Self::Xml => "XML",
            Self::Evtx => "EVTX",
            Self::Html => "HTML",
        }
    }
}

impl fmt::Display for EventExportFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// EventCorrelation
// ---------------------------------------------------------------------------

/// A group of correlated events sharing an activity ID.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventCorrelation {
    /// The shared activity ID.
    pub activity_id: String,
    /// Record IDs of all events in this group.
    pub event_record_ids: Vec<u64>,
    /// Short description of the correlated activity.
    pub description: Option<String>,
}

// ---------------------------------------------------------------------------
// EventAction
// ---------------------------------------------------------------------------

/// Actions the user can perform on system events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventAction {
    /// Open the event detail pane.
    ViewDetails,
    /// Bookmark this event for later reference.
    Bookmark,
    /// Remove a bookmark.
    RemoveBookmark,
    /// Copy the event message to the clipboard.
    CopyMessage,
    /// Copy the full event XML/JSON to the clipboard.
    CopyRawData,
    /// Look up the event ID online.
    LookupOnline,
    /// Create a filter from this event's provider and ID.
    CreateFilter,
    /// Clear all events in a log (requires elevation).
    ClearLog,
    /// Export selected events.
    ExportSelected,
    /// Attach a task (scheduled task trigger) to this event ID.
    AttachTask,
}

impl EventAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ViewDetails => "View Details",
            Self::Bookmark => "Bookmark",
            Self::RemoveBookmark => "Remove Bookmark",
            Self::CopyMessage => "Copy Message",
            Self::CopyRawData => "Copy Raw Data",
            Self::LookupOnline => "Lookup Online",
            Self::CreateFilter => "Create Filter",
            Self::ClearLog => "Clear Log",
            Self::ExportSelected => "Export Selected",
            Self::AttachTask => "Attach Task",
        }
    }
}

impl fmt::Display for EventAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
