use crate::cli::SessionsCommand;
use crate::client::Client;
use crate::error::Result;
use crate::output::Output;

pub async fn execute(client: &Client, output: &Output, cmd: &SessionsCommand) -> Result<()> {
    match cmd {
        SessionsCommand::List(args) => list(client, output, args).await,
        SessionsCommand::Show(args) => show(client, output, args).await,
        SessionsCommand::Disconnect(args) => disconnect(client, output, args).await,
        SessionsCommand::DisconnectAll(args) => disconnect_all(client, output, args).await,
    }
}

async fn list(
    _client: &Client,
    output: &Output,
    args: &crate::cli::SessionsListArgs,
) -> Result<()> {
    let _ = args;
    // TODO: GET /api/v1/sessions
    output.message("No active sessions.");
    Ok(())
}

async fn show(
    _client: &Client,
    output: &Output,
    args: &crate::cli::SessionsShowArgs,
) -> Result<()> {
    // TODO: GET /api/v1/sessions/{id}
    output.message(&format!("Session {} not found.", args.session_id));
    Ok(())
}

async fn disconnect(
    _client: &Client,
    output: &Output,
    args: &crate::cli::SessionsDisconnectArgs,
) -> Result<()> {
    let _ = args;
    // TODO: POST /api/v1/sessions/{id}/disconnect
    output.message(&format!("Session {} disconnected.", args.session_id));
    Ok(())
}

async fn disconnect_all(
    _client: &Client,
    output: &Output,
    args: &crate::cli::SessionsDisconnectAllArgs,
) -> Result<()> {
    let _ = args;
    // TODO: POST /api/v1/sessions/disconnect-all
    output.message("All sessions disconnected.");
    Ok(())
}
