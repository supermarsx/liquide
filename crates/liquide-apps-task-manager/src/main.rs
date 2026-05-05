//! CLI entry point for the Liquide Task Manager.
//!
//! Provides both a GUI launch (default) and a rich set of CLI subcommands
//! for querying processes, exporting data, unlocking files, network
//! diagnostics, energy reports, and audio management.

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use liquide_apps_task_manager::config::TaskManagerConfig;
use liquide_apps_task_manager::ui::TabId;
use liquide_apps_task_manager::{run_default_app, run_widget_app};

/// Liquide Task Manager — system monitoring and management tool.
#[derive(Debug, Parser)]
#[command(name = "liquid-taskmanager", version, about)]
struct Cli {
    /// Tab name (processes, performance, app_history, startup, users, services, devices, files, unlock, process_tree, network, energy, audio, events).
    #[arg(long)]
    tab: Option<String>,

    /// Start in floating widget mode.
    #[arg(long)]
    widget: bool,

    /// Path to a custom config file.
    #[arg(long)]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Command>,
}

/// Available CLI subcommands.
#[derive(Debug, Subcommand)]
enum Command {
    // ── Process management ──────────────────────────────────────
    /// Export the current process list.
    Export {
        /// What to export (processes, services, connections).
        target: String,
        /// Output format.
        #[arg(long, default_value = "csv")]
        format: String,
        /// Output file path.
        #[arg(long)]
        output: PathBuf,
    },

    /// Query process info by filter expression.
    Query {
        /// Filter expression (e.g. "pid=4200").
        #[arg(long)]
        filter: String,
        /// Columns to display.
        #[arg(long)]
        columns: Option<String>,
    },

    // ── File unlocking ──────────────────────────────────────────
    /// Unlock a locked file or folder.
    Unlock {
        /// Path to the locked resource.
        path: PathBuf,
    },

    /// List open handles for a path.
    Handles {
        /// Path to inspect.
        path: PathBuf,
    },

    // ── Boot timeline ───────────────────────────────────────────
    /// Display boot timeline data.
    BootTimeline {
        /// Output format.
        #[arg(long, default_value = "json")]
        format: String,
    },

    // ── Daemon / headless mode ──────────────────────────────────
    /// Run in headless monitoring mode.
    Daemon {
        /// CPU threshold for alerts (%).
        #[arg(long, default_value = "0")]
        alert_cpu: u8,
        /// Memory threshold for alerts (%).
        #[arg(long, default_value = "0")]
        alert_mem: u8,
    },

    // ── Network subcommands ─────────────────────────────────────
    /// List active network connections.
    Connections {
        /// Output format.
        #[arg(long, default_value = "json")]
        format: String,
        /// Output file path.
        #[arg(long)]
        output: Option<PathBuf>,
    },

    /// Show DNS query log.
    DnsLog {
        /// Duration to capture (seconds).
        #[arg(long, default_value = "60")]
        duration: u64,
        /// Output format.
        #[arg(long, default_value = "csv")]
        format: String,
    },

    /// Show per-process bandwidth usage.
    Bandwidth {
        /// Sort field.
        #[arg(long, default_value = "recv")]
        sort: String,
        /// Number of top entries.
        #[arg(long, default_value = "20")]
        top: usize,
    },

    /// Run a network speed test.
    SpeedTest {
        /// Output format.
        #[arg(long, default_value = "json")]
        format: String,
    },

    /// Trace route to a host.
    Traceroute {
        /// Target host or IP.
        host: String,
    },

    /// Start a packet capture session.
    Capture {
        /// Network interface to capture on.
        #[arg(long)]
        interface: String,
        /// BPF filter expression.
        #[arg(long)]
        filter: Option<String>,
        /// Output file path.
        #[arg(long)]
        output: PathBuf,
        /// Duration in seconds.
        #[arg(long)]
        duration: Option<u64>,
    },

    /// Block a remote IP address via firewall rule.
    FirewallBlock {
        /// IP address to block.
        address: String,
    },

    /// Generate a network usage report.
    NetUsage {
        /// Reporting period (daily, weekly, monthly).
        #[arg(long, default_value = "monthly")]
        period: String,
        /// Output format.
        #[arg(long, default_value = "csv")]
        format: String,
        /// Output file path.
        #[arg(long)]
        output: Option<PathBuf>,
    },

