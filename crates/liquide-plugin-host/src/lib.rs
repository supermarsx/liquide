//! Plugin host for the LiquiDE extension system.
//!
//! Manages plugin lifecycle, resource allocation, extension-point dispatch,
//! and host-function routing for the LiquiDE remote desktop platform.

pub mod config;
pub mod dispatcher;
pub mod host;
pub mod plugin;
pub mod resources;
pub mod runtime;

use std::fmt;

use thiserror::Error;

use liquide_plugin_abi::ExtensionPoint;

/// Errors produced by the plugin host subsystem.
#[derive(Debug, Error)]
pub enum PluginHostError {
    /// A plugin with the given ID was not found.
    #[error("plugin not found: {id}")]
    PluginNotFound { id: PluginId },

    /// The plugin's ABI version is incompatible with the host.
    #[error("incompatible ABI version: expected {expected}, found {found}")]
    IncompatibleAbi { expected: u32, found: u32 },

    /// Failed to parse a plugin manifest.
    #[error("manifest parse error: {0}")]
    ManifestParse(String),

    /// A resource allocation was denied because the pool is exhausted.
    #[error("resource exhausted: requested {requested} bytes, {available} available")]
    ResourceExhausted { requested: u64, available: u64 },

    /// The maximum number of loaded plugins has been reached.
    #[error("plugin limit reached: maximum {max} plugins")]
    PluginLimitReached { max: usize },

    /// The plugin is in an invalid state for the requested operation.
    #[error("invalid plugin state for {id}: {state}")]
    InvalidState { id: PluginId, state: String },

    /// The requested extension point is not permitted by configuration.
    #[error("extension point not allowed: {point}")]
    ExtensionPointNotAllowed { point: ExtensionPointDisplay },

    /// A plugin with the same manifest ID is already loaded.
    #[error("duplicate plugin: {manifest_id}")]
    DuplicatePlugin { manifest_id: String },

    /// An internal error occurred.
    #[error("internal error: {0}")]
    Internal(String),
}

/// Wrapper for displaying [`ExtensionPoint`] in error messages.
#[derive(Debug)]
pub struct ExtensionPointDisplay(pub ExtensionPoint);

impl fmt::Display for ExtensionPointDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

// Re-exports — placed before `Result` so `PluginId` is in scope for error variants.
pub use config::PluginHostConfig;
pub use plugin::{LoadedPlugin, PluginId, PluginState};

/// Result type for the plugin host subsystem.
pub type Result<T> = std::result::Result<T, PluginHostError>;
pub use dispatcher::Dispatcher;
pub use host::PluginHost;
pub use resources::{ResourceAllocation, ResourcePool};
pub use runtime::PluginRuntime;

#[cfg(test)]
mod tests;
