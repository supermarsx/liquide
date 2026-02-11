//! Software center coordinator.

use crate::catalog::Catalog;
use crate::config::SoftwareCenterConfig;
use crate::install::{InstallAction, InstallOperation, InstallQueue};
use crate::package::PackageInfo;
use crate::repository::RepoManager;
use crate::update::UpdateManager;

/// The software center runtime coordinating all subsystems.
pub struct SoftwareCenterRuntime {
    config: SoftwareCenterConfig,
    catalog: Catalog,
    repos: RepoManager,
    queue: InstallQueue,
    updates: UpdateManager,
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
        }
    }

    // ---- Catalog ----

    /// Load packages into the catalog.
    pub fn load_packages(&mut self, packages: Vec<PackageInfo>) {
        self.catalog.load(packages);
    }

    /// Get the catalog.
    #[must_use]
    pub fn catalog(&self) -> &Catalog { &self.catalog }

    /// Get mutable access to the catalog.
    pub fn catalog_mut(&mut self) -> &mut Catalog { &mut self.catalog }

    // ---- Repos ----

    /// Get the repository manager.
    #[must_use]
    pub fn repos(&self) -> &RepoManager { &self.repos }

    /// Get mutable access to the repository manager.
    pub fn repos_mut(&mut self) -> &mut RepoManager { &mut self.repos }

    // ---- Install/Remove ----

    /// Install a package by ID.
    pub fn install(&mut self, package_id: &str) -> crate::Result<()> {
        let pkg = self.catalog.find(package_id)
            .ok_or_else(|| crate::SoftwareCenterError::PackageNotFound(package_id.into()))?;

        if pkg.installed {
            return Err(crate::SoftwareCenterError::AlreadyInstalled(package_id.into()));
        }

        let op = InstallOperation::new(&pkg.id, &pkg.name, InstallAction::Install);
        self.queue.enqueue(op);
        Ok(())
    }

    /// Remove a package by ID.
    pub fn remove(&mut self, package_id: &str) -> crate::Result<()> {
        let pkg = self.catalog.find(package_id)
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
        let pkg = self.catalog.find(package_id)
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
    pub fn queue(&self) -> &InstallQueue { &self.queue }

    /// Get mutable access to the install queue.
    pub fn queue_mut(&mut self) -> &mut InstallQueue { &mut self.queue }

    // ---- Updates ----

    /// Get the update manager.
    #[must_use]
    pub fn updates(&self) -> &UpdateManager { &self.updates }

    /// Get mutable access to the update manager.
    pub fn updates_mut(&mut self) -> &mut UpdateManager { &mut self.updates }

    // ---- Config ----

    /// Get the configuration.
    #[must_use]
    pub fn config(&self) -> &SoftwareCenterConfig { &self.config }
}
