use clap::{Parser, Subcommand};

// ── flatpak ─────────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum FlatpakCommand {
    /// Search Flathub for applications.
    Search(FlatpakSearchArgs),
    /// Install a Flatpak application.
    Install(FlatpakInstallArgs),
    /// Remove a Flatpak application.
    Remove(FlatpakRemoveArgs),
    /// List installed Flatpak applications.
    List(FlatpakListArgs),
    /// Update Flatpak applications.
    Update(FlatpakUpdateArgs),
    /// Show effective permissions.
    Permissions(FlatpakPermissionsArgs),
    /// Set permission overrides.
    Override(FlatpakOverrideArgs),
    /// List configured remotes.
    RemoteList,
    /// Add a remote repository.
    RemoteAdd(FlatpakRemoteAddArgs),
    /// Remove a remote.
    RemoteRemove(FlatpakRemoteRemoveArgs),
    /// Rollback to previous commit.
    Rollback(FlatpakRollbackArgs),
    /// Show version/commit history.
    History(FlatpakHistoryArgs),
    /// Garbage-collect unused data.
    Gc(FlatpakGcArgs),
}

#[derive(Debug, Parser)]
pub struct FlatpakSearchArgs {
    pub query: String,
    #[arg(long)]
    pub remote: Option<String>,
}

#[derive(Debug, Parser)]
pub struct FlatpakInstallArgs {
    pub app_id: String,
    #[arg(long)]
    pub user: bool,
    #[arg(long)]
    pub system: bool,
    #[arg(long)]
    pub noninteractive: bool,
    #[arg(long)]
    pub no_deps: bool,
}

#[derive(Debug, Parser)]
pub struct FlatpakRemoveArgs {
    pub app_id: String,
    #[arg(long)]
    pub user: bool,
    #[arg(long)]
    pub system: bool,
    #[arg(long)]
    pub delete_data: bool,
    #[arg(long)]
    pub noninteractive: bool,
}

#[derive(Debug, Parser)]
pub struct FlatpakListArgs {
    #[arg(long)]
    pub user: bool,
    #[arg(long)]
    pub system: bool,
    #[arg(long)]
    pub all: bool,
    #[arg(long)]
    pub runtimes: bool,
    #[arg(long)]
    pub columns: Option<String>,
}

#[derive(Debug, Parser)]
pub struct FlatpakUpdateArgs {
    pub app_id: Option<String>,
    #[arg(long)]
    pub user: bool,
    #[arg(long)]
    pub system: bool,
    #[arg(long)]
    pub check: bool,
    #[arg(long)]
    pub noninteractive: bool,
}

#[derive(Debug, Parser)]
pub struct FlatpakPermissionsArgs {
    pub app_id: String,
}

#[derive(Debug, Parser)]
pub struct FlatpakOverrideArgs {
    pub app_id: String,
    #[arg(long)]
    pub filesystem: Option<String>,
    #[arg(long)]
    pub nofilesystem: Option<String>,
    #[arg(long)]
    pub socket: Option<String>,
    #[arg(long)]
    pub nosocket: Option<String>,
    #[arg(long)]
    pub device: Option<String>,
    #[arg(long)]
    pub nodevice: Option<String>,
    #[arg(long)]
    pub share: Option<String>,
    #[arg(long)]
    pub unshare: Option<String>,
    #[arg(long)]
    pub talk_name: Option<String>,
    #[arg(long)]
    pub no_talk_name: Option<String>,
    #[arg(long)]
    pub reset: bool,
    #[arg(long, name = "no-network")]
    pub no_network: bool,
}

#[derive(Debug, Parser)]
pub struct FlatpakRemoteAddArgs {
    pub name: String,
    pub url: String,
    #[arg(long)]
    pub user: bool,
    #[arg(long)]
    pub system: bool,
}

#[derive(Debug, Parser)]
pub struct FlatpakRemoteRemoveArgs {
    pub name: String,
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Parser)]
pub struct FlatpakRollbackArgs {
    pub app_id: String,
}