    // ── Energy subcommands ──────────────────────────────────────
    /// Show current power draw summary.
    PowerSummary,

    /// Generate a battery health report.
    BatteryReport {
        /// Output format.
        #[arg(long, default_value = "html")]
        format: String,
        /// Output file path.
        #[arg(long)]
        output: Option<PathBuf>,
    },

    /// Show thermal sensor readings.
    Thermals {
        /// Output format.
        #[arg(long, default_value = "json")]
        format: String,
    },

    /// Show per-process energy ranking.
    EnergyTop {
        /// Number of top entries.
        #[arg(long, default_value = "20")]
        top: usize,
    },

    /// Switch the active power profile.
    PowerProfile {
        /// Profile name (silent, balanced, performance).
        name: String,
    },

    /// Generate a carbon footprint report.
    CarbonReport {
        /// Reporting period.
        #[arg(long, default_value = "weekly")]
        period: String,
    },

    // ── Audio subcommands ───────────────────────────────────────
    /// List audio devices.
    AudioDevices {
        /// Output format.
        #[arg(long, default_value = "json")]
        format: String,
    },

    /// List active audio streams.
    AudioStreams,

    /// Set the default audio output device.
    AudioDefaultOutput {
        /// Device name.
        name: String,
    },

    /// Set volume for an audio device.
    AudioVolume {
        /// Volume level (0-100).
        level: u8,
        /// Device name.
        #[arg(long)]
        device: String,
    },

    /// Run audio diagnostics.
    AudioDiag {
        /// Comma-separated test names.
        #[arg(long)]
        test: String,
    },

    /// Record from an input device.
    AudioRecord {
        /// Input device name.
        #[arg(long)]
        device: String,
        /// Duration in seconds.
        #[arg(long)]
        duration: u64,
        /// Output file path.
        #[arg(long)]
        output: PathBuf,
    },

    // ── Event viewer subcommands ────────────────────────────────────
    /// List system events with optional filtering.
    EventLog {
        /// Log source (application, system, security, setup, hardware, all).
        #[arg(long, default_value = "all")]
        source: String,
        /// Minimum severity level (verbose, information, warning, error, critical).
        #[arg(long, default_value = "information")]
        level: String,
        /// Maximum number of events to return.
        #[arg(long, default_value = "100")]
        limit: u32,
        /// Output format.
        #[arg(long, default_value = "json")]
        format: String,
        /// Output file path.
        #[arg(long)]
        output: Option<PathBuf>,
    },

    /// Get summary statistics for the system event log.
    EventStats,

    /// Clear a specific event log (requires elevation).
    EventClear {
        /// Log source to clear (application, system, security).
        source: String,
    },

