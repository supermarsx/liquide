use serde::{Deserialize, Serialize};

use crate::cli::StatusArgs;
use crate::client::{ApiResponse, Client};
use crate::error::Result;
use crate::output::Output;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ServerStatus {
    pub version: String,
    pub status: String,
    pub uptime_seconds: u64,
    pub sessions_active: u32,
}

pub async fn execute(client: &Client, output: &Output, args: &StatusArgs) -> Result<()> {
    let _ = args;
    let resp: ApiResponse<ServerStatus> = client.get("/api/v1/status").await?;
    match resp.data {
        Some(status) => {
            output.message(&format!("LiquiDE Server {}", status.version));
            output.message(&format!("  Status:       {}", status.status));
            output.message(&format!("  Uptime:       {}s", status.uptime_seconds));
            output.message(&format!("  Sessions:     {} active", status.sessions_active));
        }
        None => {
            if let Some(err) = resp.error {
                output.error(&err);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_server_status() {
        let json = r#"{
            "success": true,
            "data": {
                "version": "0.2.0",
                "status": "running",
                "uptime_seconds": 3600,
                "sessions_active": 5
            }
        }"#;
        let resp: ApiResponse<ServerStatus> = serde_json::from_str(json).unwrap();
        assert!(resp.success);
        let data = resp.data.unwrap();
        assert_eq!(data.version, "0.2.0");
        assert_eq!(data.sessions_active, 5);
    }
}
