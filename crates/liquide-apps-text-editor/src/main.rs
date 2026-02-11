use anyhow::Result;
use tracing::info;
use liquide_apps_text_editor::{EditorConfig, EditorRuntime};

/// Built-in text editor for the LiquiDE desktop environment.
fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let config = EditorConfig::default();
    info!(font = %config.font_family, size = config.font_size, "Starting liquid-text-editor");

    let mut rt = EditorRuntime::new(config);
    let id = rt.new_document();
    info!(doc_id = id, "New document created");

    println!("liquid-text-editor: {} document(s) open", rt.document_count());

    Ok(())
}
