use anyhow::{Result, bail};
use clap::{Parser, ValueEnum};
use tracing::info;

use liquide_conformance::config::{ConformanceConfig, ConformanceMode};
use liquide_conformance::report::ConformanceStatus;
use liquide_conformance::runner::ConformanceRunner;
use liquide_conformance::suite::SuiteName;

/// Protocol conformance test runner for Liquide.
///
/// `liquide-conformance` can run offline protocol validators or, once live
/// probing is implemented, exercise a running Liquide server. Offline reports
/// are intentionally indeterminate and never certify server conformance.
#[derive(Parser, Debug)]
#[command(name = "liquide-conformance", version, about)]
struct Cli {
    /// Evidence mode to run.
    #[arg(long, value_enum, default_value = "offline")]
    mode: CliMode,

    /// Server address in the form `host:port`.
    #[arg(long, default_value = "localhost:3389")]
    server: String,

    /// Test suite to run.
    ///
    /// Available suites: handshake, auth, streaming, clipboard, security, all.
    #[arg(long, default_value = "all")]
    suite: String,

    /// Username for authentication during testing.
    #[arg(long)]
    username: Option<String>,

    /// Password for authentication during testing.
    #[arg(long)]
    password: Option<String>,

    /// Per-test timeout in milliseconds.
    #[arg(long, default_value = "5000")]
    timeout: u64,

    /// Output path for the JSON report.
    #[arg(long)]
    output: Option<String>,

    /// Enable verbose output.
    #[arg(long)]
    verbose: bool,

    /// List all test cases without running them.
    #[arg(long)]
    list: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum CliMode {
    /// Run local protocol validators without contacting the target server.
    Offline,
    /// Run live server conformance probes. Currently fails closed until implemented.
    Live,
}

impl From<CliMode> for ConformanceMode {
    fn from(value: CliMode) -> Self {
        match value {
            CliMode::Offline => Self::OfflineValidation,
            CliMode::Live => Self::LiveServer,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let suite = SuiteName::from_name(&cli.suite)
        .ok_or_else(|| anyhow::anyhow!("unknown suite: {}", cli.suite))?;

    let config = ConformanceConfig {
        mode: cli.mode.into(),
        server: cli.server,
        suite,
        username: cli.username,
        password: cli.password,
        timeout_ms: cli.timeout,
        verbose: cli.verbose,
        output: cli.output.clone(),
    };

    let mode = config.mode;
    let runner = ConformanceRunner::new(config);

    if cli.list {
        println!("Conformance test cases ({}):", runner.case_count());
        for id in runner.case_ids() {
            println!("  {id}");
        }
        return Ok(());
    }

    info!(
        mode = %mode.label(),
        suite = %suite,
        cases = runner.case_count(),
        "Starting conformance run"
    );

    let report = runner.run();

    println!("{}", report.summary());

    if let Some(output) = &cli.output {
        let json = report.to_json()?;
        std::fs::write(output, json)?;
        info!(path = %output, "Report written");
    }

    match report.status() {
        ConformanceStatus::Conformant => {}
        ConformanceStatus::NonConformant => {
            bail!("{} conformance test(s) failed", report.total_failed());
        }
        ConformanceStatus::Indeterminate => {
            bail!(
                "conformance result indeterminate: {}",
                report.status_reason()
            );
        }
    }

    Ok(())
}
