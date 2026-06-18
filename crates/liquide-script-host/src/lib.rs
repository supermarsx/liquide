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
//! **Caveat (documented honestly):** unlike `wasmtime`, boa has **no preemptive
//! interruption / instruction budget** in this version — a `while (true) {}` in a
//! script would spin the calling thread. The realistic containment is to run the
//! host on a dedicated worker thread with a wall-clock watchdog that abandons the
//! context on overrun (the same shape the planning doc calls out for boa). That
//! threading harness is a **shell-wiring concern** (the shell owns app threads),
//! so this library keeps a clean synchronous boundary and documents the
//! limitation rather than baking a thread model into the seam. If a future boa
//! release exposes a job/loop budget or `JobQueue` interruption, wire it in
//! [`host`]. See [`ScriptSandboxConfig`] for the bounds this crate *can* enforce
//! today (output/source/model byte caps).
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
