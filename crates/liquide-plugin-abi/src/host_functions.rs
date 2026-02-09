//! Definitions for functions the host makes available to plugins.

/// Function indices for host-provided functions callable from WASM plugins.
///
/// These are stable across minor ABI versions.
pub const FN_LOG: u32 = 1;
pub const FN_GET_CONFIG: u32 = 2;
pub const FN_SEND_MESSAGE: u32 = 3;
pub const FN_ALLOCATE_BUFFER: u32 = 4;
pub const FN_FREE_BUFFER: u32 = 5;

/// Describes a single host function exposed to plugins.
#[derive(Debug, Clone)]
pub struct HostFunction {
    /// Stable function index.
    pub index: u32,
    /// Human-readable name.
    pub name: &'static str,
    /// Number of parameters.
    pub param_count: usize,
}

/// Registry of all host functions.
pub const HOST_FUNCTIONS: &[HostFunction] = &[
    HostFunction { index: FN_LOG, name: "log", param_count: 2 },
    HostFunction { index: FN_GET_CONFIG, name: "get_config", param_count: 1 },
    HostFunction { index: FN_SEND_MESSAGE, name: "send_message", param_count: 2 },
    HostFunction { index: FN_ALLOCATE_BUFFER, name: "allocate_buffer", param_count: 1 },
    HostFunction { index: FN_FREE_BUFFER, name: "free_buffer", param_count: 1 },
];
