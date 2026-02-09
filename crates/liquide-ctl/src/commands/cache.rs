use crate::cli::CacheCommand;
use crate::client::Client;
use crate::error::Result;
use crate::output::Output;

pub async fn execute(_client: &Client, output: &Output, cmd: &CacheCommand) -> Result<()> {
    match cmd {
        CacheCommand::Status => {
            output.message("Cache status not available (not connected).");
        }
        CacheCommand::Clear(args) => {
            let _ = args;
            output.message("Caches cleared.");
        }
    }
    Ok(())
}
