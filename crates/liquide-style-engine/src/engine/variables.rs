//! CSS `var()` and `env()` variable resolution.

use super::StyleEngine;
use crate::value_resolve::parse_inline_value;

impl StyleEngine {
    /// Resolve all `var(--name)` / `var(--name, fallback)` references in a value string.
    ///
    /// Returns a re-parsed `PropertyValue` with variables substituted, or `None`
    /// if a referenced variable is missing and no fallback is provided.
    ///
    /// Per CSS spec, cyclic variable references produce the "guaranteed-invalid"
    /// value. We detect cycles by tracking which variables are currently being
    /// resolved in a resolution stack.
    pub(crate) fn resolve_var_in_value(
        &self,
        value: &str,
        scope_vars: &std::collections::HashMap<String, liquide_theme_css::value::PropertyValue>,
    ) -> Option<liquide_theme_css::value::PropertyValue> {
        let mut resolution_stack: Vec<String> = Vec::new();
        self.resolve_var_recursive(value, scope_vars, &mut resolution_stack)
    }

    fn resolve_var_recursive(
        &self,
        value: &str,
        scope_vars: &std::collections::HashMap<String, liquide_theme_css::value::PropertyValue>,
        resolution_stack: &mut Vec<String>,
    ) -> Option<liquide_theme_css::value::PropertyValue> {
        let mut result = value.to_string();
        // Limit iterations to prevent runaway resolution (safety valve)
        let mut iterations = 0;
        while let Some(start) = result.find("var(") {
            iterations += 1;
            if iterations > 64 {
                return None; // safety valve
            }
            let rest = &result[start + 4..];
            // Find matching close paren (handle nesting)
            let mut depth = 1i32;
            let mut end = 0;
            for (i, ch) in rest.char_indices() {
                match ch {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            end = i;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if depth != 0 {
                return None; // unmatched parens
            }

            let inner = &rest[..end];
            let (var_name, fallback) = if let Some(comma_pos) = Self::find_top_level_comma(inner) {
                (
                    inner[..comma_pos].trim(),
                    Some(inner[comma_pos + 1..].trim()),
                )
            } else {
                (inner.trim(), None)
            };

            // Cycle detection: if this variable is already being resolved, it's circular
            if resolution_stack.contains(&var_name.to_string()) {
                // Per spec: cyclic references produce the guaranteed-invalid value
                if let Some(fb) = fallback {
                    result = format!("{}{}{}", &result[..start], fb, &rest[end + 1..]);
                    continue;
                }
                return None;
            }

            if let Some(resolved) = scope_vars
                .get(var_name)
                .or_else(|| self.variables.get(var_name))
            {
                let replacement = match resolved {
                    liquide_theme_css::value::PropertyValue::Color(c) => c.to_hex(),
                    liquide_theme_css::value::PropertyValue::Length(lu) => {
                        format!("{}px", lu.to_px(self.base_font_size))
                    }
                    liquide_theme_css::value::PropertyValue::Number(n) => format!("{}", n),
                    liquide_theme_css::value::PropertyValue::Keyword(kw) => {
                        // If the keyword itself contains var() references, resolve recursively
                        if kw.contains("var(") {
                            resolution_stack.push(var_name.to_string());
                            let resolved =
                                self.resolve_var_recursive(kw, scope_vars, resolution_stack);
                            resolution_stack.pop();
                            match resolved {
                                Some(pv) => match pv {
                                    liquide_theme_css::value::PropertyValue::Keyword(k) => k,
                                    liquide_theme_css::value::PropertyValue::String(s) => s,
                                    other => format!("{}", other),
                                },
                                None => {
                                    if let Some(fb) = fallback {
                                        fb.to_string()
                                    } else {
                                        return None;
                                    }
                                }
                            }
                        } else {
                            kw.clone()
                        }
                    }
                    liquide_theme_css::value::PropertyValue::String(s) => {
                        if s.contains("var(") {
                            resolution_stack.push(var_name.to_string());
                            let resolved =
                                self.resolve_var_recursive(s, scope_vars, resolution_stack);
                            resolution_stack.pop();
                            match resolved {
                                Some(pv) => match pv {
                                    liquide_theme_css::value::PropertyValue::Keyword(k) => k,
                                    liquide_theme_css::value::PropertyValue::String(s) => s,
                                    other => format!("{}", other),
                                },
                                None => {
                                    if let Some(fb) = fallback {
                                        fb.to_string()
                                    } else {
                                        return None;
                                    }
                                }
                            }
                        } else {
                            s.clone()
                        }
                    }
                    _ => format!("{}", resolved),
                };
                result = format!("{}{}{}", &result[..start], replacement, &rest[end + 1..]);
            } else if let Some(fb) = fallback {
                // Fallback may itself contain var() references
                if fb.contains("var(") {
                    if let Some(resolved_fb) =
                        self.resolve_var_recursive(fb, scope_vars, resolution_stack)
                    {
                        let fb_str = match resolved_fb {
                            liquide_theme_css::value::PropertyValue::Keyword(k) => k,
                            liquide_theme_css::value::PropertyValue::String(s) => s,
                            other => format!("{}", other),
                        };
                        result = format!("{}{}{}", &result[..start], fb_str, &rest[end + 1..]);
                    } else {
                        return None;
                    }
                } else {
                    result = format!("{}{}{}", &result[..start], fb, &rest[end + 1..]);
                }
            } else {
                return None; // Variable not found, no fallback
            }
        }

        // ── env() resolution ──
        // CSS env() provides UA-defined environment variables.
        // We support: safe-area-inset-*, titlebar-area-*, keyboard-inset-*
        while let Some(start) = result.find("env(") {
            let rest = &result[start + 4..];
            let mut depth = 1i32;
            let mut end = 0;
            for (i, ch) in rest.char_indices() {
                match ch {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            end = i;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if depth != 0 {
                break;
            }

            let inner = &rest[..end];
            let (env_name, fallback) = if let Some(comma_pos) = Self::find_top_level_comma(inner) {
                (
                    inner[..comma_pos].trim(),
                    Some(inner[comma_pos + 1..].trim()),
                )
            } else {
                (inner.trim(), None)
            };

            let env_value = Self::resolve_env_variable(env_name);
            let replacement = if let Some(val) = env_value {
                val
            } else if let Some(fb) = fallback {
                fb.to_string()
            } else {
                "0px".to_string() // Default safe value
            };
            result = format!("{}{}{}", &result[..start], replacement, &rest[end + 1..]);
        }

        // Re-parse the resolved string
        Some(parse_inline_value(&result))
    }

    /// Resolve a CSS `env()` variable name to its value.
    /// Returns `None` for unknown variables (fallback will be used).
    fn resolve_env_variable(name: &str) -> Option<String> {
        match name {
            // Safe area insets (for notch/rounded corners) -- default to 0 for desktop
            "safe-area-inset-top"
            | "safe-area-inset-right"
            | "safe-area-inset-bottom"
            | "safe-area-inset-left" => Some("0px".into()),
            // Titlebar area (PWA window controls overlay)
            "titlebar-area-x" => Some("0px".into()),
            "titlebar-area-y" => Some("0px".into()),
            "titlebar-area-width" => Some("100%".into()),
            "titlebar-area-height" => Some("0px".into()),
            // Keyboard insets (virtual keyboard)
            "keyboard-inset-top"
            | "keyboard-inset-right"
            | "keyboard-inset-bottom"
            | "keyboard-inset-left"
            | "keyboard-inset-width"
            | "keyboard-inset-height" => Some("0px".into()),
            _ => None,
        }
    }

    /// Find the first top-level comma (not inside nested parens).
    pub(crate) fn find_top_level_comma(s: &str) -> Option<usize> {
        let mut depth = 0i32;
        for (i, ch) in s.char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => depth -= 1,
                ',' if depth == 0 => return Some(i),
                _ => {}
            }
        }
        None
    }
}
