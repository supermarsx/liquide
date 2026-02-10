//! Plugin identity, state, and loaded-plugin representation.

use std::fmt;

use liquide_plugin_abi::{ExtensionPoint, PluginManifest};
use serde::{Deserialize, Serialize};

/// Unique runtime identifier for a loaded plugin instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PluginId(pub u64);

impl fmt::Display for PluginId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Plugin({})", self.0)
    }
}

/// Lifecycle state of a loaded plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginState {
    /// Plugin is being loaded and initialised.
    Loading,
    /// Plugin is active and ready to handle calls.
    Active,
    /// Plugin has been temporarily suspended.
    Suspended,
    /// Plugin has entered an error state.
    Failed { reason: String },
    /// Plugin has been cleanly unloaded.
    Unloaded,
}

impl fmt::Display for PluginState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Loading => write!(f, "Loading"),
            Self::Active => write!(f, "Active"),
            Self::Suspended => write!(f, "Suspended"),
            Self::Failed { reason } => write!(f, "Failed({reason})"),
            Self::Unloaded => write!(f, "Unloaded"),
        }
    }
}

/// A plugin that has been loaded into the host runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadedPlugin {
    /// Runtime-assigned unique identifier.
    pub id: PluginId,
    /// Parsed manifest describing the plugin's identity and requirements.
    pub manifest: PluginManifest,
    /// Current lifecycle state.
    pub state: PluginState,
    /// Timestamp (microseconds since epoch) at which the plugin was loaded.
    pub loaded_at_us: u64,
    /// Optional per-plugin JSON configuration blob.
    pub config_json: Option<String>,
}

impl LoadedPlugin {
    /// Create a new plugin in the [`PluginState::Loading`] state.
    #[must_use]
    pub fn new(id: PluginId, manifest: PluginManifest, loaded_at_us: u64) -> Self {
        Self {
            id,
            manifest,
            state: PluginState::Loading,
            loaded_at_us,
            config_json: None,
        }
    }

    /// Returns `true` if the plugin is in the [`PluginState::Active`] state.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.state == PluginState::Active
    }

    /// Returns `true` if the plugin is in the [`PluginState::Suspended`] state.
    #[must_use]
    pub fn is_suspended(&self) -> bool {
        self.state == PluginState::Suspended
    }

    /// Returns `true` if the plugin has failed.
    #[must_use]
    pub fn is_failed(&self) -> bool {
        matches!(self.state, PluginState::Failed { .. })
    }

    /// Returns `true` if the plugin has been unloaded.
    #[must_use]
    pub fn is_unloaded(&self) -> bool {
        self.state == PluginState::Unloaded
    }

    /// Get the extension points declared by this plugin.
    #[must_use]
    pub fn extension_points(&self) -> &[ExtensionPoint] {
        &self.manifest.extension_points
    }

    /// Get the manifest ID string.
    #[must_use]
    pub fn manifest_id(&self) -> &str {
        &self.manifest.id
    }

    /// Get the human-readable name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.manifest.name
    }

    /// Get the plugin version string.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.manifest.version
    }

    /// Transition to the [`PluginState::Active`] state.
    pub fn activate(&mut self) {
        self.state = PluginState::Active;
    }

    /// Transition to the [`PluginState::Suspended`] state.
    pub fn suspend(&mut self) {
        self.state = PluginState::Suspended;
    }

    /// Transition to the [`PluginState::Failed`] state.
    pub fn fail(&mut self, reason: impl Into<String>) {
        self.state = PluginState::Failed {
            reason: reason.into(),
        };
    }

    /// Transition to the [`PluginState::Unloaded`] state.
    pub fn unload(&mut self) {
        self.state = PluginState::Unloaded;
    }
}

impl fmt::Display for LoadedPlugin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}({}, \"{}\", v{}, {})",
            self.id,
            self.manifest.id,
            self.manifest.name,
            self.manifest.version,
            self.state,
        )
    }
}
