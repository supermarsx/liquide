use crate::{PluginCapability, PluginError, PluginId};

/// Extension point describing where a plugin integrates into the desktop
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtensionPoint {
    PanelWidget,
    DesktopWidget,
    ThemeExtension,
    StatusIndicator,
    SearchProvider,
    SettingsPage,
    WindowAction,
    Custom(String),
}

/// Permission that a plugin may request
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Permission {
    Network,
    FileSystem(String),
    Notifications,
    Clipboard,
    SystemInfo,
    Screenshot,
    Audio,
    Custom(String),
}

/// Errors arising from manifest parsing
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    MissingField(String),
    InvalidFormat(String),
    UnsupportedVersion,
}

impl std::fmt::Display for ManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingField(field) => write!(f, "missing required field: {}", field),
            Self::InvalidFormat(msg) => write!(f, "invalid format: {}", msg),
            Self::UnsupportedVersion => write!(f, "unsupported manifest version"),
        }
    }
}
impl std::error::Error for ManifestError {}

/// Plugin manifest describing a plugin's metadata
#[derive(Debug, Clone)]
pub struct PluginManifest {
    pub id: PluginId,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub license: Option<String>,
    pub homepage: Option<String>,
    pub min_de_version: Option<String>,
    pub capabilities: Vec<PluginCapability>,
    pub extension_point: ExtensionPoint,
    pub permissions: Vec<Permission>,
    pub config_schema: Option<ConfigSchema>,
    pub dependencies: Vec<PluginDependency>,
    /// For native plugins: path to shared library
    pub native_lib: Option<String>,
    /// For script plugins: path to script
    pub script: Option<String>,
    /// Icon path relative to plugin directory
    pub icon: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PluginDependency {
    pub plugin_id: String,
    pub min_version: Option<String>,
}

/// Schema for plugin configuration (simple key-value)
#[derive(Debug, Clone)]
pub struct ConfigSchema {
    pub fields: Vec<ConfigField>,
}

#[derive(Debug, Clone)]
pub struct ConfigField {
    pub key: String,
    pub label: String,
    pub description: Option<String>,
    pub field_type: ConfigFieldType,
    pub default_value: Option<String>,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigFieldType {
    String,
    Integer,
    Float,
    Boolean,
    Color,
    FilePath,
    Choice(Vec<String>),
}

/// Parse a TOML-like manifest string into a `PluginManifest`.
pub fn parse_manifest(toml_str: &str) -> Result<PluginManifest, ManifestError> {
    let mut id = String::new();
    let mut name = String::new();
    let mut version = String::new();
    let mut description = String::new();
    let mut author = String::new();
    let mut license = None;
    let mut homepage = None;
    let mut min_de_version = None;
    let mut capabilities = Vec::new();
    let mut extension_point: Option<ExtensionPoint> = None;
    let mut permissions: Vec<Permission> = Vec::new();
    let mut native_lib = None;
    let mut script = None;
    let mut icon = None;

    for line in toml_str.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim().trim_matches('"');
            match key {
                "id" => id = value.to_string(),
                "name" => name = value.to_string(),
                "version" => version = value.to_string(),
                "description" => description = value.to_string(),
                "author" => author = value.to_string(),
                "license" => license = Some(value.to_string()),
                "homepage" => homepage = Some(value.to_string()),
                "min_de_version" => min_de_version = Some(value.to_string()),
                "native_lib" => native_lib = Some(value.to_string()),
                "script" => script = Some(value.to_string()),
                "icon" => icon = Some(value.to_string()),
                "capabilities" => {
                    capabilities = value
                        .split(',')
                        .filter_map(|c| parse_capability(c.trim()))
                        .collect();
                }
                "extension_point" => {
                    extension_point = Some(parse_extension_point(value));
                }
                "permissions" => {
                    permissions = value
                        .split(',')
                        .map(|p| parse_permission(p.trim()))
                        .collect();
                }
                _ => {}
            }
        }
    }

