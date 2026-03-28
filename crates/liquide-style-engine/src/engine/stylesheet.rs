//! Stylesheet parsing and compilation into prepared rules.

use liquide_theme_css::ThemeParser;

use super::{
    ContainerCondition, PreparedFontFace, PreparedRule, PreparedSheet, RegisteredPropertyDef,
    StyleEngine,
};
use crate::selector::ComplexSelector;

impl StyleEngine {
    /// Parse and add a CSS stylesheet.
    pub fn add_stylesheet(&mut self, css: &str) {
        let parser = ThemeParser::new();
        let stylesheet = match parser.parse_str(css) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("Failed to parse stylesheet: {}", e);
                return;
            }
        };

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

    /// Load and add a CSS stylesheet from a file on disk.
    ///
    /// Reads the file contents and delegates to [`add_stylesheet`]. Returns
    /// `Err` with a human-readable message if the file cannot be read.
    pub fn load_stylesheet_file(&mut self, path: &std::path::Path) -> Result<(), String> {
        let css = std::fs::read_to_string(path).map_err(|e| {
            format!("Failed to read stylesheet {}: {}", path.display(), e)
        })?;
        tracing::info!("Loaded stylesheet from {}", path.display());
        self.add_stylesheet(&css);
        Ok(())
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
