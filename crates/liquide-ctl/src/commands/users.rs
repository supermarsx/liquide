use crate::cli::UsersCommand;
use crate::client::Client;
use crate::error::Result;
use crate::output::Output;

pub async fn execute(client: &Client, output: &Output, cmd: &UsersCommand) -> Result<()> {
    match cmd {
        UsersCommand::List => list(client, output).await,
        UsersCommand::Show(args) => show(client, output, args).await,
        UsersCommand::Kick(args) => kick(client, output, args).await,
        UsersCommand::Avatar(cmd) => avatar(client, output, cmd).await,
    }
}

async fn list(_client: &Client, output: &Output) -> Result<()> {
    // TODO: GET /api/v1/users
    output.message("No connected users.");
    Ok(())
}

async fn show(
    _client: &Client,
    output: &Output,
    args: &crate::cli::UsersShowArgs,
) -> Result<()> {
    output.message(&format!("User '{}' not found.", args.username));
    Ok(())
}

async fn kick(
    _client: &Client,
    output: &Output,
    args: &crate::cli::UsersKickArgs,
) -> Result<()> {
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
            output.message(&format!(
                "Avatar updated for user '{}'.",
                args.username
            ));
        }
        crate::cli::UsersAvatarCommand::Remove(args) => {
            output.message(&format!(
                "Avatar removed for user '{}'.",
                args.username
            ));
        }
        crate::cli::UsersAvatarCommand::Show(args) => {
            output.message(&format!("Avatar metadata for '{}'.", args.username));
        }
    }
    Ok(())
}
