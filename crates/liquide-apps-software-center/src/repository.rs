//! Repository sources and management.

use serde::{Deserialize, Serialize};

/// Repository type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepoType {
    /// Official LiquiDE repository.
    Official,
    /// Community-maintained repository.
    Community,
    /// Third-party PPA or overlay.
    ThirdParty,
    /// Flatpak remote.
    Flatpak,
}

impl std::fmt::Display for RepoType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Official => f.write_str("official"),
            Self::Community => f.write_str("community"),
            Self::ThirdParty => f.write_str("third-party"),
            Self::Flatpak => f.write_str("flatpak"),
        }
    }
}

/// A software repository source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    /// Unique repository ID.
    pub id: String,
    /// Display name.
    pub name: String,
    /// URL or URI of the repository.
    pub url: String,
    /// Repository type.
    pub repo_type: RepoType,
    /// Whether the repository is enabled.
    pub enabled: bool,
    /// Number of packages available.
    pub package_count: usize,
    /// Last refresh timestamp.
    pub last_refresh: u64,
}

impl Repository {
    /// Create a new repository.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        url: impl Into<String>,
        repo_type: RepoType,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            url: url.into(),
            repo_type,
            enabled: true,
            package_count: 0,
            last_refresh: 0,
        }
    }
}

/// Repository manager.
pub struct RepoManager {
    repos: Vec<Repository>,
}

impl RepoManager {
    /// Create a repository manager with default repositories.
    #[must_use]
    pub fn new() -> Self {
        Self {
            repos: vec![
                Repository::new("official", "LiquiDE Official", "https://packages.liquide.org/stable", RepoType::Official),
                Repository::new("community", "LiquiDE Community", "https://packages.liquide.org/community", RepoType::Community),
                Repository::new("flatpak", "Flathub", "https://flathub.org/repo", RepoType::Flatpak),
            ],
        }
    }

    /// Get all repositories.
    #[must_use]
    pub fn repositories(&self) -> &[Repository] { &self.repos }

    /// Get enabled repositories.
    #[must_use]
    pub fn enabled_repos(&self) -> Vec<&Repository> {
        self.repos.iter().filter(|r| r.enabled).collect()
    }

    /// Find a repository by ID.
    #[must_use]
    pub fn find(&self, id: &str) -> Option<&Repository> {
        self.repos.iter().find(|r| r.id == id)
    }

    /// Add a repository.
    pub fn add(&mut self, repo: Repository) {
        // Avoid duplicates by ID.
        if !self.repos.iter().any(|r| r.id == repo.id) {
            self.repos.push(repo);
        }
    }

    /// Remove a repository by ID.
    pub fn remove(&mut self, id: &str) -> crate::Result<()> {
        let pos = self.repos.iter().position(|r| r.id == id)
            .ok_or_else(|| crate::SoftwareCenterError::RepositoryNotFound(id.into()))?;
        self.repos.remove(pos);
        Ok(())
    }

    /// Toggle a repository's enabled state.
    pub fn toggle(&mut self, id: &str) -> crate::Result<bool> {
        let repo = self.repos.iter_mut().find(|r| r.id == id)
            .ok_or_else(|| crate::SoftwareCenterError::RepositoryNotFound(id.into()))?;
        repo.enabled = !repo.enabled;
        Ok(repo.enabled)
    }

    /// Number of repositories.
    #[must_use]
    pub fn count(&self) -> usize { self.repos.len() }
}

impl Default for RepoManager {
    fn default() -> Self { Self::new() }
}
