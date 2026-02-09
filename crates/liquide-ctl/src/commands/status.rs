use crate::cli::StatusArgs;
use crate::client::Client;
use crate::error::Result;
use crate::output::Output;

pub async fn execute(client: &Client, output: &Output, args: &StatusArgs) -> Result<()> {
    let _ = args;
    // TODO: Fetch server status via client.get("/api/v1/status")
    output.message("LiquiDE Server v0.1.0");
    output.message("  Status:       connecting...");
    output.warn(&format!("Cannot connect to server at {}", client.server()));
    Ok(())
}
