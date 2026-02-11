use anyhow::Result;
use tracing::info;
use liquide_apps_software_center::{SoftwareCenterConfig, SoftwareCenterRuntime};

/// Built-in software center for the LiquiDE desktop environment.
fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let config = SoftwareCenterConfig::default();
    info!(auto_updates = config.auto_check_updates, "Starting liquid-software-center");

    let rt = SoftwareCenterRuntime::new(config);
    info!(repos = rt.repos().count(), "Repositories loaded");

    println!(
        "liquid-software-center: {} repositories, {} packages",
        rt.repos().count(),
        rt.catalog().total_count(),
    );

    Ok(())
}
