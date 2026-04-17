use serde::{Deserialize, Serialize};

use crate::cli::SessionsCommand;
use crate::client::{ApiResponse, Client};
use crate::error::Result;
use crate::output::Output;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct SessionInfo {
    pub id: String,
    pub user: String,
    pub status: String,
    #[serde(default)]
    pub connected_at: String,
}

pub async fn execute(client: &Client, output: &Output, cmd: &SessionsCommand) -> Result<()> {
    match cmd {
        SessionsCommand::List(args) => list(client, output, args).await,
        SessionsCommand::Show(args) => show(client, output, args).await,
        SessionsCommand::Disconnect(args) => disconnect(client, output, args).await,
        SessionsCommand::DisconnectAll(args) => disconnect_all(client, output, args).await,
    }
}

async fn list(
    client: &Client,
    output: &Output,
    args: &crate::cli::SessionsListArgs,
) -> Result<()> {
    let mut path = "/api/v1/sessions".to_string();
    if let Some(user) = &args.user {
        path.push_str(&format!("?user={user}"));
    }
    let resp: ApiResponse<Vec<SessionInfo>> = client.get(&path).await?;
    match resp.data {
        Some(sessions) if sessions.is_empty() => {
            output.message("No active sessions.");
        }
        Some(sessions) => {
            output.message(&format!(
                "{:<12} {:<16} {:<10} {}",
                "ID", "USER", "STATUS", "CONNECTED"
            ));
            for s in &sessions {
                output.message(&format!(
                    "{:<12} {:<16} {:<10} {}",
                    s.id, s.user, s.status, s.connected_at
                ));
            }
            output.message(&format!("\n{} session(s) total.", sessions.len()));
        }
        None => {
            if let Some(err) = resp.error {
                output.error(&err);
            } else {
                output.message("No active sessions.");
            }
        }
    }
    Ok(())
}

async fn show(
    client: &Client,
    output: &Output,
    args: &crate::cli::SessionsShowArgs,
) -> Result<()> {
    let path = format!("/api/v1/sessions/{}", args.session_id);
    let resp: ApiResponse<serde_json::Value> = client.get(&path).await?;
    match resp.data {
        Some(session) => {
            output.message(&serde_json::to_string_pretty(&session).unwrap_or_default());
        }
        None => {
            if let Some(err) = resp.error {
                output.error(&err);
            } else {
                output.message(&format!("Session {} not found.", args.session_id));
            }
        }
    }
    Ok(())
}

async fn disconnect(
    client: &Client,
    output: &Output,
    args: &crate::cli::SessionsDisconnectArgs,
) -> Result<()> {
    let path = format!("/api/v1/sessions/{}/disconnect", args.session_id);
    let body = serde_json::json!({ "message": args.message });
    let resp: ApiResponse<serde_json::Value> = client.post(&path, &body).await?;
    if resp.success {
        output.success(&format!("Session {} disconnected.", args.session_id));
    } else if let Some(err) = resp.error {
        output.error(&err);
    }
    Ok(())
}

async fn disconnect_all(
    client: &Client,
    output: &Output,
    args: &crate::cli::SessionsDisconnectAllArgs,
) -> Result<()> {
    let body = serde_json::json!({
        "user": args.user,
        "drain": args.drain,
    });
    let resp: ApiResponse<serde_json::Value> =
        client.post("/api/v1/sessions/disconnect-all", &body).await?;
    if resp.success {
        output.success("All sessions disconnected.");
    } else if let Some(err) = resp.error {
        output.error(&err);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_sessions_list() {
        let json = r#"{
            "success": true,
            "data": [
                {
                    "id": "sess-001",
                    "user": "alice",
                    "status": "active",
                    "connected_at": "2026-04-16T10:00:00Z"
                },
                {
                    "id": "sess-002",
                    "user": "bob",
                    "status": "idle",
                    "connected_at": "2026-04-16T09:30:00Z"
                }
            ]
        }"#;
        let resp: ApiResponse<Vec<SessionInfo>> = serde_json::from_str(json).unwrap();
        assert!(resp.success);
        let sessions = resp.data.unwrap();
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].id, "sess-001");
        assert_eq!(sessions[1].user, "bob");
    }

    #[test]
    fn session_disconnect_path_construction() {
        let session_id = "sess-42";
        let path = format!("/api/v1/sessions/{}/disconnect", session_id);
        assert_eq!(path, "/api/v1/sessions/sess-42/disconnect");
    }
}
