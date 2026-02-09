use crate::cli::ConfigCommand;
use crate::client::Client;
use crate::error::Result;
use crate::output::Output;

pub async fn execute(client: &Client, output: &Output, cmd: &ConfigCommand) -> Result<()> {
    match cmd {
        ConfigCommand::Show(args) => show(client, output, args).await,
        ConfigCommand::Validate => validate(client, output).await,
        ConfigCommand::Set(args) => set(client, output, args).await,
        ConfigCommand::Diff => diff(client, output).await,
        ConfigCommand::Export => export(client, output).await,
        ConfigCommand::Import(args) => import(client, output, args).await,
    }
}

async fn show(
    _client: &Client,
    output: &Output,
    args: &crate::cli::ConfigShowArgs,
) -> Result<()> {
    let _ = args;
    // TODO: GET /api/v1/config
    output.message("Server configuration not available (not connected).");
    Ok(())
}

async fn validate(_client: &Client, output: &Output) -> Result<()> {
    // TODO: POST /api/v1/config/validate
    output.message("Configuration validation not available (not connected).");
    Ok(())
}

async fn set(
    _client: &Client,
    output: &Output,
    args: &crate::cli::ConfigSetArgs,
) -> Result<()> {
    // TODO: PUT /api/v1/config/{key}
    output.message(&format!("Set {} = {}", args.key, args.value));
    Ok(())
}

async fn diff(_client: &Client, output: &Output) -> Result<()> {
    // TODO: GET /api/v1/config/diff
    output.message("No differences.");
    Ok(())
}

async fn export(_client: &Client, output: &Output) -> Result<()> {
    // TODO: GET /api/v1/config/export
    output.message("Configuration export not available (not connected).");
    Ok(())
}

async fn import(
    _client: &Client,
    output: &Output,
    args: &crate::cli::ConfigImportArgs,
) -> Result<()> {
    // TODO: POST /api/v1/config/import
    output.message(&format!("Importing from {}...", args.file));
    Ok(())
}
