//! Liquide Telemetry Viewer - Real-time performance monitoring and debugging.
//!
//! This tool provides comprehensive telemetry visualization for the Liquide
//! desktop environment, including:
//!
//! - Real-time frame time graphs
//! - Per-window rendering metrics
//! - Thread utilization tracking
//! - System health status
//! - Historical performance data
//!
//! ## Modes
//!
//! - **TUI** - Terminal-based interactive dashboard
//! - **Web** - Browser-based viewer with live updates
//! - **JSON** - Export telemetry data as JSON
//! - **Report** - Generate HTML performance reports

use anyhow::Result;
use clap::{Parser, Subcommand};

mod collector;
mod dashboard;
mod web;
mod export;
mod types;

/// Liquide Telemetry Viewer - Monitor desktop performance in real-time.
#[derive(Parser, Debug)]
#[command(name = "liquide-telemetry")]
#[command(about = "Real-time performance monitoring and debugging for Liquide", long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Launch interactive TUI dashboard.
    Tui {
        /// Refresh rate in milliseconds.
        #[arg(short, long, default_value = "100")]
        refresh: u64,
        
        /// Connect to remote session via TCP.
        #[arg(short, long)]
        remote: Option<String>,
    },
    
    /// Start web-based viewer.
    Web {
        /// Port to listen on.
        #[arg(short, long, default_value = "8080")]
        port: u16,
        
        /// Bind address.
        #[arg(short, long, default_value = "127.0.0.1")]
        bind: String,
    },
    
    /// Export telemetry data to JSON.
    Export {
        /// Output file path.
        #[arg(short, long)]
        output: String,
        
        /// Duration to collect in seconds.
        #[arg(short, long, default_value = "60")]
        duration: u64,
    },
    
    /// Generate HTML performance report.
    Report {
        /// Output HTML file path.
        #[arg(short, long)]
        output: String,
        
        /// Duration to collect in seconds.
        #[arg(short, long, default_value = "60")]
        duration: u64,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let args = Args::parse();

    match args.command {
        Commands::Tui { refresh, remote } => {
            tracing::info!("launching TUI dashboard (refresh: {}ms)", refresh);
            dashboard::run_tui(refresh, remote).await?;
        }
        Commands::Web { port, bind } => {
            tracing::info!("starting web viewer at {}:{}", bind, port);
            web::run_server(&bind, port).await?;
        }
        Commands::Export { output, duration } => {
            tracing::info!("exporting telemetry to {} ({}s)", output, duration);
            export::export_json(&output, duration).await?;
        }
        Commands::Report { output, duration } => {
            tracing::info!("generating report to {} ({}s)", output, duration);
            export::generate_report(&output, duration).await?;
        }
    }

    Ok(())
}
