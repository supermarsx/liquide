use clap::{Parser, Subcommand, ValueEnum};

/// liquidctl — Unified CLI for administering, monitoring, and troubleshooting LiquiDE servers.
#[derive(Debug, Parser)]
#[command(
    name = "liquidctl",
    version,
    about = "Unified CLI for LiquiDE server administration",
    long_about = "liquidctl is the unified command-line tool for administering, monitoring, \
                  and troubleshooting LiquiDE servers. It communicates with the LiquiDE server \
                  daemon via a local Unix socket or a remote API endpoint.",
    after_help = "Use 'liquidctl <command> --help' for more information about a specific command."
)]
pub struct Cli {
    /// Server address (default: local Unix socket).
    /// Use @profile to connect to a named remote profile from config.
    #[arg(long, global = true, env = "LIQUIDCTL_SERVER")]
    pub server: Option<String>,

    /// API key for remote authentication.
    #[arg(long, global = true, env = "LIQUIDCTL_API_KEY")]
    pub api_key: Option<String>,

    /// Output format.
    #[arg(long, global = true, default_value = "text", env = "LIQUIDCTL_FORMAT")]
    pub format: OutputFormat,

    /// Colorize output.
    #[arg(long, global = true, default_value = "auto")]
    pub color: ColorWhen,

    /// Suppress non-essential output.
    #[arg(long, global = true)]
    pub quiet: bool,

    /// Increase output verbosity.
    #[arg(long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
    Csv,
    Table,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum ColorWhen {
    Auto,
    Always,
    Never,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Display overall server status.
    Status(StatusArgs),

    /// Manage active sessions.
    #[command(subcommand)]
    Sessions(SessionsCommand),

    /// Manage connected users.
    #[command(subcommand)]
    Users(UsersCommand),

    /// Display real-time stream statistics.
    Stats(StatsArgs),

    /// Run performance benchmarks.
    Benchmark(BenchmarkArgs),

    /// Configuration management.
    #[command(subcommand)]
    Config(ConfigCommand),

    /// Policy management.
    #[command(subcommand)]
    Policy(PolicyCommand),

    /// Manage virtual monitors.
    #[command(subcommand)]
    Monitors(MonitorsCommand),

    /// Manage transport settings and view transport status.
    #[command(subcommand)]
    Transport(TransportCommand),

    /// Manage audio subsystem.
    #[command(subcommand)]
    Audio(AudioCommand),

    /// Manage encoders.
    #[command(subcommand)]
    Encoder(EncoderCommand),

    /// Manage USB device forwarding.
    #[command(subcommand)]
    Usb(UsbCommand),

    /// View and manage logs.
    #[command(subcommand)]
    Logs(LogsCommand),

    /// View audit events.
    #[command(subcommand)]
    Audit(AuditCommand),

    /// Monitor and manage the honeypot/tarpit system.
    #[command(subcommand)]
    Honeypot(HoneypotCommand),

    /// Manage session locks.
    Lock(LockArgs),

    /// Unlock a locked session.
    Unlock(UnlockArgs),

    /// Manage gateway connection.
    #[command(subcommand)]
    Gateway(GatewayCommand),

    /// Manage the LiquiDE service.
    #[command(subcommand)]
    Service(ServiceCommand),

    /// Manage rendering caches.
    #[command(subcommand)]
    Cache(CacheCommand),

    /// Manage RDP compatibility layer.
    #[command(subcommand)]
    Rdp(RdpCommand),

    /// WASM plugin management.
    #[command(subcommand)]
    Plugins(PluginsCommand),

    /// Crash report management.
    #[command(subcommand)]
    Crash(CrashCommand),

    /// Session supervisor management.
    #[command(subcommand)]
    Supervisor(SupervisorCommand),

    /// Flatpak application management.
    #[command(subcommand)]
    Flatpak(FlatpakCommand),

    /// Homebrew package management.
    #[command(subcommand)]
    Brew(BrewCommand),

    /// Snap package management.
    #[command(subcommand)]
    Snap(SnapCommand),

    /// Nix package management.
    #[command(subcommand)]
    Nix(NixCommand),

    /// AppImage management.
    #[command(subcommand)]
    Appimage(AppimageCommand),

    /// Generate shell completions.
    Completions(CompletionsArgs),
}

// ── status ──────────────────────────────────────────────────────────────────

#[derive(Debug, Parser)]
pub struct StatusArgs {
    /// Continuous refresh.
    #[arg(long)]
    pub watch: bool,

