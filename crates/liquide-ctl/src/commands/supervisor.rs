use crate::cli::SupervisorCommand;
use crate::client::Client;
use crate::error::Result;
use crate::output::Output;

pub async fn execute(_client: &Client, output: &Output, cmd: &SupervisorCommand) -> Result<()> {
    match cmd {
        SupervisorCommand::Status => {
            output.message("Supervisor status not available (not connected).");
        }
        SupervisorCommand::Restart(args) => {
            output.message(&format!(
                "Restarting session process {}...",
                args.session_id
            ));
        }
        SupervisorCommand::ResetRestarts(args) => {
            output.message(&format!(
                "Reset restart counter for session {}.",
                args.session_id
            ));
        }
        SupervisorCommand::Logs(_args) => {
            output.message("Supervisor logs not available (not connected).");
        }
    }
    Ok(())
}