    if id.is_empty() {
        return Err(ManifestError::MissingField("id".into()));
    }
    if name.is_empty() {
        return Err(ManifestError::MissingField("name".into()));
    }
    if version.is_empty() {
        return Err(ManifestError::MissingField("version".into()));
    }

    Ok(PluginManifest {
        id: PluginId(id),
        name,
        version,
        description,
        author,
        license,
        homepage,
        min_de_version,
        capabilities,
        extension_point: extension_point.unwrap_or(ExtensionPoint::Custom("unknown".into())),
        permissions,
        config_schema: None,
        dependencies: Vec::new(),
        native_lib,
        script,
        icon,
    })
}

fn parse_extension_point(s: &str) -> ExtensionPoint {
    match s.to_lowercase().replace(['-', '_'], "").as_str() {
        "panelwidget" => ExtensionPoint::PanelWidget,
        "desktopwidget" => ExtensionPoint::DesktopWidget,
        "themeextension" => ExtensionPoint::ThemeExtension,
        "statusindicator" => ExtensionPoint::StatusIndicator,
        "searchprovider" => ExtensionPoint::SearchProvider,
        "settingspage" => ExtensionPoint::SettingsPage,
        "windowaction" => ExtensionPoint::WindowAction,
        other => ExtensionPoint::Custom(other.to_string()),
    }
}

fn parse_permission(s: &str) -> Permission {
    let s_lower = s.to_lowercase();
    // Handle filesystem with path: "filesystem(/home/user)"
    if s_lower.starts_with("filesystem(") && s_lower.ends_with(')') {
        let path = &s[11..s.len() - 1]; // extract path between parens
        return Permission::FileSystem(path.to_string());
    }
    if s_lower.starts_with("filesystem") {
        return Permission::FileSystem("/".to_string());
    }
    match s_lower.as_str() {
        "network" => Permission::Network,
        "notifications" => Permission::Notifications,
        "clipboard" => Permission::Clipboard,
        "systeminfo" | "system_info" => Permission::SystemInfo,
        "screenshot" => Permission::Screenshot,
        "audio" => Permission::Audio,
        other => Permission::Custom(other.to_string()),
    }
}

impl PluginManifest {
    /// Parse manifest from TOML-like format (simple key=value parser).
    /// This delegates to the module-level `parse_manifest` function.
    pub fn parse(content: &str) -> Result<Self, PluginError> {
        parse_manifest(content).map_err(|e| PluginError::ManifestInvalid(e.to_string()))
    }
}

