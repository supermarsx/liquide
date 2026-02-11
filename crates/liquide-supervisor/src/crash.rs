//! Crash detection, classification, and reporting.

use std::time::Instant;

use crate::Result;

/// Category of a crash event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrashCategory {
    /// Segmentation fault (SIGSEGV).
    Segfault,
    /// Abort signal (SIGABRT).
    Abort,
    /// Bus error (SIGBUS).
    BusError,
    /// Floating point exception (SIGFPE).
    FloatingPoint,
    /// Illegal instruction (SIGILL).
    IllegalInstruction,
    /// Heartbeat timeout (session stopped responding).
    HeartbeatTimeout,
    /// OOM killer terminated the process.
    OomKill,
    /// Rust panic or equivalent.
    Panic,
    /// Resource exhaustion (file descriptors, disk, etc.).
    ResourceExhaustion,
    /// Unknown or unclassifiable crash.
    Unknown,
}

impl std::fmt::Display for CrashCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Segfault => write!(f, "Segfault"),
            Self::Abort => write!(f, "Abort"),
            Self::BusError => write!(f, "BusError"),
            Self::FloatingPoint => write!(f, "FloatingPoint"),
            Self::IllegalInstruction => write!(f, "IllegalInstruction"),
            Self::HeartbeatTimeout => write!(f, "HeartbeatTimeout"),
            Self::OomKill => write!(f, "OomKill"),
            Self::Panic => write!(f, "Panic"),
            Self::ResourceExhaustion => write!(f, "ResourceExhaustion"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

/// A snapshot of resource usage at the time of crash.
#[derive(Debug, Clone, Default)]
pub struct CrashResourceSnapshot {
    /// CPU usage percentage at crash time.
    pub cpu_pct: f64,
    /// Memory used in megabytes at crash time.
    pub memory_mb: u64,
    /// Number of active PIDs at crash time.
    pub pids: u32,
}

/// A crash report.
#[derive(Debug, Clone)]
pub struct CrashReport {
    /// Unique crash identifier.
    pub crash_id: String,
    /// Session that crashed.
    pub session_id: String,
    /// User who owned the session.
    pub user: String,
    /// When the crash was detected.
    pub timestamp: Instant,
    /// Classified crash category.
    pub category: CrashCategory,
    /// Signal that caused the crash (if applicable).
    pub signal: Option<i32>,
    /// Exit code (if available).
    pub exit_code: Option<i32>,
    /// Path to core dump (if generated).
    pub coredump_path: Option<String>,
    /// Captured log lines.
    pub log_lines: Vec<String>,
    /// Uptime of the session in seconds before crash.
    pub uptime_seconds: u64,
    /// Active plugins at the time of crash.
    pub active_plugins: Vec<String>,
    /// Resource snapshot at crash time.
    pub resource_snapshot: CrashResourceSnapshot,
}

/// Handles crash detection, classification, and report generation.
pub struct CrashHandler {
    crash_report_dir: String,
    next_crash_id: u64,
}

impl CrashHandler {
    /// Create a new crash handler.
    #[must_use]
    pub fn new(crash_report_dir: String) -> Self {
        Self {
            crash_report_dir,
            next_crash_id: 1,
        }
    }

    /// Classify a crash based on signal and exit code.
    #[must_use]
    pub fn classify_crash(signal: Option<i32>, exit_code: Option<i32>) -> CrashCategory {
        if let Some(sig) = signal {
            match sig {
                11 => CrashCategory::Segfault,       // SIGSEGV
                6 => CrashCategory::Abort,            // SIGABRT
                7 => CrashCategory::BusError,         // SIGBUS
                8 => CrashCategory::FloatingPoint,    // SIGFPE
                4 => CrashCategory::IllegalInstruction, // SIGILL
                9 => CrashCategory::OomKill,          // SIGKILL (often OOM)
                _ => CrashCategory::Unknown,
            }
        } else if let Some(code) = exit_code {
            match code {
                101 => CrashCategory::Panic,
                137 => CrashCategory::OomKill, // 128 + SIGKILL
                _ => CrashCategory::Unknown,
            }
        } else {
            CrashCategory::Unknown
        }
    }

    /// Generate a crash report.
    pub fn generate_report(
        &mut self,
        session_id: &str,
        user: &str,
        signal: Option<i32>,
        exit_code: Option<i32>,
        uptime_seconds: u64,
        log_lines: Vec<String>,
    ) -> CrashReport {
        let crash_id = format!("crash-{}", self.next_crash_id);
        self.next_crash_id += 1;

        let category = Self::classify_crash(signal, exit_code);

        CrashReport {
            crash_id,
            session_id: session_id.to_string(),
            user: user.to_string(),
            timestamp: Instant::now(),
            category,
            signal,
            exit_code,
            coredump_path: None,
            log_lines,
            uptime_seconds,
            active_plugins: Vec::new(),
            resource_snapshot: CrashResourceSnapshot::default(),
        }
    }

    /// Store a crash report to disk.
    ///
    /// In a real implementation this writes the report to the crash report
    /// directory. Returns the path where the report was stored.
    pub fn store_report(&self, report: &CrashReport) -> Result<String> {
        let path = format!("{}/{}.json", self.crash_report_dir, report.crash_id);
        // Stub: would serialize and write to disk.
        Ok(path)
    }

    /// The crash report directory.
    #[must_use]
    pub fn crash_report_dir(&self) -> &str {
        &self.crash_report_dir
    }
}
