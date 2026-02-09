use anyhow::Result;
use tracing::info;

/// Built-in file manager for the Liquide desktop environment.
///
/// `liquid-files` provides a graphical file browser with native desktop
/// integration for file previews, drag-and-drop, and host interop.
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
    info!("Starting liquid-files");

    // TODO: Initialize the Liquide UI application context.
    info!("Initializing UI application...");

    // TODO: Load the file manager CSS theme.
    info!("Loading file manager theme...");

    // TODO: Create the file browser widget with sidebar and content pane.
    info!("Creating file browser...");

    // TODO: Initialize the interop layer for host filesystem access.
    info!("Initializing host interop...");

    // TODO: Populate the initial directory listing.
    info!("Loading home directory...");

    // TODO: Enter the UI event loop (navigation, file operations, preview).
    info!("File manager ready — entering event loop");

    // Placeholder: simulate the event loop.
    println!("liquid-files: stub — event loop not yet implemented");

    Ok(())
}