    /// Custom refresh interval in seconds (default: 2).
    #[arg(long, default_value = "2")]
    pub watch_interval: u64,
}

// ── sessions ────────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum SessionsCommand {
    /// List all active sessions.
    List(SessionsListArgs),
    /// Show detailed session information.
    Show(SessionsShowArgs),
    /// Disconnect a session.
    Disconnect(SessionsDisconnectArgs),
    /// Disconnect all sessions.
    DisconnectAll(SessionsDisconnectAllArgs),
}

#[derive(Debug, Parser)]
pub struct SessionsListArgs {
    /// Filter by user.
    #[arg(long)]
    pub user: Option<String>,
    /// Sort by column.
    #[arg(long)]
    pub sort: Option<String>,
    /// Live updating.
    #[arg(long)]
    pub watch: bool,
}

#[derive(Debug, Parser)]
pub struct SessionsShowArgs {
    /// Session ID.
    pub session_id: String,
}

#[derive(Debug, Parser)]
pub struct SessionsDisconnectArgs {
    /// Session ID.
    pub session_id: String,
    /// Skip interactive confirmation.
    #[arg(long)]
    pub confirm: bool,
    /// Send a message to the user before disconnecting.
    #[arg(long)]
    pub message: Option<String>,
}

#[derive(Debug, Parser)]
pub struct SessionsDisconnectAllArgs {
    /// Disconnect only sessions for a specific user.
    #[arg(long)]
    pub user: Option<String>,
    /// Skip interactive confirmation.
    #[arg(long)]
    pub confirm: bool,
    /// Stop accepting new sessions and wait for existing to end gracefully.
    #[arg(long)]
    pub drain: bool,
}

// ── users ───────────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum UsersCommand {
    /// List connected users.
    List,
    /// Detailed user information.
    Show(UsersShowArgs),
    /// Disconnect all sessions for a user.
    Kick(UsersKickArgs),
    /// Manage user avatars.
    #[command(subcommand)]
    Avatar(UsersAvatarCommand),
}

#[derive(Debug, Parser)]
pub struct UsersShowArgs {
    /// Username.
    pub username: String,
}

#[derive(Debug, Parser)]
pub struct UsersKickArgs {
    /// Username.
    pub username: String,
}

#[derive(Debug, Subcommand)]
pub enum UsersAvatarCommand {
    /// Set or replace a user's avatar image.
    Set(UsersAvatarSetArgs),
    /// Remove a user's avatar.
    Remove(UsersAvatarRemoveArgs),
    /// Display avatar metadata.
    Show(UsersAvatarShowArgs),
}

#[derive(Debug, Parser)]
pub struct UsersAvatarSetArgs {
    /// Username.
    pub username: String,
    /// Path to avatar image file.
    pub path: String,
}

#[derive(Debug, Parser)]
pub struct UsersAvatarRemoveArgs {
    pub username: String,
}

#[derive(Debug, Parser)]
pub struct UsersAvatarShowArgs {
    pub username: String,
}

// ── stats ───────────────────────────────────────────────────────────────────

#[derive(Debug, Parser)]
pub struct StatsArgs {
    /// Show stats for a specific session.
    #[arg(long)]
    pub session: Option<String>,
    /// Live updating.
    #[arg(long)]
    pub watch: bool,
    /// Update interval in milliseconds (default: 1000).
    #[arg(long, default_value = "1000")]
    pub interval: u64,
}

// ── benchmark ───────────────────────────────────────────────────────────────

#[derive(Debug, Parser)]
pub struct BenchmarkArgs {
    /// Abbreviated benchmark (compositing + top encoder only).
    #[arg(long)]
    pub quick: bool,
    /// Full benchmark (all encoders, all blur modes, all tile codecs).
    #[arg(long)]
    pub full: bool,
    /// Save results to file for later comparison.
    #[arg(long)]
    pub save: bool,
}

// ── config ──────────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Display current server configuration.
    Show(ConfigShowArgs),
    /// Validate configuration files.
    Validate,
    /// Set a configuration value.
    Set(ConfigSetArgs),
    /// Show differences between running config and on-disk config.
    Diff,
    /// Export current config to stdout.
    Export,
    /// Import and apply a configuration file.
    Import(ConfigImportArgs),
}

