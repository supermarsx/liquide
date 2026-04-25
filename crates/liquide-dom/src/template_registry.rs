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
// TemplateContext — the data passed into template rendering
// ---------------------------------------------------------------------------

/// A value that can be substituted into a template.
#[derive(Debug, Clone)]
pub enum TemplateValue {
    /// A simple string value.
    String(String),
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

    /// Get a variable value.
    pub fn get(&self, key: &str) -> Option<&TemplateValue> {
        self.vars.get(key)
    }

    /// Get a string variable, returning empty string if missing or non-string.
    pub fn get_str(&self, key: &str) -> &str {
        match self.vars.get(key) {
            Some(TemplateValue::String(s)) => s.as_str(),
            _ => "",
        }
    }

    /// Check if a variable is truthy (exists and is not empty/false).
    pub fn is_truthy(&self, key: &str) -> bool {
        match self.vars.get(key) {
            None => false,
            Some(TemplateValue::String(s)) => !s.is_empty(),
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

const DEFAULT_NOTIFICATION: &str = r#"<notification id="{{id}}">
  <notification-title>{{title}}</notification-title>
  <notification-body>{{body}}</notification-body>
</notification>
"#;

const DEFAULT_CONTEXT_MENU: &str = r#"<context-menu id="{{id}}" style="left: {{pos_left}}; top: {{pos_top}}">
  {{#each items}}{{> menu-item}}{{/each}}
</context-menu>
"#;

const DEFAULT_LAUNCHER: &str = r#"<launcher-overlay id="launcher-overlay">
  <launcher id="shell-launcher">
    <launcher-search id="launcher-search" />
    <launcher-results>
      {{#each items}}
      <launcher-item data-app-id="{{app_id}}" data-icon="{{icon}}">{{label}}</launcher-item>
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

const DEFAULT_MENU_ITEM: &str = r#"<menu-item data-action="{{action}}" {{#if icon}}data-icon="{{icon}}"{{/if}}>{{label}}</menu-item>"#;

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
                let block_var = block_var.trim();
                let end_tag = "{{/if}}";
                if let Some(end_pos) = rest.find(end_tag) {
                    let block_body = &rest[..end_pos];
                    rest = &rest[end_pos + end_tag.len()..];
                    // Split on {{else}} if present
                    let (if_body, else_body) = match block_body.find("{{else}}") {
                        Some(else_pos) => (
                            &block_body[..else_pos],
                            &block_body[else_pos + "{{else}}".len()..],
                        ),
                        None => (block_body, ""),
                    };
                    if ctx.is_truthy(block_var) {
                        out.push_str(&self.render_source(if_body, ctx, visited));
                    } else if !else_body.is_empty() {
                        out.push_str(&self.render_source(else_body, ctx, visited));
                    }
                }
                // Malformed (no closing tag) — silently skip.
            } else if let Some(list_var) = tag.strip_prefix("#each ") {
                // ---------- iteration block ----------
                let list_var = list_var.trim();
                let end_tag = "{{/each}}";
                if let Some(end_pos) = rest.find(end_tag) {
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
                out.push_str(ctx.get_str(tag));
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
