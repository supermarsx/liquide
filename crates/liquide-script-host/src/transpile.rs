//! TypeScript → JavaScript transpilation via swc, in-process.
//!
//! This is the first stage of the pipeline: parse the TS, **strip the types**,
//! and emit plain JS that boa can execute. There is no external binary and no
//! esbuild — swc is a pure-Rust toolchain linked directly.
//!
//! Parse errors and type-strip failures are collected as
//! [`TranspileDiagnostic`]s (with source location) and returned inside
//! [`ScriptHostError::Transpile`]; this function never panics on malformed
//! input.

use swc_common::sync::Lrc;
use swc_common::{FileName, SourceMap, Spanned, GLOBALS};
use swc_ecma_ast::{EsVersion, ModuleDecl, ModuleItem, Stmt};
use swc_ecma_codegen::text_writer::JsWriter;
use swc_ecma_codegen::Emitter as CodeEmitter;
use swc_ecma_parser::error::Error as ParseError;
use swc_ecma_parser::{lexer::Lexer, Parser, StringInput, Syntax, TsSyntax};
use swc_ecma_transforms_typescript::strip_type;
use swc_ecma_visit::VisitMutWith;

use crate::{Result, ScriptHostError, TranspileDiagnostic};

/// Transpile TypeScript `src` to JavaScript (types stripped).
///
/// # Errors
///
/// Returns [`ScriptHostError::Transpile`] (with located diagnostics) if the
/// source fails to parse or type-strip. Never panics on malformed input.
pub fn transpile_ts(src: &str) -> Result<String> {
    // swc's interned-string / span machinery requires a `GLOBALS` scope.
    GLOBALS.set(&Default::default(), || transpile_inner(src))
}

fn transpile_inner(src: &str) -> Result<String> {
    let cm: Lrc<SourceMap> = Default::default();
    let fm = cm.new_source_file(
        Lrc::new(FileName::Custom("app.ts".into())),
        src.to_string(),
    );

    let lexer = Lexer::new(
        Syntax::Typescript(TsSyntax {
            tsx: false,
            ..Default::default()
        }),
        EsVersion::Es2020,
        StringInput::from(&*fm),
        // No comment store: transpile output doesn't need comment preservation.
        None,
    );

    let mut parser = Parser::new_from(lexer);

    // Recoverable (non-fatal) lexer/parser diagnostics.
    let mut diags: Vec<TranspileDiagnostic> = parser
        .take_errors()
        .into_iter()
        .map(|e| diag_of(&cm, &e))
        .collect();

    let module = match parser.parse_module() {
        Ok(m) => m,
        Err(e) => {
            diags.push(diag_of(&cm, &e));
            return Err(ScriptHostError::Transpile(diags));
        }
    };

    // A recoverable error means the parse limped through but the source is
    // malformed; do not run a half-parsed module.
    if !diags.is_empty() {
        return Err(ScriptHostError::Transpile(diags));
    }

    // Strip the TypeScript types (interfaces, annotations, generics, type-only
    // imports/exports) → a plain JS AST. `strip_type` needs no resolver marks.
    let mut module = module;
    module.visit_mut_with(&mut strip_type());

    // Lower the module to a flat script body. boa runs the transpiled JS as a
    // *script*, not an ES module, so a top-level `export function render() {}`
    // must become a plain `function render() {}` for `render` to land on the
    // global object (and to avoid a `SyntaxError: unexpected token 'export'`).
    // We unwrap `export <decl>` to its inner declaration and drop every other
    // module-level decl (imports, re-exports, default exports) — the authoring
    // contract is "define top-level `render`/`apply_action`", which this makes
    // reachable whether or not the author wrote `export`.
    module.body = module
        .body
        .into_iter()
        .filter_map(|item| match item {
            ModuleItem::Stmt(stmt) => Some(ModuleItem::Stmt(stmt)),
            ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(export)) => {
                Some(ModuleItem::Stmt(Stmt::Decl(export.decl)))
            }
            // Drop imports, named/default/all re-exports, and TS-only module
            // decls: a sandboxed script has no module loader.
            ModuleItem::ModuleDecl(_) => None,
        })
        .collect();

    // Emit JS.
    let mut buf = Vec::new();
    {
        let writer = JsWriter::new(cm.clone(), "\n", &mut buf, None);
        let mut emitter = CodeEmitter {
            cfg: swc_ecma_codegen::Config::default(),
            cm: cm.clone(),
            comments: None,
            wr: writer,
        };
        if emitter.emit_module(&module).is_err() {
            return Err(ScriptHostError::Transpile(vec![TranspileDiagnostic {
                message: "code generation failed".into(),
                line: None,
                column: None,
            }]));
        }
    }

    String::from_utf8(buf).map_err(|e| {
        ScriptHostError::Transpile(vec![TranspileDiagnostic {
            message: format!("generated non-UTF-8 output: {e}"),
            line: None,
            column: None,
        }])
    })
}

/// Convert an swc parser error into a located [`TranspileDiagnostic`].
fn diag_of(cm: &SourceMap, e: &ParseError) -> TranspileDiagnostic {
    let span = e.span();
    let message = e.kind().msg().to_string();
    // A dummy span (lo == hi == 0) carries no useful location.
    if span.lo.0 == 0 && span.hi.0 == 0 {
        return TranspileDiagnostic {
            message,
            line: None,
            column: None,
        };
    }
    let loc = cm.lookup_char_pos(span.lo);
    TranspileDiagnostic {
        message,
        line: Some(loc.line),
        column: Some(loc.col_display + 1),
    }
}
