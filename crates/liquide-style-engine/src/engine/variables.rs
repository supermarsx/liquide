//! CSS `var()` and `env()` variable resolution.

use super::StyleEngine;
use crate::dimension::Sides;
use crate::value_resolve::parse_inline_value;

/// Platform-provided environment values for CSS `env()` resolution.
///
/// All values default to zero, matching a standard desktop environment with no
/// safe-area insets, no virtual keyboard, and a zero-height titlebar area.
#[derive(Debug, Clone)]
pub struct EnvironmentValues {
    /// Safe-area insets (notch, rounded corners, etc.) in CSS px.
    pub safe_area_insets: Sides<f32>,
    /// Titlebar area geometry: (x, y, width, height) in CSS px.
    pub titlebar_area: (f32, f32, f32, f32),
    /// Virtual keyboard insets in CSS px.
    pub keyboard_insets: Sides<f32>,
    /// Virtual keyboard total width in CSS px.
    pub keyboard_inset_width: f32,
    /// Virtual keyboard total height in CSS px.
    pub keyboard_inset_height: f32,
}

impl Default for EnvironmentValues {
    fn default() -> Self {
        Self {
            safe_area_insets: Sides::default(),
            titlebar_area: (0.0, 0.0, 0.0, 0.0),
            keyboard_insets: Sides::default(),
            keyboard_inset_width: 0.0,
            keyboard_inset_height: 0.0,
        }
    }
}

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
        self.resolve_var_to_string(value, scope_vars)
            .as_deref()
            .map(parse_inline_value)
    }

    /// Resolve all `var()` / `env()` references and return the fully substituted
    /// value *string* (without collapsing it to a single `PropertyValue`).
    ///
    /// Callers that need multi-token shorthands (e.g. `box-shadow`) re-run the
    /// full property parser on this string instead of the single-value inline
    /// parser, which can only represent one number/color/keyword.
    pub(crate) fn resolve_var_to_string(
        &self,
        value: &str,
        scope_vars: &std::collections::HashMap<String, liquide_theme_css::value::PropertyValue>,
    ) -> Option<String> {
        let mut resolution_stack: Vec<String> = Vec::new();
        self.resolve_var_recursive(value, scope_vars, &mut resolution_stack)
    }

    fn resolve_var_recursive(
        &self,
        value: &str,
        scope_vars: &std::collections::HashMap<String, liquide_theme_css::value::PropertyValue>,
        resolution_stack: &mut Vec<String>,
    ) -> Option<String> {
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

            // Cycle detection: if this variable is already being resolved, it's circular.
            // Per CSS spec: cyclic references produce the guaranteed-invalid value.
            if resolution_stack.contains(&var_name.to_string()) {
                if let Some(fb) = fallback {
                    // Resolve fallback, but don't allow it to reference the cyclic variable
                    if fb.contains("var(") {
                        if let Some(fb_str) =
                            self.resolve_var_recursive(fb, scope_vars, resolution_stack)
                        {
                            result = format!("{}{}{}", &result[..start], fb_str, &rest[end + 1..]);
                        } else {
                            return None;
                        }
                    } else {
                        result = format!("{}{}{}", &result[..start], fb, &rest[end + 1..]);
                    }
                    continue;
                }
                return None;
            }

            if let Some(resolved) = scope_vars.get(var_name) {
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
                                Some(s) => s,
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
                                Some(rs) => rs,
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
                // Fallback may itself contain var() references.
                // Push var_name to resolution_stack to detect cycles through fallbacks.
                if fb.contains("var(") {
                    resolution_stack.push(var_name.to_string());
                    let resolved_fb = self.resolve_var_recursive(fb, scope_vars, resolution_stack);
                    resolution_stack.pop();
                    if let Some(fb_str) = resolved_fb {
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

            let env_value = self.resolve_env_variable(env_name);
            let replacement = if let Some(val) = env_value {
                val
            } else if let Some(fb) = fallback {
                fb.to_string()
            } else {
                "0px".to_string() // Default safe value
            };
            result = format!("{}{}{}", &result[..start], replacement, &rest[end + 1..]);
        }

        // Return the fully substituted string; the caller decides whether to
        // collapse it via the single-value inline parser or re-run the full
        // property parser (needed for multi-token shorthands like box-shadow).
        Some(result)
    }

    /// Resolve a CSS `env()` variable name to its value.
    /// Returns `None` for unknown variables (fallback will be used).
    fn resolve_env_variable(&self, name: &str) -> Option<String> {
        /// Format a pixel value, eliding the decimal when it's zero.
        fn px(v: f32) -> String {
            if v == 0.0 {
                "0px".into()
            } else if v.fract() == 0.0 {
                format!("{}px", v as i32)
            } else {
                format!("{:.2}px", v)
            }
        }

        let env = &self.env_values;
        match name {
            // Safe area insets (for notch/rounded corners)
            "safe-area-inset-top" => Some(px(env.safe_area_insets.top)),
            "safe-area-inset-right" => Some(px(env.safe_area_insets.right)),
            "safe-area-inset-bottom" => Some(px(env.safe_area_insets.bottom)),
            "safe-area-inset-left" => Some(px(env.safe_area_insets.left)),
            // Titlebar area (PWA window controls overlay)
            "titlebar-area-x" => Some(px(env.titlebar_area.0)),
            "titlebar-area-y" => Some(px(env.titlebar_area.1)),
            "titlebar-area-width" => Some(px(env.titlebar_area.2)),
            "titlebar-area-height" => Some(px(env.titlebar_area.3)),
            // Keyboard insets (virtual keyboard)
            "keyboard-inset-top" => Some(px(env.keyboard_insets.top)),
            "keyboard-inset-right" => Some(px(env.keyboard_insets.right)),
            "keyboard-inset-bottom" => Some(px(env.keyboard_insets.bottom)),
            "keyboard-inset-left" => Some(px(env.keyboard_insets.left)),
            "keyboard-inset-width" => Some(px(env.keyboard_inset_width)),
            "keyboard-inset-height" => Some(px(env.keyboard_inset_height)),
            _ => None,
        }
    }

    /// Update the platform-provided environment values used by CSS `env()`.
    pub fn set_environment_values(&mut self, values: EnvironmentValues) {
        self.env_values = values;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dimension::Sides;
    use crate::engine::ViewportSize;

    fn make_engine() -> StyleEngine {
        StyleEngine::new(ViewportSize::default(), 16.0)
    }

    fn assert_length_display(actual: &str, expected: &str) {
        assert!(
            actual == expected || actual == expected.trim_end_matches("px"),
            "expected {} (or canonical {}), got: {}",
            expected,
            expected.trim_end_matches("px"),
            actual
        );
    }

    #[test]
    fn env_defaults_to_zero() {
        let engine = make_engine();
        let vars = std::collections::HashMap::new();
        // safe-area-inset-top should resolve to 0px by default
        let result = engine.resolve_var_in_value("env(safe-area-inset-top)", &vars);
        assert!(result.is_some());
        let s = format!("{}", result.unwrap());
        assert_length_display(&s, "0px");
    }

    #[test]
    fn env_reads_platform_values() {
        let mut engine = make_engine();
        engine.set_environment_values(EnvironmentValues {
            safe_area_insets: Sides {
                top: 44.0,
                right: 0.0,
                bottom: 34.0,
                left: 0.0,
            },
            titlebar_area: (10.0, 0.0, 500.0, 32.0),
            keyboard_insets: Sides {
                top: 0.0,
                right: 0.0,
                bottom: 300.0,
                left: 0.0,
            },
            keyboard_inset_width: 360.0,
            keyboard_inset_height: 300.0,
        });
        let vars = std::collections::HashMap::new();

        let top = engine
            .resolve_var_in_value("env(safe-area-inset-top)", &vars)
            .unwrap();
        assert_length_display(&format!("{}", top), "44px");

        let bottom = engine
            .resolve_var_in_value("env(safe-area-inset-bottom)", &vars)
            .unwrap();
        assert_length_display(&format!("{}", bottom), "34px");

        let tw = engine
            .resolve_var_in_value("env(titlebar-area-width)", &vars)
            .unwrap();
        assert_length_display(&format!("{}", tw), "500px");

        let th = engine
            .resolve_var_in_value("env(titlebar-area-height)", &vars)
            .unwrap();
        assert_length_display(&format!("{}", th), "32px");

        let kb = engine
            .resolve_var_in_value("env(keyboard-inset-bottom)", &vars)
            .unwrap();
        assert_length_display(&format!("{}", kb), "300px");

        let kw = engine
            .resolve_var_in_value("env(keyboard-inset-width)", &vars)
            .unwrap();
        assert_length_display(&format!("{}", kw), "360px");
    }

    #[test]
    fn env_unknown_uses_fallback() {
        let engine = make_engine();
        let vars = std::collections::HashMap::new();
        let result = engine.resolve_var_in_value("env(unknown-thing, 10px)", &vars);
        assert!(result.is_some());
        let s = format!("{}", result.unwrap());
        assert_length_display(&s, "10px");
    }

    #[test]
    fn env_unknown_no_fallback_returns_zero() {
        let engine = make_engine();
        let vars = std::collections::HashMap::new();
        let result = engine.resolve_var_in_value("env(unknown-thing)", &vars);
        assert!(result.is_some());
        let s = format!("{}", result.unwrap());
        assert_length_display(&s, "0px");
    }
}
