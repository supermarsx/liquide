//! The feature-off stub host.
//!
//! When the `script` feature is disabled, the workspace must still build and link
//! against this crate's public API without pulling in boa or swc.
//! `NullScriptHost` mirrors the real [`crate::ScriptHost`] surface and answers
//! every operation with [`ScriptHostError::Unavailable`], the same way
//! `liquide-platform`'s `Null*` hosts (and `liquide-wasm-host`'s `NullWasmHost`)
//! stand in for an absent backend.

use crate::{
    AppWidgetAction, AppWidgetModel, Result, ScriptHostApi, ScriptHostError, ScriptSandboxConfig,
};

/// A no-op script host used when the engine is not compiled in.
///
/// Construct it the same way you'd construct the real host (`from_source`), so
/// call sites are identical regardless of feature flags; every operation then
/// reports [`ScriptHostError::Unavailable`].
#[derive(Debug, Default, Clone)]
pub struct NullScriptHost {
    _config: ScriptSandboxConfig,
}

impl NullScriptHost {
    /// Construct a null host. The source and config are accepted (so the call
    /// site matches the real host) but ignored — nothing is transpiled or run.
    ///
    /// # Errors
    ///
    /// Never errors at construction; the absence of an engine surfaces from the
    /// [`ScriptHostApi`] operations, not from loading.
    pub fn from_source(_ts_source: &str, config: ScriptSandboxConfig) -> Result<Self> {
        Ok(Self { _config: config })
    }

    /// Construct a null host with default limits.
    ///
    /// # Errors
    ///
    /// Never errors.
    pub fn from_source_default(ts_source: &str) -> Result<Self> {
        Self::from_source(ts_source, ScriptSandboxConfig::default())
    }
}

impl ScriptHostApi for NullScriptHost {
    fn render(&mut self) -> Result<AppWidgetModel> {
        Err(ScriptHostError::Unavailable)
    }

    fn apply_action(&mut self, _action: &AppWidgetAction) -> Result<bool> {
        Err(ScriptHostError::Unavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_host_constructs_but_reports_unavailable() {
        let mut host =
            NullScriptHost::from_source_default("export function render(){return {root:[]}}")
                .expect("null host constructs");
        assert!(matches!(host.render(), Err(ScriptHostError::Unavailable)));
        let action = AppWidgetAction::new("k", "click", "");
        assert!(matches!(
            host.apply_action(&action),
            Err(ScriptHostError::Unavailable)
        ));
    }
}
