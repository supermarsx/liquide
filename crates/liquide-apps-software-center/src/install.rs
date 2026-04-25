//! Installation, removal, and progress tracking.

use std::fmt;
use std::process::{Command, ExitStatus};

/// Installation operation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallAction {
    Install,
    Remove,
    Update,
}

impl fmt::Display for InstallAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Install => f.write_str("install"),
            Self::Remove => f.write_str("remove"),
            Self::Update => f.write_str("update"),
        }
    }
}

/// External package-management backend selected for a package action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageSource {
    Winget,
    Apt,
    Flatpak,
}

impl fmt::Display for PackageSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Winget => f.write_str("winget"),
            Self::Apt => f.write_str("apt"),
            Self::Flatpak => f.write_str("flatpak"),
        }
    }
}

/// Concrete shell command used to dispatch an install/remove/update action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
}

impl CommandSpec {
    #[must_use]
    pub fn new(program: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            program: program.into(),
            args,
        }
    }

    #[must_use]
    pub fn display_string(&self) -> String {
        if self.args.is_empty() {
            self.program.clone()
        } else {
            format!("{} {}", self.program, self.args.join(" "))
        }
    }

    pub fn execute(&self) -> crate::Result<ExitStatus> {
        let status = Command::new(&self.program)
            .args(&self.args)
            .status()
            .map_err(|e| crate::SoftwareCenterError::Io(e.to_string()))?;

        if status.success() {
            Ok(status)
        } else {
            Err(crate::SoftwareCenterError::BackendCommand(format!(
                "{} exited with status {status}",
                self.display_string()
            )))
        }
    }
}

/// Build the package-manager command for a specific action.
#[must_use]
pub fn build_command(
    action: InstallAction,
    source: PackageSource,
    package_id: &str,
) -> CommandSpec {
    match (source, action) {
        (PackageSource::Winget, InstallAction::Install) => CommandSpec::new(
            "winget",
            vec![
                "install".into(),
                "--id".into(),
                package_id.into(),
                "--accept-package-agreements".into(),
                "--accept-source-agreements".into(),
            ],
        ),
        (PackageSource::Winget, InstallAction::Remove) => CommandSpec::new(
            "winget",
            vec!["uninstall".into(), "--id".into(), package_id.into()],
        ),
        (PackageSource::Winget, InstallAction::Update) => CommandSpec::new(
            "winget",
            vec!["upgrade".into(), "--id".into(), package_id.into()],
        ),
        (PackageSource::Apt, InstallAction::Install) => CommandSpec::new(
            "apt-get",
            vec!["install".into(), "-y".into(), package_id.into()],
        ),
        (PackageSource::Apt, InstallAction::Remove) => CommandSpec::new(
            "apt-get",
            vec!["remove".into(), "-y".into(), package_id.into()],
        ),
        (PackageSource::Apt, InstallAction::Update) => CommandSpec::new(
            "apt-get",
            vec![
                "install".into(),
                "--only-upgrade".into(),
                "-y".into(),
                package_id.into(),
            ],
        ),
        (PackageSource::Flatpak, InstallAction::Install) => CommandSpec::new(
            "flatpak",
            vec!["install".into(), "-y".into(), package_id.into()],
        ),
        (PackageSource::Flatpak, InstallAction::Remove) => CommandSpec::new(
            "flatpak",
            vec!["uninstall".into(), "-y".into(), package_id.into()],
        ),
        (PackageSource::Flatpak, InstallAction::Update) => CommandSpec::new(
            "flatpak",
            vec!["update".into(), "-y".into(), package_id.into()],
        ),
    }
}

/// State of an installation operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallState {
    Queued,
    Downloading,
    Installing,
    Removing,
    Completed,
    Failed,
}

/// A pending or in-progress installation operation.
#[derive(Debug, Clone)]
pub struct InstallOperation {
    pub package_id: String,
    pub package_name: String,
    pub action: InstallAction,
    pub state: InstallState,
    /// Download progress (0.0..=1.0).
    pub download_progress: f64,
    /// Install progress (0.0..=1.0).
    pub install_progress: f64,
    /// Error message if failed.
    pub error: Option<String>,
}

impl InstallOperation {
    #[must_use]
    pub fn new(
        package_id: impl Into<String>,
        package_name: impl Into<String>,
        action: InstallAction,
    ) -> Self {
        Self {
            package_id: package_id.into(),
            package_name: package_name.into(),
            action,
            state: InstallState::Queued,
            download_progress: 0.0,
            install_progress: 0.0,
            error: None,
        }
    }

    /// Overall progress (0.0..=1.0).
    #[must_use]
    pub fn overall_progress(&self) -> f64 {
        match self.action {
            InstallAction::Install | InstallAction::Update => {
                (self.download_progress + self.install_progress) / 2.0
            }
            InstallAction::Remove => self.install_progress,
        }
    }

    /// Whether the operation is terminal (completed or failed).
    #[must_use]
    pub fn is_done(&self) -> bool {
        matches!(self.state, InstallState::Completed | InstallState::Failed)
    }

    /// Mark as downloading with progress.
    pub fn set_downloading(&mut self, progress: f64) {
        self.state = InstallState::Downloading;
        self.download_progress = progress;
    }

    /// Mark as installing with progress.
    pub fn set_installing(&mut self, progress: f64) {
        self.state = InstallState::Installing;
        self.install_progress = progress;
    }

    /// Mark as completed.
    pub fn complete(&mut self) {
        self.state = InstallState::Completed;
        self.download_progress = 1.0;
        self.install_progress = 1.0;
    }

    /// Mark as failed.
    pub fn fail(&mut self, error: impl Into<String>) {
        self.state = InstallState::Failed;
        self.error = Some(error.into());
    }
}

/// Queue of installation operations.
pub struct InstallQueue {
    operations: Vec<InstallOperation>,
}

impl InstallQueue {
    #[must_use]
    pub fn new() -> Self {
        Self {
            operations: Vec::new(),
        }
    }

    /// Add an operation to the queue.
    pub fn enqueue(&mut self, op: InstallOperation) {
        self.operations.push(op);
    }

    /// Get all operations.
    #[must_use]
    pub fn operations(&self) -> &[InstallOperation] {
        &self.operations
    }

    /// Get active (non-terminal) operations.
    #[must_use]
    pub fn active(&self) -> Vec<&InstallOperation> {
        self.operations.iter().filter(|o| !o.is_done()).collect()
    }

    /// Get completed operations.
    #[must_use]
    pub fn completed(&self) -> Vec<&InstallOperation> {
        self.operations
            .iter()
            .filter(|o| o.state == InstallState::Completed)
            .collect()
    }

    /// Get failed operations.
    #[must_use]
    pub fn failed(&self) -> Vec<&InstallOperation> {
        self.operations
            .iter()
            .filter(|o| o.state == InstallState::Failed)
            .collect()
    }

    /// Find an operation by package ID.
    pub fn find_mut(&mut self, package_id: &str) -> Option<&mut InstallOperation> {
        self.operations
            .iter_mut()
            .find(|o| o.package_id == package_id)
    }

    /// Remove completed operations from the queue.
    pub fn clear_completed(&mut self) {
        self.operations.retain(|o| !o.is_done());
    }

    /// Number of queued operations.
    #[must_use]
    pub fn count(&self) -> usize {
        self.operations.len()
    }

    /// Number of active operations.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.operations.iter().filter(|o| !o.is_done()).count()
    }
}

impl Default for InstallQueue {
    fn default() -> Self {
        Self::new()
    }
}
