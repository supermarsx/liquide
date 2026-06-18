//! The boa-backed script host (feature `script`).
//!
//! [`ScriptHost`] owns a boa [`Context`], the transpiled JS, and a captured log
//! ring — but it owns them **on a dedicated long-lived worker thread**, not on
//! the caller's thread. This is the runaway-script watchdog: the public
//! [`render`](ScriptHostApi::render)/[`apply_action`](ScriptHostApi::apply_action)
//! calls send a command to the worker and wait for the reply with a wall-clock
//! deadline ([`ScriptSandboxConfig::execution_timeout`]). If the script hangs,
//! the call returns [`ScriptHostError::Timeout`] promptly and the worker is
//! abandoned (a fresh one spawns on the next call), so the desktop environment
//! is never blocked. See the crate docs for the full guarantee + limitation.
//!
//! The boa `Context` is `!Send`, so it must be CREATED and DRIVEN on the worker
//! thread and never move. Only plain-data [`AppWidgetModel`]/[`AppWidgetAction`]
//! (and `String` source) cross the channel boundary, all of which are `Send`.
//!
//! The only host capability bound into the context is a bounded `console.log`;
//! everything else (`fetch`, `require`, `fs`, …) is absent, so a script that
//! touches one throws a `ReferenceError` we surface cleanly.

use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::Instant;

use boa_engine::gc::{Finalize, Gc, GcRefCell, Trace};
use boa_engine::object::JsObject;
use boa_engine::property::Attribute;
use boa_engine::{js_string, Context, JsError, JsValue, NativeFunction, Source};

use crate::{
    AppWidgetAction, AppWidgetModel, Result, ScriptHostApi, ScriptHostError, ScriptSandboxConfig,
    APPLY_ACTION_FN, RENDER_FN,
};

/// A command sent from the [`ScriptHost`] handle to its worker thread.
enum Command {
    Render,
    ApplyAction(AppWidgetAction),
    /// Drain and return the captured `console.log` lines.
    Logs,
}

/// A reply sent back from the worker to the handle.
enum Reply {
    Model(Result<AppWidgetModel>),
    Changed(Result<bool>),
    Logs(Vec<String>),
}

/// A bounded log ring shared (via boa's GC) between the engine and the bound
/// `console.log`. It holds no GC pointers, so its trace is empty.
#[derive(Debug, Default, Trace, Finalize)]
struct LogRing {
    lines: Vec<String>,
    #[unsafe_ignore_trace]
    max_lines: usize,
    #[unsafe_ignore_trace]
    max_bytes: usize,
}

impl LogRing {
    fn push(&mut self, mut line: String) {
        if line.len() > self.max_bytes {
            line.truncate(self.max_bytes);
        }
        if self.max_lines > 0 && self.lines.len() >= self.max_lines {
            self.lines.remove(0);
        }
        self.lines.push(line);
    }
}

/// The boa-owning state. Lives entirely on the worker thread (the `Context` is
/// `!Send`). Never exposed outside this module.
struct Engine {
    context: Context,
    config: ScriptSandboxConfig,
    log: Gc<GcRefCell<LogRing>>,
}

impl Engine {
    /// Transpile `ts_source` (swc) and load the resulting JS into a fresh boa
    /// context, arming the in-VM loop-iteration limit. Runs on the worker
    /// thread.
    fn load(ts_source: &str, config: ScriptSandboxConfig) -> Result<Self> {
        let js = crate::transpile_ts(ts_source)?;

        let mut context = Context::default();

        // Layer-2 watchdog (defense-in-depth): arm boa's in-VM loop-iteration
        // limit so a `while(true){}` self-terminates with a clean runtime error
        // even on an abandoned worker. The wall-clock deadline on the handle is
        // the general guarantee; this just keeps the common runaway from
        // spinning a leaked thread forever.
        context
            .runtime_limits_mut()
            .set_loop_iteration_limit(config.max_loop_iterations);

        let log = Gc::new(GcRefCell::new(LogRing {
            lines: Vec::new(),
            max_lines: config.max_log_lines,
            max_bytes: config.max_log_bytes,
        }));

        bind_console(&mut context, log.clone())?;

        // Run the (transpiled, export-lowered) script top level so
        // `render`/`apply_action` land on the global object. A throw here is a
        // clean runtime error, not a host crash.
        context
            .eval(Source::from_bytes(js.as_bytes()))
            .map_err(|e| ScriptHostError::Runtime(format_err(&e, &mut context)))?;

        Ok(Self {
            context,
            config,
            log,
        })
    }

