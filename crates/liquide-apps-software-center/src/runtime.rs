//! Software center coordinator.

use crate::catalog::Catalog;
use crate::config::SoftwareCenterConfig;
use crate::install::{
    CommandSpec, InstallAction, InstallOperation, InstallQueue, PackageSource, build_command,
};
use crate::package::PackageInfo;
use crate::repository::{RepoManager, RepoType};
use crate::update::UpdateManager;

/// The software center runtime coordinating all subsystems.
pub struct SoftwareCenterRuntime {
    config: SoftwareCenterConfig,
    catalog: Catalog,
    repos: RepoManager,
    queue: InstallQueue,
    updates: UpdateManager,
    search_query: String,
    /// The currently-selected package id in the list UI, if any.
    selected_id: Option<String>,
}

impl SoftwareCenterRuntime {
    /// Create a new software center runtime.
    #[must_use]
    pub fn new(config: SoftwareCenterConfig) -> Self {
        let auto_check = config.auto_check_updates;
        Self {
            config,
            catalog: Catalog::new(),
            repos: RepoManager::new(),
            queue: InstallQueue::new(),
            updates: UpdateManager::new(auto_check),
            search_query: String::new(),
            selected_id: None,
        }
    }

    /// Current free-text package search query.
    #[must_use]
    pub fn search_query(&self) -> &str {
        &self.search_query
    }

    /// Mutable access to the search query (used by the app-view seam).
    pub(crate) fn search_query_mut(&mut self) -> &mut String {
        &mut self.search_query
    }

    /// Replace the free-text search query wholesale.
    pub fn set_search_query(&mut self, query: impl Into<String>) {
        self.search_query = query.into();
    }

    /// The id of the currently-selected package, if any.
    #[must_use]
    pub fn selected_id(&self) -> Option<&str> {
        self.selected_id.as_deref()
    }

    /// Select a package by id. Selecting an unknown id clears the selection.
    pub fn select_package(&mut self, id: &str) {
        if self.catalog.find(id).is_some() {
            self.selected_id = Some(id.to_string());
        } else {
            self.selected_id = None;
        }
    }

    /// The packages currently matching the search query, in catalog order. An
    /// empty query returns the full catalog. This is the single source of truth
    /// for the list shown in the widget UI.
    #[must_use]
    pub fn visible_packages(&self) -> Vec<&PackageInfo> {
        let needle = self.search_query.to_lowercase();
        self.catalog
            .all_packages()
            .iter()
            .filter(|p| {
                needle.is_empty()
                    || p.name.to_lowercase().contains(&needle)
                    || p.summary.to_lowercase().contains(&needle)
            })
            .collect()
    }

    // ---- Catalog ----

    /// Load packages into the catalog.
    pub fn load_packages(&mut self, packages: Vec<PackageInfo>) {
        self.catalog.load(packages);
    }

    /// Get the catalog.
    #[must_use]
    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    /// Get mutable access to the catalog.
    pub fn catalog_mut(&mut self) -> &mut Catalog {
        &mut self.catalog
    }

    // ---- Repos ----

    /// Get the repository manager.
    #[must_use]
    pub fn repos(&self) -> &RepoManager {
        &self.repos
    }

    /// Get mutable access to the repository manager.
    pub fn repos_mut(&mut self) -> &mut RepoManager {
        &mut self.repos
    }

    /// Fetch raw repository metadata over HTTP.
    pub fn fetch_repository_metadata(&self, repo_id: &str) -> crate::Result<String> {
        let repo = self
            .repos
            .find(repo_id)
            .ok_or_else(|| crate::SoftwareCenterError::RepositoryNotFound(repo_id.into()))?;

        let client = reqwest::blocking::Client::builder()
            .user_agent("liquide-software-center/0.1")
            .build()
            .map_err(|e| crate::SoftwareCenterError::Transport(e.to_string()))?;

        let response = client
            .get(&repo.url)
            .send()
            .map_err(|e| crate::SoftwareCenterError::Transport(e.to_string()))?
            .error_for_status()
            .map_err(|e| crate::SoftwareCenterError::Transport(e.to_string()))?;

        response
            .text()
            .map_err(|e| crate::SoftwareCenterError::Transport(e.to_string()))
    }

