#![doc = "Plugin ABI definitions for the Liquide extension system."]
#![doc = ""]
#![doc = "This crate is the contract between the plugin host (`liquide-plugin-host`)"]
#![doc = "and individual WASM plugins.  Both sides depend on this crate to ensure"]
#![doc = "ABI stability."]

pub mod host_functions;
pub mod plugin_manifest;
pub mod types;

use serde::{Deserialize, Serialize};

/// Current ABI version.  Plugins compiled against a different ABI version
/// will be rejected at load time.
pub const ABI_VERSION: u32 = 1;

/// A plugin manifest describing the plugin's identity and capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Unique plugin identifier (reverse-domain, e.g. `"com.example.my-plugin"`).
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// Semantic version string.
    pub version: String,
    /// The ABI version this plugin was compiled against.
    pub abi_version: u32,
    /// Extension points this plugin hooks into.
    pub extension_points: Vec<ExtensionPoint>,
    /// Resource limits requested by the plugin.
    pub requested_memory_bytes: u64,
}

/// Locations in the Liquide stack where a plugin may insert behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExtensionPoint {
    /// Pre-authentication hook.
    PreAuth,
    /// Post-authentication hook.
    PostAuth,
    /// Input event filter.
    InputFilter,
    /// Clipboard content transform.
    ClipboardTransform,
    /// Custom protocol channel handler.
    ChannelHandler,
    /// Shell panel widget.
    ShellWidget,
    /// Policy evaluation hook.
    PolicyHook,
    /// Encoder pipeline stage.
    EncoderStage,
}
