//! Telemetry data collector - reads telemetry from live session.

use crate::types::TelemetrySnapshot;
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::time::Duration;
use tokio::time;

/// Collects telemetry data from a running Liquide session.
pub struct TelemetryCollector {
    /// Path to the telemetry shared memory or socket.
    source_path: PathBuf,

    /// Remote connection address (if collecting from remote session).
    remote_addr: Option<String>,
}

impl TelemetryCollector {
    /// Create a new collector for local session.
    pub fn local() -> Self {
        Self {
            source_path: Self::default_telemetry_path(),
            remote_addr: None,
        }
    }

    /// Create a new collector for remote session.
    pub fn remote(addr: String) -> Self {
        Self {
            source_path: PathBuf::new(),
            remote_addr: Some(addr),
        }
    }

    /// Get the default telemetry data path.
    fn default_telemetry_path() -> PathBuf {
        #[cfg(target_os = "linux")]
        {
            PathBuf::from("/tmp/liquide-telemetry.json")
        }

        #[cfg(target_os = "windows")]
        {
            std::env::temp_dir().join("liquide-telemetry.json")
        }

        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            PathBuf::from("/tmp/liquide-telemetry.json")
        }
    }

    /// Collect a single telemetry snapshot.
    pub async fn collect(&self) -> Result<TelemetrySnapshot> {
        if let Some(ref remote) = self.remote_addr {
            self.collect_remote(remote).await
        } else {
            self.collect_local().await
        }
    }

    /// Collect from local file/shared memory.
    async fn collect_local(&self) -> Result<TelemetrySnapshot> {
        if !self.source_path.exists() {
            tracing::warn!("telemetry file not found, returning default snapshot");
            return Ok(TelemetrySnapshot::default());
        }

        let contents = tokio::fs::read_to_string(&self.source_path)
            .await
            .context("failed to read telemetry file")?;

        let snapshot: TelemetrySnapshot =
            serde_json::from_str(&contents).context("failed to parse telemetry data")?;

        Ok(snapshot)
    }

    /// Collect from remote session via HTTP.
    async fn collect_remote(&self, addr: &str) -> Result<TelemetrySnapshot> {
        let _url = format!("http://{}/telemetry", addr);

        // In a real implementation, use reqwest or similar to fetch data
        tracing::warn!("remote collection not yet implemented");
        Ok(TelemetrySnapshot::default())
    }

    /// Start continuous collection with a callback.
    pub async fn collect_continuous<F>(&self, interval_ms: u64, mut callback: F) -> Result<()>
    where
        F: FnMut(TelemetrySnapshot) -> bool,
    {
        let mut interval = time::interval(Duration::from_millis(interval_ms));

        loop {
            interval.tick().await;

            match self.collect().await {
                Ok(snapshot) => {
                    if !callback(snapshot) {
                        break;
                    }
                }
                Err(e) => {
                    tracing::error!("failed to collect telemetry: {}", e);
                }
            }
        }

        Ok(())
    }
}

/// Export telemetry data to the standard location for viewers to read.
#[allow(dead_code)]
pub fn export_telemetry(snapshot: &TelemetrySnapshot) -> Result<()> {
    let path = TelemetryCollector::default_telemetry_path();
    let json = serde_json::to_string_pretty(snapshot)?;
    std::fs::write(path, json)?;
    Ok(())
}
