//! Application sandboxing architecture.
//!
//! Provides DOM isolation for applications. System applications have access to
//! the main desktop DOM, while regular applications run in isolated sandboxes
//! with their own DOM instances.

use liquide_dom::Document;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tracing::{debug, info};

/// Application sandbox isolation level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxLevel {
    /// System application with full desktop DOM access.
    System,
    /// Isolated application with private DOM.
    Isolated,
}

/// Application identifier.
pub type AppId = String;

/// A sandboxed application context.
pub struct AppSandbox {
    /// Application identifier.
    pub app_id: AppId,
    /// Sandbox isolation level.
    pub level: SandboxLevel,
    /// The application's isolated DOM (None for system apps).
    pub document: Option<Document>,
}

impl AppSandbox {
    /// Create a new system application (no isolation).
    pub fn system(app_id: AppId) -> Self {
        info!("Creating system app sandbox: {}", app_id);
        Self {
            app_id,
            level: SandboxLevel::System,
            document: None,
        }
    }
    
    /// Create a new isolated application with private DOM.
    pub fn isolated(app_id: AppId) -> Self {
        info!("Creating isolated app sandbox: {}", app_id);
        Self {
            app_id,
            level: SandboxLevel::Isolated,
            document: Some(Document::new()),
        }
    }
    
    /// Check if this app has system privileges.
    pub fn is_system(&self) -> bool {
        self.level == SandboxLevel::System
    }
}

/// Manager for all application sandboxes.
pub struct SandboxManager {
    /// Map of app_id -> sandbox.
    sandboxes: Arc<RwLock<HashMap<AppId, AppSandbox>>>,
    /// List of known system applications.
    system_apps: Vec<String>,
}

impl SandboxManager {
    /// Create a new sandbox manager.
    pub fn new() -> Self {
        Self {
            sandboxes: Arc::new(RwLock::new(HashMap::new())),
            system_apps: Self::default_system_apps(),
        }
    }
    
    /// Default system applications with full desktop DOM access.
    fn default_system_apps() -> Vec<String> {
        vec![
            "com.liquide.files".to_string(),
            "com.liquide.terminal".to_string(),
            "com.liquide.settings".to_string(),
            "com.liquide.system-monitor".to_string(),
            "com.liquide.dock".to_string(),
            "com.liquide.statusbar".to_string(),
            "com.liquide.launcher".to_string(),
            "com.liquide.notifications".to_string(),
        ]
    }
    
    /// Check if an app is a system app.
    pub fn is_system_app(&self, app_id: &str) -> bool {
        self.system_apps.iter().any(|s| s == app_id)
    }
    
    /// Register a new application.
    pub fn register_app(&self, app_id: AppId) {
        let sandbox = if self.is_system_app(&app_id) {
            AppSandbox::system(app_id.clone())
        } else {
            AppSandbox::isolated(app_id.clone())
        };
        
        let mut sandboxes = self.sandboxes.write().unwrap();
        sandboxes.insert(app_id, sandbox);
    }
    
    /// Unregister an application.
    pub fn unregister_app(&self, app_id: &str) {
        let mut sandboxes = self.sandboxes.write().unwrap();
        if sandboxes.remove(app_id).is_some() {
            debug!("Unregistered app: {}", app_id);
        }
    }
    
    /// Get an application's sandbox.
    pub fn get_sandbox(&self, app_id: &str) -> Option<SandboxLevel> {
        let sandboxes = self.sandboxes.read().unwrap();
        sandboxes.get(app_id).map(|s| s.level)
    }
    
    /// Get an application's isolated document (if it has one).
    ///
    /// Note: Returns None because Document doesn't implement Clone.
    /// Use `with_sandbox` instead to access the document.
    pub fn get_document(&self, _app_id: &str) -> Option<()> {
        // Document doesn't implement Clone, so we can't return it.
        // Use with_sandbox or with_sandbox_mut to access the document.
        None
    }
    
    /// Execute a function within an app's sandbox context.
    pub fn with_sandbox<F, R>(&self, app_id: &str, f: F) -> Option<R>
    where
        F: FnOnce(&AppSandbox) -> R,
    {
        let sandboxes = self.sandboxes.read().unwrap();
        sandboxes.get(app_id).map(f)
    }
    