#[derive(Debug, Parser)]
pub struct FlatpakHistoryArgs {
    pub app_id: String,
    #[arg(long)]
    pub limit: Option<usize>,
}

#[derive(Debug, Parser)]
pub struct FlatpakGcArgs {
    #[arg(long)]
    pub unused_runtimes: bool,
    #[arg(long)]
    pub dry_run: bool,
}

// ── brew ────────────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum BrewCommand {
    /// Search Homebrew for formulae and casks.
    Search(BrewSearchArgs),
    /// Install a Homebrew formula or cask.
    Install(BrewInstallArgs),
    /// Remove a Homebrew formula or cask.
    Remove(BrewRemoveArgs),
    /// List installed Homebrew packages.
    List(BrewListArgs),
    /// Update Homebrew packages.
    Update(BrewUpdateArgs),
    /// Show detailed information.
    Info(BrewInfoArgs),
    /// Add a Homebrew tap.
    Tap(BrewTapArgs),
    /// Remove a Homebrew tap.
    Untap(BrewUntapArgs),
    /// Pin a formula.
    Pin(BrewPinArgs),
    /// Unpin a formula.
    Unpin(BrewUnpinArgs),
    /// Rollback to previous version.
    Rollback(BrewRollbackArgs),
}

#[derive(Debug, Parser)]
pub struct BrewSearchArgs {
    pub query: String,
    #[arg(long)]
    pub formula: bool,
    #[arg(long)]
    pub cask: bool,
}

#[derive(Debug, Parser)]
pub struct BrewInstallArgs {
    pub package: String,
    #[arg(long)]
    pub cask: bool,
    #[arg(long)]
    pub formula: bool,
}

#[derive(Debug, Parser)]
pub struct BrewRemoveArgs {
    pub package: String,
    #[arg(long)]
    pub cask: bool,
    #[arg(long)]
    pub formula: bool,
}

#[derive(Debug, Parser)]
pub struct BrewListArgs {
    #[arg(long)]
    pub formula: bool,
    #[arg(long)]
    pub cask: bool,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Parser)]
pub struct BrewUpdateArgs {
    pub package: Option<String>,
    #[arg(long)]
    pub check: bool,
    #[arg(long)]
    pub cask: bool,
    #[arg(long)]
    pub formula: bool,
}

#[derive(Debug, Parser)]
pub struct BrewInfoArgs {
    pub package: String,
}

#[derive(Debug, Parser)]
pub struct BrewTapArgs {
    pub tap_name: String,
}

#[derive(Debug, Parser)]
pub struct BrewUntapArgs {
    pub tap_name: String,
}

#[derive(Debug, Parser)]
pub struct BrewPinArgs {
    pub formula: String,
}

#[derive(Debug, Parser)]
pub struct BrewUnpinArgs {
    pub formula: String,
}

#[derive(Debug, Parser)]
pub struct BrewRollbackArgs {
    pub package: String,
}

// ── snap ────────────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum SnapCommand {
    /// Search the Snap Store.
    Search(SnapSearchArgs),
    /// Install a snap package.
    Install(SnapInstallArgs),
    /// Remove a snap package.
    Remove(SnapRemoveArgs),
    /// List installed snaps.
    List(SnapListArgs),
    /// Update snap packages.
    Update(SnapUpdateArgs),
    /// Show detailed snap information.
    Info(SnapInfoArgs),
    /// List interface connections.
    Connections(SnapConnectionsArgs),
    /// Connect a snap interface plug.
    Connect(SnapConnectArgs),
    /// Disconnect a snap interface plug.
    Disconnect(SnapDisconnectArgs),
    /// Revert to previous revision.
    Revert(SnapRevertArgs),
    /// Hold automatic snap refreshes.
    RefreshHold(SnapRefreshHoldArgs),
    /// Show available channels.
    Channels(SnapChannelsArgs),
}

#[derive(Debug, Parser)]
pub struct SnapSearchArgs {
    pub query: String,
}

#[derive(Debug, Parser)]
pub struct SnapInstallArgs {
    pub snap: String,
    #[arg(long)]
    pub channel: Option<String>,
    #[arg(long)]
    pub classic: bool,
    #[arg(long)]
    pub devmode: bool,
}

