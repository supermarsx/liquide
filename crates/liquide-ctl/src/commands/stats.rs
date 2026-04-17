use serde::{Deserialize, Serialize};

use crate::cli::StatsArgs;
use crate::client::{ApiResponse, Client};
use crate::error::Result;
use crate::output::Output;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ServerStats {
    pub sessions_active: u32,
    pub connections_total: u64,
    pub bytes_in: u64,
    pub bytes_out: u64,
    #[serde(default)]
    pub uptime_seconds: u64,
}

pub async fn execute(client: &Client, output: &Output, args: &StatsArgs) -> Result<()> {
    let mut path = "/api/v1/stats".to_string();
    if let Some(session) = &args.session {
        path.push_str(&format!("?session={session}"));
    }
    let resp: ApiResponse<ServerStats> = client.get(&path).await?;
    match resp.data {
        Some(stats) => {
            output.message("Aggregate Statistics");
            output.message(&format!("  Sessions:       {} active", stats.sessions_active));
            output.message(&format!("  Connections:    {} total", stats.connections_total));
            output.message(&format!("  Bytes In:       {}", stats.bytes_in));
            output.message(&format!("  Bytes Out:      {}", stats.bytes_out));
            output.message(&format!("  Uptime:         {}s", stats.uptime_seconds));
        }
        None => {
            if let Some(err) = resp.error {
                output.error(&err);
            }
        }
    }
    Ok(())
}
