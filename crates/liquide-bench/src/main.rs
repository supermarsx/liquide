use anyhow::Result;
use clap::Parser;
use tracing::info;

use liquide_bench::config::SuiteSelection;
use liquide_bench::{BenchConfig, BenchRunner};

/// Benchmark harness for Liquide subsystems.
///
/// `liquide-bench` runs targeted performance benchmarks against the
/// compositor, encoder, and protocol layers, reporting throughput and
/// latency metrics.
#[derive(Parser, Debug)]
#[command(name = "liquide-bench", version, about)]
struct Cli {
    /// Name of the benchmark suite to run.
    ///
    /// Available suites: compositor, encoder, protocol, all, ci-quick, ci-full.
    #[arg(long, default_value = "all")]
    suite: String,

    /// Network profile to simulate.
    ///
    /// Available profiles: lan, datacenter, wan-good, wan-cross, 4g, 3g,
    /// hotel-wifi, satellite.
    #[arg(long, default_value = "lan")]
    network: String,

    /// Path to write the JSON report.
    #[arg(long)]
    output: Option<String>,

    /// Duration in seconds for sustained benchmarks.
    #[arg(long, default_value = "30")]
    duration: u64,

    /// Warmup period in seconds (excluded from measurements).
    #[arg(long, default_value = "5")]
    warmup: u64,

    /// Number of iterations for each micro-benchmark.
    #[arg(long, default_value = "100")]
    iterations: u32,

    /// Enable verbose logging.
    #[arg(long)]
    verbose: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let filter = if cli.verbose {
        "debug"
    } else {
        "info"
    };

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(filter)),
        )
        .init();

    run(cli)
}

fn run(cli: Cli) -> Result<()> {
    info!(suite = %cli.suite, network = %cli.network, "Starting liquide-bench");

    let suite = SuiteSelection::from_name(&cli.suite)
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let config = BenchConfig {
        suite,
        network_profile: cli.network,
        output_path: cli.output.clone(),
        duration_secs: cli.duration,
        warmup_secs: cli.warmup,
        iterations: cli.iterations,
        verbose: cli.verbose,
    };

    let runner = BenchRunner::new(config);
    let report = runner.run().map_err(|e| anyhow::anyhow!("{e}"))?;

    println!("{}", report.summary_text());

    if let Some(ref output_path) = cli.output {
        let json = report.to_json().map_err(|e| anyhow::anyhow!("{e}"))?;
        std::fs::write(output_path, json)?;
        info!(path = %output_path, "Report written to file");
    }

    if !report.all_passed() {
        anyhow::bail!(
            "Benchmark FAILED: {} SLO violation(s)",
            report.violation_count()
        );
    }

    Ok(())
}
