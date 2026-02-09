use crate::cli::CrashCommand;
use crate::client::Client;
use crate::error::Result;
use crate::output::Output;

pub async fn execute(_client: &Client, output: &Output, cmd: &CrashCommand) -> Result<()> {
    match cmd {
        CrashCommand::List(_args) => {
            output.message("No crash reports.");
        }
        CrashCommand::Show(args) => {
            output.message(&format!(
                "Crash report '{}' not found.",
                args.report_id
            ));
        }
        CrashCommand::Export(args) => {
            output.message(&format!("Exporting crash report '{}'...", args.report_id));
        }
        CrashCommand::Delete(args) => {
            if args.all {
                output.message("Deleting all crash reports...");
            } else if let Some(id) = &args.report_id {
                output.message(&format!("Deleting crash report '{id}'..."));
            }
        }
        CrashCommand::Stats(_args) => {
            output.message("No crash statistics available.");
        }
    }
    Ok(())
}
