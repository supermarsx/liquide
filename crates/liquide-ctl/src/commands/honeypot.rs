use crate::cli::HoneypotCommand;
use crate::client::Client;
use crate::error::Result;
use crate::output::Output;

pub async fn execute(_client: &Client, output: &Output, cmd: &HoneypotCommand) -> Result<()> {
    match cmd {
        HoneypotCommand::Status(_args) => {
            output.message("Honeypot & Tarpit status not available (not connected).");
        }
        HoneypotCommand::List => {
            output.message("No active honeypot/tarpit connections.");
        }
        HoneypotCommand::Drop(args) => {
            output.message(&format!("Dropping connection {}...", args.connection_id));
        }
        HoneypotCommand::DropAll => {
            output.message("Dropping all honeypot/tarpit connections...");
        }
        HoneypotCommand::Iocs(_args) => {
            output.message("No IOCs available.");
        }
        HoneypotCommand::Triggers => {
            output.message("Trigger configuration not available (not connected).");
        }
    }
    Ok(())
}
