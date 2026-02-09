use crate::cli::ServiceCommand;
use crate::client::Client;
use crate::error::Result;
use crate::output::Output;

pub async fn execute(_client: &Client, output: &Output, cmd: &ServiceCommand) -> Result<()> {
    match cmd {
        ServiceCommand::Status => {
            output.message("Service status not available (not connected).");
        }
        ServiceCommand::Restart => {
            output.message("Restarting LiquiDE server...");
        }
        ServiceCommand::Stop(args) => {
            let _ = args;
            output.message("Stopping LiquiDE server...");
        }
    }
    Ok(())
}
