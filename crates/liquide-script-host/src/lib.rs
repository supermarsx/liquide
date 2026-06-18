//! A sandboxed TypeScript/JavaScript app + element host for liquide.
//!
//! This crate lets a desktop-environment app or element be authored in
//! **TypeScript**: [`swc`](https://swc.rs) transpiles the TS source to plain JS
//! (stripping types), [`boa`](https://github.com/boa-dev/boa) — a pure-Rust JS
//! engine — executes that JS, and the script produces a UI as an
//! [`liquide_interop::AppWidgetModel`] (the existing toolkit-free app seam) so a
//! scripted "app"/element renders through the normal CSS-driven app pipeline,
//! exactly like the WASM host ([`liquide-wasm-host`](https://docs.rs)) but with a
//! JS/TS authoring model instead of a compiled WASM module.
//!
//! ## The pipeline
//!
//! ```text
//!   TS source ──swc──▶ JS  ──boa──▶ run module ──render()──▶ JS object
//!                                                            │ serde_json
//!                                                            ▼
//!                                                      AppWidgetModel
//! ```
//!
//! 1. [`transpile_ts`] runs swc: parse the TS, strip the types, emit JS. Parse /
//!    type-strip errors are reported with source location (never a panic).
//! 2. The JS is executed in a boa [`Context`](boa_engine::Context). The module is
//!    expected to define a `render()` function.
//! 3. `render()` returns a JSON-serialisable object describing the UI. The host
//!    `JSON.stringify`s it and `serde`-deserialises the bytes into an
//!    [`AppWidgetModel`]. (See "Authoring contract" below for why JSON-return was
//!    chosen over a host-bound builder API.)
//! 4. [`ScriptHost::apply_action`] serialises an [`AppWidgetAction`] into the JS
//!    context, calls the optional `apply_action(action)` function, and reports
//!    whether the model changed — matching the `AppView` `apply_action` seam.
//!
//! ## Authoring contract — *JSON-returning `render()`* (justified)
//!
//! Two designs were possible: (a) the script returns a JSON-serialisable object
//! the host deserialises via serde, or (b) the script calls a set of host-bound
//! builder functions (`de.panel()`, `de.button()`, …) that assemble the model in
//! Rust. **We chose (a).** Justification:
//!
//! - **Minimal JS↔Rust surface.** A builder API would require binding a large,
//!   stateful, perf-sensitive object graph into boa and keeping it in lock-step
//!   with the `AppWidget` enum forever. The JSON route binds *nothing* for the
//!   model path — `JSON.stringify` is already in the engine — so the entire
//!   authoring contract is "return an object shaped like `AppWidgetModel`", and
//!   `serde` is the single, already-tested validator/decoder.
//! - **Tight sandbox.** Fewer host functions = smaller attack surface. The only
//!   host binding we add is a bounded `console.log`.
//! - **It mirrors the WASM host.** `liquide-wasm-host`'s guest also serialises an
//!   `AppWidgetModel` (as JSON in linear memory) and the host deserialises it;
//!   keeping the same contract here means the future shell wiring is uniform.
//!
//! The TS author writes (types are illustrative — they are stripped at
//! transpile time):
//!
//! ```typescript
//! interface Widget { type: string; [k: string]: unknown }
//! interface Model { title?: string; root: Widget[] }
//!
//! export function render(): Model {
//!   return {
//!     title: "Hello",
//!     root: [{ type: "panel", children: [
//!       { type: "button", id: "ok", label: "OK", kind: "primary" },
//!     ]}],
//!   };
//! }
//! ```
//!
//! ## Sandbox posture (and boa's interruption caveat)
//!
//! boa has **no ambient IO** by default — no `fetch`, no `require`, no `fs`, no
//! real-clock `Date.now` side channel into the host, no network. We add **only**
//! a bounded `console.log` (captured into a host-side ring, never touching real
//! stdout/files). A script that references an unbound global (`fetch`, `require`,
//! `process`, `fs`) fails cleanly with a JS `ReferenceError`, surfaced as
//! [`ScriptHostError::Runtime`] — not a host crash.
//!
//! ### Runaway-script watchdog (wall-clock execution deadline)
//!
//! Unlike `wasmtime` (fuel + epoch), boa 0.21 has **no general preemptive
//! interruption / instruction budget** — a `while (true) {}` spins whatever
//! thread runs it, and the host call would never return. This crate bounds that
//! at the library level with **two layers**:
//!
//! 1. **Wall-clock deadline on a dedicated worker thread (the real kill).** A
//!    [`ScriptHost`] owns the boa [`Context`](boa_engine::Context) on its OWN
//!    long-lived worker thread (the context is `!Send`, so it never crosses
//!    threads — it is created and driven there for the host's whole life). Each
//!    [`render`](ScriptHostApi::render)/[`apply_action`](ScriptHostApi::apply_action)
//!    sends a command to the worker and waits for the reply with a
//!    [`recv_timeout`](std::sync::mpsc::Receiver::recv_timeout) of
//!    [`execution_timeout`](ScriptSandboxConfig::execution_timeout). If the
//!    deadline passes first, the call returns [`ScriptHostError::Timeout`]
//!    **promptly** and the worker is **abandoned** (a fresh one is spawned for
//!    the next call). **Guarantee:** the host call — and therefore the DE event
//!    loop — is never blocked past the deadline by a runaway script.
//!    **Honest limitation:** boa cannot be force-killed mid-eval, so the
//!    abandoned worker thread keeps running the runaway code until it next
//!    *yields* (returns from the call / hits a layer-2 limit). It consumes one
//!    background thread + CPU until then. This is a weaker kill than wasmtime's
//!    epoch interruption; *compute-heavy* untrusted code is better served by the
//!    WASM host.
//! 2. **boa's in-VM loop-iteration limit (defense-in-depth, makes the abandoned
//!    thread actually die in the common case).** boa 0.21 *does* expose one
//!    synchronous in-context interruption hook:
//!    `RuntimeLimits::set_loop_iteration_limit`, which makes a loop back-edge
//!    throw once a per-frame iteration count is exceeded. We arm it from
//!    [`max_loop_iterations`](ScriptSandboxConfig::max_loop_iterations) so the
//!    archetypal `while (true) {}` self-terminates with a clean `Runtime` error
//!    (and an *abandoned* worker stuck in such a loop dies on its own rather than
//!    spinning forever). It is **not** a general budget: it bounds loop
//!    back-edges per call frame only — deep recursion, a single enormous
//!    non-loop expression, or many separate bounded loops are NOT caught by it
//!    (that is exactly why layer 1, the wall-clock deadline, is the real
//!    guarantee).
//!
//! See [`ScriptSandboxConfig`] for the full set of bounds and the threat-model
//! table.
//!
//! ## Feature gating
//!
//! The real boa+swc host lives behind the **`script`** cargo feature, which is
//! **off by default**. With it off, [`NullScriptHost`] provides the identical API
//! and returns [`ScriptHostError::Unavailable`] from every operation, so the
//! workspace builds and tests without paying boa's / swc's build cost.

