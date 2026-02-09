use anyhow::Result;
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

    // TODO: Initialize the Liquide UI application context.
    info!("Initializing UI application...");

    // TODO: Load the terminal CSS theme.
    info!("Loading terminal theme...");

    // TODO: Create the terminal widget with a PTY backend.
    info!("Creating terminal widget...");

    // TODO: Spawn the default shell process.
    info!("Spawning shell process...");

    // TODO: Enter the UI event loop (input dispatch, PTY read, render).
    info!("Terminal ready — entering event loop");

    // Placeholder: simulate the event loop.
    println!("liquid-terminal: stub — event loop not yet implemented");

    Ok(())
}
