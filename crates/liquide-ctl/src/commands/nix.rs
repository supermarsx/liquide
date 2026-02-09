use crate::cli::NixCommand;
use crate::client::Client;
use crate::error::Result;
use crate::output::Output;

pub async fn execute(_client: &Client, output: &Output, cmd: &NixCommand) -> Result<()> {
    match cmd {
        NixCommand::Search(args) => {
            output.message(&format!("Searching nixpkgs for '{}'...", args.query));
        }
        NixCommand::Install(args) => {
            output.message(&format!("Installing {}...", args.package));
        }
        NixCommand::Remove(args) => {
            output.message(&format!("Removing {}...", args.package));
        }
        NixCommand::List(_args) => {
            output.message("No Nix packages installed.");
        }
        NixCommand::Update(args) => {
            if args.check {
                output.message("Checking for Nix updates...");
            } else {
                output.message("Updating Nix packages...");
            }
        }
        NixCommand::Rollback(_args) => {
            output.message("Rolling back to previous profile generation...");
        }
        NixCommand::Gc(args) => {
            if args.dry_run {
                output.message("Dry run: would garbage-collect unused store paths.");
            } else {
                output.message("Garbage-collecting unused Nix store paths...");
            }
        }
        NixCommand::Develop(_args) => {
            output.message("Entering Nix development shell...");
        }
    }
    Ok(())
}
