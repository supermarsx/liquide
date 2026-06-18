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
//! | **Unbounded CPU / wall-clock hang**     | **Bounded by a wall-clock watchdog (the real kill) + boa's loop-iteration limit (defense-in-depth).** The host owns boa on a dedicated worker thread and waits for each `render`/`apply_action` reply with a [`execution_timeout`](ScriptSandboxConfig::execution_timeout) deadline; on overrun it returns [`Timeout`](crate::ScriptHostError::Timeout) **promptly** and abandons the worker (a fresh one is spawned next call) — the DE is never blocked. boa 0.21's one in-VM hook, `set_loop_iteration_limit` (armed from [`max_loop_iterations`](ScriptSandboxConfig::max_loop_iterations)), makes the archetypal `while(true){}` self-terminate so an *abandoned* worker dies on its own. **Limitation (honest):** boa can't be force-killed mid-eval, so the abandoned thread runs until it next yields (consuming one bg thread + CPU until then); the loop limit bounds loop back-edges per frame only, not deep recursion / a single huge non-loop expression. Weaker than `wasmtime`'s epoch interruption — compute-heavy untrusted code is better served by the WASM host. |
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

    /// Wall-clock deadline for a single `render`/`apply_action` call. If the
    /// script does not return within this, the host call returns
    /// [`ScriptHostError::Timeout`](crate::ScriptHostError::Timeout) and the
    /// runaway worker thread is abandoned (the DE is unblocked promptly). This is
    /// the real runaway-script kill — see the crate-level "Runaway-script
    /// watchdog" docs for the guarantee and its limitation.
    pub execution_timeout: std::time::Duration,

    /// boa in-VM loop-iteration limit (per call frame), armed via
    /// `RuntimeLimits::set_loop_iteration_limit`. Defense-in-depth so the
    /// archetypal `while (true) {}` self-terminates with a clean runtime error
    /// (and an abandoned worker stuck in such a loop dies on its own rather than
    /// spinning forever). `u64::MAX` disables it. It bounds loop back-edges only,
    /// NOT deep recursion or a single huge non-loop expression — the wall-clock
    /// `execution_timeout` is the general guarantee.
    pub max_loop_iterations: u64,
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
            // A UI render/action should be near-instant. 2s is a generous
            // ceiling that still bounds a hang to a fraction of a perceptible
            // freeze while leaving ample headroom for a legitimate (cold swc /
            // first-eval) call on a slow machine.
            execution_timeout: std::time::Duration::from_secs(2),
            // ~10M loop back-edges per frame: far above any sane UI loop, low
            // enough that a `while(true){}` self-terminates in well under a
            // second on the abandoned worker. Defense-in-depth under the
            // wall-clock deadline, not the primary bound.
            max_loop_iterations: 10_000_000,
        }
    }
}
