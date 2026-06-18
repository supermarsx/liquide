//! The feature-off stub host.
//!
//! When the `wasm` feature is disabled, the workspace must still build and link
//! against this crate's public API without pulling in wasmtime. `NullWasmHost`
//! mirrors the real [`crate::WasmHost`] surface and answers every operation with
//! [`WasmHostError::Unavailable`], the same way `liquide-platform`'s `Null*`
//! hosts stand in for an absent backend.

use crate::{AppWidgetAction, AppWidgetModel, Result, WasmHostApi, WasmHostError, WasmSandboxConfig};

/// A no-op WASM host used when the runtime is not compiled in.
///
/// Construct it the same way you'd construct the real host (`from_bytes`), so
/// call sites are identical regardless of feature flags; every operation then
/// reports [`WasmHostError::Unavailable`].
#[derive(Debug, Default, Clone)]
pub struct NullWasmHost {
    _config: WasmSandboxConfig,
}

impl NullWasmHost {
    /// Construct a null host. The bytes and config are accepted (so the call
    /// site matches the real host) but ignored — nothing is loaded.
    ///
    /// # Errors
    ///
    /// Never errors at construction; the absence of a runtime surfaces from the
    /// [`WasmHostApi`] operations, not from loading.
    pub fn from_bytes(_wasm: &[u8], config: WasmSandboxConfig) -> Result<Self> {
        Ok(Self { _config: config })
    }

    /// Construct a null host with default limits.
    ///
    /// # Errors
    ///
    /// Never errors.
    pub fn from_bytes_default(wasm: &[u8]) -> Result<Self> {
        Self::from_bytes(wasm, WasmSandboxConfig::default())
    }
}

impl WasmHostApi for NullWasmHost {
    fn render(&mut self) -> Result<AppWidgetModel> {
        Err(WasmHostError::Unavailable)
    }

    fn apply_action(&mut self, _action: &AppWidgetAction) -> Result<bool> {
        Err(WasmHostError::Unavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_host_constructs_but_reports_unavailable() {
        let mut host = NullWasmHost::from_bytes_default(&[]).expect("null host constructs");
        assert!(matches!(host.render(), Err(WasmHostError::Unavailable)));
        let action = AppWidgetAction::new("k", "click", "");
        assert!(matches!(
            host.apply_action(&action),
            Err(WasmHostError::Unavailable)
        ));
    }
}