#[derive(Debug, Parser)]
pub struct ConfigShowArgs {
    /// Show only a specific section.
    #[arg(long)]
    pub section: Option<String>,
    /// Show without redacting secrets (requires admin).
    #[arg(long)]
    pub raw: bool,
    /// Show default values for all settings.
    #[arg(long)]
    pub defaults: bool,
}

#[derive(Debug, Parser)]
pub struct ConfigSetArgs {
    /// Configuration key (e.g. performance.active_fps).
    pub key: String,
    /// New value.
    pub value: String,
    /// Write to file but don't hot-reload.
    #[arg(long)]
    pub no_reload: bool,
}

#[derive(Debug, Parser)]
pub struct ConfigImportArgs {
    /// Path to config file to import.
    pub file: String,
}

// ── policy ──────────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum PolicyCommand {
    /// Display current policies.
    Show,
    /// Set a policy value.
    Set(PolicySetArgs),
    /// Show effective policy for a specific user.
    Effective(PolicyEffectiveArgs),
}

#[derive(Debug, Parser)]
pub struct PolicySetArgs {
    /// Policy scope (e.g. group.guests).
    pub scope: String,
    /// Policy key.
    pub key: String,
    /// New value.
    pub value: String,
}

#[derive(Debug, Parser)]
pub struct PolicyEffectiveArgs {
    /// Username.
    pub username: String,
}

// ── monitors ────────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum MonitorsCommand {
    /// List virtual monitors for a session.
    List(MonitorsListArgs),
    /// Add a virtual monitor to a session.
    Add(MonitorsAddArgs),
    /// Remove a virtual monitor from a session.
    Remove(MonitorsRemoveArgs),
    /// Resize a virtual monitor.
    Resize(MonitorsResizeArgs),
}

#[derive(Debug, Parser)]
pub struct MonitorsListArgs {
    /// Session ID.
    #[arg(long)]
    pub session: String,
}

#[derive(Debug, Parser)]
pub struct MonitorsAddArgs {
    /// Session ID.
    pub session_id: String,
    /// Resolution (e.g. 1920x1080).
    #[arg(long)]
    pub resolution: Option<String>,
    /// DPI.
    #[arg(long)]
    pub dpi: Option<u32>,
}

#[derive(Debug, Parser)]
pub struct MonitorsRemoveArgs {
    /// Session ID.
    pub session_id: String,
    /// Monitor ID.
    pub monitor_id: String,
}

#[derive(Debug, Parser)]
pub struct MonitorsResizeArgs {
    /// Session ID.
    pub session_id: String,
    /// Monitor ID.
    pub monitor_id: String,
    /// New resolution (e.g. 1920x1080).
    pub resolution: String,
}

// ── transport ───────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum TransportCommand {
    /// Show transport status.
    Status,
    /// Force a session to switch transport.
    Switch(TransportSwitchArgs),
}

#[derive(Debug, Parser)]
pub struct TransportSwitchArgs {
    /// Session ID.
    pub session_id: String,
    /// Target transport (e.g. quic, tls-tcp, udp, websocket).
    pub transport: String,
}

// ── audio ───────────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum AudioCommand {
    /// Show audio subsystem status.
    Status,
}

// ── encoder ─────────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum EncoderCommand {
    /// List available encoders.
    List,
    /// Benchmark a specific encoder.
    Benchmark(EncoderBenchmarkArgs),
}

#[derive(Debug, Parser)]
pub struct EncoderBenchmarkArgs {
    /// Encoder name.
    pub encoder: String,
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

// ── logs ────────────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum LogsCommand {
    /// Stream live logs.
    Tail(LogsTailArgs),
    /// Search historical logs.
    Search(LogsSearchArgs),
    /// View and modify per-subsystem log levels.
    Config,
    /// Change log level for a subsystem at runtime.
    Level(LogsLevelArgs),
    /// Force log rotation.
    Rotate(LogsRotateArgs),
}

#[derive(Debug, Parser)]
pub struct LogsTailArgs {
    /// Filter by log level.
    #[arg(long)]
    pub level: Option<String>,
    /// Filter by session.
    #[arg(long)]
    pub session: Option<String>,
    /// Filter by subsystem.
    #[arg(long)]
    pub subsystem: Option<String>,
    /// Show logs since a time.
    #[arg(long)]
    pub since: Option<String>,
    /// Stay attached and stream new logs.
    #[arg(long)]
    pub follow: bool,
}