    /// Look up a global function by name; `Runtime` error if missing / not
    /// callable.
    fn global_fn(&mut self, name: &str) -> Result<JsObject> {
        let val = self
            .context
            .global_object()
            .get(js_string!(name), &mut self.context)
            .map_err(|e| ScriptHostError::Runtime(format_err(&e, &mut self.context)))?;
        let obj = val.as_object().ok_or_else(|| {
            ScriptHostError::Runtime(format!("`{name}` is not defined as a function"))
        })?;
        if !obj.is_callable() {
            return Err(ScriptHostError::Runtime(format!("`{name}` is not callable")));
        }
        Ok(obj)
    }

    /// Is a callable global of this name present?
    fn has_global_fn(&mut self, name: &str) -> bool {
        self.context
            .global_object()
            .get(js_string!(name), &mut self.context)
            .ok()
            .and_then(|v| v.as_object())
            .map(|o| o.is_callable())
            .unwrap_or(false)
    }

    /// Stringify a returned JS value to JSON, enforce the model byte cap, and
    /// deserialise it into an [`AppWidgetModel`].
    fn decode_model(&mut self, value: &JsValue) -> Result<AppWidgetModel> {
        if value.is_undefined() || value.is_null() {
            return Err(ScriptHostError::BadModel(
                "render() returned undefined/null".into(),
            ));
        }
        let json = value.to_json(&mut self.context).map_err(|e| {
            ScriptHostError::BadModel(format!(
                "render()'s value is not JSON-serialisable: {}",
                format_err(&e, &mut self.context)
            ))
        })?;
        let json = json
            .ok_or_else(|| ScriptHostError::BadModel("render()'s value produced no JSON".into()))?;
        let text = serde_json::to_string(&json)
            .map_err(|e| ScriptHostError::BadModel(format!("could not serialise model: {e}")))?;
        if text.len() > self.config.max_model_bytes {
            return Err(ScriptHostError::BadModel(format!(
                "model is {} bytes, exceeds the {}-byte cap",
                text.len(),
                self.config.max_model_bytes
            )));
        }
        serde_json::from_str::<AppWidgetModel>(&text)
            .map_err(|e| ScriptHostError::Decode(e.to_string()))
    }

    fn render(&mut self) -> Result<AppWidgetModel> {
        let render = self.global_fn(RENDER_FN)?;
        let value = render
            .call(&JsValue::undefined(), &[], &mut self.context)
            .map_err(|e| ScriptHostError::Runtime(format_err(&e, &mut self.context)))?;
        self.decode_model(&value)
    }

    fn apply_action(&mut self, action: &AppWidgetAction) -> Result<bool> {
        // No apply_action defined → no-op, no change (matches AppView default).
        if !self.has_global_fn(APPLY_ACTION_FN) {
            return Ok(false);
        }
        let action_obj = action_to_js(action, &mut self.context)?;
        let apply = self.global_fn(APPLY_ACTION_FN)?;
        let result = apply
            .call(&JsValue::undefined(), &[action_obj], &mut self.context)
            .map_err(|e| ScriptHostError::Runtime(format_err(&e, &mut self.context)))?;
        Ok(result.to_boolean())
    }

    fn logs(&self) -> Vec<String> {
        self.log.borrow().lines.clone()
    }
}

/// A real, boa-backed script host.
///
/// The boa `Context` lives on a dedicated worker thread owned by this handle;
/// every operation is bounded by [`ScriptSandboxConfig::execution_timeout`]. See
/// the module + crate docs for the watchdog guarantee.
pub struct ScriptHost {
    /// The transpilable source + config, retained so a worker abandoned after a
    /// timeout can be respawned with identical state on the next call.
    source: String,
    config: ScriptSandboxConfig,
    /// `None` once a worker has been abandoned (timeout) — respawned lazily.
    worker: Option<Worker>,
}

/// The live channels + thread handle for one worker.
struct Worker {
    tx: Sender<Command>,
    rx: Receiver<Reply>,
    handle: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for ScriptHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScriptHost")
            .field("config", &self.config)
            .field("worker_live", &self.worker.is_some())
            .finish_non_exhaustive()
    }
}

impl ScriptHost {
    /// Transpile `ts_source` (swc) and load the resulting JS into a fresh boa
    /// context **on a dedicated worker thread**. The JS top level runs
    /// immediately on the worker, so `render`/`apply_action` definitions become
    /// available; a load error (oversize source / transpile / a throw at load)
    /// is reported synchronously here.
    ///
    /// # Errors
    ///
    /// - [`ScriptHostError::SourceTooLarge`] if `ts_source` exceeds the cap.
    /// - [`ScriptHostError::Transpile`] if swc fails to parse / strip types.
    /// - [`ScriptHostError::Runtime`] if the transpiled JS throws while loading.
    pub fn from_source(ts_source: &str, config: ScriptSandboxConfig) -> Result<Self> {
        if ts_source.len() > config.max_source_bytes {
            return Err(ScriptHostError::SourceTooLarge {
                got: ts_source.len(),
                cap: config.max_source_bytes,
            });
        }

        let source = ts_source.to_owned();
        // Spawn the worker and confirm it loaded (so load errors surface here,
        // exactly like the pre-watchdog synchronous constructor did). The load
        // itself is bounded by the same execution deadline.
        let worker = spawn_worker(&source, config)?;

        Ok(Self {
            source,
            config,
            worker: Some(worker),
        })
    }