    /// Execute a mutable function within an app's sandbox context.
    pub fn with_sandbox_mut<F, R>(&self, app_id: &str, f: F) -> Option<R>
    where
        F: FnOnce(&mut AppSandbox) -> R,
    {
        let mut sandboxes = self.sandboxes.write().unwrap();
        sandboxes.get_mut(app_id).map(f)
    }
    
    /// Get statistics about active sandboxes.
    pub fn stats(&self) -> SandboxStats {
        let sandboxes = self.sandboxes.read().unwrap();
        let total = sandboxes.len();
        let system = sandboxes.values().filter(|s| s.is_system()).count();
        let isolated = total - system;
        
        SandboxStats {
            total,
            system,
            isolated,
        }
    }
    
    /// Add a custom system application.
    pub fn add_system_app(&mut self, app_id: String) {
        if !self.system_apps.contains(&app_id) {
            info!("Adding system app: {}", app_id);
            self.system_apps.push(app_id);
        }
    }
    
    /// Clear all sandboxes (useful for testing).
    pub fn clear(&self) {
        let mut sandboxes = self.sandboxes.write().unwrap();
        sandboxes.clear();
        debug!("Cleared all sandboxes");
    }
}

impl Default for SandboxManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about active sandboxes.
#[derive(Debug, Clone, Copy)]
pub struct SandboxStats {
    /// Total number of sandboxes.
    pub total: usize,
    /// Number of system apps.
    pub system: usize,
    /// Number of isolated apps.
    pub isolated: usize,
}

// ── Tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_app_detection() {
        let manager = SandboxManager::new();
        assert!(manager.is_system_app("com.liquide.files"));
        assert!(manager.is_system_app("com.liquide.terminal"));
        assert!(!manager.is_system_app("com.example.app"));
    }

    #[test]
    fn test_sandbox_registration() {
        let manager = SandboxManager::new();
        
        // Register a system app
        manager.register_app("com.liquide.files".to_string());
        assert_eq!(manager.get_sandbox("com.liquide.files"), Some(SandboxLevel::System));
        
        // Register a regular app
        manager.register_app("com.example.app".to_string());
        assert_eq!(manager.get_sandbox("com.example.app"), Some(SandboxLevel::Isolated));
    }

    #[test]
    fn test_sandbox_unregistration() {
        let manager = SandboxManager::new();
        manager.register_app("com.example.app".to_string());
        assert!(manager.get_sandbox("com.example.app").is_some());
        
        manager.unregister_app("com.example.app");
        assert!(manager.get_sandbox("com.example.app").is_none());
    }

    #[test]
    fn test_isolated_app_has_document() {
        let manager = SandboxManager::new();
        manager.register_app("com.example.app".to_string());
        
        // Check that the sandbox exists and is isolated
        manager.with_sandbox("com.example.app", |sandbox| {
            assert!(sandbox.document.is_some());
        });
    }

    #[test]
    fn test_system_app_no_document() {
        let manager = SandboxManager::new();
        manager.register_app("com.liquide.files".to_string());
        
        // Check that the sandbox exists but has no document (system app)
        manager.with_sandbox("com.liquide.files", |sandbox| {
            assert!(sandbox.document.is_none());
        });
    }

    #[test]
    fn test_sandbox_stats() {
        let manager = SandboxManager::new();
        manager.register_app("com.liquide.files".to_string());
        manager.register_app("com.example.app1".to_string());
        manager.register_app("com.example.app2".to_string());
        
        let stats = manager.stats();
        assert_eq!(stats.total, 3);
        assert_eq!(stats.system, 1);
        assert_eq!(stats.isolated, 2);
    }

    #[test]
    fn test_add_custom_system_app() {
        let mut manager = SandboxManager::new();
        manager.add_system_app("com.custom.system".to_string());
        assert!(manager.is_system_app("com.custom.system"));
        
        manager.register_app("com.custom.system".to_string());
        assert_eq!(manager.get_sandbox("com.custom.system"), Some(SandboxLevel::System));
    }
}
