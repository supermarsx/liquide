use anyhow::Result;
use liquide_apps_files::{FilesConfig, FilesRuntime};
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

    // Load file manager configuration.
    info!("Loading configuration...");
    let config = FilesConfig::default();

    // Initialize the file manager runtime.
    info!("Initializing file manager runtime...");
    let runtime = FilesRuntime::new(config);

    info!(
        bookmarks = runtime.sidebar().bookmarks().len(),
        "File manager initialized"
    );

    // Navigate to home directory.
    let listing = runtime.current_listing();
    info!(
        path = %listing.path,
        entries = listing.entries.len(),
        "Directory listing loaded"
    );

    info!("File manager ready — entering event loop");

    // Placeholder: simulate the event loop.
    println!("liquid-files: event loop not yet wired to actual UI toolkit");

    Ok(())
}