    /// Resolve the package backend from repository type + current platform.
    pub fn package_source(&self, package_id: &str) -> crate::Result<PackageSource> {
        let pkg = self
            .catalog
            .find(package_id)
            .ok_or_else(|| crate::SoftwareCenterError::PackageNotFound(package_id.into()))?;
        let repo = self.repos.find(&pkg.repository_id).ok_or_else(|| {
            crate::SoftwareCenterError::RepositoryNotFound(pkg.repository_id.clone())
        })?;

        match repo.repo_type {
            RepoType::Flatpak => Ok(PackageSource::Flatpak),
            _ => {
                #[cfg(target_os = "windows")]
                {
                    Ok(PackageSource::Winget)
                }
                #[cfg(target_os = "linux")]
                {
                    Ok(PackageSource::Apt)
                }
                #[cfg(not(any(target_os = "windows", target_os = "linux")))]
                {
                    Err(crate::SoftwareCenterError::UnsupportedBackend(format!(
                        "no package backend configured for target {}",
                        std::env::consts::OS
                    )))
                }
            }
        }
    }

    /// Build the external command used to install a package.
    pub fn install_command(&self, package_id: &str) -> crate::Result<CommandSpec> {
        Ok(build_command(
            InstallAction::Install,
            self.package_source(package_id)?,
            package_id,
        ))
    }

    /// Build the external command used to remove a package.
    pub fn remove_command(&self, package_id: &str) -> crate::Result<CommandSpec> {
        Ok(build_command(
            InstallAction::Remove,
            self.package_source(package_id)?,
            package_id,
        ))
    }

    /// Build the external command used to update a package.
    pub fn update_command(&self, package_id: &str) -> crate::Result<CommandSpec> {
        Ok(build_command(
            InstallAction::Update,
            self.package_source(package_id)?,
            package_id,
        ))
    }

    // ---- Install/Remove ----

    /// Install a package by ID.
    pub fn install(&mut self, package_id: &str) -> crate::Result<()> {
        let pkg = self
            .catalog
            .find(package_id)
            .ok_or_else(|| crate::SoftwareCenterError::PackageNotFound(package_id.into()))?;

        if pkg.installed {
            return Err(crate::SoftwareCenterError::AlreadyInstalled(
                package_id.into(),
            ));
        }

        let op = InstallOperation::new(&pkg.id, &pkg.name, InstallAction::Install);
        self.queue.enqueue(op);
        Ok(())
    }

    /// Remove a package by ID.
    pub fn remove(&mut self, package_id: &str) -> crate::Result<()> {
        let pkg = self
            .catalog
            .find(package_id)
            .ok_or_else(|| crate::SoftwareCenterError::PackageNotFound(package_id.into()))?;

        if !pkg.installed {
            return Err(crate::SoftwareCenterError::NotInstalled(package_id.into()));
        }

        let op = InstallOperation::new(&pkg.id, &pkg.name, InstallAction::Remove);
        self.queue.enqueue(op);
        Ok(())
    }

    /// Update a package by ID.
    pub fn update_package(&mut self, package_id: &str) -> crate::Result<()> {
        let pkg = self
            .catalog
            .find(package_id)
            .ok_or_else(|| crate::SoftwareCenterError::PackageNotFound(package_id.into()))?;

        if !pkg.installed {
            return Err(crate::SoftwareCenterError::NotInstalled(package_id.into()));
        }

        let op = InstallOperation::new(&pkg.id, &pkg.name, InstallAction::Update);
        self.queue.enqueue(op);
        Ok(())
    }

    // ---- Queue ----

    /// Get the install queue.
    #[must_use]
    pub fn queue(&self) -> &InstallQueue {
        &self.queue
    }

    /// Get mutable access to the install queue.
    pub fn queue_mut(&mut self) -> &mut InstallQueue {
        &mut self.queue
    }

    // ---- Updates ----

    /// Get the update manager.
    #[must_use]
    pub fn updates(&self) -> &UpdateManager {
        &self.updates
    }

    /// Get mutable access to the update manager.
    pub fn updates_mut(&mut self) -> &mut UpdateManager {
        &mut self.updates
    }

    // ---- Config ----

    /// Get the configuration.
    #[must_use]
    pub fn config(&self) -> &SoftwareCenterConfig {
        &self.config
    }
}
