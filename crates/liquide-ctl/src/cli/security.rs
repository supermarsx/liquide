use clap::{Parser, Subcommand};

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
