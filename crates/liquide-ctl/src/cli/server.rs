use clap::{Parser, Subcommand};

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

// ── completions ─────────────────────────────────────────────────────────────

#[derive(Debug, Parser)]
pub struct CompletionsArgs {
    /// Shell to generate completions for.
    pub shell: clap_complete::Shell,
}
