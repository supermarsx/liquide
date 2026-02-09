use crate::cli::GatewayCommand;
use crate::client::Client;
use crate::error::Result;
use crate::output::Output;

pub async fn execute(_client: &Client, output: &Output, cmd: &GatewayCommand) -> Result<()> {
    match cmd {
        GatewayCommand::Status => {
            output.message("Gateway status not available (not connected).");
        }
        GatewayCommand::Register => {
            output.message("Registering with gateway...");
        }
        GatewayCommand::Deregister => {
            output.message("Deregistering from gateway...");
        }
    }
    Ok(())
}
