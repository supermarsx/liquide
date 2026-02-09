use crate::cli::BrewCommand;
use crate::client::Client;
use crate::error::Result;
use crate::output::Output;

pub async fn execute(_client: &Client, output: &Output, cmd: &BrewCommand) -> Result<()> {
    match cmd {
        BrewCommand::Search(args) => {
            output.message(&format!("Searching Homebrew for '{}'...", args.query));
        }
        BrewCommand::Install(args) => {
            output.message(&format!("Installing {}...", args.package));
        }
        BrewCommand::Remove(args) => {
            output.message(&format!("Removing {}...", args.package));
        }
        BrewCommand::List(_args) => {
            output.message("No Homebrew packages installed.");
        }
        BrewCommand::Update(args) => {
            if args.check {
                output.message("Checking for Homebrew updates...");
            } else {
                output.message("Updating Homebrew packages...");
            }
        }
        BrewCommand::Info(args) => {
            output.message(&format!("Info for {}:", args.package));
        }
        BrewCommand::Tap(args) => {
            output.message(&format!("Tapping {}...", args.tap_name));
        }
        BrewCommand::Untap(args) => {
            output.message(&format!("Untapping {}...", args.tap_name));
        }
        BrewCommand::Pin(args) => {
            output.message(&format!("Pinning {}...", args.formula));
        }
        BrewCommand::Unpin(args) => {
            output.message(&format!("Unpinning {}...", args.formula));
        }
        BrewCommand::Rollback(args) => {
            output.message(&format!("Rolling back {}...", args.package));
        }
    }
    Ok(())
}
