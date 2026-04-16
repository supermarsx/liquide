use crate::{Plugin, PluginId, PluginCapability, PluginError as LibPluginError, PluginManifest};
use crate::manifest::{ExtensionPoint, Permission};
use std::collections::HashMap;
use std::path::PathBuf;

/// Plugin state in the registry
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginState {
    Discovered,   // found on disk
    Loaded,       // manifest parsed, ready to enable
    Enabled,      // active and running
    Disabled,     // explicitly disabled by user
    Error(String),
}

/// Error types for registry operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginError {
    NotFound(String),
    AlreadyExists,
    InvalidState,
    PermissionDenied,
    DependencyMissing(String),
}

impl std::fmt::Display for PluginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(id) => write!(f, "plugin not found: {}", id),
            Self::AlreadyExists => write!(f, "plugin already registered"),
            Self::InvalidState => write!(f, "plugin is in an invalid state for this operation"),
            Self::PermissionDenied => write!(f, "permission denied"),
            Self::DependencyMissing(dep) => write!(f, "missing dependency: {}", dep),
        }
    }
}
impl std::error::Error for PluginError {}

/// Info about a registered plugin
#[derive(Debug, Clone)]
pub struct PluginInfo {
    pub manifest: PluginManifest,
    pub state: PluginState,
    pub path: String,
    pub load_order: u32,
    pub enabled_by_default: bool,
}

