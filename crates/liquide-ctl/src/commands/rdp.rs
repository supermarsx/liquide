use crate::cli::RdpCommand;
use crate::client::Client;
use crate::error::Result;
use crate::output::Output;

pub async fn execute(_client: &Client, output: &Output, cmd: &RdpCommand) -> Result<()> {
    match cmd {
        RdpCommand::Status => {
            output.message("RDP Compatibility: status not available (not connected).");
        }
        RdpCommand::Enable => {
            output.message("Enabling RDP compatibility...");
        }
        RdpCommand::Disable => {
            output.message("Disabling RDP compatibility...");
        }
    }
    Ok(())
}
