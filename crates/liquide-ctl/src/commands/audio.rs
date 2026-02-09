use crate::cli::AudioCommand;
use crate::client::Client;
use crate::error::Result;
use crate::output::Output;

pub async fn execute(_client: &Client, output: &Output, cmd: &AudioCommand) -> Result<()> {
    match cmd {
        AudioCommand::Status => {
            output.message("Audio subsystem status not available (not connected).");
        }
    }
    Ok(())
}
