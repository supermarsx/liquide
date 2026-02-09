use anyhow::{bail, Result};
use clap::Parser;
use tracing::info;

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
    /// Available suites: compositor, encoder, protocol, all.
    #[arg(long, default_value = "all")]
    suite: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    run(cli)
}

fn run(cli: Cli) -> Result<()> {
    info!(suite = %cli.suite, "Starting liquide-bench");

    match cli.suite.as_str() {
        "compositor" => run_compositor_bench(),
        "encoder" => run_encoder_bench(),
        "protocol" => run_protocol_bench(),
        "all" => {
            run_compositor_bench()?;
            run_encoder_bench()?;
            run_protocol_bench()?;
            Ok(())
        }
        other => bail!("Unknown benchmark suite: {other}"),
    }
}

fn run_compositor_bench() -> Result<()> {
    info!("Running compositor benchmark suite...");
    // TODO: Render N frames through the CPU renderer and measure throughput.
    println!("compositor: (not yet implemented)");
    Ok(())
}

fn run_encoder_bench() -> Result<()> {
    info!("Running encoder benchmark suite...");
    // TODO: Encode N frames and measure compression ratio and latency.
    println!("encoder: (not yet implemented)");
    Ok(())
}

fn run_protocol_bench() -> Result<()> {
    info!("Running protocol benchmark suite...");
    // TODO: Serialize/deserialize N protocol messages and measure throughput.
    println!("protocol: (not yet implemented)");
    Ok(())
}
