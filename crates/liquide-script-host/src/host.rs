//! The boa-backed script host (feature `script`).
//!
//! Owns a single boa [`Context`], the transpiled JS, and the captured log ring.
//! It is created once from TS source ([`ScriptHost::from_source`]) and then
//! driven via [`ScriptHostApi`]: [`render`](ScriptHost::render) calls the
//! script's `render()` and decodes the returned object into an
//! [`AppWidgetModel`]; [`apply_action`](ScriptHost::apply_action) feeds an
//! [`AppWidgetAction`] to the script's optional `apply_action(action)`.
//!
//! See the crate docs for the full sandbox posture and boa's interruption
//! caveat. The only host capability bound into the context is a bounded
//! `console.log`; everything else (`fetch`, `require`, `fs`, …) is absent, so a
//! script that touches one throws a `ReferenceError` we surface cleanly.

use boa_engine::gc::{Finalize, Gc, GcRefCell, Trace};
use boa_engine::object::JsObject;
use boa_engine::property::Attribute;
use boa_engine::{js_string, Context, JsError, JsValue, NativeFunction, Source};

use crate::{
    AppWidgetAction, AppWidgetModel, Result, ScriptHostApi, ScriptHostError, ScriptSandboxConfig,
    APPLY_ACTION_FN, RENDER_FN,
};

/// A bounded log ring shared (via boa's GC) between the host and the bound
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

/// A real, boa-backed script host.
pub struct ScriptHost {
    context: Context,
    config: ScriptSandboxConfig,
    log: Gc<GcRefCell<LogRing>>,
}

impl std::fmt::Debug for ScriptHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScriptHost")
            .field("config", &self.config)
            .field("log_lines", &self.log.borrow().lines.len())
            .finish_non_exhaustive()
    }
}

impl ScriptHost {
    /// Transpile `ts_source` (swc) and load the resulting JS into a fresh boa
    /// context (the JS top level runs immediately, so `render`/`apply_action`
    /// definitions become available).
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

        let js = crate::transpile_ts(ts_source)?;

        let mut context = Context::default();
        let log = Gc::new(GcRefCell::new(LogRing {
            lines: Vec::new(),
            max_lines: config.max_log_lines,
            max_bytes: config.max_log_bytes,
        }));

        bind_console(&mut context, log.clone())?;

        // Run the (transpiled, export-lowered) script top level. The transpile
        // step has already unwrapped `export function render()` to a plain
        // top-level `function render()`, so running this as a boa *script* puts
        // `render`/`apply_action` on the global object. A throw here is a clean
        // runtime error, not a host crash.
        context
            .eval(Source::from_bytes(js.as_bytes()))
            .map_err(|e| ScriptHostError::Runtime(format_err(&e, &mut context)))?;

        Ok(Self {
            context,
            config,
            log,
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

    /// The captured `console.log` lines, oldest first.
    #[must_use]
    pub fn logs(&self) -> Vec<String> {
        self.log.borrow().lines.clone()
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
        let json = json.ok_or_else(|| {
            ScriptHostError::BadModel("render()'s value produced no JSON".into())
        })?;
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
}

impl ScriptHostApi for ScriptHost {
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
