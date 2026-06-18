//! Sandbox configuration for the script host, and the threat model behind it.
//!
//! # Threat model (and what this crate can / cannot enforce)
//!
//! A TS/JS "app"/element is **untrusted code** executed in-process by boa. boa's
//! great virtue here is that it grants **no ambient authority**: there is no
//! `fetch`, no `require`, no `fs`, no network, no real-clock side channel —
//! nothing unless the host explicitly binds it. We bind only a bounded
//! `console.log`. So the *capability* surface is deny-by-default by construction.
//!
//! | Threat                                  | Containment (this crate)                                          |
//! |-----------------------------------------|-------------------------------------------------------------------|
//! | Ambient authority (fs/net/env/clock)    | **Nothing is bound.** boa has no IO by default; we add only a bounded `console.log`. An unbound global (`fetch`/`require`/`fs`) throws `ReferenceError`, surfaced as a clean error. |
//! | Oversized source                        | [`max_source_bytes`](ScriptSandboxConfig::max_source_bytes) cap, checked before swc runs. |
//! | Oversized produced model                | [`max_model_bytes`](ScriptSandboxConfig::max_model_bytes) cap on the JSON the script returns. |
//! | Log spam / host-alloc via logging       | [`max_log_bytes`](ScriptSandboxConfig::max_log_bytes) per line; the log ring is bounded. |
//! | **Unbounded CPU / wall-clock hang**     | **NOT enforceable in-library with this boa version.** boa has no preemptive instruction/loop budget, so `while(true){}` spins the calling thread. The realistic kill mechanism is to run the host on a worker thread with a wall-clock watchdog and abandon the context on overrun — a **shell-wiring** concern (the shell owns app threads), documented here and at crate level rather than baked into the seam. This is a weaker kill story than `wasmtime`'s epoch interruption; untrusted *compute-heavy* code is better served by the WASM host. |
//!
//! Future brokered capabilities (timers, storage, fetch via the network/file
//! capability work) must each be an explicit, allow-listed host function — never
//! ambient. None are granted today.

/// Tunable sandbox limits for a single scripted app/element.
///
/// Every field is a hard ceiling. The defaults are tight — a UI script that
/// emits a widget model needs very little.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptSandboxConfig {
    /// Maximum TypeScript/JavaScript source size, in bytes, the host will accept
    /// before transpiling. Bounds parser work / host allocation.
    pub max_source_bytes: usize,

    /// Maximum serialized-model payload, in bytes, the host will read back from
    /// the script's `render()` return value (after `JSON.stringify`). Bounds
    /// host-side allocation regardless of how large a model the script builds.
    pub max_model_bytes: usize,

    /// Maximum bytes a single `console.log` call may emit into the captured log
    /// ring. Bounds log spam / host allocation.
    pub max_log_bytes: usize,

    /// Maximum number of captured log lines retained (oldest dropped past this).
    pub max_log_lines: usize,
}

impl Default for ScriptSandboxConfig {
    fn default() -> Self {
        Self {
            // 1 MiB of source: generous for app-logic glue, small enough to bound
            // swc's work.
            max_source_bytes: 1024 * 1024,
            // 1 MiB serialized model cap (matches the wasm host's model cap).
            max_model_bytes: 1024 * 1024,
            // 8 KiB per log line.
            max_log_bytes: 8 * 1024,
            // Keep at most 256 captured log lines.
            max_log_lines: 256,
        }
    }
}
