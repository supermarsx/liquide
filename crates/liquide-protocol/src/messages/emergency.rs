//! Emergency channel message types.
//!
//! The emergency channel (0x01) operates independently of the session process
//! and is maintained by the supervisor daemon. It carries crash notifications,
//! diagnostic data, and supervisor heartbeats even when the session has failed.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Session crash notification (BSOD data).
///
/// Sent by the supervisor when the session process terminates abnormally.
/// Contains enough information for the client to display a crash dialog and
/// optionally request a full crash report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrashInfoMsg {
    /// Machine-readable error code (e.g. "SESSION_PROCESS_CRASH", "SESSION_OOM").
    pub error_code: String,
    /// Human-readable description of the crash.
    pub description: String,
    /// Severity scope: "session", "connection", or "server".
    pub severity: String,
    /// Stack trace lines, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack_trace: Option<Vec<String>>,
    /// Identifier of the crashed session.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Username that owned the session.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    /// How long the session was running before the crash.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uptime_seconds: Option<u64>,
    /// Unique identifier for the crash report (used to request the full report).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crash_report_id: Option<String>,
    /// Process exit code, if the process exited normally.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    /// Signal name that killed the process (e.g. "SIGSEGV", "SIGKILL").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal_name: Option<String>,
    /// Actions the client can take (e.g. "reconnect", "restart", "download_report").
    pub recovery_options: Vec<String>,
    /// Whether the supervisor is able to restart the session.
    pub restart_available: bool,
    /// ISO 8601 timestamp of the crash event.
    pub timestamp: String,
    /// Last N lines of the session log before the crash.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_tail: Option<Vec<String>>,
}

/// A chunk of crash-log text streamed to the client.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrashLogChunkMsg {
    /// Raw log data for this chunk.
    pub data: Vec<u8>,
    /// Zero-based index of this chunk in the stream.
    pub chunk_index: u32,
}

/// Marker indicating the crash-log stream is complete.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrashLogEndMsg {}

/// Client request for a full crash report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrashReportRequestMsg {
    /// Which crash report to retrieve.
    pub crash_report_id: String,
    /// Include the tail of the session log in the report.
    pub include_log_tail: bool,
    /// Include a stack trace in the report.
    pub include_stack_trace: bool,
    /// Include system information (OS version, hardware, etc.) in the report.
    pub include_system_info: bool,
    /// Include a core dump in the report (may be very large).
    pub include_coredump: bool,
}

/// A chunk of crash-report binary data streamed to the client.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrashReportChunkMsg {
    /// Which crash report this chunk belongs to.
    pub crash_report_id: String,
    /// Zero-based index of this chunk.
    pub chunk_index: u32,
    /// Total number of chunks in the report.
    pub total_chunks: u32,
    /// Binary data for this chunk.
    pub data: Vec<u8>,
}

/// End-of-stream marker for a crash report transfer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrashReportEndMsg {
    /// Which crash report just finished.
    pub crash_report_id: String,
    /// Total size of the crash report in bytes.
    pub total_size: u64,
    /// SHA-256 digest of the complete report for integrity verification.
    pub sha256: Vec<u8>,
}

/// Supervisor status update for a session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SupervisorStatusMsg {
    /// Session identifier being monitored.
    pub session_id: String,
    /// Current status: "running", "crashed", "restarting", "stopped".
    pub status: String,
    /// Time the session has been in its current state (seconds).
    pub uptime_seconds: u64,
    /// OS process ID of the session, if running.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
}

/// Client request to restart a session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RestartRequestMsg {
    /// Session to restart. If `None`, restart the current session.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

/// Status update during a session restart.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RestartStatusMsg {
    /// Restart status: "starting", "in_progress", "completed", "failed".
    pub status: String,
    /// Percentage of restart progress (0-100).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress_percent: Option<u32>,
    /// Human-readable status message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Emergency-channel heartbeat from the supervisor.
///
/// Sent periodically to prove the supervisor is still alive even when the
/// session process may have died.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeartbeatEmergencyMsg {
    /// Monotonic timestamp in microseconds.
    pub timestamp_us: u64,
}

/// Notification that the server is shutting down gracefully.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServerShutdownMsg {
    /// Reason for the shutdown (e.g. "maintenance", "admin_request", "update").
    pub reason: String,
    /// Whether the server is expected to restart after the shutdown.
    pub restart_expected: bool,
}

/// Real-time log line forwarded from the session process.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionLogStreamMsg {
    /// Session that produced this log line.
    pub session_id: String,
    /// ISO 8601 timestamp of the log event.
    pub timestamp: String,
    /// Log level (e.g. "error", "warn", "info", "debug", "trace").
    pub level: String,
    /// Subsystem that generated the log (e.g. "compositor", "encoder", "input").
    pub subsystem: String,
    /// The log message text.
    pub message: String,
}

/// Client request for diagnostic data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticRequestMsg {
    /// Type of diagnostic requested (e.g. "memory", "cpu", "network", "gpu").
    pub diagnostic_type: String,
}

/// Diagnostic data returned by the supervisor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticResponseMsg {
    /// Type of diagnostic this response contains.
    pub diagnostic_type: String,
    /// Key-value pairs of diagnostic data.
    pub data: BTreeMap<String, String>,
}