    /// Export system events.
    EventExport {
        /// Output format (csv, json, xml, evtx, html).
        #[arg(long, default_value = "csv")]
        format: String,
        /// Output file path.
        #[arg(long)]
        output: PathBuf,
        /// Log source filter.
        #[arg(long)]
        source: Option<String>,
        /// Maximum number of hours to include.
        #[arg(long)]
        hours: Option<u32>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialise tracing.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    let cli = Cli::parse();

    // Load configuration.
    let config = if let Some(path) = &cli.config {
        let text = tokio::fs::read_to_string(path).await?;
        toml::from_str::<TaskManagerConfig>(&text)?
    } else {
        TaskManagerConfig::default()
    };

    let active_tab = if let Some(ref tab_name) = cli.tab {
        let tab = match tab_name.as_str() {
            "processes" => TabId::Processes,
            "performance" => TabId::Performance,
            "app_history" => TabId::AppHistory,
            "startup" => TabId::Startup,
            "users" => TabId::Users,
            "services" => TabId::Services,
            "devices" => TabId::Devices,
            "files" => TabId::FilesInUse,
            "unlock" => TabId::ResourceUnlocking,
            "process_tree" => TabId::ProcessTree,
            "network" => TabId::NetworkTraffic,
            "energy" => TabId::EnergyPower,
            "audio" => TabId::Audio,
            "events" | "event_viewer" => TabId::SystemEventViewer,
            other => anyhow::bail!("unknown tab: {other}"),
        };
        Some(tab)
    } else {
        None
    };

    match cli.command {
        None => {
            if cli.widget {
                run_widget_app(config, active_tab).await
            } else {
                run_default_app(config, active_tab).await
            }
        }

        Some(Command::Export {
            target,
            format,
            output,
        }) => {
            tracing::info!(%target, %format, ?output, "exporting data");
            Ok(())
        }

        Some(Command::Query { filter, columns }) => {
            tracing::info!(%filter, ?columns, "querying processes");
            Ok(())
        }

        Some(Command::Unlock { path }) => {
            tracing::info!(?path, "unlocking resource");
            Ok(())
        }

        Some(Command::Handles { path }) => {
            tracing::info!(?path, "listing handles");
            Ok(())
        }

        Some(Command::BootTimeline { format }) => {
            tracing::info!(%format, "boot timeline");
            Ok(())
        }

        Some(Command::Daemon {
            alert_cpu,
            alert_mem,
        }) => {
            tracing::info!(alert_cpu, alert_mem, "daemon mode");
            Ok(())
        }

        Some(Command::Connections { format, output }) => {
            tracing::info!(%format, ?output, "listing connections");
            Ok(())
        }

        Some(Command::DnsLog { duration, format }) => {
            tracing::info!(duration, %format, "DNS log");
            Ok(())
        }

        Some(Command::Bandwidth { sort, top }) => {
            tracing::info!(%sort, top, "bandwidth");
            Ok(())
        }

        Some(Command::SpeedTest { format }) => {
            tracing::info!(%format, "speed test");
            Ok(())
        }

        Some(Command::Traceroute { host }) => {
            tracing::info!(%host, "traceroute");
            Ok(())
        }

        Some(Command::Capture {
            interface,
            filter,
            output,
            duration,
        }) => {
            tracing::info!(%interface, ?filter, ?output, ?duration, "packet capture");
            Ok(())
        }

        Some(Command::FirewallBlock { address }) => {
            tracing::info!(%address, "firewall block");
            Ok(())
        }

        Some(Command::NetUsage {
            period,
            format,
            output,
        }) => {
            tracing::info!(%period, %format, ?output, "net usage");
            Ok(())
        }

        Some(Command::PowerSummary) => {
            tracing::info!("power summary");
            Ok(())
        }

        Some(Command::BatteryReport { format, output }) => {
            tracing::info!(%format, ?output, "battery report");
            Ok(())
        }

        Some(Command::Thermals { format }) => {
            tracing::info!(%format, "thermals");
            Ok(())
        }

        Some(Command::EnergyTop { top }) => {
            tracing::info!(top, "energy top");
            Ok(())
        }

        Some(Command::PowerProfile { name }) => {
            tracing::info!(%name, "power profile");
            Ok(())
        }

        Some(Command::CarbonReport { period }) => {
            tracing::info!(%period, "carbon report");
            Ok(())
        }

        Some(Command::AudioDevices { format }) => {
            tracing::info!(%format, "audio devices");
            Ok(())
        }

        Some(Command::AudioStreams) => {
            tracing::info!("audio streams");
            Ok(())
        }

        Some(Command::AudioDefaultOutput { name }) => {
            tracing::info!(%name, "set default output");
            Ok(())
        }

        Some(Command::AudioVolume { level, device }) => {
            tracing::info!(level, %device, "set volume");
            Ok(())
        }

        Some(Command::AudioDiag { test }) => {
            tracing::info!(%test, "audio diagnostics");
            Ok(())
        }

        Some(Command::AudioRecord {
            device,
            duration,
            output,
        }) => {
            tracing::info!(%device, duration, ?output, "audio record");
            Ok(())
        }

        Some(Command::EventLog {
            source,
            level,
            limit,
            format,
            output,
        }) => {
            tracing::info!(%source, %level, limit, %format, ?output, "event log");
            Ok(())
        }

        Some(Command::EventStats) => {
            tracing::info!("event stats");
            Ok(())
        }

        Some(Command::EventClear { source }) => {
            tracing::info!(%source, "event clear");
            Ok(())
        }

        Some(Command::EventExport {
            format,
            output,
            source,
            hours,
        }) => {
            tracing::info!(%format, ?output, ?source, ?hours, "event export");
            Ok(())
        }
    }
}
