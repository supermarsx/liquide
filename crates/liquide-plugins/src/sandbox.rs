use crate::manifest::{Permission, ExtensionPoint};
use crate::registry::PluginError;
use crate::PluginManifest;

/// Tracks the set of permissions granted to a plugin at runtime.
pub struct PluginCapabilities {
    allowed_permissions: Vec<Permission>,
}

impl PluginCapabilities {
    /// Create capabilities from a manifest's declared permissions.
    pub fn from_manifest(manifest: &PluginManifest) -> Self {
        Self {
            allowed_permissions: manifest.permissions.clone(),
        }
    }

    /// Create empty capabilities (no permissions granted).
    pub fn empty() -> Self {
        Self {
            allowed_permissions: Vec::new(),
        }
    }

    /// Check whether the given permission is granted.
    pub fn can(&self, perm: &Permission) -> bool {
        self.allowed_permissions.contains(perm)
    }

    /// Grant a permission at runtime.
    pub fn grant(&mut self, perm: Permission) {
        if !self.allowed_permissions.contains(&perm) {
            self.allowed_permissions.push(perm);
        }
    }

    /// Revoke a permission at runtime.
    pub fn revoke(&mut self, perm: &Permission) {
        self.allowed_permissions.retain(|p| p != perm);
    }

    /// Return all currently granted permissions.
    pub fn granted(&self) -> &[Permission] {
        &self.allowed_permissions
    }

    /// Return the number of granted permissions.
    pub fn count(&self) -> usize {
        self.allowed_permissions.len()
    }
}

/// Trait that plugins implement to respond to lifecycle and UI events.
pub trait PluginApi: Send + Sync {
    /// Called when the plugin is enabled / activated.
    fn on_enable(&mut self);

    /// Called when the plugin is disabled / deactivated.
    fn on_disable(&mut self);

    /// Called when a per-plugin setting has changed.
    fn on_settings_changed(&mut self, key: &str, value: &str);

    /// Called when the user clicks the plugin's panel widget (if applicable).
    fn on_panel_click(&mut self);
}

/// A sandboxed wrapper around a plugin, enforcing permission checks before
/// allowing actions to proceed.
pub struct SandboxedPlugin {
    pub manifest: PluginManifest,
    pub capabilities: PluginCapabilities,
}

impl SandboxedPlugin {
    /// Create a new sandboxed plugin from a manifest, populating capabilities
    /// from the manifest's declared permissions.
    pub fn new(manifest: PluginManifest) -> Self {
        let capabilities = PluginCapabilities::from_manifest(&manifest);
        Self { manifest, capabilities }
    }

    /// Execute a named action. The action name is checked against a mapping
    /// of required permissions; if the plugin lacks the required permission,
    /// a `PermissionDenied` error is returned.
    pub fn execute_action(&self, action: &str) -> Result<String, PluginError> {
        let required = required_permission_for_action(action);

        if let Some(perm) = required {
            if !self.capabilities.can(&perm) {
                return Err(PluginError::PermissionDenied);
            }
        }

        Ok(format!("executed:{}", action))
    }

    /// Return the plugin's extension point.
    pub fn extension_point(&self) -> &ExtensionPoint {
        &self.manifest.extension_point
    }

    /// Return the plugin ID string.
    pub fn id(&self) -> &str {
        &self.manifest.id.0
    }
}