#[derive(Debug, Parser)]
pub struct SnapRemoveArgs {
    pub snap: String,
    #[arg(long)]
    pub purge: bool,
}

#[derive(Debug, Parser)]
pub struct SnapListArgs {
    #[arg(long)]
    pub all: bool,
}

#[derive(Debug, Parser)]
pub struct SnapUpdateArgs {
    pub snap: Option<String>,
    #[arg(long)]
    pub check: bool,
    #[arg(long)]
    pub channel: Option<String>,
}

#[derive(Debug, Parser)]
pub struct SnapInfoArgs {
    pub snap: String,
}

#[derive(Debug, Parser)]
pub struct SnapConnectionsArgs {
    pub snap: String,
}

#[derive(Debug, Parser)]
pub struct SnapConnectArgs {
    pub snap: String,
    pub interface: String,
}

#[derive(Debug, Parser)]
pub struct SnapDisconnectArgs {
    pub snap: String,
    pub interface: String,
}

#[derive(Debug, Parser)]
pub struct SnapRevertArgs {
    pub snap: String,
}

#[derive(Debug, Parser)]
pub struct SnapRefreshHoldArgs {
    pub snap: String,
    /// Duration in hours.
    #[arg(long)]
    pub duration: u64,
}

#[derive(Debug, Parser)]
pub struct SnapChannelsArgs {
    pub snap: String,
}

// ── nix ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum NixCommand {
    /// Search nixpkgs for packages.
    Search(NixSearchArgs),
    /// Install a Nix package.
    Install(NixInstallArgs),
    /// Remove a Nix package.
    Remove(NixRemoveArgs),
    /// List installed Nix packages.
    List(NixListArgs),
    /// Update Nix packages.
    Update(NixUpdateArgs),
    /// Rollback to previous profile generation.
    Rollback(NixRollbackArgs),
    /// Garbage-collect unused store paths.
    Gc(NixGcArgs),
    /// Enter a Nix development shell.
    Develop(NixDevelopArgs),
}

#[derive(Debug, Parser)]
pub struct NixSearchArgs {
    pub query: String,
    #[arg(long)]
    pub flake: Option<String>,
}

#[derive(Debug, Parser)]
pub struct NixInstallArgs {
    pub package: String,
    #[arg(long)]
    pub profile: Option<String>,
}

#[derive(Debug, Parser)]
pub struct NixRemoveArgs {
    pub package: String,
}

#[derive(Debug, Parser)]
pub struct NixListArgs {
    #[arg(long)]
    pub profile: Option<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Parser)]
pub struct NixUpdateArgs {
    pub package: Option<String>,
    #[arg(long)]
    pub profile: Option<String>,
    #[arg(long)]
    pub check: bool,
}

#[derive(Debug, Parser)]
pub struct NixRollbackArgs {
    #[arg(long)]
    pub profile: Option<String>,
}

#[derive(Debug, Parser)]
pub struct NixGcArgs {
    #[arg(long)]
    pub older_than: Option<u64>,
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Parser)]
pub struct NixDevelopArgs {
    #[arg(long)]
    pub flake: Option<String>,
}

// ── appimage ────────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum AppimageCommand {
    /// List integrated AppImage files.
    List,
    /// Check for and apply AppImage updates.
    Update(AppimageUpdateArgs),
    /// Integrate an AppImage into the desktop.
    Integrate(AppimageIntegrateArgs),
    /// Remove an integrated AppImage.
    Remove(AppimageRemoveArgs),
    /// Verify an AppImage signature.
    Verify(AppimageVerifyArgs),
}

#[derive(Debug, Parser)]
pub struct AppimageUpdateArgs {
    pub app: Option<String>,
    #[arg(long)]
    pub check: bool,
}

#[derive(Debug, Parser)]
pub struct AppimageIntegrateArgs {
    pub file: String,
}

#[derive(Debug, Parser)]
pub struct AppimageRemoveArgs {
    pub app: String,
}

#[derive(Debug, Parser)]
pub struct AppimageVerifyArgs {
    pub file: String,
}
