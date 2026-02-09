use crate::cli::PolicyCommand;
use crate::client::Client;
use crate::error::Result;
use crate::output::Output;

pub async fn execute(client: &Client, output: &Output, cmd: &PolicyCommand) -> Result<()> {
    match cmd {
        PolicyCommand::Show => show(client, output).await,
        PolicyCommand::Set(args) => set(client, output, args).await,
        PolicyCommand::Effective(args) => effective(client, output, args).await,
    }
}

async fn show(_client: &Client, output: &Output) -> Result<()> {
    // TODO: GET /api/v1/policy
    output.message("Policy not available (not connected).");
    Ok(())
}

async fn set(
    _client: &Client,
    output: &Output,
    args: &crate::cli::PolicySetArgs,
) -> Result<()> {
    // TODO: PUT /api/v1/policy/{scope}/{key}
    output.message(&format!(
        "Set {}.{} = {}",
        args.scope, args.key, args.value
    ));
    Ok(())
}

async fn effective(
    _client: &Client,
    output: &Output,
    args: &crate::cli::PolicyEffectiveArgs,
) -> Result<()> {
    // TODO: GET /api/v1/policy/effective/{username}
    output.message(&format!(
        "Effective policy for {} not available (not connected).",
        args.username
    ));
    Ok(())
}
