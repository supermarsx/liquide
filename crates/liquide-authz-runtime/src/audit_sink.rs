//! Audit-file sink configuration for the enforcement facade.
//!
//! Checkpoint B (user-confirmed):
//!   * **Path** — the platform state directory, configurable, defaulting to
//!     `%PROGRAMDATA%\liquide\audit\events.log` on Windows and
//!     `/var/log/liquide/audit.log` on Linux (and other unix).
//!   * **Format** — the existing tab-separated [`AppendOnlyEventLog`] line
//!     format (already tested in `liquide-common`); no JSONL switch.
//!
//! Rotation is a deferred follow-up — see the TODO on [`AuditSinkConfig`].
//!
//! This module derives the platform default at runtime (it does not bake an
//! absolute path into the binary) and lets callers override it. The
//! [`AuditSinkConfig::into_sink`] constructor produces a boxed
//! [`EventLogService`] backed by [`AppendOnlyEventLog`], which
//! [`crate::AuthorizationRuntime`] injects as its event-log sink.

use std::path::{Path, PathBuf};

use liquide_common::event_log::{AppendOnlyEventLog, EventLogService};

/// Default audit log file name placed under the platform state directory on
/// Windows.
const WINDOWS_AUDIT_RELATIVE: &str = r"liquide\audit\events.log";

/// Default absolute audit log path on Linux / other unix.
const UNIX_AUDIT_DEFAULT: &str = "/var/log/liquide/audit.log";

/// Environment variable holding the Windows program-data state directory.
const PROGRAMDATA_ENV: &str = "ProgramData";

/// Fallback Windows program-data path if `%ProgramData%` is unset.
const PROGRAMDATA_FALLBACK: &str = r"C:\ProgramData";

/// Configuration for the facade's append-only audit file sink.
///
/// The path defaults to the platform state location (see module docs) but is
/// fully overridable via [`AuditSinkConfig::with_path`].
///
// TODO(t51 follow-up): add a rotation cap (max bytes / max files) so the
// append-only audit file does not grow without bound. Deferred per Checkpoint
// B; the `AppendOnlyEventLog` sink currently appends unconditionally.
#[derive(Debug, Clone)]
pub struct AuditSinkConfig {
    path: PathBuf,
}

impl AuditSinkConfig {
    /// Construct a config using the platform-default audit path.
    ///
    /// The default is resolved at runtime:
    ///   * Windows → `%ProgramData%\liquide\audit\events.log`
    ///     (falling back to `C:\ProgramData\...` if the env var is unset).
    ///   * Linux / other unix → `/var/log/liquide/audit.log`.
    #[must_use]
    pub fn platform_default() -> Self {
        Self {
            path: default_audit_path(),
        }
    }

    /// Construct a config with an explicit, caller-chosen audit path.
    #[must_use]
    pub fn with_path(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// The audit file path this config will write to.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Build the boxed append-only event-log sink for this config.
    ///
    /// The returned sink writes the existing TSV line format. Note that
    /// [`AppendOnlyEventLog`] creates the file lazily on the first
    /// `record_event` call; this constructor does not touch the filesystem.
    #[must_use]
    pub fn into_sink(self) -> Box<dyn EventLogService> {
        Box::new(AppendOnlyEventLog::new(self.path))
    }
}

impl Default for AuditSinkConfig {
    fn default() -> Self {
        Self::platform_default()
    }
}

/// Resolve the platform-default audit file path at runtime.
#[must_use]
pub fn default_audit_path() -> PathBuf {
    if cfg!(windows) {
        let base = std::env::var_os(PROGRAMDATA_ENV)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(PROGRAMDATA_FALLBACK));
        base.join(WINDOWS_AUDIT_RELATIVE)
    } else {
        PathBuf::from(UNIX_AUDIT_DEFAULT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_path_resolves_to_documented_platform_location() {
        // This must NOT write to the real platform path — it only resolves it.
        let path = default_audit_path();

        if cfg!(windows) {
            // Ends with the documented relative audit path.
            assert!(
                path.ends_with(r"liquide\audit\events.log")
                    || path.ends_with("liquide/audit/events.log"),
                "windows default should end with liquide/audit/events.log, got {path:?}"
            );
            // Rooted at the program-data base (env or the C:\ProgramData fallback).
            let base = std::env::var_os(PROGRAMDATA_ENV)
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(PROGRAMDATA_FALLBACK));
            assert!(
                path.starts_with(&base),
                "windows default {path:?} should be rooted at {base:?}"
            );
        } else {
            assert_eq!(path, PathBuf::from("/var/log/liquide/audit.log"));
        }

        // The config exposes the same default and never touches the filesystem.
        assert_eq!(AuditSinkConfig::platform_default().path(), path.as_path());
        assert!(!path.as_os_str().is_empty());
    }

    #[test]
    fn with_path_overrides_the_default() {
        let custom = PathBuf::from(if cfg!(windows) {
            r"D:\custom\audit.log"
        } else {
            "/tmp/custom/audit.log"
        });
        let config = AuditSinkConfig::with_path(custom.clone());
        assert_eq!(config.path(), custom.as_path());
        assert_ne!(config.path(), default_audit_path().as_path());
    }
}
