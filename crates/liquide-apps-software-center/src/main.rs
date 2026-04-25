use anyhow::Result;
use liquide_apps_software_center::run_default_app;

/// Built-in software center for the LiquiDE desktop environment.
fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    run_default_app()
}
