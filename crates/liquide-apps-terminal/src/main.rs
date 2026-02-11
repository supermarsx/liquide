use anyhow::Result;
use liquide_apps_terminal::{TerminalConfig, TerminalRuntime};
use tracing::info;

/// Built-in terminal emulator for the Liquide desktop environment.
///
/// `liquid-terminal` provides a GPU-accelerated terminal with native
/// integration into the Liquide UI toolkit and CSS theming engine.
fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    run()
}

fn run() -> Result<()> {
    info!("Starting liquid-terminal");

    // Load terminal configuration.
    info!("Loading configuration...");
    let config = TerminalConfig::default();

    // Initialize the terminal runtime.
    info!("Initializing terminal runtime...");
    let mut runtime = TerminalRuntime::new(config);

    info!(
        tabs = runtime.tab_count(),
        shell = %runtime.config().shell,
        "Terminal runtime initialized"
    );

    // Create default tab.
    let tab_id = runtime.new_tab(None);
    info!(tab_id, "Created initial tab");

    // Build initial grid state.
    let grid = runtime.active_grid();
    info!(
        rows = grid.rows(),
        cols = grid.cols(),
        "Grid ready"
    );

    info!("Terminal ready — entering event loop");

    // Placeholder: simulate the event loop.
    println!("liquid-terminal: event loop not yet wired to actual UI toolkit");

    Ok(())
}
