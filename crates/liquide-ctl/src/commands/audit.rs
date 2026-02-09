use crate::cli::AuditCommand;
use crate::client::Client;
use crate::error::Result;
use crate::output::Output;

pub async fn execute(_client: &Client, output: &Output, cmd: &AuditCommand) -> Result<()> {
    match cmd {
        AuditCommand::List(args) => {
            let _ = args;
            output.message("Audit log not available (not connected).");
        }
    }
    Ok(())
}
