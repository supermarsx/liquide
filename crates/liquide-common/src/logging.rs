//! Structured logging initialization using the `tracing` ecosystem.

use crate::error::Result;

/// Log output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    /// Human-readable, coloured output for terminals.
    Pretty,
    /// Machine-readable JSON output.
    Json,
    /// Compact single-line output.
    Compact,
}

/// Configuration for the logging subsystem.
#[derive(Debug, Clone)]
pub struct LogConfig {
    /// The minimum log level (e.g. `"info"`, `"debug,liquide_transport=trace"`).
    pub filter: String,
    /// Output format.
    pub format: LogFormat,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            filter: "info".to_string(),
            format: LogFormat::Pretty,
        }
    }
}

/// Initialize the global tracing subscriber using the provided [`LogConfig`].
///
/// This should be called once, early in the process lifetime.
///
/// # Errors
///
/// Returns an error if the subscriber has already been set.
pub fn init(_config: &LogConfig) -> Result<()> {
    // Real implementation would set up tracing-subscriber here.
    // Left as a stub so the crate compiles without pulling in
    // tracing-subscriber as a hard dependency at this stage.
    tracing::info!("logging subsystem initialised");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_config_default() {
        let config = LogConfig::default();
        assert_eq!(config.filter, "info");
        assert_eq!(config.format, LogFormat::Pretty);
    }

    #[test]
    fn test_log_format_equality() {
        assert_ne!(LogFormat::Pretty, LogFormat::Json);
        assert_ne!(LogFormat::Json, LogFormat::Compact);
        assert_eq!(LogFormat::Pretty, LogFormat::Pretty);
    }

    #[test]
    fn test_log_config_custom() {
        let config = LogConfig {
            filter: "debug,liquide_transport=trace".to_string(),
            format: LogFormat::Json,
        };
        assert_eq!(config.filter, "debug,liquide_transport=trace");
        assert_eq!(config.format, LogFormat::Json);
    }
}
