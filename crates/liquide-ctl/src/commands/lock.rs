use crate::cli::{LockArgs, UnlockArgs};
use crate::client::Client;
use crate::error::Result;
use crate::output::Output;

pub async fn execute_lock(_client: &Client, output: &Output, args: &LockArgs) -> Result<()> {
    match args.target.as_str() {
        "status" => {
            output.message("Lock status not available (not connected).");
        }
        "all" => {
            output.message("Locking all sessions...");
        }
        "config" => {
            output.message("Lock configuration not available (not connected).");
        }
        target if target.starts_with("policy") => {
            output.message("Lock policy not available (not connected).");
        }
        session_id => {
            output.message(&format!("Locking session {session_id}..."));
        }
    }
    Ok(())
}

pub async fn execute_unlock(_client: &Client, output: &Output, args: &UnlockArgs) -> Result<()> {
    output.message(&format!("Unlocking session {}...", args.session_id));
    Ok(())
}