#[derive(Debug, Parser)]
pub struct LogsSearchArgs {
    /// Search pattern.
    pub pattern: String,
    /// Filter by subsystem.
    #[arg(long)]
    pub subsystem: Option<String>,
    /// Filter by session correlation ID.
    #[arg(long)]
    pub session: Option<String>,
    /// Time range start.
    #[arg(long)]
    pub since: Option<String>,
    /// Time range end.
    #[arg(long)]
    pub until: Option<String>,
    /// Max entries.
    #[arg(long)]
    pub limit: Option<usize>,
}

#[derive(Debug, Parser)]
pub struct LogsLevelArgs {
    /// Subsystem name.
    pub subsystem: String,
    /// Log level.
    pub level: String,
}

#[derive(Debug, Parser)]
pub struct LogsRotateArgs {
    /// Rotate only a specific subsystem.
    #[arg(long)]
    pub subsystem: Option<String>,
}

// ── audit ───────────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum AuditCommand {
    /// List audit events.
    List(AuditListArgs),
}

#[derive(Debug, Parser)]
pub struct AuditListArgs {
    /// Filter by event type.
    #[arg(long)]
    pub event: Option<String>,
    /// Filter by user.
    #[arg(long)]
    pub user: Option<String>,
    /// Time range.
    #[arg(long)]
    pub since: Option<String>,
    /// Max entries.
    #[arg(long)]
    pub limit: Option<usize>,
}

// ── honeypot ────────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum HoneypotCommand {
    /// Show honeypot/tarpit status.
    Status(HoneypotStatusArgs),
    /// List active tarpit and honeypot connections.
    List,
    /// Release a tarpit/honeypot connection.
    Drop(HoneypotDropArgs),
    /// Release all active tarpit/honeypot connections.
    DropAll,
    /// List or export collected indicators of compromise.
    Iocs(HoneypotIocsArgs),
    /// Show enabled triggers and their thresholds.
    Triggers,
}

#[derive(Debug, Parser)]
pub struct HoneypotStatusArgs {
    /// Live updating.
    #[arg(long)]
    pub watch: bool,
}

#[derive(Debug, Parser)]
pub struct HoneypotDropArgs {
    /// Connection ID.
    pub connection_id: String,
}

#[derive(Debug, Parser)]
pub struct HoneypotIocsArgs {
    /// Time range.
    #[arg(long)]
    pub since: Option<String>,
    /// Export to file.
    #[arg(long)]
    pub export: Option<String>,
}

// ── lock / unlock ───────────────────────────────────────────────────────────

#[derive(Debug, Parser)]
pub struct LockArgs {
    /// Session ID, or "all" to lock all sessions, or "status" to show lock state,
    /// or "policy <username>" to show lock policy, or "config" to show config.
    pub target: String,

    /// Skip interactive confirmation.
    #[arg(long)]
    pub confirm: bool,

    /// Custom message on the lock screen.
    #[arg(long)]
    pub message: Option<String>,

    /// Reason for lock (logged in audit).
    #[arg(long)]
    pub reason: Option<String>,
}

#[derive(Debug, Parser)]
pub struct UnlockArgs {
    /// Session ID.
    pub session_id: String,
    /// Skip interactive confirmation.
    #[arg(long)]
    pub confirm: bool,
}

// ── gateway ─────────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum GatewayCommand {
    /// Show gateway status.
    Status,
    /// Manually trigger gateway registration.
    Register,
    /// Deregister from gateway.
    Deregister,
}

// ── service ─────────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum ServiceCommand {
    /// Service health check.
    Status,
    /// Restart the server daemon.
    Restart,
    /// Stop the server daemon.
    Stop(ServiceStopArgs),
}

#[derive(Debug, Parser)]
pub struct ServiceStopArgs {
    /// Stop accepting new sessions, wait for existing to end.
    #[arg(long)]
    pub drain: bool,
    /// Immediate stop, disconnect all sessions.
    #[arg(long)]
    pub force: bool,
    /// Drain timeout in seconds (default: 300).
    #[arg(long, default_value = "300")]
    pub timeout: u64,
}

// ── cache ───────────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum CacheCommand {
    /// Show cache status.
    Status,
    /// Clear caches.
    Clear(CacheClearArgs),
}