    /// Construct with default sandbox limits.
    ///
    /// # Errors
    ///
    /// See [`ScriptHost::from_source`].
    pub fn from_source_default(ts_source: &str) -> Result<Self> {
        Self::from_source(ts_source, ScriptSandboxConfig::default())
    }

    /// The captured `console.log` lines, oldest first. Returns an empty vec if a
    /// worker has been abandoned and not yet respawned (no logs to read), or if
    /// fetching them times out.
    #[must_use]
    pub fn logs(&mut self) -> Vec<String> {
        let Some(worker) = self.worker.as_ref() else {
            return Vec::new();
        };
        if worker.tx.send(Command::Logs).is_err() {
            self.abandon_worker();
            return Vec::new();
        }
        match worker.rx.recv_timeout(self.config.execution_timeout) {
            Ok(Reply::Logs(lines)) => lines,
            Ok(_) => Vec::new(),
            Err(_) => {
                self.abandon_worker();
                Vec::new()
            }
        }
    }

    /// Ensure a live worker exists, respawning it from the retained source after
    /// a previous timeout abandoned the old one.
    fn ensure_worker(&mut self) -> Result<()> {
        if self.worker.is_none() {
            self.worker = Some(spawn_worker(&self.source, self.config)?);
        }
        Ok(())
    }

    /// Drop the live worker handle without joining (it may be stuck in a runaway
    /// script and never join). The OS reaps the thread when it finally yields.
    fn abandon_worker(&mut self) {
        if let Some(mut worker) = self.worker.take() {
            // Detach: do NOT join — the thread may be spinning in the runaway
            // script. Dropping the JoinHandle detaches it.
            let _ = worker.handle.take();
            // Dropping `tx`/`rx` closes the channels; if the thread ever yields
            // and tries to reply it gets a SendError and exits its loop.
        }
    }

    /// Send a command and wait for the reply under the wall-clock deadline.
    /// Maps a timeout to [`ScriptHostError::Timeout`] and abandons the worker.
    fn call(&mut self, cmd: Command) -> std::result::Result<Reply, ScriptHostError> {
        self.ensure_worker()?;
        let deadline = self.config.execution_timeout;
        let worker = self
            .worker
            .as_ref()
            .expect("ensure_worker guarantees a worker");

        if worker.tx.send(cmd).is_err() {
            // Worker died (panicked) between spawn and send — surface a runtime
            // error rather than hanging.
            self.abandon_worker();
            return Err(ScriptHostError::Runtime(
                "script worker thread is no longer running".into(),
            ));
        }

        let start = Instant::now();
        match worker.rx.recv_timeout(deadline) {
            Ok(reply) => Ok(reply),
            Err(RecvTimeoutError::Timeout) => {
                self.abandon_worker();
                Err(ScriptHostError::Timeout(start.elapsed().max(deadline)))
            }
            Err(RecvTimeoutError::Disconnected) => {
                // Worker thread ended without replying (panic). Not a hang.
                self.abandon_worker();
                Err(ScriptHostError::Runtime(
                    "script worker thread ended unexpectedly".into(),
                ))
            }
        }
    }
}

impl Drop for ScriptHost {
    fn drop(&mut self) {
        // Detach any live worker without joining (it could be a runaway).
        self.abandon_worker();
    }
}

impl ScriptHostApi for ScriptHost {
    fn render(&mut self) -> Result<AppWidgetModel> {
        match self.call(Command::Render)? {
            Reply::Model(r) => r,
            _ => Err(ScriptHostError::Runtime(
                "script worker returned an unexpected reply to render".into(),
            )),
        }
    }

    fn apply_action(&mut self, action: &AppWidgetAction) -> Result<bool> {
        match self.call(Command::ApplyAction(action.clone()))? {
            Reply::Changed(r) => r,
            _ => Err(ScriptHostError::Runtime(
                "script worker returned an unexpected reply to apply_action".into(),
            )),
        }
    }
}

