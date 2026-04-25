use crate::cli::SnapCommand;
use crate::client::Client;
use crate::error::Result;
use crate::output::Output;

pub async fn execute(_client: &Client, output: &Output, cmd: &SnapCommand) -> Result<()> {
    match cmd {
        SnapCommand::Search(args) => {
            output.message(&format!("Searching Snap Store for '{}'...", args.query));
        }
        SnapCommand::Install(args) => {
            output.message(&format!("Installing snap '{}'...", args.snap));
        }
        SnapCommand::Remove(args) => {
            output.message(&format!("Removing snap '{}'...", args.snap));
        }
        SnapCommand::List(_args) => {
            output.message("No snaps installed.");
        }
        SnapCommand::Update(args) => {
            if args.check {
                output.message("Checking for snap updates...");
            } else {
                output.message("Updating snaps...");
            }
        }
        SnapCommand::Info(args) => {
            output.message(&format!("Info for snap '{}':", args.snap));
        }
        SnapCommand::Connections(args) => {
            output.message(&format!("Interface connections for '{}':", args.snap));
        }
        SnapCommand::Connect(args) => {
            output.message(&format!("Connecting {}:{}...", args.snap, args.interface));
        }
        SnapCommand::Disconnect(args) => {
            output.message(&format!(
                "Disconnecting {}:{}...",
                args.snap, args.interface
            ));
        }
        SnapCommand::Revert(args) => {
            output.message(&format!("Reverting snap '{}'...", args.snap));
        }
        SnapCommand::RefreshHold(args) => {
            output.message(&format!(
                "Holding refresh for '{}' for {} hours...",
                args.snap, args.duration
            ));
        }
        SnapCommand::Channels(args) => {
            output.message(&format!("Channels for snap '{}':", args.snap));
        }
    }
    Ok(())
}
