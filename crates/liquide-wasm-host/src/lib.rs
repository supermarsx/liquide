//! A sandboxed WebAssembly runtime host for liquide.
//!
//! This crate loads and runs an **untrusted** WASM module that produces a UI as
//! an [`liquide_interop::AppWidgetModel`] — the existing toolkit-free app seam —
//! so a WASM "app"/element renders through the normal CSS-driven app pipeline
//! without the module ever touching pixels, the GPU, or any ambient OS authority.
//!
//! ## Sandbox (the whole point)
//!
//! The runtime is built around [`wasmtime`] with a deny-by-default posture. See
//! [`config::WasmSandboxConfig`] for the full threat model. In short:
//!
//! - **Fuel** bounds CPU (refilled per call, traps on exhaustion).
//! - **Epoch interruption** bounds wall-clock (a watchdog kills a hung module).
//! - **`StoreLimits`** caps linear memory + tables.
//! - **No WASI** — zero ambient filesystem / network / env / clock / RNG. The
//!   module's only authority is the explicit host functions we export (today,
//!   just a bounded `log`).
//!
//! ## Interaction model
//!
//! The guest module exports:
//!
//! - `render() -> i64` — builds an [`AppWidgetModel`], serialises it (JSON) into
//!   its own linear memory, and returns the `(ptr, len)` packed into an `i64`
//!   (`ptr` in the high 32 bits, `len` in the low 32 bits). The host
//!   bounds-checks the span, reads it back, and deserialises into an
//!   [`AppWidgetModel`].
//! - `apply_action(ptr, len) -> i32` *(documented next step, see
//!   [`WasmHost::apply_action`])* — receives a serialised [`AppWidgetAction`]
//!   the host wrote into guest memory and returns non-zero if the model changed.
//!
//! The guest may import a small, bounded host surface (currently only
//! `host::log(level, ptr, len)`); it grows ONLY via explicit, allow-listed
//! capabilities.
//!
//! ## Feature gating
//!
//! The real wasmtime-backed host lives behind the **`wasm`** cargo feature,
//! which is **off by default**. With the feature off, [`NullWasmHost`] provides
//! the identical API and returns [`WasmHostError::Unavailable`] from every
//! operation, so the workspace builds and tests without paying wasmtime's build
//! cost (the same pattern as the platform `Null*` hosts).

#![forbid(unsafe_code)]

pub mod config;

pub use config::WasmSandboxConfig;
pub use liquide_interop::{AppWidgetAction, AppWidgetModel};

use thiserror::Error;

/// The exported entry the host calls to obtain the UI model.
pub const RENDER_EXPORT: &str = "render";
/// The exported entry the host calls to deliver an action (next-step seam).
pub const APPLY_ACTION_EXPORT: &str = "apply_action";
/// The linear-memory export the host reads model bytes from.
pub const MEMORY_EXPORT: &str = "memory";
/// The host module namespace imported by the guest for host calls.
pub const HOST_MODULE: &str = "host";

/// Errors a WASM host can produce.
#[derive(Debug, Error)]
pub enum WasmHostError {
    /// The `wasm` feature is disabled, so no real runtime is available. Returned
    /// by every [`NullWasmHost`] operation.
    #[error("wasm host unavailable: built without the `wasm` feature")]
    Unavailable,

    /// The module bytes failed to compile / validate.
    #[error("failed to load module: {0}")]
    Load(String),

    /// The module is missing a required export (e.g. `render` or `memory`).
    #[error("module is missing required export: {0}")]
    MissingExport(String),

    /// Instantiation failed (e.g. an unsatisfiable import, or a limit hit while
    /// setting up the instance).
    #[error("failed to instantiate module: {0}")]
    Instantiate(String),

    /// The guest trapped during execution. This is the path taken on fuel
    /// exhaustion and on epoch (wall-clock) interruption.
    #[error("guest trapped: {0}")]
    Trap(String),

    /// The guest returned a `(ptr, len)` span that is out of bounds, or whose
    /// length exceeds the configured cap.
    #[error("guest returned an invalid memory span: {0}")]
    BadSpan(String),

    /// The bytes the guest produced did not deserialise into the expected model.
    #[error("failed to decode the guest's widget model: {0}")]
    Decode(String),
}

/// Result alias for host operations.
pub type Result<T> = std::result::Result<T, WasmHostError>;

#[cfg(feature = "wasm")]
mod host;
#[cfg(feature = "wasm")]
pub use host::WasmHost;

mod null;
pub use null::NullWasmHost;

/// The behaviour every WASM host (real or null) exposes.
///
/// This is the clean library boundary the shell/session will wire to later (a
/// follow-up — this crate deliberately does NOT touch the shell). Both
/// [`WasmHost`] (feature `wasm`) and [`NullWasmHost`] implement it, so callers
/// can be written once against the trait and the concrete type is chosen by the
/// build's features.
pub trait WasmHostApi {
    /// Run the module's `render()` entry under the sandbox limits and return the
    /// [`AppWidgetModel`] it emitted.
    fn render(&mut self) -> Result<AppWidgetModel>;

    /// Deliver an [`AppWidgetAction`] to the module's `apply_action()` entry and
    /// report whether the model changed (so the caller knows to re-`render`).
    ///
    /// This is the documented next step that matches the `AppView`
    /// `apply_action` seam; the real host implements it, the null host returns
    /// [`WasmHostError::Unavailable`].
    fn apply_action(&mut self, action: &AppWidgetAction) -> Result<bool>;
}
