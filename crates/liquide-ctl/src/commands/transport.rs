use crate::cli::TransportCommand;
use crate::client::Client;
use crate::error::Result;
use crate::output::Output;

pub async fn execute(_client: &Client, output: &Output, cmd: &TransportCommand) -> Result<()> {
    match cmd {
        TransportCommand::Status => {
            output.message("Transport status not available (not connected).");
        }
        TransportCommand::Switch(args) => {
            output.message(&format!(
                "Switching session {} to {}...",
                args.session_id, args.transport
            ));
        }
    }
    Ok(())
}
