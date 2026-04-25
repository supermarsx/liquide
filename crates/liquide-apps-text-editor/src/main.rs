use anyhow::Result;
use liquide_apps_text_editor::run_default_app;

/// Built-in text editor for the LiquiDE desktop environment.
///
/// Wires `EditorRuntime` onto `liquide_app_harness::AppBootstrap`, which
/// drives the platform event loop, input translation, layout, and paint.
fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    run_default_app()
}
