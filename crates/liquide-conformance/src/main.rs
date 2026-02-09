use anyhow::{bail, Context, Result};
use clap::Parser;
use tracing::info;

/// Protocol conformance test runner for Liquide.
///
/// `liquide-conformance` connects to a running Liquide server and
/// exercises the protocol specification, reporting which mandatory and
/// optional behaviours the server correctly implements.
#[derive(Parser, Debug)]
#[command(name = "liquide-conformance", version, about)]
struct Cli {
    /// Server address in the form `host:port`.
    #[arg(long)]
    server: String,

    /// Test suite to run.
    ///
    /// Available suites: handshake, auth, streaming, clipboard, all.
    #[arg(long, default_value = "all")]
    suite: String,

    /// Username for authentication during testing.
    #[arg(long)]
    username: Option<String>,

    /// Password for authentication during testing.
    #[arg(long)]
    password: Option<String>,
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

    run(cli).await
}

async fn run(cli: Cli) -> Result<()> {
    info!(
        server = %cli.server,
        suite = %cli.suite,
        "Starting liquide-conformance"
    );

    // TODO: Establish a connection to the target server.
    info!(server = %cli.server, "Connecting to server...");

    let results = match cli.suite.as_str() {
        "handshake" => run_handshake_suite(&cli).await,
        "auth" => run_auth_suite(&cli).await,
        "streaming" => run_streaming_suite(&cli).await,
        "clipboard" => run_clipboard_suite(&cli).await,
        "all" => {
            let mut passed = 0u32;
            let mut failed = 0u32;

            for (p, f) in [
                run_handshake_suite(&cli).await?,
                run_auth_suite(&cli).await?,
                run_streaming_suite(&cli).await?,
                run_clipboard_suite(&cli).await?,
            ] {
                passed += p;
                failed += f;
            }
            Ok((passed, failed))
        }
        other => bail!("Unknown test suite: {other}"),
    };

    let (passed, failed) = results.context("Conformance suite failed")?;

    println!("\n--- Conformance Results ---");
    println!("  Passed: {passed}");
    println!("  Failed: {failed}");

    if failed > 0 {
        bail!("{failed} conformance test(s) failed");
    }

    Ok(())
}

async fn run_handshake_suite(_cli: &Cli) -> Result<(u32, u32)> {
    info!("Running handshake conformance suite...");
    // TODO: Test protocol version negotiation, capability exchange, etc.
    println!("  [SKIP] handshake suite — not yet implemented");
    Ok((0, 0))
}

async fn run_auth_suite(_cli: &Cli) -> Result<(u32, u32)> {
    info!("Running authentication conformance suite...");
    // TODO: Test password auth, token auth, MFA challenge flows.
    println!("  [SKIP] auth suite — not yet implemented");
    Ok((0, 0))
}

async fn run_streaming_suite(_cli: &Cli) -> Result<(u32, u32)> {
    info!("Running streaming conformance suite...");
    // TODO: Test frame delivery, damage regions, resize handling.
    println!("  [SKIP] streaming suite — not yet implemented");
    Ok((0, 0))
}

async fn run_clipboard_suite(_cli: &Cli) -> Result<(u32, u32)> {
    info!("Running clipboard conformance suite...");
    // TODO: Test clipboard copy/paste, MIME type negotiation.
    println!("  [SKIP] clipboard suite — not yet implemented");
    Ok((0, 0))
}