/// Central plugin registry
pub struct PluginRegistry {
    plugins: HashMap<String, PluginInfo>,
    search_paths: Vec<PathBuf>,
    load_counter: u32,
    /// Map from plugin ID to trait-object instance (for built-in plugins)
    instances: HashMap<String, Box<dyn Plugin>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            search_paths: Vec::new(),
            load_counter: 0,
            instances: HashMap::new(),
        }
    }

    /// Add a directory to search for plugins
    pub fn add_search_path(&mut self, path: PathBuf) {
        if !self.search_paths.contains(&path) {
            self.search_paths.push(path);
        }
    }

    /// Scan given directories for plugin manifests (plugin.toml files in subdirectories)
    pub fn discover(&mut self, paths: &[String]) {
        for search_path in paths {
            let dir = PathBuf::from(search_path);
            if !dir.is_dir() {
                continue;
            }

            let entries = match std::fs::read_dir(&dir) {
                Ok(e) => e,
                Err(_) => continue,
            };

            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }

                let manifest_path = path.join("plugin.toml");
                if !manifest_path.exists() {
                    continue;
                }

                let content = match std::fs::read_to_string(&manifest_path) {
                    Ok(c) => c,
                    Err(_) => continue,
                };

                match crate::manifest::parse_manifest(&content) {
                    Ok(manifest) => {
                        let id = manifest.id.0.clone();
                        if !self.plugins.contains_key(&id) {
                            self.load_counter += 1;
                            self.plugins.insert(id, PluginInfo {
                                manifest,
                                state: PluginState::Discovered,
                                path: path.to_string_lossy().to_string(),
                                load_order: self.load_counter,
                                enabled_by_default: false,
                            });
                        }
                    }
                    Err(_) => continue,
                }
            }
        }
    }

    /// Also scan previously-added search paths (backward compat)
    pub fn discover_search_paths(&mut self) -> Vec<PluginId> {
        let mut discovered = Vec::new();

        for search_path in self.search_paths.clone() {
            if !search_path.is_dir() {
                continue;
            }

            let entries = match std::fs::read_dir(&search_path) {
                Ok(e) => e,
                Err(_) => continue,
            };

            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }

                let manifest_path = path.join("plugin.toml");
                if !manifest_path.exists() {
                    continue;
                }

                let content = match std::fs::read_to_string(&manifest_path) {
                    Ok(c) => c,
                    Err(_) => continue,
                };

                match crate::manifest::parse_manifest(&content) {
                    Ok(manifest) => {
                        let id_str = manifest.id.0.clone();
                        let id = manifest.id.clone();
                        if !self.plugins.contains_key(&id_str) {
                            self.load_counter += 1;
                            self.plugins.insert(id_str, PluginInfo {
                                manifest,
                                state: PluginState::Discovered,
                                path: path.to_string_lossy().to_string(),
                                load_order: self.load_counter,
                                enabled_by_default: false,
                            });
                            discovered.push(id);
                        }
                    }
                    Err(_) => continue,
                }
            }
        }

        discovered
    }

    /// Register a plugin info directly
    pub fn register(&mut self, info: PluginInfo) -> Result<(), PluginError> {
        let id = info.manifest.id.0.clone();
        if self.plugins.contains_key(&id) {
            return Err(PluginError::AlreadyExists);
        }
        self.plugins.insert(id, info);
        Ok(())
    }

    /// Register a built-in plugin instance directly
    pub fn register_builtin(&mut self, plugin: Box<dyn Plugin>) -> Result<(), LibPluginError> {
        let id = plugin.id().clone();
        let id_str = id.0.clone();
        if self.plugins.contains_key(&id_str) {
            return Err(LibPluginError::AlreadyLoaded(id));
        }

        let manifest = PluginManifest {
            id: id.clone(),
            name: plugin.name().to_string(),
            version: plugin.version().to_string(),
            description: String::new(),
            author: "built-in".to_string(),
            license: None,
            homepage: None,
            min_de_version: None,
            capabilities: plugin.capabilities(),
            extension_point: ExtensionPoint::Custom("builtin".into()),
            permissions: Vec::new(),
            config_schema: None,
            dependencies: Vec::new(),
            native_lib: None,
            script: None,
            icon: None,
        };

        self.load_counter += 1;
        self.plugins.insert(id_str.clone(), PluginInfo {
            manifest,
            state: PluginState::Enabled,
            path: String::new(),
            load_order: self.load_counter,
            enabled_by_default: true,
        });
        self.instances.insert(id_str, plugin);

        Ok(())
    }

    /// Enable a plugin by ID
    pub fn enable(&mut self, id: &str) -> Result<(), PluginError> {
        let info = self.plugins.get_mut(id)
            .ok_or_else(|| PluginError::NotFound(id.to_string()))?;

        match &info.state {
            PluginState::Enabled => Ok(()), // already enabled, no-op
            PluginState::Discovered | PluginState::Loaded | PluginState::Disabled => {
                info.state = PluginState::Enabled;
                Ok(())
            }
            PluginState::Error(_) => Err(PluginError::InvalidState),
        }
    }

    /// Disable a plugin by ID
    pub fn disable(&mut self, id: &str) -> Result<(), PluginError> {
        let info = self.plugins.get_mut(id)
            .ok_or_else(|| PluginError::NotFound(id.to_string()))?;

        match &info.state {
            PluginState::Disabled => Ok(()), // already disabled, no-op
            PluginState::Enabled | PluginState::Loaded | PluginState::Discovered => {
                info.state = PluginState::Disabled;
                Ok(())
            }
            PluginState::Error(_) => Err(PluginError::InvalidState),
        }
    }

    /// Uninstall a plugin by ID (remove from registry entirely)
    pub fn uninstall(&mut self, id: &str) -> Result<(), PluginError> {
        if self.plugins.remove(id).is_none() {
            return Err(PluginError::NotFound(id.to_string()));
        }
        self.instances.remove(id);
        Ok(())
    }

    /// Get plugin info by ID
    pub fn get(&self, id: &str) -> Option<&PluginInfo> {
        self.plugins.get(id)
    }

    /// Get all enabled plugins
    pub fn enabled_plugins(&self) -> Vec<&PluginInfo> {
        self.plugins.values()
            .filter(|info| info.state == PluginState::Enabled)
            .collect()
    }

    /// Get plugins matching a given extension point
    pub fn by_extension_point(&self, ep: &ExtensionPoint) -> Vec<&PluginInfo> {
        self.plugins.values()
            .filter(|info| &info.manifest.extension_point == ep)
            .collect()
    }

    /// Check whether a plugin has a specific permission granted in its manifest
    pub fn check_permissions(&self, id: &str, perm: &Permission) -> bool {
        match self.plugins.get(id) {
            Some(info) => info.manifest.permissions.contains(perm),
            None => false,
        }
    }

    /// Get info for all plugins
    pub fn list_plugins(&self) -> Vec<PluginInfo> {
        self.plugins.values().cloned().collect()
    }

    /// Get plugins with a specific capability (only enabled ones)
    pub fn plugins_with_capability(&self, cap: PluginCapability) -> Vec<&PluginInfo> {
        self.plugins.values()
            .filter(|info| info.state == PluginState::Enabled
                && info.manifest.capabilities.contains(&cap))
            .map(|info| info)
            .collect()
    }

    /// Get a plugin instance (downcast to specific type)
    pub fn get_plugin<T: Plugin + 'static>(&self, id: &PluginId) -> Option<&T> {
        self.instances.get(&id.0)
            .and_then(|p| p.as_any().downcast_ref::<T>())
    }

    pub fn get_plugin_mut<T: Plugin + 'static>(&mut self, id: &PluginId) -> Option<&mut T> {
        self.instances.get_mut(&id.0)
            .and_then(|p| p.as_any_mut().downcast_mut::<T>())
    }

    /// Activate a discovered plugin
    pub fn activate(&mut self, id: &PluginId) -> Result<(), LibPluginError> {
        let id_str = &id.0;
        let info = self.plugins.get_mut(id_str)
            .ok_or_else(|| LibPluginError::NotFound(id.clone()))?;

        if info.state == PluginState::Disabled {
            return Err(LibPluginError::Disabled(id.clone()));
        }

        if info.state == PluginState::Enabled {
            return Ok(());
        }

        // For built-in plugins with instances, call init
        if let Some(instance) = self.instances.get_mut(id_str) {
            match instance.init() {
                Ok(()) => {
                    self.load_counter += 1;
                    // Re-borrow after instance borrow ends
                    let info = self.plugins.get_mut(id_str)
                        .ok_or_else(|| LibPluginError::NotFound(id.clone()))?;
                    info.load_order = self.load_counter;
                    info.state = PluginState::Enabled;
                    Ok(())
                }
                Err(e) => {
                    let info = self.plugins.get_mut(id_str)
                        .ok_or_else(|| LibPluginError::NotFound(id.clone()))?;
                    info.state = PluginState::Error(e.to_string());
                    Err(e)
                }
            }
        } else {
            info.state = PluginState::Error("dynamic loading not yet implemented".into());
            Err(LibPluginError::LoadFailed("dynamic loading not yet implemented".into()))
        }
    }

    /// Deactivate a plugin
    pub fn deactivate(&mut self, id: &PluginId) -> Result<(), LibPluginError> {
        let id_str = &id.0;
        if !self.plugins.contains_key(id_str) {
            return Err(LibPluginError::NotFound(id.clone()));
        }

        if let Some(instance) = self.instances.get_mut(id_str) {
            instance.shutdown()?;
        }

        if let Some(info) = self.plugins.get_mut(id_str) {
            info.state = PluginState::Loaded;
        }
        Ok(())
    }

    /// Shutdown all plugins (reverse load order)
    pub fn shutdown_all(&mut self) {
        let mut ids: Vec<(String, u32)> = self.plugins.iter()
            .filter(|(_, info)| info.state == PluginState::Enabled)
            .map(|(id, info)| (id.clone(), info.load_order))
            .collect();
        ids.sort_by(|a, b| b.1.cmp(&a.1)); // reverse order

        for (id_str, _) in ids {
            let pid = PluginId(id_str);
            let _ = self.deactivate(&pid);
        }
    }

    /// Return number of registered plugins
    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    /// Return whether the registry is empty
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Plugin, PluginId, PluginCapability, PluginError as LibPluginError};
    use crate::manifest::{ExtensionPoint, Permission};
    use std::any::Any;

    /// A dummy plugin for testing
    struct DummyPlugin {
        id: PluginId,
        initialized: bool,
        shutdown_called: bool,
    }

    impl DummyPlugin {
        fn new(id: &str) -> Self {
            Self {
                id: PluginId(id.to_string()),
                initialized: false,
                shutdown_called: false,
            }
        }
    }

    impl Plugin for DummyPlugin {
        fn id(&self) -> &PluginId { &self.id }
        fn name(&self) -> &str { "Dummy" }
        fn version(&self) -> &str { "1.0" }
        fn capabilities(&self) -> Vec<PluginCapability> {
            vec![PluginCapability::ContextMenu]
        }
        fn init(&mut self) -> Result<(), LibPluginError> {
            self.initialized = true;
            Ok(())
        }
        fn shutdown(&mut self) -> Result<(), LibPluginError> {
            self.shutdown_called = true;
            Ok(())
        }
        fn as_any(&self) -> &dyn Any { self }
        fn as_any_mut(&mut self) -> &mut dyn Any { self }
    }

    /// A plugin that fails on init
    struct FailPlugin {
        id: PluginId,
    }

    impl Plugin for FailPlugin {
        fn id(&self) -> &PluginId { &self.id }
        fn name(&self) -> &str { "Fail" }
        fn version(&self) -> &str { "0.1" }
        fn capabilities(&self) -> Vec<PluginCapability> { vec![] }
        fn init(&mut self) -> Result<(), LibPluginError> {
            Err(LibPluginError::InitFailed("intentional failure".into()))
        }
        fn shutdown(&mut self) -> Result<(), LibPluginError> { Ok(()) }
        fn as_any(&self) -> &dyn Any { self }
        fn as_any_mut(&mut self) -> &mut dyn Any { self }
    }

    fn make_info(id: &str, ep: ExtensionPoint, perms: Vec<Permission>) -> PluginInfo {
        PluginInfo {
            manifest: PluginManifest {
                id: PluginId(id.into()),
                name: id.into(),
                version: "1.0".into(),
                description: String::new(),
                author: "test".into(),
                license: None,
                homepage: None,
                min_de_version: None,
                capabilities: vec![],
                extension_point: ep,
                permissions: perms,
                config_schema: None,
                dependencies: Vec::new(),
                native_lib: None,
                script: None,
                icon: None,
            },
            state: PluginState::Discovered,
            path: "/test".into(),
            load_order: 0,
            enabled_by_default: false,
        }
    }

    // ---- register / enable / disable lifecycle ----

    #[test]
    fn register_and_get() {
        let mut reg = PluginRegistry::new();
        let info = make_info("com.test.a", ExtensionPoint::PanelWidget, vec![]);
        reg.register(info).unwrap();
        assert!(reg.get("com.test.a").is_some());
        assert_eq!(reg.get("com.test.a").unwrap().state, PluginState::Discovered);
    }

    #[test]
    fn register_duplicate_fails() {
        let mut reg = PluginRegistry::new();
        let info1 = make_info("dup", ExtensionPoint::PanelWidget, vec![]);
        let info2 = make_info("dup", ExtensionPoint::PanelWidget, vec![]);
        reg.register(info1).unwrap();
        assert_eq!(reg.register(info2).unwrap_err(), PluginError::AlreadyExists);
    }

    #[test]
    fn enable_discovered_plugin() {
        let mut reg = PluginRegistry::new();
        reg.register(make_info("en", ExtensionPoint::PanelWidget, vec![])).unwrap();
        reg.enable("en").unwrap();
        assert_eq!(reg.get("en").unwrap().state, PluginState::Enabled);
    }

    #[test]
    fn enable_nonexistent_fails() {
        let mut reg = PluginRegistry::new();
        assert_eq!(reg.enable("ghost").unwrap_err(), PluginError::NotFound("ghost".into()));
    }

    #[test]
    fn disable_enabled_plugin() {
        let mut reg = PluginRegistry::new();
        reg.register(make_info("dis", ExtensionPoint::PanelWidget, vec![])).unwrap();
        reg.enable("dis").unwrap();
        reg.disable("dis").unwrap();
        assert_eq!(reg.get("dis").unwrap().state, PluginState::Disabled);
    }

    #[test]
    fn disable_nonexistent_fails() {
        let mut reg = PluginRegistry::new();
        assert_eq!(reg.disable("ghost").unwrap_err(), PluginError::NotFound("ghost".into()));
    }

    #[test]
    fn enable_disabled_plugin() {
        let mut reg = PluginRegistry::new();
        reg.register(make_info("toggle", ExtensionPoint::PanelWidget, vec![])).unwrap();
        reg.enable("toggle").unwrap();
        reg.disable("toggle").unwrap();
        assert_eq!(reg.get("toggle").unwrap().state, PluginState::Disabled);
        reg.enable("toggle").unwrap();
        assert_eq!(reg.get("toggle").unwrap().state, PluginState::Enabled);
    }

    #[test]
    fn enable_already_enabled_is_noop() {
        let mut reg = PluginRegistry::new();
        reg.register(make_info("en2", ExtensionPoint::PanelWidget, vec![])).unwrap();
        reg.enable("en2").unwrap();
        reg.enable("en2").unwrap(); // should not error
        assert_eq!(reg.get("en2").unwrap().state, PluginState::Enabled);
    }

    #[test]
    fn disable_already_disabled_is_noop() {
        let mut reg = PluginRegistry::new();
        reg.register(make_info("dis2", ExtensionPoint::PanelWidget, vec![])).unwrap();
        reg.disable("dis2").unwrap();
        reg.disable("dis2").unwrap(); // should not error
    }

    #[test]
    fn enable_error_state_fails() {
        let mut reg = PluginRegistry::new();
        let mut info = make_info("err", ExtensionPoint::PanelWidget, vec![]);
        info.state = PluginState::Error("broken".into());
        reg.register(info).unwrap();
        assert_eq!(reg.enable("err").unwrap_err(), PluginError::InvalidState);
    }

    #[test]
    fn disable_error_state_fails() {
        let mut reg = PluginRegistry::new();
        let mut info = make_info("err2", ExtensionPoint::PanelWidget, vec![]);
        info.state = PluginState::Error("broken".into());
        reg.register(info).unwrap();
        assert_eq!(reg.disable("err2").unwrap_err(), PluginError::InvalidState);
    }

    // ---- uninstall ----

    #[test]
    fn uninstall_removes_plugin() {
        let mut reg = PluginRegistry::new();
        reg.register(make_info("rem", ExtensionPoint::PanelWidget, vec![])).unwrap();
        assert!(reg.get("rem").is_some());
        reg.uninstall("rem").unwrap();
        assert!(reg.get("rem").is_none());
        assert_eq!(reg.len(), 0);
    }

    #[test]
    fn uninstall_nonexistent_fails() {
        let mut reg = PluginRegistry::new();
        assert_eq!(reg.uninstall("ghost").unwrap_err(), PluginError::NotFound("ghost".into()));
    }

    // ---- enabled_plugins ----

    #[test]
    fn enabled_plugins_filters_correctly() {
        let mut reg = PluginRegistry::new();
        reg.register(make_info("a", ExtensionPoint::PanelWidget, vec![])).unwrap();
        reg.register(make_info("b", ExtensionPoint::PanelWidget, vec![])).unwrap();
        reg.register(make_info("c", ExtensionPoint::PanelWidget, vec![])).unwrap();
        reg.enable("a").unwrap();
        reg.enable("c").unwrap();
        let enabled = reg.enabled_plugins();
        assert_eq!(enabled.len(), 2);
    }

    #[test]
    fn enabled_plugins_empty_registry() {
        let reg = PluginRegistry::new();
        assert!(reg.enabled_plugins().is_empty());
    }

    // ---- by_extension_point ----

    #[test]
    fn by_extension_point_filters() {
        let mut reg = PluginRegistry::new();
        reg.register(make_info("pw1", ExtensionPoint::PanelWidget, vec![])).unwrap();
        reg.register(make_info("pw2", ExtensionPoint::PanelWidget, vec![])).unwrap();
        reg.register(make_info("dw1", ExtensionPoint::DesktopWidget, vec![])).unwrap();
        reg.register(make_info("te1", ExtensionPoint::ThemeExtension, vec![])).unwrap();

        let panels = reg.by_extension_point(&ExtensionPoint::PanelWidget);
        assert_eq!(panels.len(), 2);

        let desktops = reg.by_extension_point(&ExtensionPoint::DesktopWidget);
        assert_eq!(desktops.len(), 1);

        let themes = reg.by_extension_point(&ExtensionPoint::ThemeExtension);
        assert_eq!(themes.len(), 1);

        let settings = reg.by_extension_point(&ExtensionPoint::SettingsPage);
        assert_eq!(settings.len(), 0);
    }

    // ---- check_permissions ----

    #[test]
    fn check_permissions_granted() {
        let mut reg = PluginRegistry::new();
        reg.register(make_info("perm1", ExtensionPoint::PanelWidget,
            vec![Permission::Network, Permission::Clipboard])).unwrap();
        assert!(reg.check_permissions("perm1", &Permission::Network));
        assert!(reg.check_permissions("perm1", &Permission::Clipboard));
        assert!(!reg.check_permissions("perm1", &Permission::Screenshot));
    }

    #[test]
    fn check_permissions_nonexistent_plugin() {
        let reg = PluginRegistry::new();
        assert!(!reg.check_permissions("ghost", &Permission::Network));
    }

    #[test]
    fn check_permissions_filesystem_path() {
        let mut reg = PluginRegistry::new();
        reg.register(make_info("fs", ExtensionPoint::PanelWidget,
            vec![Permission::FileSystem("/home".into())])).unwrap();
        assert!(reg.check_permissions("fs", &Permission::FileSystem("/home".into())));
        assert!(!reg.check_permissions("fs", &Permission::FileSystem("/root".into())));
    }

    // ---- builtin / activate / deactivate ----

    #[test]
    fn register_builtin_plugin() {
        let mut reg = PluginRegistry::new();
        let plugin = DummyPlugin::new("test.builtin");
        assert!(reg.register_builtin(Box::new(plugin)).is_ok());

        let list = reg.list_plugins();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].state, PluginState::Enabled);
        assert_eq!(list[0].manifest.id, PluginId("test.builtin".into()));
    }

    #[test]
    fn register_builtin_duplicate_fails() {
        let mut reg = PluginRegistry::new();
        reg.register_builtin(Box::new(DummyPlugin::new("dup"))).unwrap();
        let result = reg.register_builtin(Box::new(DummyPlugin::new("dup")));
        assert!(result.is_err());
        match result.unwrap_err() {
            LibPluginError::AlreadyLoaded(id) => assert_eq!(id.0, "dup"),
            other => panic!("unexpected: {:?}", other),
        }
    }

    #[test]
    fn deactivate_plugin() {
        let mut reg = PluginRegistry::new();
        reg.register_builtin(Box::new(DummyPlugin::new("deact"))).unwrap();
        let id = PluginId("deact".into());

        assert!(reg.deactivate(&id).is_ok());
        let info = reg.get("deact").unwrap();
        assert_eq!(info.state, PluginState::Loaded);
    }

    #[test]
    fn deactivate_nonexistent_fails() {
        let mut reg = PluginRegistry::new();
        let id = PluginId("nonexistent".into());
        assert!(reg.deactivate(&id).is_err());
    }

    #[test]
    fn activate_disabled_fails() {
        let mut reg = PluginRegistry::new();
        reg.register_builtin(Box::new(DummyPlugin::new("dis"))).unwrap();
        let id = PluginId("dis".into());
        reg.disable("dis").unwrap();

        let result = reg.activate(&id);
        assert!(result.is_err());
        match result.unwrap_err() {
            LibPluginError::Disabled(_) => {}
            other => panic!("unexpected: {:?}", other),
        }
    }

    #[test]
    fn plugins_with_capability() {
        let mut reg = PluginRegistry::new();
        reg.register_builtin(Box::new(DummyPlugin::new("ctx1"))).unwrap();
        reg.register_builtin(Box::new(DummyPlugin::new("ctx2"))).unwrap();

        let ctx_plugins = reg.plugins_with_capability(PluginCapability::ContextMenu);
        assert_eq!(ctx_plugins.len(), 2);

        let theme_plugins = reg.plugins_with_capability(PluginCapability::ThemeExtension);
        assert_eq!(theme_plugins.len(), 0);
    }

    #[test]
    fn get_plugin_downcast() {
        let mut reg = PluginRegistry::new();
        reg.register_builtin(Box::new(DummyPlugin::new("down"))).unwrap();
        let id = PluginId("down".into());
        let plugin = reg.get_plugin::<DummyPlugin>(&id);
        assert!(plugin.is_some());
    }

    #[test]
    fn get_plugin_mut_downcast() {
        let mut reg = PluginRegistry::new();
        reg.register_builtin(Box::new(DummyPlugin::new("downmut"))).unwrap();
        let id = PluginId("downmut".into());
        let plugin = reg.get_plugin_mut::<DummyPlugin>(&id);
        assert!(plugin.is_some());
    }

    #[test]
    fn shutdown_all_reverse_order() {
        let mut reg = PluginRegistry::new();
        reg.register_builtin(Box::new(DummyPlugin::new("first"))).unwrap();
        reg.register_builtin(Box::new(DummyPlugin::new("second"))).unwrap();
        reg.register_builtin(Box::new(DummyPlugin::new("third"))).unwrap();

        reg.shutdown_all();

        for info in reg.list_plugins() {
            assert_eq!(info.state, PluginState::Loaded);
        }
    }

    #[test]
    fn discover_with_no_paths() {
        let mut reg = PluginRegistry::new();
        reg.discover(&[]);
        assert!(reg.is_empty());
    }

    #[test]
    fn discover_with_nonexistent_path() {
        let mut reg = PluginRegistry::new();
        reg.discover(&["/nonexistent/path/to/plugins".to_string()]);
        assert!(reg.is_empty());
    }

    #[test]
    fn discover_from_temp_dir() {
        let tmp = std::env::temp_dir().join("liquide_plugin_test_discover_new");
        let plugin_dir = tmp.join("my_plugin");
        let _ = std::fs::create_dir_all(&plugin_dir);

        let manifest = r#"
id = "com.test.discovered"
name = "Discovered Plugin"
version = "0.5.0"
extension_point = "panel_widget"
"#;
        std::fs::write(plugin_dir.join("plugin.toml"), manifest).unwrap();

        let mut reg = PluginRegistry::new();
        reg.discover(&[tmp.to_string_lossy().to_string()]);

        assert_eq!(reg.len(), 1);
        let info = reg.get("com.test.discovered").unwrap();
        assert_eq!(info.state, PluginState::Discovered);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn activate_discovered_without_instance_fails() {
        let tmp = std::env::temp_dir().join("liquide_plugin_test_activate_new");
        let plugin_dir = tmp.join("no_inst");
        let _ = std::fs::create_dir_all(&plugin_dir);

        let manifest = "id = \"no_inst\"\nname = \"No Instance\"\nversion = \"1.0\"";
        std::fs::write(plugin_dir.join("plugin.toml"), manifest).unwrap();

        let mut reg = PluginRegistry::new();
        reg.discover(&[tmp.to_string_lossy().to_string()]);

        let id = PluginId("no_inst".into());
        let result = reg.activate(&id);
        assert!(result.is_err());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn add_search_path_dedup() {
        let mut reg = PluginRegistry::new();
        let path = PathBuf::from("/some/path");
        reg.add_search_path(path.clone());
        reg.add_search_path(path.clone());
        assert_eq!(reg.search_paths.len(), 1);
    }

    #[test]
    fn activate_already_active_is_ok() {
        let mut reg = PluginRegistry::new();
        reg.register_builtin(Box::new(DummyPlugin::new("already"))).unwrap();
        let id = PluginId("already".into());
        assert!(reg.activate(&id).is_ok());
    }

    #[test]
    fn activate_nonexistent_fails() {
        let mut reg = PluginRegistry::new();
        let id = PluginId("ghost".into());
        match reg.activate(&id) {
            Err(LibPluginError::NotFound(_)) => {}
            other => panic!("expected NotFound, got {:?}", other),
        }
    }

    #[test]
    fn activate_fail_plugin_sets_error_state() {
        let mut reg = PluginRegistry::new();
        let fail = FailPlugin { id: PluginId("fail.init".into()) };
        reg.register_builtin(Box::new(fail)).unwrap();
        let id = PluginId("fail.init".into());
        reg.deactivate(&id).unwrap();
        let result = reg.activate(&id);
        assert!(result.is_err());
        match result.unwrap_err() {
            LibPluginError::InitFailed(msg) => assert!(msg.contains("intentional")),
            other => panic!("expected InitFailed, got {:?}", other),
        }
        let info = reg.get("fail.init").unwrap();
        assert!(matches!(info.state, PluginState::Error(_)));
    }

    #[test]
    fn len_and_is_empty() {
        let mut reg = PluginRegistry::new();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);
        reg.register(make_info("x", ExtensionPoint::PanelWidget, vec![])).unwrap();
        assert!(!reg.is_empty());
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn plugin_error_display() {
        assert!(PluginError::NotFound("x".into()).to_string().contains("x"));
        assert!(PluginError::AlreadyExists.to_string().contains("already"));
        assert!(PluginError::InvalidState.to_string().contains("invalid"));
        assert!(PluginError::PermissionDenied.to_string().contains("permission"));
        assert!(PluginError::DependencyMissing("dep".into()).to_string().contains("dep"));
    }
}
