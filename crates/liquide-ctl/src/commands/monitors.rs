use crate::cli::MonitorsCommand;
use crate::client::Client;
use crate::error::Result;
use crate::output::Output;

pub async fn execute(_client: &Client, output: &Output, cmd: &MonitorsCommand) -> Result<()> {
    match cmd {
        MonitorsCommand::List(args) => {
            output.message(&format!("Monitors for session {}:", args.session));
            output.message("  (not connected)");
        }
        MonitorsCommand::Add(args) => {
            output.message(&format!("Adding monitor to session {}...", args.session_id));
        }
        MonitorsCommand::Remove(args) => {
            output.message(&format!(
                "Removing monitor {} from session {}...",
                args.monitor_id, args.session_id
            ));
        }
        MonitorsCommand::Resize(args) => {
            output.message(&format!(
                "Resizing monitor {} in session {} to {}...",
                args.monitor_id, args.session_id, args.resolution
            ));
        }
    }
    Ok(())
}
