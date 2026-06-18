//! Sandbox configuration and the threat model it enforces.
//!
//! # Threat model
//!
//! A WASM "app"/element is **untrusted code** running inside the desktop
//! environment's address space. The host must assume the module is hostile and
//! contain every axis along which it could damage or starve the DE:
//!
//! | Threat                                   | Containment (this crate)                                            |
//! |------------------------------------------|---------------------------------------------------------------------|
//! | Unbounded CPU (e.g. tight compute loop)  | **Fuel metering** — every instruction costs fuel; a budget is set per call and refilled per call; running out **traps** the instance instead of burning a core. |
//! | Wall-clock hang (e.g. `loop {}`, or a long syscall-free spin that fuel alone bounds expensively) | **Epoch interruption** — a watchdog bumps the engine epoch; a module that exceeds its deadline is interrupted and trapped, even mid-loop. |
//! | Memory exhaustion (grow linear memory to OOM the host) | **`StoreLimits`** caps total linear-memory bytes; a `memory.grow` past the cap fails (returns -1 to the guest) rather than allocating. |
//! | Table / instance blow-up                 | **`StoreLimits`** caps table elements, instances, tables and memories per store. |
//! | Ambient authority (filesystem, network, env, clock, RNG, stdio) | **WASI is NOT linked.** No preopened dirs, no stdio, no clocks, no random. The module's ONLY authority is the explicit host functions we export (currently just a bounded `log`). Deny-by-default. |
//! | Host-memory OOB via guest pointers       | Every `(ptr, len)` the guest hands the host is **bounds-checked** against the instance's own linear memory before any read; oversized payloads are rejected by a byte cap. |
//!
//! What is intentionally NOT yet granted (documented next steps, see crate docs):
//! file IO, network, persistent storage, timers, and a guest event-callback are
//! all future *brokered* capabilities — each must be an explicit, allow-listed
//! host function, never ambient WASI.

/// Tunable sandbox limits for a single WASM module instance.
///
/// Every field is a hard ceiling. The defaults are deliberately tight — a UI
/// module that emits a widget model needs very little CPU or memory; a module
/// that wants more must be granted it explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WasmSandboxConfig {
    /// Fuel granted to the instance for a single entry-point call (e.g. one
    /// `render()`). One unit is consumed per executed wasm instruction (roughly).
    /// When it reaches zero the call traps. Refilled before every call.
    pub fuel_per_call: u64,

    /// Wall-clock deadline for a single entry-point call, in milliseconds. A
    /// watchdog bumps the engine epoch once this elapses, interrupting (trapping)
    /// the call even if it is a syscall-free busy loop that fuel would only bound
    /// at great expense. `0` disables the epoch deadline (NOT recommended).
    pub epoch_deadline_ms: u64,

    /// Maximum total linear-memory bytes the instance may hold. A `memory.grow`
    /// that would exceed this fails (the guest sees -1).
    pub max_memory_bytes: usize,

    /// Maximum number of elements across all tables in the instance.
    pub max_table_elements: usize,

    /// Maximum serialized-model payload, in bytes, the host will read back from
    /// guest memory. Bounds host-side allocation regardless of what the guest
    /// claims its `(ptr, len)` spans.
    pub max_model_bytes: usize,

    /// Maximum bytes a single `log` host-call may emit. Bounds log spam / host
    /// allocation.
    pub max_log_bytes: usize,
}

impl WasmSandboxConfig {
    /// Construct a config from the plugin-ABI manifest's requested memory,
    /// clamped to a sane ceiling so a manifest can lower (but not raise) the
    /// cap beyond what the host is willing to give. Other limits keep their
    /// defaults. This reuses the existing ABI resource vocabulary rather than
    /// inventing a parallel one.
    #[must_use]
    pub fn from_manifest(manifest: &liquide_plugin_abi::PluginManifest) -> Self {
        let mut cfg = Self::default();
        let requested = manifest.requested_memory_bytes as usize;
        if requested > 0 {
            cfg.max_memory_bytes = requested.min(cfg.max_memory_bytes);
        }
        cfg
    }
}

impl Default for WasmSandboxConfig {
    fn default() -> Self {
        Self {
            // ~10M instructions: ample for building a UI model, trivially small
            // for the host, and small enough that a runaway loop trips quickly.
            fuel_per_call: 10_000_000,
            // 250ms hard wall-clock ceiling per call.
            epoch_deadline_ms: 250,
            // 16 MiB linear memory.
            max_memory_bytes: 16 * 1024 * 1024,
            // 10k table elements.
            max_table_elements: 10_000,
            // 1 MiB serialized model cap.
            max_model_bytes: 1024 * 1024,
            // 8 KiB per log line.
            max_log_bytes: 8 * 1024,
        }
    }
}