#![forbid(unsafe_code)]

pub mod config;

pub use config::ScriptSandboxConfig;
pub use liquide_interop::{AppWidgetAction, AppWidgetModel};

use thiserror::Error;

/// The name of the function the script must export/define to produce the UI.
pub const RENDER_FN: &str = "render";
/// The name of the optional function the script defines to handle an action.
pub const APPLY_ACTION_FN: &str = "apply_action";

/// A transpile (parse / type-strip) diagnostic with source location.
///
/// Returned inside [`ScriptHostError::Transpile`] so a syntax error in the TS is
/// reported usefully (line/column + message) rather than panicking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranspileDiagnostic {
    /// Human-readable message (e.g. "Expected ';', got '}'").
    pub message: String,
    /// 1-based line number, if known.
    pub line: Option<usize>,
    /// 1-based column number, if known.
    pub column: Option<usize>,
}

impl std::fmt::Display for TranspileDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.line, self.column) {
            (Some(l), Some(c)) => write!(f, "{}:{}: {}", l, c, self.message),
            (Some(l), None) => write!(f, "{}: {}", l, self.message),
            _ => write!(f, "{}", self.message),
        }
    }
}

/// Errors a script host can produce.
#[derive(Debug, Error)]
pub enum ScriptHostError {
    /// The `script` feature is disabled, so no real engine is available.
    /// Returned by every [`NullScriptHost`] operation.
    #[error("script host unavailable: built without the `script` feature")]
    Unavailable,

    /// The source exceeded the configured byte cap before transpiling.
    #[error("source too large: {got} bytes exceeds the {cap}-byte cap")]
    SourceTooLarge { got: usize, cap: usize },

    /// swc failed to parse the TS or strip its types. Carries the diagnostics
    /// (with location) rather than a panic.
    #[error("transpile failed: {}", format_diags(.0))]
    Transpile(Vec<TranspileDiagnostic>),

    /// The transpiled JS failed to evaluate / threw, or `render()` is missing or
    /// not callable. A *runtime* error in the script lands here — it is caught
    /// and reported, never a host crash.
    #[error("script runtime error: {0}")]
    Runtime(String),

    /// The value `render()` produced could not be turned into JSON, or exceeded
    /// the configured model byte cap.
    #[error("script produced an invalid model value: {0}")]
    BadModel(String),

    /// The JSON the script produced did not deserialise into an
    /// [`AppWidgetModel`].
    #[error("failed to decode the script's widget model: {0}")]
    Decode(String),

    /// The script did not return within the configured wall-clock execution
    /// deadline (see [`ScriptSandboxConfig::execution_timeout`]). The host call
    /// is unblocked promptly; the runaway script is abandoned on its worker
    /// thread (boa cannot be force-killed mid-eval, so that thread keeps running
    /// until it next yields — but the desktop environment is NOT blocked).
    #[error("script execution timed out after {0:?}")]
    Timeout(std::time::Duration),
}

fn format_diags(diags: &[TranspileDiagnostic]) -> String {
    diags
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ")
}

/// Result alias for host operations.
pub type Result<T> = std::result::Result<T, ScriptHostError>;

#[cfg(feature = "script")]
mod transpile;
#[cfg(feature = "script")]
pub use transpile::transpile_ts;

#[cfg(feature = "script")]
mod host;
#[cfg(feature = "script")]
pub use host::ScriptHost;

mod null;
pub use null::NullScriptHost;

/// The behaviour every script host (real or null) exposes.
///
/// This is the clean library boundary the shell/session will wire to later (a
/// follow-up — this crate deliberately does NOT touch the shell). Both
/// [`ScriptHost`] (feature `script`) and [`NullScriptHost`] implement it, so
/// callers can be written once against the trait and the concrete type is chosen
/// by the build's features (mirrors `liquide-wasm-host`'s `WasmHostApi`).
pub trait ScriptHostApi {
    /// Run the script's `render()` and return the [`AppWidgetModel`] it emitted.
    fn render(&mut self) -> Result<AppWidgetModel>;

    /// Deliver an [`AppWidgetAction`] to the script's `apply_action(action)`
    /// function and report whether the model changed (so the caller knows to
    /// re-`render`). If the script defines no `apply_action`, this reports `false`
    /// (no change) rather than erroring — matching the `AppView` default.
    fn apply_action(&mut self, action: &AppWidgetAction) -> Result<bool>;
}
