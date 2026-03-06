use clap::{Parser, Subcommand};

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
