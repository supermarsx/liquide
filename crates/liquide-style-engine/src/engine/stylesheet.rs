//! Stylesheet parsing and compilation into prepared rules.
//!
//! Supports CSS `@import` resolution through the shared `liquide-theme-css`
//! stylesheet loader so file load and watcher reload compile the same rule tree.

use std::path::Path;

use liquide_theme_css::ThemeParser;
use liquide_theme_css::stylesheet::{
    STRUCTURAL_CONDITION_SENTINEL, StructuralCondition, StyleRule,
};

use super::{
    ContainerCondition, PreparedFontFace, PreparedRule, PreparedSheet, RegisteredPropertyDef,
    StyleEngine,
};
use crate::selector::ComplexSelector;

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
        let stylesheet = liquide_theme_css::StyleSheet::load_path_with_imports(path)
            .map_err(|e| format!("Failed to load stylesheet {}: {}", path.display(), e))?;

        self.compile_stylesheet(&stylesheet);
        Ok(())
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
            self.push_prepared_rule(&mut prepared_rules, rule, &mut order, None, None);
        }

        // ── Compile @container query rules ──────────────────────────────
        for cr in stylesheet.container_rules() {
            let (container_condition, scope_prefix) =
                Self::decode_structural_container_condition(cr.name.as_deref(), &cr.condition);
            for rule in &cr.rules {
                self.push_prepared_rule(
                    &mut prepared_rules,
                    rule,
                    &mut order,
                    scope_prefix.as_deref(),
                    container_condition.clone(),
                );
            }
        }

        // ── Compile @scope rules ────────────────────────────────────────
        for scope_rule in stylesheet.scope_rules() {
            let scope_condition = if scope_rule.scope_end.is_some() {
                Some(ContainerCondition {
                    name: Some(STRUCTURAL_CONDITION_SENTINEL.to_string()),
                    condition: liquide_theme_css::StyleSheet::encode_structural_condition(
                        &StructuralCondition {
                            containers: Vec::new(),
                            scope_start: scope_rule.scope_start.clone(),
                            scope_end: scope_rule.scope_end.clone(),
                        },
                    ),
                })
            } else {
                None
            };
            for rule in &scope_rule.rules {
                self.push_prepared_rule(
                    &mut prepared_rules,
                    rule,
                    &mut order,
                    scope_rule.scope_start.as_deref(),
                    scope_condition.clone(),
                );
            }
        }

        self.sheets.push(PreparedSheet::new(prepared_rules));
    }

    fn push_prepared_rule(
        &self,
        prepared_rules: &mut Vec<PreparedRule>,
        rule: &StyleRule,
        order: &mut u32,
        scope_prefix: Option<&str>,
        container_condition: Option<ContainerCondition>,
    ) {
        let selector_str = Self::prefix_scope(scope_prefix, &rule.selector.raw);
        if let Some(complex) = ComplexSelector::parse(&selector_str) {
            let specificity = complex.specificity();
            // Per CSS Cascade 5 §6.4.2, unlayered author styles act as the LAST
            // (highest-priority for normal) implicit layer: normal unlayered rules
            // beat every `@layer` rule, and the CascadePriority `Ord` reverses this
            // for `!important`. Declared layers are 1..=N; encode "no layer" as the
            // maximum so it sorts after all declared layers. (Previously this was 0,
            // which made unlayered rules LOSE to every layered rule.)
            let layer_ord = rule
                .layer
                .as_ref()
                .and_then(|name| self.layer_order.get(name))
                .copied()
                .unwrap_or(u32::MAX);
            prepared_rules.push(PreparedRule {
                selector: complex,
                specificity,
                source_order: *order,
                properties: rule.properties.clone(),
                media_condition: rule.media_condition.clone(),
                layer_order: layer_ord,
                container_condition,
                supports_condition: rule.supports_condition.clone(),
                pseudo_element: rule.selector.pseudo_element.clone(),
            });
            *order += 1;
        }
    }

    fn decode_structural_container_condition(
        name: Option<&str>,
        condition: &str,
    ) -> (Option<ContainerCondition>, Option<String>) {
        if let Some(structural) =
            liquide_theme_css::StyleSheet::decode_structural_condition(name, condition)
        {
            let scope_prefix = structural.scope_start.clone();
            let needs_structural_runtime =
                structural.scope_end.is_some() || structural.containers.len() > 1;
            let container_condition = if needs_structural_runtime {
                Some(ContainerCondition {
                    name: Some(STRUCTURAL_CONDITION_SENTINEL.to_string()),
                    condition: liquide_theme_css::StyleSheet::encode_structural_condition(
                        &structural,
                    ),
                })
            } else if let Some(container) = structural.containers.into_iter().next() {
                Some(ContainerCondition {
                    name: container.name,
                    condition: container.condition,
                })
            } else {
                None
            };

            (container_condition, scope_prefix)
        } else {
            (
                Some(ContainerCondition {
                    name: name.map(|value| value.to_string()),
                    condition: condition.to_string(),
                }),
                None,
            )
        }
    }

    fn prefix_scope(scope_prefix: Option<&str>, selector: &str) -> String {
        match scope_prefix
            .map(str::trim)
            .filter(|prefix| !prefix.is_empty())
        {
            Some(prefix) => format!("{prefix} {selector}"),
            None => selector.to_string(),
        }
    }

    /// Load all `.css` files from a directory.
    ///
    /// Files are loaded in alphabetical order. Returns the number of
    /// stylesheets successfully loaded, or `Err` if the directory itself
    /// cannot be read. Individual file errors are logged as warnings and
    /// skipped.
    pub fn load_stylesheet_dir(&mut self, dir: &std::path::Path) -> Result<usize, String> {
        let stylesheet =
            liquide_theme_css::StyleSheet::load_paths_with_imports(&[dir.to_path_buf()]).map_err(
                |e| {
                    format!(
                        "Failed to load stylesheet directory {}: {}",
                        dir.display(),
                        e
                    )
                },
            )?;
        let loaded = Self::count_css_files(dir).map_err(|e| {
            format!(
                "Failed to count stylesheet files in {}: {}",
                dir.display(),
                e
            )
        })?;

        self.compile_stylesheet(&stylesheet);

        tracing::info!("Loaded {} stylesheet files from {}", loaded, dir.display());
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

    fn count_css_files(dir: &Path) -> std::io::Result<usize> {
        let mut count = 0usize;
        let mut entries: Vec<_> = std::fs::read_dir(dir)?.collect::<std::result::Result<_, _>>()?;
        entries.sort_by_key(|entry| entry.path());

        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                count += Self::count_css_files(&path)?;
            } else if path.extension().is_some_and(|ext| ext == "css") && path.is_file() {
                count += 1;
            }
        }

        Ok(count)
    }
}
