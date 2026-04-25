use crate::cli::FlatpakCommand;
use crate::client::Client;
use crate::error::Result;
use crate::output::Output;

pub async fn execute(_client: &Client, output: &Output, cmd: &FlatpakCommand) -> Result<()> {
    match cmd {
        FlatpakCommand::Search(args) => {
            output.message(&format!("Searching Flathub for '{}'...", args.query));
        }
        FlatpakCommand::Install(args) => {
            output.message(&format!("Installing {}...", args.app_id));
        }
        FlatpakCommand::Remove(args) => {
            output.message(&format!("Removing {}...", args.app_id));
        }
        FlatpakCommand::List(_args) => {
            output.message("No Flatpak applications installed.");
        }
        FlatpakCommand::Update(args) => {
            if args.check {
                output.message("Checking for Flatpak updates...");
            } else {
                output.message("Updating Flatpak applications...");
            }
        }
        FlatpakCommand::Permissions(args) => {
            output.message(&format!("Permissions for {} not available.", args.app_id));
        }
        FlatpakCommand::Override(args) => {
            output.message(&format!("Setting overrides for {}...", args.app_id));
        }
        FlatpakCommand::RemoteList => {
            output.message("No Flatpak remotes configured.");
        }
        FlatpakCommand::RemoteAdd(args) => {
            output.message(&format!("Adding remote '{}' ({})...", args.name, args.url));
        }
        FlatpakCommand::RemoteRemove(args) => {
            output.message(&format!("Removing remote '{}'...", args.name));
        }
        FlatpakCommand::Rollback(args) => {
            output.message(&format!("Rolling back {}...", args.app_id));
        }
        FlatpakCommand::History(args) => {
            output.message(&format!("History for {}:", args.app_id));
        }
        FlatpakCommand::Gc(_args) => {
            output.message("Garbage collecting unused Flatpak data...");
        }
    }
    Ok(())
}
