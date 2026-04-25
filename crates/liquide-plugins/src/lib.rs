pub mod context_menu;
pub mod file_preview;
pub mod manifest;
pub mod preferences;
pub mod registry;
pub mod sandbox;
pub mod statusbar_widget;
pub mod theme_extension;

pub use manifest::{ExtensionPoint, ManifestError, Permission, PluginManifest, parse_manifest};
pub use preferences::PluginPreferences;
pub use registry::{PluginError as RegistryError, PluginInfo, PluginRegistry, PluginState};
pub use sandbox::{PluginApi, PluginCapabilities, SandboxedPlugin};

use std::any::Any;

/// Unique plugin identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PluginId(pub String);

/// Plugin capability — what extensions does this plugin provide?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PluginCapability {
    ContextMenu,
    StatusBarWidget,
    FilePreview,
    ThemeExtension,
    NotificationProvider,
    SearchProvider,
    WindowDecorator,
    KeyboardShortcuts,
}

/// Error types for plugin operations
#[derive(Debug, Clone)]
pub enum PluginError {
    NotFound(PluginId),
    AlreadyLoaded(PluginId),
    LoadFailed(String),
    ManifestInvalid(String),
    IncompatibleVersion { required: String, found: String },
    InitFailed(String),
    Disabled(PluginId),
}

impl std::fmt::Display for PluginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(id) => write!(f, "plugin not found: {}", id.0),
            Self::AlreadyLoaded(id) => write!(f, "plugin already loaded: {}", id.0),
            Self::LoadFailed(msg) => write!(f, "load failed: {}", msg),
            Self::ManifestInvalid(msg) => write!(f, "invalid manifest: {}", msg),
            Self::IncompatibleVersion { required, found } => {
                write!(
                    f,
                    "incompatible version: requires {}, found {}",
                    required, found
                )
            }
            Self::InitFailed(msg) => write!(f, "init failed: {}", msg),
            Self::Disabled(id) => write!(f, "plugin disabled: {}", id.0),
        }
    }
}
impl std::error::Error for PluginError {}

/// Base plugin trait — all plugins implement this
pub trait Plugin: Send + Sync {
    fn id(&self) -> &PluginId;
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn capabilities(&self) -> Vec<PluginCapability>;
    fn init(&mut self) -> Result<(), PluginError>;
    fn shutdown(&mut self) -> Result<(), PluginError>;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}
