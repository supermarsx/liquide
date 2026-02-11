//! App catalog with categories, featured, and search.

use crate::package::{AppCategory, PackageInfo};

/// Search result with relevance score.
#[derive(Debug, Clone)]
pub struct CatalogResult {
    pub package_id: String,
    pub name: String,
    pub summary: String,
    pub category: AppCategory,
    pub score: u32,
    pub installed: bool,
}

/// App catalog managing the full package list.
pub struct Catalog {
    packages: Vec<PackageInfo>,
    featured_ids: Vec<String>,
}

impl Catalog {
    #[must_use]
    pub fn new() -> Self {
        Self { packages: Vec::new(), featured_ids: Vec::new() }
    }

    /// Load packages into the catalog.
    pub fn load(&mut self, packages: Vec<PackageInfo>) {
        self.packages = packages;
    }

    /// Set featured package IDs.
    pub fn set_featured(&mut self, ids: Vec<String>) {
        self.featured_ids = ids;
    }

    /// Get all packages.
    #[must_use]
    pub fn all_packages(&self) -> &[PackageInfo] { &self.packages }

    /// Total number of packages.
    #[must_use]
    pub fn total_count(&self) -> usize { self.packages.len() }

    /// Get a package by ID.
    #[must_use]
    pub fn find(&self, id: &str) -> Option<&PackageInfo> {
        self.packages.iter().find(|p| p.id == id)
    }

    /// Get a mutable package by ID.
    pub fn find_mut(&mut self, id: &str) -> Option<&mut PackageInfo> {
        self.packages.iter_mut().find(|p| p.id == id)
    }

    /// Get packages in a specific category.
    #[must_use]
    pub fn by_category(&self, category: AppCategory) -> Vec<&PackageInfo> {
        self.packages.iter().filter(|p| p.category == category).collect()
    }

    /// Get featured packages.
    #[must_use]
    pub fn featured(&self) -> Vec<&PackageInfo> {
        self.featured_ids.iter()
            .filter_map(|id| self.find(id))
            .collect()
    }

    /// Get installed packages.
    #[must_use]
    pub fn installed(&self) -> Vec<&PackageInfo> {
        self.packages.iter().filter(|p| p.installed).collect()
    }

    /// Get packages with available updates.
    #[must_use]
    pub fn updatable(&self) -> Vec<&PackageInfo> {
        self.packages.iter().filter(|p| p.has_update()).collect()
    }

    /// Search packages by query.
    #[must_use]
    pub fn search(&self, query: &str) -> Vec<CatalogResult> {
        let q = query.to_lowercase();
        if q.is_empty() { return Vec::new(); }

        let mut results: Vec<CatalogResult> = self.packages.iter()
            .filter_map(|p| {
                let mut score = 0u32;
                let name_lower = p.name.to_lowercase();
                let summary_lower = p.summary.to_lowercase();

                if name_lower == q { score += 100; }
                else if name_lower.starts_with(&q) { score += 80; }
                else if name_lower.contains(&q) { score += 50; }

                if summary_lower.contains(&q) { score += 20; }
                if p.id.to_lowercase().contains(&q) { score += 10; }

                if score > 0 {
                    Some(CatalogResult {
                        package_id: p.id.clone(),
                        name: p.name.clone(),
                        summary: p.summary.clone(),
                        category: p.category,
                        score,
                        installed: p.installed,
                    })
                } else {
                    None
                }
            })
            .collect();

        results.sort_by(|a, b| b.score.cmp(&a.score));
        results
    }

    /// Count of installed packages.
    #[must_use]
    pub fn installed_count(&self) -> usize {
        self.packages.iter().filter(|p| p.installed).count()
    }

    /// Count of packages with updates.
    #[must_use]
    pub fn update_count(&self) -> usize {
        self.packages.iter().filter(|p| p.has_update()).count()
    }
}

impl Default for Catalog {
    fn default() -> Self { Self::new() }
}
