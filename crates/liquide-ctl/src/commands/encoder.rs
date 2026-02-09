use crate::cli::EncoderCommand;
use crate::client::Client;
use crate::error::Result;
use crate::output::Output;

pub async fn execute(_client: &Client, output: &Output, cmd: &EncoderCommand) -> Result<()> {
    match cmd {
        EncoderCommand::List => {
            output.message("Encoder list not available (not connected).");
        }
        EncoderCommand::Benchmark(args) => {
            output.message(&format!("Benchmarking encoder '{}'...", args.encoder));
        }
    }
    Ok(())
}
