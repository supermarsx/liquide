//! Template registry — loads, caches, and serves HTML templates with a
//! partial/include system.
//!
//! Templates use a lightweight mustache-like syntax:
//!
//! - `{{variable}}` — interpolate a context variable
//! - `{{> partial_name}}` — include another template by name
//! - `{{#if var}}...{{/if}}` — conditional block (renders if var is truthy)
//! - `{{#each var}}...{{/each}}` — loop block (renders body for each item in a list)
//!
//! The registry holds named templates and resolves includes at render time.
//! Circular includes are detected and produce an empty string rather than
//! infinite recursion.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::document::Document;
use crate::node::NodeId;

// ---------------------------------------------------------------------------
// HTML escaping
// ---------------------------------------------------------------------------

/// HTML-escape a dynamic, untrusted string for safe embedding in element text
/// or double/single-quoted attribute values.
///
/// Escapes the five characters that are significant in HTML markup:
/// `&`, `<`, `>`, `"`, and `'`. `&` is replaced first (it is implicit in the
/// per-character match, since each source character maps to a single output
/// entity and entities are never re-scanned), preventing double-escaping of an
/// already-escaped entity such as `&amp;` into `&amp;amp;`.
///
/// This is the single shared escaping helper for the shell DOM pipeline: the
/// template registry uses it for every variable interpolation, and callers that
/// build raw attribute HTML by hand (e.g. `liquide-shell`'s `dom_sync`) must
/// route every untrusted substring through it. Only the *dynamic* substituted
/// values are escaped — structural template markup (tag names, attribute names,
/// quotes, `{{#if}}`/`{{#each}}` control tags) is emitted verbatim and never
/// passed here.
pub fn escape_html(s: &str) -> String {
    // Fast path: nothing to escape.
    if !s
        .bytes()
        .any(|b| matches!(b, b'&' | b'<' | b'>' | b'"' | b'\''))
    {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len() + 16);
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// TemplateContext — the data passed into template rendering
// ---------------------------------------------------------------------------

/// A value that can be substituted into a template.
#[derive(Debug, Clone)]
pub enum TemplateValue {
    /// A simple string value. Interpolated values of this kind are
    /// **HTML-escaped** on output (see [`escape_html`]) — this is the default
    /// and correct choice for any untrusted/dynamic text.
    String(String),
    /// Pre-built, trusted HTML that must be substituted **verbatim** (not
    /// escaped). Used only for HTML fragments the caller has already assembled
    /// and escaped itself — e.g. the shell's per-slot status-bar markup, which
    /// the flat template engine cannot express with `{{#each}}`. The producer
    /// is responsible for escaping every dynamic value embedded inside this
    /// fragment; never store untrusted text here unescaped.
    RawHtml(String),
    /// A boolean flag (for `{{#if}}`).
    Bool(bool),
    /// A list of sub-contexts (for `{{#each}}`).
    List(Vec<TemplateContext>),
}

impl From<&str> for TemplateValue {
    fn from(s: &str) -> Self {
        TemplateValue::String(s.to_string())
    }
}

impl From<String> for TemplateValue {
    fn from(s: String) -> Self {
        TemplateValue::String(s)
    }
}

impl From<&String> for TemplateValue {
    fn from(s: &String) -> Self {
        TemplateValue::String(s.clone())
    }
}

impl From<bool> for TemplateValue {
    fn from(b: bool) -> Self {
        TemplateValue::Bool(b)
    }
}

impl From<Vec<TemplateContext>> for TemplateValue {
    fn from(v: Vec<TemplateContext>) -> Self {
        TemplateValue::List(v)
    }
}

/// Context for template rendering — a map of variable names to values.
#[derive(Debug, Clone, Default)]
pub struct TemplateContext {
    vars: HashMap<String, TemplateValue>,
}

impl TemplateContext {
    /// Create an empty context.
    pub fn new() -> Self {
        Self {
            vars: HashMap::new(),
        }
    }

    /// Set a variable.
    pub fn set(&mut self, key: &str, value: impl Into<TemplateValue>) {
        self.vars.insert(key.to_string(), value.into());
    }

    /// Set a variable holding **pre-built, trusted HTML** that must be
    /// interpolated verbatim (not HTML-escaped).
    ///
    /// Use this only for HTML fragments the caller has already assembled and
    /// escaped itself (e.g. a slot of status-bar items built in Rust). For any
    /// untrusted/dynamic text use [`set`](Self::set) instead, which escapes the
    /// value on output.
    pub fn set_raw_html(&mut self, key: &str, html: impl Into<String>) {
        self.vars
            .insert(key.to_string(), TemplateValue::RawHtml(html.into()));
    }

    /// Render a single `{{tag}}` interpolation: `RawHtml` is emitted verbatim,
    /// every other string value is HTML-escaped. Missing/non-string values
    /// produce the empty string.
    fn render_interpolation(&self, key: &str, out: &mut String) {
        match self.vars.get(key) {
            Some(TemplateValue::RawHtml(s)) => out.push_str(s),
            Some(TemplateValue::String(s)) => out.push_str(&escape_html(s)),
            _ => {}
        }
    }

    /// Get a variable value.
    pub fn get(&self, key: &str) -> Option<&TemplateValue> {
        self.vars.get(key)
    }

    /// Get a string variable, returning empty string if missing or non-string.
    pub fn get_str(&self, key: &str) -> &str {
        match self.vars.get(key) {
            Some(TemplateValue::String(s)) | Some(TemplateValue::RawHtml(s)) => s.as_str(),
            _ => "",
        }
    }

    /// Check if a variable is truthy (exists and is not empty/false).
    pub fn is_truthy(&self, key: &str) -> bool {
        match self.vars.get(key) {
            None => false,
            Some(TemplateValue::String(s)) | Some(TemplateValue::RawHtml(s)) => !s.is_empty(),
            Some(TemplateValue::Bool(b)) => *b,
            Some(TemplateValue::List(v)) => !v.is_empty(),
        }
    }

    /// Get a list variable for iteration.
    pub fn get_list(&self, key: &str) -> Option<&[TemplateContext]> {
        match self.vars.get(key) {
            Some(TemplateValue::List(v)) => Some(v.as_slice()),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Default embedded templates
// ---------------------------------------------------------------------------

const DEFAULT_DESKTOP: &str = r#"<desktop-background id="desktop-bg" />
<statusbar id="shell-statusbar">
  <statusbar-slot class="left" id="statusbar-slot-left">
    {{#each left_items}}{{> statusbar-item}}{{/each}}
  </statusbar-slot>
  <statusbar-slot class="center" id="statusbar-slot-center">
    {{#each center_items}}{{> statusbar-item}}{{/each}}
  </statusbar-slot>
  <statusbar-slot class="right" id="statusbar-slot-right">
    {{#each right_items}}{{> statusbar-item}}{{/each}}
  </statusbar-slot>
</statusbar>
<workspace-container id="workspace-container" />
<dock id="shell-dock">
  {{#each dock_items}}{{> dock-item}}{{/each}}
</dock>
<notification-area id="notification-area" />
"#;

const DEFAULT_WINDOW: &str = r#"<window id="{{id}}" {{#if focused}}class="focused"{{/if}}>
  <window-titlebar>
    <window-title>{{title}}</window-title>
    <titlebar-buttons>
      <minimize-button />
      <maximize-button />
      <close-button />
    </titlebar-buttons>
  </window-titlebar>
  <window-content />
</window>
"#;

const DEFAULT_NOTIFICATION: &str = r#"<notification id="{{id}}" data-key="{{id}}" data-state-hash="{{state_hash}}">
  <notification-title>{{title}}</notification-title>
  <notification-body>{{body}}</notification-body>
</notification>
"#;

const DEFAULT_CONTEXT_MENU: &str = r#"<context-menu id="{{id}}" style="left: {{pos_left}}; top: {{pos_top}}">
  {{#each items}}{{> menu-item}}{{/each}}
</context-menu>
"#;

const DEFAULT_LAUNCHER: &str = r#"<launcher-overlay id="launcher-overlay" data-state-hash="{{state_hash}}">
  <launcher id="shell-launcher">
    <launcher-search id="launcher-search" />
    <launcher-results>
      {{#each items}}
            <launcher-item data-key="{{key}}" data-app-id="{{app_id}}" data-icon="{{icon}}">{{label}}</launcher-item>
      {{/each}}
    </launcher-results>
  </launcher>
</launcher-overlay>
"#;

const DEFAULT_SESSION_MENU: &str = r#"<session-menu id="{{id}}" style="left: {{pos_left}}; top: {{pos_top}}">
  {{#each items}}{{> menu-item}}{{/each}}
</session-menu>
"#;

const DEFAULT_APP_MENU: &str = r#"<app-menu id="{{id}}" style="left: {{pos_left}}; top: {{pos_top}}">
  {{#each items}}{{> menu-item}}{{/each}}
</app-menu>
"#;

const DEFAULT_DOCK_ITEM: &str = r#"<dock-item data-app-id="{{app_id}}" data-label="{{label}}" {{#if is_active}}class="active"{{/if}}>{{label}}</dock-item>"#;

// The icon and label are emitted as SEPARATE child elements so the menu CSS
// can lay them out side-by-side: `menu-item` is `display:flex`, `menu-item-icon`
// occupies a fixed 16px gutter with an 8px right margin, and `menu-item-label`
// (`flex-grow:1`) takes the remaining width to the right of the icon. Carrying
// the icon glyph only as a `data-icon` ATTRIBUTE on `<menu-item>` (as this
// partial previously did) left the CSS's `menu-item-icon`/`menu-item-label`
// child rules with no elements to target, so the icon glyph and the label text
// painted in the same columns (the icon overprinted the label). The icon
// element is only emitted when an icon is present, so icon-less menus do not get
// an empty gutter shifting their labels. `{{label}}` is interpolated as a
// `String` value and is therefore HTML-escaped on output.
const DEFAULT_MENU_ITEM: &str = r#"<menu-item data-action="{{action}}">{{#if icon}}<menu-item-icon data-icon="{{icon}}" />{{/if}}<menu-item-label>{{label}}</menu-item-label></menu-item>"#;

const DEFAULT_TOOLTIP: &str = r#"<tooltip id="{{id}}" data-position="{{position}}" style="left: {{pos_left}}; top: {{pos_top}}">
  <tooltip-arrow />
  <tooltip-content>{{text}}</tooltip-content>
</tooltip>
"#;

const DEFAULT_STATUSBAR_ITEM: &str = r#"{{#if type_logo}}<statusbar-logo id="{{id}}">{{text}}</statusbar-logo>{{else}}{{#if type_notification}}<notification-indicator id="{{id}}" {{#if class}}class="{{class}}"{{/if}}>{{text}}</notification-indicator>{{else}}{{#if type_status}}<status-indicator id="{{id}}" {{#if class}}class="{{class}}"{{/if}}>{{text}}</status-indicator>{{else}}{{#if type_tray}}<status-tray id="{{id}}" />{{else}}{{#if type_session}}<session-button id="{{id}}">{{text}}</session-button>{{else}}<statusbar-item id="{{id}}" {{#if class}}class="{{class}}"{{/if}}>{{text}}</statusbar-item>{{/if}}{{/if}}{{/if}}{{/if}}{{/if}}"#;

/// Embedded "statusbar" template — renders CHILDREN of the `<statusbar>` element.
/// The template context provides `left_items`, `center_items`, `right_items` lists.
/// Each item has `id`, `text`, `classes`, and type flags (`type_notification`, etc.).
const DEFAULT_STATUSBAR: &str = r#"<statusbar-slot class="left" id="statusbar-slot-left">
  <statusbar-logo id="logo">LiquiDE</statusbar-logo>
  {{#each left_items}}
  <statusbar-item id="{{id}}" class="{{classes}}">{{text}}</statusbar-item>
  {{/each}}
</statusbar-slot>
<statusbar-slot class="center" id="statusbar-slot-center">
  {{#each center_items}}
  <statusbar-item id="{{id}}" class="{{classes}}">{{text}}</statusbar-item>
  {{/each}}
</statusbar-slot>
<statusbar-slot class="right" id="statusbar-slot-right">
  {{#each right_items}}
  {{#if type_notification}}
  <notification-indicator id="{{id}}" class="{{classes}}">{{text}}</notification-indicator>
  {{else}}
  {{#if type_status}}
  <status-indicator id="{{id}}" class="{{classes}}">{{text}}</status-indicator>
  {{else}}
  {{#if type_tray}}
  <status-tray id="{{id}}" />
  {{else}}
  {{#if type_session}}
  <session-button id="{{id}}">{{text}}</session-button>
  {{else}}
  <statusbar-item id="{{id}}" class="{{classes}}">{{text}}</statusbar-item>
  {{/if}}
  {{/if}}
  {{/if}}
  {{/if}}
  {{/each}}
</statusbar-slot>"#;

/// Embedded "dock" template — renders CHILDREN of the `<dock>` element.
/// The template context provides `dock_items` list with `app_id`, `label`,
/// `icon`, `index`, `is_running`, `is_pinned`, `is_hovered`.
const DEFAULT_DOCK: &str = r#"{{#each dock_items}}
<dock-item data-app-id="{{app_id}}" data-icon="{{icon}}" data-label="{{label}}" data-index="{{index}}" {{#if is_running}}class="active"{{/if}}>
  <dock-item-icon data-icon="{{icon}}" />
  <dock-item-label>{{label}}</dock-item-label>
  {{#if is_running}}
  <dock-indicator class="running" />
  {{/if}}
</dock-item>
{{/each}}"#;

// ---------------------------------------------------------------------------
// Depth-tracking block matching
// ---------------------------------------------------------------------------
//
// The mustache-like block syntax (`{{#if}}`/`{{/if}}`, `{{#each}}`/`{{/each}}`)
// can nest arbitrarily. A naive first-match `body.find("{{/each}}")` matches the
// FIRST close tag in the remaining input, which — for two same-type nested blocks
// — is the INNER block's close, silently truncating the outer block's body. These
// helpers track nesting depth so a block is matched to its true partner.
//
// `body` is the input *after* an open `{{#TAG ...}}` tag, so the opener is already
// consumed and depth conceptually starts at 1 (the block we are inside). We scan
// forward, incrementing on each same-type opener and decrementing on each
// same-type closer, returning when depth returns to 0.

/// Find the byte offset, within `body`, of the `{{` that begins the `{{/tag}}`
/// closing the block whose opener was just consumed.
///
/// `body` must be the slice *following* the open `{{#tag ...}}` tag. Returns
/// `None` if no matching close tag exists (malformed template), letting the
/// caller skip the block rather than emit truncated output.
///
/// Only same-type nesting is tracked: a `{{/each}}` cannot legally appear inside
/// a balanced `{{#if}}...{{/if}}` without its own `{{#each}}` also being inside,
/// so for well-formed templates tracking the matching tag type alone is correct
/// (and matches the proven `template.rs::find_block_end` behaviour).
fn find_block_end(body: &str, tag: &str) -> Option<usize> {
    let open_prefix = format!("{{{{#{tag} ");
    let open_no_arg = format!("{{{{#{tag}}}}}"); // e.g. `{{#if}}` with no argument
    let close = format!("{{{{/{tag}}}}}");

    let mut depth = 1usize;
    let mut pos = 0usize;
    while let Some(rel) = body[pos..].find("{{") {
        let at = pos + rel;
        let here = &body[at..];
        if here.starts_with(&close) {
            depth -= 1;
            if depth == 0 {
                return Some(at);
            }
            pos = at + close.len();
        } else if here.starts_with(&open_prefix) || here.starts_with(&open_no_arg) {
            depth += 1;
            pos = at + 2;
        } else {
            pos = at + 2;
        }
    }
    None
}

/// Split a block body on its top-level `{{else}}`.
///
/// `block` is the body *between* an open `{{#tag ...}}` and its matching
/// `{{/tag}}` (as returned by [`find_block_end`]). Returns
/// `(then_part, Some(else_part))` when a depth-0 `{{else}}` is present, otherwise
/// `(block, None)`. Nesting of the same tag type is tracked so an `{{else}}`
/// belonging to a nested block of the same kind is not mistaken for this block's.
fn split_else<'a>(block: &'a str, tag: &str) -> (&'a str, Option<&'a str>) {
    let else_tag = "{{else}}";
    let open_prefix = format!("{{{{#{tag} ");
    let open_no_arg = format!("{{{{#{tag}}}}}");
    let close = format!("{{{{/{tag}}}}}");

    let mut depth = 0usize;
    let mut pos = 0usize;
    while let Some(rel) = block[pos..].find("{{") {
        let at = pos + rel;
        let here = &block[at..];
        if here.starts_with(&open_prefix) || here.starts_with(&open_no_arg) {
            depth += 1;
            pos = at + 2;
        } else if here.starts_with(&close) {
            // A nested close at depth 0 should not occur (the block is balanced),
            // but guard against underflow defensively.
            depth = depth.saturating_sub(1);
            pos = at + close.len();
        } else if depth == 0 && here.starts_with(else_tag) {
            return (&block[..at], Some(&block[at + else_tag.len()..]));
        } else {
            pos = at + 2;
        }
    }
    (block, None)
}

// ---------------------------------------------------------------------------
// TemplateRegistry
// ---------------------------------------------------------------------------

/// Central registry that loads, caches, and serves HTML templates.
///
/// Templates are stored by name (e.g. `"window"`, `"dock-item"`,
/// `"components/header"`). At render time, `{{> name}}` directives cause
/// other templates to be looked up and inlined.
///
/// The registry is `Send + Sync` because all state is behind `&mut self`
/// borrows — no interior mutability needed.
pub struct TemplateRegistry {
    /// Named templates: `"window"` -> template source string.
    templates: HashMap<String, String>,
    /// Search paths for template files (checked in order).
    search_paths: Vec<PathBuf>,
}

impl TemplateRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            templates: HashMap::new(),
            search_paths: Vec::new(),
        }
    }

    /// Add a directory to search for `.html` template files.
    pub fn add_search_path(&mut self, path: impl Into<PathBuf>) {
        self.search_paths.push(path.into());
    }

    /// Register a template from a string.
    ///
    /// If a template with the same name already exists it is replaced.
    pub fn register(&mut self, name: &str, source: &str) {
        self.templates.insert(name.to_string(), source.to_string());
    }

    /// Register all built-in default templates (embedded in the binary).
    ///
    /// These serve as fallbacks — disk-loaded or manually registered templates
    /// with the same name will override them when `load_from_disk` runs
    /// afterwards.
    pub fn register_defaults(&mut self) {
        self.register("desktop", DEFAULT_DESKTOP);
        self.register("window", DEFAULT_WINDOW);
        self.register("notification", DEFAULT_NOTIFICATION);
        self.register("context-menu", DEFAULT_CONTEXT_MENU);
        self.register("launcher", DEFAULT_LAUNCHER);
        self.register("session-menu", DEFAULT_SESSION_MENU);
        self.register("app-menu", DEFAULT_APP_MENU);
        self.register("statusbar", DEFAULT_STATUSBAR);
        self.register("dock", DEFAULT_DOCK);
        self.register("dock-item", DEFAULT_DOCK_ITEM);
        self.register("menu-item", DEFAULT_MENU_ITEM);
        self.register("statusbar-item", DEFAULT_STATUSBAR_ITEM);
        self.register("tooltip", DEFAULT_TOOLTIP);
    }

    /// Load all `.html` files from search paths.
    ///
    /// Files at `path/foo.html` become template `"foo"`.
    /// Files at `path/components/bar.html` become template `"components/bar"`.
    ///
    /// Later search paths override earlier ones (and override embedded
    /// defaults).  Returns the number of templates loaded from disk.
    pub fn load_from_disk(&mut self) -> usize {
        let mut count = 0usize;
        // Clone search paths to avoid borrowing issues.
        let paths = self.search_paths.clone();
        for base in &paths {
            if !base.is_dir() {
                continue;
            }
            count += self.load_dir_recursive(base, base);
        }
        count
    }

    /// Recursively scan a directory for `.html` files.
    fn load_dir_recursive(&mut self, base: &Path, dir: &Path) -> usize {
        let mut count = 0usize;
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return 0,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                count += self.load_dir_recursive(base, &path);
            } else if path.extension().is_some_and(|ext| ext == "html") {
                if let Some(name) = self.template_name_from_path(base, &path) {
                    if let Ok(source) = std::fs::read_to_string(&path) {
                        self.templates.insert(name, source);
                        count += 1;
                    }
                }
            }
        }
        count
    }

    /// Derive a template name from a file path relative to a base directory.
    ///
    /// `base/foo.html` → `"foo"`, `base/components/bar.html` → `"components/bar"`.
    fn template_name_from_path(&self, base: &Path, path: &Path) -> Option<String> {
        let rel = path.strip_prefix(base).ok()?;
        let stem = rel.with_extension("");
        // Normalise path separators to forward slashes.
        Some(stem.to_string_lossy().replace('\\', "/").to_string())
    }

    /// Get a template source by name.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.templates.get(name).map(String::as_str)
    }

    /// List all registered template names (sorted for determinism).
    pub fn list(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.templates.keys().map(String::as_str).collect();
        names.sort_unstable();
        names
    }

    /// Render a named template with a context, returning the resulting HTML
    /// string.
    ///
    /// Supports:
    /// - `{{variable}}` — value substitution
    /// - `{{> partial_name}}` — include another template
    /// - `{{#if var}}...{{/if}}` — conditional
    /// - `{{#each var}}...{{/each}}` — iteration
    ///
    /// Returns `None` if the named template does not exist.
    pub fn render(&self, name: &str, ctx: &TemplateContext) -> Option<String> {
        let source = self.templates.get(name)?;
        let mut visited = HashSet::new();
        visited.insert(name.to_string());
        Some(self.render_source(source, ctx, &mut visited))
    }

    /// Render a named template directly into a DOM tree.
    ///
    /// The rendered HTML is parsed into DOM nodes and appended as children
    /// of `parent`.  Returns `true` if the template was found and rendered.
    pub fn render_into(
        &self,
        name: &str,
        doc: &mut Document,
        parent: NodeId,
        ctx: &TemplateContext,
    ) -> bool {
        let html = match self.render(name, ctx) {
            Some(h) => h,
            None => return false,
        };
        parse_html_into(doc, parent, &html);
        true
    }

    // -----------------------------------------------------------------------
    // Internal rendering engine
    // -----------------------------------------------------------------------

    /// Render a template source string with the given context.
    ///
    /// `visited` tracks which template names are currently on the include
    /// stack to prevent infinite circular includes.
    fn render_source(
        &self,
        source: &str,
        ctx: &TemplateContext,
        visited: &mut HashSet<String>,
    ) -> String {
        let mut out = String::with_capacity(source.len());
        let mut rest = source;

        while let Some(open) = rest.find("{{") {
            // Emit everything before the tag.
            out.push_str(&rest[..open]);
            rest = &rest[open + 2..];

            // Find closing braces.
            let close = match rest.find("}}") {
                Some(c) => c,
                None => {
                    // Malformed — emit the opening braces literally.
                    out.push_str("{{");
                    continue;
                }
            };

            let tag = rest[..close].trim();
            rest = &rest[close + 2..];

            if let Some(partial_name) = tag.strip_prefix("> ").or_else(|| tag.strip_prefix('>')) {
                // ---------- partial include ----------
                let partial_name = partial_name.trim();
                if visited.contains(partial_name) {
                    // Circular include — break the cycle silently.
                    continue;
                }
                if let Some(partial_source) = self.templates.get(partial_name) {
                    visited.insert(partial_name.to_string());
                    out.push_str(&self.render_source(partial_source, ctx, visited));
                    visited.remove(partial_name);
                }
                // Missing partial → empty string (graceful degradation).
            } else if let Some(block_var) = tag.strip_prefix("#if ") {
                // ---------- conditional block ----------
                //
                // `rest` is the input *after* the `{{#if X}}` open tag. Find the
                // matching `{{/if}}` at the SAME nesting depth (so an inner
                // `{{#if}}...{{/if}}` does not falsely close this block), then
                // recurse into the selected branch.
                let block_var = block_var.trim();
                let end_tag = "{{/if}}";
                if let Some(end_pos) = find_block_end(rest, "if") {
                    let block_body = &rest[..end_pos];
                    rest = &rest[end_pos + end_tag.len()..];
                    // Split on the top-level {{else}} (depth-aware so an `{{else}}`
                    // belonging to a nested `{{#if}}` is not mistaken for ours).
                    let (if_body, else_body) = split_else(block_body, "if");
                    if ctx.is_truthy(block_var) {
                        out.push_str(&self.render_source(if_body, ctx, visited));
                    } else if let Some(else_body) = else_body {
                        out.push_str(&self.render_source(else_body, ctx, visited));
                    }
                }
                // Malformed (no closing tag) — silently skip.
            } else if let Some(list_var) = tag.strip_prefix("#each ") {
                // ---------- iteration block ----------
                //
                // `rest` is the input *after* the `{{#each X}}` open tag. Find the
                // matching `{{/each}}` at the SAME nesting depth so a nested
                // `{{#each}}...{{/each}}` (or an inner `{{/each}}` belonging to a
                // deeper loop) does not truncate this block's body.
                let list_var = list_var.trim();
                let end_tag = "{{/each}}";
                if let Some(end_pos) = find_block_end(rest, "each") {
                    let body = &rest[..end_pos];
                    rest = &rest[end_pos + end_tag.len()..];
                    if let Some(items) = ctx.get_list(list_var) {
                        for item_ctx in items {
                            out.push_str(&self.render_source(body, item_ctx, visited));
                        }
                    }
                }
            } else if tag.starts_with('/') {
                // Stray closing tag ({{/if}} or {{/each}} outside a block).
                // Already consumed by the block opener — ignore.
            } else {
                // ---------- variable interpolation ----------
                // `String` values are HTML-escaped so that untrusted content
                // (notification bodies, window titles, tray tooltips, …) cannot
                // break out of an attribute or inject elements into the shell
                // chrome DOM. `RawHtml` values — pre-built, already-escaped
                // markup the caller assembled itself — are emitted verbatim.
                ctx.render_interpolation(tag, &mut out);
            }
        }

        // Emit trailing text after the last tag.
        out.push_str(rest);
        out
    }
}

impl Default for TemplateRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Minimal HTML-into-DOM parser
// ---------------------------------------------------------------------------

/// Parse a snippet of HTML and append the resulting nodes as children of
/// `parent` in `doc`.
///
/// This is a lightweight parser suitable for the template output produced by
/// the registry.  It handles:
///
/// - Element tags with attributes: `<foo bar="baz">`
/// - Self-closing tags: `<img />`
/// - Text nodes
/// - Nested elements
///
/// It does **not** handle: comments, doctypes, CDATA, entities, or
/// error recovery.  That's fine — templates are authored by us.
fn parse_html_into(doc: &mut Document, parent: NodeId, html: &str) {
    let mut parser = HtmlChunkParser::new(html);
    parse_children(doc, parent, &mut parser);
}

/// A tiny pull-parser that yields open tags, close tags, self-closing tags,
/// and text chunks.
struct HtmlChunkParser<'a> {
    rest: &'a str,
}

#[derive(Debug)]
enum HtmlChunk<'a> {
    /// `<tag attr="val" ...>` — opening tag.
    OpenTag {
        tag: &'a str,
        attrs: Vec<(&'a str, &'a str)>,
    },
    /// `</tag>` — closing tag.
    CloseTag { _tag: &'a str },
    /// `<tag attr="val" ... />` — self-closing.
    SelfClosing {
        tag: &'a str,
        attrs: Vec<(&'a str, &'a str)>,
    },
    /// Raw text between tags.
    Text(&'a str),
}

impl<'a> HtmlChunkParser<'a> {
    fn new(input: &'a str) -> Self {
        Self { rest: input }
    }

    fn next(&mut self) -> Option<HtmlChunk<'a>> {
        if self.rest.is_empty() {
            return None;
        }

        // If we're not at a '<', consume text.
        if !self.rest.starts_with('<') {
            let end = self.rest.find('<').unwrap_or(self.rest.len());
            let text = &self.rest[..end];
            self.rest = &self.rest[end..];
            // Skip pure-whitespace text nodes only if entirely whitespace.
            if text.trim().is_empty() {
                // Still return it — the caller can decide.
                return Some(HtmlChunk::Text(text));
            }
            return Some(HtmlChunk::Text(text));
        }

        // Skip comments <!-- ... -->
        if self.rest.starts_with("<!--") {
            if let Some(end) = self.rest.find("-->") {
                self.rest = &self.rest[end + 3..];
                return self.next();
            }
        }

        // Find the end of this tag.
        let tag_end = self.rest.find('>')?;
        let tag_content = &self.rest[1..tag_end]; // between '<' and '>'
        self.rest = &self.rest[tag_end + 1..];

        // Closing tag?
        if let Some(name) = tag_content.strip_prefix('/') {
            return Some(HtmlChunk::CloseTag { _tag: name.trim() });
        }

        // Self-closing?
        let self_closing = tag_content.ends_with('/');
        let tag_content = if self_closing {
            &tag_content[..tag_content.len() - 1]
        } else {
            tag_content
        };

        // Parse tag name and attributes.
        let tag_content = tag_content.trim();
        let (tag_name, attr_str) = match tag_content.find(|c: char| c.is_whitespace()) {
            Some(pos) => (&tag_content[..pos], tag_content[pos..].trim()),
            None => (tag_content, ""),
        };

        let attrs = parse_attrs(attr_str);

        if self_closing {
            Some(HtmlChunk::SelfClosing {
                tag: tag_name,
                attrs,
            })
        } else {
            Some(HtmlChunk::OpenTag {
                tag: tag_name,
                attrs,
            })
        }
    }
}

/// Parse HTML attributes from a string like `id="foo" class="bar"`.
fn parse_attrs(s: &str) -> Vec<(&str, &str)> {
    let mut attrs = Vec::new();
    let mut rest = s.trim();

    while !rest.is_empty() {
        // Skip whitespace.
        rest = rest.trim_start();
        if rest.is_empty() {
            break;
        }

        // Find '=' for key=value.
        let eq_pos = match rest.find('=') {
            Some(p) => p,
            None => {
                // Boolean attribute (no value) — take the rest as the key.
                attrs.push((rest, ""));
                break;
            }
        };

        let key = rest[..eq_pos].trim();
        rest = rest[eq_pos + 1..].trim_start();

        // Value may be quoted or unquoted.
        if rest.starts_with('"') {
            rest = &rest[1..];
            let end = rest.find('"').unwrap_or(rest.len());
            let val = &rest[..end];
            rest = if end < rest.len() {
                &rest[end + 1..]
            } else {
                ""
            };
            attrs.push((key, val));
        } else if rest.starts_with('\'') {
            rest = &rest[1..];
            let end = rest.find('\'').unwrap_or(rest.len());
            let val = &rest[..end];
            rest = if end < rest.len() {
                &rest[end + 1..]
            } else {
                ""
            };
            attrs.push((key, val));
        } else {
            // Unquoted — up to next whitespace.
            let end = rest.find(|c: char| c.is_whitespace()).unwrap_or(rest.len());
            let val = &rest[..end];
            rest = &rest[end..];
            attrs.push((key, val));
        }
    }

    attrs
}

/// Recursively parse HTML chunks into DOM children of `parent`.
fn parse_children(doc: &mut Document, parent: NodeId, parser: &mut HtmlChunkParser<'_>) {
    while let Some(chunk) = parser.next() {
        match chunk {
            HtmlChunk::Text(text) => {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    let txt = doc.create_text(trimmed);
                    doc.append_child(parent, txt);
                }
            }
            HtmlChunk::SelfClosing { tag, attrs } => {
                let el = doc.create_element(tag);
                apply_attrs(doc, el, &attrs);
                doc.append_child(parent, el);
            }
            HtmlChunk::OpenTag { tag, attrs } => {
                let el = doc.create_element(tag);
                apply_attrs(doc, el, &attrs);
                doc.append_child(parent, el);
                // Parse children until we hit the matching close tag.
                parse_children(doc, el, parser);
            }
            HtmlChunk::CloseTag { .. } => {
                // We've hit the close tag for our parent — return to caller.
                return;
            }
        }
    }
}

/// Apply parsed HTML attributes to a DOM node.
fn apply_attrs(doc: &mut Document, node: NodeId, attrs: &[(&str, &str)]) {
    for &(key, val) in attrs {
        match key {
            "id" => doc.set_id(node, val),
            "class" => {
                for cls in val.split_whitespace() {
                    doc.add_class(node, cls);
                }
            }
            _ => doc.set_attribute(node, key, val),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Basic registration and retrieval --

    #[test]
    fn register_and_get() {
        let mut reg = TemplateRegistry::new();
        reg.register("hello", "<div>Hello</div>");
        assert_eq!(reg.get("hello"), Some("<div>Hello</div>"));
        assert_eq!(reg.get("nope"), None);
    }

    #[test]
    fn register_defaults_populates_all() {
        let mut reg = TemplateRegistry::new();
        reg.register_defaults();

        let names = reg.list();
        assert!(names.contains(&"desktop"));
        assert!(names.contains(&"window"));
        assert!(names.contains(&"notification"));
        assert!(names.contains(&"context-menu"));
        assert!(names.contains(&"launcher"));
        assert!(names.contains(&"session-menu"));
        assert!(names.contains(&"app-menu"));
        assert!(names.contains(&"statusbar"));
        assert!(names.contains(&"dock"));
        assert!(names.contains(&"dock-item"));
        assert!(names.contains(&"menu-item"));
        assert!(names.contains(&"statusbar-item"));
        assert!(names.contains(&"tooltip"));
        assert_eq!(names.len(), 13);
    }

    #[test]
    fn list_returns_sorted() {
        let mut reg = TemplateRegistry::new();
        reg.register("zebra", "");
        reg.register("alpha", "");
        reg.register("middle", "");
        let names = reg.list();
        assert_eq!(names, vec!["alpha", "middle", "zebra"]);
    }

    // -- Variable interpolation --

    #[test]
    fn render_simple_variables() {
        let mut reg = TemplateRegistry::new();
        reg.register("test", "<div id=\"{{id}}\">{{text}}</div>");

        let mut ctx = TemplateContext::new();
        ctx.set("id", "my-div");
        ctx.set("text", "Hello World");

        let result = reg.render("test", &ctx).unwrap();
        assert_eq!(result, "<div id=\"my-div\">Hello World</div>");
    }

    #[test]
    fn interpolated_values_are_html_escaped() {
        // Regression: T49-e5-F06 — the template engine performed NO HTML
        // escaping, so untrusted notification/title/tooltip content could break
        // out of attributes and inject elements into the shell chrome DOM.
        let mut reg = TemplateRegistry::new();
        reg.register("test", "<box title=\"{{title}}\">{{body}}</box>");

        let mut ctx = TemplateContext::new();
        ctx.set("title", "a\"b>c&d'e");
        ctx.set("body", "<script>alert(1)</script> & \"quote\" > 'apos'");

        let result = reg.render("test", &ctx).unwrap();
        // Attribute value: the closing `"` is neutralised, as is `>`.
        assert_eq!(
            result,
            "<box title=\"a&quot;b&gt;c&amp;d&#39;e\">\
             &lt;script&gt;alert(1)&lt;/script&gt; &amp; &quot;quote&quot; &gt; &#39;apos&#39;\
             </box>"
        );
        // No raw injection survives.
        assert!(!result.contains("<script>"));
        assert!(!result.contains("title=\"a\"b"));
    }

    #[test]
    fn plain_interpolated_value_passes_through_unchanged() {
        // No double-escaping / mangling of ordinary text.
        let mut reg = TemplateRegistry::new();
        reg.register("test", "<div id=\"{{id}}\">{{text}}</div>");

        let mut ctx = TemplateContext::new();
        ctx.set("id", "my-div");
        ctx.set("text", "Hello World 123");

        let result = reg.render("test", &ctx).unwrap();
        assert_eq!(result, "<div id=\"my-div\">Hello World 123</div>");
    }

    #[test]
    fn escape_html_does_not_double_escape() {
        // An already-escaped entity must not be re-escaped (the `&` of `&amp;`
        // becomes `&amp;` once, not `&amp;amp;` — we escape source chars, never
        // re-scan emitted entities).
        assert_eq!(escape_html("&amp;"), "&amp;amp;");
        // (The above documents that a literal `&amp;` in *source* text is treated
        // as the literal characters `& a m p ;` — correct, because the helper's
        // contract is "input is raw untrusted text, not pre-escaped HTML".)
        assert_eq!(escape_html("plain text"), "plain text");
        assert_eq!(
            escape_html("<a href=\"x\">"),
            "&lt;a href=&quot;x&quot;&gt;"
        );
    }

    #[test]
    fn render_missing_variable_empty() {
        let mut reg = TemplateRegistry::new();
        reg.register("test", "<div>{{missing}}</div>");

        let ctx = TemplateContext::new();
        let result = reg.render("test", &ctx).unwrap();
        assert_eq!(result, "<div></div>");
    }

    #[test]
    fn render_missing_template_returns_none() {
        let reg = TemplateRegistry::new();
        let ctx = TemplateContext::new();
        assert!(reg.render("nonexistent", &ctx).is_none());
    }

    // -- Partial includes --

    #[test]
    fn render_partial_include() {
        let mut reg = TemplateRegistry::new();
        reg.register("main", "<div>{{> header}}<p>body</p></div>");
        reg.register("header", "<h1>Title</h1>");

        let ctx = TemplateContext::new();
        let result = reg.render("main", &ctx).unwrap();
        assert_eq!(result, "<div><h1>Title</h1><p>body</p></div>");
    }

    #[test]
    fn render_partial_with_variables() {
        let mut reg = TemplateRegistry::new();
        reg.register("page", "<div>{{> item}}</div>");
        reg.register("item", "<span>{{name}}</span>");

        let mut ctx = TemplateContext::new();
        ctx.set("name", "Alice");

        let result = reg.render("page", &ctx).unwrap();
        assert_eq!(result, "<div><span>Alice</span></div>");
    }

    #[test]
    fn render_missing_partial_produces_empty() {
        let mut reg = TemplateRegistry::new();
        reg.register("test", "<div>{{> ghost}}</div>");

        let ctx = TemplateContext::new();
        let result = reg.render("test", &ctx).unwrap();
        assert_eq!(result, "<div></div>");
    }

    #[test]
    fn render_nested_partials() {
        let mut reg = TemplateRegistry::new();
        reg.register("a", "<a>{{> b}}</a>");
        reg.register("b", "<b>{{> c}}</b>");
        reg.register("c", "<c>leaf</c>");

        let ctx = TemplateContext::new();
        let result = reg.render("a", &ctx).unwrap();
        assert_eq!(result, "<a><b><c>leaf</c></b></a>");
    }

    // -- Circular include protection --

    #[test]
    fn circular_include_protection() {
        let mut reg = TemplateRegistry::new();
        reg.register("a", "<a>{{> a}}</a>");

        let ctx = TemplateContext::new();
        let result = reg.render("a", &ctx).unwrap();
        // The self-include is replaced with empty — no infinite loop.
        assert_eq!(result, "<a></a>");
    }

    #[test]
    fn mutual_circular_include_protection() {
        let mut reg = TemplateRegistry::new();
        reg.register("x", "<x>{{> y}}</x>");
        reg.register("y", "<y>{{> x}}</y>");

        let ctx = TemplateContext::new();
        let result = reg.render("x", &ctx).unwrap();
        // x includes y, y tries to include x but x is on the stack → empty.
        assert_eq!(result, "<x><y></y></x>");
    }

    // -- Conditional blocks --

    #[test]
    fn render_if_truthy() {
        let mut reg = TemplateRegistry::new();
        reg.register("test", "<div>{{#if show}}visible{{/if}}</div>");

        let mut ctx = TemplateContext::new();
        ctx.set("show", true);

        let result = reg.render("test", &ctx).unwrap();
        assert_eq!(result, "<div>visible</div>");
    }

    #[test]
    fn render_if_falsy() {
        let mut reg = TemplateRegistry::new();
        reg.register("test", "<div>{{#if show}}visible{{/if}}</div>");

        let ctx = TemplateContext::new();
        let result = reg.render("test", &ctx).unwrap();
        assert_eq!(result, "<div></div>");
    }

    #[test]
    fn render_if_with_elements() {
        let mut reg = TemplateRegistry::new();
        reg.register(
            "test",
            r#"<window {{#if focused}}class="focused"{{/if}} />"#,
        );

        let mut ctx = TemplateContext::new();
        ctx.set("focused", true);
        let result = reg.render("test", &ctx).unwrap();
        assert_eq!(result, r#"<window class="focused" />"#);

        let ctx2 = TemplateContext::new();
        let result2 = reg.render("test", &ctx2).unwrap();
        assert_eq!(result2, "<window  />");
    }

    // -- Each loops --

    #[test]
    fn render_each_loop() {
        let mut reg = TemplateRegistry::new();
        reg.register("test", "<ul>{{#each items}}<li>{{name}}</li>{{/each}}</ul>");

        let mut ctx = TemplateContext::new();
        let mut item1 = TemplateContext::new();
        item1.set("name", "Alpha");
        let mut item2 = TemplateContext::new();
        item2.set("name", "Beta");
        ctx.set("items", TemplateValue::List(vec![item1, item2]));

        let result = reg.render("test", &ctx).unwrap();
        assert_eq!(result, "<ul><li>Alpha</li><li>Beta</li></ul>");
    }

    #[test]
    fn render_each_empty_list() {
        let mut reg = TemplateRegistry::new();
        reg.register("test", "<ul>{{#each items}}<li>{{name}}</li>{{/each}}</ul>");

        let mut ctx = TemplateContext::new();
        ctx.set("items", TemplateValue::List(vec![]));

        let result = reg.render("test", &ctx).unwrap();
        assert_eq!(result, "<ul></ul>");
    }

    #[test]
    fn render_each_missing_list() {
        let mut reg = TemplateRegistry::new();
        reg.register("test", "<ul>{{#each items}}<li>nope</li>{{/each}}</ul>");

        let ctx = TemplateContext::new();
        let result = reg.render("test", &ctx).unwrap();
        assert_eq!(result, "<ul></ul>");
    }

    #[test]
    fn render_each_with_partials() {
        let mut reg = TemplateRegistry::new();
        reg.register("list", "<ul>{{#each items}}{{> item}}{{/each}}</ul>");
        reg.register("item", "<li>{{name}}</li>");

        let mut ctx = TemplateContext::new();
        let mut i1 = TemplateContext::new();
        i1.set("name", "One");
        let mut i2 = TemplateContext::new();
        i2.set("name", "Two");
        ctx.set("items", TemplateValue::List(vec![i1, i2]));

        let result = reg.render("list", &ctx).unwrap();
        assert_eq!(result, "<ul><li>One</li><li>Two</li></ul>");
    }

    // -- Disk loading --

    #[test]
    fn load_from_disk_with_temp_dir() {
        let dir = std::env::temp_dir().join("liquide_template_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("components")).unwrap();

        std::fs::write(dir.join("page.html"), "<div>page</div>").unwrap();
        std::fs::write(dir.join("components").join("card.html"), "<div>card</div>").unwrap();

        let mut reg = TemplateRegistry::new();
        reg.add_search_path(&dir);
        let count = reg.load_from_disk();

        assert_eq!(count, 2);
        assert_eq!(reg.get("page"), Some("<div>page</div>"));
        assert_eq!(reg.get("components/card"), Some("<div>card</div>"));

        // Cleanup.
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn disk_templates_override_defaults() {
        let dir = std::env::temp_dir().join("liquide_template_override_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("window.html"), "<custom-window />").unwrap();

        let mut reg = TemplateRegistry::new();
        reg.register_defaults();
        reg.add_search_path(&dir);
        reg.load_from_disk();

        // Disk template overrides the embedded default.
        assert_eq!(reg.get("window"), Some("<custom-window />"));
        // Other defaults still present.
        assert!(reg.get("desktop").is_some());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_from_nonexistent_path() {
        let mut reg = TemplateRegistry::new();
        reg.add_search_path("/this/path/does/not/exist/ever");
        let count = reg.load_from_disk();
        assert_eq!(count, 0);
    }

    // -- Render into DOM --

    #[test]
    fn render_into_dom_basic() {
        let mut reg = TemplateRegistry::new();
        reg.register("greet", "<greeting>Hello</greeting>");

        let mut doc = Document::new();
        let root = doc.root();
        let ctx = TemplateContext::new();

        assert!(reg.render_into("greet", &mut doc, root, &ctx));

        // Root should have one child element "greeting".
        let kids = doc.children(root);
        assert_eq!(kids.len(), 1);
        let greeting = kids[0];
        assert_eq!(doc.get(greeting).unwrap().tag_name(), "greeting");

        // "greeting" should have a text child "Hello".
        let text_kids = doc.children(greeting);
        assert_eq!(text_kids.len(), 1);
        assert_eq!(doc.get(text_kids[0]).unwrap().text_content(), Some("Hello"));
    }

    #[test]
    fn render_into_dom_with_attrs() {
        let mut reg = TemplateRegistry::new();
        reg.register(
            "test",
            r#"<statusbar id="sb" class="top primary"><item /></statusbar>"#,
        );

        let mut doc = Document::new();
        let root = doc.root();
        let ctx = TemplateContext::new();

        reg.render_into("test", &mut doc, root, &ctx);

        let sb = doc.get_element_by_id("sb").unwrap();
        let node = doc.get(sb).unwrap();
        assert_eq!(node.tag_name(), "statusbar");
        assert!(node.has_class("top"));
        assert!(node.has_class("primary"));

        // Self-closing child.
        assert_eq!(doc.children(sb).len(), 1);
        assert_eq!(doc.get(doc.children(sb)[0]).unwrap().tag_name(), "item");
    }

    #[test]
    fn render_into_dom_with_variables() {
        let mut reg = TemplateRegistry::new();
        reg.register("notif", r#"<notification id="{{id}}"><notification-title>{{title}}</notification-title></notification>"#);

        let mut doc = Document::new();
        let root = doc.root();
        let mut ctx = TemplateContext::new();
        ctx.set("id", "n-1");
        ctx.set("title", "Alert");

        reg.render_into("notif", &mut doc, root, &ctx);

        let notif = doc.get_element_by_id("n-1").unwrap();
        assert_eq!(doc.get(notif).unwrap().tag_name(), "notification");

        let title_el = doc.children(notif)[0];
        assert_eq!(doc.get(title_el).unwrap().tag_name(), "notification-title");

        let title_text = doc.children(title_el)[0];
        assert_eq!(doc.get(title_text).unwrap().text_content(), Some("Alert"));
    }

    #[test]
    fn render_into_missing_template_returns_false() {
        let reg = TemplateRegistry::new();
        let mut doc = Document::new();
        let root = doc.root();
        let ctx = TemplateContext::new();

        assert!(!reg.render_into("nope", &mut doc, root, &ctx));
        assert_eq!(doc.children(root).len(), 0);
    }

    #[test]
    fn render_into_dom_nested() {
        let mut reg = TemplateRegistry::new();
        reg.register("deep", "<a><b><c>text</c></b></a>");

        let mut doc = Document::new();
        let root = doc.root();
        let ctx = TemplateContext::new();
        reg.render_into("deep", &mut doc, root, &ctx);

        let a = doc.children(root)[0];
        assert_eq!(doc.get(a).unwrap().tag_name(), "a");
        let b = doc.children(a)[0];
        assert_eq!(doc.get(b).unwrap().tag_name(), "b");
        let c = doc.children(b)[0];
        assert_eq!(doc.get(c).unwrap().tag_name(), "c");
        let txt = doc.children(c)[0];
        assert_eq!(doc.get(txt).unwrap().text_content(), Some("text"));
    }

    // -- TemplateContext --

    #[test]
    fn context_truthiness() {
        let mut ctx = TemplateContext::new();
        assert!(!ctx.is_truthy("missing"));

        ctx.set("empty_str", "");
        assert!(!ctx.is_truthy("empty_str"));

        ctx.set("full_str", "hello");
        assert!(ctx.is_truthy("full_str"));

        ctx.set("flag_true", true);
        assert!(ctx.is_truthy("flag_true"));

        ctx.set("flag_false", false);
        assert!(!ctx.is_truthy("flag_false"));

        ctx.set("empty_list", TemplateValue::List(vec![]));
        assert!(!ctx.is_truthy("empty_list"));

        ctx.set(
            "full_list",
            TemplateValue::List(vec![TemplateContext::new()]),
        );
        assert!(ctx.is_truthy("full_list"));
    }

    // -- Default templates render correctly --

    #[test]
    fn default_dock_item_renders() {
        let mut reg = TemplateRegistry::new();
        reg.register_defaults();

        let mut ctx = TemplateContext::new();
        ctx.set("app_id", "files");
        ctx.set("label", "Files");

        let result = reg.render("dock-item", &ctx).unwrap();
        assert!(result.contains("data-app-id=\"files\""));
        assert!(result.contains("data-label=\"Files\""));
        assert!(result.contains(">Files<"));
    }

    #[test]
    fn default_notification_renders() {
        let mut reg = TemplateRegistry::new();
        reg.register_defaults();

        let mut ctx = TemplateContext::new();
        ctx.set("id", "notif-42");
        ctx.set("title", "Alert");
        ctx.set("body", "Something happened");

        let result = reg.render("notification", &ctx).unwrap();
        assert!(result.contains("id=\"notif-42\""));
        assert!(result.contains(">Alert<"));
        assert!(result.contains(">Something happened<"));
    }

    #[test]
    fn default_window_renders_with_focus() {
        let mut reg = TemplateRegistry::new();
        reg.register_defaults();

        let mut ctx = TemplateContext::new();
        ctx.set("id", "win-1");
        ctx.set("title", "My App");
        ctx.set("focused", true);

        let result = reg.render("window", &ctx).unwrap();
        assert!(result.contains("id=\"win-1\""));
        assert!(result.contains("class=\"focused\""));
        assert!(result.contains(">My App<"));
        assert!(result.contains("<minimize-button />"));
    }

    #[test]
    fn default_window_renders_without_focus() {
        let mut reg = TemplateRegistry::new();
        reg.register_defaults();

        let mut ctx = TemplateContext::new();
        ctx.set("id", "win-2");
        ctx.set("title", "Other App");

        let result = reg.render("window", &ctx).unwrap();
        assert!(result.contains("id=\"win-2\""));
        assert!(!result.contains("class=\"focused\""));
    }

    #[test]
    fn default_desktop_with_dock_items() {
        let mut reg = TemplateRegistry::new();
        reg.register_defaults();

        let mut ctx = TemplateContext::new();
        let mut d1 = TemplateContext::new();
        d1.set("app_id", "files");
        d1.set("label", "Files");
        let mut d2 = TemplateContext::new();
        d2.set("app_id", "term");
        d2.set("label", "Terminal");
        ctx.set("dock_items", TemplateValue::List(vec![d1, d2]));
        ctx.set("left_items", TemplateValue::List(vec![]));
        ctx.set("center_items", TemplateValue::List(vec![]));
        ctx.set("right_items", TemplateValue::List(vec![]));

        let result = reg.render("desktop", &ctx).unwrap();
        assert!(result.contains("desktop-background"));
        assert!(result.contains("shell-statusbar"));
        assert!(result.contains("workspace-container"));
        assert!(result.contains("shell-dock"));
        assert!(result.contains("data-app-id=\"files\""));
        assert!(result.contains("data-app-id=\"term\""));
    }

    // -- Partial include edge cases --

    #[test]
    fn partial_include_no_space_after_gt() {
        let mut reg = TemplateRegistry::new();
        reg.register("main", "{{>item}}");
        reg.register("item", "OK");

        let ctx = TemplateContext::new();
        let result = reg.render("main", &ctx).unwrap();
        assert_eq!(result, "OK");
    }

    #[test]
    fn multiple_partials_in_sequence() {
        let mut reg = TemplateRegistry::new();
        reg.register("page", "{{> a}}|{{> b}}|{{> c}}");
        reg.register("a", "A");
        reg.register("b", "B");
        reg.register("c", "C");

        let ctx = TemplateContext::new();
        let result = reg.render("page", &ctx).unwrap();
        assert_eq!(result, "A|B|C");
    }

    // -- Non-recursive partial reuse --

    #[test]
    fn same_partial_used_twice_not_circular() {
        let mut reg = TemplateRegistry::new();
        reg.register("page", "{{> item}}+{{> item}}");
        reg.register("item", "X");

        let ctx = TemplateContext::new();
        let result = reg.render("page", &ctx).unwrap();
        assert_eq!(result, "X+X");
    }

    // -- Nested control structures (depth-tracking block matcher) --

    #[test]
    fn nested_each_inside_each() {
        // Regression: R2 / t60-dom. The flat first-match parser matched the
        // OUTER `{{#each}}` to the INNER `{{/each}}`, truncating the outer body
        // after the first inner loop. With depth tracking the outer block spans
        // to its true `{{/each}}`.
        let mut reg = TemplateRegistry::new();
        reg.register(
            "test",
            "<g>{{#each groups}}<group>{{name}}{{#each members}}<m>{{label}}</m>{{/each}}</group>{{/each}}</g>",
        );

        let mut ctx = TemplateContext::new();
        let mut g1 = TemplateContext::new();
        g1.set("name", "G1");
        let mut m1 = TemplateContext::new();
        m1.set("label", "a");
        let mut m2 = TemplateContext::new();
        m2.set("label", "b");
        g1.set("members", TemplateValue::List(vec![m1, m2]));
        let mut g2 = TemplateContext::new();
        g2.set("name", "G2");
        let mut m3 = TemplateContext::new();
        m3.set("label", "c");
        g2.set("members", TemplateValue::List(vec![m3]));
        ctx.set("groups", TemplateValue::List(vec![g1, g2]));

        let result = reg.render("test", &ctx).unwrap();
        assert_eq!(
            result,
            "<g><group>G1<m>a</m><m>b</m></group><group>G2<m>c</m></group></g>"
        );
    }

    #[test]
    fn nested_if_inside_each() {
        let mut reg = TemplateRegistry::new();
        reg.register(
            "test",
            "{{#each items}}<i>{{name}}{{#if flag}}!{{/if}}</i>{{/each}}",
        );

        let mut ctx = TemplateContext::new();
        let mut a = TemplateContext::new();
        a.set("name", "A");
        a.set("flag", true);
        let mut b = TemplateContext::new();
        b.set("name", "B");
        b.set("flag", false);
        ctx.set("items", TemplateValue::List(vec![a, b]));

        let result = reg.render("test", &ctx).unwrap();
        assert_eq!(result, "<i>A!</i><i>B</i>");
    }

    #[test]
    fn nested_each_inside_if() {
        let mut reg = TemplateRegistry::new();
        reg.register(
            "test",
            "{{#if show}}<list>{{#each items}}<li>{{name}}</li>{{/each}}</list>{{/if}}",
        );

        let mut ctx = TemplateContext::new();
        ctx.set("show", true);
        let mut a = TemplateContext::new();
        a.set("name", "One");
        let mut b = TemplateContext::new();
        b.set("name", "Two");
        ctx.set("items", TemplateValue::List(vec![a, b]));

        let result = reg.render("test", &ctx).unwrap();
        assert_eq!(result, "<list><li>One</li><li>Two</li></list>");
    }

    #[test]
    fn deeply_nested_if_chains() {
        // Three same-type blocks nested; the matcher must pair each open with the
        // correct close at its own depth.
        let mut reg = TemplateRegistry::new();
        reg.register(
            "test",
            "{{#if a}}A{{#if b}}B{{#if c}}C{{/if}}B2{{/if}}A2{{/if}}",
        );

        let mut all = TemplateContext::new();
        all.set("a", true);
        all.set("b", true);
        all.set("c", true);
        assert_eq!(reg.render("test", &all).unwrap(), "ABCB2A2");

        let mut no_c = TemplateContext::new();
        no_c.set("a", true);
        no_c.set("b", true);
        no_c.set("c", false);
        assert_eq!(reg.render("test", &no_c).unwrap(), "ABB2A2");

        let mut only_a = TemplateContext::new();
        only_a.set("a", true);
        assert_eq!(reg.render("test", &only_a).unwrap(), "AA2");
    }

    #[test]
    fn nested_if_else_inside_each_picks_correct_branch() {
        // An `{{else}}` belonging to a nested `{{#if}}` must not be confused with
        // any outer split, and the depth-aware else splitter must select the right
        // branch per item.
        let mut reg = TemplateRegistry::new();
        reg.register(
            "test",
            "{{#each items}}<i>{{#if on}}ON{{else}}OFF{{/if}}</i>{{/each}}",
        );

        let mut ctx = TemplateContext::new();
        let mut a = TemplateContext::new();
        a.set("on", true);
        let mut b = TemplateContext::new();
        b.set("on", false);
        ctx.set("items", TemplateValue::List(vec![a, b]));

        assert_eq!(reg.render("test", &ctx).unwrap(), "<i>ON</i><i>OFF</i>");
    }

    #[test]
    fn nested_if_with_else_chain_outer_else_preserved() {
        // Outer if has its own {{else}} AFTER a fully-nested inner if/else. The
        // depth-aware splitter must pick the OUTER else, not the inner one.
        let mut reg = TemplateRegistry::new();
        reg.register(
            "test",
            "{{#if outer}}O{{#if inner}}I{{else}}NI{{/if}}{{else}}NO{{/if}}",
        );

        let mut t = TemplateContext::new();
        t.set("outer", true);
        t.set("inner", false);
        assert_eq!(reg.render("test", &t).unwrap(), "ONI");

        let mut f = TemplateContext::new();
        f.set("outer", false);
        assert_eq!(reg.render("test", &f).unwrap(), "NO");
    }

    #[test]
    fn notifications_like_template_with_nested_actions() {
        // End-to-end shape of assets/templates/notifications.html: an outer
        // `{{#each notifications}}` whose body contains `{{#if has_actions}}` →
        // `{{#each actions}}`. Under the old flat parser the outer each matched
        // the INNER `{{/each}}` (the actions loop), truncating each notification
        // after its first action block and dropping subsequent notifications.
        let mut reg = TemplateRegistry::new();
        reg.register(
            "notifications",
            concat!(
                "<notification-area>",
                "{{#each notifications}}",
                "<notification id=\"{{id}}\">",
                "<notification-title>{{title}}</notification-title>",
                "<notification-body>{{body}}</notification-body>",
                "{{#if has_actions}}",
                "<notification-actions>",
                "{{#each actions}}",
                "<notification-action data-action-id=\"{{action_id}}\">{{label}}</notification-action>",
                "{{/each}}",
                "</notification-actions>",
                "{{/if}}",
                "</notification>",
                "{{/each}}",
                "</notification-area>",
            ),
        );

        let mut ctx = TemplateContext::new();

        // First notification: has two actions.
        let mut n1 = TemplateContext::new();
        n1.set("id", "n1");
        n1.set("title", "Build done");
        n1.set("body", "Success");
        n1.set("has_actions", true);
        let mut act1 = TemplateContext::new();
        act1.set("action_id", "open");
        act1.set("label", "Open");
        let mut act2 = TemplateContext::new();
        act2.set("action_id", "dismiss");
        act2.set("label", "Dismiss");
        n1.set("actions", TemplateValue::List(vec![act1, act2]));

        // Second notification: no actions — must STILL render (proves the outer
        // loop was not truncated by the inner {{/each}}).
        let mut n2 = TemplateContext::new();
        n2.set("id", "n2");
        n2.set("title", "Reminder");
        n2.set("body", "Standup at 10");
        n2.set("has_actions", false);

        ctx.set("notifications", TemplateValue::List(vec![n1, n2]));

        let result = reg.render("notifications", &ctx).unwrap();

        // Both notifications present (outer loop not truncated).
        assert!(result.contains("id=\"n1\""), "n1 missing: {result}");
        assert!(result.contains("id=\"n2\""), "n2 missing: {result}");
        // First notification's full body survives past the nested actions block.
        assert!(result.contains("<notification-body>Success</notification-body>"));
        assert!(result.contains("<notification-body>Standup at 10</notification-body>"));
        // Both actions of n1 render.
        assert!(result.contains("data-action-id=\"open\">Open<"));
        assert!(result.contains("data-action-id=\"dismiss\">Dismiss<"));
        // n2 has no actions block.
        let n2_pos = result.find("id=\"n2\"").unwrap();
        assert!(!result[n2_pos..].contains("notification-action"));
        // Exact full render.
        assert_eq!(
            result,
            concat!(
                "<notification-area>",
                "<notification id=\"n1\">",
                "<notification-title>Build done</notification-title>",
                "<notification-body>Success</notification-body>",
                "<notification-actions>",
                "<notification-action data-action-id=\"open\">Open</notification-action>",
                "<notification-action data-action-id=\"dismiss\">Dismiss</notification-action>",
                "</notification-actions>",
                "</notification>",
                "<notification id=\"n2\">",
                "<notification-title>Reminder</notification-title>",
                "<notification-body>Standup at 10</notification-body>",
                "</notification>",
                "</notification-area>",
            )
        );
    }

    #[test]
    fn default_statusbar_nested_if_chain_renders_each_type() {
        // The embedded DEFAULT_STATUSBAR has deeply nested if/else chains inside
        // {{#each right_items}}. Verify each item type selects the correct branch
        // and the chain is not truncated.
        let mut reg = TemplateRegistry::new();
        reg.register_defaults();

        let mut ctx = TemplateContext::new();
        ctx.set("left_items", TemplateValue::List(vec![]));
        ctx.set("center_items", TemplateValue::List(vec![]));

        let mut notif = TemplateContext::new();
        notif.set("type_notification", true);
        notif.set("id", "ni");
        notif.set("classes", "");
        notif.set("text", "3");
        let mut session = TemplateContext::new();
        session.set("type_session", true);
        session.set("id", "se");
        session.set("text", "user");
        ctx.set("right_items", TemplateValue::List(vec![notif, session]));

        let result = reg.render("statusbar", &ctx).unwrap();
        // Notification branch chosen for first item.
        assert!(result.contains("<notification-indicator id=\"ni\""));
        // Session branch (the LAST else in the chain) chosen for second item —
        // proves the chain was not truncated before reaching type_session.
        assert!(result.contains("<session-button id=\"se\">user</session-button>"));
    }

    // -- Block matcher unit tests --

    #[test]
    fn find_block_end_skips_nested_same_tag() {
        // body is everything after `{{#each outer}}`.
        let body = "X{{#each inner}}Y{{/each}}Z{{/each}}TAIL";
        let end = find_block_end(body, "each").unwrap();
        assert_eq!(&body[..end], "X{{#each inner}}Y{{/each}}Z");
        assert_eq!(&body[end..], "{{/each}}TAIL");
    }

    #[test]
    fn find_block_end_missing_close_returns_none() {
        assert_eq!(find_block_end("no close here", "if"), None);
        // Unbalanced: inner closes but outer never does.
        assert_eq!(find_block_end("{{#if a}}x{{/if}}", "if"), None);
    }

    #[test]
    fn split_else_ignores_nested_else() {
        // Block body of an outer `{{#if}}`: nested if has its own else, then the
        // outer else follows.
        let block = "A{{#if x}}B{{else}}C{{/if}}D{{else}}E";
        let (then_part, else_part) = split_else(block, "if");
        assert_eq!(then_part, "A{{#if x}}B{{else}}C{{/if}}D");
        assert_eq!(else_part, Some("E"));
    }

    #[test]
    fn split_else_none_when_absent() {
        let (then_part, else_part) = split_else("just{{#if y}}z{{/if}}body", "if");
        assert_eq!(then_part, "just{{#if y}}z{{/if}}body");
        assert_eq!(else_part, None);
    }

    // -- HTML parser edge cases --

    #[test]
    fn parse_html_comments_skipped() {
        let mut reg = TemplateRegistry::new();
        reg.register("test", "<!-- comment --><div>ok</div>");

        let mut doc = Document::new();
        let root = doc.root();
        let ctx = TemplateContext::new();
        reg.render_into("test", &mut doc, root, &ctx);

        let kids = doc.children(root);
        assert_eq!(kids.len(), 1);
        assert_eq!(doc.get(kids[0]).unwrap().tag_name(), "div");
    }

    #[test]
    fn parse_html_multiple_siblings() {
        let mut reg = TemplateRegistry::new();
        reg.register("test", "<a /><b /><c />");

        let mut doc = Document::new();
        let root = doc.root();
        let ctx = TemplateContext::new();
        reg.render_into("test", &mut doc, root, &ctx);

        let kids = doc.children(root);
        assert_eq!(kids.len(), 3);
        assert_eq!(doc.get(kids[0]).unwrap().tag_name(), "a");
        assert_eq!(doc.get(kids[1]).unwrap().tag_name(), "b");
        assert_eq!(doc.get(kids[2]).unwrap().tag_name(), "c");
    }
}