/// Map well-known action names to the permission they require.
/// Returns `None` for actions that need no special permission.
fn required_permission_for_action(action: &str) -> Option<Permission> {
    match action {
        "fetch" | "http_get" | "http_post" => Some(Permission::Network),
        "read_file" | "write_file" | "list_dir" => Some(Permission::FileSystem("/".into())),
        "send_notification" => Some(Permission::Notifications),
        "read_clipboard" | "write_clipboard" => Some(Permission::Clipboard),
        "query_system" | "cpu_info" | "memory_info" => Some(Permission::SystemInfo),
        "take_screenshot" | "capture_region" => Some(Permission::Screenshot),
        "play_sound" | "record_audio" => Some(Permission::Audio),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PluginId;
    use crate::manifest::{Permission, ExtensionPoint};

    fn test_manifest(perms: Vec<Permission>) -> PluginManifest {
        PluginManifest {
            id: PluginId("com.test.sandbox".into()),
            name: "Sandbox Test".into(),
            version: "1.0".into(),
            description: "A test plugin".into(),
            author: "tester".into(),
            license: None,
            homepage: None,
            min_de_version: None,
            capabilities: vec![],
            extension_point: ExtensionPoint::PanelWidget,
            permissions: perms,
            config_schema: None,
            dependencies: Vec::new(),
            native_lib: None,
            script: None,
            icon: None,
        }
    }

    // ---- PluginCapabilities tests ----

    #[test]
    fn empty_capabilities_deny_all() {
        let caps = PluginCapabilities::empty();
        assert!(!caps.can(&Permission::Network));
        assert!(!caps.can(&Permission::Clipboard));
        assert!(!caps.can(&Permission::Audio));
        assert_eq!(caps.count(), 0);
    }

    #[test]
    fn from_manifest_grants_declared() {
        let manifest = test_manifest(vec![Permission::Network, Permission::Clipboard]);
        let caps = PluginCapabilities::from_manifest(&manifest);
        assert!(caps.can(&Permission::Network));
        assert!(caps.can(&Permission::Clipboard));
        assert!(!caps.can(&Permission::Screenshot));
        assert_eq!(caps.count(), 2);
    }

    #[test]
    fn grant_adds_permission() {
        let mut caps = PluginCapabilities::empty();
        assert!(!caps.can(&Permission::Audio));
        caps.grant(Permission::Audio);
        assert!(caps.can(&Permission::Audio));
        assert_eq!(caps.count(), 1);
    }

    #[test]
    fn grant_is_idempotent() {
        let mut caps = PluginCapabilities::empty();
        caps.grant(Permission::Network);
        caps.grant(Permission::Network);
        assert_eq!(caps.count(), 1);
    }

    #[test]
    fn revoke_removes_permission() {
        let mut caps = PluginCapabilities::empty();
        caps.grant(Permission::Network);
        caps.grant(Permission::Clipboard);
        assert_eq!(caps.count(), 2);
        caps.revoke(&Permission::Network);
        assert!(!caps.can(&Permission::Network));
        assert!(caps.can(&Permission::Clipboard));
        assert_eq!(caps.count(), 1);
    }

    #[test]
    fn revoke_nonexistent_is_noop() {
        let mut caps = PluginCapabilities::empty();
        caps.revoke(&Permission::Audio); // should not panic
        assert_eq!(caps.count(), 0);
    }

    #[test]
    fn granted_returns_all() {
        let mut caps = PluginCapabilities::empty();
        caps.grant(Permission::Network);
        caps.grant(Permission::Audio);
        let granted = caps.granted();
        assert_eq!(granted.len(), 2);
        assert!(granted.contains(&Permission::Network));
        assert!(granted.contains(&Permission::Audio));
    }

    #[test]
    fn filesystem_permission_path_matters() {
        let mut caps = PluginCapabilities::empty();
        caps.grant(Permission::FileSystem("/home/user".into()));
        assert!(caps.can(&Permission::FileSystem("/home/user".into())));
        assert!(!caps.can(&Permission::FileSystem("/root".into())));
    }

    // ---- SandboxedPlugin tests ----

    #[test]
    fn execute_unprivileged_action() {
        let sp = SandboxedPlugin::new(test_manifest(vec![]));
        // Actions with no required permission should succeed
        let result = sp.execute_action("noop");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "executed:noop");
    }

    #[test]
    fn execute_network_action_granted() {
        let sp = SandboxedPlugin::new(test_manifest(vec![Permission::Network]));
        assert!(sp.execute_action("fetch").is_ok());
        assert!(sp.execute_action("http_get").is_ok());
        assert!(sp.execute_action("http_post").is_ok());
    }

    #[test]
    fn execute_network_action_denied() {
        let sp = SandboxedPlugin::new(test_manifest(vec![]));
        let result = sp.execute_action("fetch");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), PluginError::PermissionDenied);
    }

    #[test]
    fn execute_filesystem_action_granted() {
        let sp = SandboxedPlugin::new(test_manifest(vec![Permission::FileSystem("/".into())]));
        assert!(sp.execute_action("read_file").is_ok());
        assert!(sp.execute_action("write_file").is_ok());
    }

    #[test]
    fn execute_filesystem_action_denied() {
        let sp = SandboxedPlugin::new(test_manifest(vec![]));
        assert_eq!(sp.execute_action("read_file").unwrap_err(), PluginError::PermissionDenied);
    }

    #[test]
    fn execute_notification_action() {
        let sp = SandboxedPlugin::new(test_manifest(vec![Permission::Notifications]));
        assert!(sp.execute_action("send_notification").is_ok());
    }

    #[test]
    fn execute_clipboard_action() {
        let sp = SandboxedPlugin::new(test_manifest(vec![Permission::Clipboard]));
        assert!(sp.execute_action("read_clipboard").is_ok());
        assert!(sp.execute_action("write_clipboard").is_ok());
    }

    #[test]
    fn execute_screenshot_action_denied() {
        let sp = SandboxedPlugin::new(test_manifest(vec![Permission::Network]));
        assert_eq!(sp.execute_action("take_screenshot").unwrap_err(), PluginError::PermissionDenied);
    }

    #[test]
    fn execute_audio_action_denied() {
        let sp = SandboxedPlugin::new(test_manifest(vec![]));
        assert_eq!(sp.execute_action("play_sound").unwrap_err(), PluginError::PermissionDenied);
    }

    #[test]
    fn sandboxed_plugin_id_and_extension_point() {
        let sp = SandboxedPlugin::new(test_manifest(vec![]));
        assert_eq!(sp.id(), "com.test.sandbox");
        assert_eq!(sp.extension_point(), &ExtensionPoint::PanelWidget);
    }

    #[test]
    fn sandboxed_plugin_dynamic_grant() {
        let mut sp = SandboxedPlugin::new(test_manifest(vec![]));
        // Initially denied
        assert!(sp.execute_action("fetch").is_err());
        // Grant at runtime
        sp.capabilities.grant(Permission::Network);
        assert!(sp.execute_action("fetch").is_ok());
    }

    #[test]
    fn sandboxed_plugin_dynamic_revoke() {
        let mut sp = SandboxedPlugin::new(test_manifest(vec![Permission::Network]));
        assert!(sp.execute_action("fetch").is_ok());
        sp.capabilities.revoke(&Permission::Network);
        assert!(sp.execute_action("fetch").is_err());
    }
}
