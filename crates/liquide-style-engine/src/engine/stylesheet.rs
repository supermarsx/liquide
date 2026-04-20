//! Stylesheet parsing and compilation into prepared rules.
//!
//! Supports CSS `@import` resolution when loading from files. Relative
//! import URLs are resolved against the importing stylesheet's directory.
//! Cycle detection (canonical path set) and a maximum import depth of 10
//! prevent infinite loops.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use liquide_theme_css::ThemeParser;

use super::{
    ContainerCondition, PreparedFontFace, PreparedRule, PreparedSheet, RegisteredPropertyDef,
    StyleEngine,
};
use crate::selector::ComplexSelector;

/// Maximum `@import` nesting depth to prevent stack overflow.
const MAX_IMPORT_DEPTH: usize = 10;

impl StyleEngine {
    /// Parse and add a CSS stylesheet (inline — no `@import` resolution).
    pub fn add_stylesheet(&mut self, css: &str) {
        let parser = ThemeParser::new();
        let stylesheet = match parser.parse_str(css) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("Failed to parse stylesheet: {}", e);
                return;
            }
        };

        self.compile_stylesheet(&stylesheet);
    }

    /// Load and add a CSS stylesheet from a file on disk.
    ///
    /// Reads the file contents, resolves any `@import` rules relative to
    /// the file's directory, and adds all resulting rules to the engine.
    /// Returns `Err` with a human-readable message if the file cannot be
    /// read.
    pub fn load_stylesheet_file(&mut self, path: &std::path::Path) -> Result<(), String> {
        let canonical = path.canonicalize().map_err(|e| {
            format!("Failed to resolve stylesheet path {}: {}", path.display(), e)
        })?;
        let mut visited = HashSet::new();
        self.load_stylesheet_file_inner(&canonical, &mut visited, 0)
    }

    /// Internal recursive loader with cycle detection and depth limiting.
    fn load_stylesheet_file_inner(
        &mut self,
        path: &Path,
        visited: &mut HashSet<PathBuf>,
        depth: usize,
    ) -> Result<(), String> {
        if depth > MAX_IMPORT_DEPTH {
            tracing::warn!(
                "CSS @import depth limit ({}) exceeded at {}",
                MAX_IMPORT_DEPTH,
                path.display()
            );
            return Ok(());
        }

        if !visited.insert(path.to_path_buf()) {
            tracing::warn!(
                "CSS @import cycle detected — skipping {}",
                path.display()
            );
            return Ok(());
        }

        let css = std::fs::read_to_string(path).map_err(|e| {
            format!("Failed to read stylesheet {}: {}", path.display(), e)
        })?;
        tracing::info!("Loaded stylesheet from {}", path.display());

        let parser = ThemeParser::new();
        let stylesheet = match parser.parse_str(&css) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("Failed to parse stylesheet {}: {}", path.display(), e);
                return Ok(());
            }
        };

        // ── Resolve @import rules first (per CSS spec, @import must precede
        //    all other rules) ─────────────────────────────────────────────
        let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
        for import_url in stylesheet.imports() {
            let url = Self::strip_import_url(import_url);
            if url.is_empty() {
                continue;
            }
            let import_path = base_dir.join(&url);
            let canonical = match import_path.canonicalize() {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!(
                        "CSS @import: cannot resolve '{}' (from {}): {}",
                        url,
                        path.display(),
                        e
                    );
                    continue;
                }
            };
            if let Err(e) = self.load_stylesheet_file_inner(&canonical, visited, depth + 1) {
                tracing::warn!(
                    "CSS @import: failed to load '{}' (from {}): {}",
                    url,
                    path.display(),
                    e
                );
            }
        }

        // ── Now compile the stylesheet's own rules ──────────────────────
        self.compile_stylesheet(&stylesheet);
        Ok(())
    }

    /// Strip `url(...)` wrapper and surrounding quotes from an `@import` URL
    /// string, returning the bare path.
    fn strip_import_url(raw: &str) -> String {
        let mut s = raw.trim();
        // Remove url(...) wrapper
        if let Some(inner) = s.strip_prefix("url(") {
            s = inner
                .strip_suffix(')')
                .unwrap_or(inner)
                .trim();
        }
        // Remove quotes
        if (s.starts_with('"') && s.ends_with('"'))
            || (s.starts_with('\'') && s.ends_with('\''))
        {
            s = &s[1..s.len() - 1];
        }
        s.to_string()
    }

    /// Compile a parsed stylesheet's rules and at-rules into the engine,
    /// without handling `@import` (those must be resolved by callers).
    fn compile_stylesheet(&mut self, stylesheet: &liquide_theme_css::StyleSheet) {
        // ── @layer ordering ─────────────────────────────────────────────
        for layer_name in stylesheet.layer_order() {
            let next_idx = self.layer_order.len() as u32 + 1;
            self.layer_order
                .entry(layer_name.to_string())
                .or_insert(next_idx);
        }

        // ── @font-face rules ────────────────────────────────────────────
        for ff in stylesheet.font_faces() {
            self.font_faces.push(PreparedFontFace {
                family: ff.family.clone(),
                sources: ff.src.clone(),
                weight: ff.weight,
                style: ff.style.clone(),
                display: ff.display.clone(),
                unicode_range: ff.unicode_range.clone(),
            });
        }

        // ── @property registrations ─────────────────────────────────────
        for prop in stylesheet.registered_properties() {
            self.registered_properties.insert(
                prop.name.clone(),
                RegisteredPropertyDef {
                    syntax: prop.syntax.clone(),
                    inherits: prop.inherits,
                    initial_value: prop.initial_value.clone(),
                },
            );
        }

        // ── @keyframes rules ────────────────────────────────────────────
        for (name, kf_rule) in stylesheet.keyframes() {
            self.keyframes.insert(name.clone(), kf_rule.clone());
        }

        // ── Extract variables ───────────────────────────────────────────
        for rule in stylesheet.rules() {
            for (key, val) in rule.properties.iter() {
                if key.starts_with("--") {
                    self.variables.insert(key.clone(), val.clone());
                }
            }
        }

        // ── Compile normal rules ────────────────────────────────────────
        let mut prepared_rules = Vec::new();
        let mut order = self
            .sheets
            .iter()
            .map(|s| s.rules.len() as u32)
            .sum::<u32>();

        for rule in stylesheet.rules() {
            // Use the raw selector string directly — it preserves combinators,
            // attribute selectors, and functional pseudo-classes from lightningcss.
            let selector_str = &rule.selector.raw;

            if let Some(complex) = ComplexSelector::parse(selector_str) {
                let specificity = complex.specificity();
                // Resolve layer order for this rule
                let layer_ord = rule
                    .layer
                    .as_ref()
                    .and_then(|name| self.layer_order.get(name))
                    .copied()
                    .unwrap_or(0);
                prepared_rules.push(PreparedRule {
                    selector: complex,
                    specificity,
                    source_order: order,
                    properties: rule.properties.clone(),
                    media_condition: rule.media_condition.clone(),
                    layer_order: layer_ord,
                    container_condition: None,
                    supports_condition: rule.supports_condition.clone(),
                    pseudo_element: rule.selector.pseudo_element.clone(),
                });
                order += 1;
            }
        }

        // ── Compile @container query rules ──────────────────────────────
        for cr in stylesheet.container_rules() {
            for rule in &cr.rules {
                let selector_str = &rule.selector.raw;
                if let Some(complex) = ComplexSelector::parse(selector_str) {
                    let specificity = complex.specificity();
                    let layer_ord = rule
                        .layer
                        .as_ref()
                        .and_then(|name| self.layer_order.get(name))
                        .copied()
                        .unwrap_or(0);
                    prepared_rules.push(PreparedRule {
                        selector: complex,
                        specificity,
                        source_order: order,
                        properties: rule.properties.clone(),
                        media_condition: rule.media_condition.clone(),
                        layer_order: layer_ord,
                        container_condition: Some(ContainerCondition {
                            name: cr.name.clone(),
                            condition: cr.condition.clone(),
                        }),
                        supports_condition: rule.supports_condition.clone(),
                        pseudo_element: rule.selector.pseudo_element.clone(),
                    });
                    order += 1;
                }
            }
        }

        // ── Compile @scope rules ────────────────────────────────────────
        // Current behavior: scope-start is applied as an ancestor prefix to
        // each nested selector. scope-end is retained in data model but is not
        // yet enforced here.
        for scope_rule in stylesheet.scope_rules() {
            let scope_prefix = scope_rule.scope_start.as_deref().unwrap_or("").trim();
            for rule in &scope_rule.rules {
                let selector_str = if scope_prefix.is_empty() {
                    rule.selector.raw.clone()
                } else {
                    format!("{} {}", scope_prefix, rule.selector.raw)
                };
                if let Some(complex) = ComplexSelector::parse(&selector_str) {
                    let specificity = complex.specificity();
                    let layer_ord = rule
                        .layer
                        .as_ref()
                        .and_then(|name| self.layer_order.get(name))
                        .copied()
                        .unwrap_or(0);
                    prepared_rules.push(PreparedRule {
                        selector: complex,
                        specificity,
                        source_order: order,
                        properties: rule.properties.clone(),
                        media_condition: rule.media_condition.clone(),
                        layer_order: layer_ord,
                        container_condition: None,
                        supports_condition: rule.supports_condition.clone(),
                        pseudo_element: rule.selector.pseudo_element.clone(),
                    });
                    order += 1;
                }
            }
        }

        self.sheets.push(PreparedSheet::new(prepared_rules));
    }

    /// Load all `.css` files from a directory.
    ///
    /// Files are loaded in alphabetical order. Returns the number of
    /// stylesheets successfully loaded, or `Err` if the directory itself
    /// cannot be read. Individual file errors are logged as warnings and
    /// skipped.
    pub fn load_stylesheet_dir(&mut self, dir: &std::path::Path) -> Result<usize, String> {
        let entries = std::fs::read_dir(dir).map_err(|e| {
            format!("Failed to read stylesheet directory {}: {}", dir.display(), e)
        })?;

        let mut css_files: Vec<std::path::PathBuf> = entries
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "css") && path.is_file() {
                    Some(path)
                } else {
                    None
                }
            })
            .collect();

        // Sort alphabetically for deterministic load order.
        css_files.sort();

        let mut loaded = 0usize;
        for path in &css_files {
            match self.load_stylesheet_file(path) {
                Ok(()) => loaded += 1,
                Err(e) => tracing::warn!("{}", e),
            }
        }

        tracing::info!(
            "Loaded {}/{} stylesheets from {}",
            loaded,
            css_files.len(),
            dir.display()
        );
        Ok(loaded)
    }

    /// Load user CSS overrides from a configuration directory.
    ///
    /// Looks for `custom.css` inside `config_dir`. If it exists, the file
    /// is loaded **after** theme stylesheets so its rules take precedence.
    /// If the file does not exist, this is a no-op.
    pub fn load_user_overrides(&mut self, config_dir: &std::path::Path) {
        let custom_css = config_dir.join("custom.css");
        if custom_css.is_file() {
            match self.load_stylesheet_file(&custom_css) {
                Ok(()) => tracing::info!("Loaded user CSS overrides from {}", custom_css.display()),
                Err(e) => tracing::warn!("Could not load user CSS overrides: {}", e),
            }
        }
    }
}