fn parse_capability(s: &str) -> Option<PluginCapability> {
    match s.to_lowercase().as_str() {
        "context_menu" | "contextmenu" => Some(PluginCapability::ContextMenu),
        "statusbar_widget" | "statusbarwidget" => Some(PluginCapability::StatusBarWidget),
        "file_preview" | "filepreview" => Some(PluginCapability::FilePreview),
        "theme_extension" | "themeextension" => Some(PluginCapability::ThemeExtension),
        "notification_provider" | "notificationprovider" => {
            Some(PluginCapability::NotificationProvider)
        }
        "search_provider" | "searchprovider" => Some(PluginCapability::SearchProvider),
        "window_decorator" | "windowdecorator" => Some(PluginCapability::WindowDecorator),
        "keyboard_shortcuts" | "keyboardshortcuts" => Some(PluginCapability::KeyboardShortcuts),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- parse_manifest tests ----

    #[test]
    fn parse_valid_manifest() {
        let content = r#"
[plugin]
id = "com.example.hello"
name = "Hello Plugin"
version = "1.0.0"
description = "A test plugin"
author = "Test Author"
license = "MIT"
extension_point = "panel_widget"
permissions = "network, clipboard"
capabilities = "context_menu, statusbar_widget"
"#;
        let manifest = parse_manifest(content).unwrap();
        assert_eq!(manifest.id, PluginId("com.example.hello".into()));
        assert_eq!(manifest.name, "Hello Plugin");
        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(manifest.description, "A test plugin");
        assert_eq!(manifest.author, "Test Author");
        assert_eq!(manifest.license, Some("MIT".into()));
        assert_eq!(manifest.extension_point, ExtensionPoint::PanelWidget);
        assert_eq!(manifest.permissions.len(), 2);
        assert!(manifest.permissions.contains(&Permission::Network));
        assert!(manifest.permissions.contains(&Permission::Clipboard));
        assert_eq!(manifest.capabilities.len(), 2);
        assert!(
            manifest
                .capabilities
                .contains(&PluginCapability::ContextMenu)
        );
        assert!(
            manifest
                .capabilities
                .contains(&PluginCapability::StatusBarWidget)
        );
    }

    #[test]
    fn parse_minimal_manifest() {
        let content = "id = \"test\"\nname = \"Test\"\nversion = \"0.1\"";
        let manifest = parse_manifest(content).unwrap();
        assert_eq!(manifest.id, PluginId("test".into()));
        assert!(manifest.capabilities.is_empty());
        assert!(manifest.license.is_none());
        assert!(manifest.permissions.is_empty());
    }

    #[test]
    fn parse_missing_id() {
        let content = "name = \"Test\"\nversion = \"1.0\"";
        let result = parse_manifest(content);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            ManifestError::MissingField("id".into())
        );
    }

    #[test]
    fn parse_missing_name() {
        let content = "id = \"test\"\nversion = \"1.0\"";
        let result = parse_manifest(content);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            ManifestError::MissingField("name".into())
        );
    }

    #[test]
    fn parse_missing_version() {
        let content = "id = \"test\"\nname = \"Test\"";
        let result = parse_manifest(content);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            ManifestError::MissingField("version".into())
        );
    }

    #[test]
    fn parse_empty_content() {
        let result = parse_manifest("");
        assert!(result.is_err());
    }

    #[test]
    fn parse_all_capability_variants() {
        let content = r#"
id = "cap-test"
name = "Cap Test"
version = "1.0"
capabilities = "context_menu, statusbar_widget, file_preview, theme_extension, notification_provider, search_provider, window_decorator, keyboard_shortcuts"
"#;
        let manifest = parse_manifest(content).unwrap();
        assert_eq!(manifest.capabilities.len(), 8);
    }

    #[test]
    fn parse_capability_case_insensitive() {
        let content = r#"
id = "cap-case"
name = "Cap Case"
version = "1.0"
capabilities = "ContextMenu, FilePreview"
"#;
        let manifest = parse_manifest(content).unwrap();
        assert_eq!(manifest.capabilities.len(), 2);
    }

    #[test]
    fn parse_ignores_comments_and_sections() {
        let content = r#"
# This is a comment
[plugin]
id = "commented"
name = "Commented"
version = "1.0"
# another comment
"#;
        let manifest = parse_manifest(content).unwrap();
        assert_eq!(manifest.id, PluginId("commented".into()));
    }

    #[test]
    fn parse_optional_fields() {
        let content = r#"
id = "full"
name = "Full Plugin"
version = "2.0"
homepage = "https://example.com"
min_de_version = "0.5.0"
native_lib = "libplugin.so"
script = "main.lua"
icon = "icon.png"
"#;
        let manifest = parse_manifest(content).unwrap();
        assert_eq!(manifest.homepage, Some("https://example.com".into()));
        assert_eq!(manifest.min_de_version, Some("0.5.0".into()));
        assert_eq!(manifest.native_lib, Some("libplugin.so".into()));
        assert_eq!(manifest.script, Some("main.lua".into()));
        assert_eq!(manifest.icon, Some("icon.png".into()));
    }

    // ---- ExtensionPoint tests ----

    #[test]
    fn parse_all_extension_points() {
        let points = [
            ("panel_widget", ExtensionPoint::PanelWidget),
            ("desktop_widget", ExtensionPoint::DesktopWidget),
            ("theme_extension", ExtensionPoint::ThemeExtension),
            ("status_indicator", ExtensionPoint::StatusIndicator),
            ("search_provider", ExtensionPoint::SearchProvider),
            ("settings_page", ExtensionPoint::SettingsPage),
            ("window_action", ExtensionPoint::WindowAction),
        ];
        for (input, expected) in &points {
            let content = format!(
                "id = \"ep\"\nname = \"EP\"\nversion = \"1.0\"\nextension_point = \"{}\"",
                input
            );
            let manifest = parse_manifest(&content).unwrap();
            assert_eq!(
                manifest.extension_point, *expected,
                "failed for input: {}",
                input
            );
        }
    }

    #[test]
    fn parse_custom_extension_point() {
        let content = r#"
id = "custom-ep"
name = "Custom EP"
version = "1.0"
extension_point = "my_special_thing"
"#;
        let manifest = parse_manifest(content).unwrap();
        assert_eq!(
            manifest.extension_point,
            ExtensionPoint::Custom("myspecialthing".into())
        );
    }

    #[test]
    fn default_extension_point_when_missing() {
        let content = "id = \"no-ep\"\nname = \"No EP\"\nversion = \"1.0\"";
        let manifest = parse_manifest(content).unwrap();
        assert_eq!(
            manifest.extension_point,
            ExtensionPoint::Custom("unknown".into())
        );
    }

    // ---- Permission tests ----

    #[test]
    fn parse_all_permissions() {
        let content = r#"
id = "perms"
name = "Perms"
version = "1.0"
permissions = "network, notifications, clipboard, system_info, screenshot, audio"
"#;
        let manifest = parse_manifest(content).unwrap();
        assert_eq!(manifest.permissions.len(), 6);
        assert!(manifest.permissions.contains(&Permission::Network));
        assert!(manifest.permissions.contains(&Permission::Notifications));
        assert!(manifest.permissions.contains(&Permission::Clipboard));
        assert!(manifest.permissions.contains(&Permission::SystemInfo));
        assert!(manifest.permissions.contains(&Permission::Screenshot));
        assert!(manifest.permissions.contains(&Permission::Audio));
    }

    #[test]
    fn parse_filesystem_permission_with_path() {
        let content = r#"
id = "fs"
name = "FS"
version = "1.0"
permissions = "filesystem(/home/user/docs)"
"#;
        let manifest = parse_manifest(content).unwrap();
        assert_eq!(manifest.permissions.len(), 1);
        assert_eq!(
            manifest.permissions[0],
            Permission::FileSystem("/home/user/docs".into())
        );
    }

    #[test]
    fn parse_filesystem_permission_bare() {
        let content =
            "id = \"fs2\"\nname = \"FS2\"\nversion = \"1.0\"\npermissions = \"filesystem\"";
        let manifest = parse_manifest(content).unwrap();
        assert_eq!(manifest.permissions[0], Permission::FileSystem("/".into()));
    }

    #[test]
    fn parse_custom_permission() {
        let content = "id = \"cp\"\nname = \"CP\"\nversion = \"1.0\"\npermissions = \"gpu_access\"";
        let manifest = parse_manifest(content).unwrap();
        assert_eq!(
            manifest.permissions[0],
            Permission::Custom("gpu_access".into())
        );
    }

    // ---- ManifestError tests ----

    #[test]
    fn manifest_error_display() {
        let err = ManifestError::MissingField("name".into());
        assert!(err.to_string().contains("name"));

        let err = ManifestError::InvalidFormat("bad toml".into());
        assert!(err.to_string().contains("bad toml"));

        let err = ManifestError::UnsupportedVersion;
        assert!(err.to_string().contains("unsupported"));
    }

    // ---- PluginManifest::parse() backward compat ----

    #[test]
    fn plugin_manifest_parse_method_works() {
        let content = "id = \"compat\"\nname = \"Compat\"\nversion = \"1.0\"";
        let manifest = PluginManifest::parse(content).unwrap();
        assert_eq!(manifest.id, PluginId("compat".into()));
    }

    #[test]
    fn plugin_manifest_parse_method_returns_plugin_error() {
        let content = "name = \"no-id\"";
        let result = PluginManifest::parse(content);
        assert!(result.is_err());
        // Should be a PluginError::ManifestInvalid
        match result.unwrap_err() {
            PluginError::ManifestInvalid(msg) => assert!(msg.contains("id")),
            other => panic!("unexpected error: {:?}", other),
        }
    }
}
