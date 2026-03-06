use clap::{Parser, Subcommand};

// ── sessions ────────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum SessionsCommand {
    /// List all active sessions.
    List(SessionsListArgs),
    /// Show detailed session information.
    Show(SessionsShowArgs),
    /// Disconnect a session.
    Disconnect(SessionsDisconnectArgs),
    /// Disconnect all sessions.
    DisconnectAll(SessionsDisconnectAllArgs),
}

#[derive(Debug, Parser)]
pub struct SessionsListArgs {
    /// Filter by user.
    #[arg(long)]
    pub user: Option<String>,
    /// Sort by column.
    #[arg(long)]
    pub sort: Option<String>,
    /// Live updating.
    #[arg(long)]
    pub watch: bool,
}

#[derive(Debug, Parser)]
pub struct SessionsShowArgs {
    /// Session ID.
    pub session_id: String,
}

#[derive(Debug, Parser)]
pub struct SessionsDisconnectArgs {
    /// Session ID.
    pub session_id: String,
    /// Skip interactive confirmation.
    #[arg(long)]
    pub confirm: bool,
    /// Send a message to the user before disconnecting.
    #[arg(long)]
    pub message: Option<String>,
}

#[derive(Debug, Parser)]
pub struct SessionsDisconnectAllArgs {
    /// Disconnect only sessions for a specific user.
    #[arg(long)]
    pub user: Option<String>,
    /// Skip interactive confirmation.
    #[arg(long)]
    pub confirm: bool,
    /// Stop accepting new sessions and wait for existing to end gracefully.
    #[arg(long)]
    pub drain: bool,
}

// ── users ───────────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum UsersCommand {
    /// List connected users.
    List,
    /// Detailed user information.
    Show(UsersShowArgs),
    /// Disconnect all sessions for a user.
    Kick(UsersKickArgs),
    /// Manage user avatars.
    #[command(subcommand)]
    Avatar(UsersAvatarCommand),
}

#[derive(Debug, Parser)]
pub struct UsersShowArgs {
    /// Username.
    pub username: String,
}

#[derive(Debug, Parser)]
pub struct UsersKickArgs {
    /// Username.
    pub username: String,
}

#[derive(Debug, Subcommand)]
pub enum UsersAvatarCommand {
    /// Set or replace a user's avatar image.
    Set(UsersAvatarSetArgs),
    /// Remove a user's avatar.
    Remove(UsersAvatarRemoveArgs),
    /// Display avatar metadata.
    Show(UsersAvatarShowArgs),
}

#[derive(Debug, Parser)]
pub struct UsersAvatarSetArgs {
    /// Username.
    pub username: String,
    /// Path to avatar image file.
    pub path: String,
}

#[derive(Debug, Parser)]
pub struct UsersAvatarRemoveArgs {
    pub username: String,
}

#[derive(Debug, Parser)]
pub struct UsersAvatarShowArgs {
    pub username: String,
}
