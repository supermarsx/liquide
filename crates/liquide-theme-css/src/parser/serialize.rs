//! CSS value serialization helpers.
//!
//! Converts lightningcss types to plain strings using the `ToCss` trait
//! and handles custom token list serialization (var(), env(), functions).

use crate::error::{Result, ThemeError};

use lightningcss::printer::Printer;
use lightningcss::stylesheet::PrinterOptions;
use lightningcss::traits::ToCss;

use super::ThemeParser;

impl ThemeParser {
    /// Convert lightningcss selector to string.
    pub(crate) fn selector_to_string(
        &self,
        selector: &lightningcss::selector::Selector<'_>,
    ) -> Result<String> {
        let mut css_string = String::new();
        let mut printer = Printer::new(&mut css_string, PrinterOptions::default());
        selector
            .to_css(&mut printer)
            .map_err(|e| ThemeError::ParseError {
                message: format!("Failed to serialize selector: {:?}", e),
                location: "selector".to_string(),
            })?;
        Ok(css_string)
    }

    /// Serialize any `ToCss` value to string.
    pub(crate) fn to_css_string<T: ToCss>(&self, value: &T) -> String {
        let mut s = String::new();
        let mut printer = Printer::new(&mut s, PrinterOptions::default());
        let _ = value.to_css(&mut printer);
        s
    }

    /// Serialize a `TokenList` to string by iterating its public token vector.
    pub(crate) fn to_css_string_from_token_list(
        &self,
        tokens: &lightningcss::properties::custom::TokenList,
    ) -> String {
        use lightningcss::properties::custom::TokenOrValue;
        let mut result = String::new();
        for token_or_value in &tokens.0 {
            match token_or_value {
                TokenOrValue::Color(color) => {
                    result.push_str(&self.to_css_string(color));
                }
                TokenOrValue::Length(length) => {
                    result.push_str(&self.to_css_string(length));
                }
                TokenOrValue::Angle(angle) => {
                    result.push_str(&self.to_css_string(angle));
                }
                TokenOrValue::Time(time) => {
                    result.push_str(&self.to_css_string(time));
                }
                TokenOrValue::Resolution(res) => {
                    result.push_str(&self.to_css_string(res));
                }
                TokenOrValue::Token(token) => {
                    result.push_str(&self.to_css_string(token));
                }
                TokenOrValue::Var(var) => {
                    // Serialize var() properly: var(--name) or var(--name, fallback)
                    result.push_str("var(");
                    result.push_str(&var.name.ident.0);
                    if let Some(fallback) = &var.fallback {
                        result.push_str(", ");
                        result.push_str(&self.to_css_string_from_token_list(fallback));
                    }
                    result.push(')');
                }
                TokenOrValue::Env(env) => {
                    result.push_str("env(");
                    result.push_str(&self.to_css_string(&env.name));
                    if let Some(fallback) = &env.fallback {
                        result.push_str(", ");
                        result.push_str(&self.to_css_string_from_token_list(fallback));
                    }
                    result.push(')');
                }
                TokenOrValue::Function(func) => {
                    result.push_str(&func.name);
                    result.push('(');
                    result.push_str(&self.to_css_string_from_token_list(&func.arguments));
                    result.push(')');
                }
                TokenOrValue::DashedIdent(ident) => {
                    result.push_str(&ident.0);
                }
                _ => {
                    // UnresolvedColor, Url, AnimationName, etc.
                    result.push_str(&format!("{:?}", token_or_value));
                }
            }
        }
        result.trim().to_string()
    }
}
