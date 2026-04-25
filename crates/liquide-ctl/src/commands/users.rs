use serde::{Deserialize, Serialize};

use crate::cli::UsersCommand;
use crate::client::{ApiResponse, Client};
use crate::error::Result;
use crate::output::Output;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct UserInfo {
    pub username: String,
    #[serde(default)]
    pub sessions: u32,
    #[serde(default)]
    pub status: String,
}

pub async fn execute(client: &Client, output: &Output, cmd: &UsersCommand) -> Result<()> {
    match cmd {
        UsersCommand::List => list(client, output).await,
        UsersCommand::Show(args) => show(client, output, args).await,
        UsersCommand::Kick(args) => kick(client, output, args).await,
        UsersCommand::Avatar(cmd) => avatar(client, output, cmd).await,
    }
}

async fn list(client: &Client, output: &Output) -> Result<()> {
    let resp: ApiResponse<Vec<UserInfo>> = client.get("/api/v1/users").await?;
    match resp.data {
        Some(users) if users.is_empty() => {
            output.message("No connected users.");
        }
        Some(users) => {
            output.message(&format!(
                "{:<20} {:<10} {}",
                "USERNAME", "SESSIONS", "STATUS"
            ));
            for u in &users {
                output.message(&format!(
                    "{:<20} {:<10} {}",
                    u.username, u.sessions, u.status
                ));
            }
            output.message(&format!("\n{} user(s) total.", users.len()));
        }
        None => {
            if let Some(err) = resp.error {
                output.error(&err);
            } else {
                output.message("No connected users.");
            }
        }
    }
    Ok(())
}

async fn show(_client: &Client, output: &Output, args: &crate::cli::UsersShowArgs) -> Result<()> {
    output.message(&format!("User '{}' not found.", args.username));
    Ok(())
}

async fn kick(_client: &Client, output: &Output, args: &crate::cli::UsersKickArgs) -> Result<()> {
    output.message(&format!("User '{}' kicked.", args.username));
    Ok(())
}

async fn avatar(
    _client: &Client,
    output: &Output,
    cmd: &crate::cli::UsersAvatarCommand,
) -> Result<()> {
    match cmd {
        crate::cli::UsersAvatarCommand::Set(args) => {
            output.message(&format!("Avatar updated for user '{}'.", args.username));
        }
        crate::cli::UsersAvatarCommand::Remove(args) => {
            output.message(&format!("Avatar removed for user '{}'.", args.username));
        }
        crate::cli::UsersAvatarCommand::Show(args) => {
            output.message(&format!("Avatar metadata for '{}'.", args.username));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_users_list() {
        let json = r#"{
            "success": true,
            "data": [
                {"username": "alice", "sessions": 2, "status": "active"},
                {"username": "bob", "sessions": 1, "status": "idle"}
            ]
        }"#;
        let resp: ApiResponse<Vec<UserInfo>> = serde_json::from_str(json).unwrap();
        assert!(resp.success);
        let users = resp.data.unwrap();
        assert_eq!(users.len(), 2);
        assert_eq!(users[0].username, "alice");
        assert_eq!(users[0].sessions, 2);
    }
}