#[derive(Debug, Parser)]
pub struct CacheClearArgs {
    /// Cache type: blur, wallpaper, partial, font, all.
    #[arg(long, name = "type")]
    pub cache_type: Option<String>,
    /// Clear caches for a specific session.
    #[arg(long)]
    pub session: Option<String>,
}

// ── rdp ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum RdpCommand {
    /// Show RDP compatibility status.
    Status,
    /// Enable RDP compatibility.
    Enable,
    /// Disable RDP compatibility.
    Disable,
}

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

// ── flatpak ─────────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum FlatpakCommand {
    /// Search Flathub for applications.
    Search(FlatpakSearchArgs),
    /// Install a Flatpak application.
    Install(FlatpakInstallArgs),
    /// Remove a Flatpak application.
    Remove(FlatpakRemoveArgs),
    /// List installed Flatpak applications.
    List(FlatpakListArgs),
    /// Update Flatpak applications.
    Update(FlatpakUpdateArgs),
    /// Show effective permissions.
    Permissions(FlatpakPermissionsArgs),
    /// Set permission overrides.
    Override(FlatpakOverrideArgs),
    /// List configured remotes.
    RemoteList,
    /// Add a remote repository.
    RemoteAdd(FlatpakRemoteAddArgs),
    /// Remove a remote.
    RemoteRemove(FlatpakRemoteRemoveArgs),
    /// Rollback to previous commit.
    Rollback(FlatpakRollbackArgs),
    /// Show version/commit history.
    History(FlatpakHistoryArgs),
    /// Garbage-collect unused data.
    Gc(FlatpakGcArgs),
}

#[derive(Debug, Parser)]
pub struct FlatpakSearchArgs {
    pub query: String,
    #[arg(long)]
    pub remote: Option<String>,
}

#[derive(Debug, Parser)]
pub struct FlatpakInstallArgs {
    pub app_id: String,
    #[arg(long)]
    pub user: bool,
    #[arg(long)]
    pub system: bool,
    #[arg(long)]
    pub noninteractive: bool,
    #[arg(long)]
    pub no_deps: bool,
}

#[derive(Debug, Parser)]
pub struct FlatpakRemoveArgs {
    pub app_id: String,
    #[arg(long)]
    pub user: bool,
    #[arg(long)]
    pub system: bool,
    #[arg(long)]
    pub delete_data: bool,
    #[arg(long)]
    pub noninteractive: bool,
}

#[derive(Debug, Parser)]
pub struct FlatpakListArgs {
    #[arg(long)]
    pub user: bool,
    #[arg(long)]
    pub system: bool,
    #[arg(long)]
    pub all: bool,
    #[arg(long)]
    pub runtimes: bool,
    #[arg(long)]
    pub columns: Option<String>,
}

#[derive(Debug, Parser)]
pub struct FlatpakUpdateArgs {
    pub app_id: Option<String>,
    #[arg(long)]
    pub user: bool,
    #[arg(long)]
    pub system: bool,
    #[arg(long)]
    pub check: bool,
    #[arg(long)]
    pub noninteractive: bool,
}

#[derive(Debug, Parser)]
pub struct FlatpakPermissionsArgs {
    pub app_id: String,
}

#[derive(Debug, Parser)]
pub struct FlatpakOverrideArgs {
    pub app_id: String,
    #[arg(long)]
    pub filesystem: Option<String>,
    #[arg(long)]
    pub nofilesystem: Option<String>,
    #[arg(long)]
    pub socket: Option<String>,
    #[arg(long)]
    pub nosocket: Option<String>,
    #[arg(long)]
    pub device: Option<String>,
    #[arg(long)]
    pub nodevice: Option<String>,
    #[arg(long)]
    pub share: Option<String>,
    #[arg(long)]
    pub unshare: Option<String>,
    #[arg(long)]
    pub talk_name: Option<String>,
    #[arg(long)]
    pub no_talk_name: Option<String>,
    #[arg(long)]
    pub reset: bool,
    #[arg(long, name = "no-network")]
    pub no_network: bool,
}

#[derive(Debug, Parser)]
pub struct FlatpakRemoteAddArgs {
    pub name: String,
    pub url: String,
    #[arg(long)]
    pub user: bool,
    #[arg(long)]
    pub system: bool,
}

