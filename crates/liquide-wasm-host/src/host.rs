//! The real, `wasmtime`-backed sandboxed host (feature `wasm`).
//!
//! See [`crate`] docs for the interaction model and [`crate::config`] for the
//! threat model. This module wires the four containment mechanisms — fuel,
//! epoch interruption, `StoreLimits`, and a no-WASI/deny-by-default linker — and
//! exposes the `(ptr, len)` model-emit protocol.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use wasmtime::{
    Caller, Config, Engine, Instance, Linker, Memory, Module, Store, StoreLimits,
    StoreLimitsBuilder,
};

use crate::{
    APPLY_ACTION_EXPORT, AppWidgetAction, AppWidgetModel, HOST_MODULE, MEMORY_EXPORT, RENDER_EXPORT,
    Result, WasmHostApi, WasmHostError, WasmSandboxConfig,
};

/// Per-`Store` state. Holds the `StoreLimits` (consulted by the
/// `ResourceLimiter`) plus a place for bounded host-call side effects.
struct HostState {
    limits: StoreLimits,
    /// Bytes logged by the guest this run (bounded by `max_log_bytes`). Exposed
    /// for tests and future routing to the DE log.
    logs: Vec<String>,
    config: WasmSandboxConfig,
}

/// A loaded, sandboxed WASM module ready to be run.
///
/// One `WasmHost` owns one compiled [`Module`] and the [`Engine`] it was
/// compiled with. Each [`render`](WasmHostApi::render) / [`apply_action`] call
/// instantiates a **fresh** [`Store`] + [`Instance`] so a module cannot carry
/// hidden state between calls and a trap in one call cannot poison the next.
pub struct WasmHost {
    engine: Engine,
    module: Module,
    config: WasmSandboxConfig,
}

impl WasmHost {
    /// Compile a module from raw bytes (`.wasm`) or, since the `wat` feature is
    /// enabled, from WebAssembly text — wasmtime auto-detects which.
    ///
    /// # Errors
    ///
    /// [`WasmHostError::Load`] if the bytes fail to compile or validate.
    pub fn from_bytes(wasm: &[u8], config: WasmSandboxConfig) -> Result<Self> {
        let mut cfg = Config::new();
        // CPU bound: meter every instruction.
        cfg.consume_fuel(true);
        // Wall-clock bound: a watchdog will bump the epoch on deadline.
        cfg.epoch_interruption(true);

        let engine =
            Engine::new(&cfg).map_err(|e| WasmHostError::Load(format!("engine: {e}")))?;
        let module =
            Module::new(&engine, wasm).map_err(|e| WasmHostError::Load(e.to_string()))?;

        Ok(Self {
            engine,
            module,
            config,
        })
    }

    /// Compile a module with default sandbox limits.
    ///
    /// # Errors
    ///
    /// See [`from_bytes`](Self::from_bytes).
    pub fn from_bytes_default(wasm: &[u8]) -> Result<Self> {
        Self::from_bytes(wasm, WasmSandboxConfig::default())
    }

    /// The bounded log lines produced by the most recent run that captured them.
    /// (Convenience for callers/tests; the real DE would route these to its log.)
    #[must_use]
    pub fn config(&self) -> WasmSandboxConfig {
        self.config
    }

    /// Build a fresh `Store` with the limiter, fuel, and epoch deadline applied,
    /// plus a linker carrying ONLY the explicit host functions (no WASI).
    fn fresh_instance(&self) -> Result<(Store<HostState>, Instance)> {
        let limits = StoreLimitsBuilder::new()
            .memory_size(self.config.max_memory_bytes)
            .table_elements(self.config.max_table_elements)
            // One memory, a handful of tables, one instance: a UI module needs no
            // more, and capping them blocks instance/table blow-up.
            .memories(1)
            .tables(4)
            .instances(1)
            .build();

        let state = HostState {
            limits,
            logs: Vec::new(),
            config: self.config,
        };
        let mut store = Store::new(&self.engine, state);

        // Wire the ResourceLimiter so memory.grow / table.grow are checked
        // against StoreLimits.
        store.limiter(|s| &mut s.limits);

        // CPU: grant this call its fuel budget.
        store
            .set_fuel(self.config.fuel_per_call)
            .map_err(|e| WasmHostError::Instantiate(format!("set_fuel: {e}")))?;

        // Wall-clock: because epoch interruption is enabled on the engine, EVERY
        // store must declare a deadline or wasmtime traps at the default (0)
        // epoch. When a deadline is configured we trap after one epoch tick and
        // the watchdog (below) bumps the epoch exactly once when the wall-clock
        // deadline elapses. When the deadline is disabled (0) we set the epoch
        // deadline to effectively-never so ONLY fuel bounds the call.
        if self.config.epoch_deadline_ms > 0 {
            store.set_epoch_deadline(1);
        } else {
            store.set_epoch_deadline(u64::MAX);
        }

        // Deny-by-default linker: NO WASI, NO ambient anything. The only import
        // the guest may resolve is the bounded `host::log`.
        let mut linker: Linker<HostState> = Linker::new(&self.engine);
        linker
            .func_wrap(
                HOST_MODULE,
                "log",
                |mut caller: Caller<'_, HostState>, level: i32, ptr: i32, len: i32| {
                    let cap = caller.data().config.max_log_bytes;
                    // Resolve guest memory; if absent or OOB, silently drop (a
                    // host-call must never trap the host or read OOB).
                    let Some(mem) = caller
                        .get_export(MEMORY_EXPORT)
                        .and_then(|e| e.into_memory())
                    else {
                        return;
                    };
                    let len = (len.max(0) as usize).min(cap);
                    let ptr = ptr.max(0) as usize;
                    let data = mem.data(&caller);
                    let Some(slice) = data.get(ptr..ptr.saturating_add(len)) else {
                        return;
                    };
                    let msg = String::from_utf8_lossy(slice).into_owned();
                    caller.data_mut().logs.push(format!("[{level}] {msg}"));
                },
            )
            .map_err(|e| WasmHostError::Instantiate(format!("link host::log: {e}")))?;

