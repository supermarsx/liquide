use crate::cli::AppimageCommand;
use crate::client::Client;
use crate::error::Result;
use crate::output::Output;

pub async fn execute(_client: &Client, output: &Output, cmd: &AppimageCommand) -> Result<()> {
    match cmd {
        AppimageCommand::List => {
            output.message("No integrated AppImages.");
        }
        AppimageCommand::Update(args) => {
            if args.check {
                output.message("Checking for AppImage updates...");
            } else {
                output.message("Updating AppImages...");
            }
        }
        AppimageCommand::Integrate(args) => {
            output.message(&format!("Integrating AppImage '{}'...", args.file));
        }
        AppimageCommand::Remove(args) => {
            output.message(&format!("Removing AppImage '{}'...", args.app));
        }
        AppimageCommand::Verify(args) => {
            output.message(&format!("Verifying AppImage '{}'...", args.file));
        }
    }
    Ok(())
}