#[derive(Debug, Parser)]
pub struct FlatpakRemoteRemoveArgs {
    pub name: String,
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Parser)]
pub struct FlatpakRollbackArgs {
    pub app_id: String,
}

#[derive(Debug, Parser)]
pub struct FlatpakHistoryArgs {
    pub app_id: String,
    #[arg(long)]
    pub limit: Option<usize>,
}

#[derive(Debug, Parser)]
pub struct FlatpakGcArgs {
    #[arg(long)]
    pub unused_runtimes: bool,
    #[arg(long)]
    pub dry_run: bool,
}

// ── brew ────────────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum BrewCommand {
    /// Search Homebrew for formulae and casks.
    Search(BrewSearchArgs),
    /// Install a Homebrew formula or cask.
    Install(BrewInstallArgs),
    /// Remove a Homebrew formula or cask.
    Remove(BrewRemoveArgs),
    /// List installed Homebrew packages.
    List(BrewListArgs),
    /// Update Homebrew packages.
    Update(BrewUpdateArgs),
    /// Show detailed information.
    Info(BrewInfoArgs),
    /// Add a Homebrew tap.
    Tap(BrewTapArgs),
    /// Remove a Homebrew tap.
    Untap(BrewUntapArgs),
    /// Pin a formula.
    Pin(BrewPinArgs),
    /// Unpin a formula.
    Unpin(BrewUnpinArgs),
    /// Rollback to previous version.
    Rollback(BrewRollbackArgs),
}

#[derive(Debug, Parser)]
pub struct BrewSearchArgs {
    pub query: String,
    #[arg(long)]
    pub formula: bool,
    #[arg(long)]
    pub cask: bool,
}

#[derive(Debug, Parser)]
pub struct BrewInstallArgs {
    pub package: String,
    #[arg(long)]
    pub cask: bool,
    #[arg(long)]
    pub formula: bool,
}

#[derive(Debug, Parser)]
pub struct BrewRemoveArgs {
    pub package: String,
    #[arg(long)]
    pub cask: bool,
    #[arg(long)]
    pub formula: bool,
}

#[derive(Debug, Parser)]
pub struct BrewListArgs {
    #[arg(long)]
    pub formula: bool,
    #[arg(long)]
    pub cask: bool,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Parser)]
pub struct BrewUpdateArgs {
    pub package: Option<String>,
    #[arg(long)]
    pub check: bool,
    #[arg(long)]
    pub cask: bool,
    #[arg(long)]
    pub formula: bool,
}

#[derive(Debug, Parser)]
pub struct BrewInfoArgs {
    pub package: String,
}

#[derive(Debug, Parser)]
pub struct BrewTapArgs {
    pub tap_name: String,
}

#[derive(Debug, Parser)]
pub struct BrewUntapArgs {
    pub tap_name: String,
}

#[derive(Debug, Parser)]
pub struct BrewPinArgs {
    pub formula: String,
}

#[derive(Debug, Parser)]
pub struct BrewUnpinArgs {
    pub formula: String,
}

#[derive(Debug, Parser)]
pub struct BrewRollbackArgs {
    pub package: String,
}

// ── snap ────────────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum SnapCommand {
    /// Search the Snap Store.
    Search(SnapSearchArgs),
    /// Install a snap package.
    Install(SnapInstallArgs),
    /// Remove a snap package.
    Remove(SnapRemoveArgs),
    /// List installed snaps.
    List(SnapListArgs),
    /// Update snap packages.
    Update(SnapUpdateArgs),
    /// Show detailed snap information.
    Info(SnapInfoArgs),
    /// List interface connections.
    Connections(SnapConnectionsArgs),
    /// Connect a snap interface plug.
    Connect(SnapConnectArgs),
    /// Disconnect a snap interface plug.
    Disconnect(SnapDisconnectArgs),
    /// Revert to previous revision.
    Revert(SnapRevertArgs),
    /// Hold automatic snap refreshes.
    RefreshHold(SnapRefreshHoldArgs),
    /// Show available channels.
    Channels(SnapChannelsArgs),
}

#[derive(Debug, Parser)]
pub struct SnapSearchArgs {
    pub query: String,
}

#[derive(Debug, Parser)]
pub struct SnapInstallArgs {
    pub snap: String,
    #[arg(long)]
    pub channel: Option<String>,
    #[arg(long)]
    pub classic: bool,
    #[arg(long)]
    pub devmode: bool,
}