        // NOTE: we intentionally use `instantiate` (not `instantiate_pre`) and do
        // NOT add a WASI ctx. Any import the module needs beyond `host::log` is
        // unsatisfiable -> instantiation fails -> the module is rejected. That IS
        // the deny-by-default guarantee.
        let instance = linker
            .instantiate(&mut store, &self.module)
            .map_err(|e| WasmHostError::Instantiate(e.to_string()))?;

        Ok((store, instance))
    }

    /// Spawn a one-shot watchdog that bumps the engine epoch after the deadline,
    /// interrupting (trapping) a hung call. Returns a guard whose drop signals
    /// the watchdog to stop early when the call finishes in time.
    fn spawn_epoch_watchdog(&self) -> Option<WatchdogGuard> {
        if self.config.epoch_deadline_ms == 0 {
            return None;
        }
        let engine = self.engine.clone();
        let deadline = Duration::from_millis(self.config.epoch_deadline_ms);
        let done = Arc::new(AtomicBool::new(false));
        let done_thread = done.clone();
        let handle = thread::spawn(move || {
            // Poll in small slices so a fast call doesn't keep the thread alive
            // for the full deadline, while still firing if the call hangs.
            let slice = Duration::from_millis(5).min(deadline);
            let mut elapsed = Duration::ZERO;
            while elapsed < deadline {
                if done_thread.load(Ordering::Acquire) {
                    return;
                }
                thread::sleep(slice);
                elapsed += slice;
            }
            // Deadline reached without the call finishing: interrupt the guest.
            engine.increment_epoch();
        });
        Some(WatchdogGuard {
            done,
            handle: Some(handle),
        })
    }

    /// Read a guest `(ptr, len)` span (packed in an `i64`) back as owned bytes,
    /// bounds-checked against the instance's own memory and the byte cap.
    fn read_packed_span(
        store: &mut Store<HostState>,
        memory: &Memory,
        packed: i64,
        cap: usize,
    ) -> Result<Vec<u8>> {
        // High 32 bits = ptr, low 32 bits = len. Unsigned to avoid sign issues.
        let ptr = ((packed as u64) >> 32) as usize;
        let len = ((packed as u64) & 0xFFFF_FFFF) as usize;
        if len > cap {
            return Err(WasmHostError::BadSpan(format!(
                "len {len} exceeds cap {cap}"
            )));
        }
        let data = memory.data(&*store);
        let end = ptr
            .checked_add(len)
            .ok_or_else(|| WasmHostError::BadSpan("ptr+len overflow".into()))?;
        let slice = data.get(ptr..end).ok_or_else(|| {
            WasmHostError::BadSpan(format!(
                "span {ptr}..{end} out of bounds (mem {} bytes)",
                data.len()
            ))
        })?;
        Ok(slice.to_vec())
    }

    /// Allocate `bytes.len()` in the guest via its exported `alloc(len) -> ptr`
    /// (used to deliver an action). Returns the guest pointer.
    fn guest_alloc(store: &mut Store<HostState>, instance: &Instance, len: usize) -> Result<usize> {
        let alloc = instance
            .get_typed_func::<i32, i32>(&mut *store, "alloc")
            .map_err(|_| WasmHostError::MissingExport("alloc".into()))?;
        let ptr = alloc
            .call(&mut *store, len as i32)
            .map_err(|e| map_trap(&store, e))?;
        Ok(ptr.max(0) as usize)
    }
}

