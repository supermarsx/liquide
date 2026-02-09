//! Policy engine core: loading, caching, and querying policies.

use std::path::Path;

use crate::{PolicyEngine, Result};

/// Load a [`PolicyEngine`] from a TOML configuration directory.
///
/// Expects files named `server.toml`, `group/*.toml`, `user/*.toml` inside
/// the given directory.
pub fn load_from_dir(_dir: &Path) -> Result<PolicyEngine> {
    // Stub — real implementation would walk the directory tree.
    Ok(PolicyEngine::new())
}
