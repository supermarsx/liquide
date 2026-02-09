//! Primitive types shared across the ABI boundary.

use serde::{Deserialize, Serialize};

/// A handle to a host-side resource passed to the plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResourceHandle(pub u64);

/// Result code returned by plugin-exported functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum PluginResult {
    /// Success.
    Ok = 0,
    /// Generic error.
    Error = -1,
    /// The plugin does not handle this call.
    NotHandled = -2,
    /// The operation was denied by policy.
    PermissionDenied = -3,
}

/// A key-value pair for passing metadata across the ABI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataEntry {
    /// Key string.
    pub key: String,
    /// Value string.
    pub value: String,
}
