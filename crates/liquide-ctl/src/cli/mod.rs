mod packages;
mod plugins;
mod security;
mod server;
mod sessions;
mod streaming;

pub use packages::*;
pub use plugins::*;
pub use security::*;
pub use server::*;
pub use sessions::*;
pub use streaming::*;

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
