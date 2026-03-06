use clap::{Parser, Subcommand};

// ── plugins ─────────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum PluginsCommand {
    /// List installed plugins.
    List(PluginsListArgs),
    /// Show detailed plugin information.
    Info(PluginsInfoArgs),
    /// Install a plugin.
    Install(PluginsInstallArgs),
    /// Remove an installed plugin.
    Uninstall(PluginsUninstallArgs),
    /// Enable a plugin.
    Enable(PluginsEnableArgs),
    /// Disable a plugin.
    Disable(PluginsDisableArgs),
    /// Hot-reload a plugin.
    Reload(PluginsReloadArgs),
    /// Get or set per-plugin configuration.
    Config(PluginsConfigArgs),
}

#[derive(Debug, Parser)]
pub struct PluginsListArgs {
    #[arg(long)]
    pub session: Option<String>,
}

#[derive(Debug, Parser)]
pub struct PluginsInfoArgs {
    pub plugin_id: String,
    #[arg(long)]
    pub session: Option<String>,
}

#[derive(Debug, Parser)]
pub struct PluginsInstallArgs {
    /// Source: local .wasm file, directory, or registry URL.
    pub source: String,
    /// Require Ed25519 signature validation.
    #[arg(long)]
    pub signature_check: bool,
    /// Validate without installing.
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Parser)]
pub struct PluginsUninstallArgs {
    pub plugin_id: String,
    /// Remove plugin configuration and stored data.
    #[arg(long)]
    pub purge: bool,
}

#[derive(Debug, Parser)]
pub struct PluginsEnableArgs {
    pub plugin_id: String,
    #[arg(long)]
    pub session: Option<String>,
}

#[derive(Debug, Parser)]
pub struct PluginsDisableArgs {
    pub plugin_id: String,
    #[arg(long)]
    pub session: Option<String>,
}

#[derive(Debug, Parser)]
pub struct PluginsReloadArgs {
    pub plugin_id: String,
    #[arg(long)]
    pub session: Option<String>,
}

#[derive(Debug, Parser)]
pub struct PluginsConfigArgs {
    pub plugin_id: String,
    /// Config key.
    pub key: Option<String>,
    /// Config value (set mode).
    pub value: Option<String>,
}

// ── usb ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum UsbCommand {
    /// Show USB/IP subsystem status.
    Status,
    /// List USB devices forwarded in a session.
    List(UsbListArgs),
    /// Disconnect a forwarded USB device.
    Disconnect(UsbDisconnectArgs),
    /// Disconnect all forwarded USB devices from a session.
    DisconnectAll(UsbDisconnectAllArgs),
}

#[derive(Debug, Parser)]
pub struct UsbListArgs {
    /// Session ID.
    #[arg(long)]
    pub session: String,
}

#[derive(Debug, Parser)]
pub struct UsbDisconnectArgs {
    /// Session ID.
    pub session_id: String,
    /// Device ID.
    pub device_id: String,
    /// Skip interactive confirmation.
    #[arg(long)]
    pub confirm: bool,
    /// Force disconnect without safe eject.
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Parser)]
pub struct UsbDisconnectAllArgs {
    /// Session ID.
    pub session_id: String,
    /// Skip interactive confirmation.
    #[arg(long)]
    pub confirm: bool,
}

// ── crash ───────────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum CrashCommand {
    /// List crash reports.
    List(CrashListArgs),
    /// Show full crash report details.
    Show(CrashShowArgs),
    /// Export a crash report to a file.
    Export(CrashExportArgs),
    /// Delete crash reports.
    Delete(CrashDeleteArgs),
    /// Show crash statistics.
    Stats(CrashStatsArgs),
}

#[derive(Debug, Parser)]
pub struct CrashListArgs {
    #[arg(long)]
    pub limit: Option<usize>,
    #[arg(long)]
    pub session: Option<String>,
    #[arg(long)]
    pub since: Option<String>,
}

#[derive(Debug, Parser)]
pub struct CrashShowArgs {
    pub report_id: String,
}

#[derive(Debug, Parser)]
pub struct CrashExportArgs {
    pub report_id: String,
    #[arg(long)]
    pub output: Option<String>,
    /// Bundle coredump in .tar.gz archive.
    #[arg(long)]
    pub include_coredump: bool,
}

#[derive(Debug, Parser)]
pub struct CrashDeleteArgs {
    pub report_id: Option<String>,
    /// Delete all reports.
    #[arg(long)]
    pub all: bool,
    /// Delete reports older than N days.
    #[arg(long)]
    pub older_than: Option<u64>,
}

#[derive(Debug, Parser)]
pub struct CrashStatsArgs {
    #[arg(long)]
    pub since: Option<String>,
}

// ── supervisor ──────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum SupervisorCommand {
    /// Show supervisor status.
    Status,
    /// Restart a session process.
    Restart(SupervisorRestartArgs),
    /// Reset the restart counter for a session.
    ResetRestarts(SupervisorResetRestartsArgs),
    /// View supervisor logs.
    Logs(SupervisorLogsArgs),
}

#[derive(Debug, Parser)]
pub struct SupervisorRestartArgs {
    pub session_id: String,
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Parser)]
pub struct SupervisorResetRestartsArgs {
    pub session_id: String,
}

#[derive(Debug, Parser)]
pub struct SupervisorLogsArgs {
    #[arg(long)]
    pub session: Option<String>,
    #[arg(long)]
    pub lines: Option<usize>,
    #[arg(long)]
    pub follow: bool,
}
