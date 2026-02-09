//! Plugin manifest parsing and validation.

use crate::PluginManifest;

/// Validate that a manifest is compatible with the current host.
///
/// Returns `true` if the plugin's ABI version matches the host's.
#[must_use]
pub fn is_compatible(manifest: &PluginManifest) -> bool {
    manifest.abi_version == crate::ABI_VERSION
}

/// Parse a plugin manifest from JSON bytes.
pub fn parse_manifest(json: &[u8]) -> std::result::Result<PluginManifest, String> {
    serde_json::from_slice(json).map_err(|e| e.to_string())
}