/// Maps a wasmtime call error to a [`WasmHostError`], distinguishing a fuel/epoch
/// trap (the containment firing) from other failures.
fn map_trap(store: &Store<HostState>, e: wasmtime::Error) -> WasmHostError {
    let _ = store;
    // wasmtime surfaces out-of-fuel and epoch interruption as a Trap. Either way
    // the guest was stopped by the sandbox; report it as a trap with the detail.
    WasmHostError::Trap(e.to_string())
}

/// Drop guard that stops the epoch watchdog as soon as the call returns, so a
/// fast call doesn't leave a thread spinning and doesn't bump the epoch late.
struct WatchdogGuard {
    done: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Drop for WatchdogGuard {
    fn drop(&mut self) {
        self.done.store(true, Ordering::Release);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl WasmHostApi for WasmHost {
    fn render(&mut self) -> Result<AppWidgetModel> {
        let (mut store, instance) = self.fresh_instance()?;

        let memory = instance
            .get_memory(&mut store, MEMORY_EXPORT)
            .ok_or_else(|| WasmHostError::MissingExport(MEMORY_EXPORT.into()))?;

        let render = instance
            .get_typed_func::<(), i64>(&mut store, RENDER_EXPORT)
            .map_err(|_| WasmHostError::MissingExport(RENDER_EXPORT.into()))?;

        // Arm the wall-clock watchdog only for the duration of the call.
        let packed = {
            let _guard = self.spawn_epoch_watchdog();
            render
                .call(&mut store, ())
                .map_err(|e| map_trap(&store, e))?
        };

        let bytes =
            Self::read_packed_span(&mut store, &memory, packed, self.config.max_model_bytes)?;
        serde_json::from_slice(&bytes).map_err(|e| WasmHostError::Decode(e.to_string()))
    }

    fn apply_action(&mut self, action: &AppWidgetAction) -> Result<bool> {
        let (mut store, instance) = self.fresh_instance()?;

        let memory = instance
            .get_memory(&mut store, MEMORY_EXPORT)
            .ok_or_else(|| WasmHostError::MissingExport(MEMORY_EXPORT.into()))?;

        // Serialise the action and hand it to the guest at a guest-allocated ptr.
        let payload = serde_json::to_vec(action)
            .map_err(|e| WasmHostError::Decode(format!("encode action: {e}")))?;
        if payload.len() > self.config.max_model_bytes {
            return Err(WasmHostError::BadSpan("action payload exceeds cap".into()));
        }
        let ptr = Self::guest_alloc(&mut store, &instance, payload.len())?;
        memory
            .write(&mut store, ptr, &payload)
            .map_err(|e| WasmHostError::BadSpan(format!("write action: {e}")))?;

        let apply = instance
            .get_typed_func::<(i32, i32), i32>(&mut store, APPLY_ACTION_EXPORT)
            .map_err(|_| WasmHostError::MissingExport(APPLY_ACTION_EXPORT.into()))?;

        let changed = {
            let _guard = self.spawn_epoch_watchdog();
            apply
                .call(&mut store, (ptr as i32, payload.len() as i32))
                .map_err(|e| map_trap(&store, e))?
        };
        Ok(changed != 0)
    }
}

impl WasmHost {
    /// Run `render` and also return the bounded log lines the guest emitted —
    /// used by tests to prove the `host::log` surface works and that NO other
    /// host authority is reachable.
    ///
    /// # Errors
    ///
    /// Same as [`render`](WasmHostApi::render).
    pub fn render_with_logs(&mut self) -> Result<(AppWidgetModel, Vec<String>)> {
        let (mut store, instance) = self.fresh_instance()?;
        let memory = instance
            .get_memory(&mut store, MEMORY_EXPORT)
            .ok_or_else(|| WasmHostError::MissingExport(MEMORY_EXPORT.into()))?;
        let render = instance
            .get_typed_func::<(), i64>(&mut store, RENDER_EXPORT)
            .map_err(|_| WasmHostError::MissingExport(RENDER_EXPORT.into()))?;
        let packed = {
            let _guard = self.spawn_epoch_watchdog();
            render
                .call(&mut store, ())
                .map_err(|e| map_trap(&store, e))?
        };
        let bytes =
            Self::read_packed_span(&mut store, &memory, packed, self.config.max_model_bytes)?;
        let model: AppWidgetModel =
            serde_json::from_slice(&bytes).map_err(|e| WasmHostError::Decode(e.to_string()))?;
        let logs = std::mem::take(&mut store.data_mut().logs);
        Ok((model, logs))
    }
}