#[derive(Debug, Parser)]
pub struct SnapRemoveArgs {
    pub snap: String,
    #[arg(long)]
    pub purge: bool,
}

#[derive(Debug, Parser)]
pub struct SnapListArgs {
    #[arg(long)]
    pub all: bool,
}

#[derive(Debug, Parser)]
pub struct SnapUpdateArgs {
    pub snap: Option<String>,
    #[arg(long)]
    pub check: bool,
    #[arg(long)]
    pub channel: Option<String>,
}

#[derive(Debug, Parser)]
pub struct SnapInfoArgs {
    pub snap: String,
}

#[derive(Debug, Parser)]
pub struct SnapConnectionsArgs {
    pub snap: String,
}

#[derive(Debug, Parser)]
pub struct SnapConnectArgs {
    pub snap: String,
    pub interface: String,
}

#[derive(Debug, Parser)]
pub struct SnapDisconnectArgs {
    pub snap: String,
    pub interface: String,
}

#[derive(Debug, Parser)]
pub struct SnapRevertArgs {
    pub snap: String,
}

#[derive(Debug, Parser)]
pub struct SnapRefreshHoldArgs {
    pub snap: String,
    /// Duration in hours.
    #[arg(long)]
    pub duration: u64,
}

#[derive(Debug, Parser)]
pub struct SnapChannelsArgs {
    pub snap: String,
}

// ── nix ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum NixCommand {
    /// Search nixpkgs for packages.
    Search(NixSearchArgs),
    /// Install a Nix package.
    Install(NixInstallArgs),
    /// Remove a Nix package.
    Remove(NixRemoveArgs),
    /// List installed Nix packages.
    List(NixListArgs),
    /// Update Nix packages.
    Update(NixUpdateArgs),
    /// Rollback to previous profile generation.
    Rollback(NixRollbackArgs),
    /// Garbage-collect unused store paths.
    Gc(NixGcArgs),
    /// Enter a Nix development shell.
    Develop(NixDevelopArgs),
}

#[derive(Debug, Parser)]
pub struct NixSearchArgs {
    pub query: String,
    #[arg(long)]
    pub flake: Option<String>,
}

#[derive(Debug, Parser)]
pub struct NixInstallArgs {
    pub package: String,
    #[arg(long)]
    pub profile: Option<String>,
}

#[derive(Debug, Parser)]
pub struct NixRemoveArgs {
    pub package: String,
}

#[derive(Debug, Parser)]
pub struct NixListArgs {
    #[arg(long)]
    pub profile: Option<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Parser)]
pub struct NixUpdateArgs {
    pub package: Option<String>,
    #[arg(long)]
    pub profile: Option<String>,
    #[arg(long)]
    pub check: bool,
}

#[derive(Debug, Parser)]
pub struct NixRollbackArgs {
    #[arg(long)]
    pub profile: Option<String>,
}

#[derive(Debug, Parser)]
pub struct NixGcArgs {
    #[arg(long)]
    pub older_than: Option<u64>,
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Parser)]
pub struct NixDevelopArgs {
    #[arg(long)]
    pub flake: Option<String>,
}

// ── appimage ────────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum AppimageCommand {
    /// List integrated AppImage files.
    List,
    /// Check for and apply AppImage updates.
    Update(AppimageUpdateArgs),
    /// Integrate an AppImage into the desktop.
    Integrate(AppimageIntegrateArgs),
    /// Remove an integrated AppImage.
    Remove(AppimageRemoveArgs),
    /// Verify an AppImage signature.
    Verify(AppimageVerifyArgs),
}

#[derive(Debug, Parser)]
pub struct AppimageUpdateArgs {
    pub app: Option<String>,
    #[arg(long)]
    pub check: bool,
}

#[derive(Debug, Parser)]
pub struct AppimageIntegrateArgs {
    pub file: String,
}

#[derive(Debug, Parser)]
pub struct AppimageRemoveArgs {
    pub app: String,
}

#[derive(Debug, Parser)]
pub struct AppimageVerifyArgs {
    pub file: String,
}

// ── completions ─────────────────────────────────────────────────────────────

#[derive(Debug, Parser)]
pub struct CompletionsArgs {
    /// Shell to generate completions for.
    pub shell: clap_complete::Shell,
}