/// Spawn a worker thread that loads the engine and then serves commands. Blocks
/// (under the execution deadline) until the engine reports its load result, so
/// constructor errors surface synchronously.
fn spawn_worker(source: &str, config: ScriptSandboxConfig) -> Result<Worker> {
    let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<Command>();
    let (reply_tx, reply_rx) = std::sync::mpsc::channel::<Reply>();
    // A one-shot channel for the load result (so load errors surface at
    // construction, bounded by the same deadline).
    let (load_tx, load_rx) = std::sync::mpsc::channel::<Result<()>>();

    let source_owned = source.to_owned();
    let handle = std::thread::Builder::new()
        .name("liquide-script-host".into())
        .spawn(move || {
            let mut engine = match Engine::load(&source_owned, config) {
                Ok(e) => {
                    // If the handle already gave up on load (timeout), bail.
                    if load_tx.send(Ok(())).is_err() {
                        return;
                    }
                    e
                }
                Err(e) => {
                    let _ = load_tx.send(Err(e));
                    return;
                }
            };

            // Serve commands until the handle drops the command sender (or a
            // reply send fails because the handle abandoned us).
            while let Ok(cmd) = cmd_rx.recv() {
                let reply = match cmd {
                    Command::Render => Reply::Model(engine.render()),
                    Command::ApplyAction(action) => Reply::Changed(engine.apply_action(&action)),
                    Command::Logs => Reply::Logs(engine.logs()),
                };
                if reply_tx.send(reply).is_err() {
                    // Handle abandoned us (timeout) — nobody is listening.
                    return;
                }
            }
        })
        .map_err(|e| ScriptHostError::Runtime(format!("could not spawn script worker: {e}")))?;

    // Wait for the load result under the execution deadline. A load that hangs
    // (e.g. an infinite loop at module top level) is bounded just like a render.
    match load_rx.recv_timeout(config.execution_timeout) {
        Ok(Ok(())) => Ok(Worker {
            tx: cmd_tx,
            rx: reply_rx,
            handle: Some(handle),
        }),
        Ok(Err(e)) => Err(e),
        Err(RecvTimeoutError::Timeout) => {
            // The module top level is hanging; abandon the thread (detach).
            drop(handle);
            Err(ScriptHostError::Timeout(config.execution_timeout))
        }
        Err(RecvTimeoutError::Disconnected) => Err(ScriptHostError::Runtime(
            "script worker thread ended during load".into(),
        )),
    }
}

/// Build a `{ widget, name, payload }` JS object for the action.
fn action_to_js(action: &AppWidgetAction, ctx: &mut Context) -> Result<JsValue> {
    let obj = JsObject::with_object_proto(ctx.intrinsics());
    let set = |k: &str, v: &str, ctx: &mut Context| {
        obj.set(js_string!(k), js_string!(v), false, ctx)
            .map_err(|e| ScriptHostError::Runtime(format_err(&e, ctx)))
    };
    set("widget", &action.widget, ctx)?;
    set("name", &action.name, ctx)?;
    set("payload", &action.payload, ctx)?;
    Ok(JsValue::from(obj))
}

/// Bind a bounded `console.log` (the ONLY host capability). It appends a joined
/// string of its arguments to the host log ring; it performs no real IO.
fn bind_console(ctx: &mut Context, log: Gc<GcRefCell<LogRing>>) -> Result<()> {
    let console = JsObject::with_object_proto(ctx.intrinsics());

    // `from_copy_closure_with_captures` is SAFE: the closure is `Copy` (captures
    // nothing itself) and the GC-traceable state is passed as the `captures`
    // argument, which boa traces correctly. This is why the crate keeps
    // `#![forbid(unsafe_code)]`.
    let log_fn = NativeFunction::from_copy_closure_with_captures(
        |_this, args, log: &Gc<GcRefCell<LogRing>>, ctx| {
            let mut parts = Vec::with_capacity(args.len());
            for a in args {
                let s = a
                    .to_string(ctx)
                    .map(|js| js.to_std_string_escaped())
                    .unwrap_or_else(|_| "<unprintable>".into());
                parts.push(s);
            }
            log.borrow_mut().push(parts.join(" "));
            Ok(JsValue::undefined())
        },
        log,
    );

    console
        .set(
            js_string!("log"),
            log_fn.to_js_function(ctx.realm()),
            false,
            ctx,
        )
        .map_err(|e| ScriptHostError::Runtime(format_err(&e, ctx)))?;

    ctx.register_global_property(js_string!("console"), console, Attribute::all())
        .map_err(|e| ScriptHostError::Runtime(format_err(&e, ctx)))?;
    Ok(())
}

/// Render a `JsError` to a readable string using the live context (so a thrown
/// `Error` object surfaces its `.message`).
fn format_err(e: &JsError, ctx: &mut Context) -> String {
    match e.to_opaque(ctx).to_string(ctx) {
        Ok(s) => s.to_std_string_escaped(),
        Err(_) => e.to_string(),
    }
}
