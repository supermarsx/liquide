use anyhow::Result;
use tracing::info;
use liquide_apps_settings::{SettingsConfig, SettingsRuntime};

/// Built-in settings application for the LiquiDE desktop environment.
fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let config = SettingsConfig::default();
    info!(category = %config.default_category, "Starting liquid-settings");

    let rt = SettingsRuntime::new(config);
    info!(entries = rt.total_entries(), "Loaded settings");

    let infos = rt.category_infos();
    for info_item in &infos {
        info!(
            category = %info_item.category.label(),
            entries = info_item.entry_count,
            "Category loaded"
        );
    }

    println!("liquid-settings: {} categories, {} total settings", infos.len(), rt.total_entries());

    Ok(())
}
