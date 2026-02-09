use crate::cli::StatsArgs;
use crate::client::Client;
use crate::error::Result;
use crate::output::Output;

pub async fn execute(_client: &Client, output: &Output, args: &StatsArgs) -> Result<()> {
    let _ = args;
    // TODO: GET /api/v1/stats
    output.message("Aggregate Statistics");
    output.message("  Sessions:       0 active");
    Ok(())
}
